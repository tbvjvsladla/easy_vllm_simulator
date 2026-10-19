import { test } from "node:test";
import assert from "node:assert/strict";
import { MIN_WORD_LEN, normalize, queryTokens, tokenize } from "../src/tokenize.ts";

test("tokenize: 한글 구간은 겹침 bigram, 1글자 구간은 unigram", () => {
  assert.deepEqual(tokenize("멀티턴 임베딩 차원"), ["멀티", "티턴", "임베", "베딩", "차원"]);
  assert.deepEqual(tokenize("이 값"), ["이", "값"]);
});

test("tokenize: 조사가 붙어도 어간 bigram이 남는다", () => {
  const t = tokenize("임베딩을");
  assert.ok(t.includes("임베") && t.includes("베딩"));
});

test("tokenize: 식별자는 부분 + 원형 토큰", () => {
  assert.deepEqual(tokenize("agent_config.yaml"), ["agent", "config", "yaml", "agent_config.yaml"]);
  assert.deepEqual(tokenize("gemini-embedding-2"), ["gemini", "embedding", "gemini-embedding-2"]);
  assert.deepEqual(tokenize("app/utils/config.py"), ["app", "utils", "config", "py", "app/utils/config.py"]);
  assert.deepEqual(tokenize("structured_output"), ["structured", "output", "structured_output"]);
});

test("tokenize: 띄어 쓴 질의도 식별자 부분 토큰과 맞물린다", () => {
  const doc = new Set(tokenize("structured_output"));
  for (const t of tokenize("structured output")) assert.ok(doc.has(t));
});

test("tokenize: 문장 끝 마침표·구분자 단독은 식별자에 붙지 않는다", () => {
  assert.deepEqual(tokenize("config.yaml. 끝"), ["config", "yaml", "config.yaml", "끝"]);
  assert.deepEqual(tokenize("a - b / cc"), ["cc"]);
});

test("tokenize: NFKC + 소문자화, 한영 경계 분리", () => {
  assert.equal(normalize("ＬＬＭ"), "llm");
  assert.deepEqual(tokenize("ＬＬＭ이 RAG를"), ["llm", "이", "rag", "를"]);
});

test("tokenize: 비한글 1글자 토큰은 버린다", () => {
  assert.equal(MIN_WORD_LEN, 2);
  assert.deepEqual(tokenize("a x 2 ab 12"), ["ab", "12"]);
  assert.deepEqual(tokenize("v0.6.0"), ["v0", "v0.6.0"]);
});

test("tokenize: 빈 문자열·기호만 → 빈 배열", () => {
  assert.deepEqual(tokenize(""), []);
  assert.deepEqual(tokenize("--- ``` | *"), []);
});

test("tokenize: 모든 토큰은 정규화 문자열의 부분 문자열(사전 필터 불변식)", () => {
  const s = "Ｇｅｍｉｎｉ-Embedding-2 임베딩을 agent_config.yaml에서 v0.6.0";
  const norm = normalize(s);
  for (const t of tokenize(s)) assert.ok(norm.includes(t), t);
});

test("queryTokens: 그룹 합집합, 중복 제거, 첫 등장 순서", () => {
  assert.deepEqual(queryTokens(["임베딩 차원", "embedding 차원", "임베딩"]), ["임베", "베딩", "차원", "embedding"]);
  assert.deepEqual(queryTokens([]), []);
});
