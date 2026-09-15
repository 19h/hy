//! Python end-anchor semantics for native setting validation.
//!
//! The scanner preserves escaped literals, character classes and comments. It
//! tracks verbose-mode scopes only to distinguish comments from pattern syntax;
//! fancy-regex remains responsible for parsing and validating the expression.

pub fn matches_prefix(pattern: &str, text: &str) -> Result<bool, Box<fancy_regex::Error>> {
    let expression = fancy_regex::Regex::new(&translate(pattern))?;
    Ok(expression.find(text)?.is_some_and(|matched| matched.start() == 0))
}

#[derive(Clone, Copy)]
enum CharacterClass {
    Outside,
    First {
        can_negate: bool,
    },
    Inside,
}

fn translate(pattern: &str) -> String {
    let mut output = String::with_capacity(pattern.len());
    let mut characters = pattern.chars();
    let mut class = CharacterClass::Outside;
    let mut verbose = false;
    let mut scopes = Vec::new();
    let mut group_comment = false;

    while let Some(character) = characters.next() {
        if character == '\\' {
            output.push(character);
            if let Some(escaped) = characters.next() {
                output.push(if escaped == 'Z' {
                    'z'
                } else {
                    escaped
                });
            }
            if matches!(class, CharacterClass::First { .. }) {
                class = CharacterClass::Inside;
            }
            continue;
        }
        if group_comment {
            output.push(character);
            group_comment = character != ')';
            continue;
        }
        match class {
            CharacterClass::First {
                can_negate,
            } => {
                output.push(character);
                if character == '^' && can_negate {
                    class = CharacterClass::First {
                        can_negate: false,
                    };
                } else {
                    class = CharacterClass::Inside;
                }
                continue;
            }
            CharacterClass::Inside => {
                output.push(character);
                if character == ']' {
                    class = CharacterClass::Outside;
                }
                continue;
            }
            CharacterClass::Outside => (),
        }
        match character {
            '[' => {
                class = CharacterClass::First {
                    can_negate: true,
                };
                output.push(character);
            }
            '#' if verbose => {
                output.push(character);
                for character in characters.by_ref() {
                    output.push(character);
                    if character == '\n' {
                        break;
                    }
                }
            }
            '(' if characters.as_str().starts_with("?#") => {
                output.push_str("(?#");
                characters.next();
                characters.next();
                group_comment = true;
            }
            '(' => {
                output.push(character);
                if let Some(flags) = verbose_flags(characters.as_str(), verbose) {
                    if flags.scoped {
                        scopes.push(verbose);
                    }
                    verbose = flags.verbose;
                    for _ in 0..flags.length {
                        output.push(characters.next().expect("parsed flag header"));
                    }
                } else {
                    scopes.push(verbose);
                }
            }
            ')' => {
                verbose = scopes.pop().unwrap_or(verbose);
                output.push(character);
            }
            '$' => output.push_str(r"(?=$|\n\z)"),
            character => output.push(character),
        }
    }
    output
}

struct Flags {
    length: usize,
    verbose: bool,
    scoped: bool,
}

fn verbose_flags(header: &str, current: bool) -> Option<Flags> {
    let header = header.strip_prefix('?')?;
    let mut enabled = true;
    let mut verbose = current;
    for (index, character) in header.chars().enumerate() {
        match character {
            'x' => verbose = enabled,
            '-' => enabled = false,
            'a' | 'i' | 'L' | 'm' | 's' | 'u' => (),
            ':' | ')' => {
                return Some(Flags {
                    length: index + 2,
                    verbose,
                    scoped: character == ':',
                });
            }
            _ => return None,
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anchors_lookarounds_and_backreferences_match_the_python_oracle() {
        for (pattern, text, expected) in [
            (r"abc", "abcdef", true),
            (r"abc", "xabc", false),
            (r"abc|def", "xdef", false),
            (r"abc$", "abc\n", true),
            (r"abc$", "abc\n\n", false),
            (r"abc\Z", "abc\n", false),
            (r"abc\Z", "abc", true),
            (r"\\Z", r"\Z", true),
            (r"\$", "$", true),
            (r"[$]", "$", true),
            (r"[]$]+\Z", "]$", true),
            (r"[^^]$", "a\n", true),
            (r"(?#$)abc$", "abc\n", true),
            ("(?x)abc # [ ignored\n$", "abc\n", true),
            ("(?x)abc # trailing comment", "abc", true),
            (r"(?x: a (?-x:#) )$", "a#\n", true),
            (r"(?m)abc$", "abc\nnext", true),
            (r"(?m)^abc", "x\nabc", false),
            (r"(?i)abc\Z", "ABC", true),
            (r"(?=.{4}\Z)[a-z]+", "abcd", true),
            (r"(?=.{4}\Z)[a-z]+", "abc", false),
            (r"(?P<word>[a-z]+)-(?P=word)\Z", "abc-abc", true),
            (r"(?P<word>[a-z]+)-(?P=word)\Z", "abc-def", false),
            (r"([a-z]+)-\1\Z", "abc-abc", true),
            (r"(a)?(?(1)b|c)\Z", "ab", true),
        ] {
            assert_eq!(matches_prefix(pattern, text).unwrap(), expected, "{pattern:?} {text:?}");
        }
    }
}
