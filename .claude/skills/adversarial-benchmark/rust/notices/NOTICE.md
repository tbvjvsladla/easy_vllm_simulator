# Third-party notices for the `sweep-stop` prototype

This directory is the owner-local place for notices of material that parts of this crate were derived from. It
assigns no license to the project's original code (`publish = false`; the Cargo `license` field is left unset on
purpose). Full license texts, downloaded 2026-11-04 from the primary URLs below, sit next to this file.

| Derived file | Derived from (exact version, primary source) | License | Changes |
|---|---|---|---|
| `src/utc.rs` | CPython **v3.12.3** `Modules/_datetimemodule.c` — `_find_isoformat_datetime_separator`, `parse_isoformat_date`, `parse_isoformat_time`, `parse_hh_mm_ss_ff`, `iso_to_ymd`, `_sanitize_isoformat_str`, `tzinfo_from_isoformat_results`, `datetime_fromisoformat` — <https://github.com/python/cpython/blob/v3.12.3/Modules/_datetimemodule.c>; and `Lib/_pydatetime.py` — the ordinal/calendar helpers (`_days_before_year`, `_ymd2ord`, `_ord2ymd`) — <https://github.com/python/cpython/blob/v3.12.3/Lib/_pydatetime.py> | PSF License (`CPython-3.12.3-LICENSE.txt`) | Translated from C/Python to Rust; C strings modelled as bytes plus an implicit NUL; only the `fromisoformat` → `astimezone(utc)` path is kept; errors are returned as the exception text. |
| `src/pyjson.rs` | CPython **v3.12.3** `Modules/_json.c` (`scanstring_unicode`, `_match_number_unicode`, `_parse_object_unicode`, `_parse_array_unicode`, `scan_once_unicode`) — <https://github.com/python/cpython/blob/v3.12.3/Modules/_json.c>; `Lib/json/decoder.py` and `Lib/json/__init__.py` (error positions, BOM check) — <https://github.com/python/cpython/blob/v3.12.3/Lib/json/decoder.py> | PSF License | Translated to Rust over code points; produces this crate's value type; recursion limit set to the value observed on the original CLI. |
| `src/argv.rs` | CPython **v3.12.3** `Lib/argparse.py` (`_parse_known_args`, `consume_optional`, `_parse_optional`, `_get_option_tuples`, `_match_argument`, `_get_nargs_pattern`, error messages) — <https://github.com/python/cpython/blob/v3.12.3/Lib/argparse.py> | PSF License | Specialised to parsers made of `-h`, value options and flags (no positionals, no subparsers); a `Spec` table per original program (`sweep_stop.py`, `parse_bench.py`); usage and help text come from `src/help_layouts.rs` / `src/help_layouts_parse_bench.rs` by terminal width. |
| `src/help_layouts.rs` | The *output* of CPython **v3.12.3** `Lib/argparse.py` (`HelpFormatter`, `format_usage`, `format_help`) and `Lib/textwrap.py` for the original `sweep_stop.py` parser at terminal widths 1..200 — <https://github.com/python/cpython/blob/v3.12.3/Lib/argparse.py>, <https://github.com/python/cpython/blob/v3.12.3/Lib/textwrap.py> | PSF License | Generated text captured by running the original program (the dev-time generator `gen_help_layouts.py`); no argparse or textwrap source is copied. `src/terminal.rs` re-implements the documented behaviour of `Lib/shutil.py` `get_terminal_size` (<https://github.com/python/cpython/blob/v3.12.3/Lib/shutil.py>) and of `int(str)`. |
| `src/help_layouts_parse_bench.rs` | The *output* of CPython **v3.12.3** `Lib/argparse.py` and `Lib/textwrap.py` for the original `parse_bench.py` parser at terminal widths 1..200 (same sources as the row above) | PSF License | Generated text captured by running the original program (the dev-time `probe_width.py`, `gen_help_layouts.py`); no argparse or textwrap source is copied. |
| `src/unicode_tables.rs` | Unicode Character Database **15.0.0** (<https://www.unicode.org/Public/15.0.0/ucd/>), as exposed by CPython 3.12.3 `str.isprintable` / `isspace` / `isdecimal` | Unicode License V3 (`Unicode-License-V3.txt`, <https://www.unicode.org/license.txt>) | Only the code point ranges of three character properties were extracted by the dev-time generator `gen_unicode_tables.py`. |

Cargo dependencies (`num-bigint`, `num-traits`, `rustix`, and build-only `serde_json`) are used as libraries under
their own licenses (see the downloaded Cargo package metadata); none of their source is vendored here.
`Cargo.lock` pins the resolved versions.

Behaviour documented elsewhere in the crate (Python `repr` of floats and strings, `json.dumps` layout, `bool`/`int`
semantics) was re-implemented from observed behaviour and tests, not copied from source.
The same holds for `src/pynumber.rs` (`float()`/`int()` string grammars after CPython's
`_PyUnicode_TransformDecimalAndSpaceToASCII`, `_Py_string_to_number_with_underscores`, `PyLong_FromString` and
`float.__round__`) and `src/stdout.rs` (when `TextIOWrapper`/`BufferedWriter` hand bytes to the operating system) and `src/engine_log.rs` (the `re.IGNORECASE` Unicode equivalences `i`/`ı`, `s`/`ſ`, `İ`, `K` of
`Lib/re/_compiler.py`): the rules were taken from the behaviour of CPython 3.12.3 and verified against values it
produced; no CPython source is reproduced in them.

## Retained notices

* Python Software Foundation: the PSF License Agreement requires that "PSF's License Agreement and PSF's notice
  of copyright, i.e., "Copyright (c) 2001, 2002, 2003, 2004, 2005, 2006, 2007, 2008, 2009, 2010, 2011, 2012, 2013,
  2014, 2015, 2016, 2017, 2018, 2019, 2020, 2021, 2022, 2023 Python Software Foundation; All Rights Reserved" are
  retained in … any derivative version". The agreement and that notice are in `CPython-3.12.3-LICENSE.txt`
  (from <https://raw.githubusercontent.com/python/cpython/v3.12.3/LICENSE>). The derived files above are, in the sense
  of that agreement, a derivative work based on Python; this document is the brief summary of changes it asks for.
* Unicode, Inc.: "Copyright © 1991-2026 Unicode, Inc." with the full permission notice in `Unicode-License-V3.txt`
  (from <https://www.unicode.org/license.txt>); the notice must accompany copies of the data-derived table.

## Distribution

Copies of `src/utc.rs`, `src/pyjson.rs`, `src/argv.rs`, `src/help_layouts.rs`, `src/help_layouts_parse_bench.rs` or `src/unicode_tables.rs` must travel
with this directory (or an equivalent notice file in the destination skill folder). Generated checks data under
`checks/data/` contains output captured from CPython 3.12.3 and is test data, not source code.
