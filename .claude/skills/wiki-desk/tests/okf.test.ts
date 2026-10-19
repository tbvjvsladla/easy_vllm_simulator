import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { checkConcept, parseCard, renderCard, roundTrip, trustTier, tryParseCard, verifiedEvents } from "../src/okf.ts";
import { bodyText, parseYaml, splitFrontmatter } from "../src/frontmatter.ts";
import type { Card, Frontmatter } from "../src/types.ts";

/** kind="ok"를 단언하고 data를 꺼낸다 */
const okData = (fm: Frontmatter): Record<string, unknown> => {
  assert.equal(fm.kind, "ok", JSON.stringify(fm));
  return (fm as Extract<Frontmatter, { kind: "ok" }>).data;
};

// OKF 0.2 예시 번들 픽스처(출처: tests/fixtures/okf/ATTRIBUTION.md)
const fixture = (name: string): string => readFileSync(new URL(`./fixtures/okf/${name}`, import.meta.url), "utf8");
const FIXTURES: [string, string][] = [
  ["acme_index.md", "index.md"],
  ["acme_log.md", "log.md"],
  ["acme_revenue.md", "metrics/revenue.md"],
  ["acme_revenue_ytd.md", "computations/revenue-ytd.md"],
  ["ga4_purchasers.md", "references/metrics/purchasers.md"],
  ["so_content_licenses.md", "references/content_licenses.md"],
];
const codes = (text: string, rel: string): string[] => checkConcept(text, rel).map((i) => i.code);

test("OKF 예시 번들 픽스처 6건은 문제 없음", () => {
  for (const [file, rel] of FIXTURES) assert.deepEqual(checkConcept(fixture(file), rel), [], file);
});

// §4.1 "type: A short string ... # REQUIRED" / §11-2 "Every frontmatter block contains a non-empty `type` field."
test("§4.1 REQUIRED type: 부재·빈 문자열·비문자열이면 okf.missing_type", () => {
  assert.deepEqual(codes("---\ntitle: x\n---\n", "a.md"), ["okf.missing_type"]);
  assert.deepEqual(codes("---\ntype: '  '\n---\n", "a.md"), ["okf.missing_type"]);
  assert.deepEqual(codes("---\ntype: 3\n---\n", "a.md"), ["okf.missing_type"]);
  const issue = checkConcept("---\n---\n", "dir/a.md")[0];
  assert.deepEqual(issue, { code: "okf.missing_type", path: "dir/a.md", detail: "type(비어있지 않은 문자열) 필수" });
});

// §11-1 "Every non-reserved .md file in the tree contains a parseable YAML frontmatter block."
test("§11-1 concept는 파싱 가능한 frontmatter 필요", () => {
  assert.deepEqual(codes("# 제목만\n", "a.md"), ["okf.missing_frontmatter"]);
  const parse = checkConcept("---\na: 1\na: 2\n---\n", "a.md");
  assert.deepEqual(parse, [{ code: "okf.frontmatter_parse", path: "a.md", detail: "2행: 중복 키: a" }]);
  // 닫힘 없음도 throw 대신 문제 1건으로 표면화(다른 문서 검사를 막지 않음)
  assert.deepEqual(checkConcept("---\ntype: R\n", "a.md"), [
    { code: "okf.frontmatter_parse", path: "a.md", detail: "frontmatter 닫힘 구분선(---) 없음" },
  ]);
});

// §4.1 "a concept carrying just `type` is fully conformant (§11)."
test("§4.1 type 하나만 가진 concept는 적합", () => {
  assert.deepEqual(checkConcept("---\ntype: Reference\n---\n", "a.md"), []);
});

// §4.1 "consumers MUST tolerate unknown types gracefully" / §11 "MUST NOT reject ... Unknown `type` values."
test("§4.1 MUST tolerate unknown types", () => {
  assert.deepEqual(checkConcept("---\ntype: Unregistered Alien Type\n---\n", "a.md"), []);
});

