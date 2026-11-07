//! Which certificate authorities the HTTPS lookup trusts: the same choice OpenSSL makes for the Python original
//! (`ssl.create_default_context()` → `SSL_CTX_set_default_verify_paths`).
//!
//! * The CA **file** (`SSL_CERT_FILE`, else the host's default bundle) is one PEM bundle: every certificate in it is in the
//!   store. *Being in the store is not being a trust anchor*: with CPython's default context (no partial chains) only a
//!   self-signed certificate ([`is_self_signed`]) may end a chain; the others only help to build it (see [`crate::tls_verify`]).
//! * The CA **directories** (`SSL_CERT_DIR`, a `:`-separated list, else the host's default directories) are OpenSSL's hashed
//!   directories: a certificate is found through a file named for the *subject hash* of the name being looked up, and is an
//!   authority only for the subject it has, so a file whose certificate hashes to another name is never one. Which file names
//!   count was observed on the reference interpreter (CPython 3.12.3 with OpenSSL 3.0.13, whose default store also reads directories
//!   through its store loader): the name is the hash in eight hexadecimal digits (either case), a dot, an optional lower-case `r`
//!   and one or more ASCII digits, in any number and with gaps (`<hash>.0`, `<HASH>.7`, `<hash>.r0`, `<hash>.12345`); other
//!   names (`<hash>`, `<hash>.pem`, `<hash>.x`, `<hash>.R0`, a different digit script) are ignored. The entries of one hash are
//!   read in the order the operating system lists the directory, and the first entry that cannot be loaded (dangling link,
//!   directory, empty or non-PEM file) ends the reading for that hash, keeping what was read before it.
//! * Each side is chosen on its own: a variable that is set replaces only its own side (set but empty: that side is off); no
//!   variable: the host's default for that side. The process environment and the global trust are never changed.
//!
//! The subject hash is OpenSSL's `X509_subject_name_hash`: SHA-1 of the *canonical* encoding of the subject name, the first four
//! bytes read little-endian. The canonical encoding (`x509_name_canon` in `crypto/x509/x_name.c`, OpenSSL 3.0.13) turns every
//! string of a supported type (UTF8, BMP, Universal, Printable, T61, IA5, Visible) into UTF-8, drops leading and trailing ASCII
//! spaces, collapses inner runs of ASCII white space to one space, lower-cases ASCII letters, re-tags it as UTF8String, orders the
//! attributes of each RDN as a DER `SET OF` does (by their encodings) and leaves out the outer SEQUENCE. This module is a
//! transcription of that algorithm in pure Rust (`crypto/x509/x_name.c`, `x509_cmp.c`); the upstream license (Apache License 2.0) is
//! carried in `notices/OpenSSL-LICENSE.txt` (see `notices/NOTICE.md`). The directory lookup of `crypto/x509/by_dir.c` reads
//! `<hash>.0`, `<hash>.1`, ... without a gap; the interpreter the original runs on reads a directory more loosely (see above), and
//! the observed behaviour is what is implemented (the difference is recorded in `PORTING.md`).
//!
//! Scope of the directory rules: they were observed once, with Python's `ssl`/`urllib` as the oracle of the primitive (never the
//! original scripts), on CPython 3.12.3 / OpenSSL 3.0.13 (Ubuntu build) on the development host, for the layouts listed in
//! `PORTING.md`. Not claimed: the same behaviour in other OpenSSL versions or builds, or the answer for layouts whose result
//! depends on the order the file system lists a directory (not registered as checks), or the corner of a certificate whose
//! file name is another subject's hash. A certificate is never trusted merely because of its file name: its own subject hash
//! must equal the hash in the name.

use ring::digest::{digest, SHA1_FOR_LEGACY_USE_ONLY};
use rustls::pki_types::CertificateDer;
use rustls_native_certs::load_certs_from_paths;
use std::path::{Path, PathBuf};

// ── a small DER reader ──────────────────────────────────────────────────────────────────────────

/// One DER element: its tag, its content, and its whole encoding.
struct Tlv<'a> {
    tag: u8,
    content: &'a [u8],
    whole: &'a [u8],
}

