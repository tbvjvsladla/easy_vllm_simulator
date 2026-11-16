//! The gzip container Python's `gzip` module reads and writes (RFC 1952), over raw DEFLATE from `miniz_oxide`.
//!
//! Writing follows `gzip.open(path, "wb")`: FNAME = the file's base name without `.gz`, MTIME = now, XFL 2 (level 9), OS 255.
//! The compressed bytes differ from CPython's zlib (another DEFLATE encoder); the decompressed content is identical.

use std::io::{Error, ErrorKind};

pub fn compress(data: &[u8], fname: &str, mtime: u32) -> Vec<u8> {
    let mut out = vec![0x1f, 0x8b, 0x08, 0x08];
    out.extend_from_slice(&mtime.to_le_bytes());
    out.extend_from_slice(&[0x02, 0xff]);
    out.extend(
        fname
            .chars()
            .map(|c| if (c as u32) < 256 { c as u8 } else { b'?' }),
    );
    out.push(0);
    out.extend(miniz_oxide::deflate::compress_to_vec(data, 9));
    let mut h = crc32fast::Hasher::new();
    h.update(data);
    out.extend_from_slice(&h.finalize().to_le_bytes());
    out.extend_from_slice(&(data.len() as u32).to_le_bytes());
    out
}

fn bad(m: &str) -> Error {
    Error::new(ErrorKind::InvalidData, m.to_string())
}

/// Every member of a gzip file, concatenated (`gzip.open(path).read()`).
pub fn decompress(mut b: &[u8]) -> Result<Vec<u8>, Error> {
    let mut out = vec![];
    let mut first = true;
    while !b.is_empty() {
        if b.len() < 10 || b[0] != 0x1f || b[1] != 0x8b {
            if !first && b.iter().all(|&x| x == 0) {
                break;
            }
            return Err(bad("Not a gzipped file"));
        }
        if b[2] != 8 {
            return Err(bad("Unknown compression method"));
        }
        let flags = b[3];
        let mut i = 10;
        if flags & 4 != 0 {
            if b.len() < i + 2 {
                return Err(bad("truncated"));
            }
            let n = u16::from_le_bytes([b[i], b[i + 1]]) as usize;
            i += 2 + n;
        }
        for f in [8u8, 16] {
            if flags & f != 0 {
                let z = b
                    .get(i..)
                    .and_then(|r| r.iter().position(|&x| x == 0))
                    .ok_or_else(|| bad("truncated"))?;
                i += z + 1;
            }
        }
        if flags & 2 != 0 {
            i += 2;
        }
        let body = b.get(i..).ok_or_else(|| bad("truncated"))?;
        let (data, used) = inflate_member(body)?;
        out.extend(data);
        let rest = body.get(used + 8..).ok_or_else(|| {
            bad("Compressed file ended before the end-of-stream marker was reached")
        })?;
        b = rest;
        first = false;
    }
    Ok(out)
}

fn inflate_member(body: &[u8]) -> Result<(Vec<u8>, usize), Error> {
    use miniz_oxide::inflate::stream::{inflate, InflateState};
    use miniz_oxide::{DataFormat, MZFlush, MZStatus};
    let mut state = InflateState::new_boxed(DataFormat::Raw);
    let mut out = vec![];
    let mut buf = vec![0u8; 64 * 1024];
    let mut input = body;
    let mut consumed = 0usize;
    loop {
        let r = inflate(&mut state, input, &mut buf, MZFlush::None);
        consumed += r.bytes_consumed;
        input = &input[r.bytes_consumed..];
        out.extend_from_slice(&buf[..r.bytes_written]);
        match r.status {
            Ok(MZStatus::StreamEnd) => return Ok((out, consumed)),
            Ok(_) if r.bytes_consumed > 0 || r.bytes_written > 0 => {}
            _ => {
                return Err(bad(
                    "Compressed file ended before the end-of-stream marker was reached",
                ))
            }
        }
    }
}
