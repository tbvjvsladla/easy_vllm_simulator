// 통합 행동: init → sync → query/read/tree → check → remove. 원문 불변·--apply 규율·고아 차단·사용자 수정 보호를 핀한다.
import { test, describe, beforeEach, afterEach } from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { execFileSync, spawnSync } from "node:child_process";
import { initWiki, syncWiki, queryWiki, readRef, treeOf, checkWiki, statusWiki, removeWiki } from "../src/commands.ts";

const CONTRACT = path.join(import.meta.dirname, "fixtures", "contract", "valid.json");
const CLI = path.join(import.meta.dirname, "..", "src", "cli.ts");
const NOW = "2026-10-14T00:00:00Z";
const LATER = "2026-10-15T00:00:00Z";

const PLAN_REL = "docs/plan/plan_26100101_멀티턴_전환.md";
const DEVLOG_REL = "docs/devlog/devlog_26100102_세션.md";
const card = (src: string): string => `__llm-wiki/cards/${src}.card.md`;

let root = "";
const put = (rel: string, data: string | Buffer): void => {
  const p = path.join(root, rel);
  fs.mkdirSync(path.dirname(p), { recursive: true });
  fs.writeFileSync(p, data);
};
const read = (rel: string): string => fs.readFileSync(path.join(root, rel), "utf8");
const exists = (rel: string): boolean => fs.existsSync(path.join(root, rel));
const snapshot = (dir: string): Record<string, string> => {
  const out: Record<string, string> = {};
  const walk = (d: string): void => {
    for (const e of fs.readdirSync(path.join(root, d), { withFileTypes: true })) {
      const rel = path.posix.join(d, e.name);
      if (e.isDirectory()) walk(rel);
      else out[rel] = fs.readFileSync(path.join(root, rel)).toString("base64");
    }
  };
  if (exists(dir)) walk(dir);
  return out;
};

const PLAN = `---
title: 멀티턴 기본값 전환 계획
document_type: plan
created: 2026-10-01
plan: plan_26093001_이전계획
---
# 멀티턴 기본값 전환

## 배경
멀티턴 RAG는 기본 OFF였다.

## 결정
커밋 기본 workflow를 멀티턴 ON으로 바꾼다.
gemini-embedding-2 임베딩 차원은 768로 둔다.
`;
const DEVLOG = `# 세션 기록

## 결과
임베딩 output_dimensionality 768 적용 완료.
`;
const BADNAME = `---
title: "깨진" 제목
---
본문
`;

beforeEach(() => {
  root = fs.realpathSync(fs.mkdtempSync(path.join(os.tmpdir(), "wiki-desk-cmd-")));
  put(PLAN_REL, PLAN);
  put(DEVLOG_REL, DEVLOG);
  put("docs/misc/notes.md", BADNAME);
});
afterEach(() => fs.rmSync(root, { recursive: true, force: true }));

const init = (): void => { initWiki(root, CONTRACT, { apply: true, now: NOW }); };
const synced = (): void => { init(); syncWiki(root, { apply: true, now: NOW }); };

describe("init", () => {
  test("기본은 계획만 출력하고 아무것도 쓰지 않는다", () => {
    const r = initWiki(root, CONTRACT, { apply: false, now: NOW });
    assert.equal(r.applied, false);
    assert.ok(r.writes.includes("__llm-wiki/contract.json"));
    assert.equal(exists("__llm-wiki"), false);
  });
  test("--apply는 계약·receipt·캐시 제외 .gitignore·index·log를 만든다", () => {
    init();
    assert.deepEqual(JSON.parse(read("__llm-wiki/contract.json")), JSON.parse(fs.readFileSync(CONTRACT, "utf8")));
    assert.match(read("__llm-wiki/.gitignore"), /^\.cache\/$/m);
    assert.ok(exists("__llm-wiki/receipt.json"), "receipt는 추적 대상(캐시 밖)");
    assert.ok(exists("__llm-wiki/index.md") && exists("__llm-wiki/log.md"));
  });
  test("기존 위키가 있으면 거부한다(덮어쓰기 없음)", () => {
    init();
    assert.throws(() => initWiki(root, CONTRACT, { apply: true, now: NOW }), /이미 존재/);
  });
  test("다른 리터럴의 위키(__llm_wiki)가 있어도 거부한다", () => {
    put("__llm_wiki/contract.json", "{}");
    assert.throws(() => initWiki(root, CONTRACT, { apply: true, now: NOW }), /이미 존재/);
    assert.equal(exists("__llm-wiki"), false);
  });
});

