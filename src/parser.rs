use super::PERMUTATION_LIMIT;
use crate::constants::*;

use std::borrow::Cow;
use std::cmp::max;
use std::mem::replace;
use std::ops::Range;

type StepError = String;
type PassOutput<'a> = Result<(), StepError>;

pub struct PermutationsGenerator<'a> {
    //entries: Vec<
    head_calculator_memory: Vec<usize>,
    body_calculator_memory: Vec<usize>,

    partitioning: Vec<(usize, usize)>,
    chords_memory: Vec<Chord>,
    action_memory: Vec<Cow<'a, str>>, // Dealing with escaping with owned data
}

pub fn parse_into_shortcut_list(
    first_pass: EntryBlobMetadata,
) -> Result<PermutationsGenerator, StepError> {
    // This is basically a lexer
    // Validate the format and calculates the sizes of allocations
    // We still do not pre-calculate the necessary number of chord allocations

    let head_variant_total_count: usize = first_pass
        .entries
        .iter()
        .map(|entry| entry.permutation_count)
        .sum();

    let mut hc_mem = vec![0; first_pass.max_head_set_count * 3];
    let mut bc_mem = vec![0; first_pass.max_body_set_count * 3];
    let mut partitioning = Vec::with_capacity(head_variant_total_count);
    let mut chords_memory = Vec::new(); // TODO: calculate this capacity
    let mut body_memory = Vec::with_capacity(first_pass.total_body_space);

    for UnparsedEntry {
        row: _row,
        head,
        head_set_count,
        body,
        body_set_count,
        permutation_count,
        ..
    } in first_pass.entries
    {
        let mut head_calc = Calculator::new(head, head_set_count, &mut hc_mem);
        let mut body_calc = Calculator::new(body, body_set_count, &mut bc_mem);

        for i in 0..permutation_count {
            let chord_count =
                push_head_variant(&mut chords_memory, head, head_calc.permute(i)).unwrap();
            //let action_mem_width = body_set_count * 2 + 1;
            let action_mem_width =
                push_body_variant(&mut body_memory, body.trim(), body_calc.permute(i));
            partitioning.push((chord_count, action_mem_width));
        }
    }
    Ok(PermutationsGenerator {
        head_calculator_memory: hc_mem,
        body_calculator_memory: bc_mem,

        partitioning,
        chords_memory,
        action_memory: body_memory,
    })
}

impl<'a> PermutationsGenerator<'a> {
    // Easier for debugging
    fn allocate_unsorted_unchecked_shortcut_list<'b>(
        &'b self,
    ) -> Result<Vec<Shortcut<'a, 'b>>, StepError> {
        let len = self.partitioning.len();

        let mut shortcut_list = Vec::with_capacity(len);
        let mut chords_buffer = &self.chords_memory[..];
        let mut action_buffer = &self.action_memory[..];

        for (chords_count, action_width) in &self.partitioning {
            let hotkey = &chords_buffer[0..*chords_count];
            let action = &action_buffer[0..*action_width];
            chords_buffer = &chords_buffer[*chords_count..];
            action_buffer = &action_buffer[*action_width..];
            shortcut_list.push(Shortcut {
                hotkey: Hotkey(hotkey),
                action,
            });
        }
        debug_assert_eq!(
            chords_buffer.len(),
            0,
            "Did not fully consume 'chords_buffer'"
        );
        debug_assert_eq!(
            action_buffer.len(),
            0,
            "Did not fully consume 'action_buffer'"
        );

        Ok(shortcut_list)
    }

    // Sorted and validated 'shorcut_list'
    pub fn allocate_shortcut_list<'b>(&'b self) -> Result<Vec<Shortcut<'a, 'b>>, StepError> {
        // TODO: unit test for duplicates
        let mut shortcut_list = self.allocate_unsorted_unchecked_shortcut_list()?;
        shortcut_list.sort_unstable();
        for i in 0..shortcut_list.len() - 1 {
            let chord_list1 = &shortcut_list[i].hotkey.0;
            let chord_list2 = &shortcut_list[i + 1].hotkey.0;
            let len = std::cmp::min(chord_list1.len(), chord_list2.len());
            if chord_list1[0..len] == chord_list2[0..len] {
                return Err(format!(
                    "Duplicate keys {} and {}",
                    shortcut_list[i].hotkey,
                    shortcut_list[i + 1].hotkey,
                ));
            }
        }
        Ok(shortcut_list)
    }
}

