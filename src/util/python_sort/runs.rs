#[derive(Clone, Copy)]
pub(super) struct Run {
    pub start: usize,
    pub length: usize,
    pub power: usize,
}

pub(super) fn min_run(mut length: usize) -> usize {
    let mut remainder = 0;
    while length >= 64 {
        remainder |= length & 1;
        length >>= 1;
    }
    length + remainder
}

pub(super) fn power(start: usize, left: usize, right: usize, total: usize) -> usize {
    let total = total as u128;
    let mut a = 2 * start as u128 + left as u128;
    let mut b = a + left as u128 + right as u128;
    let mut depth = 0;
    loop {
        depth += 1;
        if a >= total {
            a -= total;
            b -= total;
        } else if b >= total {
            return depth;
        }
        a *= 2;
        b *= 2;
    }
}

pub(super) fn count_run<T>(values: &mut [T], less: &impl Fn(&T, &T) -> bool) -> usize {
    let mut length = 1;
    while length < values.len() && !less(&values[length], &values[length - 1]) {
        length += 1;
    }
    if length == values.len() {
        return length;
    }
    if length > 1 {
        if less(&values[0], &values[length - 1]) {
            return length;
        }
        values[..length].reverse();
    }
    length += 1;
    let mut equal = 0;
    while length < values.len() {
        if less(&values[length], &values[length - 1]) {
            reverse_equal_tail(values, length, &mut equal);
        } else if less(&values[length - 1], &values[length]) {
            break;
        } else {
            equal += 1;
        }
        length += 1;
    }
    reverse_equal_tail(values, length, &mut equal);
    values[..length].reverse();
    while length < values.len() && !less(&values[length], &values[length - 1]) {
        length += 1;
    }
    length
}

fn reverse_equal_tail<T>(values: &mut [T], length: usize, equal: &mut usize) {
    if *equal > 0 {
        values[length - *equal - 1..length].reverse();
        *equal = 0;
    }
}

pub(super) fn extend_run<T: Copy>(values: &mut [T], sorted: usize, less: &impl Fn(&T, &T) -> bool) {
    for index in sorted..values.len() {
        let pivot = values[index];
        let (mut low, mut high) = (0, index);
        while low < high {
            let middle = low + (high - low) / 2;
            if less(&pivot, &values[middle]) {
                high = middle;
            } else {
                low = middle + 1;
            }
        }
        values.copy_within(low..index, low + 1);
        values[low] = pivot;
    }
}
