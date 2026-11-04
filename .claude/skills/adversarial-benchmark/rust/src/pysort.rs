//! Python's `==` and `<` on JSON values, and `sorted(..., key=...)` with the comparison schedule of CPython 3.12.3's
//! `list.sort` for every list length.
//!
//! The order of comparisons is observable: the first pair of incomparable keys decides which `TypeError` message is
//! raised, and keys that are not totally ordered (a NaN) decide where elements land. This module ports the
//! algorithm of `Objects/listobject.c` (v3.12.3, PSF License; see `notices/NOTICE.md`) - natural-run discovery
//! (`count_run`), `merge_compute_minrun`, `binarysort`, the powersort merge policy (`powerloop`, `found_new_run`,
//! `merge_force_collapse`), `merge_at`, `merge_lo`/`merge_hi` with `MIN_GALLOP` and the adaptive `min_gallop`, and
//! `gallop_left`/`gallop_right` - performing the same comparisons in the same order, so results and the first error
//! agree for any keys, ordered or not. Elements are permuted by index; the comparison always sees the keys.
//! The list-sort pre-check that selects specialised comparison functions (all-float, bounded-int, latin-1 string
//! keys) only changes how a comparison is computed, never its result or the order of comparisons.
//!
//! Not ported: the temporary-array growth policy (a `Vec` is used per merge) and reverse-sorting (not used here).

use crate::guidellm::Failure;
use crate::pynumber::PyException;
use crate::pyops::{compare, num_of, Compare};
use crate::pyvalue::{same_nan_object, PyValue};

/// Python's container comparison and `in`: two values that are the same object are equal, whatever `==` says
/// (`[nan] == [nan]` when both hold one NaN object). The top-level `==` operator does not do this; `py_eq` does it
/// only for the items of containers.
pub fn same_or_eq(a: &PyValue, b: &PyValue) -> bool {
    same_nan_object(a, b) || py_eq(a, b)
}

/// Python `a == b` for JSON values (numbers compare by value across `bool`/`int`/`float`).
pub fn py_eq(a: &PyValue, b: &PyValue) -> bool {
    let (a, b) = (a.peel(), b.peel());
    if let (Some(x), Some(y)) = (num_of(a), num_of(b)) {
        // Both operands are numbers, so the comparisons cannot raise.
        let _ = (x, y);
        let not_less = compare(Compare::GreaterEqual, a, b).unwrap_or(false);
        let not_greater = compare(Compare::LessEqual, a, b).unwrap_or(false);
        return not_less && not_greater;
    }
    match (a, b) {
        (PyValue::None, PyValue::None) => true,
        (PyValue::Str(x), PyValue::Str(y)) => x == y,
        (PyValue::Bytes(x), PyValue::Bytes(y)) => x == y,
        (PyValue::List(x), PyValue::List(y)) | (PyValue::Tuple(x), PyValue::Tuple(y)) => {
            x.len() == y.len() && x.iter().zip(y).all(|(p, q)| same_or_eq(p, q))
        }
        (PyValue::Dict(x), PyValue::Dict(y)) => {
            x.len() == y.len()
                && x.iter().all(|(key, value)| {
                    y.iter()
                        .find(|(other, _)| other == key)
                        .is_some_and(|(_, other)| same_or_eq(value, other))
                })
        }
        (PyValue::Opaque(x), PyValue::Opaque(y)) => x == y,
        _ => false,
    }
}

/// Python `a < b`: numbers, strings and sequences compare; other pairs raise `TypeError`.
pub fn py_lt(a: &PyValue, b: &PyValue) -> Result<bool, PyException> {
    let (a, b) = (a.peel(), b.peel());
    match (a, b) {
        (PyValue::List(x), PyValue::List(y)) | (PyValue::Tuple(x), PyValue::Tuple(y)) => {
            for (p, q) in x.iter().zip(y) {
                if !py_eq(p, q) {
                    return py_lt(p, q);
                }
            }
            Ok(x.len() < y.len())
        }
        (PyValue::Bytes(x), PyValue::Bytes(y)) => Ok(x < y),
        _ => compare(Compare::Less, a, b),
    }
}

fn lt_failure(error: PyException) -> Failure {
    Failure::from(error)
}

