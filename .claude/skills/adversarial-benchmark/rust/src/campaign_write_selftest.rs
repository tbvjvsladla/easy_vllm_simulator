//! `campaign_init --selftest` of the native program: the writer contracts exercised through the same entry point the
//! callers use (`campaign_write::run`) on a scratch checkout. Exit 0 and a final `[campaign_init] PASS` mean every check
//! held; `FAIL` lines name the broken one. (The owner program's own `--selftest`, which tests its in-process functions,
//! stays reachable through `CAMPAIGN_READ_PYTHON`.)

use crate::campaign_write::{run, Ctx, Outcome, Sinks};
use std::path::{Path, PathBuf};

fn put(root: &Path, rel: &str, text: &str) {
    let path = root.join(rel);
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(path, text);
}

fn read(root: &Path, rel: &str) -> String {
    std::fs::read_to_string(root.join(rel)).unwrap_or_default()
}

struct Probe {
    status: i32,
    out: String,
    err: String,
}

fn call(root: &Path, args: &[&str]) -> Option<Probe> {
    let argv: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    match run(&Ctx { root: root.to_path_buf() }, &argv, &mut Sinks { out: &mut out, err: &mut err }) {
        Outcome::Done(status) => Some(Probe {
            status,
            out: String::from_utf8_lossy(&out).into_owned(),
            err: String::from_utf8_lossy(&err).into_owned(),
        }),
        Outcome::Fallback => None,
    }
}

fn scratch() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("campaign-selftest-{}-{:?}", std::process::id(), std::thread::current().id()));
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::create_dir_all(&dir);
    put(&dir, "campaigns/_template/campaign.yaml", "{\"_note\": \"x\", \"id\": \"<<FILL>>\", \"plan_ref\": \"<<FILL>>\"}\n");
    put(&dir, "campaigns/_template/evidence_pointers.json", "{\"_note\": \"x\", \"campaign_id\": \"<<FILL>>\"}\n");
    put(&dir, ".claude/skills/terraforming_node/scripts/campaign_template_validator.py", "#");
    put(&dir, "docs/testlog/t.md", "x");
    put(&dir, "docs/testlog/u.md", "x");
    dir
}

const DECL: &str = "{\"id\": \"w1\", \"plan_ref\": \"docs/plan/p.md\", \"declared_utc\": \"2026-01-01T00:00:00Z\", \"nodes\": [{\"node_id\": \"main\", \"role\": \"main\", \"topology\": \"single\"}, {\"node_id\": \"sub\", \"role\": \"sub\", \"topology\": \"single\"}], \"assignments\": {\"main\": [{\"cell\": \"c1\"}, {\"cell\": \"c2\", \"mode\": \"STAY\"}], \"sub\": [{\"cell\": \"s1\"}]}, \"control_variables\": {\"model\": \"m0\", \"vllm_version\": \"0.1.0\", \"topology\": \"single\"}}";

