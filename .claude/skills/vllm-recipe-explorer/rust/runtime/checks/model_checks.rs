//! The registered checks of the two model scripts, one test target: `reference_cases` (the saved original results, observed whole),
//! `cases` (behaviour the saved results do not pin down, against Python's own `re`, `json`, `http.client`, `argparse` and number
//! formatting) and `bench_cases` (the frozen measurement program's preflight and report). `support` is what they share.
//! `extra_cases` holds the second round's boundary corrections (HTTPS trust, timeouts, key identity, `--`, private directories).

#[path = "model_support.rs"]
mod support;

#[path = "model_reference_cases.rs"]
mod reference_cases;

#[path = "model_cases.rs"]
mod cases;

#[path = "model_bench_cases.rs"]
mod bench_cases;

#[path = "model_extra_cases.rs"]
mod extra_cases;

#[path = "model_path_cases.rs"]
mod path_cases;
