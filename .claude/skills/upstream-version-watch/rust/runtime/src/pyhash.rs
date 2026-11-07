//! CPython 3.12.3 hashing of `str` and `tuple` values and the slot layout of a `frozenset` built from an iterable.
//!
//! Python iterates a `set` in hash-table slot order, so code that walks a `frozenset` of keys sees an order that depends on the hash of every key,
//! on the table size and on collisions. This module reproduces that order for one profile only: CPython 3.12.3, 64-bit little-endian host,
//! `sys.hash_info.algorithm == "siphash13"`, `hash_info.cutoff == 0` (small-string optimisation off, so strings of every length take SipHash-1-3).
//! Written from `Python/pyhash.c`, `Python/bootstrap_hash.c`, `Objects/unicodeobject.c`, `Objects/tupleobject.c` and `Objects/setobject.c` of v3.12.3.
//! The SipHash code follows `Python/pyhash.c` (MIT, Copyright (c) 2013 Marek Majkowski) and the module is derived from CPython under the PSF License
//! Version 2: the license texts and the summary of changes ship as `notices/NOTICE-pyhash.md` next to this crate's other notices.
//!
//! Not covered: 32-bit and big-endian builds, the FNV/SipHash-2-4 builds, `Py_HASH_CUTOFF > 0`; hashing of values other than `str` and `tuple`
//! (the caller supplies those hashes as [`PyHash`]); the `-R` interpreter option (it turns randomization on and overrides a configured fixed seed,
//! whereas seed 0 disables it); the interpreter's own failure report for an invalid `PYTHONHASHSEED` (that parser, `Python/initconfig.c`, is not among
//! the primary sources; [`parse_hash_seed`] refuses such a value by name); deletion, dummy slots and set-to-set copies, none of which a freshly built
//! `frozenset` has.
//!
//! Production seed source: [`parse_hash_seed`] selects the setting from `PYTHONHASHSEED` (unset, empty or `random` = randomized; a plain integer in
//! `[0; 4294967295]` = fixed), [`HashSecret::for_setting`] builds the secret as `_Py_HashRandomization_Init` does (fixed: [`HashSecret::from_seed`];
//! random: the whole 24-byte secret from an injectable entropy source, called once per process) and [`os_entropy`] is the process's source.

use agent_runtime_support::pystr::PyStr;

// The slot arithmetic below is CPython's `size_t` arithmetic on a 64-bit host.
const _: () = assert!(usize::BITS == 64);

/// A `Py_hash_t` as `hash()` returns it: a signed 64-bit value that is never `-1` (CPython reserves `-1` for "error").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PyHash(i64);

impl PyHash {
    /// The hash of an object whose `__hash__` returned `raw`, as `hash(obj)` shows it: `-1` becomes `-2`, every other `i64` is kept.
    ///
    /// The `-1 -> -2` rule is the one `_Py_HashBytes` applies to strings; for a Python-level `__hash__` it is taken from the saved CPython 3.12.3
    /// observation (a key whose `__hash__` returns `-1` is stored with hash `-2`), not from a file of the primary-source set. A `__hash__` result
    /// outside the `i64` range is reduced by CPython and is not supported here.
    pub fn from_dunder_hash_result(raw: i64) -> Self {
        PyHash(if raw == -1 { -2 } else { raw })
    }

    /// The value `hash()` returns.
    pub fn value(self) -> i64 {
        self.0
    }

    /// The same 64 bits as `Py_uhash_t`, the type the tuple hash does its arithmetic in.
    fn as_unsigned(self) -> u64 {
        self.0 as u64
    }
}

/// `sizeof(_Py_HashSecret_t)` on a 64-bit host.
const SECRET_BYTES: usize = 24;

/// The 16 secret bytes SipHash-1-3 uses: `_Py_HashSecret.siphash.k0` and `.k1`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HashSecret {
    k0: u64,
    k1: u64,
}

impl HashSecret {
    /// The secret of an interpreter started with the fixed hash seed `seed` (`PYTHONHASHSEED=<seed>`).
    ///
    /// Seed `0` disables randomization and leaves the whole 24-byte secret zero. Any other seed fills the 24 bytes with CPython's linear
    /// congruential generator (`lcg_urandom`: `x = x * 214013 + 2531011` modulo 2^32, one byte per step from bits 23..16); SipHash reads the first
    /// 16 bytes as two little-endian 64-bit words. The remaining 8 bytes (the small-string suffix) are unused while `Py_HASH_CUTOFF` is 0.
    pub fn from_seed(seed: u32) -> Self {
        let mut bytes = [0u8; SECRET_BYTES];
        if seed != 0 {
            let mut state = seed;
            for byte in &mut bytes {
                state = state.wrapping_mul(214_013).wrapping_add(2_531_011);
                *byte = (state >> 16) as u8;
            }
        }
        HashSecret::from_secret_bytes(&bytes)
    }

