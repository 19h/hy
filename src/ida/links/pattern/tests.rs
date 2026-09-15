//! Character-class results captured from CPython 3.13 fnmatch.fnmatchcase.

use super::*;

#[test]
fn exhaustive_short_character_classes_match_the_python_oracle_digest() {
    use sha2::{Digest, Sha256};

    let alphabet = ['a', '-', 'z', '!', ']', '^'];
    let candidates = "!&-]^abcz~|[\\";
    let mut digest = Sha256::new();
    let mut count = 0;
    for length in 0..=6 {
        for mut number in 0..alphabet.len().pow(length as u32) {
            let mut inner = vec![' '; length];
            for character in inner.iter_mut().rev() {
                *character = alphabet[number % alphabet.len()];
                number /= alphabet.len();
            }
            let spelling = format!("[{}]", inner.into_iter().collect::<String>());
            let pattern = Pattern::new(&spelling);
            for candidate in candidates.chars() {
                let matched = pattern.matches(&candidate.to_string());
                digest.update([u8::from(matched)]);
                count += 1;
            }
        }
    }
    assert_eq!(count, 727_831);
    assert_eq!(
        format!("{:x}", digest.finalize()),
        "36f3dce5242eabe75f3de48956f7522b42d9bc20232591649f14c3e823a8b92a"
    );
}

#[test]
fn character_classes_match_the_python_oracle() {
    let alphabet = "!&-]^abcz~|[\\";
    for (pattern, expected) in [
        ("[a-z]", "abcz"),
        ("[z-a]", ""),
        ("[!z-a]", "!&-]^abcz~|[\\"),
        ("[a--b]", "b"),
        ("[a-b-c]", "-abc"),
        ("[z-a-b-c]", "-bc"),
        ("[--z]", "-]^abcz[\\"),
        ("[!-a]", "!&]^bcz~|[\\"),
        ("[^^]", "^"),
        ("[]]", "]"),
        ("[[]", "["),
        ("[!]", ""),
        ("[]", ""),
        ("[abc", ""),
        ("[a&&b]", "&ab"),
        ("[a~~b]", "ab~"),
        ("[a||b]", "ab|"),
        (r"[\]]", ""),
    ] {
        let compiled = Pattern::new(pattern);
        let actual: String =
            alphabet.chars().filter(|character| compiled.matches(&character.to_string())).collect();
        assert_eq!(actual, expected, "{pattern}");
    }
}

#[test]
fn wildcard_matching_includes_hidden_files_and_treats_backslashes_literally() {
    for (pattern, text, expected) in [
        ("*.i64", ".hidden.i64", true),
        ("sample", "sample.i64", false),
        ("sample.idb", "sample.i64", false),
        ("a?c", "aéc", true),
        ("a?c", "a\nc", true),
        ("a*c", "a\nbc", true),
        ("a**b*c", "abbbc", true),
        ("*", "", true),
        ("*a*b", "abc", false),
        ("[]", "[]", true),
        ("[!]", "[!]", true),
        ("[abc", "[abc", true),
        (r"a\*", r"a\file", true),
        (r"a\*", "afile", false),
    ] {
        assert_eq!(Pattern::new(pattern).matches(text), expected, "{pattern:?} {text:?}");
    }
}
