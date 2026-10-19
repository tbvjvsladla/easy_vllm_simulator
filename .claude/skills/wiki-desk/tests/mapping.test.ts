import { test } from "node:test";
import assert from "node:assert/strict";
import { FILENAME_GROUPS, PRODUCER_ACTOR, buildAliasMap, buildCard, inferFromPath, mapFrontmatter } from "../src/mapping.ts";
import { parseYaml, splitFrontmatter } from "../src/frontmatter.ts";
import { checkConcept, parseCard, renderCard } from "../src/okf.ts";
import type { AuthorityClass, Contract, SourceDoc } from "../src/types.ts";

// 테스트용 계약 값(합성 예시)
const PATTERN = "^(?<prefix>[a-z][a-z_]*?)_(?<yymmddhh>\\d{8})_(?<title>.+)\\.md$";
const ALIASES: Record<string, string[]> = {
  created: [],
  related_issued: [],
  related_plan: ["plan"],
  branches: ["branch"],
  ouroboros_interview_id: [],
  ouroboros_seed_id: [],
  status: [],
  tags: [],
  document_type: ["doc_type"],
};
const CONTRACT: Contract = {
  schema_version: 1,
  project_name: "example-project",
  wiki_dir: "__llm-wiki",
  source_roots: ["docs", "letter"],
  excluded_roots: [],
  source_suffixes: [".md"],
  authority_rules: [],
  fallback: { document_type: "reference", authority_rank: 0, role: "미분류" },
  copy_policy: "path_reference",
  frontmatter_aliases: ALIASES,
  filename_pattern: PATTERN,
  layout: { cards_dir: "cards", cache_dir: ".cache", archive_dir: "archive" },
  search: {
    bm25_k1: 1.2,
    bm25_b: 0.75,
    field_boost: { title: 3, heading: 2, meta: 1, body: 1 },
    authority_weight: 0.1,
    span_max_lines: 40,
    result_limit: 10,
    spans_per_result: 2,
  },
};
const AUTH: AuthorityClass = { document_type: "plan_rule", authority_rank: 4, role: "세부 계획" };
const NOW = "2026-10-14T09:00:00+09:00";
const PLAN_PATH = "docs/plan/plan_26090109_예시_검색도구_도입_설계.md";
const doc = (path: string, text: string): SourceDoc => ({ path, sha256: "cd".repeat(32), text });
const card = (path: string, text: string, auth: AuthorityClass = AUTH, contract: Contract = CONTRACT) =>
  buildCard(doc(path, text), splitFrontmatter(text), contract, auth, NOW, null);

test("mapFrontmatter: 별칭 흡수·슬롯 추출·미매핑 키 원형 보존", () => {
  const m = mapFrontmatter(
    { title: "T", description: "원문 설명", doc_type: "plan", plan: "p1", branch: ["a"], created: "2026-10-14", mystery: { k: 1 } },
    ALIASES,
  );
  assert.deepEqual(m, {
    source_meta: { related_plan: "p1", branches: ["a"], created: "2026-10-14" },
    source_meta_unknown: { mystery: { k: 1 } },
    title: "T",
    description: "원문 설명",
    docType: "plan",
    conflicts: [],
    blanks: [],
  });
});

test("mapFrontmatter: description은 원문 값 복사만, 없으면 null(생성 금지)", () => {
  assert.equal(mapFrontmatter({ title: "T", body: "긴 본문" }, ALIASES).description, null);
  assert.equal(mapFrontmatter({ description: null }, ALIASES).description, null);
  assert.equal(mapFrontmatter({ description: "  그대로  " }, ALIASES).description, "  그대로  ");
});

test("mapFrontmatter: 같은 정본에 다른 값 → 첫 값 유지, 나머지는 source_meta_unknown + conflicts", () => {
  const m = mapFrontmatter({ related_plan: "a", plan: "b", branches: ["x"], branch: ["x"], title: 3, document_type: "plan", doc_type: "devlog" }, ALIASES);
  assert.deepEqual(m.source_meta, { related_plan: "a", branches: ["x"] });
  assert.deepEqual(m.source_meta_unknown, { plan: "b", title: 3, doc_type: "devlog" });
  assert.deepEqual(m.conflicts, ["plan", "title", "doc_type"]);
  assert.equal(m.title, null);
  assert.equal(m.docType, "plan");
});

