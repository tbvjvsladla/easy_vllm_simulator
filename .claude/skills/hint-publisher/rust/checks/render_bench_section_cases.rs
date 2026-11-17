//! `render_bench_section` checks (owner: the render_bench_section port). Byte parity with the original Python is the
//! parity stage `hp_rbs` (goldens under `.claude/rust/parity/data/HP-rbs-*`); these checks never run Python.
use hintlib::render_bench_section as r;
use hintlib::value::J;
use hintlib::{jd, jl};

fn full_report() -> String {
    [
        "# 성능 보고서",
        "",
        r::MEASUREMENT_CONFIG_TITLE,
        "",
        "| 키 | 값 |",
        "|---|---|",
        "| bench_mode | full |",
        "| bench_tool | guidellm |",
        "| repeats | 3 |",
        "| repeats_completed | N/A |",
        "| dataset_column_map | a \\| b |",
        "",
        "## lite 지표 (full ⊇ lite)",
        "",
        "| 메트릭 | Main | Sub |",
        "|---|---|---|",
        "| gen tokens/sec (warm) [master] | 18.81 t/s | — |",
        "",
        "## 부하 스윕 곡선 (client-load · reload 없음)",
        "",
        r::REPORT_HEADER,
        "|---|---|---|---|---|---|---|",
        "| 1 ★판정점 | 22.26 | 22.115934004263103 | 113.17 | 750.48 | 42.13 | 16/0 |",
        "| 2 | 40.5 | 0.125 | N/A | 0.375 | - | 32/0 |",
        "",
        "**⚠ 절삭된 부하 레벨(적응 상한 클램프):**",
        "- level 4 truncated",
        "",
        "## 다음",
        "",
    ]
    .join("\n")
}

#[test]
fn selftest_passes() {
    assert_eq!(r::selftest(), Vec::<String>::new());
}

#[test]
fn report_parse_and_render() {
    let text = full_report();
    let p = r::parse_report(&text).unwrap();
    let lv = p["levels"].items();
    assert_eq!(lv.len(), 2);
    assert_eq!(lv[0]["level"], J::Int(1));
    assert_eq!(lv[0]["verdict_point"], J::Bool(true));
    assert_eq!(lv[1]["total_token_throughput"], J::Null);
    assert_eq!(p["truncated"], jl!["level 4 truncated"]);
    let body = r::render(&p, "bench_report_x.md").unwrap();
    assert!(body.starts_with(r::SECTION_TITLE));
    assert!(body.contains("| 1 ★판정점 | 22.26 | 22.12 | 113.17 | 750.48 | 42.13 | 16/0 |"));
    // half-even display rounding of the exact binary value (Python round + .2f)
    assert!(body.contains("| 2 | 40.50 | 0.12 | N/A | 0.38 | N/A | 32/0 |"));
    assert!(body.contains("- level 4 truncated"));
    let doc = format!("앞\n\n{body}\n## 뒤\n");
    assert_eq!(r::extract_section(&doc, r::SECTION_TITLE).unwrap().unwrap().trim(), body.trim());
    assert!(r::extract_section("없음\n", r::SECTION_TITLE).unwrap().is_none());
}

#[test]
fn measurement_config_and_lite_predicates() {
    let text = full_report();
    let mc = r::parse_measurement_config(&text).unwrap().unwrap();
    assert_eq!(mc, jd! {"bench_mode" => "full", "bench_tool" => "guidellm", "repeats" => 3, "repeats_completed" => J::Null,
        "dataset_column_map" => "a | b"});
    assert!(r::parse_measurement_config("# 없음\n").unwrap().is_none());
    let bad = r::parse_measurement_config(&text.replace("| repeats | 3 |", "| surprise | 3 |")).unwrap_err();
    assert_eq!(bad.code, "py:BenchSectionFailure");
    assert!(r::lite_metrics_present(&text).unwrap());
    assert!(!r::lite_metrics_present(&text.replace("18.81 t/s", "N/A")).unwrap());
    assert!(!r::is_lite_report(&text).unwrap());
    assert!(r::is_lite_report("x\n  mode: lite \n").unwrap());
    assert!(r::parse_report("# 표 없음\n").is_err());
}

#[test]
fn sweep_index_tail_columns() {
    let idx = jd! {"verdict_point_level" => 2, "levels" => jl![
        jd!{"level" => 1, "measured" => jd!{"decode_tps" => 10.0, "ttft_ms_p99" => 5.125, "completed" => 4, "failed" => 0}},
        jd!{"level" => 2.0, "measured" => J::Null}],
        "truncated" => jl![]};
    let p = r::parse_sweep_index(&idx).unwrap();
    assert_eq!(p["levels"].items()[1]["verdict_point"], J::Bool(true));
    let body = r::render(&p, "sweep_index.json").unwrap();
    assert!(body.contains("TTFT p99"));
    assert!(body.contains("| 1 | 10 | N/A | N/A | N/A | 5.12 | N/A | N/A | 4/0 |"));
    assert!(body.contains("| 2.0 ★판정점 |"));
    assert!(body.contains("_절삭된 레벨 없음(요청 전 레벨 완주)._"));
    assert!(r::parse_sweep_index(&jd! {"levels" => jl![]}).is_err());
}

#[test]
fn fmt_and_num() {
    assert_eq!(r::_fmt(&J::Null, 2).unwrap(), "N/A");
    assert_eq!(r::_fmt(&J::Float(16.0), 2).unwrap(), "16");
    assert_eq!(r::_fmt(&J::Float(2.675), 2).unwrap(), "2.67");
    assert_eq!(r::_fmt(&J::Str("12.345".into()), 2).unwrap(), "12.35");
    assert_eq!(r::_fmt(&J::Str("x".into()), 2).unwrap_err().code, "py:ValueError");
    assert_eq!(r::_num(" N/A ").unwrap(), J::Null);
    assert_eq!(r::_num("1_000").unwrap(), J::Float(1000.0));
    assert_eq!(r::_num("1__0").unwrap(), J::Null);
}

#[test]
fn cli_verify_round_trip() {
    let dir = std::env::temp_dir().join(format!("rbs_check_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let rep = dir.join("bench_report_x.md");
    std::fs::write(&rep, full_report()).unwrap();
    let out = dir.join("sec.md");
    let s = |p: &std::path::Path| p.to_string_lossy().into_owned();
    let (code, _, _) = hintlib::io::capture(|| r::main(&["--report".into(), s(&rep), "--out".into(), s(&out)]));
    assert_eq!(code, 0);
    let (code, o, _) = hintlib::io::capture(|| r::main(&["--verify".into(), "--section".into(), s(&out), "--report".into(), s(&rep)]));
    assert_eq!(code, 0);
    assert!(o.contains("VERIFY PASS diff=0 (bench_report_x.md)"));
    let tampered = std::fs::read_to_string(&out).unwrap().replace("22.26", "99.99");
    std::fs::write(&out, tampered).unwrap();
    let (code, _, e) = hintlib::io::capture(|| r::main(&["--verify".into(), "--section".into(), s(&out), "--report".into(), s(&rep)]));
    assert_eq!(code, 1);
    assert!(e.contains("-| 1 ★판정점 | 22.26 |") && e.contains("+| 1 ★판정점 | 99.99 |"));
    let (code, _, e) = hintlib::io::capture(|| r::main(&["--verify".into(), "--all".into()]));
    assert_eq!(code, 2);
    assert!(e.ends_with("render_bench_section.sh: error: unrecognized arguments: --all\n"));
    let _ = std::fs::remove_dir_all(&dir);
}
