import { test } from "node:test";
import assert from "node:assert/strict";
import type { IndexDoc, SearchConfig } from "../src/types.ts";
import { buildIndex, FIELDS, INDEX_FORMAT_VERSION, loadOrRebuild, scoreQuery, serializeIndex, termWeights } from "../src/index.ts";
import type { Index } from "../src/index.ts";
import { queryTokens } from "../src/tokenize.ts";
import { sha256 } from "../src/fs-safety.ts";

const CFG: SearchConfig = {
  bm25_k1: 1.2,
  bm25_b: 0.75,
  field_boost: { title: 3, heading: 2, meta: 1.5, body: 1 },
  authority_weight: 0.1,
  span_max_lines: 8,
  result_limit: 10,
  spans_per_result: 2,
};

/** 호출자(commands) 경로 재현: term group → 토큰 → idf 가중치 1회 계산 → 점수 */
function sq(index: Index, groups: string[], cfg: SearchConfig): { path: string; score: number }[] {
  return scoreQuery(index, termWeights(index, queryTokens(groups)), cfg);
}

function doc(path: string, body: string, extra: Partial<IndexDoc> = {}): IndexDoc {
  const lines = ["---", "type: devlog", "---", ...body.split("\n")];
  return { path, sha256: sha256(`${path}\n${body}`), title: "", metaText: "", lines, bodyStartLine: 4, authorityRank: 0, ...extra };
}

const CORPUS: IndexDoc[] = [
  doc("a.md", "# 임베딩 차원\ngemini-embedding-2의 출력 차원은 3072다.\n\n무관한 문단", { title: "임베딩 설정" }),
  doc("b.md", "# 멀티턴\nmulti-turn 이력은 checkpointer로 관리한다.\n\nagent_config.yaml의 multi_turn 키"),
  doc("c.md", "# 기타\n서빙 포트와 프로필 목록\n\n```yaml\nport: 8000\n```"),
  doc("d.md", "# 차원 축소\nPCA로 차원을 줄인다", { metaText: "paper 임베딩" }),
];

test("buildIndex: 필드별 길이·평균, frontmatter 줄은 색인 제외", () => {
  const idx = buildIndex(CORPUS);
  assert.deepEqual(FIELDS, ["title", "heading", "meta", "body"]);
  assert.equal(idx.docs.length, 4);
  assert.equal(idx.postings.has("devlog"), false); // frontmatter 값은 metaText로만 들어온다
  const a = idx.docs[0];
  assert.equal(a.lens[0], 3); // "임베딩 설정" → 임베·베딩·설정
  assert.ok(idx.avgLens.every((v) => v >= 0));
  const p = idx.postings.get("차원")!;
  assert.deepEqual(p.map((x) => x.doc).sort(), [0, 3]);
  assert.deepEqual(p.find((x) => x.doc === 0)!.tf, [0, 1, 0, 1]); // heading 1 + body 1
});

test("scoreQuery: 관련 문서 상위, 무관 문서 제외", () => {
  const idx = buildIndex(CORPUS);
  const r = sq(idx, ["임베딩 차원"], CFG);
  assert.equal(r[0].path, "a.md");
  assert.deepEqual(r.map((x) => x.path).sort(), ["a.md", "d.md"]);
  assert.ok(r.every((x) => x.score > 0));
});

test("scoreQuery: 식별자 원형 질의가 정확 일치 문서를 잡는다", () => {
  const idx = buildIndex(CORPUS);
  assert.equal(sq(idx, ["agent_config.yaml"], CFG)[0].path, "b.md");
  assert.equal(sq(idx, ["gemini-embedding-2"], CFG)[0].path, "a.md");
});

test("scoreQuery: 그룹 OR — 각 그룹이 다른 문서를 잡으면 둘 다 나온다", () => {
  const idx = buildIndex(CORPUS);
  const paths = sq(idx, ["checkpointer", "PCA"], CFG).map((x) => x.path).sort();
  assert.deepEqual(paths, ["b.md", "d.md"]);
});

test("scoreQuery: 그룹 간 중복 토큰은 한 번만 센다", () => {
  const idx = buildIndex(CORPUS);
  const once = sq(idx, ["checkpointer"], CFG);
  const twice = sq(idx, ["checkpointer", "checkpointer", "CHECKPOINTER"], CFG);
  assert.deepEqual(twice, once);
});

test("scoreQuery: field_boost가 점수에 반영된다(title 부스트)", () => {
  const idx = buildIndex(CORPUS);
  const base = sq(idx, ["설정"], CFG)[0];
  const boosted = sq(idx, ["설정"], { ...CFG, field_boost: { ...CFG.field_boost, title: 6 } })[0];
  assert.equal(base.path, "a.md");
  assert.ok(Math.abs(boosted.score - 2 * base.score) < 1e-9);
});