/// The first element of `data` and what follows it (definite lengths and single-byte tags only, which is what a certificate
/// uses).
fn read_tlv(data: &[u8]) -> Option<(Tlv<'_>, &[u8])> {
    let tag = *data.first()?;
    if tag & 0x1F == 0x1F {
        return None;
    }
    let first = *data.get(1)?;
    let (length, header) = if first < 0x80 {
        (usize::from(first), 2)
    } else {
        let count = usize::from(first & 0x7F);
        if count == 0 || count > 4 {
            return None;
        }
        let mut length = 0usize;
        for &byte in data.get(2..2 + count)? {
            length = (length << 8) | usize::from(byte);
        }
        (length, 2 + count)
    };
    let end = header.checked_add(length)?;
    Some((
        Tlv {
            tag,
            content: data.get(header..end)?,
            whole: &data[..end],
        },
        &data[end..],
    ))
}

/// DER encoding of `content` under `tag`.
fn encode_tlv(tag: u8, content: &[u8]) -> Vec<u8> {
    let mut out = vec![tag];
    if content.len() < 0x80 {
        out.push(content.len() as u8);
    } else {
        let bytes = content.len().to_be_bytes();
        let skip = bytes.iter().take_while(|&&byte| byte == 0).count();
        out.push(0x80 | (bytes.len() - skip) as u8);
        out.extend_from_slice(&bytes[skip..]);
    }
    out.extend_from_slice(content);
    out
}

/// The subject `Name` (the whole SEQUENCE element) of a DER certificate.
fn subject_name(certificate: &[u8]) -> Option<&[u8]> {
    let (outer, _) = read_tlv(certificate)?;
    let (tbs, _) = read_tlv(outer.content)?;
    let mut rest = tbs.content;
    let (mut element, mut after) = read_tlv(rest)?;
    if element.tag == 0xA0 {
        rest = after;
        (element, after) = read_tlv(rest)?;
    }
    // serialNumber, signature, issuer, validity, subject
    let mut elements = vec![element];
    rest = after;
    for _ in 0..4 {
        let (next, after) = read_tlv(rest)?;
        elements.push(next);
        rest = after;
    }
    let subject = elements.pop()?;
    (subject.tag == 0x30).then_some(subject.whole)
}

// ── the canonical form of a name ────────────────────────────────────────────────────────────────

const UTF8_STRING: u8 = 0x0C;
const PRINTABLE_STRING: u8 = 0x13;
const T61_STRING: u8 = 0x14;
const IA5_STRING: u8 = 0x16;
const VISIBLE_STRING: u8 = 0x1A;
const UNIVERSAL_STRING: u8 = 0x1C;
const BMP_STRING: u8 = 0x1E;

/// `ASN1_STRING_to_UTF8`: the string as UTF-8, or `None` when it is not valid in its own encoding.
fn to_utf8(tag: u8, content: &[u8]) -> Option<Vec<u8>> {
    match tag {
        UTF8_STRING => std::str::from_utf8(content)
            .ok()
            .map(|text| text.as_bytes().to_vec()),
        // One byte, one character (ISO 8859-1), as `MBSTRING_ASC` reads them.
        PRINTABLE_STRING | T61_STRING | IA5_STRING | VISIBLE_STRING => {
            let text: String = content.iter().map(|&byte| char::from(byte)).collect();
            Some(text.into_bytes())
        }
        BMP_STRING => {
            let (pairs, rest) = content.as_chunks::<2>();
            if !rest.is_empty() {
                return None;
            }
            let units: Vec<u16> = pairs.iter().map(|pair| u16::from_be_bytes(*pair)).collect();
            String::from_utf16(&units).ok().map(String::into_bytes)
        }
        UNIVERSAL_STRING => {
            let (quads, rest) = content.as_chunks::<4>();
            if !rest.is_empty() {
                return None;
            }
            let mut text = String::new();
            for quad in quads {
                text.push(char::from_u32(u32::from_be_bytes(*quad))?);
            }
            Some(text.into_bytes())
        }
        _ => None,
    }
}

fn is_space(byte: u8) -> bool {
    matches!(byte, b' ' | 0x09..=0x0D)
}