#[derive(Debug)]
enum State {
    Head,
    HeadBrackets,
    Body,
    BodyBrackets,
}

#[derive(Debug)]
struct UnparsedEntry<'a> {
    head: &'a str,
    body: &'a str,
    head_set_count: usize,
    body_set_count: usize,
    permutation_count: usize,
    row: usize,
}

impl<'a> UnparsedEntry<'a> {
    fn new(text: &'a str, row: usize) -> Self {
        Self {
            head: text,
            body: text,
            head_set_count: 0,
            body_set_count: 0,
            permutation_count: 1,
            row,
        }
    }
}

#[derive(Debug)]
pub struct EntryBlobMetadata<'a> {
    entries: Vec<UnparsedEntry<'a>>,
    max_head_set_count: usize,
    max_body_set_count: usize,
    //total_head_space: usize,
    total_body_space: usize,
}
impl<'a> EntryBlobMetadata<'a> {
    fn new(after_first_pipe: &'a str) -> Self {
        Self {
            entries: Vec::with_capacity(after_first_pipe.split("\n|").count()),
            max_head_set_count: 0,
            max_body_set_count: 0,
            //total_head_space: 0,
            total_body_space: 0,
        }
    }

    fn push_entry(
        &mut self,
        body_permutation_count: usize,
        entry: UnparsedEntry<'a>,
    ) -> Result<(), StepError> {
        if body_permutation_count > entry.permutation_count {
            //println!("{:?}\n{:?}", entry.head, entry.body);
            //println!("head count {:?}", entry.permutation_count);
            //println!("body count {:?}", entry.permutation_count);
            Err(
                "This body for (TODO) needs more options than there are hotkey permutations for"
                    .into(),
            )
        } else {
            self.max_head_set_count = max(self.max_head_set_count, entry.head_set_count);
            self.max_body_set_count = max(self.max_body_set_count, entry.body_set_count);
            //self.total_head_space += (entry.head_set_count * 2 + 1) * entry.permutation_count;
            self.total_body_space += (entry.body_set_count * 2 + 1) * entry.permutation_count;
            self.entries.push(entry);
            Ok(())
        }
    }
}

// TODO: test when body has more sets than head
// e.g. |{{a,b,c}};{{1,2,3,4}}| {{a,b}} {{e, f}} {{g,h}}
// This has 12 vs 8 permutations, the last 4 permutations will all have the
// same body variant but
// The reverse case (more body variants) than

struct FiniteStateMachine<'a> {
    original: &'a str,
    walker: CharsWithIndex<'a>,
    state: State,

    key_start_index: usize,
    head_set_size: usize,
    body_set_size: usize,
    entry_body_permutation_count: usize,
    hotkeys_count: usize,
    actions_count: usize,

    entry: UnparsedEntry<'a>,
    metadata: EntryBlobMetadata<'a>,
}

