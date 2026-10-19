import { test } from "node:test";
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { bodyText, emitYaml, locate, parseYaml, splitFrontmatter, tryParseYaml, resolvePlain } from "../src/frontmatter.ts";
import type { Frontmatter } from "../src/types.ts";

/** kind="ok"를 단언하고 data를 꺼낸다 */
const okData = (fm: Frontmatter): Record<string, unknown> => {
  assert.equal(fm.kind, "ok", JSON.stringify(fm));
  return (fm as Extract<Frontmatter, { kind: "ok" }>).data;
};

// 관례형 frontmatter(docs/plan 형태 + 블록 스칼라·중첩 매핑 확장) — 합성 예시
const PLAN_FM = [
  "---",
  "title: 예시 검색도구 도입·전후 비교 — 설계",
  "created: 2026-10-14",
  "document_type: plan",
  "ouroboros_interview_id: interview_20260901_120000",
  "ouroboros_seed_id: seed_0123456789ab",
  "related_issued: null   # operator 직접 지시(대화) — issued SDD 없음",
  "status: issued   # 2026-09-29 P3-4에서 확정값 반영",
  "related_plan: docs/plan/plan_26080101_예시_*(설계·구현) / docs/plan/plan_26080201_*",
  "branches: [branch-a, branch-b]",
  "workspace_scope: [example-project]",
  "depends_on:",
  "  - docs/plan/plan_26082801_예시_선행_설계.md   # 개정 code-style 기준",
  "review:",
  "  owner: operator",
  "  gates:",
  "    hitl: true",
  "    rounds: 3",
  "note: |",
  "  첫 줄",
  "    들여쓴 줄",
  "",
  "  빈 줄 뒤",
  "---",
  "",
  "# 본문",
  "",
].join("\n");

test("splitFrontmatter: 관례형 plan frontmatter를 서브셋으로 파싱한다", () => {
  const fm = splitFrontmatter(PLAN_FM);
  assert.equal(fm.bodyStartLine, 25);
  assert.deepEqual(okData(fm), {
    title: "예시 검색도구 도입·전후 비교 — 설계",
    created: "2026-10-14", // ISO 날짜는 문자열 유지
    document_type: "plan",
    ouroboros_interview_id: "interview_20260901_120000",
    ouroboros_seed_id: "seed_0123456789ab",
    related_issued: null, // 값 뒤 `# 주석` 제거
    status: "issued",
    related_plan: "docs/plan/plan_26080101_예시_*(설계·구현) / docs/plan/plan_26080201_*",
    branches: ["branch-a", "branch-b"],
    workspace_scope: ["example-project"],
    depends_on: ["docs/plan/plan_26082801_예시_선행_설계.md"],
    review: { owner: "operator", gates: { hitl: true, rounds: 3 } },
    note: "첫 줄\n  들여쓴 줄\n\n빈 줄 뒤\n",
  });
  assert.equal(bodyText(PLAN_FM, fm), "\n# 본문\n");
});

test("splitFrontmatter: frontmatter 없음 → kind none, bodyStartLine 1", () => {
  assert.deepEqual(splitFrontmatter("# 제목\n---\n본문\n"), { kind: "none", bodyStartLine: 1 });
  assert.deepEqual(splitFrontmatter(""), { kind: "none", bodyStartLine: 1 });
});

test("splitFrontmatter: CRLF·BOM 허용, 본문 바이트 보존", () => {
  const text = "\ufeff---\r\ntype: Reference\r\ntags: [a, b]\r\n---\r\n본문\r\n";
  const fm = splitFrontmatter(text);
  assert.deepEqual(okData(fm), { type: "Reference", tags: ["a", "b"] });
  assert.equal(fm.bodyStartLine, 5);
  assert.equal(bodyText(text, fm), "본문\r\n");
});

const UNCLOSED = { kind: "error", raw: null, error: "frontmatter 닫힘 구분선(---) 없음", bodyStartLine: 1 };

test("splitFrontmatter: 닫힘 구분선이 없으면 throw하지 않고 kind error(raw null)", () => {
  const text = "---\ntype: R\n본문\n";
  assert.deepEqual(splitFrontmatter(text), UNCLOSED);
  assert.deepEqual(splitFrontmatter("---"), UNCLOSED);
  assert.deepEqual(splitFrontmatter("\ufeff---\r\n"), UNCLOSED);
  assert.equal(bodyText(text, splitFrontmatter(text)), text); // 본문 = 원문 전체
});