/// `asn1_string_canon`: the string of a supported type in canonical form, as UTF-8.
fn canonical_string(tag: u8, content: &[u8]) -> Option<Vec<u8>> {
    let utf8 = to_utf8(tag, content)?;
    let mut from = 0usize;
    let mut len = utf8.len();
    while len > 0 && is_space(utf8[from]) {
        from += 1;
        len -= 1;
    }
    while len > 0 && is_space(utf8[from + len - 1]) {
        len -= 1;
    }
    let slice = &utf8[from..from + len];
    let mut out = Vec::with_capacity(slice.len());
    let mut at = 0;
    while at < slice.len() {
        let byte = slice[at];
        if !byte.is_ascii() {
            out.push(byte);
            at += 1;
        } else if is_space(byte) {
            out.push(b' ');
            while at < slice.len() && is_space(slice[at]) {
                at += 1;
            }
        } else {
            out.push(byte.to_ascii_lowercase());
            at += 1;
        }
    }
    Some(out)
}

fn is_canonicalised(tag: u8) -> bool {
    matches!(
        tag,
        UTF8_STRING
            | BMP_STRING
            | UNIVERSAL_STRING
            | PRINTABLE_STRING
            | T61_STRING
            | IA5_STRING
            | VISIBLE_STRING
    )
}

/// The canonical encoding of a `Name` element (the SETs of its RDNs, one after the other, without the outer SEQUENCE).
fn canonical_name(name: &[u8]) -> Option<Vec<u8>> {
    let (sequence, _) = read_tlv(name)?;
    if sequence.tag != 0x30 {
        return None;
    }
    let mut canonical = Vec::new();
    let mut rest = sequence.content;
    while !rest.is_empty() {
        let (rdn, after) = read_tlv(rest)?;
        rest = after;
        if rdn.tag != 0x31 {
            return None;
        }
        let mut entries: Vec<Vec<u8>> = Vec::new();
        let mut attributes = rdn.content;
        while !attributes.is_empty() {
            let (attribute, after) = read_tlv(attributes)?;
            attributes = after;
            if attribute.tag != 0x30 {
                return None;
            }
            let (object, remaining) = read_tlv(attribute.content)?;
            let (value, _) = read_tlv(remaining)?;
            let mut body = object.whole.to_vec();
            if is_canonicalised(value.tag) {
                body.extend(encode_tlv(
                    UTF8_STRING,
                    &canonical_string(value.tag, value.content)?,
                ));
            } else {
                body.extend_from_slice(value.whole);
            }
            entries.push(encode_tlv(0x30, &body));
        }
        // `SET OF` is written in the order of the encodings (`memcmp`, then the shorter one first).
        entries.sort();
        canonical.extend(encode_tlv(0x31, &entries.concat()));
    }
    Some(canonical)
}

