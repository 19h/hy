//! String processing utilities.

/// Extract the domain portion of an email address.
#[allow(dead_code)]
pub fn email_domain(email: &str) -> &str {
    email.split_once('@').map_or("", |(_, d)| d)
}

/// Abbreviate a string to `max` characters, respecting word boundaries.
#[allow(dead_code)]
pub fn abbreviate(text: &str, max: usize) -> String {
    if text.len() <= max {
        return text.to_owned();
    }
    let trimmed = &text[..max];
    match trimmed.rfind(' ') {
        Some(i) if i > 0 => format!("{}...", &trimmed[..i]),
        _ => format!("{trimmed}..."),
    }
}

/// Truncate in the middle, keeping start and end.
#[allow(dead_code)]
pub fn truncate_middle(text: &str, max: usize) -> String {
    if text.len() <= max {
        return text.to_owned();
    }
    let sep = "...";
    if max <= sep.len() {
        return sep[..max].to_owned();
    }
    let content = max - sep.len();
    let start = content / 2;
    let end = content - start;
    format!("{}{sep}{}", &text[..start], &text[text.len() - end..])
}

/// Simple slugify: lowercase, replace non-alphanumeric runs with `sep`.
#[allow(dead_code)]
pub fn slugify(text: &str, sep: char) -> String {
    let s: String = text
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { sep })
        .collect();
    // Collapse repeated separators and trim.
    let sep_str = sep.to_string();
    let doubled = format!("{sep}{sep}");
    let mut result = s;
    while result.contains(&doubled) {
        result = result.replace(&doubled, &sep_str);
    }
    result.trim_matches(sep).to_owned()
}

/// Levenshtein distance between two strings.
#[allow(dead_code)]
pub fn levenshtein(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let (m, n) = (a.len(), b.len());

    let mut prev: Vec<usize> = (0..=n).collect();
    let mut curr = vec![0usize; n + 1];

    for i in 1..=m {
        curr[0] = i;
        for j in 1..=n {
            let cost = if a[i - 1] == b[j - 1] { 0 } else { 1 };
            curr[j] = (prev[j] + 1).min(curr[j - 1] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }

    prev[n]
}

/// Similarity ratio (0.0–1.0) based on Levenshtein distance.
#[allow(dead_code)]
pub fn similarity(a: &str, b: &str) -> f64 {
    if a == b {
        return 1.0;
    }
    let max_len = a.len().max(b.len());
    if max_len == 0 {
        return 1.0;
    }
    1.0 - (levenshtein(a, b) as f64 / max_len as f64)
}

/// Find the best matching candidate (above `threshold`) for `target`.
#[allow(dead_code)]
pub fn best_match<'a>(target: &str, candidates: &'a [String], threshold: f64) -> Option<&'a str> {
    let target_lower = target.to_lowercase();
    candidates
        .iter()
        .filter_map(|c| {
            let ratio = similarity(&target_lower, &c.to_lowercase());
            if ratio >= threshold {
                Some((c.as_str(), ratio))
            } else {
                None
            }
        })
        .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(s, _)| s)
}