test("splitFrontmatter: YAML 파싱 실패 → kind error(raw·사유·bodyStartLine 유지)", () => {
  assert.deepEqual(splitFrontmatter('---\ntitle: "a" 뒤\n---\n본문\n'), {
    kind: "error",
    raw: 'title: "a" 뒤\n',
    error: "1행: 따옴표 스칼라 뒤 잔여 내용",
    bodyStartLine: 4,
  });
});

test("locate: 결과형(none·unclosed·found), throw 없음, bodyStartLine 항상 제공", () => {
  assert.deepEqual(locate("# x\n"), { kind: "none", bodyStartLine: 1 });
  assert.deepEqual(locate("---\na: 1\n"), { kind: "unclosed", bodyStartLine: 1 });
  assert.deepEqual(locate("---\na: 1\r\nb: 2\n--- \nbody"), { kind: "found", raw: "a: 1\r\nb: 2\n", bodyStartLine: 5 });
  assert.deepEqual(locate("\ufeff---\n---\n"), { kind: "found", raw: "", bodyStartLine: 3 });
  assert.deepEqual(locate("--- x\n---\n"), { kind: "none", bodyStartLine: 1 });
});

test("splitFrontmatter: 빈 frontmatter는 빈 매핑, 닫는 줄 뒤 본문 없음", () => {
  const fm = splitFrontmatter("---\n---");
  assert.deepEqual(okData(fm), {});
  assert.equal(fm.bodyStartLine, 3);
  assert.equal(bodyText("---\n---", fm), "");
});

test("parseYaml: 스칼라 해석(YAML 1.2 core 서브셋)", () => {
  const data = parseYaml(
    [
      "s: plain text",
      "q1: 'it''s'",
      'q2: "tab\\tnl\\n\\u00e9 \\"q\\""',
      "i: 42",
      "neg: -7",
      "f: 0.5",
      "e: 1e3",
      "big: 12345678901234567890",
      "t: true",
      "fl: False",
      "n1: ~",
      "n2:",
      "yes_word: yes",
      "date: 2026-10-14",
      "ts: 2026-06-30T14:00:00Z",
      "ver: '0.2'",
      "url: https://example.com/a#frag",
      "hash: a#b",
    ].join("\n"),
  );
  assert.deepEqual(data, {
    s: "plain text",
    q1: "it's",
    q2: 'tab\tnl\né "q"',
    i: 42,
    neg: -7,
    f: 0.5,
    e: 1000,
    big: "12345678901234567890", // 안전 정수 범위 밖은 문자열 유지
    t: true,
    fl: false,
    n1: null,
    n2: null,
    yes_word: "yes",
    date: "2026-10-14",
    ts: "2026-06-30T14:00:00Z",
    ver: "0.2",
    url: "https://example.com/a#frag",
    hash: "a#b",
  });
});

test("parseYaml: 블록 리스트(같은 들여쓰기·콤팩트 매핑·중첩 리스트)", () => {
  const data = parseYaml(
    ["tags:", "- a", "- b", "sources:", "- id: x", "  resource: https://e.com", "- id: y", "nested:", "  - - 1", "    - 2", "  -", "    - 3"].join("\n"),
  );
  assert.deepEqual(data, {
    tags: ["a", "b"],
    sources: [{ id: "x", resource: "https://e.com" }, { id: "y" }],
    nested: [[1, 2], [3]],
  });
});

test("parseYaml: 흐름 매핑·시퀀스(OKF generated/verified 표기)", () => {
  const data = parseYaml(
    [
      "generated: { by: reference_agent/gemini-2.5-pro, at: 2026-06-30T14:00:00Z }",
      "verified:",
      "  - { by: human:jsmith@acme, at: 2026-07-01T09:00:00Z }",
      "executor:",
      "  receipt: [job_id, executed_sql, result]",
      "multi: [a,",
      "  'b, c', {k: [1, 2]}]   # 주석",
      "empty: { }",
      "emptyl: []",
    ].join("\n"),
  );
  assert.deepEqual(data, {
    generated: { by: "reference_agent/gemini-2.5-pro", at: "2026-06-30T14:00:00Z" },
    verified: [{ by: "human:jsmith@acme", at: "2026-07-01T09:00:00Z" }],
    executor: { receipt: ["job_id", "executed_sql", "result"] },
    multi: ["a", "b, c", { k: [1, 2] }],
    empty: {},
    emptyl: [],
  });
});

test("parseYaml: plain 다중 줄 접힘(OKF ga4 description 형태)", () => {
  const data = parseYaml("description: Computes the count or\n  in-app purchase.\nnext: x");
  assert.deepEqual(data, { description: "Computes the count or in-app purchase.", next: "x" });
});

