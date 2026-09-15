//! Exponential searches retain CPython's probes, including for incomparable keys.

pub(super) fn left<T>(key: &T, values: &[T], hint: usize, less: &impl Fn(&T, &T) -> bool) -> usize {
    boundary(values, hint, |value| less(value, key))
}

pub(super) fn right<T>(
    key: &T,
    values: &[T],
    hint: usize,
    less: &impl Fn(&T, &T) -> bool,
) -> usize {
    boundary(values, hint, |value| !less(key, value))
}

fn boundary<T>(values: &[T], hint: usize, before: impl Fn(&T) -> bool) -> usize {
    let (mut previous, mut offset) = (0, 1);
    let (mut low, mut high) = if before(&values[hint]) {
        let limit = values.len() - hint;
        while offset < limit && before(&values[hint + offset]) {
            previous = offset;
            offset = offset.saturating_mul(2).saturating_add(1);
        }
        (hint + previous + 1, hint + offset.min(limit))
    } else {
        let limit = hint + 1;
        while offset < limit && !before(&values[hint - offset]) {
            previous = offset;
            offset = offset.saturating_mul(2).saturating_add(1);
        }
        (hint + 1 - offset.min(limit), hint - previous)
    };
    while low < high {
        let middle = low + (high - low) / 2;
        if before(&values[middle]) {
            low = middle + 1;
        } else {
            high = middle;
        }
    }
    low
}