/// `sorted(items, key=...)` given the key of each item: the items in sorted order, or the first `TypeError`.
pub fn sorted_by_key<T>(items: Vec<T>, keys: &[PyValue]) -> Result<Vec<T>, Failure> {
    assert_eq!(items.len(), keys.len(), "one key per item");
    let order = sorted_order(keys)?;
    let mut slots: Vec<Option<T>> = items.into_iter().map(Some).collect();
    Ok(order
        .into_iter()
        .map(|index| slots[index].take().expect("each index appears once"))
        .collect())
}

/// The permutation `list.sort` produces for the keys.
fn sorted_order(keys: &[PyValue]) -> Result<Vec<usize>, Failure> {
    sort_with(keys.len(), &|left, right| {
        py_lt(&keys[left], &keys[right]).map_err(lt_failure)
    })
}

/// What [`comparison_schedule`] reports: the comparisons made and how the sort ended.
pub type ScheduleTrace = (Vec<(usize, usize)>, Result<Vec<usize>, Failure>);

/// The comparisons `list.sort` makes for the keys, as `(left, right)` key indices in call order (`left < right?`),
/// and the outcome. A failing comparison is the last pair. For tests that prove the schedule against CPython.
#[doc(hidden)]
pub fn comparison_schedule(keys: &[PyValue]) -> ScheduleTrace {
    let trace = std::cell::RefCell::new(Vec::new());
    let outcome = sort_with(keys.len(), &|left, right| {
        trace.borrow_mut().push((left, right));
        py_lt(&keys[left], &keys[right]).map_err(lt_failure)
    });
    (trace.into_inner(), outcome)
}

type Less<'a> = &'a dyn Fn(usize, usize) -> Result<bool, Failure>;

/// `MIN_GALLOP`: galloping mode starts when one run wins this many times in a row.
const MIN_GALLOP: usize = 7;

#[derive(Clone, Copy)]
struct Run {
    base: usize,
    len: usize,
    power: u32,
}

/// CPython's `MergeState`, without the temporary array (each merge uses a `Vec`).
struct MergeState {
    pending: Vec<Run>,
    min_gallop: usize,
    list_len: usize,
}

/// `list.sort` of `count` elements whose keys are compared through `less(left_key_index, right_key_index)`.
fn sort_with(count: usize, less: Less<'_>) -> Result<Vec<usize>, Failure> {
    let mut order: Vec<usize> = (0..count).collect();
    if count < 2 {
        return Ok(order);
    }
    let mut state = MergeState {
        pending: Vec::new(),
        min_gallop: MIN_GALLOP,
        list_len: count,
    };
    let min_run = merge_compute_minrun(count);
    let (mut low, mut remaining) = (0, count);
    loop {
        let (mut run, descending) = count_run(&order[low..low + remaining], less)?;
        if descending {
            order[low..low + run].reverse();
        }
        if run < min_run {
            let force = remaining.min(min_run);
            binary_insertion(&mut order, low, low + force, low + run, less)?;
            run = force;
        }
        state.found_new_run(&mut order, run, less)?;
        state.pending.push(Run {
            base: low,
            len: run,
            power: 0,
        });
        low += run;
        remaining -= run;
        if remaining == 0 {
            break;
        }
    }
    state.force_collapse(&mut order, less)?;
    Ok(order)
}

/// `merge_compute_minrun`: below 64 the whole list; otherwise 32..=64 so that `n / minrun` is close to, but below,
/// a power of two.
fn merge_compute_minrun(mut n: usize) -> usize {
    let mut any_bit_shifted_off = 0;
    while n >= 64 {
        any_bit_shifted_off |= n & 1;
        n >>= 1;
    }
    n + any_bit_shifted_off
}

/// `count_run`: the length of the run at the start of `keys` (at least one element) and whether it is strictly
/// descending (the caller reverses it).
fn count_run(keys: &[usize], less: Less<'_>) -> Result<(usize, bool), Failure> {
    if keys.len() == 1 {
        return Ok((1, false));
    }
    let mut length = 2;
    if less(keys[1], keys[0])? {
        while length < keys.len() && less(keys[length], keys[length - 1])? {
            length += 1;
        }
        Ok((length, true))
    } else {
        while length < keys.len() && !less(keys[length], keys[length - 1])? {
            length += 1;
        }
        Ok((length, false))
    }
}