// §4.1 "Consumers ... MUST NOT reject documents with unrecognized fields." / §11 "Unknown additional frontmatter keys."
test("§4.1 MUST NOT reject documents with unrecognized fields", () => {
  const text = "---\ntype: R\nx-extension: {nested: [one, 2]}\ncycle: v3\n01_custom:\n  deep: {a: 1}\n---\n";
  assert.deepEqual(checkConcept(text, "a.md"), []);
});

// §4.1 "Consumers SHOULD preserve unknown keys when round-tripping" + 본문 바이트 보존
test("roundTrip: unknown 키·키 순서·본문 바이트 보존", () => {
  const body = "\n# 사실\n\n```python\nx = \"[[target]]\"\n```\r\n끝  \n";
  const text = "---\ntype: Unregistered\ncreated: 2026-06-09\nzeta: {nested: [one, 2]}\nalpha: 'a: b'\n---" + "\n" + body;
  const out = roundTrip(text);
  const before = splitFrontmatter(text);
  const after = splitFrontmatter(out);
  assert.deepEqual(okData(after), okData(before));
  assert.deepEqual(Object.keys(okData(after)), ["type", "created", "zeta", "alpha"]);
  assert.equal(bodyText(out, after), bodyText(text, before));
  assert.equal(roundTrip(out), out); // 고정점
});

test("roundTrip: 예시 번들 전부 의미 동등 + 본문 동일, frontmatter 없으면 원문 그대로", () => {
  for (const [file] of FIXTURES) {
    const text = fixture(file);
    const out = roundTrip(text);
    const a = splitFrontmatter(text);
    const b = splitFrontmatter(out);
    assert.deepEqual(b.kind === "ok" ? b.data : b, a.kind === "ok" ? a.data : a, file);
    assert.equal(bodyText(out, b), bodyText(text, a), file);
  }
  const plain = fixture("acme_index.md");
  assert.equal(roundTrip(plain), plain);
});

test("roundTrip: frontmatter 파싱 불가·닫힘 없음이면 throw(원문을 다시 쓰지 않음)", () => {
  assert.throws(() => roundTrip("---\na: &x 1\n---\n"), /YAML 파싱 실패: 1행: 앵커/);
  assert.throws(() => roundTrip("---\na: 1\n"), /YAML 파싱 실패: frontmatter 닫힘 구분선/);
});

// §10.3 "The agent MAY only supply values for the declared parameters; it MUST NOT author or edit the computation."
test("§10.3 MUST NOT edit the computation: Attested Computation 본문 fence는 바이트 그대로", () => {
  const text = fixture("acme_revenue_ytd.md");
  const out = roundTrip(text);
  const fence = /```sql\n[\s\S]*?```/.exec(text)?.[0];
  assert.ok(fence !== undefined);
  assert.ok(out.includes(fence));
  assert.equal(bodyText(out, splitFrontmatter(out)), bodyText(text, splitFrontmatter(text)));
});

// §10.2 "runtime: REQUIRED for this type."
test("§10.2 REQUIRED runtime: Attested Computation에 runtime 없으면 okf.runtime_missing", () => {
  assert.deepEqual(codes("---\ntype: Attested Computation\n---\n", "c.md"), ["okf.runtime_missing"]);
  assert.deepEqual(codes("---\ntype: Attested Computation\nruntime: bigquery\n---\n", "c.md"), []);
});

// §5.1 "`resource`: REQUIRED within an entry."
test("§5.1 REQUIRED sources[].resource", () => {
  assert.deepEqual(codes("---\ntype: R\nsources:\n  - id: x\n  - resource: all queries in project X\n---\n", "a.md"), ["okf.source_resource"]);
  assert.deepEqual(codes("---\ntype: R\nsources: ga4\n---\n", "a.md"), ["okf.sources_shape"]);
});

