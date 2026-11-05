//! OpenSSL's CA *directory* rule: a directory (`SSL_CERT_DIR`, `-CApath`) is not a bundle. It is an index. A certificate in
//! it is looked up by the hash of its subject name, from a file named `<hash>.<n>` (eight lowercase hexadecimal digits, a
//! dot, a number; `openssl rehash` makes these, usually as symbolic links). A file with another name is never opened by
//! OpenSSL, and a file whose name carries the *wrong* hash is never found when its certificate is looked up - so
//! neither can become a trust anchor. A CA *file* (`SSL_CERT_FILE`, `-CAfile`) is the opposite: a PEM bundle of any name.
//!
//! This module reads a directory with that rule, so the Rust lookup trusts exactly what OpenSSL (and Python's `ssl`)
//! would trust from the same directory:
//!
//! * [`hashed_entry_index`] recognises the entry name;
//! * [`subject_name_hash`] computes OpenSSL's `X509_subject_name_hash` of a DER certificate (the first four bytes,
//!   little-endian, of the SHA-1 of the *canonical* subject: every attribute value converted to UTF-8, outer spaces
//!   removed, inner space runs collapsed, ASCII lower-cased, and each RDN re-encoded as a DER set). SHA-1 is used here
//!   only because it names the files; it is not a security decision (the certificate is trusted or not by the verifier);
//! * [`load_directory_roots`] keeps a certificate of such a file only when its subject hash equals the hash in the name.
//!
//! The directory is read once, not recursively. Entries that cannot be read contribute nothing.

use ring::digest::{digest, SHA1_FOR_LEGACY_USE_ONLY};
use rustls_native_certs::load_certs_from_paths;
use std::path::Path;

/// `<hash>.<n>`: the hash and the collision index of an entry name OpenSSL looks for, or `None` for any other name.
pub fn hashed_entry_index(name: &str) -> Option<(u32, u32)> {
    let (hash, index) = name.split_once('.')?;
    let is_lower_hex = |c: char| c.is_ascii_digit() || ('a'..='f').contains(&c);
    if hash.len() != 8 || !hash.chars().all(is_lower_hex) {
        return None;
    }
    if index.is_empty() || !index.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    Some((u32::from_str_radix(hash, 16).ok()?, index.parse().ok()?))
}

/// One DER element: its tag, its content and what follows it.
struct Element<'a> {
    tag: u8,
    content: &'a [u8],
    whole: &'a [u8],
    rest: &'a [u8],
}

fn read_element(data: &[u8]) -> Option<Element<'_>> {
    let tag = *data.first()?;
    if tag & 0x1F == 0x1F {
        return None; // high tag numbers do not occur in certificates
    }
    let first = *data.get(1)?;
    let (length, header) = if first < 0x80 {
        (usize::from(first), 2)
    } else {
        let count = usize::from(first & 0x7F);
        if count == 0 || count > 4 {
            return None;
        }
        let bytes = data.get(2..2 + count)?;
        (
            bytes
                .iter()
                .fold(0_usize, |n, &b| (n << 8) | usize::from(b)),
            2 + count,
        )
    };
    let end = header.checked_add(length)?;
    Some(Element {
        tag,
        content: data.get(header..end)?,
        whole: &data[..end],
        rest: &data[end..],
    })
}

fn encode_element(tag: u8, content: &[u8], out: &mut Vec<u8>) {
    out.push(tag);
    match content.len() {
        n if n < 0x80 => out.push(n as u8),
        n if n < 0x100 => out.extend([0x81, n as u8]),
        n if n < 0x10000 => out.extend([0x82, (n >> 8) as u8, n as u8]),
        n => out.extend([0x83, (n >> 16) as u8, (n >> 8) as u8, n as u8]),
    }
    out.extend_from_slice(content);
}

const UTF8_STRING: u8 = 0x0C;
const PRINTABLE_STRING: u8 = 0x13;
const T61_STRING: u8 = 0x14;
const IA5_STRING: u8 = 0x16;
const VISIBLE_STRING: u8 = 0x1A;
const UNIVERSAL_STRING: u8 = 0x1C;
const BMP_STRING: u8 = 0x1E;