/// `binarysort`: `order[low..start]` is sorted; insert `order[start..high]` one by one by bisection, after equal
/// keys (stable).
fn binary_insertion(
    order: &mut [usize],
    low: usize,
    high: usize,
    start: usize,
    less: Less<'_>,
) -> Result<(), Failure> {
    let start = if start == low { start + 1 } else { start };
    for position in start..high {
        let pivot = order[position];
        let (mut left, mut right) = (low, position);
        while left < right {
            let middle = left + ((right - left) >> 1);
            if less(pivot, order[middle])? {
                right = middle;
            } else {
                left = middle + 1;
            }
        }
        order.copy_within(left..position, left + 1);
        order[left] = pivot;
    }
    Ok(())
}

/// `gallop_left`: where `key` belongs in the sorted `a[..n]`, before any equal element, starting the search at
/// `hint`. Returns `k` with `a[k-1] < key <= a[k]`.
fn gallop_left(
    key: usize,
    a: &[usize],
    n: usize,
    hint: usize,
    less: Less<'_>,
) -> Result<usize, Failure> {
    let (n, hint) = (n as isize, hint as isize);
    let at = |offset: isize| a[(hint + offset) as usize];
    let (mut last_offset, mut offset) = (0isize, 1isize);
    if less(at(0), key)? {
        // a[hint] < key: gallop right until a[hint + last_offset] < key <= a[hint + offset]
        let max_offset = n - hint;
        while offset < max_offset {
            if less(at(offset), key)? {
                last_offset = offset;
                offset = (offset << 1) + 1;
            } else {
                break;
            }
        }
        offset = offset.min(max_offset);
        last_offset += hint;
        offset += hint;
    } else {
        // key <= a[hint]: gallop left until a[hint - offset] < key <= a[hint - last_offset]
        let max_offset = hint + 1;
        while offset < max_offset {
            if less(at(-offset), key)? {
                break;
            }
            last_offset = offset;
            offset = (offset << 1) + 1;
        }
        offset = offset.min(max_offset);
        let previous = last_offset;
        last_offset = hint - offset;
        offset = hint - previous;
    }
    // a[last_offset] < key <= a[offset]: bisect with the invariant a[last_offset-1] < key <= a[offset].
    last_offset += 1;
    while last_offset < offset {
        let middle = last_offset + ((offset - last_offset) >> 1);
        if less(a[middle as usize], key)? {
            last_offset = middle + 1;
        } else {
            offset = middle;
        }
    }
    Ok(offset as usize)
}

/// `gallop_right`: like [`gallop_left`], but after any equal element: `a[k-1] <= key < a[k]`.
fn gallop_right(
    key: usize,
    a: &[usize],
    n: usize,
    hint: usize,
    less: Less<'_>,
) -> Result<usize, Failure> {
    let (n, hint) = (n as isize, hint as isize);
    let at = |offset: isize| a[(hint + offset) as usize];
    let (mut last_offset, mut offset) = (0isize, 1isize);
    if less(key, at(0))? {
        // key < a[hint]: gallop left until a[hint - offset] <= key < a[hint - last_offset]
        let max_offset = hint + 1;
        while offset < max_offset {
            if less(key, at(-offset))? {
                last_offset = offset;
                offset = (offset << 1) + 1;
            } else {
                break;
            }
        }
        offset = offset.min(max_offset);
        let previous = last_offset;
        last_offset = hint - offset;
        offset = hint - previous;
    } else {
        // a[hint] <= key: gallop right until a[hint + last_offset] <= key < a[hint + offset]
        let max_offset = n - hint;
        while offset < max_offset {
            if less(key, at(offset))? {
                break;
            }
            last_offset = offset;
            offset = (offset << 1) + 1;
        }
        offset = offset.min(max_offset);
        last_offset += hint;
        offset += hint;
    }
    last_offset += 1;
    while last_offset < offset {
        let middle = last_offset + ((offset - last_offset) >> 1);
        if less(key, a[middle as usize])? {
            offset = middle;
        } else {
            last_offset = middle + 1;
        }
    }
    Ok(offset as usize)
}