// §5.2 "`generated.by`: REQUIRED within `generated`."
test("§5.2 REQUIRED generated.by", () => {
  assert.deepEqual(codes("---\ntype: R\ngenerated: { at: 2026-06-20T22:53:05Z }\n---\n", "a.md"), ["okf.generated_by"]);
  assert.deepEqual(codes("---\ntype: R\ngenerated: { by: process:x }\n---\n", "a.md"), []);
});

// §5.2 / §11 "Consumers MUST treat a bare mapping as a one-element list."
test("§5.2 MUST treat a bare verified mapping as a one-element list", () => {
  const bare = parseYaml("verified: { by: human:ahormati, at: 2026-06-25T09:00:00Z }");
  const list = parseYaml("verified:\n  - { by: human:ahormati, at: 2026-06-25T09:00:00Z }");
  assert.deepEqual(verifiedEvents(bare), verifiedEvents(list));
  assert.equal(verifiedEvents(bare).length, 1);
  assert.equal(trustTier(bare), trustTier(list));
  assert.deepEqual(codes("---\ntype: R\nverified: { by: human:a, at: 2026-06-25T09:00:00Z }\n---\n", "a.md"), []);
  assert.deepEqual(codes("---\ntype: R\nverified: { at: 2026-06-25T09:00:00Z }\n---\n", "a.md"), ["okf.verified_by"]);
});

// §5.3 "A concept with no trust frontmatter is still consumable; consumers MUST NOT reject it (§11)."
// §11 "MUST NOT reject a concept for missing any optional family (§5.3)." / "Missing optional frontmatter fields."
test("§5.3 MUST NOT reject a concept without trust/optional families", () => {
  const data = parseYaml("type: R");
  assert.deepEqual(verifiedEvents(data), []);
  assert.equal(trustTier(data), "unverified");
  assert.deepEqual(checkConcept("---\ntype: R\n---\n본문\n", "a.md"), []);
});

// §7 "Consumers that classify trust (§5.3) key off the `human:` prefix, so producers MUST use it ..."
test("§5.3·§7 trust tier는 human: 접두로 판정", () => {
  assert.equal(trustTier(parseYaml("verified: [{ by: process:nightly, at: 2026-06-26T02:00:00Z }]")), "machine-confirmed");
  assert.equal(
    trustTier(parseYaml("verified:\n  - { by: process:n, at: 2026-06-26T02:00:00Z }\n  - { by: human:a, at: 2026-06-25T09:00:00Z }")),
    "human-reviewed",
  );
  assert.equal(trustTier(parseYaml(fixture("acme_revenue.md").split("---\n")[1])), "human-reviewed");
});

// §6.1 "Consumers MUST tolerate broken links" / §11 "Broken cross-links." "Missing `index.md` files."
test("§6.1 MUST tolerate broken links (index.md 부재도 무관)", () => {
  const text = "---\ntype: R\n---\n[없음](/tables/not-yet.md) [상대](../nope.md) [[wiki-missing]]\n";
  assert.deepEqual(checkConcept(text, "deep/dir/without/index/a.md"), []);
});

// §3.1 "The following filenames ... MUST NOT be used for concept documents" — index.md·log.md는 concept 규칙(type) 비적용
test("§3.1 예약 파일명 index.md·log.md는 concept로 검사하지 않는다", () => {
  assert.deepEqual(checkConcept("# Index\n\n* [a](a.md) - 설명\n", "index.md"), []);
  assert.deepEqual(checkConcept("# Log\n", "sub/log.md"), []);
  assert.deepEqual(codes("# 제목만\n", "sub/concept.md"), ["okf.missing_frontmatter"]);
});

