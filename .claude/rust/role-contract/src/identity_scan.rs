//! Which lines of `grep -E '^self_role:...' FILE` GNU grep 3.11 prints, and whether it adds its
//! `grep: FILE: binary file matches` notice, for the one fixed pattern the node-identity resolver uses.
//!
//! Source-derived from the official `grep-3.11/src/grep.c` and gnulib `xpalloc` (read, not executed): `grep()` (the
//! per-buffer loop), `fillbuf()` (read sizes, saved incomplete line, buffer growth), `print_line_head()` (a selected line
//! with an encoding error is withheld and `encoding_error_output` is set, the scan goes on), `buf_has_nulls()` (NUL
//! detection looks at the bytes of each newly read buffer) and `main()` (`bufalloc = ALIGN_TO (INITIAL_BUFSIZE, pagesize)
//! + pagesize + uword_size`, the page size from `sysconf (_SC_PAGESIZE)`). The rules:
//!
//! * a selected line that is valid is printed, one that is not (UTF-8 locale) is not printed and sets the notice flag;
//!   later lines are still looked at;
//! * the first read buffer that contains a NUL switches the scan to "quiet" for that buffer and every later one: NUL
//!   then ends a line like a newline, the first selected line completed from that buffer on is only counted, and the
//!   notice is added when such a line exists (`nlines_first_null < nlines`). Lines completed in earlier buffers were
//!   printed normally and stay printed;
//! * which buffer a byte belongs to follows from `fillbuf`: the first read is `ALIGN_TO(96 KiB, pagesize)` bytes; before
//!   each later read the saved incomplete line `s` is moved to the page-aligned read position of the buffer; when
//!   `s + P` does not fit (`incr_min = s + P - bufalloc + P + 8 > 0`) the buffer grows (`xpalloc`: +50%, at least
//!   `incr_min`, at most the file's remaining bytes plus the saved line and slop) into a **new allocation**.
//!
//! **Genuine upstream nondeterminism.** Where the page-aligned read position falls depends on the address of grep's
//! buffer modulo the page size (and, after growth, of the new allocation). Upstream grep makes the binary heuristic
//! buffer- and host-dependent; the contract is the *set* of outputs GNU permits, not one shared heap address. This
//! model performs real native allocations of the same sizes and uses their actual addresses, so the answer is one
//! member of that set - the same kind of member another process's allocation could produce, not a claim of the original
//! process's heap. [`permitted_outcomes`] enumerates the set (every 16-byte-aligned residue of the initial buffer, and of
//! every grown buffer) for source-derived range checks. Inputs whose output does not depend on any of this (no NUL, a
//! NUL in the first read, lines far from the buffer boundaries) have a one-element set.

use super::Profile;

/// The pattern's literal prefix: `^self_role:` (the trailing `[[:space:]]*` of the original always matches).
const SELECTED_PREFIX: &[u8] = b"self_role:";
/// `INITIAL_BUFSIZE` of grep 3.11.
const INITIAL_BUFSIZE: usize = 96 * 1024;
/// `uword_size` (a `uintmax_t`) in `fillbuf`'s `min_after_buflim` and `readsize` on the 64-bit Linux GNU platform.
const UWORD_SIZE: usize = 8;
/// glibc `malloc` alignment on the 64-bit Linux GNU platform.
const MALLOC_ALIGN: usize = 16;

/// What grep leaves for the pattern.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Outcome {
    /// Byte ranges of the selected lines grep prints, in order (the resolver reads the first, `head -1`).
    pub printed: Vec<(usize, usize)>,
    /// Whether `grep: FILE: binary file matches` is written to stderr.
    pub notice: bool,
}

struct Selected {
    start: usize,
    end: usize,
    /// Position of the terminating newline or NUL (`None`: the final line has no terminator).
    terminator: Option<usize>,
    encoding_error: bool,
}

fn selected_lines(bytes: &[u8], profile: Profile) -> Vec<Selected> {
    let mut out = Vec::new();
    let mut start = 0;
    let mut line = |start: usize, end: usize, terminator: Option<usize>| {
        if bytes[start..end].starts_with(SELECTED_PREFIX) {
            out.push(Selected {
                start,
                end,
                terminator,
                encoding_error: profile == Profile::CUtf8
                    && std::str::from_utf8(&bytes[start..end]).is_err(),
            });
        }
    };
    for (index, byte) in bytes.iter().enumerate() {
        if *byte == b'\n' || *byte == 0 {
            line(start, index, Some(index));
            start = index + 1;
        }
    }
    if start < bytes.len() {
        line(start, bytes.len(), None);
    }
    out
}

/// The positions of the newlines: before the first NUL these are the only line ends.
fn newline_positions(bytes: &[u8]) -> Vec<usize> {
    bytes
        .iter()
        .enumerate()
        .filter_map(|(at, byte)| (*byte == b'\n').then_some(at))
        .collect()
}

fn align_to(value: usize, alignment: usize) -> usize {
    value.div_ceil(alignment) * alignment
}

