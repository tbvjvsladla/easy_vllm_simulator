# Attribution and change summary of `src/tomllib/` (R35 default `tomllib.loads` component)

The modules of `src/tomllib/` are Rust re-expressions of **`tomllib` of CPython 3.12.3** (the frozen interpreter profile, Debian/Ubuntu package
`python3.12`; upstream https://github.com/python/cpython tag `v3.12.3`; the code is Taneli Hukkinen's `tomli`, "Licensed to PSF under a Contributor
Agreement"). The Python sources below were read from the installed copies under `/usr/lib/python3.12/tomllib/` while writing the port.

| Installed source (818 lines in 4 files) | What the port takes from it | Rust module |
|---|---|---|
| `tomllib/_parser.py` | `loads`, `Flags`, `NestedDict`, `skip_chars`, `skip_until`, `skip_comment`, `skip_comments_and_array_ws`, `create_dict_rule`, `create_list_rule`, `key_value_rule`, `parse_key_value_pair`, `parse_key`, `parse_key_part`, `parse_array`, `parse_inline_table`, `parse_basic_str_escape`, `parse_hex_char`, `parse_literal_str`, `parse_multiline_str`, `parse_basic_str`, `parse_value`, `suffixed_err`, `is_unicode_scalar_value`, the illegal-character sets and the escape table; the control flow, order of checks, messages and reported positions | `parser.rs`, `flags.rs`, `nested.rs`, `failure.rs`, `repr.rs` |
| `tomllib/_re.py` | `RE_NUMBER`, `RE_LOCALTIME`, `RE_DATETIME` (matched by hand), `match_to_datetime`, `match_to_localtime`, `match_to_number`, `cached_tz` (offset meaning) | `scalars.rs`, `value.rs` |
| `tomllib/__init__.py`, `tomllib/_types.py` | the exported names and the `TOMLDecodeError.__module__ = "tomllib"` rule; `Key`, `Pos` | `failure.rs`, `flags.rs` |

Every one of those files opens with the lines reproduced, together with the permission text of the license, in
`tomllib-MIT-Expat-from-debian-python3-pip-copyright.txt` (SPDX `MIT`, copyright 2021 Taneli Hukkinen). The Python Software Foundation license that
CPython is distributed under, and under which these files are licensed to the PSF by contributor agreement, is `CPython-3.12.3-LICENSE.txt` in this
directory. This is translated, derived code: the Python text is not shipped.

Two interpreter-level behaviours are not in `tomllib` and were not re-read from the C sources in this task; they are stated from CPython 3.12's
documented behaviour and checked where the saved originals contain them:

* the decimal-integer digit limit of `int(text, 0)` (`Objects/longobject.c`, the `ValueError` text and the count of digits without underscores or sign):
  the 4300/4301-digit message is confirmed by the saved original of the 4301-digit case; the exclusion of underscores from the count is NOT covered by a
  saved original;
* the `ValueError` texts of `datetime.date`/`datetime.datetime` (`day is out of range for month` is confirmed by a saved original; `year 0 is out of
  range` is NOT).

## Changes against CPython

* Rust types and explicit results replace exceptions; the exception **objects** CPython would raise are modelled as `Raised` (class, module, `str()`,
  the single argument, `__suppress_context__`, `__cause__`).
* Strings are code-point vectors, so positions and columns count characters, as Python's `str` does.
* The three regular expressions are matched by hand in the pattern's alternative and greedy order; no regular-expression library is used.
* Recursion is bounded by a modelled share of the interpreter's recursion limit (`RECURSION_FRAME_BUDGET`), and beyond it the component refuses by
  name instead of answering.
* `float` conversion uses Rust's correctly rounded `f64` parser after dropping underscores; `inf`/`nan` are built from explicit IEEE-754 bits so the NaN
  sign survives.
* Existing notices (packaging, CPython http/https, pyhash, Unicode names) are unchanged.