/// `X509_NAME_hash` of a `Name` element: the first four bytes of SHA-1 of its canonical encoding, little-endian.
pub fn name_hash(name: &[u8]) -> Option<u32> {
    let canonical = canonical_name(name)?;
    let hash = digest(&SHA1_FOR_LEGACY_USE_ONLY, &canonical);
    let bytes = hash.as_ref();
    Some(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

/// `X509_subject_name_hash` of a DER certificate (`None` when it cannot be read).
pub fn subject_hash(certificate: &[u8]) -> Option<u32> {
    name_hash(subject_name(certificate)?)
}

// ── what OpenSSL reads from a certificate to decide whether it may end a chain ──────────────────────────────

/// The authority key identifier of a certificate (`AUTHORITY_KEYID`): each part when present.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthorityKey {
    pub key_id: Option<Vec<u8>>,
    /// Canonical encoding of the first directory name of `authorityCertIssuer`.
    pub issuer: Option<Vec<u8>>,
    pub serial: Option<Vec<u8>>,
}

/// The facts of a certificate that OpenSSL 3.0.13's `ossl_x509v3_cache_extensions`, `X509_check_akid`, `check_sig_alg_match`
/// and `ossl_x509_likely_issued` (`crypto/x509/v3_purp.c`) work from.
#[derive(Clone, Debug)]
pub struct Facts {
    pub serial: Vec<u8>,
    /// Object identifier (content bytes) of the signature algorithm named in the signed part.
    pub signature_algorithm: Vec<u8>,
    /// Object identifier (content bytes) of the subject public key algorithm.
    pub key_algorithm: Vec<u8>,
    /// Canonical encodings (see [`canonical_name`]) of the issuer and the subject name.
    pub issuer: Vec<u8>,
    pub subject: Vec<u8>,
    pub not_before: i64,
    pub not_after: i64,
    pub subject_key_id: Option<Vec<u8>>,
    pub authority_key: Option<AuthorityKey>,
}

const SUBJECT_KEY_ID_OID: &[u8] = &[0x55, 0x1D, 0x0E];
const AUTHORITY_KEY_ID_OID: &[u8] = &[0x55, 0x1D, 0x23];

/// Days since 1970-01-01 of a civil date (proleptic Gregorian).
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let year_of_era = year - era * 400;
    let day_of_year = (153 * (month + if month > 2 { -3 } else { 9 }) + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

/// Seconds since the epoch of an ASN.1 `UTCTime` / `GeneralizedTime` in the `...Z` form certificates use.
fn asn1_time(tag: u8, content: &[u8]) -> Option<i64> {
    let text = std::str::from_utf8(content).ok()?;
    let digits = text.strip_suffix('Z')?;
    if !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let (year, rest) = match tag {
        0x17 if digits.len() == 12 => {
            let two: i64 = digits[..2].parse().ok()?;
            (if two < 50 { 2000 + two } else { 1900 + two }, &digits[2..])
        }
        0x18 if digits.len() == 14 => (digits[..4].parse().ok()?, &digits[4..]),
        _ => return None,
    };
    let field = |at: usize| rest[at..at + 2].parse::<i64>().ok();
    let (month, day, hour, minute, second) =
        (field(0)?, field(2)?, field(4)?, field(6)?, field(8)?);
    if !(1..=12).contains(&month)
        || !(1..=31).contains(&day)
        || hour > 23
        || minute > 59
        || second > 60
    {
        return None;
    }
    Some(days_from_civil(year, month, day) * 86_400 + hour * 3600 + minute * 60 + second)
}

/// The authority key identifier extension value (`SEQUENCE { [0] keyIdentifier, [1] authorityCertIssuer, [2] serial }`).
fn authority_key(value: &[u8]) -> Option<AuthorityKey> {
    let (sequence, _) = read_tlv(value)?;
    if sequence.tag != 0x30 {
        return None;
    }
    let mut found = AuthorityKey {
        key_id: None,
        issuer: None,
        serial: None,
    };
    let mut rest = sequence.content;
    while !rest.is_empty() {
        let (element, after) = read_tlv(rest)?;
        rest = after;
        match element.tag {
            0x80 => found.key_id = Some(element.content.to_vec()),
            0x82 => found.serial = Some(element.content.to_vec()),
            0xA1 => {
                // `GeneralNames`: only the first directory name (`[4]`) is looked at.
                let mut names = element.content;
                while !names.is_empty() {
                    let (name, after) = read_tlv(names)?;
                    names = after;
                    if name.tag == 0xA4 {
                        found.issuer = Some(canonical_name(name.content)?);
                        break;
                    }
                }
            }
            _ => {}
        }
    }
    Some(found)
}

/// The facts of a DER certificate (`None` when it cannot be read as one).
pub fn facts(certificate: &[u8]) -> Option<Facts> {
    let (outer, _) = read_tlv(certificate)?;
    let (tbs, _) = read_tlv(outer.content)?;
    let mut rest = tbs.content;
    let (mut element, mut after) = read_tlv(rest)?;
    if element.tag == 0xA0 {
        rest = after;
        (element, after) = read_tlv(rest)?;
    }
    let serial = element;
    rest = after;
    let (signature, after) = read_tlv(rest)?;
    let (issuer, after) = read_tlv(after)?;
    let (validity, after) = read_tlv(after)?;
    let (subject, after) = read_tlv(after)?;
    let (key_info, after) = read_tlv(after)?;
    rest = after;
    let (algorithm_object, _) = read_tlv(signature.content)?;
    let (key_algorithm, _) = read_tlv(read_tlv(key_info.content)?.0.content)?;
    let (not_before, after_time) = read_tlv(validity.content)?;
    let (not_after, _) = read_tlv(after_time)?;
    let mut facts = Facts {
        serial: serial.content.to_vec(),
        signature_algorithm: algorithm_object.content.to_vec(),
        key_algorithm: key_algorithm.content.to_vec(),
        issuer: canonical_name(issuer.whole)?,
        subject: canonical_name(subject.whole)?,
        not_before: asn1_time(not_before.tag, not_before.content)?,
        not_after: asn1_time(not_after.tag, not_after.content)?,
        subject_key_id: None,
        authority_key: None,
    };
    // Extensions are the `[3]` element after the optional unique identifiers.
    while !rest.is_empty() {
        let (element, after) = read_tlv(rest)?;
        rest = after;
        if element.tag != 0xA3 {
            continue;
        }
        let (extensions, _) = read_tlv(element.content)?;
        let mut list = extensions.content;
        while !list.is_empty() {
            let (extension, after) = read_tlv(list)?;
            list = after;
            let (object, mut tail) = read_tlv(extension.content)?;
            let (mut next, mut after_next) = read_tlv(tail)?;
            if next.tag == 0x01 {
                tail = after_next;
                (next, after_next) = read_tlv(tail)?;
            }
            let _ = after_next;
            if next.tag != 0x04 {
                continue;
            }
            if object.content == SUBJECT_KEY_ID_OID {
                let (inner, _) = read_tlv(next.content)?;
                facts.subject_key_id = Some(inner.content.to_vec());
            } else if object.content == AUTHORITY_KEY_ID_OID {
                facts.authority_key = authority_key(next.content);
            }
        }
    }
    Some(facts)
}

/// The key family a public key algorithm or a signature algorithm belongs to, for the match `check_sig_alg_match` demands.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Family {
    Ec,
    Rsa,
    Ed25519,
    Ed448,
}

fn key_family(oid: &[u8]) -> Option<Family> {
    match oid {
        [0x2A, 0x86, 0x48, 0xCE, 0x3D, 0x02, 0x01] => Some(Family::Ec),
        [0x2A, 0x86, 0x48, 0x86, 0xF7, 0x0D, 0x01, 0x01, 0x01] => Some(Family::Rsa),
        [0x2B, 0x65, 0x70] => Some(Family::Ed25519),
        [0x2B, 0x65, 0x71] => Some(Family::Ed448),
        _ => None,
    }
}

fn signature_family(oid: &[u8]) -> Option<Family> {
    match oid {
        // ecdsa-with-SHA1, ecdsa-with-SHA224/256/384/512
        [0x2A, 0x86, 0x48, 0xCE, 0x3D, 0x04, 0x01] => Some(Family::Ec),
        [0x2A, 0x86, 0x48, 0xCE, 0x3D, 0x04, 0x03, 0x01..=0x04] => Some(Family::Ec),
        // md5/sha1/sha256/sha384/sha512/sha224 WithRSAEncryption and RSASSA-PSS (`RSA` keys sign with PSS too)
        [0x2A, 0x86, 0x48, 0x86, 0xF7, 0x0D, 0x01, 0x01, 0x04 | 0x05 | 0x0A | 0x0B | 0x0C | 0x0D | 0x0E] => {
            Some(Family::Rsa)
        }
        [0x2B, 0x65, 0x70] => Some(Family::Ed25519),
        [0x2B, 0x65, 0x71] => Some(Family::Ed448),
        _ => None,
    }
}

/// `check_sig_alg_match(issuer_key, subject)`: the signature algorithm the subject names suits the issuer's key.
fn signature_matches_key(issuer: &Facts, subject: &Facts) -> bool {
    match (
        key_family(&issuer.key_algorithm),
        signature_family(&subject.signature_algorithm),
    ) {
        (Some(key), Some(signature)) => key == signature,
        _ => false,
    }
}

/// `X509_check_akid(issuer, akid)`: the authority key identifier of a subject (when it has one) agrees with the issuer's own
/// subject key identifier, serial number and issuer name.
fn authority_key_agrees(issuer: &Facts, akid: Option<&AuthorityKey>) -> bool {
    let Some(akid) = akid else {
        return true;
    };
    if let (Some(wanted), Some(held)) = (&akid.key_id, &issuer.subject_key_id) {
        if wanted != held {
            return false;
        }
    }
    if let Some(serial) = &akid.serial {
        if *serial != issuer.serial {
            return false;
        }
    }
    if let Some(name) = &akid.issuer {
        if *name != issuer.issuer {
            return false;
        }
    }
    true
}

/// `EXFLAG_SS`: the subject and issuer names are the same, any authority key identifier agrees with the certificate's own
/// identifiers and the signature algorithm suits the certificate's own key. This is what makes a certificate of the store an
/// OpenSSL chain terminator when the trust attributes of the certificate say nothing (`trust_compat` in `x509_trs.c`); the
/// signature itself is not verified (`X509_V_FLAG_CHECK_SS_SIGNATURE` is off by default). Explicit trust attributes of a
/// `TRUSTED CERTIFICATE` are not read (see `PORTING.md`).
pub fn is_self_signed(certificate: &Facts) -> bool {
    certificate.subject == certificate.issuer
        && authority_key_agrees(certificate, certificate.authority_key.as_ref())
        && signature_matches_key(certificate, certificate)
}

/// `ossl_x509_likely_issued(issuer, subject)`: the names fit, the subject's authority key identifier agrees with the issuer
/// and the subject's signature algorithm suits the issuer's key.
pub fn likely_issued(issuer: &Facts, subject: &Facts) -> bool {
    issuer.subject == subject.issuer
        && authority_key_agrees(issuer, subject.authority_key.as_ref())
        && signature_matches_key(issuer, subject)
}

/// A certificate of the store with what is known about it.
#[derive(Clone, Debug)]
pub struct StoreCertificate {
    pub der: CertificateDer<'static>,
    pub facts: Facts,
}

/// The certificates of the chosen sources, split by what OpenSSL lets them do with the default settings of CPython 3.12
/// (`VERIFY_X509_TRUSTED_FIRST`, no `VERIFY_X509_PARTIAL_CHAIN`): `anchors` (self-signed, [`is_self_signed`]) may end a chain;
/// `others` may only be passed through while a chain is built, and never end it.
#[derive(Clone, Debug, Default)]
pub struct Store {
    pub anchors: Vec<StoreCertificate>,
    pub others: Vec<StoreCertificate>,
}

/// Splits certificates (the ones that cannot be read are dropped, as they cannot be loaded).
pub fn split_store(certificates: Vec<CertificateDer<'static>>) -> Store {
    let mut store = Store::default();
    for der in certificates {
        let Some(facts) = facts(der.as_ref()) else {
            continue;
        };
        if is_self_signed(&facts) {
            store.anchors.push(StoreCertificate { der, facts });
        } else {
            store.others.push(StoreCertificate { der, facts });
        }
    }
    store
}

// ── the directory lookup ────────────────────────────────────────────────────────────────────────

/// The hash a directory entry name stands for: eight hexadecimal digits (either case), a dot, an optional lower-case `r` and
/// one or more ASCII digits (what the reference interpreter's OpenSSL accepts in a hashed directory).
fn hash_of_name(name: &str) -> Option<u32> {
    let (stem, suffix) = name.split_once('.')?;
    if stem.len() != 8 || !stem.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    let digits = suffix.strip_prefix('r').unwrap_or(suffix);
    if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    u32::from_str_radix(stem, 16).ok()
}

/// Whether the file holds a certificate revocation list (read by OpenSSL as a CRL, which is not a failure to load).
fn holds_revocation_list(path: &Path) -> bool {
    std::fs::read(path).is_ok_and(|bytes| {
        bytes
            .windows(b"-----BEGIN X509 CRL-----".len())
            .any(|window| window == b"-----BEGIN X509 CRL-----")
    })
}

/// The certificates a hashed directory offers: for each hash named by an entry, its files in directory order,
/// stopping at the first that cannot be loaded; a certificate counts only when its own subject hash is the one in the name.
pub fn directory_certificates(directory: &Path) -> Vec<CertificateDer<'static>> {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return Vec::new();
    };
    let mut groups: Vec<(u32, Vec<PathBuf>)> = Vec::new();
    for entry in entries.filter_map(Result::ok) {
        let name = entry.file_name();
        let Some(hash) = name.to_str().and_then(hash_of_name) else {
            continue;
        };
        match groups.iter_mut().find(|(held, _)| *held == hash) {
            Some((_, paths)) => paths.push(entry.path()),
            None => groups.push((hash, vec![entry.path()])),
        }
    }
    let mut anchors = Vec::new();
    for (hash, paths) in groups {
        for path in paths {
            if std::fs::metadata(&path).is_err() {
                break;
            }
            let loaded = load_certs_from_paths(Some(&path), None).certs;
            if loaded.is_empty() {
                if holds_revocation_list(&path) {
                    continue;
                }
                break;
            }
            for der in &loaded {
                if subject_hash(der.as_ref()) == Some(hash) {
                    anchors.push(der.clone().into_owned());
                }
            }
        }
    }
    anchors
}

