//! hintlib — hint-publisher in Rust (port of `scripts/hintlib/*.py`, `scripts/hint.py`, `scripts/render_bench_section.py`).
//!
//! Module layout mirrors the Python package one to one (same names, same function names in snake_case):
//!
//!     core       저장소 경로 · 실패 표현(HintError) · git · 주입 시각 · 결정론 직렬화 · 소유 CLI 실행
//!     naming     태그 이름 파생 · 어휘표 · 신/구 문법 파서
//!     pii        PII 스캐너 단일본(배포 4종 · 비배포 산문 3종) · 발췌 기계 치환
//!     evidence   셀 증거 수집 · 발행 자격 관측 · completion_gate · evidence_publisher 구동
//!     lineage    계보 파생(LINEAGE.json)
//!     artifacts  슬롯 발견 · 적용 판정 · 실행 평면 · 서브 레시피
//!     template   챕터별 기재 지시 템플릿 · 사실 블록 · 린터 · 발췌 무결성
//!     branch     hint 브랜치 배관 커밋
//!     tag        봉인 · 로컬 검증 · 선별 push
//!     catalog    원격 발행 태그 → hints/index.json + HINTS.md 파생
//!     render_bench_section  bench_report 판독기(scripts/render_bench_section.py)
//!     cli        단일 CLI(scripts/hint.py)
//!
//! Shared porting substrate: `value` (the dynamic `J` value + Python-exact json), `pyre` (Python `re`), `io` (capturable
//! stdout/stderr), `api` (the JSON-lines bridge the golden checks drive).
//!
//! No side effect at load time; the repository is always an argument. This crate never starts a Python interpreter.
pub mod io;
pub mod pyre;
pub mod value;

pub mod core;

pub mod artifacts;
pub mod branch;
pub mod catalog;
pub mod cli;
pub mod evidence;
pub mod lineage;
pub mod naming;
pub mod pii;
pub mod render_bench_section;
pub mod tag;
pub mod template;

pub mod api;
