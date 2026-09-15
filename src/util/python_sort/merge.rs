use super::search;

pub(super) fn merge<T: Copy>(
    values: &mut [T],
    middle: usize,
    less: &impl Fn(&T, &T) -> bool,
    minimum_gallop: &mut usize,
) {
    // Sorting references makes this buffer proportional to record count, not payload size.
    let source = values.to_vec();
    let (left, right) = source.split_at(middle);
    if left.len() <= right.len() {
        forward(values, left, right, less, minimum_gallop);
    } else {
        backward(values, left, right, less, minimum_gallop);
    }
}

fn forward<T: Copy>(
    output: &mut [T],
    left: &[T],
    right: &[T],
    less: &impl Fn(&T, &T) -> bool,
    minimum_gallop: &mut usize,
) {
    let (mut a, mut b, mut destination) = (0, 1, 1);
    output[0] = right[0];
    let mut threshold = *minimum_gallop;
    'merge: while left.len() - a > 1 && b < right.len() {
        let (mut left_wins, mut right_wins) = (0, 0);
        loop {
            if less(&right[b], &left[a]) {
                output[destination] = right[b];
                b += 1;
                right_wins += 1;
                left_wins = 0;
            } else {
                output[destination] = left[a];
                a += 1;
                left_wins += 1;
                right_wins = 0;
            }
            destination += 1;
            if left.len() - a == 1 || b == right.len() {
                break 'merge;
            }
            if left_wins >= threshold || right_wins >= threshold {
                break;
            }
        }
        threshold += 1;
        loop {
            threshold = threshold.saturating_sub(1).max(1);
            *minimum_gallop = threshold;
            left_wins = search::right(&right[b], &left[a..], 0, less);
            copy_forward(output, &mut destination, &left[a..a + left_wins]);
            a += left_wins;
            if left.len() - a <= 1 {
                break 'merge;
            }
            copy_forward(output, &mut destination, &right[b..b + 1]);
            b += 1;
            if b == right.len() {
                break 'merge;
            }
            right_wins = search::left(&left[a], &right[b..], 0, less);
            copy_forward(output, &mut destination, &right[b..b + right_wins]);
            b += right_wins;
            if b == right.len() {
                break 'merge;
            }
            copy_forward(output, &mut destination, &left[a..a + 1]);
            a += 1;
            if left.len() - a == 1 {
                break 'merge;
            }
            if left_wins < 7 && right_wins < 7 {
                break;
            }
        }
        threshold += 1;
        *minimum_gallop = threshold;
    }
    if left.len() - a == 1 {
        copy_forward(output, &mut destination, &right[b..]);
        copy_forward(output, &mut destination, &left[a..]);
    } else {
        copy_forward(output, &mut destination, &left[a..]);
        copy_forward(output, &mut destination, &right[b..]);
    }
}

fn backward<T: Copy>(
    output: &mut [T],
    left: &[T],
    right: &[T],
    less: &impl Fn(&T, &T) -> bool,
    minimum_gallop: &mut usize,
) {
    let (mut a, mut b, mut destination) = (left.len() - 1, right.len(), output.len() - 1);
    output[destination] = left[a];
    let mut threshold = *minimum_gallop;
    'merge: while a > 0 && b > 1 {
        let (mut left_wins, mut right_wins) = (0, 0);
        loop {
            destination -= 1;
            if less(&right[b - 1], &left[a - 1]) {
                a -= 1;
                output[destination] = left[a];
                left_wins += 1;
                right_wins = 0;
            } else {
                b -= 1;
                output[destination] = right[b];
                right_wins += 1;
                left_wins = 0;
            }
            if a == 0 || b == 1 {
                break 'merge;
            }
            if left_wins >= threshold || right_wins >= threshold {
                break;
            }
        }
        threshold += 1;
        loop {
            threshold = threshold.saturating_sub(1).max(1);
            *minimum_gallop = threshold;
            left_wins = a - search::right(&right[b - 1], &left[..a], a - 1, less);
            copy_backward(output, &mut destination, &left[a - left_wins..a]);
            a -= left_wins;
            if a == 0 {
                break 'merge;
            }
            copy_backward(output, &mut destination, &right[b - 1..b]);
            b -= 1;
            if b == 1 {
                break 'merge;
            }
            right_wins = b - search::left(&left[a - 1], &right[..b], b - 1, less);
            copy_backward(output, &mut destination, &right[b - right_wins..b]);
            b -= right_wins;
            if b <= 1 {
                break 'merge;
            }
            copy_backward(output, &mut destination, &left[a - 1..a]);
            a -= 1;
            if a == 0 {
                break 'merge;
            }
            if left_wins < 7 && right_wins < 7 {
                break;
            }
        }
        threshold += 1;
        *minimum_gallop = threshold;
    }
    if b == 1 {
        copy_backward(output, &mut destination, &left[..a]);
        copy_backward(output, &mut destination, &right[..b]);
    } else {
        copy_backward(output, &mut destination, &right[..b]);
        copy_backward(output, &mut destination, &left[..a]);
    }
}

fn copy_forward<T: Copy>(output: &mut [T], destination: &mut usize, source: &[T]) {
    output[*destination..*destination + source.len()].copy_from_slice(source);
    *destination += source.len();
}

fn copy_backward<T: Copy>(output: &mut [T], destination: &mut usize, source: &[T]) {
    output[*destination - source.len()..*destination].copy_from_slice(source);
    *destination -= source.len();
}