pub fn validate_and_calculate_allocations(source: &str) -> Result<EntryBlobMetadata, String> {
    let (text, start_row) = FiniteStateMachine::step_init_until_first(source)?;
    let mut fsm = FiniteStateMachine {
        original: text,
        walker: CharsWithIndex::new(text, start_row),
        state: State::Head,

        key_start_index: 0,
        head_set_size: 0,
        body_set_size: 0,
        entry_body_permutation_count: 0,
        hotkeys_count: 0,
        actions_count: 0,

        entry: UnparsedEntry::new(text, start_row),
        metadata: EntryBlobMetadata::new(text),
    };

    while let Some(ch) = fsm.walker.next() {
        match fsm.state {
            State::Head => fsm.step_head(ch)?,
            State::HeadBrackets => fsm.step_head_brackets(ch)?,
            State::Body => fsm.step_body(ch)?, // This may push
            State::BodyBrackets => fsm.step_body_brackets(ch)?,
        };
    }
    if let State::HeadBrackets | State::BodyBrackets = fsm.state {
        return Err("Brackets not closed. Expected a '}}'".into());
    }
    let last = fsm.entry;
    if !last.head.is_empty() {
        fsm.metadata
            .push_entry(fsm.entry_body_permutation_count, last)?;
    }
    Ok(fsm.metadata)
}

impl<'a> FiniteStateMachine<'a> {
    fn step_init_until_first(source: &str) -> Result<(&str, usize), StepError> {
        let mut row = 0;
        let mut start = source.len();
        for line in source.lines() {
            row += 1;
            if let Some('|') = line.chars().next() {
                let one = 'l'.len_utf8();
                start = line.as_ptr() as usize - source.as_ptr() as usize + one;
                break;
            }
            match line.trim_start().chars().next() {
                Some('#') => {}
                Some(_) => return Err("Lines can only be a comment (first non-whitespace character is '#') or whitespace before the first entry (first character in line is '|')".into()),
                None => {}
            }
        }

        Ok((&source[start..], row))
    }

    #[inline]
    fn step_head(&mut self, ch: char) -> PassOutput {
        match ch {
            '|' => {
                let base = self.original.as_ptr() as usize;
                let offset = self.entry.head.as_ptr() as usize - base;
                self.entry.head = &self.original[offset..self.walker.prev];
                self.entry.body = &self.original[self.walker.post..];
                self.change_state(State::Body)?; // Call last
                                                 //println!("==={:?}===\n{:?}", self.entry.head, self.entry.body);
            }
            '{' => {
                if let Some('{') = self.walker.next() {
                    // Want these three things on
                    self.change_state(State::HeadBrackets)?; // Call last
                } else {
                    return Err(
                        "Missing a second opening curly brace. Need '{{' to start an enumeration"
                            .into(),
                    );
                }
            }
            ',' => return Err("Unexpected comma ','. Type 'comma' for the key, ';' for a chord separator. ',' only has meaning inside an enumeration group '{{..}}'".into()),
            ';' => {
                self.walker.eat_separator();
                self.key_start_index = self.walker.post;
            }
            _ if SEPARATOR.contains(&ch) => {
                self.walker.eat_separator();
                self.key_start_index = self.walker.post;
            }
            _ => {
                let start = self.key_start_index;
                let key = &self.original[start..self.walker.post];
                if key.len() > KEYSTR_MAX_LEN {
                    panic!("Invalid keycode {:?}", key);
                }
                // Key validation check will happen when we parse the key
                // so we do since we allocate at that time
            }
        }
        Ok(())
    }

    #[inline]
    fn step_head_brackets(&mut self, ch: char) -> PassOutput {
        match ch {
            '|' => return Err("Unexpected bar '|'. Close the enumeration first with '}}'".into()),
            '\\' => {
                return Err("You cannot escape characters with backslash '\\' in the hotkey definition portion".into());
            }
            ',' => self.head_set_member(),
            '}' => {
                if let Some('}') = self.walker.next() {
                    self.change_state(State::Head)?; // Call last
                } else {
                    return Err(
                        "Missing a second closing curly brace. Need '}}' to close an enumeration"
                            .into(),
                    );
                }
            }
            _ if SEPARATOR.contains(&ch) => {
                self.walker.eat_whitespace();
                self.key_start_index = self.walker.post;
            }
            _ => {}
        }
        Ok(())
    }

