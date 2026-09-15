//! Python filename glob matching with bounded dynamic-programming state.

pub(super) struct Pattern {
    tokens: Vec<Token>,
}

enum Token {
    Literal(char),
    Any,
    Star,
    Class {
        negated: bool,
        ranges: Vec<(char, char)>,
    },
}

impl Pattern {
    pub fn new(pattern: &str) -> Self {
        let pattern = fold_case(pattern);
        let characters: Vec<_> = pattern.chars().collect();
        let last_closing_bracket = characters.iter().rposition(|character| *character == ']');
        let mut tokens = Vec::new();
        let mut index = 0;
        while index < characters.len() {
            let character = characters[index];
            index += 1;
            let token = match character {
                '*' if matches!(tokens.last(), Some(Token::Star)) => continue,
                '*' => Token::Star,
                '?' => Token::Any,
                // Avoid rescanning the remaining suffix for each unclosed '['.
                '[' if last_closing_bracket.is_some_and(|end| index <= end) => {
                    match character_class(&characters[index..]) {
                        Some((token, consumed)) => {
                            index += consumed;
                            token
                        }
                        None => Token::Literal('['),
                    }
                }
                character => Token::Literal(character),
            };
            tokens.push(token);
        }
        Self {
            tokens,
        }
    }

    pub fn matches(&self, name: &str) -> bool {
        let mut previous = vec![false; self.tokens.len() + 1];
        previous[0] = true;
        for (index, token) in self.tokens.iter().enumerate() {
            previous[index + 1] = previous[index] && matches!(token, Token::Star);
        }
        let mut current = vec![false; previous.len()];
        for character in fold_case(name).chars() {
            current[0] = false;
            for (index, token) in self.tokens.iter().enumerate() {
                current[index + 1] = match token {
                    Token::Star => current[index] || previous[index + 1],
                    token => previous[index] && token.matches(character),
                };
            }
            std::mem::swap(&mut previous, &mut current);
        }
        previous[self.tokens.len()]
    }
}

impl Token {
    fn matches(&self, character: char) -> bool {
        match self {
            Self::Literal(expected) => *expected == character,
            Self::Any => true,
            Self::Class {
                negated,
                ranges,
            } => {
                ranges.iter().any(|(start, end)| *start <= character && character <= *end)
                    != *negated
            }
            Self::Star => unreachable!("star transitions are handled by the matching state"),
        }
    }
}

fn character_class(characters: &[char]) -> Option<(Token, usize)> {
    let start = usize::from(characters.first() == Some(&'!'));
    let search_start = start + usize::from(characters.get(start) == Some(&']'));
    let end = search_start
        + characters.get(search_start..)?.iter().position(|character| *character == ']')?;
    let mut members = Vec::new();
    if start == 1 {
        members.push(ClassMember::Literal('!'));
    }
    let mut index = start;
    while index < end {
        let first = characters[index];
        if index + 2 < end && characters[index + 1] == '-' {
            let last = characters[index + 2];
            if first <= last {
                members.push(ClassMember::Range(first, last));
            }
            index += 3;
        } else {
            members.push(ClassMember::Literal(first));
            index += 1;
        }
    }
    // Python removes reversed ranges before interpreting a leading '!'. A
    // removed prefix can therefore expose a negation that was not initially first.
    let mut members = members.into_iter();
    let mut ranges = Vec::new();
    let negated = match members.next() {
        Some(ClassMember::Literal('!')) => true,
        Some(ClassMember::Range('!', last)) => {
            ranges.extend([('-', '-'), (last, last)]);
            true
        }
        Some(member) => {
            ranges.push(member.range());
            false
        }
        None => false,
    };
    ranges.extend(members.map(ClassMember::range));
    Some((
        Token::Class {
            negated,
            ranges,
        },
        end + 1,
    ))
}

enum ClassMember {
    Literal(char),
    Range(char, char),
}

impl ClassMember {
    fn range(self) -> (char, char) {
        match self {
            Self::Literal(character) => (character, character),
            Self::Range(first, last) => (first, last),
        }
    }
}

fn fold_case(text: &str) -> std::borrow::Cow<'_, str> {
    if cfg!(windows) {
        text.to_lowercase().into()
    } else {
        text.into()
    }
}

#[cfg(test)]
mod tests;