test("mapFrontmatter: 한 별칭이 두 정본에 걸리면 계약 오류 throw", () => {
  assert.throws(() => mapFrontmatter({}, { a: ["x"], b: ["x"] }), /중복 별칭: x/);
});

test("mapFrontmatter: 빈/공백 title·description·document_type은 null + blanks", () => {
  const m = mapFrontmatter({ title: "  ", description: "", doc_type: " \n " }, ALIASES);
  assert.deepEqual([m.title, m.description, m.docType], [null, null, null]);
  assert.deepEqual(m.blanks, ["title", "description", "doc_type"]);
  assert.deepEqual([m.source_meta_unknown, m.conflicts], [{}, []]);
});

test("mapFrontmatter: __proto__ 키도 source_meta_unknown에 일반 데이터로 보존", () => {
  const m = mapFrontmatter(parseYaml("__proto__: x\ntitle: T"), ALIASES);
  assert.deepEqual(Object.keys(m.source_meta_unknown), ["__proto__"]);
  assert.equal(Object.getOwnPropertyDescriptor(m.source_meta_unknown, "__proto__")?.value, "x");
  assert.equal(Object.getPrototypeOf(m.source_meta_unknown), Object.prototype);
});

test("buildAliasMap: 별칭·정본·슬롯 키 → 정본 역맵", () => {
  const rev = buildAliasMap({ related_plan: ["plan"], tags: [] });
  assert.deepEqual([...rev], [
    ["title", "title"],
    ["description", "description"],
    ["document_type", "document_type"],
    ["related_plan", "related_plan"],
    ["plan", "related_plan"],
    ["tags", "tags"],
  ]);
  assert.equal(buildAliasMap({ title: ["제목"] }).get("제목"), "title");
});

test("buildAliasMap: 중복 별칭(두 정본·다른 정본 이름·슬롯 키 탈취)은 throw", () => {
  assert.throws(() => buildAliasMap({ a: ["x"], b: ["x"] }), /frontmatter_aliases 중복 별칭: x \(a, b\)/);
  assert.throws(() => buildAliasMap({ a: ["b"], b: [] }), /중복 별칭: b \(a, b\)/);
  assert.throws(() => buildAliasMap({ related_plan: ["title"] }), /중복 별칭: title \(title, related_plan\)/);
});

test("FILENAME_GROUPS: 파일명 규칙 필수 named group", () => {
  assert.deepEqual([...FILENAME_GROUPS], ["prefix", "yymmddhh", "title"]);
});

test("inferFromPath: 문서구분_YYMMDDHH_한글제목.md", () => {
  assert.deepEqual(inferFromPath(PLAN_PATH, PATTERN), {
    prefix: "plan",
    yymmddhh: "26090109",
    koTitle: "예시 검색도구 도입 설계",
    matches: true,
  });
  const report = inferFromPath("docs/h_report/h_report_26081209_경영진_보고.md", PATTERN);
  assert.ok(report.matches);
  assert.equal(report.prefix, "h_report");
  assert.deepEqual(inferFromPath("docs/README.md", PATTERN), { matches: false });
  assert.throws(() => inferFromPath(PLAN_PATH, "^(?<prefix>[a-z]+)_.*$"), /named group 필요: yymmddhh, title/);
});

// TDA-3: 선택적 named group이 매치에 참여하지 않으면 필수 구성요소 누락 → 규칙 불일치
test("inferFromPath: 필수 group이 빠진 매치는 matches:false(throw·undefined 없음)", () => {
  const optionalTitle = "^(?<prefix>[a-z]+)_(?<yymmddhh>\\d{8})(?:_(?<title>.+))?\\.md$";
  assert.deepEqual(inferFromPath("docs/plan_26090109.md", optionalTitle), { matches: false });
  assert.equal(inferFromPath("docs/plan_26090109_제목.md", optionalTitle).matches, true);
  const optionalPrefix = "^(?:(?<prefix>[a-z]+)_)?(?<yymmddhh>\\d{8})_(?<title>.+)\\.md$";
  assert.deepEqual(inferFromPath("26090109_제목.md", optionalPrefix), { matches: false });
});