    #[inline]
    fn step_body(&mut self, ch: char) -> PassOutput {
        match (ch, self.walker.peek()) {
            ('\n', Some('|')) => {
                self.walker.next();
                let base = self.original.as_ptr() as usize;
                let offset = self.entry.body.as_ptr() as usize - base;
                self.entry.body = &self.original[offset..self.walker.prev];
                //println!("==={}===\n{:?}", self.entry.head, self.entry.body);

                let new_entry =
                    UnparsedEntry::new(&self.original[self.walker.post..], self.walker.row);
                self.metadata.push_entry(
                    self.entry_body_permutation_count,
                    replace(&mut self.entry, new_entry),
                )?;

                self.change_state(State::Head)?; // Call last
            }
            ('{', Some('{')) => self.change_state(State::BodyBrackets)?, // Call last
            _ => {}
        }
        Ok(())
    }

    #[inline]
    fn step_body_brackets(&mut self, ch: char) -> PassOutput {
        match ch {
            '\\' => {
                self.walker.next();
            }
            ',' => self.body_set_member(),
            '}' => {
                if let Some('}') = self.walker.next() {
                    self.change_state(State::Body)?; // Call last
                } else {
                    return Err("Missing a second closing curly brace. Need '}}' to close. If you want a '}' as output, escape it with backslash like '\\}'".into());
                }
            }
            _ => {}
        }
        Ok(())
    }

    #[inline]
    fn head_set_start(&mut self) {
        self.walker.eat_separator();
        self.key_start_index = self.walker.post;
        self.head_set_size = 0;
    }

    #[inline]
    fn head_set_member(&mut self) {
        self.walker.eat_separator();
        self.key_start_index = self.walker.post;
        self.head_set_size += 1;
    }

    #[inline]
    fn head_set_close(&mut self) -> Result<(), StepError> {
        self.head_set_size += 1;
        self.entry.permutation_count *= self.head_set_size;
        self.entry.head_set_count += 1;
        //println!("group_end {:?}", self.entry.permutation_count, )
        if self.entry.permutation_count > PERMUTATION_LIMIT {
            Err("Too many permutations for <line>".into())
        } else {
            Ok(())
        }
    }

    #[inline]
    fn body_set_start(&mut self) {
        self.body_set_size = 0;
    }

    #[inline]
    fn body_set_member(&mut self) {
        self.body_set_size += 1;
    }
    #[inline]
    fn body_set_close(&mut self) {
        self.body_set_member(); // adds to 'self.body_set_size'
        self.entry_body_permutation_count *= self.body_set_size;
        self.entry.body_set_count += 1;
    }

    fn change_state(&mut self, target: State) -> Result<(), StepError> {
        // From 'self.state' to 'target'
        match (&self.state, &target) {
            (_, State::HeadBrackets) => self.head_set_start(),
            (State::HeadBrackets, _) => self.head_set_close()?,

            (_, State::BodyBrackets) => self.body_set_start(),
            (State::BodyBrackets, _) => self.body_set_close(),

            (_, State::Head) => {
                self.walker.eat_separator();
                self.key_start_index = self.walker.post;
            }

            // TODO: Maybe change to compile-time state transition validation
            // See 'pretty state machines' blog post
            _ => {} // Maybe panic on invalid transitions? Kind of unnecessary
        }
        self.state = target;
        Ok(())
    }
}

#[inline]
fn peek_while<T, F>(iter: &mut std::iter::Peekable<T>, mut predicate: F)
where
    T: Iterator,
    F: FnMut(&T::Item) -> bool,
{
    while let Some(item) = iter.peek() {
        if !predicate(item) {
            break;
        }
        iter.next();
    }
}