/// The checks; returns the exit status (0 = all held).
pub fn selftest(io: &mut Sinks<'_>) -> i32 {
    let mut ok = true;
    let mut say = |io: &mut Sinks<'_>, label: &str, cond: bool| {
        let _ = writeln!(io.out, "{}{label}", if cond { "  PASS " } else { "  FAIL " });
        ok &= cond;
    };
    let root = scratch();
    let r = root.as_path();

    // active pointer and derived paths (the read shapes are answered by campaign_read; the writers need an instance)
    put(r, "campaigns/ACTIVE", "w1\n");
    put(r, "campaigns/w1/campaign.yaml", DECL);
    put(r, "campaigns/w1/evidence_pointers.json", "{\"pointers\": []}");
    let p = call(r, &["--assigned-cells", "main"]).unwrap();
    say(io, "배정 질의는 선언 순서로 셀을 낸다", p.status == 0 && p.out == "c1,c2\n");
    let p = call(r, &["--assigned-cells", "zz"]).unwrap();
    say(io, "★음성대조 배정 없는 노드는 거부", p.status == 1 && p.err.contains("가 비었다"));

    // phase writer
    let p = call(r, &["--phase-set", "build", "--node", "main", "--state", "done", "--proof-ok", "--proof-source", "docker inspect", "--proof-predicate", "이미지 실재", "--cell", "c1"]).unwrap();
    let phase = read(r, "campaigns/w1/phases/main/build.status.json");
    say(io, "phase writer 가 proof 를 쓴다", p.status == 0 && phase.contains("\"ok\": true") && phase.contains("docker inspect"));
    let p = call(r, &["--phase-set", "serve", "--node", "main", "--state", "done", "--proof-ok"]).unwrap();
    say(io, "★음성대조 proof.ok 인데 source 없으면 거부", p.status == 1 && p.err.contains("--proof-source 가 필수다"));
    let p = call(r, &["--phase-set", "nope", "--node", "main", "--state", "done"]).unwrap();
    say(io, "★음성대조 알 수 없는 phase 거부", p.status == 1);
    let p = call(r, &["--phase-set", "serve", "--node", "main", "--state", "nope"]).unwrap();
    say(io, "★음성대조 알 수 없는 state 거부", p.status == 1);
    let p = call(r, &["--phase-set", "serve", "--node", "<<FILL>>", "--state", "pending"]).unwrap();
    say(io, "★음성대조 <<FILL>> 잔존 거부", p.status == 1 && p.err.contains("<<FILL>>"));

    // cell writer and journey
    let p = call(r, &["--cell-set", "c1", "--outcome", "measured", "--node", "main", "--version", "0.1.0", "--model", "m0", "--measurement-decode-tps", "12.5", "--measurement-source", "certificate x.yaml", "--axis-citation", "kv dtype 축", "--next-intent", "c2 착수 — attn 축", "--utc", "2026-01-01T01:00:00Z"]).unwrap();
    let cell = read(r, "campaigns/w1/cells/c1/cell.status.json");
    say(io, "cell writer 가 outcome·측정·축을 쓴다", p.status == 0 && cell.contains("\"decode_tps_conc1\": 12.5") && cell.contains("kv dtype 축"));
    say(io, "여정 한 줄이 같은 트랜잭션에서 쌓인다", read(r, "campaigns/w1/journey.jsonl").lines().count() == 1);
    let p = call(r, &["--cell-set", "c2", "--outcome", "nope"]).unwrap();
    say(io, "★음성대조 알 수 없는 outcome 거부", p.status == 1);
    let p = call(r, &["--cell-set", "c2", "--outcome", "void", "--void-reason", "열 트립"]).unwrap();
    say(io, "★음성대조 void_reason 에 출처 없으면 거부", p.status == 1);
    let p = call(r, &["--cell-set", "c2", "--outcome", "measured", "--measurement-decode-tps", "1.0"]).unwrap();
    say(io, "★음성대조 측정값에 출처 없으면 거부", p.status == 1 && p.err.contains("--measurement-source"));

    // evidence writer
    let add = |cell: &str, node: &str| call(r, &["--evidence-add", "--kind", "testlog", "--path", "docs/testlog/t.md", "--cell", cell, "--node", node]).unwrap();
    say(io, "evidence writer 가 포인터를 쓴다", add("c1", "main").status == 0 && read(r, "campaigns/w1/evidence_pointers.json").contains("docs/testlog/t.md"));
    add("c1", "main");
    say(io, "같은 포인터는 두 번 적지 않는다(멱등)", read(r, "campaigns/w1/evidence_pointers.json").matches("docs/testlog/t.md").count() == 1);
    say(io, "★음성대조 이미 다른 셀로 귀속된 증거는 조용히 바꾸지 않는다", add("c2", "main").status == 1);
    let p = call(r, &["--evidence-add", "--kind", "testlog", "--path", "does/not/exist.md", "--node", "main"]).unwrap();
    say(io, "★음성대조 실재하지 않는 증거 거부", p.status == 1 && p.err.contains("증거가 실재하지 않는다"));
    let p = call(r, &["--evidence-add", "--kind", "nope", "--path", "docs/testlog/t.md", "--node", "main"]).unwrap();
    say(io, "★음성대조 알 수 없는 kind 거부", p.status == 1);
    let p = call(r, &["--evidence-untag", "--kind", "testlog", "--path", "docs/testlog/t.md", "--cell", "c1", "--untag-reason", " ", "--utc", "U"]).unwrap();
    say(io, "★음성대조 사유 없는 셀 태그 해제 거부", p.status == 1);
    let p = call(r, &["--evidence-untag", "--kind", "testlog", "--path", "docs/testlog/t.md", "--cell", "c1", "--untag-reason", "셀 선행 근거", "--utc", "U"]).unwrap();
    say(io, "셀 태그 해제: 포인터는 남고 이력에 사유가 적힌다", p.status == 0 && read(r, "campaigns/w1/evidence_pointers.json").contains("셀 선행 근거"));
    let p = call(r, &["--freeze-evidence", "--utc", "2026-01-02T00:00:00Z"]).unwrap();
    say(io, "동결은 명시 연산이다", p.status == 0 && read(r, "campaigns/w1/evidence_pointers.json").contains("frozen_utc"));
    let p = call(r, &["--evidence-add", "--kind", "testlog", "--path", "docs/testlog/u.md", "--node", "main"]).unwrap();
    say(io, "★음성대조 동결된 스냅샷에는 추가가 거부된다", p.status == 1 && p.err.contains("동결"));
    let p = call(r, &["--evidence-add", "--kind", "testlog", "--path", "docs/testlog/u.md", "--node", "main", "--unfreeze"]).unwrap();
    say(io, "사람이 명시한 --unfreeze 만 동결을 연다", p.status == 0);

    // revision and hint approval
    let rev = ["--revise", "--revise-set", "model=m1", "--revise-reason", "r", "--revise-evidence", "e", "--revise-approved-by", "a", "--utc", "U"];
    let p = call(r, &rev).unwrap();
    say(io, "개정은 t0 원본 뒤에 append 된다", p.status == 0 && read(r, "campaigns/w1/campaign.yaml").contains("t0 선언"));
    let p = call(r, &["--revise", "--revise-set", "model=m2"]).unwrap();
    say(io, "★음성대조 사유·근거·승인자 없는 개정 거부", p.status == 1);
    let hint = ["--hint-approve", "--node", "main", "--cells", "c1", "--approved-by", "사람", "--utc", "2026-01-01T01:00:00Z"];
    say(io, "hint 사전승인이 hint_targets 에 전사된다", call(r, &hint).unwrap().status == 0 && read(r, "campaigns/w1/campaign.yaml").contains("hint_targets"));
    let p = call(r, &["--hint-approve", "--node", "main", "--cells", "s1", "--approved-by", "a", "--utc", "2026-01-01T01:00:00Z"]).unwrap();
    say(io, "★음성대조 그 노드 배정 밖 셀은 승인할 수 없다", p.status == 1 && p.err.contains("에 없다"));
    let p = call(r, &["--hint-approve", "--node", "main", "--cells", "c2", "--approved-by", "a", "--utc", "T"]).unwrap();
    say(io, "★음성대조 주입 시각 모양이 아니면 거부", p.status == 1);

    // assignment writer
    let assign = |cells: &'static str| call(r, &["--assign-set", "main", "--assign-cells", cells, "--assign-reason", "r", "--assign-evidence", "e", "--assign-approved-by", "a", "--utc", "U"]).unwrap();
    say(io, "★음성대조 승인을 든 셀은 뺄 수 없다", assign("c2").status == 1);
    say(io, "★음성대조 다른 노드의 셀은 가져올 수 없다", assign("c1,c2,s1").status == 1);
    say(io, "★음성대조 같은 셀이 두 번이면 거부", assign("c1,c1").status == 1);
    let p = call(r, &["--assign-set", "main", "--assign-cells", "c1:zz", "--assign-reason", "r", "--assign-evidence", "e", "--assign-approved-by", "a", "--utc", "U"]).unwrap();
    say(io, "★음성대조 mode 는 닫힌 어휘다", p.status == 1);

    // purge gate and close
    let p = call(r, &["--verify-purge-gate", "w1"]).unwrap();
    say(io, "★게이트는 증거·진행표를 묻는다(셀 c2 는 sweep 이 없어도 P3 sub 진행표 부재로 닫힌다)", p.status == 1 && p.err.contains("P3 sub"));
    put(r, "campaigns/w1/phases/sub/bench.status.json", "{}");
    let p = call(r, &["--verify-purge-gate", "w1"]).unwrap();
    say(io, "전수 실재 + 선언 노드 진행표 → 게이트 열림", p.status == 0 && p.out.contains("열림"));
    let p = call(r, &["--close", "w1"]).unwrap();
    say(io, "★종결 음성대조: --utc 없으면 거부(벽시계 금지)", p.status == 1 && p.err.contains("--utc"));
    let p = call(r, &["--close", "other", "--utc", "U"]).unwrap();
    say(io, "★종결 음성대조: 활성이 아닌 id 는 거부", p.status == 1 && p.err.contains("활성 캠페인은"));
    let p = call(r, &["--close", "w1", "--utc", "2026-01-03T00:00:00Z"]).unwrap();
    say(io, "종결 dry-run 은 ACTIVE 를 바꾸지 않는다", p.status == 0 && read(r, "campaigns/ACTIVE").trim() == "w1");
    let p = call(r, &["--close", "w1", "--utc", "2026-01-03T00:00:00Z", "--apply"]).unwrap();
    say(io, "종결 apply → ACTIVE=_bootstrap · 인스턴스 보존 · 여정 기재", p.status == 0 && read(r, "campaigns/ACTIVE").trim() == "_bootstrap" && read(r, "campaigns/w1/journey.jsonl").contains("campaign_closed"));
    let p = call(r, &["--close", "w1", "--utc", "2026-01-03T00:00:00Z", "--apply"]).unwrap();
    say(io, "종결은 멱등이다(이미 _bootstrap 이면 no-op)", p.status == 0 && p.out.contains("no-op"));
    let p = call(r, &["--phase-set", "bench", "--node", "main", "--state", "running"]).unwrap();
    say(io, "★_bootstrap 이면 writer 는 no-op(rc 0 · stderr 한 줄)", p.status == 0 && p.err.contains("no-op"));

    // lifecycle: suspension needs its declarations; resume needs a suspended campaign
    put(r, "campaigns/ACTIVE", "w1\n");
    let _ = std::fs::remove_file(r.join("campaigns/w1/journey.jsonl"));
    let p = call(r, &["--suspend", "w1", "--utc", "U", "--approved-by", "a"]).unwrap();
    say(io, "★보류는 자원 반납 선언 없이는 거부된다(정상 차단)", p.status == 1 && p.err.contains("--release"));
    let p = call(r, &["--resume-campaign", "w1", "--utc", "U", "--approved-by", "a"]).unwrap();
    say(io, "★재개는 보류 캠페인에만 한다", p.status == 1 && p.err.contains("재개는 보류 캠페인에만 한다"));
    let p = call(r, &["--init", "w2", "--plan-ref", "docs/plan/p.md", "--target-topology", "single"]).unwrap();
    say(io, "★처분 없이 활성 캠페인을 덮어쓰는 --init 은 거부된다", p.status == 1 && p.err.contains("처분 없이 덮어쓰는"));

    // brief and resume brief
    let p = call(r, &["--write-brief", "--node", "main", "--utc", "2026-01-03T01:00:00Z"]).unwrap();
    say(io, "브리핑은 docs/logs 평면에 선언된 관측면 한 파일로 쓴다", p.status == 0 && read(r, "docs/logs/main/campaign_brief.json").contains("\"node_id\": \"main\""));
    let p = call(r, &["--resume-brief"]).unwrap();
    say(io, "재개 요약은 인스턴스만 읽고 한 화면을 낸다", p.status == 0 && p.out.starts_with("상태: ACTIVE=w1") && p.out.contains("## 6. 반증된 축"));

    // the validator is this program's own module: no file beside it is needed to answer an assignment question
    let _ = std::fs::remove_file(r.join(".claude/skills/terraforming_node/scripts/campaign_template_validator.py"));
    let p = call(r, &["--assigned-cells", "main"]).unwrap();
    say(io, "배정 질의는 검증기 파일 없이도 내장 검증기로 답한다", p.status == 0 && !p.err.contains("검증기를 찾지 못했다"));

    // argparse's surface is native: an abbreviation is accepted, an unknown option is the usage error (status 2)
    let p = call(r, &["--assigned", "main"]).unwrap();
    say(io, "약어(--assigned)는 argparse 처럼 받아들인다", p.status == 0);
    let p = call(r, &["--bogus"]).unwrap();
    say(io, "★미지 옵션은 usage 오류(rc 2)", p.status == 2 && p.err.contains("campaign_init.py: error: unrecognized arguments: --bogus"));

    let _ = std::fs::remove_dir_all(&root);
    let _ = writeln!(io.out, "[campaign_init] {}", if ok { "PASS" } else { "FAIL" });
    if ok {
        0
    } else {
        1
    }
}

