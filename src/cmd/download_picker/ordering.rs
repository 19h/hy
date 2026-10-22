//! Natural numeric order, with service packs above releases and prereleases below.

use std::cmp::Reverse;

use num_bigint::BigInt;

use crate::api::TreeNode;
use crate::error::{Error, Result};

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Part {
    Number(Reverse<BigInt>),
    Text(String),
    End,
    Prerelease(Reverse<String>),
}

fn key(name: &str) -> Result<Vec<Part>> {
    let name = name.to_lowercase();
    let mut runs = Vec::new();
    let mut start = 0;
    let mut numeric = false;
    for (index, character) in name.char_indices() {
        let digit = crate::util::python_integer::is_decimal_digit(character);
        if index > start && digit != numeric {
            runs.push((numeric, &name[start..index]));
            start = index;
        }
        numeric = digit;
    }
    if start < name.len() {
        runs.push((numeric, &name[start..]));
    }
    let mut saw_digit = false;
    let mut prerelease = false;
    let mut key = Vec::new();
    for (numeric, text) in runs {
        if numeric {
            let number = crate::util::python_integer::parse(text).ok_or_else(|| {
                Error::Other("download name exceeds the decimal digit limit".into())
            })?;
            key.push(Part::Number(Reverse(number)));
            saw_digit = true;
        } else {
            prerelease |= saw_digit && text.starts_with('-');
            key.push(if prerelease {
                Part::Prerelease(Reverse(text.into()))
            } else {
                Part::Text(text.into())
            });
        }
    }
    key.push(Part::End);
    Ok(key)
}

pub(super) fn sort(nodes: &mut Vec<&TreeNode>) -> Result<()> {
    let mut keyed =
        nodes.iter().map(|node| Ok((key(&node.name)?, *node))).collect::<Result<Vec<_>>>()?;
    keyed.sort_by(|(left, _), (right, _)| left.cmp(right));
    *nodes = keyed.into_iter().map(|(_, node)| node).collect();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn orders_versions_categories_and_prereleases() {
        let mut names = [
            "sdk",
            "9.0",
            "9.0sp1",
            "3.1.7",
            "3.1.10",
            "ida-pro",
            "1.0.0-beta.10",
            "1.0.0",
            "1.0.0-rc.1",
            "1.0.0-beta.2",
        ];
        names.sort_by_cached_key(|name| key(name).unwrap());
        assert_eq!(
            names,
            [
                "9.0sp1",
                "9.0",
                "3.1.10",
                "3.1.7",
                "1.0.0",
                "1.0.0-rc.1",
                "1.0.0-beta.10",
                "1.0.0-beta.2",
                "ida-pro",
                "sdk"
            ]
        );
    }
}
