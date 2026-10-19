import { test } from "node:test";
import assert from "node:assert/strict";
import type { IndexDoc, SearchConfig } from "../src/types.ts";
import { findSpans, splitBlocks } from "../src/query.ts";
import { headingTree } from "../src/tree.ts";
import { tokenize } from "../src/tokenize.ts";

const CFG: SearchConfig = {
  bm25_k1: 1.2,
  bm25_b: 0.75,
  field_boost: { title: 3, heading: 2, meta: 1.5, body: 1 },
  authority_weight: 0.1,
  span_max_lines: 4,
  result_limit: 10,
  spans_per_result: 2,
};

const LINES = [
  "---", //1
  "type: devlog", //2
  "---", //3
  "서문: 임베딩 이야기", //4
  "", //5
  "# 설정", //6
  "", //7
  "gemini-embedding-2 임베딩 차원은", //8
  "3072이다.", //9
  "", //10
  "무관한 문단", //11
  "## 코드", //12
  "```yaml", //13
  "a: 1", //14
  "", //15
  "b: 2", //16
  "embedding_dim: 768", //17
  "c: 3", //18
  "d: 4", //19
  "```", //20
  "# 다른 장", //21
  "차원 이야기 차원 차원", //22
];
const DOC: IndexDoc = { path: "docs/a.md", sha256: "x", title: "", metaText: "", lines: LINES, bodyStartLine: 4, authorityRank: 0 };

/** 테스트용 가중치: 질의 토큰마다 지정 idf */
function weights(q: string, idf: Record<string, number> = {}): Map<string, number> {
  return new Map(tokenize(q).map((t) => [t, idf[t] ?? 1]));
}

test("splitBlocks: 빈 줄 문단, 헤딩은 첫 블록에 붙음, 펜스는 빈 줄 포함 통째, frontmatter 제외", () => {
  assert.deepEqual(splitBlocks(LINES, 4), [
    { start: 4, end: 4 },
    { start: 6, end: 9 },
    { start: 11, end: 11 },
    { start: 12, end: 20 },
    { start: 21, end: 22 },
  ]);
});

test("splitBlocks: 문단은 헤딩 경계를 넘지 않는다", () => {
  assert.deepEqual(splitBlocks(["가", "# 나", "다", "## 라"], 1), [
    { start: 1, end: 1 },
    { start: 2, end: 3 },
    { start: 4, end: 4 },
  ]);
});

test("splitBlocks: 닫히지 않은 펜스는 파일 끝까지 한 블록", () => {
  assert.deepEqual(splitBlocks(["글", "```", "x", "", "# 안"], 1), [{ start: 1, end: 1 }, { start: 2, end: 5 }]);
});

test("findSpans: 서로 다른 질의 토큰을 많이 덮는 블록이 1순위, ref·heading_path·원문 줄번호", () => {
  const tree = headingTree(LINES, 4);
  const spans = findSpans(DOC, tree, weights("gemini-embedding-2 임베딩 차원"), CFG);
  assert.equal(spans.length, CFG.spans_per_result);
  assert.deepEqual(spans[0], { ref: "docs/a.md:6-9", heading_path: ["설정"], start: 6, end: 9 });
});

test("findSpans: 반복보다 커버리지 — tf 포화로 '차원' 반복 블록이 밀린다", () => {
  const tree = headingTree(LINES, 4);
  const spans = findSpans(DOC, tree, weights("임베딩 차원"), CFG);
  assert.equal(spans[0].start, 6);
  // 서문(임베·베딩 2종, 각 1회)이 '차원' 4회 반복 블록보다 앞선다
  assert.deepEqual(spans.map((s) => s.start), [6, 4]);
});

test("findSpans: idf 가중이 순위를 바꾼다", () => {
  const tree = headingTree(LINES, 4);
  const spans = findSpans(DOC, tree, weights("임베딩 차원", { 임베: 0.01, 베딩: 0.01, 차원: 5 }), CFG);
  assert.equal(spans[0].start, 21);
});

test("findSpans: 긴 코드 블록은 span_max_lines 창으로 최고 밀도 구간만 자른다", () => {
  const tree = headingTree(LINES, 4);
  const spans = findSpans(DOC, tree, weights("embedding_dim"), { ...CFG, spans_per_result: 1 });
  assert.equal(spans.length, 1);
  const s = spans[0];
  assert.ok(s.end - s.start + 1 <= CFG.span_max_lines);
  assert.ok(s.start <= 17 && 17 <= s.end && s.start >= 12 && s.end <= 20);
  assert.equal(s.start, 14); // 동점 창 중 가장 앞(14~17)
  assert.deepEqual(s.heading_path, ["설정", "코드"]);
});

test("findSpans: span끼리 겹치지 않고 spans_per_result를 넘지 않는다", () => {
  const tree = headingTree(LINES, 4);
  const spans = findSpans(DOC, tree, weights("임베딩 차원 embedding"), { ...CFG, spans_per_result: 10 });
  assert.ok(spans.length <= 10);
  const sorted = [...spans].sort((a, b) => a.start - b.start);
  for (let i = 1; i < sorted.length; i++) assert.ok(sorted[i - 1].end < sorted[i].start);
  assert.ok(spans.every((s) => s.start >= DOC.bodyStartLine));
});

test("findSpans: 서문 매칭은 빈 heading_path", () => {
  const tree = headingTree(LINES, 4);
  const spans = findSpans(DOC, tree, weights("서문"), CFG);
  assert.deepEqual(spans, [{ ref: "docs/a.md:4-4", heading_path: [], start: 4, end: 4 }]);
});

test("findSpans: 헤딩만 매칭돼도 섹션 첫 내용과 함께 나온다", () => {
  const tree = headingTree(LINES, 4);
  const spans = findSpans(DOC, tree, weights("설정"), CFG);
  assert.deepEqual([spans[0].start, spans[0].end], [6, 9]);
});

test("findSpans: 매칭 없음·빈 가중치 → 빈 배열", () => {
  const tree = headingTree(LINES, 4);
  assert.deepEqual(findSpans(DOC, tree, weights("없는말"), CFG), []);
  assert.deepEqual(findSpans(DOC, tree, new Map(), CFG), []);
});

test("findSpans: 부분 문자열만 겹치고 토큰은 다르면 매칭 아님(사전 필터 오탐 제거)", () => {
  const doc: IndexDoc = { ...DOC, lines: ["configuration 설명"], bodyStartLine: 1 };
  assert.deepEqual(findSpans(doc, headingTree(doc.lines, 1), weights("config"), CFG), []);
});