test("parseYaml: 블록 스칼라 | > 와 chomping", () => {
  const data = parseYaml(
    ["lit: |", "  a", "  b", "", "keep: |+", "  a", "", "strip: |-", "  a", "fold: >", "  one", "  two", "", "  three", "ind: |2", "    x", "last: 1"].join("\n"),
  );
  assert.deepEqual(data, {
    lit: "a\nb\n",
    keep: "a\n\n",
    strip: "a",
    fold: "one two\nthree\n",
    ind: "  x\n",
    last: 1,
  });
});

// PyYAML safe_load 기대값과 대조(접힘 정리·keep chomp 단순화 회귀 핀)
test("parseYaml: 접힘의 더 들여쓴 줄·선행 빈 줄, 빈 keep 블록", () => {
  assert.deepEqual(parseYaml("fold: >\n  a\n    b\n  c\nlead: >\n\n  a\n"), { fold: "a\n  b\nc\n", lead: "\na\n" });
  assert.deepEqual(parseYaml("k: |+\n\nx: 1"), { k: "\n", x: 1 });
});

test("parseYaml: 시퀀스 항목 뒤 들여쓰기 불일치는 throw, 더 깊은 '- '는 plain 연속", () => {
  assert.throws(() => parseYaml("a:\n  - k: 1\n   - b"), /3행: 시퀀스 들여쓰기 불일치/);
  assert.throws(() => parseYaml("a:\n  - k: 1\n   z: 2"), /3행: 시퀀스 들여쓰기 불일치/);
  assert.deepEqual(parseYaml("a:\n  - x\n   - y"), { a: ["x - y"] }); // PyYAML 동일
});

test("parseYaml: 지원 밖 구문은 줄 위치를 담아 throw", () => {
  const cases: [string, RegExp][] = [
    ["a: &x 1", /1행: 앵커·별칭·태그/],
    ["a: 1\nb: *x", /2행: 앵커·별칭·태그/],
    ["a: !!str 1", /1행: 앵커·별칭·태그/],
    ["a: 1\n---\nb: 2", /2행: 멀티문서/],
    ["%YAML 1.2\na: 1", /1행: 디렉티브/],
    ["a:\n\t- x", /2행: 들여쓰기에 탭/],
    ["a: 1\na: 2", /2행: 중복 키: a/],
    ["a: {b: 1, b: 2}", /중복 키: b/],
    ["a: 'open", /1행: 닫히지 않은 따옴표/],
    ["- a\n- b", /최상위는 매핑/],
    ["a: b: c", /1행: plain 값 안의 ': '/],
    ['title: "모르면" — 정본', /1행: 따옴표 스칼라 뒤 잔여 내용/],
    ["a: [1, 2", /닫히지 않은 흐름/],
    ["? a\n: b", /최상위는 매핑/],
    ["a:\n  b: 1\n   c: 2", /3행: plain 연속 줄 안의/],
    ["a:\n  b:\n    c: 1\n   d: 2", /4행: 예상치 못한 들여쓰기/],
    ['a: "\\q"', /알 수 없는 이스케이프/],
  ];
  for (const [raw, re] of cases) {
    assert.throws(() => parseYaml(raw), re, raw);
    const r = tryParseYaml(raw);
    assert.ok("error" in r, raw);
  }
});

test("tryParseYaml: 성공은 data, 실패는 error — 예외를 던지지 않는다", () => {
  assert.deepEqual(tryParseYaml("a: 1"), { data: { a: 1 } });
  assert.deepEqual(tryParseYaml("a: 1\na: 2"), { error: "2행: 중복 키: a" });
  assert.deepEqual(tryParseYaml("# 주석만\n\n"), { data: {} });
});

test("parseYaml: __proto__ 키는 일반 데이터로 저장(프로토타입 오염 없음)", () => {
  const data = parseYaml("__proto__: x\nb: 1");
  assert.equal(Object.getPrototypeOf(data), Object.prototype);
  assert.deepEqual(Object.keys(data), ["__proto__", "b"]);
  assert.equal(({} as Record<string, unknown>).x, undefined);
});

test("emitYaml: 결정론·키 순서 유지·왕복 의미 보존", () => {
  const obj: Record<string, unknown> = {
    z: 1,
    a: "plain",
    tricky: ["123", "null", "true", "- dash", "a: b", "x #c", "end:", " pad", "", "줄\n바꿈", "'q'", "2026-10-14", "0.2", "tab\tin"],
    nested: { m: { k: [{ id: "x", list: [1, [2, 3]], empty: {} }, []] } },
    nums: [0, -1.5, 1e21],
    flags: [true, false, null],
    "key with space": "v",
    "a: key": "colon key",
    "": "empty key",
  };
  const out = emitYaml(obj);
  assert.equal(out, emitYaml(obj)); // 결정론
  assert.deepEqual(Object.keys(parseYaml(out)), Object.keys(obj));
  assert.deepEqual(parseYaml(out), obj);
});

