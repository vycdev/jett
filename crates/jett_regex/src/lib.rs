//! Portable regex pattern parsing and canonical automaton preflight for Jett.

use std::collections::BTreeSet;

use unicode_segmentation::UnicodeSegmentation;

pub const PATTERN_GRAPHEME_LIMIT: usize = 4_096;
pub const CAPTURE_LIMIT: u8 = 64;
pub const REPETITION_LIMIT: u64 = 1_000_000;
pub const COMPILED_STATE_LIMIT: u64 = 65_536;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Flags {
    pub case_insensitive: bool,
    pub multi_line: bool,
    pub dot_matches_line_endings: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompiledPattern {
    flags: Flags,
    capture_count: u8,
    named_captures: Vec<(String, u8)>,
    state_count: u64,
}

impl CompiledPattern {
    pub fn flags(&self) -> Flags {
        self.flags
    }

    pub fn capture_count(&self) -> u8 {
        self.capture_count
    }

    pub fn named_captures(&self) -> &[(String, u8)] {
        &self.named_captures
    }

    pub fn state_count(&self) -> u64 {
        self.state_count
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvalidPattern {
    pub position: usize,
    pub message: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompileError {
    InvalidPattern(InvalidPattern),
    PatternTooLarge { limit: usize },
    CompiledPatternTooLarge { limit: u64 },
}

pub fn compile_pattern(pattern: &str) -> Result<CompiledPattern, CompileError> {
    let grapheme_count = UnicodeSegmentation::graphemes(pattern, true).count();
    if grapheme_count > PATTERN_GRAPHEME_LIMIT {
        return Err(CompileError::PatternTooLarge {
            limit: PATTERN_GRAPHEME_LIMIT,
        });
    }

    let mut parser = Parser::new(pattern, grapheme_count);
    let flags = parser.parse_flags()?;
    let body_states = parser.parse_alternation()?;
    if !parser.at_end() {
        return Err(parser.invalid_here("unexpected token"));
    }
    let state_count = parser.add_states(body_states, 1)?;
    if state_count > COMPILED_STATE_LIMIT {
        return Err(CompileError::CompiledPatternTooLarge {
            limit: COMPILED_STATE_LIMIT,
        });
    }

    Ok(CompiledPattern {
        flags,
        capture_count: parser.capture_count,
        named_captures: parser.named_captures,
        state_count,
    })
}

struct Parser {
    chars: Vec<char>,
    scalar_to_grapheme: Vec<usize>,
    cursor: usize,
    capture_count: u8,
    capture_names: BTreeSet<String>,
    named_captures: Vec<(String, u8)>,
}

impl Parser {
    fn new(pattern: &str, grapheme_count: usize) -> Self {
        let mut scalar_to_grapheme = Vec::new();
        for (index, grapheme) in UnicodeSegmentation::graphemes(pattern, true).enumerate() {
            scalar_to_grapheme.extend(std::iter::repeat_n(index, grapheme.chars().count()));
        }
        scalar_to_grapheme.push(grapheme_count);
        Self {
            chars: pattern.chars().collect(),
            scalar_to_grapheme,
            cursor: 0,
            capture_count: 0,
            capture_names: BTreeSet::new(),
            named_captures: Vec::new(),
        }
    }

    fn parse_flags(&mut self) -> Result<Flags, CompileError> {
        if !self.starts_with("(?") || matches!(self.peek_n(2), Some(':' | 'P')) {
            return Ok(Flags::default());
        }
        if !self.peek_n(2).is_some_and(|ch| ch.is_ascii_lowercase()) {
            return Ok(Flags::default());
        }

        let start = self.cursor;
        let mut end = 2;
        while self.peek_n(end).is_some_and(|ch| ch.is_ascii_lowercase()) {
            end += 1;
        }
        if self.peek_n(end) != Some(')') && self.peek_n(end).is_some() {
            return Ok(Flags::default());
        }
        let spelling: String = self.chars[start + 2..start + end].iter().collect();
        let flags = match spelling.as_str() {
            "i" => Flags {
                case_insensitive: true,
                ..Flags::default()
            },
            "m" => Flags {
                multi_line: true,
                ..Flags::default()
            },
            "s" => Flags {
                dot_matches_line_endings: true,
                ..Flags::default()
            },
            "im" => Flags {
                case_insensitive: true,
                multi_line: true,
                ..Flags::default()
            },
            "is" => Flags {
                case_insensitive: true,
                dot_matches_line_endings: true,
                ..Flags::default()
            },
            "ms" => Flags {
                multi_line: true,
                dot_matches_line_endings: true,
                ..Flags::default()
            },
            "ims" => Flags {
                case_insensitive: true,
                multi_line: true,
                dot_matches_line_endings: true,
            },
            _ => return Err(self.invalid_at(start, "flag group must be leading and canonical")),
        };
        if self.peek_n(end).is_none() {
            return Err(self.invalid_at(start, "flag group must be leading and canonical"));
        }
        self.cursor += end + 1;
        Ok(flags)
    }

    fn parse_alternation(&mut self) -> Result<u64, CompileError> {
        // Legal noncapturing nesting can exhaust a host thread's call stack.
        // Keep group continuations explicitly on the heap instead.
        let mut groups = Vec::new();
        let mut states = 0;
        let mut alternatives = 0;
        loop {
            let Some(ch) = self.peek() else {
                return if groups.is_empty() {
                    self.add_states(alternatives, states)
                } else {
                    Err(self.invalid_here("unclosed group"))
                };
            };
            if ch == '(' {
                let capturing = self.begin_group()?;
                groups.push((states, alternatives, capturing));
                states = 0;
                alternatives = 0;
                continue;
            }
            if ch == '|' {
                self.cursor += 1;
                alternatives = self.add_states(alternatives, states)?;
                alternatives = self.add_states(alternatives, 1)?;
                states = 0;
                continue;
            }
            if ch == ')' {
                let Some((outer_states, outer_alternatives, capturing)) = groups.pop() else {
                    return self.add_states(alternatives, states);
                };
                self.cursor += 1;
                let body = self.add_states(alternatives, states)?;
                let atom = if capturing {
                    self.add_states(body, 2)?
                } else {
                    body
                };
                let quantified = self.parse_quantifier(atom)?;
                states = self.add_states(outer_states, quantified)?;
                alternatives = outer_alternatives;
                continue;
            }
            if matches!(ch, ']' | '}') {
                return Err(self.invalid_here("unexpected token"));
            }
            if matches!(ch, '?' | '*' | '+') {
                return Err(self.invalid_here("invalid quantifier"));
            }

            if self.starts_literal() {
                let literal_atoms = self.parse_literal_run()?;
                states = self.add_states(states, literal_atoms.saturating_sub(1))?;
                let quantified = self.parse_quantifier(1)?;
                states = self.add_states(states, quantified)?;
            } else {
                let atom = self.parse_atom()?;
                let quantified = self.parse_quantifier(atom)?;
                states = self.add_states(states, quantified)?;
            }
        }
    }

    fn parse_atom(&mut self) -> Result<u64, CompileError> {
        match self.peek() {
            Some('.' | '^' | '$') => {
                self.cursor += 1;
                Ok(1)
            }
            Some('[') => self.parse_class(),
            Some('\\') => {
                self.parse_shorthand()?;
                Ok(1)
            }
            Some('{') => Err(self.invalid_here("invalid quantifier")),
            Some(_) => Err(self.invalid_here("unexpected token")),
            None => Ok(0),
        }
    }

    fn begin_group(&mut self) -> Result<bool, CompileError> {
        let open = self.cursor;
        self.cursor += 1;
        let mut capturing = true;
        let mut name = None;
        if self.peek() == Some('?') {
            if self.peek_n(1) == Some(':') {
                capturing = false;
                self.cursor += 2;
            } else if self.peek_n(1) == Some('P') && self.peek_n(2) == Some('<') {
                self.cursor += 3;
                name = Some(self.parse_capture_name()?);
            } else if self.flags_looking_here() {
                return Err(self.invalid_at(open, "flag group must be leading and canonical"));
            } else {
                return Err(self.invalid_at(open, "unsupported construct"));
            }
        }

        let capture_index = if capturing {
            if self.capture_count == CAPTURE_LIMIT {
                return Err(self.invalid_at(open, "too many capture groups"));
            }
            self.capture_count += 1;
            if let Some(name) = name {
                if !self.capture_names.insert(name.clone()) {
                    let name_start = open + 4;
                    return Err(self.invalid_at(name_start, "capture name is duplicated"));
                }
                self.named_captures.push((name, self.capture_count));
            }
            Some(self.capture_count)
        } else {
            None
        };

        Ok(capture_index.is_some())
    }

    fn parse_capture_name(&mut self) -> Result<String, CompileError> {
        let start = self.cursor;
        let Some(first) = self.peek() else {
            return Err(self.invalid_here("capture name is invalid"));
        };
        if !first.is_ascii_alphabetic() {
            return Err(self.invalid_here("capture name is invalid"));
        }
        self.cursor += 1;
        while self
            .peek()
            .is_some_and(|ch| ch.is_ascii_alphanumeric() || ch == '_')
        {
            self.cursor += 1;
        }
        if self.peek() != Some('>') {
            return Err(self.invalid_here("capture name is invalid"));
        }
        let name = self.chars[start..self.cursor].iter().collect();
        self.cursor += 1;
        Ok(name)
    }

    fn parse_class(&mut self) -> Result<u64, CompileError> {
        self.cursor += 1;
        if self.peek() == Some('^') {
            self.cursor += 1;
        }
        if self.peek() == Some(']') {
            return Err(self.invalid_here("empty character class"));
        }

        loop {
            if self.at_end() {
                return Err(self.invalid_here("unclosed character class"));
            }
            if self.peek() == Some(']') {
                self.cursor += 1;
                return Ok(1);
            }
            if self.peek() == Some('-') {
                return Err(self.invalid_here("unexpected token"));
            }

            let left = self.parse_class_item()?;
            if self.peek() == Some('-') {
                let dash = self.cursor;
                self.cursor += 1;
                if self.peek().is_none() || self.peek() == Some(']') {
                    return Err(self.invalid_at(dash, "unexpected token"));
                }
                let right = self.parse_class_item()?;
                let (ClassItem::Scalar(left), ClassItem::Scalar(right)) = (left, right) else {
                    return Err(self.invalid_at(dash, "unexpected token"));
                };
                if left > right {
                    return Err(self.invalid_at(dash, "character class range is reversed"));
                }
            }
        }
    }

    fn parse_class_item(&mut self) -> Result<ClassItem, CompileError> {
        let Some(ch) = self.peek() else {
            return Err(self.invalid_here("unclosed character class"));
        };
        if ch != '\\' {
            self.cursor += 1;
            return Ok(ClassItem::Scalar(ch));
        }

        let slash = self.cursor;
        self.cursor += 1;
        let Some(escaped) = self.peek() else {
            return Err(self.invalid_at(slash, "invalid escape"));
        };
        self.cursor += 1;
        match escaped {
            'd' | 'D' | 'w' | 'W' | 's' | 'S' => Ok(ClassItem::Shorthand),
            'n' => Ok(ClassItem::Scalar('\n')),
            'r' => Ok(ClassItem::Scalar('\r')),
            't' => Ok(ClassItem::Scalar('\t')),
            'f' => Ok(ClassItem::Scalar('\u{000c}')),
            'a' => Ok(ClassItem::Scalar('\u{0007}')),
            'e' => Ok(ClassItem::Scalar('\u{001b}')),
            ']' | '-' | '\\' | '^' => Ok(ClassItem::Scalar(escaped)),
            ch if ch.is_ascii_alphanumeric() => {
                Err(self.invalid_at(slash, "unsupported construct"))
            }
            _ => Err(self.invalid_at(slash, "invalid escape")),
        }
    }

    fn starts_literal(&self) -> bool {
        match self.peek() {
            Some('\\') => self.peek_n(1).is_some_and(is_literal_escape),
            Some(ch) => !is_metacharacter(ch),
            None => false,
        }
    }

    fn parse_literal_run(&mut self) -> Result<u64, CompileError> {
        let mut literal = String::new();
        while let Some(ch) = self.peek() {
            if ch == '\\' {
                let slash = self.cursor;
                let Some(escaped) = self.peek_n(1) else {
                    return Err(self.invalid_at(slash, "invalid escape"));
                };
                if matches!(escaped, 'd' | 'D' | 'w' | 'W' | 's' | 'S') {
                    break;
                }
                let decoded = decode_literal_escape(escaped).ok_or_else(|| {
                    if escaped.is_ascii_alphanumeric() {
                        self.invalid_at(slash, "unsupported construct")
                    } else {
                        self.invalid_at(slash, "invalid escape")
                    }
                })?;
                literal.push(decoded);
                self.cursor += 2;
            } else if is_metacharacter(ch) {
                break;
            } else {
                literal.push(ch);
                self.cursor += 1;
            }
        }
        Ok(UnicodeSegmentation::graphemes(literal.as_str(), true).count() as u64)
    }

    fn parse_shorthand(&mut self) -> Result<(), CompileError> {
        let slash = self.cursor;
        self.cursor += 1;
        match self.peek() {
            Some('d' | 'D' | 'w' | 'W' | 's' | 'S') => {
                self.cursor += 1;
                Ok(())
            }
            Some(ch) if ch.is_ascii_alphanumeric() => {
                Err(self.invalid_at(slash, "unsupported construct"))
            }
            Some(_) | None => Err(self.invalid_at(slash, "invalid escape")),
        }
    }

    fn parse_quantifier(&mut self, atom_states: u64) -> Result<u64, CompileError> {
        let Some(ch) = self.peek() else {
            return Ok(atom_states);
        };
        let start = self.cursor;
        let states = match ch {
            '?' | '*' | '+' => {
                self.cursor += 1;
                self.add_states(atom_states, 1)?
            }
            '{' => self.parse_bounded_quantifier(atom_states, start)?,
            _ => return Ok(atom_states),
        };
        if self.peek() == Some('?') {
            self.cursor += 1;
        }
        if self
            .peek()
            .is_some_and(|next| matches!(next, '?' | '*' | '+' | '{'))
        {
            return Err(self.invalid_here("invalid quantifier"));
        }
        Ok(states)
    }

    fn parse_bounded_quantifier(
        &mut self,
        atom_states: u64,
        start: usize,
    ) -> Result<u64, CompileError> {
        self.cursor += 1;
        let minimum = self.parse_decimal(start)?;
        let states = match self.peek() {
            Some('}') => {
                self.cursor += 1;
                self.multiply_states(atom_states, minimum)?
            }
            Some(',') => {
                self.cursor += 1;
                if self.peek() == Some('}') {
                    self.cursor += 1;
                    let required = self.multiply_states(atom_states, minimum)?;
                    let tail = self.add_states(atom_states, 1)?;
                    self.add_states(required, tail)?
                } else {
                    let maximum = self.parse_decimal(start)?;
                    if minimum > maximum {
                        return Err(self.invalid_at(start, "quantifier range is reversed"));
                    }
                    if self.peek() != Some('}') {
                        return Err(self.invalid_at(start, "invalid quantifier"));
                    }
                    self.cursor += 1;
                    let clones = self.multiply_states(atom_states, maximum)?;
                    self.add_states(clones, maximum - minimum)?
                }
            }
            _ => return Err(self.invalid_at(start, "invalid quantifier")),
        };
        Ok(states)
    }

    fn parse_decimal(&mut self, quantifier_start: usize) -> Result<u64, CompileError> {
        let start = self.cursor;
        if !self.peek().is_some_and(|ch| ch.is_ascii_digit()) {
            return Err(self.invalid_at(quantifier_start, "invalid quantifier"));
        }
        if self.peek() == Some('0') && self.peek_n(1).is_some_and(|ch| ch.is_ascii_digit()) {
            return Err(self.invalid_at(quantifier_start, "invalid quantifier"));
        }
        let mut value = 0_u64;
        while let Some(ch) = self.peek().filter(|ch| ch.is_ascii_digit()) {
            value = value
                .checked_mul(10)
                .and_then(|current| current.checked_add(u64::from(ch as u8 - b'0')))
                .ok_or_else(|| self.invalid_at(quantifier_start, "invalid quantifier"))?;
            self.cursor += 1;
        }
        if self.cursor == start || value > REPETITION_LIMIT {
            return Err(self.invalid_at(quantifier_start, "invalid quantifier"));
        }
        Ok(value)
    }

    fn flags_looking_here(&self) -> bool {
        if self.peek() != Some('?') || !self.peek_n(1).is_some_and(|ch| ch.is_ascii_lowercase()) {
            return false;
        }
        let mut offset = 1;
        while self
            .peek_n(offset)
            .is_some_and(|ch| ch.is_ascii_lowercase())
        {
            offset += 1;
        }
        matches!(self.peek_n(offset), Some(')') | None)
    }

    fn add_states(&self, left: u64, right: u64) -> Result<u64, CompileError> {
        Ok(left.saturating_add(right).min(COMPILED_STATE_LIMIT + 1))
    }

    fn multiply_states(&self, left: u64, right: u64) -> Result<u64, CompileError> {
        Ok(left.saturating_mul(right).min(COMPILED_STATE_LIMIT + 1))
    }

    fn starts_with(&self, expected: &str) -> bool {
        expected
            .chars()
            .enumerate()
            .all(|(offset, ch)| self.peek_n(offset) == Some(ch))
    }

    fn peek(&self) -> Option<char> {
        self.peek_n(0)
    }

    fn peek_n(&self, offset: usize) -> Option<char> {
        self.chars.get(self.cursor + offset).copied()
    }

    fn at_end(&self) -> bool {
        self.cursor == self.chars.len()
    }

    fn invalid_here(&self, message: &'static str) -> CompileError {
        self.invalid_at(self.cursor, message)
    }

    fn invalid_at(&self, scalar: usize, message: &'static str) -> CompileError {
        CompileError::InvalidPattern(InvalidPattern {
            position: self.scalar_to_grapheme[scalar.min(self.chars.len())],
            message,
        })
    }
}

#[derive(Clone, Copy)]
enum ClassItem {
    Scalar(char),
    Shorthand,
}

fn is_metacharacter(ch: char) -> bool {
    matches!(
        ch,
        '\\' | '.' | '^' | '$' | '|' | '?' | '*' | '+' | '(' | ')' | '[' | ']' | '{' | '}'
    )
}

fn is_literal_escape(ch: char) -> bool {
    decode_literal_escape(ch).is_some()
}

fn decode_literal_escape(ch: char) -> Option<char> {
    match ch {
        'n' => Some('\n'),
        'r' => Some('\r'),
        't' => Some('\t'),
        'f' => Some('\u{000c}'),
        'a' => Some('\u{0007}'),
        'e' => Some('\u{001b}'),
        '\\' | '.' | '^' | '$' | '|' | '?' | '*' | '+' | '(' | ')' | '[' | ']' | '{' | '}' => {
            Some(ch)
        }
        _ => None,
    }
}
