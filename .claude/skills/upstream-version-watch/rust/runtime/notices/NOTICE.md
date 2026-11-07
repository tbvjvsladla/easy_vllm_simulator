# Third-party attribution of the `torch_pin` / `pep440` modules (R35 stage A)

`src/pep440.rs` is an adaptation to Rust of the version parsing and ordering of **packaging 24.0** (`packaging/version.py`: `VERSION_PATTERN`,
`Version.__init__`, `_parse_letter_version`, `_parse_local_version`, `_cmpkey`), read from the installed copy
`/usr/lib/python3/dist-packages/packaging/version.py` of the frozen interpreter profile (CPython 3.12.3, Debian `python3-packaging` 24.0).
The regular expression is not translated to a regular-expression library: its matching (alternatives in the pattern's order, greedy choices,
`re.IGNORECASE` for a `str` pattern, Unicode `\s`) is written out by hand in the same module.

* Upstream: https://github.com/pypa/packaging . The file header of `version.py` states: "This file is dual licensed under the terms of the Apache License,
  Version 2.0, and the BSD License." The local Debian copyright file (`/usr/share/doc/python3-packaging/copyright`, copied here as
  `packaging-24.0-copyright.txt`) lists Copyright (C) 2014 Donald Stufft and Copyright (C) 2012-2013 Vinay Sajip under the Apache License 2.0; this port is
  distributed under the Apache License 2.0 option, whose full text (`/usr/share/common-licenses/Apache-2.0`) is `Apache-2.0.txt`. The text of the
  BSD alternative is not part of the local files and is not reproduced here.
* The adaptation changes: Rust types for the parsed version (arbitrary-precision integers from `num-bigint`), a hand-written matcher for the
  pattern, the decimal-digit limit of the frozen profile (4300) in the integer conversions, and the sort key as a comparison function.
  No code of the Python package is shipped.
* The Unicode classes (`str.isdigit` etc.) are the ones already shipped with this crate (see the common package notices).
