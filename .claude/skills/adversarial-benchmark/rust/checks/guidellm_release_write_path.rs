//! The shared record read/write path of `merge_record` (`read_bytes`, `decode_file_text`, the chunked JSON dump and the
//! `FileSink`), checked for what an optimisation of it must not change: the exact chunks the dump hands to its sink
//! (borrowed-ASCII and text chunks alike), the bytes a file ends up with, where a failing chunk stops the file, which
//! error wins when both the dump and the close fail, how large and block-sized files are read, and carriage-return
//! translation. Everything here is a property of the generic code, not of any recorded case.

use std::io::Write;
use sweep_stop::guidellm::Failure;
use sweep_stop::json_stream::{dump_indented, open_for_writing, ChunkSink, FileSink};
use sweep_stop::pyfs::{read_bytes, strict_utf8, strict_utf8_into};
use sweep_stop::pyjson::{decode_file_text, loads_text};
use sweep_stop::pystr::PyStr;
use sweep_stop::pyvalue::{DictKey, NonStringKey, PyDict, PyValue};

type Res<T> = Result<T, Failure>;

/// Records the chunks as text, ignoring which of the two entry points delivered them.
#[derive(Default)]
struct Chunks(Vec<String>);

impl ChunkSink for Chunks {
    fn write(&mut self, chunk: &PyStr) -> Res<()> {
        self.0
            .push(chunk.to_utf8().expect("no surrogates in this document"));
        Ok(())
    }

    fn write_ascii(&mut self, chunk: &str) -> Res<()> {
        assert!(chunk.is_ascii(), "{chunk:?} was handed over as ASCII");
        self.0.push(chunk.to_string());
        Ok(())
    }
}

/// A sink with only the original entry point: the default `write_ascii` must give the same chunks.
#[derive(Default)]
struct TextOnly(Vec<String>);

impl ChunkSink for TextOnly {
    fn write(&mut self, chunk: &PyStr) -> Res<()> {
        self.0
            .push(chunk.to_utf8().expect("no surrogates in this document"));
        Ok(())
    }
}

fn parse(text: &str) -> PyValue {
    loads_text(&PyStr::new(text)).expect("JSON")
}

fn rich_document() -> PyValue {
    let mut document = match parse(
        r#"{"a": [1, -2, 3.5, 1e300, null, true, false, "xé中", [], {}, [[1], {"k": "v"}]],
            "b": {"nested": {"deeper": [{"x": 1}, "text"]}, "empty": [], "none": null},
            "한글": "값 \"quoted\" \\ \n tab\t", "big": 123456789012345678901234567890}"#,
    ) {
        PyValue::Dict(map) => map,
        _ => unreachable!(),
    };
    // keys that are not strings, as a merge into a record can make them
    let key = |identity: &str, repr: &str| {
        DictKey::Other(NonStringKey {
            identity: identity.into(),
            repr: repr.into(),
            value: None,
        })
    };
    document.insert_key(key("None", "None"), PyValue::str("none key"));
    document.insert_key(key("n:1", "True"), PyValue::str("bool key"));
    document.insert_key(key("v:1.5", "1.5"), PyValue::str("float key"));
    PyValue::Dict(document)
}

fn chunks_of<S: ChunkSink>(
    mut sink: S,
    document: &PyValue,
    extract: impl Fn(S) -> Vec<String>,
) -> Vec<String> {
    dump_indented(document, 2, 6, &mut sink).expect("dump");
    extract(sink)
}

#[test]
fn the_ascii_entry_point_hands_over_exactly_the_chunks_of_the_text_entry_point() {
    let document = rich_document();
    let fast = chunks_of(Chunks::default(), &document, |s| s.0);
    let plain = chunks_of(TextOnly::default(), &document, |s| s.0);
    assert_eq!(fast, plain);
    assert!(
        fast.len() > 60,
        "a document with real structure: {} chunks",
        fast.len()
    );
    // a scalar list item shares its chunk with the separator in front of it; a dict key, its `: ` and its value are chunks
    assert!(fast.contains(&",\n      -2".to_string()) || fast.iter().any(|c| c.ends_with("-2")));
    assert!(fast.iter().any(|c| c == ": "));
    assert!(fast.iter().any(|c| c == "\"한글\""));
}