    /// The secret of a 24-byte `_Py_HashSecret` filled by the caller (SipHash reads the first 16 bytes as two little-endian words).
    pub fn from_secret_bytes(bytes: &[u8; SECRET_BYTES]) -> Self {
        HashSecret {
            k0: u64::from_le_bytes(bytes[0..8].try_into().expect("8 bytes")),
            k1: u64::from_le_bytes(bytes[8..16].try_into().expect("8 bytes")),
        }
    }

    /// `_Py_HashRandomization_Init` of CPython 3.12.3 for `setting`: a fixed seed is expanded as [`HashSecret::from_seed`] does, a random start
    /// fills the whole secret from `entropy` ONCE (the caller calls this once per process). The entropy source is injected so that the
    /// source-derived branch is testable; the process uses [`os_entropy`].
    pub fn for_setting(
        setting: HashSeedSetting,
        entropy: &mut dyn FnMut(&mut [u8]) -> std::io::Result<()>,
    ) -> std::io::Result<HashSecret> {
        match setting {
            HashSeedSetting::Fixed(seed) => Ok(HashSecret::from_seed(seed)),
            HashSeedSetting::Random => {
                let mut bytes = [0u8; SECRET_BYTES];
                entropy(&mut bytes)?;
                Ok(HashSecret::from_secret_bytes(&bytes))
            }
        }
    }
}

/// The hash randomization the interpreter would start with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HashSeedSetting {
    /// `use_hash_seed == 0`: the secret comes from the operating system.
    Random,
    /// `PYTHONHASHSEED=<n>`: `0` disables randomization, any other value seeds the generator of [`HashSecret::from_seed`].
    Fixed(u32),
}

/// A `PYTHONHASHSEED` value this port does not decide.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HashSeedRefusal {
    pub text: Vec<u8>,
}

/// The setting `PYTHONHASHSEED` selects: unset or empty (CPython's `_Py_GetEnv` treats an empty value as unset) and `random` start randomized; a
/// plain decimal integer in `[0; 4294967295]` is a fixed seed. Anything else is refused by name: the interpreter's fatal start-up report for it
/// lives in `Python/initconfig.c`, which is not among the primary sources this port was built from, so it is neither guessed nor replaced by a
/// random seed.
pub fn parse_hash_seed(value: Option<&[u8]>) -> Result<HashSeedSetting, HashSeedRefusal> {
    let Some(text) = value.filter(|text| !text.is_empty()) else {
        return Ok(HashSeedSetting::Random);
    };
    if text == b"random" {
        return Ok(HashSeedSetting::Random);
    }
    let refusal = || HashSeedRefusal {
        text: text.to_vec(),
    };
    if !text.iter().all(u8::is_ascii_digit) {
        return Err(refusal());
    }
    let digits = std::str::from_utf8(text).map_err(|_| refusal())?;
    digits
        .parse::<u32>()
        .map(HashSeedSetting::Fixed)
        .map_err(|_| refusal())
}

/// `pyurandom(secret, size, blocking=0, raise=0)` through the kernel's non-blocking random source (`/dev/urandom`; the interpreter prefers the
/// `getrandom` system call, which feeds the same pool and is not reachable from the standard library alone).
pub fn os_entropy(buffer: &mut [u8]) -> std::io::Result<()> {
    use std::io::Read;
    std::fs::File::open("/dev/urandom")?.read_exact(buffer)
}

/// `hash(text)` of a Python `str`.
///
/// CPython hashes the text's PEP 393 buffer: one, two or four bytes per code point, chosen by the largest code point (up to U+00FF, up to U+FFFF,
/// otherwise), little-endian on this profile. A lone surrogate is a two-byte code point (U+D800..U+DFFF). The empty string hashes to 0 whatever
/// the secret is.
pub fn hash_str(text: &PyStr, secret: &HashSecret) -> PyHash {
    let points = text.code_points();
    let width = code_unit_width(points);
    let mut buffer = Vec::with_capacity(points.len() * width);
    for &point in points {
        buffer.extend_from_slice(&point.to_le_bytes()[..width]);
    }
    hash_bytes(&buffer, secret)
}

/// Bytes per code unit of the PEP 393 representation of `points`.
fn code_unit_width(points: &[u32]) -> usize {
    match points.iter().copied().max().unwrap_or(0) {
        0..=0xFF => 1,
        0x100..=0xFFFF => 2,
        _ => 4,
    }
}

/// `_Py_HashBytes` for the SipHash-1-3 build with a zero small-string cutoff.
fn hash_bytes(bytes: &[u8], secret: &HashSecret) -> PyHash {
    if bytes.is_empty() {
        return PyHash(0);
    }
    PyHash::from_dunder_hash_result(siphash13(secret.k0, secret.k1, bytes) as i64)
}