//#[inline]
//fn next_until<T, F>(iter: &mut T, mut predicate: F)
//    where T: Iterator,
//          F: FnMut(T::Item) -> bool
//{
//    while let Some(item) = iter.next() {
//        if predicate(item) {
//            break;
//        }
//    }
//
//}
fn push_head_variant(
    chord_memory: &mut Vec<Chord>,
    head: &str,
    permutation: &[usize],
) -> Result<usize, String> {
    fn push_chord(
        chords: &mut Vec<Chord>,
        key: &mut Option<Key>,
        modifiers: &mut Modifiers,
    ) -> Result<(), StepError> {
        if let Some(code) = std::mem::take(key) {
            chords.push(Chord {
                key: code,
                modifiers: replace(modifiers, 0),
            });
            Ok(())
        } else {
            Err("No key set".into())
        }
    };

    let mut walker = DelimSplit::new(head, 1, head_lexer).peekable();
    let mut set_index = 0;

    let mut modifiers = 0;
    let mut key = None;
    let mut chord_count = 0;
    while let Some((field, _, _row)) = walker.next() {
        match field {
            "{{" => {
                let mut pos = 0;
                let choice = permutation[set_index];
                peek_while(&mut walker, |(peek, _, _)| {
                    if pos >= choice {
                        false
                    } else {
                        if *peek == "," {
                            pos += 1;
                        }
                        true
                    }
                });
            }
            // 'first_pass()' ensures ',' is never outside of '{{..}}'
            "," => peek_while(&mut walker, |(field, _, _)| *field != "}}"),
            "}}" => set_index += 1,
            ";" => {
                chord_count += 1;
                push_chord(chord_memory, &mut key, &mut modifiers)?;
            }

            "shift" => modifiers |= Mod::Shift as Modifiers,
            "super" => modifiers |= Mod::Super as Modifiers,
            "ctrl" => modifiers |= Mod::Ctrl as Modifiers,
            "alt" => modifiers |= Mod::Alt as Modifiers,

            _ if key.is_some() => panic!("Key already defined"),
            _ => {
                if let Some(i) = KEYSTRS.iter().position(|x| *x == field) {
                    key = Some(KEYCODES[i].clone());
                } else {
                    return Err(format!("Key {:?} not found", field));
                }
            }
        }
    }
    chord_count += 1;
    push_chord(chord_memory, &mut key, &mut modifiers)?;
    Ok(chord_count)
}

fn head_lexer(substr: &str) -> Range<usize> {
    let mut chars = substr.chars();
    let mut delim_start = 0;
    let mut delim_close = 0;

    while let Some(ch) = chars.next() {
        delim_close += ch.len_utf8(); // represents post index
                                      // At this point, `ch == &substr[delim_start..delim_close]`
        match ch {
            '{' | '}' if delim_start == 0 => {
                chars.next();
                delim_close += '}'.len_utf8();
                delim_start = delim_close;
                break;
            }
            '{' | '}' => return delim_start..delim_start,
            ',' | ';' if delim_start == 0 => {
                delim_start = delim_close;
                break;
            }
            ',' | ';' => return delim_start..delim_start,
            _ if SEPARATOR.contains(&ch) => break,
            _ => delim_start = delim_close, // represents prev index
        }
    }

    // Eat separators
    for ch in chars {
        match ch {
            _ if !SEPARATOR.contains(&ch) => break,
            _ => {}
        }
        // Although this is a post-index, add after to simulate 'chars.peek()'
        delim_close += ch.len_utf8(); // Post last separator
    }
    delim_start..delim_close
}

#[derive(Debug)]
struct Calculator<'b> {
    permutation: &'b mut [usize],
    set_sizes: &'b mut [usize],
    digit_values: &'b mut [usize],
}
impl<'b> Calculator<'b> {
    fn new(source: &str, set_count: usize, memory: &'b mut [usize]) -> Self {
        let (permutation, rest) = memory.split_at_mut(set_count);
        let (set_sizes, rest) = rest.split_at_mut(set_count);
        let (digit_values, _) = rest.split_at_mut(set_count);
        if set_count > 0 {
            // split of non-blank is minimum 'len()' 1
            // Splits into regular keys and optional (enumerated) keys
            let reg_opt_pairs = DelimSplit::new(source, 1, split_brackets);
            for (i, (_, brackets, _)) in reg_opt_pairs.enumerate() {
                if !brackets.is_empty() {
                    // Must be at least "{{}}"
                    set_sizes[i] = brackets.split(',').count();
                }
            }
            let mut product = 1;
            for (i, total) in set_sizes.iter().enumerate().rev() {
                digit_values[i] = product;
                product *= *total;
            }
        }
        Calculator {
            permutation,
            set_sizes,
            digit_values,
        }
    }

