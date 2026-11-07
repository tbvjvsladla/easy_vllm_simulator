//! Generated data - do not edit by hand.
//!
//! The source locations of the frames of an uncaught-exception traceback of `check_spec_layout.py` (CPython 3.12.3): line, source text and byte
//! columns of the instruction the interpreter reports, for the sites the command can raise through. Generator: gen_layout_sites.py (the saved
//! source is only compiled, never run; the reference interpreter's `json/__init__.py` and `codecs.py` are compiled/parsed the same way).

pub const OWNER_SOURCE_SHA256: &str =
    "c1eed9af09992c06b7cef8bf3a4923353cd6bb4ceaa6fb51e4e493e144aeda52";
pub const REFERENCE_INTERPRETER: &str = "CPython 3.12.3";
pub const STDLIB_PREFIX: &str = "/usr/lib/python3.12";
pub const STDLIB_SOURCE_SHA256: [(&str, &str); 2] = [
    (
        "json/__init__.py",
        "d5d41e2c29049515d295d81a6d40b4890fbec8d8482cfb401630f8ef2f77e4d5",
    ),
    (
        "codecs.py",
        "7b7839e53a77961153240aecfe11edc8054d05b1dedd83894450dae21ec05785",
    ),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SiteId {
    ModuleEntry,
    MainReadMethod,
    MainLoadIndex,
    MainReadNextn,
    MainNextnMinusOne,
    ConfigIteration,
    MethodGet,
    IndexJsonLoad,
    IndexGet,
    IndexKeys,
    NextnJsonLoad,
    NextnGet,
    JsonLoadRead,
    CodecsDecode,
}

#[derive(Clone, Copy, Debug)]
pub enum SiteFile {
    /// The script itself (its path is the caller's `argv[0]`).
    Owner,
    /// A file of the reference interpreter's standard library, relative to the stdlib prefix.
    Stdlib(&'static str),
    /// A frozen module: a frame without a source line.
    Frozen(&'static str),
}

#[derive(Clone, Copy, Debug)]
pub struct Site {
    pub id: SiteId,
    pub file: SiteFile,
    pub line: u32,
    pub function: &'static str,
    /// The original source line, with its indentation.
    pub text: Option<&'static str>,
    /// The instruction's span in the line, as UTF-8 byte offsets.
    pub columns: Option<(u32, u32)>,
    /// For a binary operation: where the operator starts and ends (byte offsets in the line).
    pub anchors: Option<(u32, u32)>,
}

pub const SITES: &[Site] = &[
    Site {
        id: SiteId::ModuleEntry,
        file: SiteFile::Owner,
        line: 196,
        function: "<module>",
        text: Some("    main()"),
        columns: Some((4, 10)),
        anchors: None,
    },
    Site {
        id: SiteId::MainReadMethod,
        file: SiteFile::Owner,
        line: 134,
        function: "main",
        text: Some(
            "    method = a.method or (read_method_from_config(a.config) if a.config else None)",
        ),
        columns: Some((26, 59)),
        anchors: None,
    },
    Site {
        id: SiteId::MainLoadIndex,
        file: SiteFile::Owner,
        line: 143,
        function: "main",
        text: Some("    keys, src = load_index_keys(a.model_dir)"),
        columns: Some((16, 44)),
        anchors: None,
    },
    Site {
        id: SiteId::MainReadNextn,
        file: SiteFile::Owner,
        line: 156,
        function: "main",
        text: Some("    nextn = read_num_nextn(a.model_dir)"),
        columns: Some((12, 39)),
        anchors: None,
    },
    Site {
        id: SiteId::MainNextnMinusOne,
        file: SiteFile::Owner,
        line: 163,
        function: "main",
        text: Some(
            "        visible = \"mtp.0\" if nextn in (1, None) else f\"mtp.0..{nextn - 1}\"",
        ),
        columns: Some((63, 72)),
        anchors: Some((69, 70)),
    },
    Site {
        id: SiteId::ConfigIteration,
        file: SiteFile::Owner,
        line: 74,
        function: "read_method_from_config",
        text: Some("            for line in f:"),
        columns: Some((12, 26)),
        anchors: None,
    },
    Site {
        id: SiteId::MethodGet,
        file: SiteFile::Owner,
        line: 80,
        function: "read_method_from_config",
        text: Some("                    return (json.loads(raw) or {}).get(\"method\")"),
        columns: Some((27, 54)),
        anchors: None,
    },
    Site {
        id: SiteId::IndexJsonLoad,
        file: SiteFile::Owner,
        line: 96,
        function: "load_index_keys",
        text: Some("                    wm = json.load(f).get(\"weight_map\") or {}"),
        columns: Some((25, 37)),
        anchors: None,
    },
    Site {
        id: SiteId::IndexGet,
        file: SiteFile::Owner,
        line: 96,
        function: "load_index_keys",
        text: Some("                    wm = json.load(f).get(\"weight_map\") or {}"),
        columns: Some((25, 41)),
        anchors: None,
    },
    Site {
        id: SiteId::IndexKeys,
        file: SiteFile::Owner,
        line: 99,
        function: "load_index_keys",
        text: Some("            return list(wm.keys()), name"),
        columns: Some((24, 31)),
        anchors: None,
    },
    Site {
        id: SiteId::NextnJsonLoad,
        file: SiteFile::Owner,
        line: 122,
        function: "read_num_nextn",
        text: Some("            return json.load(f).get(\"num_nextn_predict_layers\")"),
        columns: Some((19, 31)),
        anchors: None,
    },
    Site {
        id: SiteId::NextnGet,
        file: SiteFile::Owner,
        line: 122,
        function: "read_num_nextn",
        text: Some("            return json.load(f).get(\"num_nextn_predict_layers\")"),
        columns: Some((19, 35)),
        anchors: None,
    },
    Site {
        id: SiteId::JsonLoadRead,
        file: SiteFile::Stdlib("json/__init__.py"),
        line: 293,
        function: "load",
        text: Some("    return loads(fp.read(),"),
        columns: Some((17, 26)),
        anchors: None,
    },
    Site {
        id: SiteId::CodecsDecode,
        file: SiteFile::Frozen("<frozen codecs>"),
        line: 322,
        function: "decode",
        text: None,
        columns: None,
        anchors: None,
    },
];