test("scoreQuery: authority 가산은 매칭 문서에만, 동점 정렬 반영", () => {
  const docs = [doc("x.md", "공통 단어"), doc("y.md", "공통 단어", { authorityRank: 3 }), doc("z.md", "무관", { authorityRank: 9 })];
  const idx = buildIndex(docs);
  const r = sq(idx, ["공통"], CFG);
  assert.deepEqual(r.map((x) => x.path), ["y.md", "x.md"]);
  assert.ok(Math.abs(r[0].score - r[1].score - 3 * CFG.authority_weight) < 1e-9);
});

test("scoreQuery: 동점은 경로 오름차순, result_limit으로 자르지 않는다(자르기는 호출자 몫)", () => {
  const docs = ["c", "a", "b"].map((n) => doc(`${n}.md`, "같은 내용"));
  const idx = buildIndex(docs);
  assert.deepEqual(sq(idx, ["같은"], CFG).map((x) => x.path), ["a.md", "b.md", "c.md"]);
  assert.equal(sq(idx, ["같은"], { ...CFG, result_limit: 1 }).length, 3);
});

test("scoreQuery: 가중치 맵을 그대로 쓴다(빈 맵 → 빈 배열, 가중치 배수 → 점수 배수)", () => {
  const idx = buildIndex(CORPUS);
  assert.deepEqual(scoreQuery(idx, new Map(), CFG), []);
  const w = termWeights(idx, ["checkpointer"]);
  const doubled = new Map([...w].map(([t, v]) => [t, 2 * v]));
  const once = scoreQuery(idx, w, { ...CFG, authority_weight: 0 })[0];
  const twice = scoreQuery(idx, doubled, { ...CFG, authority_weight: 0 })[0];
  assert.equal(once.path, "b.md");
  assert.ok(Math.abs(twice.score - 2 * once.score) < 1e-9);
});

test("scoreQuery: 매칭 없음·빈 질의·빈 색인 → 빈 배열", () => {
  const idx = buildIndex(CORPUS);
  assert.deepEqual(sq(idx, ["존재하지않는토큰zzz"], CFG), []);
  assert.deepEqual(sq(idx, [], CFG), []);
  assert.deepEqual(sq(buildIndex([]), ["차원"], CFG), []);
});

test("buildIndex: 경로 중복은 throw", () => {
  assert.throws(() => buildIndex([doc("a.md", "x"), doc("a.md", "y")]), /색인 입력 경로 중복: a.md/);
});

test("termWeights: 희소 토큰일수록 idf가 크고, 미색인 토큰은 빠진다", () => {
  const idx = buildIndex(CORPUS);
  const w = termWeights(idx, ["차원", "checkpointer", "없는토큰"]);
  assert.equal(w.has("없는토큰"), false);
  assert.ok(w.get("checkpointer")! > w.get("차원")!);
});

test("loadOrRebuild: 캐시 없음 → 전량 rebuilt, 캐시 결과는 전량 빌드와 같은 점수", () => {
  const first = loadOrRebuild(null, CORPUS);
  assert.deepEqual(first.rebuilt, CORPUS.map((d) => d.path));
  const again = loadOrRebuild(first.json, CORPUS);
  assert.deepEqual(again.rebuilt, []);
  assert.equal(again.json, first.json);
  assert.deepEqual(sq(again.index, ["임베딩 차원"], CFG), sq(buildIndex(CORPUS), ["임베딩 차원"], CFG));
});

test("loadOrRebuild: 변경·신규만 재색인, 삭제 문서 제거, 통계 재계산", () => {
  const { json } = loadOrRebuild(null, CORPUS);
  const changed = doc("b.md", "# 멀티턴\nPCA 언급 추가");
  const added = doc("e.md", "PCA 새 문서");
  const next = [CORPUS[0], changed, CORPUS[3], added]; // c.md 삭제
  const r = loadOrRebuild(json, next);
  assert.deepEqual(r.rebuilt, ["b.md", "e.md"]);
  assert.deepEqual(r.index.docs.map((d) => d.path), ["a.md", "b.md", "d.md", "e.md"]);
  assert.equal(r.index.postings.has("8000"), false);
  assert.deepEqual(sq(r.index, ["PCA"], CFG), sq(buildIndex(next), ["PCA"], CFG));
});

test("loadOrRebuild: sha 동일해도 title·meta·authority는 새 값으로 반영", () => {
  const { json } = loadOrRebuild(null, CORPUS);
  const retitled = CORPUS.map((d) => (d.path === "c.md" ? { ...d, title: "포트 안내", authorityRank: 5 } : d));
  const r = loadOrRebuild(json, retitled);
  assert.deepEqual(r.rebuilt, []);
  const c = r.index.docs.find((d) => d.path === "c.md")!;
  assert.equal(c.authorityRank, 5);
  assert.equal(sq(r.index, ["안내"], CFG)[0].path, "c.md");
});

