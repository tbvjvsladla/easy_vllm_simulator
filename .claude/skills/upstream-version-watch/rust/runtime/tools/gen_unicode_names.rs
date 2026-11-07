//! Source-data generator (separate from the runtime; never run by the product program).
//!
//! Reads CPython 3.12.3 `Modules/unicodename_db.h` (the Unicode 15.0.0 character-name database that `unicodedata.c` compiles in) and writes
//! `src/requirement/unicode_names_data.rs`: the seven lookup arrays and the constants `_getucname` / `_getcode` of `unicodedata.c` read.
//! The arrays are copied number for number; nothing is computed, filtered or looked up by calling an interpreter.
//!
//! Usage: `rustc -O tools/gen_unicode_names.rs -o <out>/gen_unicode_names && <out>/gen_unicode_names <unicodename_db.h> <unicode_names_data.rs>`.
//! Provenance and notices: `notices/NOTICE-unicode-names.md`.

use std::fmt::Write as _;

fn array_body<'a>(source: &'a str, name: &str) -> &'a str {
    let marker = format!("{name}[] = {{");
    let start = source
        .find(&marker)
        .unwrap_or_else(|| panic!("array {name} not found"))
        + marker.len();
    let end = source[start..]
        .find("};")
        .unwrap_or_else(|| panic!("array {name} not closed"))
        + start;
    &source[start..end]
}

fn numbers(body: &str) -> Vec<u64> {
    body.split(|c: char| c == ',' || c.is_whitespace())
        .filter(|token| !token.is_empty())
        .map(|token| match token.strip_prefix("0x") {
            Some(hex) => u64::from_str_radix(hex, 16).expect("hex literal"),
            None => token.parse().unwrap_or_else(|_| panic!("number {token:?}")),
        })
        .collect()
}

fn define(source: &str, name: &str) -> u64 {
    let marker = format!("#define {name} ");
    let start = source
        .find(&marker)
        .unwrap_or_else(|| panic!("define {name} not found"))
        + marker.len();
    let line = source[start..].lines().next().expect("define value");
    numbers(line)[0]
}

fn constant(source: &str, name: &str) -> u64 {
    let marker = format!("{name} = ");
    let start = source
        .find(&marker)
        .unwrap_or_else(|| panic!("constant {name} not found"))
        + marker.len();
    let end = source[start..].find(';').expect("constant end") + start;
    numbers(&source[start..end])[0]
}

fn emit_array(out: &mut String, rust_name: &str, element: &str, values: &[u64]) {
    let _ = writeln!(out, "pub(super) static {rust_name}: [{element}; {}] = [", values.len());
    for chunk in values.chunks(16) {
        let line: Vec<String> = chunk.iter().map(u64::to_string).collect();
        let _ = writeln!(out, "    {},", line.join(", "));
    }
    out.push_str("];\n\n");
}

fn main() {
    let arguments: Vec<String> = std::env::args().collect();
    let [_, header, target] = arguments.as_slice() else {
        eprintln!("usage: gen_unicode_names <unicodename_db.h> <unicode_names_data.rs>");
        std::process::exit(2);
    };
    let source = std::fs::read_to_string(header).expect("read header");
    let mut out = String::new();
    out.push_str(
        "//! Generated data - do not edit by hand.\n//!\n//! Source: CPython 3.12.3 `Modules/unicodename_db.h` (Unicode 15.0.0 character names, generated there by\n\
         //! `Tools/unicode/makeunicodedata.py`). Generator: `tools/gen_unicode_names.rs` (copies the arrays number for number).\n//!\n\
         //! Derived from CPython (PSF License Version 2) and the Unicode Character Database 15.0.0 (Unicode License V3); see `notices/NOTICE-unicode-names.md`.\n\n",
    );
    let _ = writeln!(out, "pub(super) const NAME_MAX_LENGTH: usize = {};", define(&source, "NAME_MAXLEN"));
    let _ = writeln!(out, "pub(super) const PHRASEBOOK_SHIFT: u32 = {};", define(&source, "phrasebook_shift"));
    let _ = writeln!(out, "pub(super) const PHRASEBOOK_SHORT: u8 = {};", define(&source, "phrasebook_short"));
    let _ = writeln!(out, "pub(super) const CODE_MAGIC: u32 = {};", define(&source, "code_magic"));
    let _ = writeln!(out, "pub(super) const CODE_SIZE: u32 = {};", define(&source, "code_size"));
    let _ = writeln!(out, "pub(super) const CODE_POLY: u32 = {};", define(&source, "code_poly"));
    let _ = writeln!(out, "pub(super) const ALIASES_START: u32 = {:#x};", constant(&source, "aliases_start"));
    let _ = writeln!(out, "pub(super) const ALIASES_END: u32 = {:#x};", constant(&source, "aliases_end"));
    let _ = writeln!(out, "pub(super) const NAMED_SEQUENCES_START: u32 = {:#x};", constant(&source, "named_sequences_start"));
    let _ = writeln!(out, "pub(super) const NAMED_SEQUENCES_END: u32 = {:#x};\n", constant(&source, "named_sequences_end"));
    emit_array(&mut out, "LEXICON", "u8", &numbers(array_body(&source, "lexicon")));
    emit_array(&mut out, "LEXICON_OFFSET", "u32", &numbers(array_body(&source, "lexicon_offset")));
    emit_array(&mut out, "PHRASEBOOK", "u8", &numbers(array_body(&source, "phrasebook")));
    emit_array(&mut out, "PHRASEBOOK_OFFSET1", "u16", &numbers(array_body(&source, "phrasebook_offset1")));
    emit_array(&mut out, "PHRASEBOOK_OFFSET2", "u32", &numbers(array_body(&source, "phrasebook_offset2")));
    emit_array(&mut out, "CODE_HASH", "u32", &numbers(array_body(&source, "code_hash")));
    emit_array(&mut out, "NAME_ALIASES", "u32", &numbers(array_body(&source, "name_aliases")));
    std::fs::write(target, out).expect("write output");
}
