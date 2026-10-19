// 색인·질의 마이크로 벤치(node --test 기본 글롭 밖). 실행: node tests/perf.probe.ts [repoDir] [commit]
// 실제 코퍼스를 git show로 읽기 전용 적재하고, 목표 규모(문서 수·바이트)까지 복제 확장한다.
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { performance } from "node:perf_hooks";
import type { IndexDoc, SearchConfig } from "../src/types.ts";
import { buildIndex, loadOrRebuild, scoreQuery, termWeights } from "../src/index.ts";
import { queryTokens } from "../src/tokenize.ts";
import { findSpans } from "../src/query.ts";
import { headingTree } from "../src/tree.ts";

// 사용: node tests/perf.probe.ts <git 저장소> <커밋> <원문 루트(쉼표 구분)> <질의>... — 코퍼스는 그 커밋의 루트 아래 md
const USAGE = "사용법: node tests/perf.probe.ts <git 저장소> <커밋> <원문 루트(쉼표 구분)> <질의>...";
const [REPO, COMMIT, ROOTS, ...QUERY_ARGS] = process.argv.slice(2);
if (REPO === undefined || COMMIT === undefined || ROOTS === undefined || QUERY_ARGS.length === 0) throw new Error(USAGE);
/** 측정 목표 규모(문서 수·바이트). 실코퍼스가 이보다 작으면 복제본으로 채운다 */
const TARGET_DOCS = 250;
const TARGET_BYTES = 3.5e6;
/** 반복 측정 횟수(중앙값 사용) */
const REPEAT = 7;
const CFG: SearchConfig = {
  bm25_k1: 1.2,
  bm25_b: 0.75,
  field_boost: { title: 3, heading: 2, meta: 1.5, body: 1 },
  authority_weight: 0.1,
  span_max_lines: 40,
  result_limit: 10,
  spans_per_result: 3,
};
// 질의 인자 1개 = 질의 1건. "|"로 나누면 같은 질의의 OR 변형(--terms 여러 개와 같다)
const QUERIES = QUERY_ARGS.map((q) => q.split("|"));

const git = (...args: string[]): string => execFileSync("git", ["-C", REPO, ...args], { encoding: "utf8", maxBuffer: 1 << 28 });

function toDoc(path: string, text: string): IndexDoc {
  const lines = text.split("\n");
  // 벤치 전용 간이 frontmatter 판정(정식 판정은 frontmatter.ts 몫)
  const close = lines[0] === "---" ? lines.indexOf("---", 1) : -1;
  const bodyStartLine = close > 0 ? close + 2 : 1;
  const title = lines.find((l) => l.startsWith("# "))?.slice(2) ?? path;
  return { path, sha256: createHash("sha256").update(text).digest("hex"), title, metaText: path, lines, bodyStartLine, authorityRank: 0 };
}

function median(xs: number[]): number {
  const s = [...xs].sort((a, b) => a - b);
  return s[Math.floor(s.length / 2)];
}

function time<T>(fn: () => T): [T, number] {
  const t0 = performance.now();
  const r = fn();
  return [r, performance.now() - t0];
}

const paths = git("ls-tree", "-r", "--name-only", "-z", COMMIT, "--", ...ROOTS.split(",")).split("\0").filter((p) => p.endsWith(".md"));
const real = paths.map((p) => toDoc(p, git("show", `${COMMIT}:${p}`)));
const realBytes = real.reduce((n, d) => n + Buffer.byteLength(d.lines.join("\n")), 0);
const docs = [...real];
let bytes = realBytes;
for (let copy = 1; docs.length < TARGET_DOCS || bytes < TARGET_BYTES; copy++) {
  for (const d of real) {
    // 복제본 해시도 캐시 형식(64자리 16진수)을 지켜야 캐시 재사용이 측정된다
    docs.push({ ...d, path: `copy${copy}/${d.path}`, sha256: createHash("sha256").update(`${d.sha256}-${copy}`).digest("hex") });
    bytes += Buffer.byteLength(d.lines.join("\n"));
    if (docs.length >= TARGET_DOCS && bytes >= TARGET_BYTES) break;
  }
}
console.log(`실코퍼스 ${real.length}건 ${(realBytes / 1e6).toFixed(2)}MB → 벤치 ${docs.length}건 ${(bytes / 1e6).toFixed(2)}MB`);

const builds: number[] = [];
let index = buildIndex(docs);
for (let i = 0; i < REPEAT; i++) builds.push(time(() => (index = buildIndex(docs)))[1]);
console.log(`buildIndex 중앙값 ${median(builds).toFixed(0)}ms (어휘 ${index.postings.size})`);

const [cold, coldMs] = time(() => loadOrRebuild(null, docs));
console.log(`loadOrRebuild(캐시 없음) ${coldMs.toFixed(0)}ms, 캐시 JSON ${(cold.json.length / 1e6).toFixed(2)}MB`);
const warmMs = median(Array.from({ length: REPEAT }, () => time(() => loadOrRebuild(cold.json, docs))[1]));
console.log(`loadOrRebuild(캐시 적중) 중앙값 ${warmMs.toFixed(0)}ms`);
const edited = docs.map((d, i) => (i === 0 ? { ...d, sha256: "f".repeat(64) } : d));
const [inc, incMs] = time(() => loadOrRebuild(cold.json, edited));
console.log(`loadOrRebuild(1건 변경) ${incMs.toFixed(0)}ms rebuilt=${inc.rebuilt.length}`);

for (const q of QUERIES) {
  const run = (): { path: string; spans: number }[] => {
    const w = termWeights(index, queryTokens(q));
    const hits = scoreQuery(index, w, CFG).slice(0, CFG.result_limit);
    const byPath = new Map(docs.map((d) => [d.path, d]));
    return hits.map((h) => {
      const d = byPath.get(h.path)!;
      return { path: h.path, spans: findSpans(d, headingTree(d.lines, d.bodyStartLine), w, CFG).length };
    });
  };
  run();
  const ms = median(Array.from({ length: REPEAT }, () => time(run)[1]));
  const top = run().slice(0, 3).map((r) => `${r.path}(${r.spans})`);
  console.log(`질의 ${JSON.stringify(q)} 중앙값 ${ms.toFixed(1)}ms 상위: ${top.join(", ")}`);
}