// §8 "Index files contain no frontmatter, with one exception: a bundle-root `index.md` MAY carry an `okf_version` key"
test("§8 index.md frontmatter: 루트만 okf_version 허용", () => {
  assert.deepEqual(checkConcept('---\nokf_version: "0.2"\n---\n# I\n', "index.md"), []);
  assert.deepEqual(codes('---\nokf_version: "0.2"\n---\n# I\n', "sub/index.md"), ["okf.index_frontmatter"]);
  assert.deepEqual(codes("---\ntype: R\nokf_version: '0.2'\n---\n# I\n", "index.md"), ["okf.index_frontmatter"]);
});

// §9 "Date headings MUST use ISO 8601 `YYYY-MM-DD` form."
test("§9 MUST: 로그 날짜 헤딩은 YYYY-MM-DD (펜스 코드 안은 무시)", () => {
  const ok = "# Log\n\n## 2026-05-22\n* **Update**: x\n\n```md\n## not a date\n```\n\n## 2026-05-15\n";
  assert.deepEqual(checkConcept(ok, "log.md"), []);
  const bad = "# Log\n\n## [2026-06-09] init | x\n## 2026-02-30\n## 26-05-22\n### 하위 헤딩은 대상 아님\n";
  assert.deepEqual(
    checkConcept(bad, "a/log.md").map((i) => i.detail),
    ["로그 날짜 헤딩은 YYYY-MM-DD여야 함: [2026-06-09] init | x", "로그 날짜 헤딩은 YYYY-MM-DD여야 함: 2026-02-30", "로그 날짜 헤딩은 YYYY-MM-DD여야 함: 26-05-22"],
  );
});

const CARD: Card = {
  type: "plan",
  title: "예시 검색도구 도입 설계",
  description: null,
  resource: "docs/plan/plan_26090109_예시_검색도구_도입_설계.md",
  tags: ["plan"],
  generated: { by: "process:wiki-desk-ts", at: "2026-10-14T09:00:00+09:00" },
  authority: { document_type: "plan", authority_rank: 4, role: "세부 계획" },
  source_meta: { created: "2026-10-14", related_issued: null, branches: ["branch-a", "branch-b"] },
  source_meta_unknown: { "x-weird key": { nested: ["123", 123, "null", true, "줄\n바꿈"] }, related_interview: "a: b # c" },
  needs_review: false,
  review_reasons: [],
  source_sha256: "ab".repeat(32),
  extra: {},
};