    fn permute(&mut self, permutation_index: usize) -> &[usize] {
        for i in 0..self.permutation.len() {
            let x = permutation_index / self.digit_values[i];
            self.permutation[i] = x % self.set_sizes[i];
        }
        &self.permutation
    }
}

fn push_body_variant<'a>(memory: &mut Vec<Cow<'a, str>>, body: &'a str, permutation: &[usize]) -> usize {
    if body.is_empty() {
        memory.push(body.into());
        return 1;
    }
    let mut items_pushed = 0;
    let mut buffer = String::new();
    let split = DelimSplit::new(body, 1, split_brackets);
    for (set_index, (regular, delim, _row)) in split.enumerate() {
        memory.push(regular.into());
        items_pushed += 1;

        buffer.clear();
        let delim = if delim.is_empty() {
            delim
        } else {
            buffer.reserve(delim.len() - "{{}}".len());
            &delim["{{".len()..]
        };

        // Basically a `delim.split(',')` but with escaping backslash
        // Additionally escaped newlines are ignored (similar to shellscript)
        // Push the delim when we get to the correct field
        let mut walker = delim.chars().peekable();
        let mut start = 0;
        let mut until = start;
        let mut field_index = 0;
        while let Some(ch) = walker.next() {
            match ch {
                '\\' => {
                    buffer.push_str(&delim[start..until]);
                    let escaped = walker.next().unwrap();
                    if escaped != '\n' {
                        buffer.push(escaped); // Special case escaped newline
                    }
                    until += '\\'.len_utf8() + escaped.len_utf8();
                    start = until;
                }
                ',' | '}' => {
                    if field_index == permutation[set_index] {
                        buffer.push_str(&delim[start..until]);
                        memory.push(buffer.split_off(0).into());
                        items_pushed += 1;
                        break;
                    }
                    debug_assert_eq!(','.len_utf8(), '}'.len_utf8());
                    start = until + ','.len_utf8();
                    until = start;
                    field_index += 1;
                    buffer.clear();
                }
                c => until += c.len_utf8(),
            }
        }
        //println!("{:?} {:?}", regular, brackets);
    }
    items_pushed
}

/******************************************************************************
 * A 'std::str::Chars' wrapper for use in 'first_pass()'
 ******************************************************************************/
struct CharsWithIndex<'a> {
    pub(self) iter: std::iter::Peekable<std::str::Chars<'a>>,
    prev: usize,
    post: usize,
    row: usize,
    col: usize,
    last_char: char,
}
impl<'a> CharsWithIndex<'a> {
    fn new(text: &'a str, start_row: usize) -> Self {
        let last_char = ' ';
        debug_assert!(last_char != '\n');
        Self {
            iter: text.chars().peekable(),
            prev: 0,
            post: 0,
            row: start_row,
            col: 0,
            last_char,
        }
    }

    #[inline]
    fn peek(&mut self) -> Option<&<Self as Iterator>::Item> {
        self.iter.peek()
    }

    fn eat_whitespace(&mut self) {
        while let Some(peek) = self.iter.peek() {
            if peek.is_whitespace() {
                self.next();
            } else {
                break;
            }
        }
    }

    fn eat_separator(&mut self) {
        while let Some(peek) = self.iter.peek() {
            if SEPARATOR.contains(peek) {
                self.next();
            } else {
                break;
            }
        }
    }
}