// ── the choice of the two sources ───────────────────────────────────────────────────────────────

/// What of the environment and of the host's defaults the choice needs, as values.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Environment {
    /// `SSL_CERT_FILE`: `None` when unset, `Some("")` when set to the empty string.
    pub ssl_cert_file: Option<std::ffi::OsString>,
    /// `SSL_CERT_DIR`: `None` when unset, `Some("")` when set to the empty string.
    pub ssl_cert_dir: Option<std::ffi::OsString>,
    /// The host's default CA file and directories, used for a side no variable overrides.
    pub default_file: Option<PathBuf>,
    pub default_directories: Vec<PathBuf>,
}

impl Environment {
    /// This process's environment and this host's default store (found only for a side no variable overrides).
    pub fn from_process() -> Environment {
        let ssl_cert_file = std::env::var_os("SSL_CERT_FILE");
        let ssl_cert_dir = std::env::var_os("SSL_CERT_DIR");
        Environment {
            default_file: if ssl_cert_file.is_some() {
                None
            } else {
                openssl_probe::probe().cert_file
            },
            default_directories: if ssl_cert_dir.is_some() {
                Vec::new()
            } else {
                openssl_probe::candidate_cert_dirs()
                    .map(PathBuf::from)
                    .collect()
            },
            ssl_cert_file,
            ssl_cert_dir,
        }
    }
}

