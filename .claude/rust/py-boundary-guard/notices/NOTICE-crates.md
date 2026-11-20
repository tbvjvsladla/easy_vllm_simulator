# 외부 크레이트 고지 (Third-party crates)

이 표는 하네스 Rust 크레이트들의 추적 `Cargo.lock` 이 고정한 crates.io 패키지를 적는다(plan 26111805 D2·D9, cell c20). 각 패키지는
내려받은 Cargo 패키지로 링크·빌드되며, 이 저장소에 그 소스를 벤더링하지 않는다. 라이선스 원문과 저작권 고지는 각 패키지의 배포본
(`<cargo home>/registry/src/<index>/<crate>-<version>/`의 LICENSE 파일)에 있다. 이 고지는 프로젝트 자체 코드의 라이선스를 정하지 않는다.

- 검사: `bash .claude/rust/py-boundary-guard/crate_allow.sh check`는 모든 추적 `Cargo.lock`의 외부 패키지가 이 표에 같은 판·`license`로
  있는지 본다. `license`는 패키지 manifest의 SPDX 식 그대로다.
- 패키지를 처음 들이는 cell은 `Cargo.lock`을 바꾸는 같은 커밋에서 이 표에 행을 더한다. 승인된 허용 목록
  (`crate_allowlist.tsv`)에 있으나 아직 어느 lock에도 없는 판은 여기 적지 않는다. 허용 목록 머리 주석도 같은 규칙이다.
- 검사기는 `crate`·`version`·`license` 칸만 읽는다. `used by` 칸은 이 표를 쓴 시점(h9, base `ead7087d`)의 값이며 유지 대상이 아니다.
- 크레이트 일부를 옮겨 적은 경우(소스 이식)의 고지는 각 크레이트의 `notices/NOTICE*.md`가 따로 맡는다.

