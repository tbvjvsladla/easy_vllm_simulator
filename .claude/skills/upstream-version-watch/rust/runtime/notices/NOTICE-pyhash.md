# Attribution and change summary of `src/pyhash.rs` (R35 generic str/tuple hash and frozenset slot order)

`src/pyhash.rs` is a Rust re-expression of parts of **CPython 3.12.3** (tag `v3.12.3`, https://github.com/python/cpython), read from these files:

| CPython 3.12.3 file | What `pyhash.rs` takes from it |
|---|---|
| `Python/pyhash.c` | `siphash13`, `HALF_ROUND`/`SINGLE_ROUND` (the SipHash-1-3 rounds, the initial state constants, the modified final xor), `_Py_HashBytes` (empty input = 0, `-1` becomes `-2`), `Py_HASH_CUTOFF == 0` profile, `pysiphash` key use |
| `Python/bootstrap_hash.c` | `lcg_urandom` and the seed handling of `_Py_HashRandomization_Init` (seed 0 = zero secret, other seeds = LCG bytes) |
| `Include/pyhash.h` | the 24-byte `_Py_HashSecret_t` layout (`siphash.k0`/`k1` = bytes 0..16) |
| `Objects/unicodeobject.c` | the rule of `unicode_hash` that the hashed buffer is the PEP 393 data times the kind |
| `Objects/tupleobject.c` | `tuplehash` (xxHash-style accumulator, the constants, the length mix, the `1546275796` sentinel) |
| `Objects/setobject.c` | `set_add_entry`, `set_insert_clean`, `set_table_resize`, `set_next`, `set_update_internal` / `make_new_set` for an iterable (slot order, 10-slot linear probing, perturbation, growth, clean rehash) |

This is translated, derived code: the arithmetic, constants and control flow follow the C sources; no C text is copied verbatim.

## Licenses that apply (full texts shipped next to this file)

1. **PSF License Version 2 of CPython**: `CPython-3.12.3-LICENSE.txt` is the byte-exact `LICENSE` file of the v3.12.3 tag. Its PSF License Agreement
   requires that the PSF License Agreement and the PSF copyright notice ("Copyright (c) 2001 ... 2023 Python Software Foundation; All Rights Reserved") are
   retained in a derivative version (clause 2) and that a derivative work made available to others includes a brief summary of the changes (clause 3);
   the summary is the section below. This package is distributed under its own terms for the Rust code; the CPython-derived parts are under the PSF
   License Version 2 as stated in that file.
2. **MIT License of SipHash (CPython `Python/pyhash.c`)**: `SipHash-MIT-from-CPython-pyhash.c-lines-295-330.txt` is the comment block of lines 295-330 of
   `Python/pyhash.c` copied unchanged: *Copyright (c) 2013 Marek Majkowski <marek@popcount.org>*, the MIT permission notice, the original location
   (https://github.com/majek/csiphash/), the credit to Samuel Neves, djb and Jean-Philippe Aumasson, and CPython's "Modified for Python by Christian
   Heimes" lines. `half_round`, `sip_round` and `siphash13` re-express that SipHash code, so the copyright and permission notice must stay with this
   module in every copy or substantial portion.

Authors named by the shipped texts: the Python Software Foundation (CPython), Marek Majkowski (csiphash, MIT), Samuel Neves, djb and Jean-Philippe
Aumasson (credited by the csiphash notice), Christian Heimes (CPython modification credit).

## Summary of changes made to the CPython code (PSF License clause 3)

* Language and structure: C macros and functions became Rust functions with named constants (`half_round`, `sip_round`, `siphash13`, `hash_str`,
  `hash_tuple`, `SlotTable`, `ProbeGroups`); explicit `wrapping_*`/`rotate_left` 64-bit arithmetic replaces C unsigned overflow.
* Only the SipHash-1-3, 64-bit, little-endian, `cutoff == 0` profile is kept; the FNV, SipHash-2-4, 32-bit, big-endian and small-string (DJBX33A) paths
  are omitted.
* The hash secret is a plain value (`HashSecret`) built by a pure function from an explicit seed; there is no global `_Py_HashSecret`, no environment or
  entropy reading.
* Strings are `PyStr` code-point vectors encoded to the PEP 393 1/2/4-byte little-endian buffer before hashing.
* The set table keeps input indexes instead of object pointers, supports additions only (no deletion, dummies or merge fast paths), and compares items with
  `PartialEq` only when the stored hashes are equal.
* `frozenset_iteration_order` is new API that returns the retained input positions in slot order; CPython has no such function.

## Not covered / reviewer decision

* `Objects/unicodeobject.c` carries a file-header permission notice for the original string type (Secret Labs AB, Fredrik Lundh; Corporation for National
  Research Initiatives). Only the one-line buffer rule of `unicode_hash` was consulted; no string implementation code is adapted, and that notice is **not**
  shipped here. If a reviewer judges that rule to be derived from that file's string implementation, that notice is a further required addition.
* `Objects/tupleobject.c` says its hash is a simplified xxHash and uses the specification's constants; only that statement and those constants are used.
  No xxHash source or license text is shipped.
* The existing `notices/NOTICE.md` (packaging 24.0) and the notices of `agent-runtime-support` are unchanged and do not cover this module.