/// The CA file and directories in force.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Sources {
    pub file: Option<PathBuf>,
    pub directories: Vec<PathBuf>,
}

/// Each side takes its variable when that is set (an empty value means none; the directory list drops empty entries and
/// repeated names) and the host's default only when it is unset.
pub fn select_sources(environment: &Environment) -> Sources {
    let file = match &environment.ssl_cert_file {
        Some(path) if path.is_empty() => None,
        Some(path) => Some(PathBuf::from(path)),
        None => environment.default_file.clone(),
    };
    let mut directories: Vec<PathBuf> = match &environment.ssl_cert_dir {
        Some(list) => std::env::split_paths(list)
            .filter(|entry| !entry.as_os_str().is_empty())
            .collect(),
        None => environment.default_directories.clone(),
    };
    let mut seen: Vec<PathBuf> = Vec::new();
    directories.retain(|directory| {
        if seen.contains(directory) {
            false
        } else {
            seen.push(directory.clone());
            true
        }
    });
    Sources { file, directories }
}

/// The certificates of the chosen sources: the file's own, then the directories' (those whose subject hash is the one in
/// their file name). A location that cannot be read contributes nothing.
pub fn load_store(sources: &Sources) -> Vec<CertificateDer<'static>> {
    let mut certificates: Vec<CertificateDer<'static>> = Vec::new();
    if let Some(file) = sources.file.as_deref() {
        certificates.extend(load_certs_from_paths(Some(file), None).certs);
    }
    for directory in &sources.directories {
        certificates.extend(directory_certificates(directory));
    }
    certificates
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn certificate_times_are_read_as_utc_seconds() {
        // Values from Python's `datetime`.
        assert_eq!(asn1_time(0x17, b"240101000000Z"), Some(1_704_067_200));
        assert_eq!(asn1_time(0x18, b"20400101000000Z"), Some(2_208_988_800));
        assert_eq!(asn1_time(0x17, b"491231235959Z"), Some(2_524_607_999));
        assert_eq!(asn1_time(0x17, b"500101000000Z"), Some(-631_152_000));
        assert_eq!(asn1_time(0x18, b"20200229120000Z"), Some(1_582_977_600));
        for refused in [
            &b"240101000000"[..],
            b"24010100000Z",
            b"241301000000Z",
            b"2401010000aaZ",
        ] {
            assert_eq!(asn1_time(0x17, refused), None, "{refused:?}");
        }
        assert_eq!(asn1_time(0x18, b"240101000000Z"), None);
    }

    #[test]
    fn the_der_reader_handles_short_and_long_lengths() {
        let long = encode_tlv(0x04, &[7u8; 300]);
        assert_eq!(&long[..4], &[0x04, 0x82, 0x01, 0x2C]);
        let (element, rest) = read_tlv(&long).expect("an element");
        assert_eq!(
            (element.tag, element.content.len(), rest.len()),
            (0x04, 300, 0)
        );
        assert!(read_tlv(&long[..10]).is_none());
        assert!(read_tlv(&[0x30, 0x80, 0x00, 0x00]).is_none());
    }

    #[test]
    fn strings_are_canonicalised_like_openssl() {
        let canonical = |tag: u8, content: &[u8]| {
            canonical_string(tag, content).map(|bytes| String::from_utf8(bytes).expect("UTF-8"))
        };
        assert_eq!(
            canonical(PRINTABLE_STRING, b"  Local   FIXTURE\t Root  ").as_deref(),
            Some("local fixture root")
        );
        assert_eq!(
            canonical(UTF8_STRING, "Ünï  Cödé".as_bytes()).as_deref(),
            Some("Ünï cödé")
        );
        assert_eq!(canonical(T61_STRING, b"caf\xe9").as_deref(), Some("café"));
        assert_eq!(
            canonical(BMP_STRING, &[0x00, 0x41, 0x00, 0x20, 0x00, 0x42]).as_deref(),
            Some("a b")
        );
        assert_eq!(
            canonical(UNIVERSAL_STRING, &[0, 0, 0, 0x41]).as_deref(),
            Some("a")
        );
        assert_eq!(canonical(UTF8_STRING, &[0xFF]), None);
        assert_eq!(canonical(BMP_STRING, &[0x00]), None);
        assert_eq!(canonical(PRINTABLE_STRING, b"   ").as_deref(), Some(""));
    }

    #[test]
    fn directory_entry_names_are_read_as_the_reference_openssl_reads_them() {
        for (name, hash) in [
            ("56c899cd.0", Some(0x56c8_99cd)),
            ("56C899CD.0", Some(0x56c8_99cd)),
            ("56C899cd.7", Some(0x56c8_99cd)),
            ("56c899cd.00", Some(0x56c8_99cd)),
            ("56c899cd.12345", Some(0x56c8_99cd)),
            ("56c899cd.r0", Some(0x56c8_99cd)),
        ] {
            assert_eq!(hash_of_name(name), hash, "{name}");
        }
        for name in [
            "56c899cd",
            "56c899cd.",
            "56c899cd.x",
            "56c899cd.0x",
            "56c899cd.pem",
            "56c899cd_0",
            "56c899c.0",
            "056c899cd.0",
            " 56c899cd.0",
            "56c899cd.0 ",
            "56c899cd.\u{663}",
            "56c899cd.1.0",
            "56c899cd.r",
            "56c899cd.R0",
            "ca.pem",
        ] {
            assert_eq!(hash_of_name(name), None, "{name}");
        }
    }

    #[test]
    fn sources_are_chosen_side_by_side() {
        let environment = |file: Option<&str>, directory: Option<&str>| Environment {
            ssl_cert_file: file.map(Into::into),
            ssl_cert_dir: directory.map(Into::into),
            default_file: Some(PathBuf::from("/default/bundle.crt")),
            default_directories: vec![PathBuf::from("/default/certs")],
        };
        let chosen = select_sources(&environment(None, None));
        assert_eq!(chosen.file, Some(PathBuf::from("/default/bundle.crt")));
        assert_eq!(chosen.directories, [PathBuf::from("/default/certs")]);
        let chosen = select_sources(&environment(Some("/my/ca.pem"), None));
        assert_eq!(chosen.file, Some(PathBuf::from("/my/ca.pem")));
        assert_eq!(chosen.directories, [PathBuf::from("/default/certs")]);
        let chosen = select_sources(&environment(Some(""), Some("/a::/b:/a")));
        assert_eq!(chosen.file, None);
        assert_eq!(
            chosen.directories,
            [PathBuf::from("/a"), PathBuf::from("/b")]
        );
        assert_eq!(
            select_sources(&environment(None, Some(""))).directories,
            Vec::<PathBuf>::new()
        );
    }
}
