# Third-party attribution of the `requirement` modules (R35 Requirement constructor projection)

This note covers `src/requirement/` and `tools/gen_unicode_names.rs`. The older notices of this package (`NOTICE.md`, `NOTICE-pyhash.md`, ...) keep covering
`pep440.rs`, `pyhash.rs` and the other modules; the `canonical_text` method added to `pep440.rs` is derived from the same `packaging` source and falls under
`NOTICE.md`.

## packaging 24.0 (`pypa/packaging`)

Adapted to Rust: `src/requirement/grammar.rs` (from `packaging/_tokenizer.py` and `packaging/_parser.py`: `DEFAULT_RULES`, `Tokenizer`, `parse_requirement` and the
marker grammar), `src/requirement/specifier.rs` (from `packaging/specifiers.py`: `Specifier._operator_regex_str`, `_version_regex_str` and `_regex` are
**copied verbatim** as pattern text; `Specifier.__init__`, `_canonical_spec`, `__eq__`, `__hash__`, `SpecifierSet.__init__`, `__str__`, `__iter__`; and from
`packaging/utils.py`: `canonicalize_version`), and `Requirement.__init__` of `packaging/requirements.py` (`src/requirement/projection.rs`). The pinned copies read
are `r35-requirement-reference/source/packaging/` (the files of the installed Debian `python3-packaging` 24.0).

The file headers of those sources state: "This file is dual licensed under the terms of the Apache License, Version 2.0, and the BSD License." The package
copyright file (`packaging-24.0-copyright.txt`) lists Copyright (C) 2014 Donald Stufft and Copyright (C) 2012-2013 Vinay Sajip. This port is distributed under
the Apache License 2.0 option (`Apache-2.0.txt`); the BSD alternative text is not part of the local files and is not reproduced. No code of the Python package
is shipped beyond the pattern texts named above.

Changes: Rust types; a backtracking matcher for the `re` subset the patterns use (`src/requirement/pyre.rs`, original work) instead of the `re` module; the
tokenizer without its stored next-token state (the parser reads each token right after checking it).

## CPython 3.12.3 (PSF License Version 2, `CPython-3.12.3-LICENSE.txt`)

Adapted to Rust: `src/requirement/string_literal.rs` (the escape decoding of `_PyUnicode_DecodeUnicodeEscapeInternal`, `Objects/unicodeobject.c`;
`decode_unicode_with_escapes`, `warn_invalid_escape_sequence`, `_PyPegen_parse_string`, `Parser/string_parser.c`; the string scan of the tokenizer),
`src/requirement/unicode_names.rs` (`_gethash`, `_getucname`, `_cmpname`, `_getcode`, `find_syllable`, `is_unified_ideograph`, `hangul_syllables`,
`Modules/unicodedata.c`) and `src/requirement/unicode_names_data.rs`, which `tools/gen_unicode_names.rs` generates from `Modules/unicodename_db.h`
(lexicon, phrasebook, code hash and alias tables, copied number for number). The pinned sources are under `r35-primary-sources/cpython-3.12.3/`
(SHA-256 of `unicodename_db.h`: f198a1766c238d97706217e48c5cced5198cca1da8edac962319924a4150ad23). The modules are derived works of CPython, distributed under
the PSF License Version 2 with the summary of changes above; Copyright (c) 2001-2024 Python Software Foundation; All Rights Reserved.

## Unicode Character Database 15.0.0

The character names in `unicode_names_data.rs` derive from the Unicode Character Database 15.0.0 (via CPython's `unicodename_db.h`; the name aliases of
`NameAliases.txt` are among them). The Unicode License V3 as shipped with the pinned data is `Unicode-License-V3-from-unicode-15.0.0.txt`; its copyright and
permission notice applies to that data and is preserved here.