//
impl<'a> Iterator for CharsWithIndex<'a> {
    type Item = char;
    fn next(&mut self) -> Option<<Self as Iterator>::Item> {
        if let Some(c) = self.iter.next() {
            // This is sound in the first '.next()' case
            // (prev, post) => (0, 0).next() -> (0, 1)
            self.prev = self.post;
            self.post += c.len_utf8();

            self.col += 1;
            if self.last_char == '\n' {
                self.row += 1;
                self.col = 1;
            }
            self.last_char = c;

            Some(c)
        } else {
            self.prev = self.post;
            None
        }
    }
}

#[test]
fn chars_with_index() {
    let mut iter = CharsWithIndex::new("a", 1);
    assert_eq!(iter.next(), Some('a'));
    assert_eq!(iter.next(), None);
    assert_eq!(iter.next(), None);

    let mut iter = CharsWithIndex::new("", 1);
    assert_eq!(iter.next(), None);
    assert_eq!(iter.next(), None);

    let mut iter = CharsWithIndex::new("你m好!!我是y只mao", 1);
    assert_eq!(iter.next(), Some('你'));
    assert_eq!(iter.next(), Some('m'));
    assert_eq!(iter.next(), Some('好'));
    assert_eq!(iter.next(), Some('!'));
    assert_eq!(iter.next(), Some('!'));
    assert_eq!(iter.next(), Some('我'));
    assert_eq!(iter.next(), Some('是'));
    assert_eq!(iter.next(), Some('y'));
    assert_eq!(iter.next(), Some('只'));
    assert_eq!(iter.next(), Some('m'));
    assert_eq!(iter.next(), Some('a'));
    assert_eq!(iter.next(), Some('o'));
    assert_eq!(iter.next(), None);
    assert_eq!(iter.next(), None);

    let source = "你m好!!我是y只mao";
    let mut iter = CharsWithIndex::new(source, 1);
    while let Some(c) = iter.next() {
        assert_eq!(&c.to_string(), &source[iter.prev..iter.post]);
    }

    // TODO: test peek and eat_whitespace
    //let mut iter = CharsWithIndex::new("你m好!!我", 1);
}

/******************************************************************************
 * A 'std::str::Chars' wrapper for use in 'first_pass()'
 ******************************************************************************/

// Split with delimiter of '{{..}}'
// Backslash escaping is allowed within the delimiter
fn split_brackets(substr: &str) -> Range<usize> {
    let len = substr.len();
    let (start, mut close) = if let Some(i) = substr.find("{{") {
        (i, i + "{{".len())
    } else {
        (len, len)
    };
    //if start > substr.find("}}").unwrap_or(len) {
    //    panic!("DEV: Validation did not catch '}}' found without an opening '{{'");
    //}

    let mut chars = substr[close..].chars();
    while let Some(ch) = chars.next() {
        close += ch.len_utf8();
        match ch {
            '\\' => {
                close += chars.next().map(|c| c.len_utf8()).unwrap_or(0);
            }
            '}' => {
                if let Some(c) = chars.next() {
                    close += c.len_utf8();
                    if c == '}' {
                        break;
                    }
                }
            }
            _ => {}
        }
    }
    start..close
}

struct DelimSplit<'a> {
    buffer: &'a str,
    row: usize,
    delimit_by: fn(&str) -> Range<usize>,
}
impl<'a> DelimSplit<'a> {
    fn new(s: &'a str, start_row: usize, f: fn(&str) -> Range<usize>) -> Self {
        Self {
            buffer: s,
            row: start_row,
            delimit_by: f,
        }
    }
}

impl<'a> Iterator for DelimSplit<'a> {
    type Item = (&'a str, &'a str, usize);
    fn next(&mut self) -> Option<Self::Item> {
        let rel_delim = (self.delimit_by)(self.buffer);
        let buffer_len = self.buffer.len();
        if buffer_len > 0 {
            let field = &self.buffer[0..rel_delim.start];
            let delimiter = &self.buffer[rel_delim.start..rel_delim.end];
            let row = self.row;
            self.row += field.lines().count();
            self.buffer = &self.buffer[rel_delim.end..];
            Some((field, delimiter, row))
        } else {
            None
        }
    }
}