/// `fillbuf` over the file's bytes, up to the read that holds the first NUL: the start offset of that read.
/// `allocate(size)` returns the address of a new allocation of `size` bytes (the buffer of `main()` first, grown buffers
/// of `xpalloc` after it).
fn detection_start(
    bytes: &[u8],
    newlines: &[usize],
    first_nul: usize,
    page: usize,
    allocate: &mut dyn FnMut(usize) -> usize,
) -> usize {
    let length = bytes.len();
    let initial = align_to(INITIAL_BUFSIZE, page);
    let min_after_buflim = page + UWORD_SIZE;
    let mut bufalloc = initial + page + UWORD_SIZE;
    let mut buffer = allocate(bufalloc);
    // reset(): bufbeg = buflim = ALIGN_TO (buffer + 1, pagesize)
    let mut buflim = align_to(buffer + 1, page);
    let mut file_pos = 0usize;
    let mut save = 0usize;
    loop {
        // fillbuf (save, st)
        let readbuf = if min_after_buflim <= buffer + bufalloc - buflim {
            buflim
        } else {
            let minsize = save + page;
            let incr_min = (minsize + min_after_buflim) as i128 - bufalloc as i128;
            let newbuf = if incr_min <= 0 {
                buffer
            } else {
                let incr_min = incr_min as usize;
                // alloc_max: the file is a regular file, its size is usable
                let to_be_read = length - file_pos;
                let alloc_max = (to_be_read + save + min_after_buflim).max(bufalloc + incr_min);
                // xpalloc (NULL, &bufalloc, incr_min, alloc_max, 1)
                let n0 = bufalloc;
                let mut n = (n0 + (n0 >> 1)).min(alloc_max);
                if n - n0 < incr_min {
                    n = n0 + incr_min;
                }
                bufalloc = n;
                allocate(n)
            };
            let readbuf = align_to(newbuf + 1 + save, page);
            if newbuf != buffer {
                buffer = newbuf;
            }
            readbuf
        };
        let mut readsize = buffer + bufalloc - UWORD_SIZE - readbuf;
        readsize -= readsize % page;
        let start = file_pos;
        let end = (start + readsize).min(length);
        file_pos = end;
        buflim = readbuf + (end - start);
        if first_nul < end {
            return start;
        }
        if end == start {
            // end of input without a NUL (callers only ask when there is one)
            return length;
        }
        // before the first NUL only a newline ends a line; the saved part is what follows the last one
        let before = newlines.partition_point(|at| *at < end);
        let last_newline_end = before.checked_sub(1).map_or(0, |last| newlines[last] + 1);
        save = end - last_newline_end;
    }
}

fn outcome_for(lines: &[Selected], quiet_from: Option<usize>) -> Outcome {
    let mut printed = Vec::new();
    let mut notice = false;
    for line in lines {
        let quiet = match (quiet_from, line.terminator) {
            (None, _) => false,
            (Some(from), Some(t)) => t >= from,
            (Some(_), None) => true,
        };
        if quiet {
            notice = true;
            break;
        }
        if line.encoding_error {
            notice = true;
        } else {
            printed.push((line.start, line.end));
        }
    }
    Outcome { printed, notice }
}

/// Decided without any allocation: no NUL at all, or no selected line, or a NUL in the first read (always
/// `ALIGN_TO(96 KiB, pagesize)` bytes, whatever the address).
fn decided(bytes: &[u8], lines: &[Selected], page: usize) -> Option<Outcome> {
    let Some(first_nul) = bytes.iter().position(|b| *b == 0) else {
        return Some(outcome_for(lines, None));
    };
    if lines.is_empty() {
        return Some(outcome_for(lines, None));
    }
    (first_nul < align_to(INITIAL_BUFSIZE, page)).then(|| outcome_for(lines, Some(0)))
}

/// The lines grep prints and whether it adds the notice, for a file's bytes under the profile, with grep's buffers
/// allocated natively (one member of the permitted set; see the module documentation).
pub fn grep_self_role(bytes: &[u8], profile: Profile, page_size: usize) -> Outcome {
    let lines = selected_lines(bytes, profile);
    if let Some(outcome) = decided(bytes, &lines, page_size) {
        return outcome;
    }
    let first_nul = bytes
        .iter()
        .position(|b| *b == 0)
        .expect("decided() answered the no-NUL case");
    // Real allocations of the sizes grep 3.11 asks for; only their addresses are used. Kept alive until the end.
    let mut held: Vec<Vec<u8>> = Vec::new();
    let mut allocate = |size: usize| {
        let block: Vec<u8> = Vec::with_capacity(size);
        let address = block.as_ptr() as usize;
        held.push(block);
        address
    };
    let newlines = newline_positions(bytes);
    let from = detection_start(bytes, &newlines, first_nul, page_size, &mut allocate);
    outcome_for(&lines, Some(from))
}

/// Every output GNU permits for the input: the initial buffer at each 16-byte-aligned address residue, grown buffers
/// likewise (all growth allocations of one run share one residue). For source-derived range checks; with growth the
/// cost is `(page / 16)^2` simulations, so checks use it with the 4096-byte page.
pub fn permitted_outcomes(bytes: &[u8], profile: Profile, page_size: usize) -> Vec<Outcome> {
    let lines = selected_lines(bytes, profile);
    if let Some(outcome) = decided(bytes, &lines, page_size) {
        return vec![outcome];
    }
    let first_nul = bytes
        .iter()
        .position(|b| *b == 0)
        .expect("decided() answered the no-NUL case");
    let newlines = newline_positions(bytes);
    let mut set: Vec<Outcome> = Vec::new();
    for first in (0..page_size).step_by(MALLOC_ALIGN) {
        for grown in (0..page_size).step_by(MALLOC_ALIGN) {
            let mut calls = 0usize;
            let mut allocate = |_size: usize| {
                calls += 1;
                // distinct, page-aligned bases keep every address residue exact
                let base = page_size * 1024;
                base + if calls == 1 { first } else { grown }
            };
            let from = detection_start(bytes, &newlines, first_nul, page_size, &mut allocate);
            let outcome = outcome_for(&lines, Some(from));
            if !set.contains(&outcome) {
                set.push(outcome);
            }
            if calls == 1 {
                break; // no growth happened: the second residue is irrelevant
            }
        }
    }
    set
}