/// `powerloop`: the "power" (depth in the conceptual binary merge tree) of the boundary between two adjacent runs
/// of lengths `n1` (starting at `s1`) and `n2` in a list of `n` elements.
fn powerloop(s1: usize, n1: usize, n2: usize, n: usize) -> u32 {
    let mut result = 0;
    let mut a = 2 * s1 + n1;
    let mut b = a + n1 + n2;
    loop {
        result += 1;
        if a >= n {
            a -= n;
            b -= n;
        } else if b >= n {
            break;
        }
        a <<= 1;
        b <<= 1;
    }
    result
}

/// How a `merge_lo`/`merge_hi` ends: all merged, or the one remaining element of the other run is placed last/first.
enum Ending {
    Succeed,
    CopyLast,
}

impl MergeState {
    /// `found_new_run`: a run of length `next_len` starts after the pending ones; apply the powersort policy.
    fn found_new_run(
        &mut self,
        order: &mut [usize],
        next_len: usize,
        less: Less<'_>,
    ) -> Result<(), Failure> {
        let Some(&top) = self.pending.last() else {
            return Ok(());
        };
        let power = powerloop(top.base, top.len, next_len, self.list_len);
        while self.pending.len() > 1 && self.pending[self.pending.len() - 2].power > power {
            self.merge_at(order, self.pending.len() - 2, less)?;
        }
        let last = self.pending.len() - 1;
        self.pending[last].power = power;
        Ok(())
    }

    /// `merge_force_collapse`: merge everything that is left.
    fn force_collapse(&mut self, order: &mut [usize], less: Less<'_>) -> Result<(), Failure> {
        while self.pending.len() > 1 {
            let mut index = self.pending.len() - 2;
            if index > 0 && self.pending[index - 1].len < self.pending[index + 1].len {
                index -= 1;
            }
            self.merge_at(order, index, less)?;
        }
        Ok(())
    }

    /// `merge_at`: merge the runs at stack positions `index` and `index + 1`.
    fn merge_at(
        &mut self,
        order: &mut [usize],
        index: usize,
        less: Less<'_>,
    ) -> Result<(), Failure> {
        let (first, second) = (self.pending[index], self.pending[index + 1]);
        let (mut start_a, mut len_a, start_b, mut len_b) =
            (first.base, first.len, second.base, second.len);
        self.pending[index].len = len_a + len_b;
        if index + 3 == self.pending.len() {
            self.pending[index + 1] = self.pending[index + 2];
        }
        self.pending.pop();

        // Where does b start in a? Elements of a before that are already in place.
        let skip = gallop_right(order[start_b], &order[start_a..], len_a, 0, less)?;
        start_a += skip;
        len_a -= skip;
        if len_a == 0 {
            return Ok(());
        }
        // Where does a end in b? Elements of b after that are already in place.
        len_b = gallop_left(
            order[start_a + len_a - 1],
            &order[start_b..],
            len_b,
            len_b - 1,
            less,
        )?;
        if len_b == 0 {
            return Ok(());
        }
        if len_a <= len_b {
            self.merge_low(order, start_a, len_a, start_b, len_b, less)
        } else {
            self.merge_high(order, start_a, len_a, start_b, len_b, less)
        }
    }

