import { test } from "node:test";
import assert from "node:assert/strict";
import { parseRef, readSpan } from "../src/read.ts";

test("parseRef: 정상·경로에 ':' 포함", () => {
  assert.deepEqual(parseRef("docs/a.md:3-7"), { path: "docs/a.md", start: 3, end: 7 });
  assert.deepEqual(parseRef("x:y.md:1-1"), { path: "x:y.md", start: 1, end: 1 });
});

test("parseRef: 형식·범위 오류는 throw", () => {
  assert.throws(() => parseRef("docs/a.md"), /ref 형식 오류/);
  assert.throws(() => parseRef("docs/a.md:3"), /ref 형식 오류/);
  assert.throws(() => parseRef(":1-2"), /ref 형식 오류/);
  assert.throws(() => parseRef("a.md:-1-2"), /ref 형식 오류/);
  assert.throws(() => parseRef("a.md:0-2"), /ref 범위 오류/);
  assert.throws(() => parseRef("a.md:5-4"), /ref 범위 오류/);
});

test("readSpan: 1-based 포함 범위", () => {
  const lines = ["a", "b", "c"];
  assert.equal(readSpan(lines, 1, 1), "a");
  assert.equal(readSpan(lines, 2, 3), "b\nc");
});

test("readSpan: 파일 밖·역순·비정수는 throw", () => {
  const lines = ["a", "b"];
  assert.throws(() => readSpan(lines, 0, 1), /span 범위 오류/);
  assert.throws(() => readSpan(lines, 2, 3), /span 범위 오류/);
  assert.throws(() => readSpan(lines, 2, 1), /span 범위 오류/);
  assert.throws(() => readSpan(lines, 1.5, 2), /span 범위 오류/);
  assert.throws(() => readSpan([], 1, 1), /span 범위 오류/);
});