test("buildCard: 관례형 frontmatter 문서 → type은 원문 document_type, source_meta 매핑", () => {
  const text = "---\ntitle: 원문 제목\ncreated: 2026-10-14\ndocument_type: plan\nrelated_issued: null   # 주석\nbranches: [branch-a]\nfoo: bar\n---\n# 본문\n";
  const c = card(PLAN_PATH, text);
  assert.deepEqual(c, {
    type: "plan",
    title: "원문 제목",
    description: null,
    resource: PLAN_PATH,
    tags: ["plan"],
    generated: { by: PRODUCER_ACTOR, at: NOW },
    authority: AUTH,
    source_meta: { created: "2026-10-14", related_issued: null, branches: ["branch-a"] },
    source_meta_unknown: { foo: "bar" },
    needs_review: false,
    review_reasons: [],
    source_sha256: "cd".repeat(32),
    extra: {},
  });
  assert.deepEqual(parseCard(renderCard(c), "cards/x.md"), c);
  assert.deepEqual(checkConcept(renderCard(c), "cards/x.md"), []);
});

// OKF §7: 자동 처리 actor는 process:, human:은 사람 확인 전용
test("buildCard: generated.by는 process: actor(human: 사칭 금지)", () => {
  assert.match(PRODUCER_ACTOR, /^process:/);
  assert.equal(card(PLAN_PATH, "# x\n").generated.by.startsWith("human:"), false);
});

test("buildCard: type 도출 순서 document_type → 파일명 접두사 → authority.document_type", () => {
  assert.equal(card(PLAN_PATH, "---\ndocument_type: response\n---\n").type, "response");
  assert.equal(card(PLAN_PATH, "---\ntitle: x\n---\n").type, "plan");
  const noPrefix = card("docs/README.md", "# 안내\n");
  assert.equal(noPrefix.type, "plan_rule");
  assert.equal(noPrefix.title, null);
  assert.deepEqual(noPrefix.review_reasons, ["파일명 규칙 위반: docs/README.md"]);
  assert.equal(noPrefix.needs_review, true);
  assert.throws(() => card("docs/README.md", "# 안내\n", { ...AUTH, document_type: "" }), /type 도출 불가/);
});

test("buildCard: frontmatter 없음 → 파일명 추론 최소 카드", () => {
  const c = card(PLAN_PATH, "# 본문만\n");
  assert.equal(c.title, "예시 검색도구 도입 설계");
  assert.equal(c.description, null);
  assert.deepEqual([c.source_meta, c.source_meta_unknown, c.needs_review], [{}, {}, false]);
});

test("buildCard: frontmatter 파싱 불가 → needs_review, source_meta 비움, 제목은 파일명", () => {
  const text = '---\ntitle: "모르면 모른다고 답하라" — 정본\ndocument_type: paper\n---\n# 본문\n';
  const c = card("docs/paper/paper_26081107_RAG_abstention_통합본.md", text);
  assert.equal(c.needs_review, true);
  assert.deepEqual(c.review_reasons, ["frontmatter 파싱 불가: 1행: 따옴표 스칼라 뒤 잔여 내용"]);
  assert.deepEqual([c.source_meta, c.source_meta_unknown], [{}, {}]);
  assert.equal(c.title, "RAG abstention 통합본");
  assert.equal(c.type, "paper"); // 파싱 불가라 document_type 대신 파일명 접두사
});

test("buildCard: 키 충돌은 review 사유, 원문 tags는 카드 tags에 합침", () => {
  const c = card(PLAN_PATH, "---\nrelated_plan: a\nplan: b\ntags: [rag, plan]\n---\n");
  assert.deepEqual(c.review_reasons, ["frontmatter 키 충돌(원형은 source_meta_unknown): plan"]);
  assert.deepEqual(c.source_meta_unknown, { plan: "b" });
  assert.deepEqual(c.tags, ["plan", "rag"]);
});

