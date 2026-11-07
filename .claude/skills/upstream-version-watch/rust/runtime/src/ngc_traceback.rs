//! The uncaught-exception frames of `resolve_ngc_tag.py` (CPython 3.12.3 layout, source of 139 lines).
//!
//! Provenance of each site: the frames reached through `inspect_env` line 56 (`img.get`), `gen_candidates` line 68 (the unpack) and the script's
//! `main()` line were read from the saved raw tracebacks of the original capture (`cli-013`, `cli-014`, `cli-020`). Every other site is DERIVED by
//! CPython's rule for the instruction that raises (an attribute load covers `object.attribute`, a call covers the whole call, an unpack covers its
//! targets, `in` covers the comparison, a `for` over a name shows no carets) and was not captured; the spans are found in the line text itself,
//! never typed as numbers, so they cannot drift from the text. No `AttributeError` suggestion line is reproduced.

use crate::torch_traceback::{Site, SiteFile};

fn site(line: u32, function: &'static str, text: &'static str, needle: Option<&str>) -> Site {
    let columns = needle.map(|needle| {
        let start = text.find(needle).expect("needle is in the line");
        (start as u32, (start + needle.len()) as u32)
    });
    Site {
        file: SiteFile::Script,
        line,
        function,
        text,
        columns,
    }
}

/// Where `main` calls the function that raised.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Entry {
    /// `target = norm(a.torch_pin)` (line 92).
    Target,
    /// `cands = gen_candidates(a.start, a.months)` (line 96).
    Candidates,
    /// `env = inspect_env(tag, a.arch)` (line 106).
    Probe,
    /// `bp = norm(bv) if bv else None` (line 111).
    Prefix,
}

/// The `print` of the match report (line 114): the call spans several lines, so the span is the whole first line and no caret line is shown.
pub fn print_match() -> Vec<Site> {
    let text = "            print(json.dumps({";
    vec![
        site(114, "main", text, Some(text.trim_start())),
        module_entry(),
    ]
}

fn module_entry() -> Site {
    site(139, "<module>", "    main()", None)
}

fn main_site(entry: Entry) -> Site {
    match entry {
        Entry::Target => site(
            92,
            "main",
            "    target = norm(a.torch_pin)",
            Some("norm(a.torch_pin)"),
        ),
        Entry::Candidates => site(
            96,
            "main",
            "        cands = gen_candidates(a.start, a.months)",
            Some("gen_candidates(a.start, a.months)"),
        ),
        Entry::Probe => site(
            106,
            "main",
            "        env = inspect_env(tag, a.arch)",
            Some("inspect_env(tag, a.arch)"),
        ),
        Entry::Prefix => site(
            111,
            "main",
            "        bp = norm(bv) if bv else None",
            Some("norm(bv)"),
        ),
    }
}

/// The frames of an exception raised at `at` under the call of `entry`, innermost first.
pub fn frames(entry: Entry, at: crate::ngc_tag::Where) -> Vec<Site> {
    use crate::ngc_tag::Where::*;
    let unpack = "    yy, mm = (int(x) for x in start.split(\".\"))";
    let mut stack = match at {
        ReleaseSegments => {
            let tuple_line = "    return tuple(int(part) for part in match.group(1).split(\".\")) if match else None";
            vec![
                site(30, "<genexpr>", tuple_line, Some("int(part)")),
                site(
                    30,
                    "release_tuple",
                    tuple_line,
                    Some("tuple(int(part) for part in match.group(1).split(\".\"))"),
                ),
                site(
                    23,
                    "norm",
                    "    release = release_tuple(v)",
                    Some("release_tuple(v)"),
                ),
            ]
        }
        GenSplit => vec![site(68, "gen_candidates", unpack, Some("start.split"))],
        GenInt => vec![
            site(68, "<genexpr>", unpack, Some("int(x)")),
            site(68, "gen_candidates", unpack, Some("yy, mm")),
        ],
        GenUnpack => vec![site(68, "gen_candidates", unpack, Some("yy, mm"))],
        GenRange => vec![site(
            70,
            "gen_candidates",
            "    for _ in range(months):",
            Some("range(months)"),
        )],
        ImgGet => vec![site(
            56,
            "inspect_env",
            "    cfg = img.get(f\"linux/{arch}\") or img  # multi-arch dict 또는 단일",
            Some("img.get"),
        )],
        CfgGet | ConfigGet => {
            let text = "    env = ((cfg or {}).get(\"config\", {}) or {}).get(\"Env\", []) or []";
            let needle = if at == CfgGet {
                "(cfg or {}).get"
            } else {
                "((cfg or {}).get(\"config\", {}) or {}).get"
            };
            vec![site(57, "inspect_env", text, Some(needle))]
        }
        EnvLoop => vec![site(59, "inspect_env", "    for e in env:", None)],
        EnvIn => vec![site(
            60,
            "inspect_env",
            "        if \"=\" in e:",
            Some("\"=\" in e"),
        )],
        EnvSplit => vec![site(
            61,
            "inspect_env",
            "            k, v = e.split(\"=\", 1)",
            Some("e.split"),
        )],
        Elsewhere => Vec::new(),
    };
    if at != Elsewhere {
        stack.push(main_site(entry));
        stack.push(module_entry());
    }
    stack
}
