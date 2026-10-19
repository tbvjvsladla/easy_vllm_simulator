import { test } from "node:test";
import assert from "node:assert/strict";
import { headingPathAt, headingTree, PREAMBLE_LEVEL, scanLines } from "../src/tree.ts";

const DOC = [
  "---", //1
  "title: x", //2
  "---", //3
  "서문 문단", //4
  "", //5
  "# 개요", //6
  "본문", //7
  "## 설치 ##", //8
  "```bash", //9
  "# 주석은 헤딩 아님", //10
  "```", //11
  "### 세부", //12
  "## 구성", //13
  "~~~", //14
  "## 펜스 안", //15
  "~~~", //16
  "# 부록", //17
  "끝", //18
];

test("headingTree: 서문 level0, 중첩, 펜스 내부 무시, end 계산", () => {
  const tree = headingTree(DOC, 4);
  assert.equal(tree.length, 3);
  assert.deepEqual({ ...tree[0], children: tree[0].children.length }, { level: PREAMBLE_LEVEL, title: "", start: 4, end: 5, children: 0 });
  const [, overview, appendix] = tree;
  assert.equal(overview.title, "개요");
  assert.equal(overview.start, 6);
  assert.equal(overview.end, 16);
  assert.deepEqual(overview.children.map((c) => [c.title, c.level, c.start, c.end]), [
    ["설치", 2, 8, 12],
    ["구성", 2, 13, 16],
  ]);
  assert.deepEqual(overview.children[0].children.map((c) => [c.title, c.start, c.end]), [["세부", 12, 12]]);
  assert.deepEqual([appendix.title, appendix.start, appendix.end], ["부록", 17, 18]);
});

test("headingTree: frontmatter 구간의 '#'은 헤딩으로 보지 않는다", () => {
  const lines = ["---", "# 가짜", "---", "# 진짜"];
  const tree = headingTree(lines, 4);
  assert.deepEqual(tree.map((n) => n.title), ["진짜"]);
});

test("headingTree: 빈 서문은 level0 노드를 만들지 않고, 헤딩 없는 문서는 서문 1개", () => {
  assert.equal(headingTree(["", "# a"], 1)[0].level, 1);
  const only = headingTree(["본문만", "둘째 줄"], 1);
  assert.deepEqual(only, [{ level: 0, title: "", start: 1, end: 2, children: [] }]);
  assert.deepEqual(headingTree([], 1), []);
});

test("headingTree: '#' 뒤 공백 없음·7개는 헤딩 아님, 4칸 들여쓰기 아님, CRLF 제거", () => {
  const tree = headingTree(["#태그", "####### 일곱", "    # 코드", "# 제목\r"], 1);
  assert.deepEqual(tree.map((n) => [n.level, n.title, n.start]), [[0, "", 1], [1, "제목", 4]]);
});

test("headingTree: 상위 레벨 건너뛰기(### 다음 #)도 스택으로 처리", () => {
  const tree = headingTree(["### 깊음", "# 얕음", "### 다시"], 1);
  assert.deepEqual(tree.map((n) => [n.title, n.end]), [["깊음", 1], ["얕음", 3]]);
  assert.equal(tree[1].children[0].title, "다시");
});

test("scanLines: 더 긴 펜스는 짧은 펜스로 닫히지 않는다", () => {
  const { kinds } = scanLines(["````", "```", "# x", "````", "# y"], 1);
  assert.deepEqual(kinds, ["fence", "code", "code", "fence", "heading"]);
});

test("headingTree: bodyStartLine 범위 밖은 throw", () => {
  assert.throws(() => headingTree(["a"], 0), /bodyStartLine 범위 오류/);
  assert.throws(() => headingTree(["a"], 3), /bodyStartLine 범위 오류/);
  assert.throws(() => headingTree(["a"], 1.5), /bodyStartLine 범위 오류/);
});

test("headingPathAt: 루트→가장 깊은 헤딩, 서문은 빈 경로", () => {
  const tree = headingTree(DOC, 4);
  assert.deepEqual(headingPathAt(tree, 4), []);
  assert.deepEqual(headingPathAt(tree, 12), ["개요", "설치", "세부"]);
  assert.deepEqual(headingPathAt(tree, 15), ["개요", "구성"]);
  assert.deepEqual(headingPathAt(tree, 2), []);
});

test("headingTree: 헤딩 수가 매우 많아도 첫 헤딩 위치를 구한다(cs-15: spread 인자 상한 없음)", () => {
  const MANY = 300_000; // V8 spread 인자 한도를 넘는 규모
  const lines = ["서문", ...Array.from({ length: MANY }, (_, i) => `# h${i}`)];
  const tree = headingTree(lines, 1);
  assert.equal(tree.length, MANY + 1);
  assert.deepEqual(tree[0], { level: 0, title: "", start: 1, end: 1, children: [] });
  assert.equal(tree[1].start, 2);
});