/// `ossl_isspace`: the six C-locale white-space characters.
fn is_space(byte: u8) -> bool {
    matches!(byte, b' ' | 0x09..=0x0D)
}

/// The attribute value as OpenSSL's `asn1_string_canon` makes it: UTF-8, no outer spaces, inner space runs collapsed to
/// one space, ASCII lower-cased. `None` for a string whose bytes are not valid for its type.
fn canonical_text(tag: u8, content: &[u8]) -> Option<Vec<u8>> {
    let utf8: Vec<u8> = match tag {
        UTF8_STRING => {
            std::str::from_utf8(content).ok()?;
            content.to_vec()
        }
        PRINTABLE_STRING | IA5_STRING | VISIBLE_STRING => content.to_vec(),
        // OpenSSL reads a T61String as ISO 8859-1
        T61_STRING => content
            .iter()
            .map(|&b| char::from(b))
            .collect::<String>()
            .into_bytes(),
        BMP_STRING => {
            if !content.len().is_multiple_of(2) {
                return None;
            }
            let units: Vec<u16> = content
                .chunks(2)
                .map(|p| u16::from_be_bytes([p[0], p[1]]))
                .collect();
            String::from_utf16(&units).ok()?.into_bytes()
        }
        UNIVERSAL_STRING => {
            if !content.len().is_multiple_of(4) {
                return None;
            }
            let mut text = String::new();
            for quad in content.chunks(4) {
                text.push(char::from_u32(u32::from_be_bytes([
                    quad[0], quad[1], quad[2], quad[3],
                ]))?);
            }
            text.into_bytes()
        }
        _ => return None,
    };
    let start = utf8
        .iter()
        .position(|&b| !is_space(b))
        .unwrap_or(utf8.len());
    let end = utf8
        .iter()
        .rposition(|&b| !is_space(b))
        .map_or(start, |last| last + 1);
    let mut out = Vec::with_capacity(end - start);
    let mut previous_space = false;
    for &byte in &utf8[start..end] {
        if byte.is_ascii() && is_space(byte) {
            if !previous_space {
                out.push(b' ');
            }
            previous_space = true;
        } else {
            out.push(byte.to_ascii_lowercase());
            previous_space = false;
        }
    }
    Some(out)
}

/// The canonical encoding of a subject `Name` (its RDNs without the outer sequence), as `i2d_name_canon` writes it.
fn canonical_name(name: &[u8]) -> Option<Vec<u8>> {
    let mut canonical = Vec::new();
    let mut rdns = name;
    while !rdns.is_empty() {
        let rdn = read_element(rdns)?;
        rdns = rdn.rest;
        let mut attributes: Vec<Vec<u8>> = Vec::new();
        let mut members = rdn.content;
        while !members.is_empty() {
            let attribute = read_element(members)?;
            members = attribute.rest;
            let oid = read_element(attribute.content)?;
            let value = read_element(oid.rest)?;
            let mut body = oid.whole.to_vec();
            match value.tag {
                UTF8_STRING | PRINTABLE_STRING | T61_STRING | IA5_STRING | VISIBLE_STRING
                | UNIVERSAL_STRING | BMP_STRING => {
                    encode_element(
                        UTF8_STRING,
                        &canonical_text(value.tag, value.content)?,
                        &mut body,
                    );
                }
                _ => body.extend_from_slice(value.whole),
            }
            let mut entry = Vec::new();
            encode_element(0x30, &body, &mut entry);
            attributes.push(entry);
        }
        // a DER `SET OF` is written in the order of its encodings
        attributes.sort();
        encode_element(0x31, &attributes.concat(), &mut canonical);
    }
    Some(canonical)
}

/// The subject `Name` (content only) of a DER certificate.
fn subject_of(certificate: &[u8]) -> Option<&[u8]> {
    let certificate = read_element(certificate)?;
    let tbs = read_element(certificate.content)?;
    let mut fields = tbs.content;
    let mut field = read_element(fields)?;
    if field.tag == 0xA0 {
        fields = field.rest; // the explicit version
    }
    // serial, signature algorithm, issuer, validity, then the subject
    for _ in 0..4 {
        field = read_element(fields)?;
        fields = field.rest;
    }
    let subject = read_element(fields)?;
    (subject.tag == 0x30).then_some(subject.content)
}