test("loadOrRebuild: 정상 캐시·캐시 없음은 cacheReset=null", () => {
  const first = loadOrRebuild(null, CORPUS);
  assert.equal(first.cacheReset, null);
  assert.equal(loadOrRebuild(first.json, CORPUS).cacheReset, null);
});

test("loadOrRebuild: 포맷 버전 불일치·깨진 JSON → 전량 재색인 + 사유 표면화(throw 아님)", () => {
  const fresh = loadOrRebuild(null, CORPUS).json;
  const stale = loadOrRebuild(JSON.stringify({ v: INDEX_FORMAT_VERSION + 1, docs: [] }), CORPUS);
  assert.deepEqual(stale.rebuilt, CORPUS.map((d) => d.path));
  assert.match(stale.cacheReset!, /버전/);
  assert.equal(stale.json, fresh);
  const broken = loadOrRebuild("{not json", CORPUS);
  assert.deepEqual(broken.rebuilt, CORPUS.map((d) => d.path));
  assert.match(broken.cacheReset!, /JSON/);
  assert.equal(broken.json, fresh);
});

test("loadOrRebuild: 캐시 형태 위반은 전량 재색인하고 사유에 위치를 담는다 (TDA-8, sfh-10)", () => {
  const good = JSON.parse(loadOrRebuild(null, CORPUS).json);
  const fresh = loadOrRebuild(null, CORPUS);
  for (const top of ["[]", "null", "7"]) assert.match(loadOrRebuild(top, CORPUS).cacheReset ?? "", /객체/, top);
  const cases: [RegExp, (f: any) => void][] = [
    [/docs/, (f) => { f.docs = {}; }],
    [/docs\[0\]/, (f) => { f.docs[0] = null; }],
    [/docs\[0\]\.p/, (f) => { f.docs[0].p = 7; }],
    [/docs\[1\]\.h/, (f) => { f.docs[1].h = "abc"; }],
    [/docs\[1\]\.h/, (f) => { f.docs[1].h = "G".repeat(64); }],
    [/docs\[0\]\.l/, (f) => { f.docs[0].l = [1]; }],
    [/docs\[0\]\.l/, (f) => { f.docs[0].l = [-1, 2]; }],
    [/docs\[0\]\.l/, (f) => { f.docs[0].l = [1.5, 2]; }],
    [/docs\[0\]\.t/, (f) => { f.docs[0].t = [{}]; }],
    [/docs\[0\]\.t/, (f) => { f.docs[0].t = [{ a: -1 }, {}]; }],
    [/docs\[0\]\.t/, (f) => { f.docs[0].t = [{ a: "1" }, {}]; }],
    [/docs\[0\]\.t/, (f) => { f.docs[0].t = [[], {}]; }],
    [/docs\[0\]\.l/, (f) => { f.docs[0].l = [f.docs[0].l[0] + 1, f.docs[0].l[1]]; }],
    [/docs\[1\]\.p.*중복/, (f) => { f.docs[1].p = f.docs[0].p; }],
  ];
  for (const [why, mutate] of cases) {
    const file = structuredClone(good);
    mutate(file);
    const r = loadOrRebuild(JSON.stringify(file), CORPUS);
    assert.match(r.cacheReset ?? "", why, why.source);
    assert.deepEqual(r.rebuilt, CORPUS.map((d) => d.path), why.source);
    assert.equal(r.json, fresh.json, why.source);
  }
});

test("loadOrRebuild: 빈 코퍼스라도 캐시 리셋이면 새로 직렬화한다", () => {
  const r = loadOrRebuild(JSON.stringify({ v: INDEX_FORMAT_VERSION + 1, docs: [] }), []);
  assert.notEqual(r.cacheReset, null);
  assert.equal(r.json, serializeIndex(buildIndex([])));
});

test("serializeIndex: 압축 JSON(공백 없음)이며 title·meta는 담지 않는다", () => {
  const json = serializeIndex(buildIndex(CORPUS));
  assert.equal(json.includes("\n"), false);
  assert.equal(json.includes("설정"), false); // a.md title 전용 토큰
  assert.equal(JSON.parse(json).v, INDEX_FORMAT_VERSION);
});

test("loadOrRebuild: 변경 없으면 캐시 JSON을 그대로 반환, 삭제가 있으면 다시 직렬화", () => {
  const first = loadOrRebuild(null, CORPUS);
  const again = loadOrRebuild(first.json, CORPUS);
  assert.equal(again.rebuilt.length, 0);
  assert.equal(again.json, first.json);
  const fewer = loadOrRebuild(first.json, CORPUS.slice(1));
  assert.equal(fewer.rebuilt.length, 0);
  assert.notEqual(fewer.json, first.json);
  assert.equal(JSON.parse(fewer.json).docs.length, CORPUS.length - 1);
});