/// One SipHash half round (`HALF_ROUND` of `pyhash.c`).
fn half_round(a: &mut u64, b: &mut u64, c: &mut u64, d: &mut u64, rotate_b: u32, rotate_d: u32) {
    *a = a.wrapping_add(*b);
    *c = c.wrapping_add(*d);
    *b = b.rotate_left(rotate_b) ^ *a;
    *d = d.rotate_left(rotate_d) ^ *c;
    *a = a.rotate_left(32);
}

/// One SipHash round (`SINGLE_ROUND`): the two half rounds in CPython's operand order.
fn sip_round(v: &mut [u64; 4]) {
    let [v0, v1, v2, v3] = v;
    half_round(v0, v1, v2, v3, 13, 16);
    half_round(v2, v1, v0, v3, 17, 21);
}

/// `siphash13` of `pyhash.c`: one compression round per 8-byte word and three finalization rounds. The result is the plain xor of the four state
/// words (CPython's "modified" finalization), not the stock SipHash output.
fn siphash13(k0: u64, k1: u64, bytes: &[u8]) -> u64 {
    let mut v = [
        k0 ^ 0x736f_6d65_7073_6575,
        k1 ^ 0x646f_7261_6e64_6f6d,
        k0 ^ 0x6c79_6765_6e65_7261,
        k1 ^ 0x7465_6462_7974_6573,
    ];
    let (words, leftover) = bytes.as_chunks::<8>();
    for word in words {
        let message = u64::from_le_bytes(*word);
        v[3] ^= message;
        sip_round(&mut v);
        v[0] ^= message;
    }
    // The last word holds the 0-7 leftover bytes in its low bytes and the length (modulo 256) in its top byte.
    let mut tail = [0u8; 8];
    tail[..leftover.len()].copy_from_slice(leftover);
    let last = u64::from_le_bytes(tail) | ((bytes.len() as u64) << 56);
    v[3] ^= last;
    sip_round(&mut v);
    v[0] ^= last;
    v[2] ^= 0xff;
    for _ in 0..3 {
        sip_round(&mut v);
    }
    (v[0] ^ v[1]) ^ (v[2] ^ v[3])
}

const XX_PRIME_1: u64 = 11_400_714_785_074_694_791;
const XX_PRIME_2: u64 = 14_029_467_366_897_019_727;
const XX_PRIME_5: u64 = 2_870_177_450_012_600_261;

/// The hash `tuplehash` returns when its accumulator would be `-1`.
const TUPLE_HASH_WHEN_ACCUMULATOR_IS_MINUS_ONE: i64 = 1_546_275_796;

/// `hash(tuple)` from the hashes of its items, in order (a simplified xxHash with one accumulator, 64-bit constants).
///
/// Each item hash is one lane taken as its unsigned 64-bit image; all arithmetic wraps modulo 2^64. The length is mixed in last. An accumulator of
/// all ones (which would read as `-1`) is replaced by the fixed `1546275796`.
pub fn hash_tuple(item_hashes: &[PyHash]) -> PyHash {
    let mut accumulator = XX_PRIME_5;
    for lane in item_hashes {
        accumulator = accumulator.wrapping_add(lane.as_unsigned().wrapping_mul(XX_PRIME_2));
        accumulator = accumulator.rotate_left(31);
        accumulator = accumulator.wrapping_mul(XX_PRIME_1);
    }
    accumulator = accumulator.wrapping_add((item_hashes.len() as u64) ^ (XX_PRIME_5 ^ 3_527_539));
    if accumulator == u64::MAX {
        return PyHash(TUPLE_HASH_WHEN_ACCUMULATOR_IS_MINUS_ONE);
    }
    PyHash(accumulator as i64)
}

/// `PySet_MINSIZE`: the table of an empty set.
const MINIMUM_TABLE_SIZE: usize = 8;
/// `LINEAR_PROBES`: slots after the probed one that are tried before the probe sequence jumps.
const LINEAR_PROBES: u64 = 9;
/// `PERTURB_SHIFT`.
const PERTURB_SHIFT: u32 = 5;
/// A table is rebuilt larger once `used * 5 >= mask * 3` (`fill * 5 >= mask * 3`; fill equals used while nothing is deleted).
const GROWTH_FILL_FACTOR: (u64, u64) = (5, 3);
/// Above this many entries a rebuild doubles the entry count, below it quadruples it.
const QUADRUPLING_LIMIT: u64 = 50_000;

/// An occupied hash-table slot: the stored hash and which input item holds the slot.
#[derive(Clone, Copy)]
struct Slot {
    hash: PyHash,
    item: usize,
}