/// OpenSSL's `X509_subject_name_hash` of a DER certificate, or `None` when the certificate cannot be parsed far enough.
pub fn subject_name_hash(certificate: &[u8]) -> Option<u32> {
    let canonical = canonical_name(subject_of(certificate)?)?;
    let sha1 = digest(&SHA1_FOR_LEGACY_USE_ONLY, &canonical);
    let bytes = sha1.as_ref();
    Some(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

/// The trust anchors of one CA directory: the certificates of its `<hash>.<n>` entries whose subject hash is the hash in
/// the entry's name, as DER. Other names, wrong hashes, directories inside it and unreadable entries add nothing.
pub fn load_directory_roots(directory: &Path) -> Vec<Vec<u8>> {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return Vec::new();
    };
    let mut roots = Vec::new();
    for entry in entries.flatten() {
        let Some((named_hash, _)) = entry.file_name().to_str().and_then(hashed_entry_index) else {
            continue;
        };
        // a symbolic link (what `openssl rehash` makes) is followed; a directory is not a certificate file
        if !std::fs::metadata(entry.path()).is_ok_and(|metadata| metadata.is_file()) {
            continue;
        }
        let loaded = load_certs_from_paths(Some(&entry.path()), None);
        roots.extend(
            loaded
                .certs
                .iter()
                .map(|certificate| certificate.as_ref().to_vec())
                .filter(|certificate| subject_name_hash(certificate) == Some(named_hash)),
        );
    }
    roots
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A one-attribute name `CN=<tag, value>`.
    fn name(tag: u8, value: &[u8]) -> Vec<u8> {
        let mut oid = Vec::new();
        encode_element(0x06, &[0x55, 0x04, 0x03], &mut oid);
        encode_element(tag, value, &mut oid);
        let mut attribute = Vec::new();
        encode_element(0x30, &oid, &mut attribute);
        let mut rdn = Vec::new();
        encode_element(0x31, &attribute, &mut rdn);
        rdn
    }

    #[test]
    fn entry_names_are_eight_lowercase_hex_digits_a_dot_and_a_number() {
        assert_eq!(hashed_entry_index("0a1b2c3d.0"), Some((0x0A1B_2C3D, 0)));
        assert_eq!(hashed_entry_index("00000000.12"), Some((0, 12)));
        for bad in [
            "authority.pem",
            "0A1B2C3D.0",
            "0a1b2c3.0",
            "0a1b2c3d0",
            "0a1b2c3d.",
            "0a1b2c3d.r0",
            "0a1b2c3d.0.pem",
            "0a1b2c3dd.0",
            "0a1b2c3g.0",
        ] {
            assert_eq!(hashed_entry_index(bad), None, "{bad}");
        }
    }

    #[test]
    fn the_canonical_form_ignores_case_outer_spaces_and_string_type() {
        let plain = canonical_name(&name(UTF8_STRING, b"my ca")).unwrap();
        assert_eq!(
            canonical_name(&name(UTF8_STRING, b"  My   CA ")).unwrap(),
            plain
        );
        assert_eq!(
            canonical_name(&name(PRINTABLE_STRING, b"MY\tCA")).unwrap(),
            plain
        );
        assert_eq!(
            canonical_name(&name(
                BMP_STRING,
                &[0, b'm', 0, b'y', 0, b' ', 0, b'c', 0, b'a']
            ))
            .unwrap(),
            plain
        );
        assert_ne!(canonical_name(&name(UTF8_STRING, b"my-ca")).unwrap(), plain);
        // non-ASCII text is kept (only ASCII is folded)
        assert_ne!(
            canonical_name(&name(UTF8_STRING, "É".as_bytes())).unwrap(),
            canonical_name(&name(UTF8_STRING, "é".as_bytes())).unwrap()
        );
    }

    #[test]
    fn malformed_input_has_no_hash() {
        assert_eq!(subject_name_hash(&[]), None);
        assert_eq!(subject_name_hash(&[0x30, 0x03, 0x30, 0x01]), None);
    }
}