    /// `merge_lo`: merge a (copied out) into the front, `len_a <= len_b`.
    fn merge_low(
        &mut self,
        order: &mut [usize],
        start_a: usize,
        mut len_a: usize,
        start_b: usize,
        mut len_b: usize,
        less: Less<'_>,
    ) -> Result<(), Failure> {
        let temp: Vec<usize> = order[start_a..start_a + len_a].to_vec();
        let (mut dest, mut a, mut b) = (start_a, 0usize, start_b);
        order[dest] = order[b];
        dest += 1;
        b += 1;
        len_b -= 1;
        let ending = 'merge: {
            if len_b == 0 {
                break 'merge Ending::Succeed;
            }
            if len_a == 1 {
                break 'merge Ending::CopyLast;
            }
            let mut min_gallop = self.min_gallop;
            loop {
                let (mut a_wins, mut b_wins) = (0usize, 0usize);
                // Straightforward merging until one run seems to win consistently.
                loop {
                    if less(order[b], temp[a])? {
                        order[dest] = order[b];
                        dest += 1;
                        b += 1;
                        b_wins += 1;
                        a_wins = 0;
                        len_b -= 1;
                        if len_b == 0 {
                            break 'merge Ending::Succeed;
                        }
                        if b_wins >= min_gallop {
                            break;
                        }
                    } else {
                        order[dest] = temp[a];
                        dest += 1;
                        a += 1;
                        a_wins += 1;
                        b_wins = 0;
                        len_a -= 1;
                        if len_a == 1 {
                            break 'merge Ending::CopyLast;
                        }
                        if a_wins >= min_gallop {
                            break;
                        }
                    }
                }
                // Galloping mode, until neither run wins consistently any more.
                min_gallop += 1;
                loop {
                    min_gallop -= usize::from(min_gallop > 1);
                    self.min_gallop = min_gallop;
                    let k = gallop_right(order[b], &temp[a..], len_a, 0, less)?;
                    a_wins = k;
                    if k > 0 {
                        order[dest..dest + k].copy_from_slice(&temp[a..a + k]);
                        dest += k;
                        a += k;
                        len_a -= k;
                        if len_a == 1 {
                            break 'merge Ending::CopyLast;
                        }
                        if len_a == 0 {
                            break 'merge Ending::Succeed;
                        }
                    }
                    order[dest] = order[b];
                    dest += 1;
                    b += 1;
                    len_b -= 1;
                    if len_b == 0 {
                        break 'merge Ending::Succeed;
                    }
                    let k = gallop_left(temp[a], &order[b..], len_b, 0, less)?;
                    b_wins = k;
                    if k > 0 {
                        order.copy_within(b..b + k, dest);
                        dest += k;
                        b += k;
                        len_b -= k;
                        if len_b == 0 {
                            break 'merge Ending::Succeed;
                        }
                    }
                    order[dest] = temp[a];
                    dest += 1;
                    a += 1;
                    len_a -= 1;
                    if len_a == 1 {
                        break 'merge Ending::CopyLast;
                    }
                    if !(a_wins >= MIN_GALLOP || b_wins >= MIN_GALLOP) {
                        break;
                    }
                }
                min_gallop += 1; // penalise leaving galloping mode
                self.min_gallop = min_gallop;
            }
        };
        match ending {
            Ending::Succeed => {
                if len_a > 0 {
                    order[dest..dest + len_a].copy_from_slice(&temp[a..a + len_a]);
                }
            }
            Ending::CopyLast => {
                // The last element of a belongs at the end of the merge.
                order.copy_within(b..b + len_b, dest);
                order[dest + len_b] = temp[a];
            }
        }
        Ok(())
    }

    /// `merge_hi`: merge b (copied out) into the back, `len_a >= len_b`.
    fn merge_high(
        &mut self,
        order: &mut [usize],
        start_a: usize,
        mut len_a: usize,
        start_b: usize,
        mut len_b: usize,
        less: Less<'_>,
    ) -> Result<(), Failure> {
        let temp: Vec<usize> = order[start_b..start_b + len_b].to_vec();
        let mut dest = (start_b + len_b - 1) as isize;
        let mut a = (start_a + len_a - 1) as isize;
        let mut b = (len_b - 1) as isize;
        order[dest as usize] = order[a as usize];
        dest -= 1;
        a -= 1;
        len_a -= 1;
        let ending = 'merge: {
            if len_a == 0 {
                break 'merge Ending::Succeed;
            }
            if len_b == 1 {
                break 'merge Ending::CopyLast;
            }
            let mut min_gallop = self.min_gallop;
            loop {
                let (mut a_wins, mut b_wins) = (0usize, 0usize);
                loop {
                    if less(temp[b as usize], order[a as usize])? {
                        order[dest as usize] = order[a as usize];
                        dest -= 1;
                        a -= 1;
                        a_wins += 1;
                        b_wins = 0;
                        len_a -= 1;
                        if len_a == 0 {
                            break 'merge Ending::Succeed;
                        }
                        if a_wins >= min_gallop {
                            break;
                        }
                    } else {
                        order[dest as usize] = temp[b as usize];
                        dest -= 1;
                        b -= 1;
                        b_wins += 1;
                        a_wins = 0;
                        len_b -= 1;
                        if len_b == 1 {
                            break 'merge Ending::CopyLast;
                        }
                        if b_wins >= min_gallop {
                            break;
                        }
                    }
                }
                min_gallop += 1;
                loop {
                    min_gallop -= usize::from(min_gallop > 1);
                    self.min_gallop = min_gallop;
                    let k =
                        gallop_right(temp[b as usize], &order[start_a..], len_a, len_a - 1, less)?;
                    let k = len_a - k;
                    a_wins = k;
                    if k > 0 {
                        dest -= k as isize;
                        a -= k as isize;
                        let (to, from) = ((dest + 1) as usize, (a + 1) as usize);
                        order.copy_within(from..from + k, to);
                        len_a -= k;
                        if len_a == 0 {
                            break 'merge Ending::Succeed;
                        }
                    }
                    order[dest as usize] = temp[b as usize];
                    dest -= 1;
                    b -= 1;
                    len_b -= 1;
                    if len_b == 1 {
                        break 'merge Ending::CopyLast;
                    }
                    let k = gallop_left(order[a as usize], &temp, len_b, len_b - 1, less)?;
                    let k = len_b - k;
                    b_wins = k;
                    if k > 0 {
                        dest -= k as isize;
                        b -= k as isize;
                        let (to, from) = ((dest + 1) as usize, (b + 1) as usize);
                        order[to..to + k].copy_from_slice(&temp[from..from + k]);
                        len_b -= k;
                        if len_b == 1 {
                            break 'merge Ending::CopyLast;
                        }
                        if len_b == 0 {
                            break 'merge Ending::Succeed;
                        }
                    }
                    order[dest as usize] = order[a as usize];
                    dest -= 1;
                    a -= 1;
                    len_a -= 1;
                    if len_a == 0 {
                        break 'merge Ending::Succeed;
                    }
                    if !(a_wins >= MIN_GALLOP || b_wins >= MIN_GALLOP) {
                        break;
                    }
                }
                min_gallop += 1;
                self.min_gallop = min_gallop;
            }
        };
        match ending {
            Ending::Succeed => {
                if len_b > 0 {
                    let to = (dest + 1) as usize - len_b;
                    order[to..to + len_b].copy_from_slice(&temp[..len_b]);
                }
            }
            Ending::CopyLast => {
                // The first element of b belongs at the front of the merge.
                let to = (dest + 1) as usize - len_a;
                let from = (a + 1) as usize - len_a;
                order.copy_within(from..from + len_a, to);
                dest -= len_a as isize;
                order[dest as usize] = temp[b as usize];
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ints(values: &[i64]) -> Vec<PyValue> {
        values.iter().map(|&v| PyValue::int(v)).collect()
    }

    #[test]
    fn sorting_is_stable_and_matches_python_for_small_lists() {
        let keys = ints(&[3, 1, 2, 1, 3, 0]);
        let sorted = sorted_by_key(vec!['a', 'b', 'c', 'd', 'e', 'f'], &keys).unwrap();
        assert_eq!(sorted, vec!['f', 'b', 'd', 'c', 'a', 'e']);
    }

    #[test]
    fn mixed_keys_report_the_first_failing_comparison() {
        let keys = vec![PyValue::int(1), PyValue::str("a")];
        let error = sorted_by_key(vec![0, 1], &keys).unwrap_err();
        assert_eq!(error.class, "TypeError");
        assert_eq!(
            error.message.to_utf8().unwrap(),
            "'<' not supported between instances of 'str' and 'int'"
        );
    }

    #[test]
    fn lists_compare_lexicographically_and_equal_across_numeric_types() {
        let a = PyValue::List(ints(&[1, 2]));
        let b = PyValue::List(vec![PyValue::Float(1.0), PyValue::int(3)]);
        assert!(py_lt(&a, &b).unwrap());
        assert!(py_eq(&PyValue::Bool(true), &PyValue::Float(1.0)));
        assert!(!py_eq(&PyValue::Float(f64::NAN), &PyValue::Float(f64::NAN)));
    }

    #[test]
    fn long_lists_sort_stably() {
        let values: Vec<i64> = (0..200).map(|i| (i * 37) % 11).collect();
        let keys = ints(&values);
        let sorted = sorted_by_key((0..200).collect::<Vec<usize>>(), &keys).unwrap();
        for pair in sorted.windows(2) {
            let (a, b) = (values[pair[0]], values[pair[1]]);
            assert!(a < b || (a == b && pair[0] < pair[1]));
        }
    }
}