/// The open-addressing probe sequence of one hash (`i` and `perturb` of `set_add_entry`), yielded as groups of consecutive slot indexes.
struct ProbeGroups {
    mask: u64,
    index: u64,
    perturb: u64,
}

impl ProbeGroups {
    fn new(hash: PyHash, mask: u64) -> Self {
        let perturb = hash.as_unsigned();
        ProbeGroups {
            mask,
            index: perturb & mask,
            perturb,
        }
    }

    /// The slots of the next group: the probed slot plus `LINEAR_PROBES` followers when they all fit in the table, the probed slot alone otherwise.
    /// Then the sequence moves on (`perturb >>= 5; i = i * 5 + 1 + perturb`).
    fn next_group(&mut self) -> std::ops::RangeInclusive<usize> {
        let first = self.index;
        let last = if first + LINEAR_PROBES <= self.mask {
            first + LINEAR_PROBES
        } else {
            first
        };
        self.perturb >>= PERTURB_SHIFT;
        self.index = first
            .wrapping_mul(5)
            .wrapping_add(1)
            .wrapping_add(self.perturb)
            & self.mask;
        first as usize..=last as usize
    }
}

/// The hash table of a set that only ever had items added (no deleted-slot markers).
struct SlotTable {
    slots: Vec<Option<Slot>>,
    /// Occupied slots; equal to the number of distinct items because nothing is ever removed.
    used: u64,
}

impl SlotTable {
    fn empty() -> Self {
        SlotTable {
            slots: vec![None; MINIMUM_TABLE_SIZE],
            used: 0,
        }
    }

    fn mask(&self) -> u64 {
        self.slots.len() as u64 - 1
    }

    /// `set_add_entry` for `items[item]`: keeps the first of several equal items, otherwise fills the first free slot of the probe sequence and
    /// rebuilds the table larger when it got too full.
    fn add<T: PartialEq>(&mut self, items: &[(PyHash, T)], item: usize) {
        let hash = items[item].0;
        let mut groups = ProbeGroups::new(hash, self.mask());
        loop {
            for index in groups.next_group() {
                match self.slots[index] {
                    None => {
                        self.slots[index] = Some(Slot { hash, item });
                        self.used += 1;
                        self.grow_if_crowded();
                        return;
                    }
                    // Items are compared only when the stored hashes are equal, in the order (stored, new). A shared hash alone never merges items.
                    Some(stored)
                        if stored.hash == hash && items[stored.item].1 == items[item].1 =>
                    {
                        return
                    }
                    Some(_) => {}
                }
            }
        }
    }

    /// The `fill * 5 < mask * 3` test of `set_add_entry` and, when it fails, `set_table_resize`.
    fn grow_if_crowded(&mut self) {
        let (fill_factor, mask_factor) = GROWTH_FILL_FACTOR;
        if self.used * fill_factor < self.mask() * mask_factor {
            return;
        }
        let wanted = if self.used > QUADRUPLING_LIMIT {
            self.used * 2
        } else {
            self.used * 4
        };
        self.rebuild(wanted);
    }

    /// `set_table_resize`: a new table of the smallest power of two above `wanted` (at least the minimum size), refilled from the old slots in
    /// slot order with `set_insert_clean` (no equality tests; the first free slot of the probe sequence).
    fn rebuild(&mut self, wanted: u64) {
        let mut new_size = MINIMUM_TABLE_SIZE as u64;
        while new_size <= wanted {
            new_size <<= 1;
        }
        let old_slots = std::mem::replace(&mut self.slots, vec![None; new_size as usize]);
        for slot in old_slots.into_iter().flatten() {
            let mut groups = ProbeGroups::new(slot.hash, new_size - 1);
            'placed: loop {
                for index in groups.next_group() {
                    if self.slots[index].is_none() {
                        self.slots[index] = Some(slot);
                        break 'placed;
                    }
                }
            }
        }
    }

    /// The items in the order `set_next` visits them: slot 0 up to the last slot.
    fn item_order(&self) -> Vec<usize> {
        self.slots.iter().flatten().map(|slot| slot.item).collect()
    }
}

/// The input positions `frozenset(generator)` keeps, in the order iterating the frozenset yields them.
///
/// `items` are the generator's values in production order, each with its `hash()`. A value is kept only when no earlier kept value has an equal
/// hash and compares equal to it (`PartialEq`, called as `earlier == later`); the first of several equal values is the one retained, and values that
/// merely share a hash stay distinct. The returned indexes point into `items`, so the caller keeps its own spelling of each retained value and
/// needs no sort.
pub fn frozenset_iteration_order<T: PartialEq>(items: &[(PyHash, T)]) -> Vec<usize> {
    let mut table = SlotTable::empty();
    for item in 0..items.len() {
        table.add(items, item);
    }
    table.item_order()
}
