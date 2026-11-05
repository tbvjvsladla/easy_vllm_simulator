//! CPython 3.12's `random.Random` seeded with a `str`, as far as `randrange(n)` needs it: the Mersenne Twister
//! (MT19937), the version-2 string seeding (`int.from_bytes(text + sha512(text).digest())` fed to `init_by_array`
//! as little-endian 32-bit words) and the rejection sampling of `Random._randbelow_with_getrandbits`.
//!
//! The generator is the published algorithm (Matsumoto and Nishimura, 1998); nothing here is tied to a seed, a
//! pool size or a result.

use crate::pyfs::strict_utf8;
use crate::pystr::{EncodeFailure, PyStr};
use ring::digest::{digest, SHA512};

const STATE_WORDS: usize = 624;
const SHIFT_WORDS: usize = 397;
const UPPER_MASK: u32 = 0x8000_0000;
const LOWER_MASK: u32 = 0x7fff_ffff;
const MATRIX_A: u32 = 0x9908_b0df;

/// A seeded MT19937 generator.
pub struct PythonRandom {
    state: [u32; STATE_WORDS],
    index: usize,
}

impl PythonRandom {
    /// `random.Random(seed)` for a `str` seed; a lone surrogate cannot be encoded (`UnicodeEncodeError`).
    pub fn from_text(seed: &PyStr) -> Result<Self, EncodeFailure> {
        let mut bytes = strict_utf8(seed)?;
        let hash = digest(&SHA512, &bytes);
        bytes.extend_from_slice(hash.as_ref());
        // The integer is big-endian; its 32-bit words are taken from the least significant end.
        let significant: &[u8] = {
            let first = bytes
                .iter()
                .position(|&byte| byte != 0)
                .unwrap_or(bytes.len());
            &bytes[first..]
        };
        let mut key: Vec<u32> = significant
            .rchunks(4)
            .map(|chunk| {
                chunk
                    .iter()
                    .fold(0_u32, |word, &byte| (word << 8) | u32::from(byte))
            })
            .collect();
        if key.is_empty() {
            key.push(0);
        }
        Ok(Self::from_key(&key))
    }

    fn from_key(key: &[u32]) -> Self {
        let mut generator = PythonRandom {
            state: [0; STATE_WORDS],
            index: STATE_WORDS,
        };
        generator.init_genrand(19_650_218);
        let mut i = 1_usize;
        let mut j = 0_usize;
        for _ in 0..STATE_WORDS.max(key.len()) {
            let previous = generator.state[i - 1];
            generator.state[i] = (generator.state[i]
                ^ (previous ^ (previous >> 30)).wrapping_mul(1_664_525))
            .wrapping_add(key[j])
            .wrapping_add(j as u32);
            i += 1;
            j += 1;
            if i >= STATE_WORDS {
                generator.state[0] = generator.state[STATE_WORDS - 1];
                i = 1;
            }
            if j >= key.len() {
                j = 0;
            }
        }
        for _ in 0..STATE_WORDS - 1 {
            let previous = generator.state[i - 1];
            generator.state[i] = (generator.state[i]
                ^ (previous ^ (previous >> 30)).wrapping_mul(1_566_083_941))
            .wrapping_sub(i as u32);
            i += 1;
            if i >= STATE_WORDS {
                generator.state[0] = generator.state[STATE_WORDS - 1];
                i = 1;
            }
        }
        generator.state[0] = UPPER_MASK;
        generator.index = STATE_WORDS;
        generator
    }

    fn init_genrand(&mut self, seed: u32) {
        self.state[0] = seed;
        for i in 1..STATE_WORDS {
            let previous = self.state[i - 1];
            self.state[i] = 1_812_433_253_u32
                .wrapping_mul(previous ^ (previous >> 30))
                .wrapping_add(i as u32);
        }
        self.index = STATE_WORDS;
    }

    fn twist(&mut self) {
        let mix = |upper: u32, lower: u32| {
            let y = (upper & UPPER_MASK) | (lower & LOWER_MASK);
            (y >> 1) ^ if y & 1 == 1 { MATRIX_A } else { 0 }
        };
        for k in 0..STATE_WORDS - SHIFT_WORDS {
            self.state[k] = self.state[k + SHIFT_WORDS] ^ mix(self.state[k], self.state[k + 1]);
        }
        for k in STATE_WORDS - SHIFT_WORDS..STATE_WORDS - 1 {
            self.state[k] =
                self.state[k + SHIFT_WORDS - STATE_WORDS] ^ mix(self.state[k], self.state[k + 1]);
        }
        self.state[STATE_WORDS - 1] =
            self.state[SHIFT_WORDS - 1] ^ mix(self.state[STATE_WORDS - 1], self.state[0]);
        self.index = 0;
    }

    /// `genrand_uint32`.
    fn next_word(&mut self) -> u32 {
        if self.index >= STATE_WORDS {
            self.twist();
        }
        let mut y = self.state[self.index];
        self.index += 1;
        y ^= y >> 11;
        y ^= (y << 7) & 0x9d2c_5680;
        y ^= (y << 15) & 0xefc6_0000;
        y ^ (y >> 18)
    }

    /// `getrandbits(k)` for `1 <= k <= 64`: whole words fill the result from the low end; a last partial word
    /// keeps its top bits.
    fn random_bits(&mut self, bits: u32) -> u64 {
        debug_assert!((1..=64).contains(&bits));
        let mut remaining = bits;
        let mut value = 0_u64;
        let mut shift = 0;
        while remaining > 0 {
            let mut word = self.next_word();
            if remaining < 32 {
                word >>= 32 - remaining;
            }
            value |= u64::from(word) << shift;
            shift += 32;
            remaining = remaining.saturating_sub(32);
        }
        value
    }

    /// `randrange(n)` for `n >= 1`: `_randbelow_with_getrandbits`.
    pub fn randbelow(&mut self, n: u64) -> u64 {
        assert!(n >= 1, "randbelow needs a positive bound");
        let bits = 64 - n.leading_zeros();
        loop {
            let candidate = self.random_bits(bits);
            if candidate < n {
                return candidate;
            }
        }
    }
}
