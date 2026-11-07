//! The part of CPython 3.12.3 `zipfile.ZipFile(path)` that `wheel_metadata` uses: `namelist()` and `read(name)` over an in-memory archive.
//!
//! Ported from `zipfile/__init__.py` (`_EndRecData`, `_RealGetContents`, `ZipFile.open`, `ZipExtFile`): the end-of-central-directory record is found
//! at the end of the file or by a back-scan over the last 64 KiB (`rfind` of its signature), a central directory that does not start where the
//! record says is located through the prepended-data offset, names are decoded as UTF-8 when the entry says so and as CP437 otherwise and are cut at
//! the first NUL, the local header must carry the same name, an entry must not run into the next one, and the CRC-32 of what was read is compared at
//! the end of the entry. Nothing is extracted and no path is sanitized: names are only listed and one member is read.
//!
//! Raw DEFLATE is decoded by `miniz_oxide` and the CRC-32 by `crc32fast` (the approved low-level crates); the container logic is this module's.
//!
//! Resource boundary (explicit): like `zlib.decompressobj` as `zipfile` drives it, the decoded size of an entry is not capped; a result larger than
//! memory fails like any allocation. A truncated DEFLATE stream yields the bytes decoded so far and is then judged by the CRC check, as in the
//! original. Named scope (reported as [`ZipError::OutOfScope`], never invented as a Python exception): ZIP64 records and fields, the Unicode-path
//! extra field, compression methods other than stored and DEFLATE, and corrupt DEFLATE data (the original's `zlib.error` text is not reproduced).

use crate::http_direct::PyException;
use crate::py_errors::{builtin, with_message};
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::bytes_repr;
use miniz_oxide::inflate::{decompress_to_vec, TINFLStatus};

const END_SIGNATURE: &[u8; 4] = b"PK\x05\x06";
const END64_LOCATOR_SIGNATURE: &[u8; 4] = b"PK\x06\x07";
const CENTRAL_SIGNATURE: &[u8; 4] = b"PK\x01\x02";
const LOCAL_SIGNATURE: &[u8; 4] = b"PK\x03\x04";
const END_RECORD_SIZE: usize = 22;
const CENTRAL_HEADER_SIZE: usize = 46;
const LOCAL_HEADER_SIZE: usize = 30;
const MAX_EXTRACT_VERSION: u8 = 63;
const FLAG_ENCRYPTED: u16 = 0x1;
const FLAG_COMPRESSED_PATCH: u16 = 0x20;
const FLAG_STRONG_ENCRYPTION: u16 = 0x40;
const FLAG_UTF8_NAME: u16 = 0x800;
const STORED: u16 = 0;
const DEFLATED: u16 = 8;

/// CP437 for the bytes 0x80..=0xFF (Python's `cp437` codec).
const CP437_HIGH: [char; 128] = [
    'Ç', 'ü', 'é', 'â', 'ä', 'à', 'å', 'ç', 'ê', 'ë', 'è', 'ï', 'î', 'ì', 'Ä', 'Å', 'É', 'æ', 'Æ',
    'ô', 'ö', 'ò', 'û', 'ù', 'ÿ', 'Ö', 'Ü', '¢', '£', '¥', '₧', 'ƒ', 'á', 'í', 'ó', 'ú', 'ñ', 'Ñ',
    'ª', 'º', '¿', '⌐', '¬', '½', '¼', '¡', '«', '»', '░', '▒', '▓', '│', '┤', '╡', '╢', '╖', '╕',
    '╣', '║', '╗', '╝', '╜', '╛', '┐', '└', '┴', '┬', '├', '─', '┼', '╞', '╟', '╚', '╔', '╩', '╦',
    '╠', '═', '╬', '╧', '╨', '╤', '╥', '╙', '╘', '╒', '╓', '╫', '╪', '┘', '┌', '█', '▄', '▌', '▐',
    '▀', 'α', 'ß', 'Γ', 'π', 'Σ', 'σ', 'µ', 'τ', 'Φ', 'Θ', 'Ω', 'δ', '∞', 'φ', 'ε', '∩', '≡', '±',
    '≥', '≤', '⌠', '⌡', '÷', '≈', '°', '∙', '·', '√', 'ⁿ', '²', '■', '\u{a0}',
];