| crate | version | license | used by |
|---|---|---|---|
| adler2 | 2.0.1 | 0BSD OR MIT OR Apache-2.0 | policy-audit, policy-predicates, policy-selftest, terraforming, upstream-runtime |
| aho-corasick | 1.1.5 | Unlicense OR MIT | adversarial-benchmark, hint-publisher, policy-audit, policy-distribution, policy-predicates, policy-selftest, upstream-runtime |
| autocfg | 1.5.1 | Apache-2.0 OR MIT | adversarial-benchmark, hint-publisher, policy-audit, policy-distribution, policy-gate, policy-predicates, policy-publisher, policy-selftest, recipe-runtime, role-contract, runtime-support, terraforming, upstream-runtime |
| base64 | 0.23.1 | MIT OR Apache-2.0 | adversarial-benchmark, policy-audit, policy-predicates, policy-selftest, recipe-runtime, upstream-runtime |
| bit-set | 0.8.0 | Apache-2.0 OR MIT | adversarial-benchmark, hint-publisher, policy-audit, policy-distribution, policy-predicates, policy-selftest, upstream-runtime |
| bit-vec | 0.8.0 | Apache-2.0 OR MIT | adversarial-benchmark, hint-publisher, policy-audit, policy-distribution, policy-predicates, policy-selftest, upstream-runtime |
| bitflags | 2.13.2 | MIT OR Apache-2.0 | adversarial-benchmark, policy-audit, policy-predicates, policy-selftest, recipe-runtime, upstream-runtime |
| block-buffer | 0.10.4 | MIT OR Apache-2.0 | adversarial-benchmark, policy-audit, policy-gate, policy-predicates, policy-publisher, policy-selftest, terraforming |
| bytes | 1.12.1 | MIT | adversarial-benchmark, policy-audit, policy-predicates, policy-selftest, recipe-runtime, upstream-runtime |
| cc | 1.11.0 | MIT OR Apache-2.0 | adversarial-benchmark, hint-publisher, policy-audit, policy-distribution, policy-predicates, policy-selftest, recipe-runtime, upstream-runtime |
| cfg-if | 1.0.5 | MIT OR Apache-2.0 | adversarial-benchmark, hint-publisher, policy-audit, policy-distribution, policy-gate, policy-predicates, policy-publisher, policy-selftest, recipe-runtime, terraforming, upstream-runtime |
| core-foundation | 0.10.1 | MIT OR Apache-2.0 | adversarial-benchmark, policy-audit, policy-predicates, policy-selftest, recipe-runtime, upstream-runtime |
| core-foundation-sys | 0.8.7 | MIT OR Apache-2.0 | adversarial-benchmark, policy-audit, policy-predicates, policy-selftest, recipe-runtime, upstream-runtime |
| cpufeatures | 0.2.17 | MIT OR Apache-2.0 | adversarial-benchmark, policy-audit, policy-gate, policy-predicates, policy-publisher, policy-selftest, terraforming |
| crc32fast | 1.5.3 | MIT OR Apache-2.0 | policy-audit, policy-predicates, policy-selftest, terraforming, upstream-runtime |
| crypto-common | 0.1.7 | MIT OR Apache-2.0 | adversarial-benchmark, policy-audit, policy-gate, policy-predicates, policy-publisher, policy-selftest, terraforming |
| digest | 0.10.7 | MIT OR Apache-2.0 | adversarial-benchmark, policy-audit, policy-gate, policy-predicates, policy-publisher, policy-selftest, terraforming |
| errno | 0.3.14 | MIT OR Apache-2.0 | adversarial-benchmark, policy-predicates |
| fancy-regex | 0.19.2 | MIT | adversarial-benchmark, hint-publisher, policy-audit, policy-distribution, policy-predicates, policy-selftest, upstream-runtime |
| find-msvc-tools | 0.1.15 | MIT OR Apache-2.0 | adversarial-benchmark, hint-publisher, policy-audit, policy-distribution, policy-predicates, policy-selftest, recipe-runtime, upstream-runtime |
| generic-array | 0.14.7 | MIT | adversarial-benchmark, policy-audit, policy-gate, policy-predicates, policy-publisher, policy-selftest, terraforming |
| getrandom | 0.2.17 | MIT OR Apache-2.0 | adversarial-benchmark, hint-publisher, policy-audit, policy-distribution, policy-predicates, policy-selftest, recipe-runtime, upstream-runtime |
| http | 1.5.0 | MIT OR Apache-2.0 | adversarial-benchmark, policy-audit, policy-predicates, policy-selftest, recipe-runtime, upstream-runtime |
| httparse | 1.10.1 | MIT OR Apache-2.0 | adversarial-benchmark, policy-audit, policy-predicates, policy-selftest, recipe-runtime, upstream-runtime |
| itoa | 1.0.18 | MIT OR Apache-2.0 | adversarial-benchmark, policy-audit, policy-predicates, policy-selftest, recipe-runtime, upstream-runtime |
| libc | 0.2.191 | MIT OR Apache-2.0 | adversarial-benchmark, hint-publisher, policy-audit, policy-distribution, policy-gate, policy-predicates, policy-publisher, policy-selftest, recipe-runtime, terraforming, upstream-runtime |
| linux-raw-sys | 0.12.1 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT | adversarial-benchmark, policy-predicates |
| log | 0.4.35 | MIT OR Apache-2.0 | adversarial-benchmark, policy-audit, policy-predicates, policy-selftest, recipe-runtime, upstream-runtime |
| memchr | 2.8.3 | Unlicense OR MIT | adversarial-benchmark, hint-publisher, policy-audit, policy-distribution, policy-predicates, policy-selftest, upstream-runtime |
| miniz_oxide | 0.9.1 | MIT OR Zlib OR Apache-2.0 | policy-audit, policy-predicates, policy-selftest, terraforming, upstream-runtime |
| num-bigint | 0.4.8 | MIT OR Apache-2.0 | adversarial-benchmark, hint-publisher, policy-audit, policy-distribution, policy-gate, policy-predicates, policy-publisher, policy-selftest, recipe-runtime, role-contract, runtime-support, terraforming, upstream-runtime |
| num-integer | 0.1.47 | MIT OR Apache-2.0 | adversarial-benchmark, hint-publisher, policy-audit, policy-distribution, policy-gate, policy-predicates, policy-publisher, policy-selftest, recipe-runtime, role-contract, runtime-support, terraforming, upstream-runtime |
| num-traits | 0.2.19 | MIT OR Apache-2.0 | adversarial-benchmark, hint-publisher, policy-audit, policy-distribution, policy-gate, policy-predicates, policy-publisher, policy-selftest, recipe-runtime, role-contract, runtime-support, terraforming, upstream-runtime |
| once_cell | 1.21.4 | MIT OR Apache-2.0 | adversarial-benchmark, policy-audit, policy-predicates, policy-selftest, recipe-runtime, upstream-runtime |
| openssl-probe | 0.2.2 | MIT OR Apache-2.0 | adversarial-benchmark, policy-audit, policy-predicates, policy-selftest, recipe-runtime, upstream-runtime |
| percent-encoding | 2.3.2 | MIT OR Apache-2.0 | adversarial-benchmark, policy-audit, policy-predicates, policy-selftest, recipe-runtime, upstream-runtime |
| proc-macro2 | 1.0.107 | MIT OR Apache-2.0 | adversarial-benchmark, policy-predicates, py-boundary-guard |
| quote | 1.0.47 | MIT OR Apache-2.0 | adversarial-benchmark, policy-predicates, py-boundary-guard |
| regex-automata | 0.4.18 | MIT OR Apache-2.0 | adversarial-benchmark, hint-publisher, policy-audit, policy-distribution, policy-predicates, policy-selftest, upstream-runtime |
| regex-syntax | 0.8.11 | MIT OR Apache-2.0 | adversarial-benchmark, hint-publisher, policy-audit, policy-distribution, policy-predicates, policy-selftest, upstream-runtime |
| ring | 0.17.14 | Apache-2.0 AND ISC | adversarial-benchmark, hint-publisher, policy-audit, policy-distribution, policy-predicates, policy-selftest, recipe-runtime, upstream-runtime |
| rustix | 1.1.6 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT | adversarial-benchmark, policy-predicates |
| rustls | 0.23.46 | Apache-2.0 OR ISC OR MIT | adversarial-benchmark, policy-audit, policy-predicates, policy-selftest, recipe-runtime, upstream-runtime |
| rustls-native-certs | 0.8.5 | Apache-2.0 OR ISC OR MIT | adversarial-benchmark, policy-audit, policy-predicates, policy-selftest, recipe-runtime, upstream-runtime |
| rustls-pki-types | 1.15.1 | MIT OR Apache-2.0 | adversarial-benchmark, policy-audit, policy-predicates, policy-selftest, recipe-runtime, upstream-runtime |
| rustls-webpki | 0.103.16 | ISC | adversarial-benchmark, policy-audit, policy-predicates, policy-selftest, recipe-runtime, upstream-runtime |
| schannel | 0.1.30 | MIT | adversarial-benchmark, policy-audit, policy-predicates, policy-selftest, recipe-runtime, upstream-runtime |
| security-framework | 3.7.0 | MIT OR Apache-2.0 | adversarial-benchmark, policy-audit, policy-predicates, policy-selftest, recipe-runtime, upstream-runtime |
| security-framework-sys | 2.17.0 | MIT OR Apache-2.0 | adversarial-benchmark, policy-audit, policy-predicates, policy-selftest, recipe-runtime, upstream-runtime |
| serde | 1.0.229 | MIT OR Apache-2.0 | adversarial-benchmark, policy-predicates |
| serde_core | 1.0.229 | MIT OR Apache-2.0 | adversarial-benchmark, policy-predicates |
| serde_derive | 1.0.229 | MIT OR Apache-2.0 | adversarial-benchmark, policy-predicates |
| serde_json | 1.0.154 | MIT OR Apache-2.0 | adversarial-benchmark, policy-predicates |
| sha2 | 0.10.9 | MIT OR Apache-2.0 | adversarial-benchmark, policy-audit, policy-gate, policy-predicates, policy-publisher, policy-selftest, terraforming |
| shlex | 2.0.1 | MIT OR Apache-2.0 | adversarial-benchmark, hint-publisher, policy-audit, policy-distribution, policy-predicates, policy-selftest, recipe-runtime, upstream-runtime |
| subtle | 2.6.1 | BSD-3-Clause | adversarial-benchmark, policy-audit, policy-predicates, policy-selftest, recipe-runtime, upstream-runtime |
| syn | 3.0.9 | MIT OR Apache-2.0 | adversarial-benchmark, policy-predicates, py-boundary-guard |
| typenum | 1.20.1 | MIT OR Apache-2.0 | adversarial-benchmark, policy-audit, policy-gate, policy-predicates, policy-publisher, policy-selftest, terraforming |
| unicode-ident | 1.0.27 | (MIT OR Apache-2.0) AND Unicode-3.0 | adversarial-benchmark, policy-predicates, py-boundary-guard |
| untrusted | 0.9.0 | ISC | adversarial-benchmark, hint-publisher, policy-audit, policy-distribution, policy-predicates, policy-selftest, recipe-runtime, upstream-runtime |
| ureq | 3.5.0 | MIT OR Apache-2.0 | adversarial-benchmark, policy-audit, policy-predicates, policy-selftest, recipe-runtime, upstream-runtime |
| ureq-proto | 0.6.5 | MIT OR Apache-2.0 | adversarial-benchmark, policy-audit, policy-predicates, policy-selftest, recipe-runtime, upstream-runtime |
| utf8-zero | 0.8.1 | MIT OR Apache-2.0 | adversarial-benchmark, policy-audit, policy-predicates, policy-selftest, recipe-runtime, upstream-runtime |
| version_check | 0.9.5 | MIT/Apache-2.0 | adversarial-benchmark, policy-audit, policy-gate, policy-predicates, policy-publisher, policy-selftest, terraforming |
| wasi | 0.11.1+wasi-snapshot-preview1 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT | adversarial-benchmark, hint-publisher, policy-audit, policy-distribution, policy-predicates, policy-selftest, recipe-runtime, upstream-runtime |
| webpki-roots | 1.0.11 | CDLA-Permissive-2.0 | adversarial-benchmark, policy-audit, policy-predicates, policy-selftest, recipe-runtime, upstream-runtime |
| windows-link | 0.2.1 | MIT OR Apache-2.0 | adversarial-benchmark, policy-audit, policy-predicates, policy-selftest, recipe-runtime, upstream-runtime |
| windows-sys | 0.52.0 | MIT OR Apache-2.0 | adversarial-benchmark, hint-publisher, policy-audit, policy-distribution, policy-predicates, policy-selftest, recipe-runtime, upstream-runtime |
| windows-sys | 0.61.2 | MIT OR Apache-2.0 | adversarial-benchmark, policy-audit, policy-predicates, policy-selftest, recipe-runtime, upstream-runtime |
| windows-targets | 0.52.6 | MIT OR Apache-2.0 | adversarial-benchmark, hint-publisher, policy-audit, policy-distribution, policy-predicates, policy-selftest, recipe-runtime, upstream-runtime |
| windows_aarch64_gnullvm | 0.52.6 | MIT OR Apache-2.0 | adversarial-benchmark, hint-publisher, policy-audit, policy-distribution, policy-predicates, policy-selftest, recipe-runtime, upstream-runtime |
| windows_aarch64_msvc | 0.52.6 | MIT OR Apache-2.0 | adversarial-benchmark, hint-publisher, policy-audit, policy-distribution, policy-predicates, policy-selftest, recipe-runtime, upstream-runtime |
| windows_i686_gnu | 0.52.6 | MIT OR Apache-2.0 | adversarial-benchmark, hint-publisher, policy-audit, policy-distribution, policy-predicates, policy-selftest, recipe-runtime, upstream-runtime |
| windows_i686_gnullvm | 0.52.6 | MIT OR Apache-2.0 | adversarial-benchmark, hint-publisher, policy-audit, policy-distribution, policy-predicates, policy-selftest, recipe-runtime, upstream-runtime |
| windows_i686_msvc | 0.52.6 | MIT OR Apache-2.0 | adversarial-benchmark, hint-publisher, policy-audit, policy-distribution, policy-predicates, policy-selftest, recipe-runtime, upstream-runtime |
| windows_x86_64_gnu | 0.52.6 | MIT OR Apache-2.0 | adversarial-benchmark, hint-publisher, policy-audit, policy-distribution, policy-predicates, policy-selftest, recipe-runtime, upstream-runtime |
| windows_x86_64_gnullvm | 0.52.6 | MIT OR Apache-2.0 | adversarial-benchmark, hint-publisher, policy-audit, policy-distribution, policy-predicates, policy-selftest, recipe-runtime, upstream-runtime |
| windows_x86_64_msvc | 0.52.6 | MIT OR Apache-2.0 | adversarial-benchmark, hint-publisher, policy-audit, policy-distribution, policy-predicates, policy-selftest, recipe-runtime, upstream-runtime |
| zeroize | 1.9.1 | Apache-2.0 OR MIT | adversarial-benchmark, policy-audit, policy-predicates, policy-selftest, recipe-runtime, upstream-runtime |
| zmij | 1.0.23 | MIT | adversarial-benchmark, policy-predicates |