describe("sync", () => {
  test("계획은 쓰기 0, 적용은 원문당 카드 1개를 만들고 원문은 불변", () => {
    init();
    const before = snapshot("docs");
    const wikiBefore = snapshot("__llm-wiki");
    const plan = syncWiki(root, { apply: false, now: NOW });
    assert.equal(plan.applied, false);
    assert.equal(plan.created.length, 3);
    assert.deepEqual(snapshot("__llm-wiki"), wikiBefore);
    const r = syncWiki(root, { apply: true, now: NOW });
    assert.equal(r.applied, true);
    assert.deepEqual(snapshot("docs"), before);
    const c = read(card(PLAN_REL));
    assert.match(c, /^type: plan$/m);
    assert.match(c, /^resource: docs\/plan\/plan_26100101_멀티턴_전환\.md$/m);
    assert.match(c, /^description: null$/m); // 원문에 description 없음 → 생성하지 않는다
  });
  test("변경 없는 재실행은 고정점(쓰기 0, generated.at 유지)", () => {
    synced();
    const wiki = snapshot("__llm-wiki");
    const r = syncWiki(root, { apply: true, now: LATER });
    assert.equal(r.created.length + r.updated.length + r.archived.length, 0);
    assert.deepEqual(snapshot("__llm-wiki"), wiki);
  });
  test("파일명 규칙 위반·frontmatter 파싱 불가는 needs_review로 표면화", () => {
    init();
    const r = syncWiki(root, { apply: true, now: NOW });
    assert.deepEqual(r.needs_review, ["docs/misc/notes.md"]);
    const c = read(card("docs/misc/notes.md"));
    assert.match(c, /^needs_review: true$/m);
    assert.match(c, /파일명 규칙 위반/);
    assert.match(c, /frontmatter 파싱 불가/);
  });
  test("frontmatter 닫힘 누락은 명령을 멈추지 않고 needs_review", () => {
    put("docs/plan/plan_26100105_닫힘없음.md", "---\ntitle: x\n본문\n");
    init();
    const r = syncWiki(root, { apply: true, now: NOW });
    assert.ok(r.needs_review.includes("docs/plan/plan_26100105_닫힘없음.md"));
    assert.match(read(card("docs/plan/plan_26100105_닫힘없음.md")), /닫힘/);
  });
  test("UTF-8이 아닌 원문은 needs_review, 해시는 원 바이트 기준이라 check·query·sync가 일치", () => {
    put("docs/plan/plan_26100106_인코딩.md", Buffer.from([0x23, 0x20, 0xb0, 0xa1, 0x0a])); // "# " + CP949 '가'
    init();
    const r = syncWiki(root, { apply: true, now: NOW });
    assert.ok(r.needs_review.includes("docs/plan/plan_26100106_인코딩.md"));
    assert.match(read(card("docs/plan/plan_26100106_인코딩.md")), /UTF-8/);
    assert.equal(syncWiki(root, { apply: true, now: LATER }).updated.length, 0);
    assert.deepEqual(checkWiki(root).issues, []);
    assert.deepEqual(queryWiki(root, ["멀티턴"]).stale, []);
  });
  test("원문이 바뀌면 해당 카드만 갱신하고 log에 기록한다", () => {
    synced();
    put(DEVLOG_REL, DEVLOG + "\n추가 줄\n");
    const r = syncWiki(root, { apply: true, now: LATER });
    assert.deepEqual(r.updated, [DEVLOG_REL]);
    assert.match(read("__llm-wiki/log.md"), /^## 2026-10-15$/m);
  });
  test("예약 파일명 원문(index.md·log.md)도 카드는 concept 이름을 쓴다 — check 통과", () => {
    put("docs/index.md", "---\ntitle: 홈\n---\n# 홈\n");
    put("docs/sub/log.md", "# 기록\n");
    synced();
    assert.ok(exists(card("docs/index.md")) && exists(card("docs/sub/log.md")));
    assert.deepEqual(checkWiki(root).issues, []);
  });
  test("대문자·다른 접미사 원문도 멱등(재생성 반복 없음)", () => {
    const c = JSON.parse(fs.readFileSync(CONTRACT, "utf8"));
    c.source_suffixes = [".md", ".txt"];
    const alt = path.join(root, "alt-contract.json");
    fs.writeFileSync(alt, JSON.stringify(c));
    put("docs/UP.MD", "# 대문자\n");
    put("docs/note.txt", "메모\n");
    initWiki(root, alt, { apply: true, now: NOW });
    syncWiki(root, { apply: true, now: NOW });
    const again = syncWiki(root, { apply: false, now: LATER });
    assert.equal(again.created.length + again.updated.length, 0);
    fs.rmSync(path.join(root, "docs/note.txt"));
    assert.deepEqual(syncWiki(root, { apply: false, now: LATER }).orphans_blocked, ["docs/note.txt"]);
  });

  describe("사용자 수정 보호", () => {
    test("사람이 고친 카드(본문 메모·verified 키)는 덮어쓰지 않고 protected로 보고", () => {
      synced();
      const edited = read(card(PLAN_REL)).replace(/^---\n/, "---\nverified:\n  by: \"human:ash\"\n  at: \"2026-10-14T09:00:00+09:00\"\n") + "\n사용자 메모\n";
      put(card(PLAN_REL), edited);
      put(PLAN_REL, PLAN + "\n원문 변경\n");
      const r = syncWiki(root, { apply: true, now: LATER });
      assert.deepEqual(r.protected, [card(PLAN_REL)]);
      assert.equal(read(card(PLAN_REL)), edited);
      assert.ok(!r.updated.includes(PLAN_REL));
    });
    test("도구 형식 그대로 사람 키(reviewed_by)만 추가한 카드는 원문 변경 시 갱신되고 키는 보존, remove는 지우지 않는다", () => {
      synced();
      const withKey = read(card(PLAN_REL)).replace(/^---\n/, "---\nreviewed_by: kim\n");
      put(card(PLAN_REL), withKey);
      put(PLAN_REL, PLAN + "\n원문 변경\n");
      const r = syncWiki(root, { apply: true, now: LATER });
      assert.deepEqual(r.protected, []);
      assert.deepEqual(r.updated, [PLAN_REL]);
      const after = read(card(PLAN_REL));
      assert.match(after, /^reviewed_by: kim$/m);
      assert.deepEqual(checkWiki(root).issues, []); // stale 해소
      assert.equal(syncWiki(root, { apply: true, now: LATER }).writes.length, 0); // 고정점
      // 원문이 그대로면 사람 키를 더한 것만으로 재기록하지 않는다(키 위치가 달라도)
      const top = read(card(DEVLOG_REL)).replace(/^---\n/, "---\nverified_by: \"human:kim\"\n");
      put(card(DEVLOG_REL), top);
      assert.equal(syncWiki(root, { apply: true, now: LATER }).updated.length, 0);
      assert.equal(read(card(DEVLOG_REL)), top);
      removeWiki(root, { apply: true });
      assert.match(read(card(PLAN_REL)), /^reviewed_by: kim$/m);
    });
    test("사람 확인(verified)이 있는 카드는 원문이 바뀌어도 확인을 새 내용에 옮기지 않는다 — reverify로 보고", () => {
      synced();
      const attested = read(card(PLAN_REL)).replace(/^---\n/, "---\nverified:\n  by: human:ash\n  at: \"2026-10-14T09:00:00+09:00\"\n");
      put(card(PLAN_REL), attested);
      put(PLAN_REL, PLAN + "\n결정 뒤집힘\n");
      const r = syncWiki(root, { apply: true, now: LATER });
      assert.deepEqual(r.reverify, [PLAN_REL]);
      assert.ok(!r.updated.includes(PLAN_REL));
      assert.equal(read(card(PLAN_REL)), attested);
      assert.ok(checkWiki(root).issues.some((i) => i.code === "card.stale" && i.path === PLAN_REL));
    });
    for (const [name, line] of [
      ["따옴표가 섞인 plain 값 뒤 주석", "reviewer: kim's check # 근거: 3절 표 대조 완료 '중요'"],
      ["인치 표기 뒤 주석", "screen: 27\" monitor # 메모: 표는 \"부록 B\" 참조"],
      ["도구 형식과 다른 따옴표", "reviewed_by: \"kim\""],
      ["카드 본문 제목과 같은 주석 줄", "# 멀티턴 기본값 전환 계획\nreviewed_by: kim"],
    ] as const) {
      test(`사람 키가 도구 형식으로 다시 쓸 수 없는 모양이면 보호(${name})`, () => {
        synced();
        const edited = read(card(PLAN_REL)).replace(/\n---\n/, `\n${line}\n---\n`);
        put(card(PLAN_REL), edited);
        put(PLAN_REL, PLAN + "\n원문 변경\n");
        const r = syncWiki(root, { apply: true, now: LATER });
        assert.ok(r.protected.includes(card(PLAN_REL)));
        assert.equal(read(card(PLAN_REL)), edited);
      });
    }
    for (const sourceChanged of [false, true]) {
      test(`사람 키를 더하면서 도구 키(description)도 고친 카드는 보호(원문 변경 ${sourceChanged})`, () => {
        synced();
        const edited = read(card(PLAN_REL))
          // 도구 출력 형식 그대로(따옴표 없이) 고쳐야 무손실 검사를 통과해 pureSha 가드까지 도달한다
          .replace(/^description: null$/m, "description: 사람 설명")
          .replace(/^---\n/, "---\nreviewed_by: kim\n");
        put(card(PLAN_REL), edited);
        if (sourceChanged) put(PLAN_REL, PLAN + "\n원문 변경\n");
        const r = syncWiki(root, { apply: true, now: LATER });
        assert.ok(r.protected.includes(card(PLAN_REL)));
        assert.ok(!r.updated.includes(PLAN_REL));
        assert.equal(read(card(PLAN_REL)), edited);
      });
    }
    test("사람이 frontmatter에 YAML 주석을 남긴 카드는 사람 글로 보고 보호한다(주석 유실 방지)", () => {
      synced();
      const withComment = read(card(PLAN_REL)).replace(/^---\n/, "---\nverified:\n  by: \"human:ash\" # 확인 메모\n  at: \"2026-10-14T09:00:00+09:00\"\n");
      put(card(PLAN_REL), withComment);
      put(PLAN_REL, PLAN + "\n원문 변경\n");
      const r = syncWiki(root, { apply: true, now: LATER });
      assert.deepEqual(r.protected, [card(PLAN_REL)]);
      assert.equal(read(card(PLAN_REL)), withComment);
    });
    test("remove 뒤에도 사람 키 카드는 소유 기록이 남아 원문 변경 시 계속 갱신된다", () => {
      synced();
      put(card(PLAN_REL), read(card(PLAN_REL)).replace(/^---\n/, "---\nreviewed_by: kim\n"));
      removeWiki(root, { apply: true });
      assert.ok(exists(card(PLAN_REL)));
      put(PLAN_REL, PLAN + "\n원문 변경\n");
      const r = syncWiki(root, { apply: true, now: LATER });
      assert.deepEqual(r.protected, []);
      assert.ok(r.updated.includes(PLAN_REL));
      assert.match(read(card(PLAN_REL)), /^reviewed_by: kim$/m);
    });
    test("frontmatter의 넘치는 실수(1e999)도 sync를 멈추지 않는다", () => {
      put("docs/plan/plan_26100110_큰수.md", "---\nversion: 1e999\n---\n# 큰수\n");
      init();
      const r = syncWiki(root, { apply: true, now: NOW });
      assert.ok(r.created.includes("docs/plan/plan_26100110_큰수.md"));
      assert.match(read(card("docs/plan/plan_26100110_큰수.md")), /1e999/);
    });
    test("사람이 고친 index.md도 보존하고 remove가 지우지 않는다", () => {
      synced();
      put("__llm-wiki/index.md", read("__llm-wiki/index.md") + "\n사용자 추가 섹션\n");
      put("docs/plan/plan_26100107_신규.md", "# 신규\n");
      const r = syncWiki(root, { apply: true, now: LATER });
      assert.ok(r.protected.includes("__llm-wiki/index.md"));
      assert.match(read("__llm-wiki/index.md"), /사용자 추가 섹션/);
      removeWiki(root, { apply: true });
      assert.match(read("__llm-wiki/index.md"), /사용자 추가 섹션/);
    });
    test("receipt가 없으면 sync 적용을 거부한다(소유 근거 없음)", () => {
      synced();
      fs.rmSync(path.join(root, "__llm-wiki/receipt.json"));
      assert.throws(() => syncWiki(root, { apply: true, now: LATER }), /receipt/);
    });
  });

  describe("고아 조정", () => {
    const orphaned = (): void => { synced(); fs.rmSync(path.join(root, DEVLOG_REL)); };
    test("고아가 있으면 적용 거부 — 신규 원문 카드도 만들지 않는다(쓰기 0)", () => {
      orphaned();
      put("docs/plan/plan_26100108_새것.md", "# 새것\n");
      const wiki = snapshot("__llm-wiki");
      assert.deepEqual(syncWiki(root, { apply: false, now: LATER }).orphans_blocked, [DEVLOG_REL]);
      assert.throws(() => syncWiki(root, { apply: true, now: LATER }), /고아/);
      assert.deepEqual(snapshot("__llm-wiki"), wiki);
    });
    test("명시 수용 시 archive로 바이트 보존 이동", () => {
      orphaned();
      const before = read(card(DEVLOG_REL));
      const r = syncWiki(root, { apply: true, now: LATER, acceptRemoved: [DEVLOG_REL] });
      assert.deepEqual(r.archived, [DEVLOG_REL]);
      assert.equal(exists(card(DEVLOG_REL)), false);
      assert.equal(read(`__llm-wiki/archive/${DEVLOG_REL}.card.md`), before);
    });
    test("relocate는 옛 카드를 보존 이동하고 새 경로 카드를 만들며 log에 이동을 기록", () => {
      synced();
      const neu = "docs/devlog/devlog_26100103_세션.md";
      fs.renameSync(path.join(root, DEVLOG_REL), path.join(root, neu));
      const r = syncWiki(root, { apply: true, now: LATER, relocate: [[DEVLOG_REL, neu]] });
      assert.deepEqual(r.archived, [DEVLOG_REL]);
      assert.ok(r.created.includes(neu));
      assert.match(read("__llm-wiki/log.md"), /devlog_26100102_세션\.md → docs\/devlog\/devlog_26100103_세션\.md/);
    });
    const rejects: [string, () => Parameters<typeof syncWiki>[1], RegExp][] = [
      ["고아가 아닌 수용 지정", () => ({ apply: true, now: LATER, acceptRemoved: [PLAN_REL] }), /고아가 아님/],
      ["수용 지정 중복", () => ({ apply: true, now: LATER, acceptRemoved: [DEVLOG_REL, DEVLOG_REL] }), /중복/],
      ["accept와 relocate에 같은 경로", () => ({ apply: true, now: LATER, acceptRemoved: [DEVLOG_REL], relocate: [[DEVLOG_REL, PLAN_REL]] }), /중복/],
      ["relocate 목적지가 원문 범위 밖", () => ({ apply: true, now: LATER, relocate: [[DEVLOG_REL, "docs/none.md"]] }), /원문 범위/],
      ["안전하지 않은 경로", () => ({ apply: true, now: LATER, acceptRemoved: ["../x.md"] }), /경로/],
    ];
    for (const [name, opts, msg] of rejects) {
      test(`거부: ${name} — 위키 불변`, () => {
        orphaned();
        const wiki = snapshot("__llm-wiki");
        assert.throws(() => syncWiki(root, opts()), msg);
        assert.deepEqual(snapshot("__llm-wiki"), wiki);
      });
    }
    test("거부: archive 위치에 이미 파일이 있으면 덮어쓰지 않는다", () => {
      orphaned();
      put(`__llm-wiki/archive/${DEVLOG_REL}.card.md`, "이전 보존본\n");
      assert.throws(() => syncWiki(root, { apply: true, now: LATER, acceptRemoved: [DEVLOG_REL] }), /보존 위치/);
      assert.equal(read(`__llm-wiki/archive/${DEVLOG_REL}.card.md`), "이전 보존본\n");
    });
  });
});

describe("query·read·tree", () => {
  test("질의는 path:L-L span을 돌려주고, 변형 질의(OR)를 받는다", () => {
    synced();
    const r = queryWiki(root, ["임베딩 차원", "output_dimensionality"]);
    const paths = r.results.map((x) => x.path);
    assert.ok(paths.includes(PLAN_REL));
    assert.ok(paths.includes(DEVLOG_REL));
    const planHit = r.results.find((x) => x.path === PLAN_REL)!;
    assert.match(planHit.spans[0].ref, /^docs\/plan\/plan_26100101_멀티턴_전환\.md:\d+-\d+$/);
    assert.match(readRef(root, planHit.spans[0].ref).text, /임베딩 차원/);
    assert.equal(planHit.spans[0].heading_path.at(-1), "결정");
  });
  test("출력 키 계약과 query_tokens·unmatched_tokens", () => {
    synced();
    const r = queryWiki(root, ["멀티턴 zzqqxx"]);
    assert.deepEqual(Object.keys(r).sort(), [
      "corpus", "elapsed_ms", "index_cache_reset", "index_rebuilt", "invalid_cards", "missing_cards",
      "query_tokens", "results", "stale", "terms", "unmatched_tokens",
    ]);
    assert.ok(r.unmatched_tokens.includes("zzqqxx"));
    assert.deepEqual(Object.keys(r.results[0]).sort(), ["authority_rank", "path", "score", "spans", "title"]);
  });
  test("색인 가능한 토큰이 없는 질의는 거부(빈 결과와 구분)", () => {
    synced();
    assert.throws(() => queryWiki(root, ["a", "#"]), /토큰/);
  });
  test("--limit이 result_limit보다 커도 그 수만큼 돌려준다", () => {
    init();
    for (let i = 0; i < 13; i++) put(`docs/plan/plan_261001${String(i).padStart(2, "0")}_공통${i}.md`, `# 공통어휘 ${i}\n`);
    syncWiki(root, { apply: true, now: NOW });
    assert.equal(queryWiki(root, ["공통어휘"]).results.length, 10); // fixture result_limit
    assert.equal(queryWiki(root, ["공통어휘"], 13).results.length, 13);
  });
  test("frontmatter에만 있는 관례 값(source_meta·source_meta_unknown)도 검색된다", () => {
    put("docs/plan/plan_26100109_메타.md", "---\ntitle: 메타\nouroboros_seed_id: seed_b061f7a39e47\ntags: [zebraword]\n---\n# 본문\n내용\n");
    synced();
    assert.equal(queryWiki(root, ["seed_b061f7a39e47"]).results[0].path, "docs/plan/plan_26100109_메타.md");
    assert.equal(queryWiki(root, ["zebraword"]).results[0].path, "docs/plan/plan_26100109_메타.md");
  });
  test("질의는 카드를 쓰지 않고, 파생 캐시만 cache_dir에 만든다", () => {
    synced();
    const cards = snapshot("__llm-wiki/cards");
    queryWiki(root, ["멀티턴"]);
    assert.deepEqual(snapshot("__llm-wiki/cards"), cards);
    assert.ok(exists("__llm-wiki/.cache/index.json"));
  });
  test("원문이 카드보다 새로우면 stale 목록에 표시한다", () => {
    synced();
    put(PLAN_REL, PLAN + "\n멀티턴 추가 메모\n");
    assert.ok(queryWiki(root, ["멀티턴"]).stale.includes(PLAN_REL));
  });
  test("손상 카드는 질의를 멈추지 않고 invalid_cards로 보고", () => {
    synced();
    put(card(PLAN_REL), read(card(PLAN_REL)).replace(/^type: plan\n/m, ""));
    const r = queryWiki(root, ["멀티턴"]);
    assert.ok(r.invalid_cards.includes(PLAN_REL));
    assert.ok(r.results.some((x) => x.path === PLAN_REL)); // 원문 검색은 계속된다
  });
  test("read는 원문 범위만 허용한다(범위 밖·원문 외 경로 거부)", () => {
    init();
    assert.match(readRef(root, `${DEVLOG_REL}:3-4`).text, /^## 결과/);
    assert.throws(() => readRef(root, `${DEVLOG_REL}:3-99`), /범위/);
    assert.throws(() => readRef(root, "__llm-wiki/contract.json:1-2"), /원문 범위 밖/);
  });
  test("tree는 헤딩 줄범위를 돌려준다", () => {
    init();
    const titles = JSON.stringify(treeOf(root, PLAN_REL));
    assert.match(titles, /"배경"/);
    assert.match(titles, /"결정"/);
  });
});

describe("check·status", () => {
  test("동기화 직후 OKF 위반 0·누락/고아/stale 0", () => {
    synced();
    const r = checkWiki(root);
    assert.deepEqual(r.issues, []);
    assert.equal(r.ok, true);
    assert.equal(r.counts.needs_review, 1);
  });
  test("누락·stale·고아를 각각 보고한다", () => {
    synced();
    put("docs/plan/plan_26100104_신규.md", "# 신규\n");
    put(PLAN_REL, PLAN + "\n변경\n");
    fs.rmSync(path.join(root, DEVLOG_REL));
    const codes = checkWiki(root).issues.map((i) => `${i.code} ${i.path}`).sort();
    assert.deepEqual(codes, [
      `card.missing docs/plan/plan_26100104_신규.md`,
      `card.orphan ${DEVLOG_REL}`,
      `card.stale ${PLAN_REL}`,
    ]);
  });
  test("손상 카드는 card.invalid로 보고하고 검사를 계속한다", () => {
    synced();
    put(card(PLAN_REL), read(card(PLAN_REL)).replace(/^needs_review: false$/m, "needs_review: maybe"));
    put("docs/plan/plan_26100104_신규.md", "# 신규\n");
    const codes = checkWiki(root).issues.map((i) => i.code).sort();
    assert.ok(codes.includes("card.invalid"));
    assert.ok(codes.includes("card.missing"));
  });
  test("카드 디렉터리의 규칙 밖 파일은 card.unrecognized", () => {
    synced();
    put("__llm-wiki/cards/docs/mine.md", "---\ntype: note\n---\n# 내 메모\n");
    assert.ok(checkWiki(root).issues.some((i) => i.code === "card.unrecognized"));
  });
  test("status는 카드·원문 수를 요약한다", () => {
    synced();
    const s = statusWiki(root);
    assert.equal(s.sources, 3);
    assert.equal(s.cards, 3);
  });
});

describe("remove", () => {
  test("기본은 계획만(쓰기 0), 적용은 소유·미변경 생성물과 캐시만 지우고 계약·수정 카드·사용자 파일·원문 보존", () => {
    synced();
    queryWiki(root, ["멀티턴"]);
    fs.appendFileSync(path.join(root, card(PLAN_REL)), "\n사용자 메모\n");
    put("__llm-wiki/cards/notes/mine.md", "내 파일\n");
    const before = snapshot("docs");
    const wiki = snapshot("__llm-wiki");
    const plan = removeWiki(root, { apply: false });
    assert.equal(plan.applied, false);
    assert.ok(plan.preserved.includes(card(PLAN_REL)));
    assert.deepEqual(snapshot("__llm-wiki"), wiki);
    removeWiki(root, { apply: true });
    assert.ok(exists("__llm-wiki/contract.json"));
    assert.ok(exists(card(PLAN_REL)));
    assert.ok(exists("__llm-wiki/cards/notes/mine.md"));
    assert.equal(exists(card(DEVLOG_REL)), false);
    assert.equal(exists("__llm-wiki/.cache"), false);
    assert.deepEqual(snapshot("docs"), before);
  });
  for (const [name, forged] of [["원문 경로", PLAN_REL], ["루트 밖 경로", "../outside.md"], ["위키 안이지만 해시 아님", "__llm-wiki/x.md"]] as const) {
    test(`receipt 위조(${name})는 입력 계약 거부 — 원문 불변`, () => {
      synced();
      const receipt = JSON.parse(read("__llm-wiki/receipt.json"));
      receipt.files[forged] = forged === "__llm-wiki/x.md" ? "not-a-hash" : "0".repeat(64);
      put("__llm-wiki/receipt.json", JSON.stringify(receipt));
      const before = snapshot("docs");
      assert.throws(() => removeWiki(root, { apply: true }), /receipt/);
      assert.deepEqual(snapshot("docs"), before);
    });
  }
  test("cache_dir에 하위 디렉터리가 있어도 제거가 멈추지 않는다", () => {
    synced();
    queryWiki(root, ["멀티턴"]);
    fs.mkdirSync(path.join(root, "__llm-wiki/.cache/sub"));
    removeWiki(root, { apply: true });
    assert.equal(exists(card(DEVLOG_REL)), false);
  });
});

describe("소유권 이동·제거 후 재동기화", () => {
  for (const mode of ["accept", "relocate"] as const) {
    test(`사람이 고친 고아 카드는 보존 이동(${mode}) 후에도 remove가 지우지 않는다`, () => {
      synced();
      fs.appendFileSync(path.join(root, card(DEVLOG_REL)), "\n사람이 쓴 메모\n");
      const neu = "docs/devlog/devlog_26100103_세션.md";
      if (mode === "accept") fs.rmSync(path.join(root, DEVLOG_REL));
      else fs.renameSync(path.join(root, DEVLOG_REL), path.join(root, neu));
      syncWiki(root, mode === "accept"
        ? { apply: true, now: LATER, acceptRemoved: [DEVLOG_REL] }
        : { apply: true, now: LATER, relocate: [[DEVLOG_REL, neu]] });
      const archived = `__llm-wiki/archive/${DEVLOG_REL}.card.md`;
      assert.ok(removeWiki(root, { apply: false }).preserved.includes(archived));
      removeWiki(root, { apply: true });
      assert.match(read(archived), /사람이 쓴 메모/);
    });
  }
  test("미수정 고아 카드의 보존본은 계속 소유물로 남아 remove가 지운다", () => {
    synced();
    fs.rmSync(path.join(root, DEVLOG_REL));
    syncWiki(root, { apply: true, now: LATER, acceptRemoved: [DEVLOG_REL] });
    removeWiki(root, { apply: true });
    assert.equal(exists(`__llm-wiki/archive/${DEVLOG_REL}.card.md`), false);
  });
  test("remove 뒤 sync --apply로 위키를 다시 채울 수 있다(사람 수정 카드는 protected)", () => {
    synced();
    fs.appendFileSync(path.join(root, card(PLAN_REL)), "\n사용자 메모\n");
    removeWiki(root, { apply: true });
    const r = syncWiki(root, { apply: true, now: LATER });
    assert.ok(r.protected.includes(card(PLAN_REL)));
    assert.ok(r.created.includes(DEVLOG_REL));
    assert.ok(exists("__llm-wiki/index.md") && exists("__llm-wiki/log.md"));
    assert.match(read("__llm-wiki/.gitignore"), /^\.cache\/$/m); // 캐시가 git에 드러나지 않게 재생성
    assert.ok(Object.hasOwn(JSON.parse(read("__llm-wiki/receipt.json")).files, "__llm-wiki/.gitignore"));
    assert.deepEqual(checkWiki(root).issues, []);
  });
  test("receipt 없음은 계획(dry-run)에서도 거부한다", () => {
    synced();
    fs.rmSync(path.join(root, "__llm-wiki/receipt.json"));
    assert.throws(() => syncWiki(root, { apply: false, now: LATER }), /receipt/);
    assert.throws(() => removeWiki(root, { apply: false }), /receipt/);
  });
});

describe("cli", () => {
  const run = (...args: string[]) => execFileSync(process.execPath, [CLI, ...args, "--root", root], { encoding: "utf8" });
  const runRaw = (...args: string[]) => spawnSync(process.execPath, [CLI, ...args, "--root", root], { encoding: "utf8" });
  test("JSON을 stdout으로, 오류는 exit≠0", () => {
    run("init", "--contract", CONTRACT, "--apply");
    run("sync", "--apply");
    const out = JSON.parse(run("query", "--terms", "멀티턴", "--terms", "multi-turn"));
    assert.ok(out.results.length > 0);
    assert.notEqual(runRaw("read", "nope.md:1-2").status, 0);
  });
  test("sync는 --apply 없이 쓰지 않는다", () => {
    run("init", "--contract", CONTRACT, "--apply");
    run("sync");
    assert.equal(exists("__llm-wiki/cards"), false);
  });
  test("check는 문제가 있으면 JSON을 내고 exit 1", () => {
    run("init", "--contract", CONTRACT, "--apply");
    const r = runRaw("check");
    assert.equal(r.status, 1);
    assert.equal(JSON.parse(r.stdout).ok, false);
  });
  test("입력 계약 거부: 알 수 없는 명령·프로토타입 키·--relocate 형식·--limit", () => {
    run("init", "--contract", CONTRACT, "--apply");
    for (const args of [["nope"], ["toString"], ["constructor"], ["sync", "--relocate", "a"], ["query", "--terms", "x1", "--limit", "0"], ["query", "--terms", "x1", "--limit", "abc"]]) {
      assert.notEqual(runRaw(...args).status, 0, args.join(" "));
    }
  });
});
