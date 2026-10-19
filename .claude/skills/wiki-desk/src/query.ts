// 문서 안에서 읽을 구간(span) 고르기. 순수 함수.
//
// 1) 본문을 블록으로 나눈다: 빈 줄로 구분된 문단, 펜스 코드는 통째로 한 블록.
//    헤딩 줄은 항상 블록을 끊으며(섹션 경계를 넘지 않음), 헤딩은 바로 뒤 첫 블록에 붙는다
//    (헤딩만 매칭돼도 그 섹션 첫 내용이 함께 나오도록).
// 2) 블록마다 창 1개를 고른다. 창 폭 = min(블록 길이, span_max_lines)로 블록 안에서는 모두 같고,
//    블록이 더 길면 창을 미끄러뜨려 최고점 창을 고른다(블록 안 동점은 앞선 창).
// 3) 창 점수 = Σ_t idf(t) · tf(k1+1)/(tf+k1)  (질의 토큰별 포화 tf; 같은 토큰 반복보다 서로 다른
//    질의 토큰을 많이 덮는 창이 이긴다).
// 4) 블록끼리는 겹치지 않으므로 블록별 창을 점수순으로 정렬해 상위 spans_per_result개를 반환한다.
//    블록 간 동점이면 줄 수가 적은 창(밀도↑), 그다음 앞선 창.
// 줄 번호는 원문 파일 기준 1-based(end 포함)이며 frontmatter 줄도 센다.
import type { HeadingNode, IndexDoc, SearchConfig, Span } from "./types.ts";
import { normalize, tokenize } from "./tokenize.ts";
import { headingPathAt, scanLines } from "./tree.ts";

type Block = { start: number; end: number };
type Candidate = { start: number; end: number; score: number };

/** 본문 블록 분할(1-based 줄 번호, 끝의 빈 줄 제외) */
export function splitBlocks(lines: string[], bodyStartLine: number): Block[] {
  const { kinds } = scanLines(lines, bodyStartLine);
  const blocks: Block[] = [];
  let cur: { start: number; last: number; headingOnly: boolean } | null = null;
  let inFence = false;
  const flush = (): void => {
    if (cur !== null) blocks.push({ start: cur.start, end: cur.last });
    cur = null;
  };
  for (let i = bodyStartLine - 1; i < lines.length; i++) {
    const n = i + 1;
    const kind = kinds[i];
    if (kind === "fence" && inFence) {
      // 닫는 펜스: 코드 블록 종료
      cur!.last = n;
      inFence = false;
      flush();
      continue;
    }
    if (kind === "heading") {
      flush();
      cur = { start: n, last: n, headingOnly: true };
      continue;
    }
    if (kind === "blank") {
      if (cur !== null && !cur.headingOnly) flush();
      continue;
    }
    if (kind === "fence") {
      // 여는 펜스: 앞 문단을 끊고(헤딩만 있으면 붙임) 코드 블록 시작
      if (cur !== null && !cur.headingOnly) flush();
      inFence = true;
    }
    if (cur === null) cur = { start: n, last: n, headingOnly: false };
    cur.last = n;
    cur.headingOnly = false;
  }
  flush();
  return blocks;
}

function windowScore(counts: Map<string, number>, weights: ReadonlyMap<string, number>, k1: number): number {
  let s = 0;
  for (const [t, tf] of counts) if (tf > 0) s += weights.get(t)! * ((tf * (k1 + 1)) / (tf + k1));
  return s;
}

/** 줄별 질의 토큰 빈도. 질의 토큰이 부분 문자열로도 없는 줄은 토큰화를 건너뛴다(토큰은 정규화 문자열의 부분 문자열). */
function lineMatches(line: string, weights: ReadonlyMap<string, number>): Map<string, number> | null {
  const norm = normalize(line);
  let any = false;
  for (const t of weights.keys()) {
    if (norm.includes(t)) {
      any = true;
      break;
    }
  }
  if (!any) return null;
  const m = new Map<string, number>();
  for (const t of tokenize(line)) if (weights.has(t)) m.set(t, (m.get(t) ?? 0) + 1);
  return m.size === 0 ? null : m;
}

function add(counts: Map<string, number>, m: Map<string, number> | null, sign: number): void {
  if (m === null) return;
  for (const [t, c] of m) counts.set(t, (counts.get(t) ?? 0) + sign * c);
}

function bestWindow(block: Block, matches: (Map<string, number> | null)[], weights: ReadonlyMap<string, number>, cfg: SearchConfig): Candidate | null {
  const len = block.end - block.start + 1;
  const width = Math.min(len, cfg.span_max_lines);
  const counts = new Map<string, number>();
  const at = (n: number): Map<string, number> | null => matches[n - block.start];
  for (let n = block.start; n < block.start + width; n++) add(counts, at(n), 1);
  let best: Candidate = { start: block.start, end: block.start + width - 1, score: windowScore(counts, weights, cfg.bm25_k1) };
  for (let s = block.start + 1; s + width - 1 <= block.end; s++) {
    add(counts, at(s - 1), -1);
    add(counts, at(s + width - 1), 1);
    const score = windowScore(counts, weights, cfg.bm25_k1);
    if (score > best.score) best = { start: s, end: s + width - 1, score };
  }
  return best.score > 0 ? best : null;
}

/**
 * 질의에 가장 잘 맞는 구간들(최대 spans_per_result개, 점수순).
 * weights = 질의 토큰 → idf(index.termWeights). 매칭 블록이 없으면 빈 배열.
 */
export function findSpans(doc: IndexDoc, tree: HeadingNode[], weights: ReadonlyMap<string, number>, cfg: SearchConfig): Span[] {
  const cands: Candidate[] = [];
  for (const block of splitBlocks(doc.lines, doc.bodyStartLine)) {
    const matches: (Map<string, number> | null)[] = [];
    let hit = false;
    for (let n = block.start; n <= block.end; n++) {
      const m = lineMatches(doc.lines[n - 1], weights);
      hit ||= m !== null;
      matches.push(m);
    }
    if (!hit) continue;
    const c = bestWindow(block, matches, weights, cfg);
    if (c !== null) cands.push(c);
  }
  cands.sort((x, y) => y.score - x.score || x.end - x.start - (y.end - y.start) || x.start - y.start);
  return cands.slice(0, cfg.spans_per_result).map((c) => ({
    ref: `${doc.path}:${c.start}-${c.end}`,
    heading_path: headingPathAt(tree, c.start),
    start: c.start,
    end: c.end,
  }));
}