#[test]
fn a_file_dump_is_byte_for_byte_the_whole_document_encoding() {
    let document = rich_document();
    let folder =
        std::env::temp_dir().join(format!("guidellm-write-path-{}-whole", std::process::id()));
    std::fs::create_dir_all(&folder).expect("folder");
    let path = PyStr::new(folder.join("out.json").to_str().expect("path"));
    let mut sink = FileSink::new(open_for_writing(&path).expect("open"));
    dump_indented(&document, 2, 6, &mut sink).expect("dump");
    sink.write(&PyStr::new("\n")).expect("newline");
    sink.close().expect("close");
    let mut expected = document
        .dumps_pretty()
        .expect("whole document")
        .to_utf8()
        .expect("UTF-8")
        .into_bytes();
    expected.push(b'\n');
    assert_eq!(
        std::fs::read(folder.join("out.json")).expect("file"),
        expected
    );
    let _ = std::fs::remove_dir_all(&folder);
}

#[test]
fn a_document_larger_than_the_write_buffer_and_a_very_long_string_are_written_whole() {
    let mut map = PyDict::new();
    for index in 0..2000 {
        map.set(
            &format!("key-{index}"),
            parse(&format!("[{index}, \"value-{index}\", null, 1.25]")),
        );
    }
    map.set("long", PyValue::str(&"é".repeat(100_000)));
    let document = PyValue::Dict(map);
    let folder =
        std::env::temp_dir().join(format!("guidellm-write-path-{}-large", std::process::id()));
    std::fs::create_dir_all(&folder).expect("folder");
    let path = PyStr::new(folder.join("big.json").to_str().expect("path"));
    let mut sink = FileSink::new(open_for_writing(&path).expect("open"));
    dump_indented(&document, 2, 6, &mut sink).expect("dump");
    sink.close().expect("close");
    let written = std::fs::read(folder.join("big.json")).expect("file");
    assert!(written.len() > 100_000 + 8192);
    assert_eq!(
        written,
        document
            .dumps_pretty()
            .expect("whole")
            .to_utf8()
            .expect("UTF-8")
            .into_bytes()
    );
    let _ = std::fs::remove_dir_all(&folder);
}

#[test]
fn a_lone_surrogate_stops_the_file_after_the_chunks_before_it_whichever_entry_point_wrote_them() {
    let folder = std::env::temp_dir().join(format!(
        "guidellm-write-path-{}-surrogate",
        std::process::id()
    ));
    std::fs::create_dir_all(&folder).expect("folder");
    let path = PyStr::new(folder.join("partial.json").to_str().expect("path"));
    let mut map = PyDict::new();
    map.set("before", parse("[1, 2, {\"deep\": \"é\"}]"));
    map.set(
        "unicode",
        PyValue::Str(PyStr::from_code_points(vec![0xD800])),
    );
    map.set("after", PyValue::str("never written"));
    let document = PyValue::Dict(map);
    let mut sink = FileSink::new(open_for_writing(&path).expect("open"));
    let dumped = dump_indented(&document, 2, 6, &mut sink);
    let closed = sink.close();
    assert!(closed.is_ok(), "what was accepted is flushed on close");
    let failure = dumped.expect_err("the surrogate cannot be encoded");
    assert_eq!(failure.class, "UnicodeEncodeError");
    assert_eq!(
        failure.message.to_utf8().expect("text"),
        "'utf-8' codec can't encode character '\\ud800' in position 1: surrogates not allowed"
    );
    let partial = String::from_utf8(std::fs::read(folder.join("partial.json")).expect("file"))
        .expect("UTF-8");
    assert_eq!(
        partial,
        "{\n  \"before\": [\n    1,\n    2,\n    {\n      \"deep\": \"é\"\n    }\n  ],\n  \"unicode\": "
    );
    let _ = std::fs::remove_dir_all(&folder);
}

