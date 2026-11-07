# Third-party notices for `agent-runtime-support`

The common support package holds Python-compatible value, text, number, JSON and YAML semantics that were derived from, or
verified against, the material below. It assigns no license to the project's original code (`publish = false`; the Cargo
`license` field is left unset on purpose). Full license texts sit next to this file. The files moved here unchanged from the
benchmark owner package `sweep-stop` (`.claude/skills/adversarial-benchmark/rust`), which re-exports them.

| Derived file | Derived from (exact version, primary source) | License | Changes |
|---|---|---|---|
| `src/pyjson.rs` | CPython **v3.12.3** `Modules/_json.c` (`scanstring_unicode`, `_match_number_unicode`, `_parse_object_unicode`, `_parse_array_unicode`, `scan_once_unicode`) — <https://github.com/python/cpython/blob/v3.12.3/Modules/_json.c>; `Lib/json/decoder.py` and `Lib/json/__init__.py` (error positions, BOM check) — <https://github.com/python/cpython/blob/v3.12.3/Lib/json/decoder.py> | PSF License | Translated to Rust over code points; produces this crate's value type; recursion limit set to the value observed on the original CLI. |
| `src/pyyaml/dump.rs` | **PyYAML 6.0.3** `yaml/representer.py` (`SafeRepresenter` scalars, lists, dicts), `yaml/serializer.py`, `yaml/emitter.py` (`Emitter`: scalar analysis, plain/single/double-quoted writers, block collections), `yaml/resolver.py` (implicit resolvers, shared with the loader), `yaml/nodes.py`, `yaml/events.py` — <https://github.com/yaml/pyyaml/tree/6.0.3/lib/yaml> (the pure-Python `SafeDumper` that `yaml.safe_dump` uses) | MIT (`PyYAML-6.0.3-LICENSE.txt`; Copyright (c) 2017-2021 Ingy döt Net, (c) 2006-2016 Kirill Simonov) | Translated from Python to Rust function by function for JSON-valued data with `allow_unicode=True`, `sort_keys=False`, block style, indent 2, width 80; no external YAML library and no C code is used. |
| `src/pyyaml/{mod,reader,scanner,parser,compose,frames}.rs` | **PyYAML 6.0.3** `yaml/reader.py`, `yaml/scanner.py`, `yaml/parser.py`, `yaml/composer.py`, `yaml/resolver.py` (implicit resolvers), `yaml/constructor.py` (`SafeConstructor`), `yaml/error.py`, `yaml/tokens.py`, `yaml/events.py`, `yaml/nodes.py` — <https://github.com/yaml/pyyaml/tree/6.0.3/lib/yaml> (the pure-Python `SafeLoader` that `yaml.safe_load` uses) | MIT (`PyYAML-6.0.3-LICENSE.txt`; Copyright (c) 2017-2021 Ingy döt Net, (c) 2006-2016 Kirill Simonov) | Translated from Python to Rust function by function; the stream is a text file read in the same 8192-byte / 4096-character chunks; the constructor keeps the original's breadth-first filling of containers; the result is this crate's `PyValue` (a node that contains itself becomes a `[...]` marker); `frames.rs` counts the Python call stack of those functions to reproduce `RecursionError` (its names are the original's function names); no external YAML library and no C code is used. |
| `src/pyyaml/compose.rs` — `decode_base64` | CPython **v3.12.3** `Modules/binascii.c` — `binascii_a2b_base64_impl`, non-strict mode (what `base64.decodebytes` calls) — <https://github.com/python/cpython/blob/v3.12.3/Modules/binascii.c> | PSF License (`CPython-3.12.3-LICENSE.txt`) | The padding/skip/quad state machine and the two error messages translated from C to Rust; checked against the real `binascii.a2b_base64` on 1 484 documents (`yaml_reader_cases2.jsonl`). The ASCII-encode error text of `!!binary` follows `UnicodeEncodeError.__str__` as observed. |
| `src/unicode_tables.rs` | Unicode Character Database **15.0.0** (<https://www.unicode.org/Public/15.0.0/ucd/>), as exposed by CPython 3.12.3 `str.isprintable` / `isspace` / `isdecimal` | Unicode License V3 (`Unicode-License-V3.txt`, <https://www.unicode.org/license.txt>) | Only the code point ranges of three character properties were extracted by the dev-time generator `gen_unicode_tables.py`. |
| `src/unicode_alnum_table.rs` | Unicode Character Database **15.0.0**, as exposed by CPython 3.12.3 `str.isalnum` (Python `re` `\w` = `isalnum() or '_'`) | Unicode License V3 (`Unicode-License-V3.txt`, <https://www.unicode.org/license.txt>) | Only the code point ranges of that one character property were extracted, by the dev-time generator `gen_unicode_alnum.py` (not shipped). |
| `src/unicode_lower_table.rs` | Unicode Character Database **15.0.0**, as exposed by CPython 3.12.3 `str.lower` (full lower-case mappings, and the Cased / Case_Ignorable properties CPython's Final_Sigma rule reads) | Unicode License V3 (`Unicode-License-V3.txt`, <https://www.unicode.org/license.txt>) | Only the lower-case mappings and the two context property ranges were extracted, by the dev-time generator `gen_unicode_lower.py` (not shipped). |

`src/pynumber.rs`, `src/pyops.rs`, `src/pyvalue.rs`, `src/pystr.rs`, `src/pydyn.rs`, `src/pyformat.rs`, `src/pysort.rs` and `src/pylower.rs`
re-implement observed behaviour of CPython 3.12.3 (`float()`/`int()` grammars, comparison and operator messages, `repr`,
`str` predicates, ordering, `str.lower()` with the Unicode 15.0.0 tables above) and were verified against values it produced;
no CPython source is reproduced in them. `src/pysort.rs` additionally follows the algorithm of `list.sort` (timsort) as documented.

Cargo dependencies (`num-bigint`, `num-traits` and their dependencies) are used as libraries under their own licenses; none
of their source is vendored. `Cargo.lock` pins the resolved versions (the same versions the benchmark owner package locks).

## Retained notices

* Python Software Foundation: `CPython-3.12.3-LICENSE.txt` carries the PSF License Agreement and the copyright notice it
  requires to stay with any derivative version; the derived files above are such a work, and this document is the brief
  summary of changes it asks for.
* PyYAML: `PyYAML-6.0.3-LICENSE.txt` carries the MIT License text and the copyright notices (Copyright (c) 2017-2021 Ingy döt Net,
  (c) 2006-2016 Kirill Simonov) that must accompany the derived `src/pyyaml/`.
* Unicode, Inc.: `Unicode-License-V3.txt` carries the permission notice that must accompany the data-derived tables.

## Distribution

Copies of `src/pyyaml/`, `src/pysort.rs`, `src/pyjson.rs` or any `src/unicode_*.rs` must travel with this directory (or an
equivalent notice file in the destination).