/// What can end a read of the archive.
#[derive(Clone, Debug)]
pub enum ZipError {
    /// An exception the original raises (`BadZipFile`, `NotImplementedError`, `RuntimeError`, `EOFError`, `UnicodeDecodeError`).
    Raised(Box<PyException>),
    /// A situation outside the named scope of this module.
    OutOfScope {
        boundary: &'static str,
        detail: String,
    },
}

impl From<PyException> for ZipError {
    fn from(error: PyException) -> ZipError {
        ZipError::Raised(Box::new(error))
    }
}

fn bad_zip_file(message: &str) -> ZipError {
    with_message("BadZipFile", "zipfile", message).into()
}

fn out_of_scope(boundary: &'static str, detail: &str) -> ZipError {
    ZipError::OutOfScope {
        boundary,
        detail: detail.to_string(),
    }
}

fn le16(bytes: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([bytes[at], bytes[at + 1]])
}

fn le32(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]])
}

/// One member as the central directory describes it.
#[derive(Clone, Debug)]
pub struct ZipEntry {
    /// `ZipInfo.filename`: the decoded name cut at its first NUL.
    pub name: PyStr,
    /// `ZipInfo.orig_filename`: the decoded name as stored, compared with the local header's.
    original_name: PyStr,
    flag_bits: u16,
    compress_type: u16,
    crc: u32,
    compress_size: u32,
    file_size: u32,
    header_offset: usize,
    end_offset: usize,
    extract_version: u8,
}

/// The table of contents of one archive held in memory.
#[derive(Debug)]
pub struct ZipArchive<'a> {
    data: &'a [u8],
    entries: Vec<ZipEntry>,
}

/// `bytes.decode("utf-8")` of a name or `cp437` when the entry does not say UTF-8.
fn decode_name(raw: &[u8], flags: u16) -> Result<PyStr, ZipError> {
    if flags & FLAG_UTF8_NAME != 0 {
        return match std::str::from_utf8(raw) {
            Ok(text) => Ok(PyStr::new(text)),
            Err(error) => Err(crate::http_direct::failure::utf8_decode_error(raw, &error).into()),
        };
    }
    Ok(PyStr::new(
        &raw.iter()
            .map(|&byte| {
                if byte < 0x80 {
                    char::from(byte)
                } else {
                    CP437_HIGH[usize::from(byte) - 0x80]
                }
            })
            .collect::<String>(),
    ))
}

/// `_EndRecData`: the offset of the end-of-central-directory record, or `None`.
fn find_end_record(data: &[u8]) -> Option<usize> {
    let size = data.len();
    if size < END_RECORD_SIZE {
        return None;
    }
    let tail = &data[size - END_RECORD_SIZE..];
    if &tail[..4] == END_SIGNATURE && tail[END_RECORD_SIZE - 2..] == [0, 0] {
        return Some(size - END_RECORD_SIZE);
    }
    let scan_start = size.saturating_sub((1 << 16) + END_RECORD_SIZE);
    let window = &data[scan_start..];
    let found = window
        .windows(4)
        .enumerate()
        .rev()
        .find(|(_, candidate)| *candidate == END_SIGNATURE)
        .map(|(at, _)| at)?;
    // a record cut short by the end of the file is "not a zip file"
    (window.len() - found >= END_RECORD_SIZE).then_some(scan_start + found)
}

/// The extra fields of a central directory entry: corrupt lengths fail, the ZIP64 and Unicode-path fields are outside the named scope.
fn check_extra(extra: &[u8], entry: &ZipEntry, header_offset_field: u32) -> Result<(), ZipError> {
    let mut rest = extra;
    while rest.len() >= 4 {
        let (id, length) = (le16(rest, 0), usize::from(le16(rest, 2)));
        if length + 4 > rest.len() {
            return Err(bad_zip_file(&format!(
                "Corrupt extra field {id:04x} (size={length})"
            )));
        }
        match id {
            0x0001
                if entry.file_size == u32::MAX
                    || entry.compress_size == u32::MAX
                    || header_offset_field == u32::MAX =>
            {
                return Err(out_of_scope(
                    "zip64",
                    "a ZIP64 extra field supplies a size or offset",
                ));
            }
            0x7075 => {
                return Err(out_of_scope(
                    "zip_unicode_path",
                    "the Unicode path extra field renames the entry",
                ));
            }
            _ => {}
        }
        rest = &rest[length + 4..];
    }
    Ok(())
}