// sfh-7·CA-7: tags는 별칭 등록 여부와 무관하게 원문에서 읽는다
test("buildCard: tags가 별칭에 없어도 카드 tags에 합치고 원형은 source_meta_unknown", () => {
  const { tags: _drop, ...noTags } = ALIASES;
  const contract: Contract = { ...CONTRACT, frontmatter_aliases: noTags };
  const c = card(PLAN_PATH, "---\ntags: [rag, 검색]\n---\n", AUTH, contract);
  assert.deepEqual(c.tags, ["plan", "rag", "검색"]);
  assert.deepEqual(c.source_meta_unknown, { tags: ["rag", "검색"] });
  assert.deepEqual(c.review_reasons, []);
});

test("buildCard: 문자열이 아닌 tags 원소·리스트 아닌 tags는 review 사유", () => {
  const c = card(PLAN_PATH, "---\ntags: [rag, 3, {a: 1}, null]\n---\n");
  assert.deepEqual(c.tags, ["plan", "rag"]);
  assert.deepEqual(c.review_reasons, ['tags 원소가 문자열이 아님: 3, {"a":1}, null']);
  assert.equal(c.needs_review, true);
  const scalar = card(PLAN_PATH, "---\ntags: rag\n---\n");
  assert.deepEqual(scalar.tags, ["plan"]);
  assert.deepEqual(scalar.review_reasons, ['tags가 리스트가 아님: "rag"']);
  assert.deepEqual(card(PLAN_PATH, "---\ntags: null\n---\n").review_reasons, []); // null = 값 없음
});

// sfh-12
test("buildCard: 빈/공백 title·description은 null + review 사유(title은 파일명 추론)", () => {
  const c = card(PLAN_PATH, "---\ntitle: '   '\ndescription: ''\n---\n");
  assert.equal(c.title, "예시 검색도구 도입 설계");
  assert.equal(c.description, null);
  assert.deepEqual(c.review_reasons, ["frontmatter 값이 비어 있음: title", "frontmatter 값이 비어 있음: description"]);
  assert.equal(c.needs_review, true);
});

test("buildCard: 닫힘 없는 frontmatter → needs_review, 파일명 추론", () => {
  const c = card(PLAN_PATH, "---\ntitle: x\n# 본문\n");
  assert.deepEqual(c.review_reasons, ["frontmatter 파싱 불가: frontmatter 닫힘 구분선(---) 없음"]);
  assert.deepEqual([c.source_meta, c.source_meta_unknown, c.title, c.type], [{}, {}, "예시 검색도구 도입 설계", "plan"]);
});

test("buildCard: decodeError는 review 사유로 기록", () => {
  const text = "---\ntitle: T\n---\n";
  const c = buildCard(doc(PLAN_PATH, text), splitFrontmatter(text), CONTRACT, AUTH, NOW, "UTF-8 아님(바이트 12)");
  assert.deepEqual(c.review_reasons, ["원문 UTF-8 디코딩 불가: UTF-8 아님(바이트 12)"]);
  assert.equal(c.needs_review, true);
  assert.equal(c.title, "T");
});

test("buildCard: 산출 카드는 항상 parseCard 불변식을 만족(review·type∈tags)", () => {
  for (const text of ["# x\n", "---\ntags: [a, 3]\n---\n", "---\na: &x 1\n---\n", "---\ndocument_type: memo\ntags: [memo]\n---\n"]) {
    for (const path of [PLAN_PATH, "docs/README.md"]) {
      const c = card(path, text);
      assert.deepEqual(parseCard(renderCard(c), "cards/x.md"), c, `${path} ${text}`);
    }
  }
});

test("buildCard: nowIso는 UTC offset 포함 ISO 8601이어야 함", () => {
  for (const bad of ["2026-10-14", "2026-10-14T09:00:00", "now"]) {
    assert.throws(() => buildCard(doc(PLAN_PATH, ""), splitFrontmatter(""), CONTRACT, AUTH, bad, null), /UTC offset/);
  }
});

test("buildCard: 파일명에서 추론한 제목이 공백뿐이면 title null", () => {
  const src = { path: "docs/plan/plan_26090109__.md", sha256: "a".repeat(64), text: "본문\n" };
  const card = buildCard(src, splitFrontmatter(src.text), CONTRACT, { document_type: "plan", authority_rank: 1, role: "intent" }, "2026-10-14T00:00:00Z", null);
  assert.equal(card.title, null);
});
