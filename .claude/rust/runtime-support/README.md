# agent-runtime-support

Unpublished library (`publish = false`, no `license` field on purpose, like the owner packages) with the Python-compatible
typed data and text semantics that several owner Rust packages share: `pystr` (code-point text with lone surrogates), `pyvalue`
(the JSON/YAML value type, key kinds, NaN identity, `repr`, JSON writers), `pyjson` (the JSON reader and its errors),
`pyyaml` (the PyYAML 6.0.3 `safe_load` port over bytes the caller read), `pynumber`/`pyops`/`pydyn`/`pysort`/`pyformat`
(`int`/`float` grammars, operators, dynamic operations, ordering, `%`-formatting), `pylower` (`str.lower()`), the Unicode 15
tables they need, and `failure::Failure` (class name plus message).

It contains no skill, node, topology, serving or orchestration rule, starts no process, opens no connection and reads no file or
credential (the YAML reader takes the bytes its caller read). Owner packages depend on it by path
(`../../../rust/runtime-support` from `.claude/skills/<skill>/rust`) and re-export the modules under their old paths, so
a type keeps one nominal identity. Only `num-bigint` and `num-traits` (the versions the owner packages already lock) are
dependencies. Third-party notices (PSF, PyYAML MIT, Unicode License V3) are in `notices/`.

Checks: `cargo test --release --lib` (47 unit tests, moved with their modules); the behavioural checks of every consumer
run in the consumer's own `checks/`.

`pyjson::loads_text_unique` is `json.loads(text, object_pairs_hook=hook)` for a hook that refuses an object holding a key twice: the
same reader as `loads_text`, reporting the first repeated key of an object when that object has been read to its closing brace
(innermost first, empty objects included). It exists for the recipe owner's MXFP4 header validation; the plain readers are unchanged.

Two tiny public helpers (`PyStr::is_blank`, `PyDict::set`) carry an ordinary `#[inline]` so consumer crates can compile them into
their own loops; this is a codegen boundary only (R22b), not a semantic change.

Two further generic helpers exist for the recipe owner's model scripts (R24 fix round; no owner or network rule in them):
`pynumber::int_true_divide` is Python's `int / int` (the exact quotient rounded once, half to even, subnormals at their own precision,
`OverflowError` beyond the float range, `ZeroDivisionError`, `0 / -n` is `-0.0`), and `pydyn::dict_key` is the `dict` key a hashable value stands
for (`1`, `1.0` and `True` one key, integral floats by exact value, a NaN equal only to the same object). Each has a unit test with
values taken from CPython.

`pycall` is the binding of a Python call to a function's parameters as plain data (R25): ordinary positional-or-keyword parameters, trailing
defaults, an optional `**rest`, supplied values borrowed from the caller, CPython 3.12's `TypeError`s in CPython's order (an unexpected or
repeated keyword, then too many positionals, then missing ones). It is generic: it knows no function of any owner and does no file, network or
clock work. The older owner binders (numeric `Arguments`, model `Call`) are unchanged; the generation owner uses this one for signatures of up to
eleven parameters with five defaults and for `recipe_from_candidate(candidate, **extra)`. Checks: `cargo test --release --test pycall_cases`
(binding messages taken from CPython 3.12.3 with one stdlib-only probe) next to the 47 library unit tests.
