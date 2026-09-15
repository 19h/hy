//! CPython 3.13 sorting for comparisons that do not define a total order.
//!
//! Repository compatibility keys contain sets: incomparable sets are not equal,
//! but neither is less than the other. Rust's sort contract excludes this case.
//! Keep CPython's run detection, merge schedule and comparison order here.
//! Adapted from CPython v3.13.15 Objects/listobject.c; see python_sort/LICENSE.

mod merge;
mod runs;
mod search;

use runs::{Run, count_run, extend_run, min_run, power};

pub fn sort_by<T: Copy>(values: &mut [T], less: impl Fn(&T, &T) -> bool) {
    let mut pending: Vec<Run> = Vec::new();
    let mut minimum_gallop = 7;
    let minimum_run = min_run(values.len());
    let mut start = 0;

    while start < values.len() {
        let natural = count_run(&mut values[start..], &less);
        let length = natural.max(minimum_run.min(values.len() - start));
        extend_run(&mut values[start..start + length], natural, &less);

        if let Some(previous) = pending.last() {
            let next_power = power(previous.start, previous.length, length, values.len());
            while pending.len() > 1 && pending[pending.len() - 2].power > next_power {
                let index = pending.len() - 2;
                merge_at(values, &mut pending, index, &less, &mut minimum_gallop);
            }
            pending.last_mut().unwrap().power = next_power;
        }
        pending.push(Run {
            start,
            length,
            power: 0,
        });
        start += length;
    }

    while pending.len() > 1 {
        let mut index = pending.len() - 2;
        if index > 0 && pending[index - 1].length < pending[index + 1].length {
            index -= 1;
        }
        merge_at(values, &mut pending, index, &less, &mut minimum_gallop);
    }
}

fn merge_at<T: Copy>(
    values: &mut [T],
    pending: &mut Vec<Run>,
    index: usize,
    less: &impl Fn(&T, &T) -> bool,
    minimum_gallop: &mut usize,
) {
    let left = pending[index];
    let right = pending.remove(index + 1);
    pending[index].length += right.length;
    let skipped = search::right(&values[right.start], &values[left.start..right.start], 0, less);
    let start = left.start + skipped;
    if start == right.start {
        return;
    }
    let length = search::left(
        &values[right.start - 1],
        &values[right.start..right.start + right.length],
        right.length - 1,
        less,
    );
    if length == 0 {
        return;
    }
    merge::merge(
        &mut values[start..right.start + length],
        right.start - start,
        less,
        minimum_gallop,
    );
}

#[cfg(test)]
mod tests;