test("renderCard → parseCard 왕복은 카드 원형 보존(source_meta_unknown 포함)", () => {
  const text = renderCard(CARD);
  assert.deepEqual(parseCard(text, "cards/x.md"), CARD);
  assert.ok(text.startsWith("---\ntype: plan\ntitle: 예시 검색도구 도입 설계\n"));
  assert.ok(text.endsWith("\n# 예시 검색도구 도입 설계\n\n- 원문: `docs/plan/plan_26090109_예시_검색도구_도입_설계.md`\n"));
  assert.deepEqual(checkConcept(text, "cards/x.md"), []); // 카드 자체가 OKF concept
  const reviewed = { ...CARD, title: null, needs_review: true, review_reasons: ["파일명 규칙 위반: x"] };
  assert.deepEqual(parseCard(renderCard(reviewed), "cards/x.md"), reviewed);
  assert.match(renderCard(reviewed), /\n# docs\/plan\/plan_26090109/);
});

const REL = "cards/docs/plan/x.md";
const errorOf = (text: string): string => {
  const r = tryParseCard(text, REL);
  assert.ok("error" in r, JSON.stringify(r));
  return r.error;
};
const RENDERED = renderCard(CARD);

test("tryParseCard: 결과형 — 성공은 card, 실패는 rel 포함 error(throw 없음)", () => {
  assert.deepEqual(tryParseCard(RENDERED, REL), { card: CARD });
  const cases: [string, RegExp][] = [
    ["# 본문만\n", /frontmatter 없음/],
    ["---\ntype: plan\n", /닫힘 구분선/],
    ["---\na: &x 1\n---\n", /1행: 앵커/],
    [RENDERED.replace("needs_review: false", "needs_review: maybe"), /카드 필드 형식 오류: needs_review/],
    [RENDERED.replace(/source_sha256: .*\n/, ""), /카드 필드 형식 오류: source_sha256/],
    [RENDERED.replace("authority_rank: 4", "authority_rank: four"), /카드 필드 형식 오류: authority.authority_rank/],
  ];
  for (const [text, re] of cases) {
    const e = errorOf(text);
    assert.match(e, re, text);
    assert.ok(e.includes(REL), e);
  }
});

test("tryParseCard: 불가능한 카드 거부(review 불일치·at offset·rank 비정수·role 빈값·type∉tags)", () => {
  const cases: [string, RegExp][] = [
    [RENDERED.replace("needs_review: false", "needs_review: true"), /needs_review.*review_reasons/],
    [renderCard({ ...CARD, review_reasons: ["사유"] }), /needs_review.*review_reasons/],
    [RENDERED.replace("2026-10-14T09:00:00+09:00", "2026-10-14T09:00:00"), /카드 필드 형식 오류: generated.at/],
    [RENDERED.replace('"2026-10-14T09:00:00+09:00"', "yesterday"), /카드 필드 형식 오류: generated.at/],
    [RENDERED.replace("authority_rank: 4", "authority_rank: 4.5"), /카드 필드 형식 오류: authority.authority_rank/],
    [RENDERED.replace("role: 세부 계획", 'role: ""'), /카드 필드 형식 오류: authority.role/],
    [renderCard({ ...CARD, tags: ["other"] }), /type\(plan\)이 tags에 없음/],
  ];
  for (const [text, re] of cases) {
    assert.notEqual(text, RENDERED);
    const e = errorOf(text);
    assert.match(e, re, text);
    assert.ok(e.startsWith(`${REL}: `), e);
  }
});

test("parseCard: tryParseCard 오류를 같은 메시지로 throw", () => {
  assert.throws(() => parseCard("# 본문만\n", REL), new RegExp(`^Error: ${REL}: 카드에 frontmatter 없음`.replace(/\//g, "\\/")));
  assert.throws(() => parseCard(RENDERED.replace("authority_rank: 4", "authority_rank: four"), REL), /cards\/docs\/plan\/x\.md: .*authority_rank/);
});

// OKF §4.1 "Consumers SHOULD preserve unknown keys when round-tripping" — 사람이 카드에 추가한 키(verified 등)
test("parseCard·renderCard: CARD_KEYS 밖 최상위 키는 extra로 원형·순서 보존, 재렌더 시 뒤에 다시 쓴다", () => {
  const edited = RENDERED.replace(
    "title: ",
    "verified:\n  - { by: human:jsmith, at: 2026-10-15T09:00:00+09:00 }\nx-note: 사람 메모\ntitle: ",
  ).replace("---\n\n# ", "zeta: [1, two]\n---\n\n# ");
  const card = parseCard(edited, REL);
  assert.deepEqual(card.extra, {
    verified: [{ by: "human:jsmith", at: "2026-10-15T09:00:00+09:00" }],
    "x-note": "사람 메모",
    zeta: [1, "two"],
  });
  const { extra, ...rest } = card;
  assert.deepEqual({ ...rest, extra: {} }, CARD);
  const out = renderCard(card);
  const keys = Object.keys(okData(splitFrontmatter(out)));
  assert.deepEqual(keys.slice(-3), ["verified", "x-note", "zeta"]);
  assert.deepEqual(keys.slice(0, 2), ["type", "title"]);
  assert.deepEqual(parseCard(out, REL), card);
  assert.equal(trustTier(okData(splitFrontmatter(out))), "human-reviewed");
  assert.deepEqual(checkConcept(out, "cards/x.md"), []);
});

test("renderCard: extra 키가 카드 키와 겹치면 throw", () => {
  assert.throws(() => renderCard({ ...CARD, extra: { type: "x" } }), /extra 키가 카드 키와 겹침: type/);
});