impl<'a> ZipArchive<'a> {
    /// `ZipFile(path)` over the bytes of the file: reads the table of contents.
    pub fn open(data: &'a [u8]) -> Result<ZipArchive<'a>, ZipError> {
        let record = find_end_record(data).ok_or_else(|| bad_zip_file("File is not a zip file"))?;
        if record >= 20 && &data[record - 20..record - 16] == END64_LOCATOR_SIGNATURE {
            return Err(out_of_scope(
                "zip64",
                "a ZIP64 end-of-central-directory locator precedes the end record",
            ));
        }
        let field = &data[record..record + END_RECORD_SIZE];
        let (size_cd, offset_cd) = (le32(field, 12) as usize, le32(field, 16) as usize);
        let concat = i64::try_from(record).expect("offset") - size_cd as i64 - offset_cd as i64;
        let start_dir = offset_cd as i64 + concat;
        if start_dir < 0 {
            return Err(bad_zip_file("Bad offset for central directory"));
        }
        let start_dir = start_dir as usize;
        let directory = data.get(start_dir..).unwrap_or(&[]);
        let directory = &directory[..directory.len().min(size_cd)];
        let mut entries = Vec::new();
        let mut at = 0;
        while at < size_cd {
            let header = directory
                .get(at..at + CENTRAL_HEADER_SIZE)
                .ok_or_else(|| bad_zip_file("Truncated central directory"))?;
            if &header[..4] != CENTRAL_SIGNATURE {
                return Err(bad_zip_file("Bad magic number for central directory"));
            }
            let (name_length, extra_length, comment_length) = (
                usize::from(le16(header, 28)),
                usize::from(le16(header, 30)),
                usize::from(le16(header, 32)),
            );
            let raw_name = directory
                .get(at + CENTRAL_HEADER_SIZE..at + CENTRAL_HEADER_SIZE + name_length)
                .unwrap_or(&[]);
            let extra = directory
                .get(
                    at + CENTRAL_HEADER_SIZE + name_length
                        ..at + CENTRAL_HEADER_SIZE + name_length + extra_length,
                )
                .unwrap_or(&[]);
            let flags = le16(header, 8);
            let original_name = decode_name(raw_name, flags)?;
            let cut = original_name
                .code_points()
                .iter()
                .position(|&code| code == 0);
            let name = match cut {
                Some(at) => PyStr::from_code_points(original_name.code_points()[..at].to_vec()),
                None => original_name.clone(),
            };
            let entry = ZipEntry {
                name,
                original_name,
                flag_bits: flags,
                compress_type: le16(header, 10),
                crc: le32(header, 16),
                compress_size: le32(header, 20),
                file_size: le32(header, 24),
                header_offset: le32(header, 42) as usize,
                end_offset: 0,
                extract_version: header[6],
            };
            if entry.extract_version > MAX_EXTRACT_VERSION {
                return Err(builtin(
                    "NotImplementedError",
                    &format!(
                        "zip file version {:.1}",
                        f64::from(entry.extract_version) / 10.0
                    ),
                )
                .into());
            }
            check_extra(extra, &entry, le32(header, 42))?;
            entries.push(ZipEntry {
                header_offset: entry.header_offset.wrapping_add_signed(concat as isize),
                ..entry
            });
            at += CENTRAL_HEADER_SIZE + name_length + extra_length + comment_length;
        }
        // each entry ends where the next one (by offset) starts: the overlap check of `open`
        let mut order: Vec<usize> = (0..entries.len()).collect();
        order.sort_by(|&left, &right| {
            entries[right]
                .header_offset
                .cmp(&entries[left].header_offset)
        });
        let mut end = start_dir;
        for index in order {
            entries[index].end_offset = end;
            end = entries[index].header_offset;
        }
        Ok(ZipArchive { data, entries })
    }

    /// `namelist()`: the names in central-directory order, duplicates included.
    pub fn namelist(&self) -> Vec<&PyStr> {
        self.entries.iter().map(|entry| &entry.name).collect()
    }

    /// `read(name)` for the entry at `index` of [`ZipArchive::namelist`].
    pub fn read_entry(&self, index: usize) -> Result<Vec<u8>, ZipError> {
        let entry = &self.entries[index];
        let header = self
            .data
            .get(entry.header_offset..entry.header_offset + LOCAL_HEADER_SIZE)
            .ok_or_else(|| bad_zip_file("Truncated file header"))?;
        if &header[..4] != LOCAL_SIGNATURE {
            return Err(bad_zip_file("Bad magic number for file header"));
        }
        let (name_length, extra_length) =
            (usize::from(le16(header, 26)), usize::from(le16(header, 28)));
        let name_at = entry.header_offset + LOCAL_HEADER_SIZE;
        let raw_name = self.data.get(name_at..name_at + name_length).unwrap_or(&[]);
        let data_at = name_at + name_length + extra_length;
        if entry.flag_bits & FLAG_COMPRESSED_PATCH != 0 {
            return Err(builtin(
                "NotImplementedError",
                "compressed patched data (flag bit 5)",
            )
            .into());
        }
        if entry.flag_bits & FLAG_STRONG_ENCRYPTION != 0 {
            return Err(builtin("NotImplementedError", "strong encryption (flag bit 6)").into());
        }
        let local_name = decode_name(raw_name, le16(header, 6))?;
        if local_name != entry.original_name {
            return Err(bad_zip_file(&format!(
                "File name in directory {} and header {} differ.",
                entry.original_name.repr(),
                bytes_repr(raw_name)
            )));
        }
        if data_at + entry.compress_size as usize > entry.end_offset {
            return Err(bad_zip_file(&format!(
                "Overlapped entries: {} (possible zip bomb)",
                entry.original_name.repr()
            )));
        }
        if entry.flag_bits & FLAG_ENCRYPTED != 0 {
            return Err(builtin(
                "RuntimeError",
                &format!(
                    "File {} is encrypted, password required for extraction",
                    entry.name.repr()
                ),
            )
            .into());
        }
        let available = self.data.get(data_at..).unwrap_or(&[]);
        let region = &available[..available.len().min(entry.compress_size as usize)];
        let wanted = entry.file_size as usize;
        let decoded = match entry.compress_type {
            STORED => {
                if wanted <= region.len() {
                    region[..wanted].to_vec()
                } else if region.len() == entry.compress_size as usize {
                    region.to_vec()
                } else {
                    // the file ends inside the stored region before enough bytes were found: `_read2` raises EOFError
                    return Err(builtin("EOFError", "").into());
                }
            }
            DEFLATED => {
                let mut output = match decompress_to_vec(region) {
                    Ok(output) => output,
                    Err(failure)
                        if matches!(
                            failure.status,
                            TINFLStatus::NeedsMoreInput | TINFLStatus::FailedCannotMakeProgress
                        ) =>
                    {
                        failure.output
                    }
                    Err(_) => {
                        return Err(out_of_scope("zlib_error", "the DEFLATE data is corrupt; the interpreter's zlib.error text is not reproduced"));
                    }
                };
                output.truncate(wanted);
                output
            }
            other => {
                return Err(out_of_scope(
                    "zip_compression",
                    &format!("compression method {other} is outside the stored/DEFLATE scope"),
                ));
            }
        };
        if crc32fast::hash(&decoded) != entry.crc {
            return Err(bad_zip_file(&format!(
                "Bad CRC-32 for file {}",
                entry.name.repr()
            )));
        }
        Ok(decoded)
    }
}

/// A minimal ZIP writer for the source's own self-test: STORED members with fixed fields, central directory and end record. It is not a general
/// writer (no ZIP64, no comments, no compression).
pub fn write_stored_archive(members: &[(&str, &[u8])]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut central = Vec::new();
    for (name, content) in members {
        let offset = u32::try_from(out.len()).expect("self-test archives are small");
        let crc = crc32fast::hash(content);
        let size = u32::try_from(content.len()).expect("self-test members are small");
        let name_bytes = name.as_bytes();
        out.extend_from_slice(LOCAL_SIGNATURE);
        out.extend_from_slice(&[20, 0, 0x00, 0x08, 0, 0, 0, 0, 0x21, 0]);
        out.extend_from_slice(&crc.to_le_bytes());
        out.extend_from_slice(&size.to_le_bytes());
        out.extend_from_slice(&size.to_le_bytes());
        out.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(name_bytes);
        out.extend_from_slice(content);
        central.extend_from_slice(CENTRAL_SIGNATURE);
        central.extend_from_slice(&[20, 3, 20, 0, 0x00, 0x08, 0, 0, 0, 0, 0x21, 0]);
        central.extend_from_slice(&crc.to_le_bytes());
        central.extend_from_slice(&size.to_le_bytes());
        central.extend_from_slice(&size.to_le_bytes());
        central.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes());
        central.extend_from_slice(&[0; 12]);
        central.extend_from_slice(&offset.to_le_bytes());
        central.extend_from_slice(name_bytes);
    }
    let directory_offset = u32::try_from(out.len()).expect("small");
    let directory_size = u32::try_from(central.len()).expect("small");
    out.extend_from_slice(&central);
    out.extend_from_slice(END_SIGNATURE);
    out.extend_from_slice(&[0, 0, 0, 0]);
    out.extend_from_slice(&(members.len() as u16).to_le_bytes());
    out.extend_from_slice(&(members.len() as u16).to_le_bytes());
    out.extend_from_slice(&directory_size.to_le_bytes());
    out.extend_from_slice(&directory_offset.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn message(error: &ZipError) -> String {
        match error {
            ZipError::Raised(error) => error.message.to_utf8().unwrap(),
            ZipError::OutOfScope { boundary, .. } => format!("out of scope: {boundary}"),
        }
    }

    fn names(archive: &ZipArchive<'_>) -> Vec<String> {
        archive
            .namelist()
            .iter()
            .map(|name| name.to_utf8().unwrap())
            .collect()
    }

    #[test]
    fn stored_members_round_trip_in_directory_order_with_duplicates_listed() {
        let bytes = write_stored_archive(&[("a/x", b"one"), ("a/x", b"two"), ("b", b"")]);
        let archive = ZipArchive::open(&bytes).unwrap();
        assert_eq!(names(&archive), ["a/x", "a/x", "b"]);
        assert_eq!(archive.read_entry(0).unwrap(), b"one");
        assert_eq!(archive.read_entry(1).unwrap(), b"two");
        assert_eq!(archive.read_entry(2).unwrap(), b"");
    }

    #[test]
    fn not_a_zip_file_and_a_bad_crc_are_the_originals_bad_zip_file_texts() {
        for data in [&b""[..], b"not a zip", &[0u8; 21]] {
            assert_eq!(
                message(&ZipArchive::open(data).unwrap_err()),
                "File is not a zip file"
            );
        }
        let mut bytes =
            write_stored_archive(&[("vllm-0.32.0.dist-info/METADATA", b"Name: vllm\n")]);
        // the central directory's CRC field is the one `ZipExtFile` compares against
        let central = bytes
            .windows(4)
            .position(|window| window == CENTRAL_SIGNATURE)
            .unwrap();
        bytes[central + 16..central + 20].copy_from_slice(&[0, 0, 0, 0]);
        let archive = ZipArchive::open(&bytes).unwrap();
        assert_eq!(
            message(&archive.read_entry(0).unwrap_err()),
            "Bad CRC-32 for file 'vllm-0.32.0.dist-info/METADATA'"
        );
    }

    #[test]
    fn a_comment_after_the_record_is_found_by_the_back_scan_and_prepended_data_shifts_offsets() {
        let mut bytes = b"prefix-bytes".to_vec();
        bytes.extend(write_stored_archive(&[("m", b"data")]));
        let archive = ZipArchive::open(&bytes).unwrap();
        assert_eq!(archive.read_entry(0).unwrap(), b"data");
        let mut commented = write_stored_archive(&[("m", b"data")]);
        let length = commented.len();
        commented[length - 2..].copy_from_slice(&3u16.to_le_bytes());
        commented.extend_from_slice(b"abc");
        assert_eq!(
            ZipArchive::open(&commented).unwrap().read_entry(0).unwrap(),
            b"data"
        );
    }

    /// One member stored with `method` around the already-compressed `payload`; `text` is what it must decode to (it gives the CRC and size).
    fn archive_with_method(method: u8, payload: &[u8], text: &[u8]) -> Vec<u8> {
        let crc = crc32fast::hash(text);
        let (compressed, size) = (payload.len() as u32, text.len() as u32);
        let mut out = Vec::new();
        out.extend_from_slice(LOCAL_SIGNATURE);
        out.extend_from_slice(&[20, 0, 0, 0, method, 0, 0, 0, 0, 0]);
        for field in [crc, compressed, size] {
            out.extend_from_slice(&field.to_le_bytes());
        }
        out.extend_from_slice(&[1, 0, 0, 0]);
        out.push(b'm');
        out.extend_from_slice(payload);
        let directory_offset = out.len() as u32;
        out.extend_from_slice(CENTRAL_SIGNATURE);
        out.extend_from_slice(&[20, 3, 20, 0, 0, 0, method, 0, 0, 0, 0, 0]);
        for field in [crc, compressed, size] {
            out.extend_from_slice(&field.to_le_bytes());
        }
        out.extend_from_slice(&[1, 0]);
        out.extend_from_slice(&[0; 12]);
        out.extend_from_slice(&0u32.to_le_bytes());
        out.push(b'm');
        let directory_size = out.len() as u32 - directory_offset;
        out.extend_from_slice(END_SIGNATURE);
        out.extend_from_slice(&[0, 0, 0, 0, 1, 0, 1, 0]);
        out.extend_from_slice(&directory_size.to_le_bytes());
        out.extend_from_slice(&directory_offset.to_le_bytes());
        out.extend_from_slice(&[0, 0]);
        out
    }

    #[test]
    fn deflate_members_are_decoded_and_checked_and_a_truncated_stream_fails_the_crc() {
        // raw DEFLATE of "hello" in one fixed-Huffman block (the body of the well-known zlib stream 78 9c cb 48 cd c9 c9 07 00 ...)
        let fixed_huffman: &[u8] = &[0xcb, 0x48, 0xcd, 0xc9, 0xc9, 0x07, 0x00];
        let bytes = archive_with_method(8, fixed_huffman, b"hello");
        assert_eq!(
            ZipArchive::open(&bytes).unwrap().read_entry(0).unwrap(),
            b"hello"
        );
        // one stored DEFLATE block: final bit, length, its complement, bytes
        let stored_block: &[u8] = &[0x01, 3, 0, 0xfc, 0xff, b'a', b'b', b'c'];
        let bytes = archive_with_method(8, stored_block, b"abc");
        assert_eq!(
            ZipArchive::open(&bytes).unwrap().read_entry(0).unwrap(),
            b"abc"
        );
        // a stream cut short decodes what it has and the CRC check judges it, as `zipfile` does
        let bytes = archive_with_method(8, &fixed_huffman[..4], b"hello");
        let error = ZipArchive::open(&bytes).unwrap().read_entry(0).unwrap_err();
        assert_eq!(message(&error), "Bad CRC-32 for file 'm'");
        // a method outside stored/DEFLATE is named, not guessed
        let bytes = archive_with_method(12, b"zz", b"zz");
        let error = ZipArchive::open(&bytes).unwrap().read_entry(0).unwrap_err();
        assert_eq!(message(&error), "out of scope: zip_compression");
    }

    #[test]
    fn names_decode_as_cp437_unless_flagged_utf8_and_stop_at_a_nul() {
        assert_eq!(
            decode_name(&[0x82, b'a'], 0).unwrap().to_utf8().unwrap(),
            "éa"
        );
        assert_eq!(
            decode_name(&[0xe1, 0xfd, 0xff], 0)
                .unwrap()
                .to_utf8()
                .unwrap(),
            "ß²\u{a0}"
        );
        assert_eq!(
            decode_name("é".as_bytes(), FLAG_UTF8_NAME)
                .unwrap()
                .to_utf8()
                .unwrap(),
            "é"
        );
        assert!(matches!(
            decode_name(&[0xff], FLAG_UTF8_NAME),
            Err(ZipError::Raised(_))
        ));
        let bytes = write_stored_archive(&[("good\0hidden", b"x")]);
        assert_eq!(names(&ZipArchive::open(&bytes).unwrap()), ["good"]);
    }
}
