//! `lineage.selftest()` — 합성 docs 트리 + 임시 git 저장소(격리 · 라이브 문서·태그·캠페인 비의존). ★ = 음성대조.
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use super::*;
use crate::value::loads;

struct Ck(Vec<String>);

impl Ck {
    fn ck(&mut self, name: &str, cond: bool) {
        if !cond {
            self.0.push(format!("lineage: {name}"));
        }
    }
}

fn set(ev: &J, k: &str, v: J) -> J {
    let mut e = ev.clone();
    e.set(k, v);
    e
}

fn docs_of(lin: &J) -> BTreeMap<String, J> {
    lin["documents"].items().iter().map(|d| (pystr(&d["path"]), d.clone())).collect()
}

fn dget<'a>(docs: &'a BTreeMap<String, J>, p: &str) -> &'a J {
    docs.get(p).unwrap_or(&crate::value::NULL)
}

fn strs(v: &J) -> Vec<String> {
    v.items().iter().map(pystr).collect()
}

fn derive_kw(repo: &Path, ev: &J, base_slug: &str, publish_kst: &str, depth: i64, declared: &J) -> HR<J> {
    derive_j(
        repo,
        ev,
        base_slug,
        &J::from(publish_kst),
        &J::Int(depth),
        declared,
        &J::Null,
        &J::Null,
        &J::Null,
        &J::Null,
        &[],
        "PAYLOAD.identity.base_slug",
    )
}

fn selftest_tool_sources(c: &mut Ck, repo: &Path, ev: &J) {
    let sha = match core::git(repo, &["rev-parse", "HEAD"]) {
        Ok(p) => p.stdout.trim().to_string(),
        Err(e) => {
            c.ck(&format!("git rev-parse {}", e.code), false);
            return;
        }
    };
    let good = jd! {"path" => tool_snapshot_rel("run_bench.sh", &sha).unwrap_or_default(), "kind" => TOOL_SOURCE_KIND,
                    "origin" => format!("git:{sha}:tools/run_bench.sh")};
    let other = "f".repeat(40);
    let with = |k: &str, v: String| set(&good, k, J::Str(v));
    let mut no_origin = good.clone();
    no_origin.as_dict_mut().unwrap().remove("origin");
    let bads: Vec<(&str, J, &str)> = vec![
        ("★rev12 불일치", with("path", format!("{TOOL_SNAPSHOT_DIR}/run_bench.sh@{}", &other[..12])), "tool-source-mismatch"),
        ("★이름 불일치", with("path", format!("{TOOL_SNAPSHOT_DIR}/lite_bench.sh@{}", &sha[..12])), "tool-source-mismatch"),
        ("★origin 경로 탈출", with("origin", format!("git:{sha}:tools/../run_bench.sh")), "tool-source-origin-path"),
        ("★origin 부재", no_origin, "tool-source-shape"),
        ("★저장소 경로 모양(draft 밖)", with("path", "tools/run_bench.sh".into()), "tool-source-shape"),
        (
            "★git 이 들지 않는 경로",
            set(
                &with("origin", format!("git:{sha}:tools/none.sh")),
                "path",
                J::Str(format!("{TOOL_SNAPSHOT_DIR}/none.sh@{}", &sha[..12])),
            ),
            "git-object-absent",
        ),
    ];
    for (name, cand, want) in &bads {
        let got = _tool_candidate_problem(repo, cand).ok().flatten().unwrap_or_default();
        c.ck(&format!("{name} — 결손 사유 {want}"), got.split('(').next() == Some(*want));
    }
    c.ck("정상 후보는 결함 없음", matches!(_tool_candidate_problem(repo, &good), Ok(None)));
    let unv = _tool_candidate_problem(&repo.join("docs"), &good).ok().flatten().unwrap_or_else(|| "None".into());
    c.ck("★git 저장소가 아니면 검증 불가(후보 ✗)", unv.starts_with("tool-source-unverifiable"));
    let mut seeds = ev["lineage_seeds"].clone();
    if !seeds.is_dict() {
        seeds = J::dict();
    }
    let mut ecs: Vec<J> = seeds["evidence_candidates"].items().to_vec();
    ecs.push(good.clone());
    ecs.extend(bads.iter().map(|b| b.1.clone()));
    seeds.set("evidence_candidates", J::List(ecs));
    let lin = match derive_kw(repo, &set(ev, "lineage_seeds", seeds.clone()), "zeta2.5-flash", "26010800", DEFAULT_DEPTH, &J::list()) {
        Ok(l) => l,
        Err(e) => {
            c.ck(&format!("도구 후보 파생 {}", e.code), false);
            return;
        }
    };
    let cands: BTreeMap<String, J> = lin["evidence_candidates"].items().iter().map(|x| (pystr(&x["path"]), x.clone())).collect();
    let gp = pystr(&good["path"]);
    let row = cands.get(&gp).cloned().unwrap_or(J::dict());
    c.ck(
        "도구 스냅숏 후보 = draft 상대 경로 · kind · origin(저장소 실재 검사 ✗ · git 바이트 검사 ○)",
        row.get_str("kind") == Some(TOOL_SOURCE_KIND) && row["origin"] == good["origin"] && !repo.join(&gp).exists(),
    );
    let bad_paths = [
        format!("{TOOL_SNAPSHOT_DIR}/run_bench.sh@{}", &other[..12]),
        format!("{TOOL_SNAPSHOT_DIR}/lite_bench.sh@{}", &sha[..12]),
        format!("{TOOL_SNAPSHOT_DIR}/none.sh@{}", &sha[..12]),
    ];
    let nmiss = lin["seeds_missing"]
        .items()
        .iter()
        .filter(|m| {
            m.get_str("source") == Some("lineage_seeds:evidence_candidates")
                && m.get_str("reason").is_some_and(|r| r.starts_with("tool-source") || r.starts_with("git-object-absent"))
        })
        .count();
    c.ck(
        "★결함 후보는 후보가 아니고 결손으로 적힌다(같은 경로의 결함 후보가 정상 origin 을 덮지 않는다)",
        !bad_paths.iter().any(|p| cands.contains_key(p))
            && cands.get("tools/run_bench.sh").and_then(|x| x.get_str("kind")) != Some(TOOL_SOURCE_KIND)
            && nmiss == bads.len(),
    );
    c.ck(
        "발췌 출처 해소 — `<이름>@<rev12>` · 전체 draft 상대 경로",
        resolve_stem(&lin, &format!("run_bench.sh@{}", &sha[..12])).ok().flatten().as_deref() == Some(gp.as_str())
            && resolve_stem(&lin, &gp).ok().flatten().as_deref() == Some(gp.as_str()),
    );
    c.ck("★rev 가 다른 표기는 해소되지 않는다", matches!(resolve_stem(&lin, &format!("run_bench.sh@{}", &other[..12])), Ok(None)));
    match tool_snapshot_rel("../x", &sha) {
        Ok(_) => c.ck("★도구 스냅숏 이름 모양 거부", false),
        Err(e) => c.ck("★도구 스냅숏 이름 모양 거부", e.code == "HINT_LINEAGE_TOOL_SOURCE_SHAPE"),
    }
    let dup = set(&good, "origin", J::Str(format!("git:{sha}:tools/sub/run_bench.sh")));
    let seeds2 = set(&seeds, "evidence_candidates", J::List(vec![good.clone(), dup]));
    match derive_kw(repo, &set(ev, "lineage_seeds", seeds2), "zeta2.5-flash", "26010800", DEFAULT_DEPTH, &J::list()) {
        Ok(_) => c.ck("★같은 경로에 서로 다른 유효 origin 둘 = 차단", false),
        Err(e) => c.ck("★같은 경로에 서로 다른 유효 origin 둘 = 차단", e.code == "HINT_LINEAGE_TOOL_ORIGIN_CONFLICT"),
    }
}

