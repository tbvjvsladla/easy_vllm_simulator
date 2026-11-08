# Notices

`src/tls_trust.rs` re-implements, in Rust, the subject-name hash and the hashed-directory lookup of OpenSSL 3.0.13
(`crypto/x509/x_name.c`: `x509_name_canon`, `asn1_string_canon`, `i2d_name_canon`; `crypto/x509/x509_cmp.c`: `X509_NAME_hash_ex`;
`crypto/x509/by_dir.c`: `get_cert_by_subject_ex`). The algorithms and their order of steps follow those sources; no OpenSSL code is linked or run.
OpenSSL is licensed under the Apache License, Version 2.0; its license text is in `OpenSSL-LICENSE.txt`.
The project's own license is not changed by this notice.

## R24 trust round (chain termination)

`src/tls_trust.rs` (the certificate facts, `is_self_signed`, `likely_issued`) and `src/tls_verify.rs` (the order in which a chain is walked to name
a refusal) are transcribed from OpenSSL 3.0.13: `crypto/x509/v3_purp.c` (`ossl_x509v3_cache_extensions`, `X509_check_akid`, `check_sig_alg_match`,
`ossl_x509_likely_issued`), `crypto/x509/x509_trs.c` (`trust_compat`) and `crypto/x509/x509_vfy.c` (`check_trust`, `build_chain`, `internal_verify`,
`find_issuer`). The files were read from the official repository tag `openssl-3.0.13` (`raw.githubusercontent.com/openssl/openssl/openssl-3.0.13/...`,
sha256 of the copies read: v3_purp.c 986f2db2..., x509_vfy.c 366177b2..., x509_trs.c d5558cd4..., x509_cmp.c c4963b31...; recorded in the worker directory); no OpenSSL
code is linked or run. OpenSSL is licensed under the Apache License, Version 2.0 (`OpenSSL-LICENSE.txt`).

`src/tls_verify.rs` also adapts the structure of `ureq` 3.5.0's `RustlsConnector` (`src/tls/rustls.rs`): it wraps a connected transport in a `rustls`
client connection, with this package's verifier in place of `ureq`'s root-store verifier. `ureq` is licensed under MIT OR Apache-2.0; the licenses are in
`ureq-LICENSE-MIT.txt` and `ureq-LICENSE-APACHE.txt`. Verification itself is the `rustls` / `rustls-webpki` implementation already in the lock file
(same versions, no new package).