test("emitYaml: 블록 표기 형태(빈 컬렉션은 흐름 표기)", () => {
  assert.equal(
    emitYaml({ type: "Plan", tags: ["plan", "x"], source_meta: {}, g: { by: "process:a", at: "2026-10-14T00:00:00Z" }, l: [{ a: 1, b: 2 }] }),
    [
      "type: Plan",
      "tags:",
      "  - plan",
      "  - x",
      "source_meta: {}",
      "g:",
      "  by: process:a",
      '  at: "2026-10-14T00:00:00Z"', // 시각은 YAML 1.1 timestamp로 읽히지 않게 따옴표
      "l:",
      "  - a: 1",
      "    b: 2",
      "",
    ].join("\n"),
  );
  assert.equal(emitYaml({}), "");
});

test("emitYaml: 표현 불가 값은 위치를 담아 throw", () => {
  assert.throws(() => emitYaml({ a: undefined }), /출력 불가 값\(undefined\): \$\.a/);
  assert.throws(() => emitYaml({ a: [Number.NaN] }), /비유한 수\): \$\.a\[0\]/);
  assert.throws(() => emitYaml({ a: new Date(0) }), /출력 불가 객체: \$\.a/);
  assert.throws(() => emitYaml([] as unknown as Record<string, unknown>), /최상위는 일반 객체/);
});

// YAML 1.1(PyYAML)·1.2 core 어느 쪽에서든 비문자열로 읽힐 수 있는 문자열
const AMBIGUOUS = [
  "0x1F", "0o17", "017", "0b101", "1_000", "+1", "-1", "0", "3.14", "1.", ".5", "+.5", "1e3", "1E-3",
  ".inf", "-.Inf", "+.INF", ".nan", ".NaN", "1:20", "12:30:45",
  "y", "Y", "n", "N", "yes", "Yes", "YES", "no", "No", "NO", "on", "On", "ON", "off", "Off", "OFF",
  "true", "True", "TRUE", "tRUE", "false", "fAlse", "null", "Null", "NULL", "nULL", "~",
  "2026-10-14", "2026-10-14T09:00:00+09:00", "2026-10-14 09:00:00", "2026-10-14t09:00:00z", "=", "<<",
];

test("emitYaml: 모호한 문자열은 따옴표 — 자체 파서 왕복", () => {
  const obj = Object.fromEntries(AMBIGUOUS.map((s) => [s, s]));
  const out = emitYaml(obj);
  for (const s of AMBIGUOUS) assert.ok(out.includes(`${JSON.stringify(s)}: ${JSON.stringify(s)}\n`), s);
  assert.deepEqual(parseYaml(out), obj);
  assert.equal(emitYaml({ a: "plain", b: "007_agent는" }), "a: plain\nb: \"007_agent는\"\n");
});

// python3·PyYAML 필요 — 부재 시 실패로 남긴다(면제 아님)
test("emitYaml: PyYAML safe_load로 읽어도 키·값이 문자열로 남는다", () => {
  const obj = Object.fromEntries(AMBIGUOUS.map((s) => [s, s]));
  const script = [
    "import sys, json, yaml",
    "d = yaml.safe_load(sys.stdin.read())",
    "print(json.dumps([[k if isinstance(k, str) else repr(k), v if isinstance(v, str) else repr(v)] for k, v in d.items()]))",
  ].join("\n");
  const rows = JSON.parse(execFileSync("python3", ["-c", script], { input: emitYaml(obj), encoding: "utf8" }));
  // 정수형 키("0")는 JS 객체 순서상 앞으로 오므로 obj 키 순서로 비교
  assert.deepEqual(rows, Object.keys(obj).map((s) => [s, s]));
});

test("emitYaml: YAML 줄바꿈 문자(NEL·U+2028·U+2029)는 이스케이프되어 왕복 보존", () => {
  const obj = { c: "p\u0085q", d: "a\u2028b", e: "x\u2029y" };
  const out = emitYaml(obj);
  assert.doesNotMatch(out, /[\u0085\u2028\u2029]/);
  assert.deepEqual(parseYaml(out), obj);
});

test("resolvePlain: 넘치는 실수는 원문 문자열", () => {
  assert.equal(resolvePlain("1e999"), "1e999");
  assert.equal(resolvePlain("1.5"), 1.5);
});