pub fn selftest() -> Vec<String> {
    let mut c = Ck(Vec::new());
    let own = own_checkout();
    let td = core::TempDir::new("hint_lineage_selftest_");
    let repo: PathBuf = core::resolve(td.path());
    let repo = repo.as_path();
    // The naming grammar sources are the TS prose SSOT and the Rust `bench_naming.rs` (plan_26111615 D4 · no `.py`).
    let bench_rs = super::REL_BENCH_NAMING_RS;
    for rel_path in [core::REL_DOC_NAMING_WIKI, core::REL_DOC_NAMING_TS, bench_rs] {
        let dst = repo.join(rel_path);
        let src = own.join(rel_path);
        let _ = std::fs::create_dir_all(dst.parent().unwrap());
        let optional = rel_path == core::REL_DOC_NAMING_WIKI;
        if std::fs::copy(&src, &dst).is_err() && !optional {
            c.ck(&format!("명명 SSOT 원천 복사 실패: {rel_path}"), false);
            return c.0;
        }
    }
    let w = |rel_path: &str, text: &str| {
        let p = repo.join(rel_path);
        let _ = std::fs::create_dir_all(p.parent().unwrap());
        let _ = std::fs::write(&p, text);
    };
    let dumps = |v: &J| crate::value::dumps_unicode(v);
    let dumps_ascii = |v: &J| crate::value::dumps_default(v);
    let s = "Zeta2.5-Flash";
    w("docs/plan/plan_26010001_zeta_zero.md", &format!("# {s} 영점\n`docs/plan/plan_26010000_zeta_origin.md` 를 잇는다.\n"));
    w("docs/plan/plan_26010000_zeta_origin.md", &format!("# {s} 기원 — 깊이 5 라 잘려야 한다\n"));
    w("docs/plan/plan_26010100_zeta_first.md", &format!("# {s} 첫 계획\n선행 `plan_26010001_zeta_zero.md`.\n"));
    w("docs/plan/plan_26010115_alpha.md", &format!("# {s} alpha\n"));
    w("docs/plan/plan_26010115_beta.md", &format!("# {s} beta\n"));
    w("docs/plan/plan_26010115_gamma_delta.md", &format!("# {s} gamma delta\n"));
    w(
        "docs/testlog/testlog_26010200_zeta_smoke.md",
        &format!(
            "# testlog\n\n> **SUPERSEDED-IN-PART** — `docs/testlog/testlog_26010600_zeta_verdict.md` §2 ·\n\
             > `docs/report/perf_26010400_zeta_종합.md` (2026-01-06)\n\n\
             {s}-NVFP4 스모크. 계획 `plan_26010100_zeta_first.md` · 형제 `plan_26010115` · 무관 `testlog_26010250_other`.\n\
             고유문장-본문복제감지-7f3a\n"
        ),
    );
    w("docs/testlog/testlog_26010250_other.md", "# 다른 모델 zeta2.50-flash 와 zeta2.5-flashy 만 적힌 문서\n");
    w(
        "docs/devlog/devlog_26010300_zeta_narr.md",
        &format!("# devlog\n> 뒤집은 선행: `testlog_26010200` (SUPERSEDED-IN-PART)\n\n{s} 서사 — testlog_26010200 을 서술.\n"),
    );
    w(
        "docs/report/perf_26010400_zeta_종합.md",
        &format!(
            "# perf\n{s} 종합. `docs/testlog/testlog_26010200_zeta_smoke.md` §1 · \
             `docs/benchmark/sweep_map_26010400_*.md` · `docs/benchmark/sweep_map_26010400_c1-a-levers.md` · \
             후속 교정 `docs/plan/plan_26010455_harness.md`\n"
        ),
    );
    w("docs/plan/plan_26010455_harness.md", "# 하네스 교정 — 모델 약어 ZF 만 적는다\n`docs/report/perf_26010460_zeta_far.md` · `plan_26010465_misc`\n");
    w("docs/report/perf_26010460_zeta_far.md", &format!("# {s} 타 HW 선례\n"));
    w("docs/plan/plan_26010465_misc.md", "# 무관 plan(약어도 없음)\n`docs/report/perf_26010470_zeta_chain.md`\n");
    w("docs/report/perf_26010470_zeta_chain.md", &format!("# {s} 연쇄 끝 — 닿으면 안 된다\n"));
    w("docs/report/perf_26010450_zeta_dirty.md", &format!("# perf dirty\n{s}\n"));
    w("docs/benchmark/sweep_map_26010400_c1-a-levers.md", &format!("# sweep {s}\n"));
    w("docs/benchmark/sweep_map_26010400_c1-a-levers.json", &dumps_ascii(&jd! {"model" => "zeta2.5-flash-nvfp4"}));
    w("docs/benchmark/sweep_map_26010400_c2-b-levers.md", &format!("# sweep {s} c2\n"));
    w("docs/benchmark/sweep_map_26010400_c3-c-levers.md", &format!("# sweep {s} c3\n"));
    w("docs/benchmark/sweep_map_26010400_c1-a-levers-extra.md", &format!("# sweep {s} extra\n"));
    w("docs/benchmark/sweep_map_26010410_c1-a-note.md", &format!("# note {s} c1-a\n"));
    w("docs/benchmark/sweep_map_26010410_c9-z-note.md", &format!("# note {s} c9\n"));
    w("docs/benchmark/sweep_map_26010420_c1-a-x.md", &format!("# {s} c1-a x\n자매 `sweep_map_26010420`\n"));
    w("docs/benchmark/sweep_map_26010420_c2-b-y.md", "# 다른 모델 c2 y\n");
    w(
        "docs/plan/plan_26010500_zeta_second.md",
        &format!(
            "# plan\n> 선행 `docs/report/perf_26010400_zeta_종합.md`\n> 참고 `docs/report/perf_26010450_zeta_dirty.md`\n\n\
             {s} 두 번째 계획. `docs/report/perf_26010900_zeta_later.md` 는 사후 편집 인용(D-W8).\n"
        ),
    );
    w(
        "docs/testlog/testlog_26010600_zeta_verdict.md",
        &format!(
            "# testlog\n> 계획 `docs/plan/plan_26010500_zeta_second.md`\n\n{s} 판정. \
             `docs/simlog/26010600_run` · `docs/benchmark/benchmark_26010600_zeta2.5-flash-nvfp4_GB10_0.1.0.yaml` · \
             `output/multi/benchlog/serve_fail_x/master_failure.log`\n"
        ),
    );
    w(
        "docs/devlog/devlog_26010600_zeta_main.md",
        &format!("# devlog\n{s} 본 서사. 형제 비교 `docs/benchmark/sweep_map_26010400_c3-c-levers.md` 정확 인용.\n"),
    );
    w("docs/benchmark/bench_report_26010600_zeta2.5-flash-nvfp4_GB10_0.1.0.md", &format!("# bench {s}\n"));
    w(
        "docs/benchmark/benchmark_26010600_zeta2.5-flash-nvfp4_GB10_0.1.0.yaml",
        "model: zeta2.5-flash-nvfp4\nmeasured_utc: '2026-01-05T15:00:00Z'\n",
    );
    w("docs/simlog/26010600_run/sweep_index.json", &dumps_ascii(&jd! {"meta" => jd!{"model" => "zeta2.5-flash-nvfp4"}}));
    w("output/multi/benchlog/serve_fail_x/master_failure.log", "ValueError: boom\n");
    w("output/multi/benchlog/sweep_c1-a/engine_1.log", "engine\n");
    w("output/multi/benchlog/sweep_c1-a/sweep_index.json", "{}");
    w("output/multi/benchlog/sweep_c1-a/level_01/bench_c1-a.json", "{\"completed\": 4}");
    w("output/multi/benchlog/sweep_c1-a/level_01/.tokstage/tokenizer.json", "{}");
    w("output/multi/benchlog/serve_fail_y/master_failure.log", "AttributeError: fixture\n");
    w("output/multi/benchlog/serve_fail_y/slave_failure.log", "slave\n");
    w("output/multi/benchlog/serve_fail_y/deep/c1-a/w.log", "cell-deep\n");
    w("output/multi/benchlog/serve_fail_y/deep/other/z.log", "other-deep\n");
    w("output/multi/benchlog/serve_fail_y/.hidden/x.log", "hidden\n");
    for i in 0..EXPAND_MAX_FILES + 2 {
        w(&format!("output/multi/benchlog/serve_fail_big/f{i:03}.log"), "x\n");
    }
    w("docs/plan/plan_26010700_hub.md", &format!("# hub {s}\n`testlog_26010600_zeta_verdict` · `plan_26010500_zeta_second`\n"));
    w("docs/devlog/devlog_26010710_hub_narr.md", &format!("# {s}\n`docs/plan/plan_26010500_zeta_second.md` 를 서술\n"));
    w("docs/devlog/devlog_26010900_zeta_late.md", &format!("# {s}\n`testlog_26010600` 서술(발행 뒤)\n"));
    w("docs/report/perf_26010900_zeta_later.md", &format!("# {s} later\n"));
    w("seed/이전 plan 백업/plan_26010100_zeta_first.md", &format!("# 백업 {s}\n"));
    w("sync_staging/sub_docs/report/perf_26010400_zeta_종합.md", &format!("# 미러 {s}\n"));
    w("docs/plan/example.md", "# 스켈레톤\n");
    let this_ident = jd! {"model" => "zeta2.5-flash-nvfp4", "gpu" => "NVIDIA GB10", "topology" => "multi", "tp" => 2, "vllm" => "0.1.0"};
    let rec_this = jd! {
        "publication_id" => "campx_bench_c1_a", "generated_utc" => "2026-01-05T16:00:00Z", "identity" => this_ident.clone(),
        "narrative_status" => jd!{"plan" => jd!{"source" => "docs/plan/plan_26010500_zeta_second.md"},
                                  "devlog" => jd!{"source" => "docs/devlog/devlog_26010600_zeta_main.md"},
                                  "testlog" => jd!{"source" => "docs/testlog/testlog_26010600_zeta_verdict.md"}},
        "scaffolded" => jd!{"bench_report" => "docs/benchmark/bench_report_26010600_zeta2.5-flash-nvfp4_GB10_0.1.0.md",
                            "certificate" => "docs/benchmark/benchmark_26010600_zeta2.5-flash-nvfp4_GB10_0.1.0.yaml",
                            "simlog" => "docs/simlog/26010600_run", "report" => J::Null,
                            "verification" => "sync_staging/sub_docs/report/perf_26010400_zeta_종합.md"},
    };
    let rec_prior = jd! {
        "publication_id" => "old_full_c1_a", "generated_utc" => "2026-01-01T02:00:00Z",
        "identity" => jd!{"model" => "Zeta2.5-Flash-NVFP4", "gpu" => "NVIDIA GB10", "topology" => "multi", "tp" => 2, "vllm" => "0.0.9"},
        "scaffolded" => jd!{"plan" => "docs/plan/plan_26010100_zeta_first.md",
                            "report" => "seed/이전 plan 백업/plan_26010100_zeta_first.md",
                            "devlog" => format!("/{}", "home/op-fixture/elsewhere/devlog_26010099_x.md")},
    };
    let rec_other_model = jd! {"identity" => jd!{"model" => "omega-1b", "gpu" => "NVIDIA GB10", "topology" => "multi", "tp" => 2},
                               "scaffolded" => jd!{"plan" => "docs/plan/plan_26010115_alpha.md"}};
    w("docs/plan/plan_26010016_c2b.md", &format!("# {s} 다른 셀(c2-b) 계획\n"));
    let rec_other_cell = jd! {"identity" => this_ident.clone(), "scaffolded" => jd!{"plan" => "docs/plan/plan_26010016_c2b.md"}};
    let rec_late = jd! {"generated_utc" => "2026-01-09T03:00:00Z", "identity" => this_ident.clone(),
                        "scaffolded" => jd!{"plan" => "docs/report/perf_26010900_zeta_later.md"}};
    w("docs/simlog/26010103_bc1a_run/sweep_index.json", &dumps_ascii(&jd! {"meta" => jd!{"config_name" => "b-c1-a"}}));
    w("docs/simlog/26010102_zz_run/sweep_index.json", &dumps_ascii(&jd! {"meta" => jd!{"config_name" => "c1-a"}}));
    w("docs/plan/plan_26010103_bc1a.md", &format!("# {s} 다른 셀(b-c1-a) 계획 — 시드가 되면 안 된다\n"));
    w("docs/plan/plan_26010102_zz_sim.md", &format!("# {s} 같은 셀(c1-a)을 잰 기록의 계획\n"));
    let rec_suffix_clash = jd! {"generated_utc" => "2026-01-01T03:00:00Z", "identity" => this_ident.clone(),
                                "raw_log_paths" => jd!{"simlog" => "docs/simlog/26010103_bc1a_run"},
                                "scaffolded" => jd!{"plan" => "docs/plan/plan_26010103_bc1a.md"}};
    let rec_sim_match = jd! {"generated_utc" => "2026-01-01T02:30:00Z", "identity" => this_ident.clone(),
                             "raw_log_paths" => jd!{"simlog" => "docs/simlog/26010102_zz_run"},
                             "scaffolded" => jd!{"plan" => "docs/plan/plan_26010102_zz_sim.md"}};
    w("docs/plan/plan_26010020_zbr_bringup.md", "# 브링업 계획 — 별칭 zbr7 만 적었다(base 슬러그 0회)\n");
    w("docs/testlog/testlog_26010021_zbr_two.md", &format!("# {s} 브링업 판정\n"));
    w("docs/devlog/devlog_26010022_zbr_three.md", &format!("# {s} 브링업 서사(identity.model 없는 기록 · 별칭 대조)\n"));
    w("docs/plan/plan_26010023_qq9x_three.md", &format!("# {s} 별칭 아님(다른 모델 id 에도 나오는 토큰) — 시드 ✗\n"));
    w("docs/plan/plan_26010024_zeta_h100.md", &format!("# {s} 다른 HW — 시드 ✗\n"));
    w("docs/benchmark/bench_report_26010020_zeta2.5-flash_GB10_0.0.8.md", &format!("# {s} 다른 셀의 측정 표면 — 시드 ✗\n"));
    let zid = jd! {"model" => "zeta2.5-flash", "gpu" => "NVIDIA GB10", "topology" => "single", "tp" => 1};
    let mut zid_h100 = zid.clone();
    zid_h100.set("gpu", "NVIDIA H100");
    let g0 = "2026-01-01T00:00:00Z";
    let s3_recs: Vec<(&str, J)> = vec![
        ("zbr7_one", jd! {"generated_utc" => g0, "identity" => zid.clone(),
            "scaffolded" => jd!{"plan" => "docs/plan/plan_26010020_zbr_bringup.md",
                                "bench_report" => "docs/benchmark/bench_report_26010020_zeta2.5-flash_GB10_0.0.8.md",
                                "simlog" => "docs/simlog/26010020_zbr_run"}}),
        ("zbr7_two", jd! {"generated_utc" => g0, "identity" => zid.clone(),
            "scaffolded" => jd!{"testlog" => "docs/testlog/testlog_26010021_zbr_two.md"}}),
        ("zbr7_three", jd! {"generated_utc" => g0, "identity" => jd!{"gpu" => "NVIDIA GB10"},
            "scaffolded" => jd!{"devlog" => "docs/devlog/devlog_26010022_zbr_three.md"}}),
        ("qq9x_one", jd! {"generated_utc" => g0, "identity" => zid.clone(), "scaffolded" => J::dict()}),
        ("qq9x_two", jd! {"generated_utc" => g0, "identity" => zid.clone(), "scaffolded" => J::dict()}),
        ("omega_qq9x", jd! {"generated_utc" => g0, "identity" => jd!{"model" => "omega-1b"}, "scaffolded" => J::dict()}),
        ("qq9x_three", jd! {"generated_utc" => g0, "identity" => J::dict(),
            "scaffolded" => jd!{"plan" => "docs/plan/plan_26010023_qq9x_three.md"}}),
        ("zeta_h100_run", jd! {"generated_utc" => g0, "identity" => zid_h100,
            "scaffolded" => jd!{"plan" => "docs/plan/plan_26010024_zeta_h100.md"}}),
    ];
    w("docs/simlog/26010020_zbr_run/sweep_index.json", &dumps_ascii(&jd! {"meta" => jd!{"config_name" => "zbr-cell"}}));
    let base_recs: Vec<(&str, &J)> = vec![
        ("campx_bench_c1_a", &rec_this),
        ("old_full_c1_a", &rec_prior),
        ("other_full_c1_a", &rec_other_model),
        ("campx_bench_c2_b", &rec_other_cell),
        ("redo_full_c1_a", &rec_late),
        ("old_full_b_c1_a", &rec_suffix_clash),
        ("zz_unrelated_name", &rec_sim_match),
    ];
    for (rid, rec) in base_recs.iter().map(|(a, b)| (*a, *b)).chain(s3_recs.iter().map(|(a, b)| (*a, b))) {
        w(&format!("docs/_evidence/{rid}.json"), &dumps(rec));
    }
    w("docs/_evidence/campx_bench_c1_a.work-manifest.json", "{}");
    w("docs/_evidence/broken.json", "{not json");

    let email = core::synthetic_email();
    let gdate = core::git_date("2026-01-04T00:00:00Z").unwrap_or_default();
    let genv: Vec<(String, String)> = [
        ("GIT_CONFIG_GLOBAL", "/dev/null"),
        ("GIT_CONFIG_NOSYSTEM", "1"),
        ("GIT_AUTHOR_NAME", core::SYNTHETIC_NAME),
        ("GIT_AUTHOR_EMAIL", email.as_str()),
        ("GIT_COMMITTER_NAME", core::SYNTHETIC_NAME),
        ("GIT_COMMITTER_EMAIL", email.as_str()),
        ("GIT_AUTHOR_DATE", gdate.as_str()),
        ("GIT_COMMITTER_DATE", gdate.as_str()),
    ]
    .iter()
    .map(|(a, b)| (a.to_string(), b.to_string()))
    .collect();
    w("tools/run_bench.sh", "#!/bin/bash\n# fixture bench tool\nvllm bench serve --temperature 0\n");
    w("tools/sub/run_bench.sh", "#!/bin/bash\n# 같은 basename 의 다른 도구(origin 충돌 대조)\n");
    let git_ok = (|| -> HR<()> {
        core::git_full(repo, &["init", "-q"], true, None, &genv)?;
        core::git_full(
            repo,
            &[
                "add",
                "docs/report/perf_26010400_zeta_종합.md",
                "docs/report/perf_26010450_zeta_dirty.md",
                "tools/run_bench.sh",
                "tools/sub/run_bench.sh",
            ],
            true,
            None,
            &genv,
        )?;
        core::git_full(
            repo,
            &["-c", "core.hooksPath=/dev/null", "-c", "commit.gpgsign=false", "commit", "-q", "-m", "fixture"],
            true,
            None,
            &genv,
        )?;
        Ok(())
    })();
    let git_ok = match git_ok {
        Ok(()) => true,
        Err(e) => {
            c.0.push(format!("lineage: 임시 git 픽스처 실패 {}", e.code));
            false
        }
    };
    w("docs/report/perf_26010450_zeta_dirty.md", &format!("# perf dirty (추적 후 수정)\n{s}\n"));

    // ── 문법 파싱 ──
    let pn = |p: &str| parse_doc_name(p, Some(repo)).unwrap_or(J::dict());
    let a = pn("docs/testlog/testlog_26090317_08_56_x.md");
    c.ck(
        "산문 이름 _MM_SS",
        a.get_str("kind") == Some("testlog") && a.get_str("yymmddhh") == Some("26090317") && a.get_str("mmss") == Some("08_56")
            && a.get_str("topic") == Some("x"),
    );
    c.ck("★7자리 시간 토큰은 규약 밖(legacy)", pn("docs/plan/plan_2601010_x.md")["legacy"] == J::Bool(true));
    let r = pn("docs/report/harness_26082218_08_32_노드.md");
    c.ck("report 분류·접미", r.get_str("kind") == Some("report") && r.get_str("class") == Some("harness") && r.get_str("mmss") == Some("08_32"));
    c.ck("무날짜 report = legacy", pn("docs/report/rtxpro6000-benchmark-explorer.html")["legacy"] == J::Bool(true));
    c.ck("bench_report", pn("docs/benchmark/bench_report_26091304_m_GB10_0.29.0.md").get_str("kind") == Some("bench_report"));
    c.ck("인증서 = 데이터", pn("docs/benchmark/benchmark_NA_m_GB10_0.29.0.yaml").get_str("kind") == Some("certificate"));
    c.ck("sweep json 쌍둥이", pn("docs/benchmark/sweep_map_26091008_x.json").get_str("kind") == Some("sweep_json"));
    let s1 = pn("docs/simlog/26091304_48_40_camp_x");
    c.ck("simlog 접미형 왕복 대조", s1.get_str("kind") == Some("simlog") && s1.get_str("mmss") == Some("48_40") && s1["legacy"] == J::Bool(false));
    let s2 = pn("docs/simlog/2026090115_1_gptoss120b_single");
    c.ck("레거시 simlog 10자리 → YYMMDDHH", s2.get_str("yymmddhh") == Some("26090115") && s2["legacy"] == J::Bool(true));
    c.ck("★숨김 스윕 상태는 문서 아님", pn("docs/benchmark/.sweep_x.json")["kind"].is_null());

    // ── 배너 ──
    let bn = supersede_banners(&repo.join("docs/testlog/testlog_26010200_zeta_smoke.md")).unwrap_or_default();
    c.ck("배너 여러 줄(연속 인용 문단)", bn == ["testlog_26010600_zeta_verdict", "perf_26010400_zeta_종합"]);

    // ── 파생 ──
    let ev = jd! {
        "cell" => "c1-a", "publication" => jd!{"topic" => "campx_bench_c1_a"}, "pointers" => J::list(),
        "lineage_seeds" => jd!{"journey_note" => "형제 지도 sweep_map_26010410 비교 · sweep_map_26010420", "evidence_candidates" => J::List(vec![
            jd!{"path" => "output/multi/benchlog/serve_fail_y", "kind" => "engine_failure_logs"},
            jd!{"path" => "output/multi/benchlog/serve_fail_big", "kind" => "engine_failure_logs"}])},
        "sweep" => jd!{"dir" => "output/multi/benchlog/sweep_c1-a"},
    };
    let kw = |ev: &J, depth: i64, declared: &J| derive_kw(repo, ev, "zeta2.5-flash", "26010800", depth, declared);
    let lin = match kw(&ev, DEFAULT_DEPTH, &J::list()) {
        Ok(l) => l,
        Err(e) => {
            c.0.push(format!("lineage: 파생 실패 {}: {}", e.code, e.message));
            return c.0;
        }
    };
    let docs = docs_of(&lin);
    c.ck(
        "시드 문서 채택(depth 0)",
        [
            "docs/plan/plan_26010500_zeta_second.md",
            "docs/testlog/testlog_26010600_zeta_verdict.md",
            "docs/devlog/devlog_26010600_zeta_main.md",
            "docs/benchmark/bench_report_26010600_zeta2.5-flash-nvfp4_GB10_0.1.0.md",
            "docs/plan/plan_26010100_zeta_first.md",
        ]
        .iter()
        .all(|p| dget(&docs, p)["depth"] == J::Int(0)),
    );
    let sr = &lin["method"]["seed_records"];
    c.ck(
        "과거 발행 기록(대소문자 다른 identity · id 접미 · simlog 관측) 시드",
        strs(&sr["prior"]) == ["old_full_c1_a", "zz_unrelated_name"]
            && sr["prior_match"]
                == jd! {"old_full_c1_a" => "publication-id-suffix(simlog 미관측)", "zz_unrelated_name" => "simlog:sweep_index.meta.config_name"},
    );
    let src_of = |p: &str| -> BTreeSet<String> {
        lin["seeds"].items().iter().filter(|x| x.get_str("path") == Some(p)).map(|x| pystr(&x["source"])).collect()
    };
    let one = |s: String| -> BTreeSet<String> { [s].into_iter().collect() };
    c.ck(
        "★셀 키 접미 충돌(측정 셀이 다른 기록)은 같은 셀 시드(S2) 아님 · 같은 모델 앞선 셀(S3)로만",
        strs(&sr["prior_cell_mismatch"]) == ["old_full_b_c1_a"]
            && src_of("docs/plan/plan_26010103_bc1a.md") == one(format!("{MODEL_PRIOR_SOURCE}old_full_b_c1_a"))
            && dget(&docs, "docs/plan/plan_26010102_zz_sim.md")["depth"] == J::Int(0),
    );
    c.ck(
        "★다른 모델 기록은 어떤 시드도 아님 · 다른 셀 기록은 S2 시드 아님",
        !lin["seeds"].items().iter().any(|x| pystr(&x["source"]).ends_with("other_full_c1_a"))
            && !lin["seeds"].items().iter().any(|x| x.get_str("source") == Some("prior_publication:campx_bench_c2_b")),
    );
    let mpr = &lin["method"]["model_prior_records"];
    let mrecs: BTreeSet<String> = strs(&mpr["records"]).into_iter().collect();
    c.ck(
        "S3: 같은 모델 앞선 다른 셀 기록 = 시드(identity.model · 대소문자·양자화 접미 무관)",
        ["campx_bench_c2_b", "zbr7_one", "zbr7_two"].iter().all(|x| mrecs.contains(*x))
            && mpr["match"].get_str("zbr7_one") == Some("identity.model~base_slug")
            && src_of("docs/plan/plan_26010016_c2b.md") == one(format!("{MODEL_PRIOR_SOURCE}campx_bench_c2_b")),
    );
    c.ck("S3: 별칭만 적은 브링업 문서도 채택(identity 가 모델을 선언 · 필터 면제)", dget(&docs, "docs/plan/plan_26010020_zbr_bringup.md")["depth"] == J::Int(0));
    c.ck(
        "S3: 관측 별칭 파생(같은 모델 id 첫 토큰 2건+ · 다른 모델 id 에 없음)",
        strs(&mpr["aliases"]) == ["zbr7"] && mpr["alias_evidence"] == jd! {"zbr7" => vec!["zbr7_one", "zbr7_two"]},
    );
    c.ck(
        "S3: identity.model 없는 기록은 별칭으로 대조",
        mpr["match"].get_str("zbr7_three").unwrap_or("").starts_with("id-alias:zbr7")
            && dget(&docs, "docs/devlog/devlog_26010022_zbr_three.md")["depth"] == J::Int(0),
    );
    c.ck(
        "★S3: 다른 모델 id 에도 나오는 토큰은 별칭 아님 → 모델 없는 기록 불채택",
        !mrecs.contains("qq9x_three") && !docs.contains_key("docs/plan/plan_26010023_qq9x_three.md"),
    );
    c.ck(
        "★S3: 다른 모델·다른 HW·발행 뒤 기록 제외",
        !mrecs.contains("other_full_c1_a")
            && strs(&mpr["gpu_mismatch"]) == ["zeta_h100_run"]
            && !docs.contains_key("docs/plan/plan_26010024_zeta_h100.md")
            && strs(&mpr["after_ceiling"]).contains(&"redo_full_c1_a".to_string())
            && !mrecs.contains("redo_full_c1_a"),
    );
    c.ck("★S3: S2 가 잡은 같은 셀 기록은 S3 에 중복하지 않는다", !strs(&sr["prior"]).iter().any(|x| mrecs.contains(x)));
    c.ck(
        "★S3: 다른 셀의 측정 표면·데이터 포인터는 싣지 않는다",
        !docs.contains_key("docs/benchmark/bench_report_26010020_zeta2.5-flash_GB10_0.0.8.md")
            && !lin["evidence_candidates"].items().iter().any(|x| pystr(&x["path"]).starts_with("docs/simlog/26010020_zbr_run"))
            && strs(&mpr["skipped_non_narrative"]).contains(&"docs/benchmark/bench_report_26010020_zeta2.5-flash_GB10_0.0.8.md".to_string()),
    );
    c.ck("★발행 뒤 과거 기록은 제외", strs(&sr["prior_after_ceiling"]) == ["redo_full_c1_a"]);
    c.ck(
        "조상 체인(시드 plan_first → plan_zero depth 1 → plan_origin depth 2)",
        dget(&docs, "docs/plan/plan_26010001_zeta_zero.md")["depth"] == J::Int(1)
            && dget(&docs, "docs/plan/plan_26010000_zeta_origin.md")["depth"] == J::Int(2),
    );
    match kw(&ev, 1, &J::list()) {
        Ok(lin_d1) => {
            let d1 = docs_of(&lin_d1);
            c.ck(
                "★깊이 상한(depth=1) 밖 제외",
                d1.contains_key("docs/plan/plan_26010001_zeta_zero.md")
                    && !d1.contains_key("docs/plan/plan_26010000_zeta_origin.md")
                    && lin_d1["documents"].items().iter().all(|x| x["depth"].as_i64().unwrap_or(99) <= 1),
            );
        }
        Err(e) => c.ck(&format!("깊이 1 파생 {}", e.code), false),
    }
    let transit_paths: BTreeSet<String> = lin["transit"].items().iter().map(|t| pystr(&t["path"])).collect();
    c.ck(
        "필터 탈락 문서 경유 → 너머의 슬러그 문서 채택",
        dget(&docs, "docs/report/perf_26010460_zeta_far.md")["depth"] == J::Int(3) && transit_paths.contains("docs/plan/plan_26010455_harness.md"),
    );
    c.ck("★경유 노드는 읽기 목록 밖", !docs.contains_key("docs/plan/plan_26010455_harness.md"));
    c.ck(
        "★경유 → 경유 연쇄 금지(연속 2홉 필터 탈락)",
        !docs.contains_key("docs/report/perf_26010470_zeta_chain.md") && !transit_paths.contains("docs/plan/plan_26010465_misc.md"),
    );
    let via_has = |p: &str, f: &dyn Fn(&J) -> bool| dget(&docs, p)["via"].items().iter().any(f);
    c.ck("경유 간선도 via 에 실린다", via_has("docs/report/perf_26010460_zeta_far.md", &|v| v.get_str("from") == Some("docs/plan/plan_26010455_harness.md")));
    c.ck("★허브(후손 plan) 제외", !docs.contains_key("docs/plan/plan_26010700_hub.md"));
    c.ck("★plan 만 서술한 devlog 는 서술자 아님", !docs.contains_key("docs/devlog/devlog_26010710_hub_narr.md"));
    c.ck("testlog 서술 devlog 보강(narrates)", via_has("docs/devlog/devlog_26010300_zeta_narr.md", &|v| v.get_str("edge") == Some("narrates")));
    c.ck(
        "★상한 뒤 문서 제외(사후 편집 인용·발행 뒤 서술)",
        !docs.contains_key("docs/report/perf_26010900_zeta_later.md") && !docs.contains_key("docs/devlog/devlog_26010900_zeta_late.md"),
    );
    c.ck("★필터(모델 슬러그 불일치·경계) 제외", !docs.contains_key("docs/testlog/testlog_26010250_other.md"));
    let amb: Vec<J> = ["docs/plan/plan_26010115_alpha.md", "docs/plan/plan_26010115_beta.md"]
        .iter()
        .map(|p| dget(&docs, p)["resolution"].clone())
        .collect();
    c.ck("★주제 없는 토큰의 동시각 형제 = 모호 전부", amb == [J::from("ambiguous"), J::from("ambiguous")]);
    c.ck(
        "유일 형제 = unique",
        via_has("docs/devlog/devlog_26010300_zeta_narr.md", &|v| v.get_str("resolution") == Some("unique") && v.get_str("edge") == Some("narrates")),
    );
    let amb_ex: BTreeMap<String, J> = lin["ambiguous_excluded"].items().iter().map(|a| (pystr(&a["path"]), a.clone())).collect();
    c.ck("글롭 형제: 정확 인용 exact 유지", dget(&docs, "docs/benchmark/sweep_map_26010400_c1-a-levers.md").get_str("resolution") == Some("exact"));
    let c2b = "docs/benchmark/sweep_map_26010400_c2-b-levers.md";
    c.ck(
        "★글롭 형제 중 셀 키 불일치(c2-b)는 채택하지 않고 ambiguous_excluded 에 사유·출처와 함께 적는다",
        !docs.contains_key(c2b)
            && amb_ex.get(c2b).map(|a| strs(&a["from"])) == Some(vec!["docs/report/perf_26010400_zeta_종합.md".to_string()])
            && amb_ex.get(c2b).is_some_and(|a| pystr(&a["reason"]).contains("셀 키 적중 0"))
            && lin["method"]["stats"]["ambiguous_excluded"] == J::Int(lin["ambiguous_excluded"].len() as i64),
    );
    c.ck(
        "★글롭 형제여도 다른 문서가 정확 인용하면 채택(필터는 모호 간선에만) · 제외 목록에 없다",
        docs.contains_key("docs/benchmark/sweep_map_26010400_c3-c-levers.md") && !amb_ex.contains_key("docs/benchmark/sweep_map_26010400_c3-c-levers.md"),
    );
    c.ck("최선 형제 동률(md/json 쌍둥이)은 둘 다 유지", !amb_ex.contains_key("docs/benchmark/sweep_map_26010400_c1-a-levers.json"));
    let extra = "docs/benchmark/sweep_map_26010400_c1-a-levers-extra.md";
    c.ck(
        "★셀 키 적중이 같으면 이질 토큰이 적은 형제만(c1-a-levers-extra 제외 · 사유 기재)",
        !docs.contains_key(extra) && amb_ex.get(extra).is_some_and(|a| pystr(&a["reason"]).contains("이질 토큰 2 < 최선 형제 2·1")),
    );
    c.ck(
        "★인용 원천(lineage_seeds 본문)의 모호 토큰도 형제 필터를 탄다(c9-z 제외 · 출처 = 그 시드 키)",
        docs.contains_key("docs/benchmark/sweep_map_26010410_c1-a-note.md")
            && !docs.contains_key("docs/benchmark/sweep_map_26010410_c9-z-note.md")
            && amb_ex.get("docs/benchmark/sweep_map_26010410_c9-z-note.md").map(|a| strs(&a["from"])) == Some(vec!["lineage_seeds:journey_note".to_string()]),
    );
    match kw(&ev, 1, &J::list()) {
        Ok(lin_d1) => {
            let y = "docs/benchmark/sweep_map_26010420_c2-b-y.md";
            c.ck(
                "★다른 자리에서 남겨진 형제는 ambiguous_excluded 에 적지 않는다(그 뒤 부재 = 슬러그 필터 · 경유 아님)",
                docs.contains_key("docs/benchmark/sweep_map_26010420_c1-a-x.md")
                    && !docs.contains_key(y)
                    && !amb_ex.contains_key(y)
                    && !lin_d1["transit"].items().iter().any(|t| t.get_str("path") == Some(y))
                    && !lin_d1["ambiguous_excluded"].items().iter().any(|t| t.get_str("path") == Some(y)),
            );
        }
        Err(e) => c.ck(&format!("깊이 1 파생 {}", e.code), false),
    }
    let mem2: BTreeSet<String> =
        ["docs/benchmark/sweep_map_26010400_c1-a-levers.md", "docs/benchmark/sweep_map_26010400_c2-b-levers.md"].iter().map(|s| s.to_string()).collect();
    let mut recs2: Vec<BTreeMap<String, (String, String, Vec<String>)>> = Vec::new();
    for order in [["sweep_map_26010400_z", "sweep_map_26010400"], ["sweep_map_26010400", "sweep_map_26010400_z"]] {
        let Ok(corp) = Corpus::new(repo, &DEFAULT_ROOTS, &EXCLUDED_ROOTS) else {
            c.ck("코퍼스 생성", false);
            continue;
        };
        *corp.sf.borrow_mut() = Some(SiblingFilter::new(&[J::from("c1-a")]));
        for (i, tok) in order.iter().enumerate() {
            let _ = corp.sf_keep(&format!("src{i}"), tok, &mem2);
        }
        let sf = corp.sf.borrow();
        recs2.push(
            sf.as_ref()
                .unwrap()
                .excluded
                .iter()
                .map(|(k, v)| (k.clone(), (v.token.clone(), v.reason.clone(), v.from.iter().cloned().collect())))
                .collect(),
        );
    }
    c.ck(
        "★형제 제외 기록 = 호출 순서 무관(결정론)",
        recs2.len() == 2
            && recs2[0] == recs2[1]
            && recs2[0].len() == 1
            && recs2[0].get(c2b).map(|x| x.0.as_str()) == Some("sweep_map_26010400"),
    );
    c.ck("무신호 형제(셀 키 적중 0 — alpha/beta)는 제외 목록에 없다(전부 유지)", !amb_ex.keys().any(|p| p.starts_with("docs/plan/plan_26010115_")));
    let ev_q = set(&ev, "cell", J::from("c1-q"));
    let ev_ax = set(&ev_q, "cell_config", jd! {"declared_axes" => jd!{"lever" => "c2-b", "blank" => "<<FILL>>"}});
    match (kw(&ev_q, DEFAULT_DEPTH, &J::list()), kw(&ev_ax, DEFAULT_DEPTH, &J::list())) {
        (Ok(lin_q), Ok(lin_ax)) => {
            c.ck("★셀 선언 축 없음 — c2-b 는 셀 키 적중 0 이라 제외", !docs_of(&lin_q).contains_key(c2b));
            let kt = strs(&lin_ax["method"]["ambiguous_siblings"]["key_tokens"]);
            c.ck(
                "셀 선언 축(declared_axes) 값이 셀 키에 든다 — c2-b 채택",
                docs_of(&lin_ax).contains_key(c2b) && kt.contains(&"c2-b".to_string()) && !kt.iter().any(|k| k.contains("fill")),
            );
        }
        _ => c.ck("셀 선언 축 파생", false),
    }
    c.ck(
        "★seed/·sync_staging/ 사본은 노드 아님",
        !docs.keys().any(|p| p.starts_with("seed/") || p.starts_with("sync_staging/"))
            && lin["seeds_missing"].items().iter().any(|m| m.get_str("reason") == Some("excluded-root")),
    );
    c.ck("★스켈레톤 example.md 제외", !docs.contains_key("docs/plan/example.md"));
    c.ck("헤더 라벨 간선", via_has("docs/report/perf_26010400_zeta_종합.md", &|v| v.get_str("edge") == Some("header:선행")));
    c.ck(
        "뒤집힘(양쪽 배너 · 날짜 방향)",
        strs(&dget(&docs, "docs/testlog/testlog_26010200_zeta_smoke.md")["superseded_by"])
            == [
                "docs/devlog/devlog_26010300_zeta_narr.md",
                "docs/report/perf_26010400_zeta_종합.md",
                "docs/testlog/testlog_26010600_zeta_verdict.md",
            ],
    );
    c.ck("★배너의 이전 문서는 뒤집은 쪽이 아님", dget(&docs, "docs/devlog/devlog_26010300_zeta_narr.md")["superseded_by"] == J::list());
    let cands: BTreeMap<String, J> = lin["evidence_candidates"].items().iter().map(|x| (pystr(&x["path"]), x.clone())).collect();
    c.ck(
        "데이터 후보(simlog·인증서·엔진 로그·원시 경로)",
        [
            "docs/simlog/26010600_run",
            "docs/benchmark/benchmark_26010600_zeta2.5-flash-nvfp4_GB10_0.1.0.yaml",
            "output/multi/benchlog/sweep_c1-a/engine_1.log",
            "output/multi/benchlog/serve_fail_x/master_failure.log",
        ]
        .iter()
        .all(|p| cands.contains_key(*p)),
    );
    c.ck("★데이터 후보는 문서 목록에 없다", !cands.keys().any(|p| docs.contains_key(p)));
    c.ck("스윕 레벨 원시 JSON 도 후보", cands.contains_key("output/multi/benchlog/sweep_c1-a/level_01/bench_c1-a.json"));
    c.ck("★숨김 경로(.tokstage)는 후보가 아니다", !cands.keys().any(|p| p.contains("/.")));
    let yd = "output/multi/benchlog/serve_fail_y";
    c.ck(
        "디렉터리 후보 → 바로 아래 파일 + 셀 키 경로 파일",
        [format!("{yd}/master_failure.log"), format!("{yd}/slave_failure.log"), format!("{yd}/deep/c1-a/w.log")]
            .iter()
            .all(|p| cands.contains_key(p))
            && cands.get(&format!("{yd}/master_failure.log")).map(|x| strs(&x["sources"])) == Some(vec![format!("expanded:{yd}")]),
    );
    c.ck(
        "★깊은 타 셀 파일은 펴지 않되 개수를 적고(unlisted) 전체 경로로는 닿는다",
        !cands.contains_key(&format!("{yd}/deep/other/z.log"))
            && cands.get(yd).map(|x| x["expanded"].clone()) == Some(jd! {"listed" => 3, "total" => 4, "unlisted" => 1, "truncated" => false})
            && resolve_stem(&lin, &format!("{yd}/deep/other/z.log")).ok().flatten() == Some(format!("{yd}/deep/other/z.log")),
    );
    let big = cands.get("output/multi/benchlog/serve_fail_big").map(|x| x["expanded"].clone()).unwrap_or(J::dict());
    c.ck(
        "★펼치기 상한(EXPAND_MAX_FILES) · 잘림 기재",
        big["listed"] == J::Int(EXPAND_MAX_FILES as i64)
            && big["truncated"] == J::Bool(true)
            && cands.keys().filter(|p| p.starts_with("output/multi/benchlog/serve_fail_big/")).count() == EXPAND_MAX_FILES,
    );
    c.ck(
        "펼친 파일은 발췌 출처로 해소된다(전체 경로)",
        resolve_stem(&lin, &format!("{yd}/master_failure.log")).ok().flatten() == Some(format!("{yd}/master_failure.log")),
    );
    if git_ok {
        let rep = dget(&docs, "docs/report/perf_26010400_zeta_종합.md");
        let dirty = dget(&docs, "docs/report/perf_26010450_zeta_dirty.md");
        c.ck("추적 report = 경로+커밋(digest 없음)", rep["commit"].truthy() && rep["sha256"].is_null());
        c.ck("★추적 후 수정 = sha256", dirty["commit"].is_null() && dirty["sha256"].truthy());
        selftest_tool_sources(&mut c, repo, &ev);
    }
    c.ck("비추적 = sha256", dget(&docs, "docs/plan/plan_26010500_zeta_second.md")["sha256"].truthy());
    c.ck(
        "relevance 셀 축 토큰",
        strs(&dget(&docs, "docs/benchmark/sweep_map_26010400_c1-a-levers.md")["relevance"]["axis_tokens"]).contains(&"c1-a".to_string()),
    );
    let dump = core::dumps(&lin);
    c.ck(
        "★저장소 밖 포인터 원문(운영자 경로) 비유출 · 결손은 기재",
        !dump.contains("op-fixture")
            && lin["seeds_missing"].items().iter().any(|m| m.get_str("path") == Some("<outside-repo>") && m.get_str("reason") == Some("outside-repo")),
    );
    c.ck("★원문 복제 금지(본문 문장 미포함)", !dump.contains("고유문장-본문복제감지-7f3a"));
    c.ck("결정론(두 번 파생 동일)", kw(&ev, DEFAULT_DEPTH, &J::list()).map(|l| core::dumps(&l)).ok().as_deref() == Some(dump.as_str()));
    c.ck("절대경로 미포함", !dump.contains(&core::posix(repo)));

    w("campaigns/campx/journey.jsonl", &(dumps_ascii(&jd! {"reason" => "반증 — `testlog_26010200_zeta_smoke` 참조"}) + "\n"));
    match kw(&set(&ev, "lineage_seeds", jd! {"journey" => "campaigns/campx/journey.jsonl"}), DEFAULT_DEPTH, &J::list()) {
        Ok(lin_j) => c.ck(
            "여정 파일 본문 인용 = 시드",
            lin_j["seeds"].items().iter().any(|x| {
                x.get_str("path") == Some("docs/testlog/testlog_26010200_zeta_smoke.md") && x.get_str("source") == Some("lineage_seeds:journey:file")
            }),
        ),
        Err(e) => c.ck(&format!("여정 파생 {}", e.code), false),
    }

    // ── 선언 ──
    match kw(&ev, DEFAULT_DEPTH, &J::List(vec![J::from("docs/testlog/testlog_26010250_other.md=사람 보충(간선 끊김)")])) {
        Ok(lin2) => {
            let d2 = docs_of(&lin2);
            c.ck(
                "선언 = 필터 면제 시드",
                dget(&d2, "docs/testlog/testlog_26010250_other.md")["depth"] == J::Int(0)
                    && lin2["declared"]
                        == J::List(vec![jd! {"path" => "docs/testlog/testlog_26010250_other.md", "source" => "declared", "reason" => "사람 보충(간선 끊김)"}]),
            );
        }
        Err(e) => c.ck(&format!("선언 파생 {}", e.code), false),
    }
    for (label, item, code) in [
        ("★사유 없는 선언", "docs/testlog/testlog_26010250_other.md=", "HINT_LINEAGE_DECLARED_REASON_ABSENT"),
        ("★seed/ 선언", "seed/이전 plan 백업/plan_26010100_zeta_first.md=x", "HINT_LINEAGE_DECLARED_EXCLUDED"),
        ("★부재 선언", "docs/plan/plan_26010199_none.md=x", "HINT_LINEAGE_DECLARED_ABSENT"),
        ("★상한 뒤 선언", "docs/report/perf_26010900_zeta_later.md=x", "HINT_LINEAGE_DECLARED_AFTER_CEILING"),
    ] {
        match kw(&ev, DEFAULT_DEPTH, &J::List(vec![J::from(item)])) {
            Ok(_) => c.ck(label, false),
            Err(e) => c.ck(&format!("{label}({})", e.code), e.code == code),
        }
    }
    for (label, slug, kst, code) in [
        ("★비주입 시각", "zeta2.5-flash", "2026-01-08", "HINT_TIME_NOT_INJECTED"),
        ("★빈 슬러그", "--", "26010800", "HINT_LINEAGE_FILTER_ABSENT"),
    ] {
        match derive_kw(repo, &ev, slug, kst, DEFAULT_DEPTH, &J::list()) {
            Ok(_) => c.ck(label, false),
            Err(e) => c.ck(&format!("{label}({})", e.code), e.code == code),
        }
    }

    // ── 소비자 도우미 ──
    let rl = reading_list(&lin).unwrap_or_default();
    c.ck("읽기 목록 = 문서 수", rl.len() == lin["documents"].len());
    let tiers: Vec<bool> = rl.iter().map(|x| x.get_str("tier") != Some("primary")).collect();
    let mut st = tiers.clone();
    st.sort();
    c.ck("읽기 목록: 모호 계층이 뒤", tiers == st);
    let prim: Vec<i64> = rl.iter().filter(|x| x.get_str("tier") == Some("primary")).map(|x| x["depth"].as_i64().unwrap_or(0)).collect();
    c.ck("읽기 목록: primary 는 깊이 오름차순", prim.windows(2).all(|w| w[0] <= w[1]));
    let rs = |s: &str| resolve_stem(&lin, s).ok().flatten();
    c.ck("resolve_stem 전체 stem", rs("testlog_26010200_zeta_smoke").as_deref() == Some("docs/testlog/testlog_26010200_zeta_smoke.md"));
    c.ck("resolve_stem 맨 stem(유일)", rs("testlog_26010600").as_deref() == Some("docs/testlog/testlog_26010600_zeta_verdict.md"));
    c.ck("★resolve_stem 모호 = None", rs("plan_26010115").is_none());
    c.ck("★resolve_stem LINEAGE 밖 = None", rs("plan_26010700_hub").is_none());
    c.ck("resolve_stem md/json 쌍둥이 확장자", rs("sweep_map_26010400_c1-a-levers.md").as_deref() == Some("docs/benchmark/sweep_map_26010400_c1-a-levers.md"));
    c.ck(
        "resolve_stem 후보 디렉터리 안 파일",
        rs("docs/simlog/26010600_run/sweep_index.json").as_deref() == Some("docs/simlog/26010600_run/sweep_index.json"),
    );
    match require_documents(&jd! {"documents" => J::list()}) {
        Ok(()) => c.ck("★문서 0건 차단", false),
        Err(e) => c.ck("★문서 0건 차단(code)", e.code == "HINT_LINEAGE_EMPTY"),
    }
    match mention_graph(repo, &DEFAULT_ROOTS.map(String::from), &EXCLUDED_ROOTS.map(String::from)) {
        Ok(mg) => c.ck(
            "mention_graph 는 seed/ 원천 없음",
            !mg.as_dict().unwrap().keys().any(|k| k.starts_with("seed/") || k.starts_with("sync_staging/")),
        ),
        Err(e) => c.ck(&format!("mention_graph {}", e.code), false),
    }
    let recs = load_publication_records(repo).unwrap_or_default();
    let mut want: Vec<String> = [
        "broken",
        "campx_bench_c1_a",
        "campx_bench_c2_b",
        "old_full_b_c1_a",
        "old_full_c1_a",
        "other_full_c1_a",
        "redo_full_c1_a",
        "zz_unrelated_name",
    ]
    .iter()
    .map(|s| s.to_string())
    .chain(s3_recs.iter().map(|(r, _)| r.to_string()))
    .collect();
    want.sort();
    c.ck(
        "발행 기록 적재: work-manifest 제외 · 깨진 기록 표시",
        recs.iter().map(|r| pystr(&r["_id"])).collect::<Vec<_>>() == want && recs.first().is_some_and(|r| r.get("_unreadable").is_some()),
    );
    match seeds_from_publications(repo, &J::List(recs.clone()), Some("nope"), &J::Null, Some("c1-a"), None) {
        Ok(sp) => c.ck(
            "★이 발행 기록 부재 = 결손 기재(차단 ✗)",
            sp["missing"][0].get_str("reason") == Some("record-absent") && sp["seeds"] == J::list(),
        ),
        Err(e) => c.ck(&format!("시드 {}", e.code), false),
    }

    // ── 수신자 요약 · 봉인 출처 스냅샷 ──
    let d0 = pystr(&lin["documents"][0]["path"]);
    let counts = {
        let mut m = Map::new();
        m.insert(d0.clone(), 2);
        m.insert("docs/simlog/26010600_run/sweep_index.json", 1);
        J::Dict(m)
    };
    let summ = match receiver_summary(&lin, &counts, &J::Null) {
        Ok(s) => s,
        Err(e) => {
            c.0.push(format!("lineage: 요약 실패 {}", e.code));
            return c.0;
        }
    };
    let s0 = &summ["documents"][0];
    c.ck(
        "요약: 문서 수 = 전체 · 경로 · 역할 · 날짜 · 발췌 수만(해시 · via ✗ · path 파생 stem 중복 ✗ v3)",
        summ["documents"].len() == lin["documents"].len()
            && s0["excerpts"] == J::Int(2)
            && s0.get_str("path") == Some(d0.as_str())
            && !["sha256", "via", "commit", "stem"].iter().any(|k| s0.get(k).is_some())
            && summ.get(SEALED_SOURCES_KEY).is_none()
            && summ.get_str("kind") == Some(SUMMARY_KIND),
    );
    c.ck(
        "요약: 후보는 수 + 인용된 것만",
        summ["evidence_candidates"]["count"] == J::Int(lin["evidence_candidates"].len() as i64)
            && summ["evidence_candidates"]["cited"] == J::List(vec![jd! {"path" => "docs/simlog/26010600_run/sweep_index.json", "excerpts" => 1}]),
    );
    c.ck("요약 ≤ 상한 · 전체보다 작다", core::dumps(&summ).len() <= SUMMARY_MAX_BYTES && clen(&core::dumps(&summ)) < clen(&core::dumps(&lin)));
    let doc0 = lin["documents"][0].clone();
    let with_docs = |docs: Vec<J>| set(&lin, "documents", J::List(docs));
    let bigl = with_docs((0..200).map(|i| set(&doc0, "path", J::Str(format!("docs/plan/plan_26010100_{}_{i}.md", "x".repeat(200))))).collect());
    match receiver_summary(&bigl, &J::Null, &J::Null) {
        Ok(_) => c.ck("★요약 상한 초과 = HINT_LINEAGE_SUMMARY_TOO_LARGE", false),
        Err(e) => c.ck("★요약 상한 초과 = HINT_LINEAGE_SUMMARY_TOO_LARGE(code)", e.code == "HINT_LINEAGE_SUMMARY_TOO_LARGE"),
    }
    let kinds = ["plan", "testlog", "devlog", "bench_report"];
    let q79_docs: Vec<J> = (0..79)
        .map(|i| {
            let d = set(&doc0, "kind", J::from(kinds[i % 4]));
            set(&d, "path", J::Str(format!("docs/testlog/testlog_2610{i:04}_q38fn_nv4_멀티_캠페인_판정_{i:02}.md")))
        })
        .collect();
    let q79 = set(&with_docs(q79_docs.clone()), "evidence_candidates", J::List((0..276).map(|i| jd! {"path" => format!("x{i}")}).collect()));
    match receiver_summary(&q79, &J::Null, &J::Null) {
        Ok(s79) => {
            let v2_docs: Vec<J> = s79["documents"]
                .items()
                .iter()
                .map(|x| {
                    let p = pystr(&x["path"]);
                    set(x, "stem", J::Str(_strip_ext(&pname(&p))))
                })
                .collect();
            let v2_like = set(&s79, "documents", J::List(v2_docs));
            c.ck(
                "★79문서 요약 상한 안 · 역사적 v2 들여쓰기 형상의 중복 stem 크기 대조",
                core::dumps(&s79).len() <= SUMMARY_MAX_BYTES
                    && s79["documents"].items().iter().all(|x| x.get("stem").is_none())
                    && (crate::value::dumps_pretty_sorted(&v2_like) + "\n").len() > SUMMARY_MAX_BYTES
                    && s79["schema_version"] == J::Int(SUMMARY_SCHEMA_VERSION)
                    && SUMMARY_SCHEMA_VERSION == 3,
            );
        }
        Err(e) => c.ck(&format!("79문서 요약 {}", e.code), false),
    }
    let q97_docs: Vec<J> = (0..97)
        .map(|i| set(&q79_docs[i % 79], "path", J::Str(format!("docs/testlog/testlog_2610{i:04}_Qwen_1M_로딩비교와_대표_NVMe_full_bench_{i:02}.md"))))
        .collect();
    let q97 = set(&q79, "documents", J::List(q97_docs.clone()));
    let seals97: Vec<J> = q97_docs[..11].iter().map(|d| jd! {"path" => pystr(&d["path"]), "sha256" => "a".repeat(64)}).collect();
    match receiver_summary(&q97, &J::Null, &J::List(seals97.clone())) {
        Ok(s97) => {
            let encoded97 = core::dumps(&s97);
            let pretty97 = crate::value::dumps_pretty_sorted(&s97) + "\n";
            let summary_path = repo.join("receiver-summary.json");
            let _ = core::write_json(&summary_path, &s97);
            c.ck(
                "★97문서+11봉인 출처: 공백 압축만 · 전 필드 roundtrip · 실제 파일 상한",
                pretty97.len() > SUMMARY_MAX_BYTES
                    && encoded97.len() <= SUMMARY_MAX_BYTES
                    // dict equality is order-insensitive in Python: compare the canonical (sorted) serialisation.
                    && loads(&encoded97).ok().map(|v| core::dumps(&v)).as_deref() == Some(encoded97.as_str())
                    && s97[SEALED_SOURCES_KEY] == J::List(seals97)
                    && std::fs::read(&summary_path).ok() == Some(encoded97.clone().into_bytes()),
            );
        }
        Err(e) => c.ck(&format!("97문서 요약 {}", e.code), false),
    }
    let ss = sealed_sources(repo, &J::List(vec![J::Str(d0.clone()), J::Str(d0.clone())])).unwrap_or_default();
    let want_sha = std::fs::read(repo.join(&d0)).map(|b| core::sha256_bytes(&b)).unwrap_or_default();
    c.ck("봉인 스냅샷: 중복 제거 · sha256 = 현재 바이트", ss == vec![jd! {"path" => d0.clone(), "sha256" => want_sha}]);
    c.ck(
        "요약에 스냅샷 싣기",
        receiver_summary(&lin, &J::Null, &J::List(ss.clone())).map(|x| x[SEALED_SOURCES_KEY].clone()).ok() == Some(J::List(ss.clone())),
    );
    for (label, bad) in [("★읽을 수 없는 출처", "docs/plan/nope.md"), ("★저장소 밖 경로", "../x.md"), ("★절대 경로", "/etc/hosts")] {
        match sealed_sources(repo, &J::List(vec![J::from(bad)])) {
            Ok(_) => c.ck(&format!("{label} = HINT_SEALED_SOURCE_UNREADABLE"), false),
            Err(e) => c.ck(&format!("{label} = HINT_SEALED_SOURCE_UNREADABLE(code)"), e.code == "HINT_SEALED_SOURCE_UNREADABLE"),
        }
    }
    drop(td);
    c.0
}