#[cfg(target_os = "linux")]
#[test]
fn when_the_dump_and_the_close_both_fail_each_error_is_reported_where_the_original_reports_it() {
    // `/dev/full` accepts the open and fails every write with `ENOSPC`.
    let path = PyStr::new("/dev/full");
    // small document: nothing reaches the device before the close, so the dump succeeds and the close fails
    let mut sink = FileSink::new(open_for_writing(&path).expect("open /dev/full"));
    assert!(dump_indented(&parse("{\"a\": [1, 2]}"), 2, 6, &mut sink).is_ok());
    let closed = sink.close().expect_err("the flush at close fails");
    assert_eq!(closed.class, "OSError");
    assert_eq!(
        closed.message.to_utf8().expect("text"),
        "[Errno 28] No space left on device"
    );
    // large document: the buffer is flushed during the dump, so the dump itself fails - and so does the close
    let mut map = PyDict::new();
    map.set("long", PyValue::str(&"x".repeat(50_000)));
    let mut sink = FileSink::new(open_for_writing(&path).expect("open /dev/full"));
    let dumped = dump_indented(&PyValue::Dict(map), 2, 6, &mut sink);
    assert_eq!(dumped.expect_err("mid-dump flush").class, "OSError");
    let _ = sink.close();
    // a failing text chunk still wins over nothing written: the encode error is reported, not a write error
    let mut sink = FileSink::new(open_for_writing(&path).expect("open /dev/full"));
    let surrogate = PyStr::from_code_points(vec![0xDC80]);
    assert_eq!(
        sink.write(&surrogate).expect_err("encode").class,
        "UnicodeEncodeError"
    );
    let _ = sink.write_ascii("x");
}

#[test]
fn encoding_into_a_reused_buffer_equals_strict_utf8_and_leaves_the_buffer_alone_on_failure() {
    let mut out = b"keep".to_vec();
    for text in ["", "ascii", "é中😀", "\u{0}\n\"\\"] {
        let point_text = PyStr::new(text);
        let mark = out.len();
        strict_utf8_into(&point_text, &mut out).expect("encodes");
        assert_eq!(
            &out[mark..],
            strict_utf8(&point_text).expect("encodes").as_slice()
        );
    }
    let before = out.clone();
    for points in [
        vec![0x61, 0xD800, 0x62],
        vec![0xDC00, 0xDC01],
        vec![0xE9, 0xDFFF],
    ] {
        let text = PyStr::from_code_points(points);
        let error = strict_utf8_into(&text, &mut out).expect_err("a surrogate");
        assert_eq!(Err(error), strict_utf8(&text));
        assert_eq!(out, before, "the buffer is left as it was");
    }
}

#[test]
fn files_of_every_block_size_and_the_errors_are_read_as_before() {
    let folder =
        std::env::temp_dir().join(format!("guidellm-write-path-{}-read", std::process::id()));
    std::fs::create_dir_all(&folder).expect("folder");
    for size in [
        0_usize, 1, 4095, 4096, 4097, 8191, 8192, 8193, 12288, 100_000,
    ] {
        let data: Vec<u8> = (0..size).map(|n| (n % 251) as u8).collect();
        let file = folder.join(format!("f{size}"));
        std::fs::File::create(&file)
            .and_then(|mut f| f.write_all(&data))
            .expect("file");
        let read = read_bytes(&PyStr::new(file.to_str().expect("path"))).expect("read");
        assert_eq!(read, data, "{size} bytes");
    }
    let missing = folder.join("absent");
    let failure = read_bytes(&PyStr::new(missing.to_str().expect("path"))).expect_err("missing");
    assert_eq!(failure.class, "FileNotFoundError");
    assert_eq!(
        failure.message.to_utf8().expect("text"),
        format!(
            "[Errno 2] No such file or directory: '{}'",
            missing.display()
        )
    );
    let failure = read_bytes(&PyStr::new(folder.to_str().expect("path"))).expect_err("a directory");
    assert_eq!(failure.class, "IsADirectoryError");
    assert_eq!(
        failure.message.to_utf8().expect("text"),
        format!("[Errno 21] Is a directory: '{}'", folder.display())
    );
    assert_eq!(
        read_bytes(&PyStr::new("a\0b")).expect_err("NUL").class,
        "ValueError"
    );
    let _ = std::fs::remove_dir_all(&folder);
}

#[test]
fn file_text_translates_carriage_returns_only_when_there_are_any() {
    let plain = decode_file_text(b"{\n  \"a\": 1\n}\n").expect("text");
    assert_eq!(plain.to_utf8().as_deref(), Some("{\n  \"a\": 1\n}\n"));
    let crlf = decode_file_text(b"a\r\nb\rc\n").expect("text");
    assert_eq!(crlf.to_utf8().as_deref(), Some("a\nb\nc\n"));
    let invalid = decode_file_text(b"{\"x\":\"\xFF\"}").expect_err("not UTF-8");
    assert_eq!(
        invalid.to_string(),
        "'utf-8' codec can't decode byte 0xff in position 6: invalid start byte"
    );
}
