// 필드별 BM25 역색인 + 증분 캐시(JSON 문자열). 파일 시스템 접근 없음(순수 함수).
import { SHA256_HEX } from "./fs-safety.ts";
//
// 점수(문서 d, 질의 가중치 W = 토큰 → idf):
//   score(d) = Σ_{t∈W} W(t) · Σ_f boost_f · tf_f(k1+1) / (tf_f + k1·(1 − b + b·len_f/avglen_f))
//              + authorityRank(d) · authority_weight      (매칭 문서에만 가산)
// - W는 호출자가 termWeights(index, queryTokens(groups))로 1회 계산해 scoreQuery·findSpans에 함께 넘긴다.
//   queryTokens가 모든 term group 토큰의 합집합(중복 제거)이므로 그룹 간 OR이며, 여러 그룹에 같은
//   토큰이 나와도 1번만 센다 → 말바꿈 변형을 여러 그룹으로 보내도 공통 토큰이 부풀지 않는다.
// - idf는 문서 단위 df(어느 필드든 등장)로 계산한다(BM25F 관례).
import type { IndexDoc, SearchConfig } from "./types.ts";
import { tokenize } from "./tokenize.ts";
import { scanLines } from "./tree.ts";

/** 색인 필드 순서. tf·len 배열의 인덱스가 이 순서를 따른다. */
export const FIELDS = ["title", "heading", "meta", "body"] as const;
const F_TITLE = 0;
const F_HEADING = 1;
const F_META = 2;
const F_BODY = 3;

/** 캐시 포맷 버전. 토크나이저·필드 규칙이 바뀌면 올린다(불일치 시 전량 재색인). */
export const INDEX_FORMAT_VERSION = 1;

/** BM25 idf의 표준 0.5 보정(Robertson–Spärck Jones). 조정 대상 아님. */
const IDF_SMOOTHING = 0.5;

/** 캐시 h 필드: sha256 16진 표기 */

type TermFreq = Map<string, number>;

/** 원문 텍스트에서만 파생되는 필드(heading·body). sha256가 같으면 재사용한다. */
type TextPostings = { sha256: string; heading: TermFreq; body: TermFreq; headingLen: number; bodyLen: number };

export type IndexEntry = {
  path: string;
  sha256: string;
  title: string;
  authorityRank: number;
  /** FIELDS 순서의 필드별 토큰 수 */
  lens: number[];
};

/** 문서 1건의 토큰 1개 등장 정보. tf는 FIELDS 순서. */
export type Posting = { doc: number; tf: number[] };

export type Index = {
  docs: IndexEntry[];
  postings: Map<string, Posting[]>;
  avgLens: number[];
  /** 직렬화용 텍스트 파생 색인(docs와 같은 순서) */
  text: TextPostings[];
};

function countInto(tf: TermFreq, tokens: string[]): number {
  for (const t of tokens) tf.set(t, (tf.get(t) ?? 0) + 1);
  return tokens.length;
}

function indexText(doc: IndexDoc): TextPostings {
  const { kinds, headings } = scanLines(doc.lines, doc.bodyStartLine);
  const heading: TermFreq = new Map();
  const body: TermFreq = new Map();
  let headingLen = 0;
  let bodyLen = 0;
  for (let i = doc.bodyStartLine - 1; i < doc.lines.length; i++) {
    if (kinds[i] === "heading") headingLen += countInto(heading, tokenize(headings.get(i + 1)!.title));
    else bodyLen += countInto(body, tokenize(doc.lines[i]));
  }
  return { sha256: doc.sha256, heading, body, headingLen, bodyLen };
}

function assemble(docs: IndexDoc[], cache: Map<string, TextPostings>): { index: Index; rebuilt: string[] } {
  const rebuilt: string[] = [];
  const entries: IndexEntry[] = [];
  const texts: TextPostings[] = [];
  const postings = new Map<string, Posting[]>();
  const seen = new Set<string>();
  const lenSums = FIELDS.map(() => 0);
  for (const doc of docs) {
    if (seen.has(doc.path)) throw new Error(`색인 입력 경로 중복: ${doc.path}`);
    seen.add(doc.path);
    const cached = cache.get(doc.path);
    let text: TextPostings;
    if (cached !== undefined && cached.sha256 === doc.sha256) {
      text = cached;
    } else {
      text = indexText(doc);
      rebuilt.push(doc.path);
    }
    // title·meta는 카드/계약에서 오므로(원문 sha와 무관) 매번 새로 센다
    const title: TermFreq = new Map();
    const meta: TermFreq = new Map();
    const lens = FIELDS.map(() => 0);
    lens[F_TITLE] = countInto(title, tokenize(doc.title));
    lens[F_META] = countInto(meta, tokenize(doc.metaText));
    lens[F_HEADING] = text.headingLen;
    lens[F_BODY] = text.bodyLen;
    const docNo = entries.length;
    const merged = new Map<string, number[]>();
    const fieldTfs: [number, TermFreq][] = [[F_TITLE, title], [F_HEADING, text.heading], [F_META, meta], [F_BODY, text.body]];
    for (const [f, tf] of fieldTfs) {
      for (const [t, c] of tf) {
        let row = merged.get(t);
        if (row === undefined) {
          row = FIELDS.map(() => 0);
          merged.set(t, row);
        }
        row[f] = c;
      }
    }
    for (const [t, tf] of merged) {
      const list = postings.get(t);
      if (list === undefined) postings.set(t, [{ doc: docNo, tf }]);
      else list.push({ doc: docNo, tf });
    }
    FIELDS.forEach((_, f) => (lenSums[f] += lens[f]));
    entries.push({ path: doc.path, sha256: doc.sha256, title: doc.title, authorityRank: doc.authorityRank, lens });
    texts.push(text);
  }
  const avgLens = lenSums.map((s) => (entries.length === 0 ? 0 : s / entries.length));
  return { index: { docs: entries, postings, avgLens, text: texts }, rebuilt };
}

/** 전량 색인 */
export function buildIndex(docs: IndexDoc[]): Index {
  return assemble(docs, new Map()).index;
}

/** BM25 idf(문서 단위 df) */
function idf(n: number, df: number): number {
  return Math.log(1 + (n - df + IDF_SMOOTHING) / (df + IDF_SMOOTHING));
}

/** 질의 토큰 → idf. 색인에 없는 토큰은 빠진다(어디에도 매칭될 수 없음). scoreQuery·findSpans 공용 가중치. */
export function termWeights(index: Index, tokens: string[]): Map<string, number> {
  const out = new Map<string, number>();
  for (const t of tokens) {
    const list = index.postings.get(t);
    if (list !== undefined) out.set(t, idf(index.docs.length, list.length));
  }
  return out;
}

/** 매칭 문서 전체를 점수 내림차순(동점은 경로 오름차순)으로. 개수 제한은 호출자가 한다. */
export function scoreQuery(index: Index, weights: ReadonlyMap<string, number>, cfg: SearchConfig): { path: string; score: number }[] {
  const k1 = cfg.bm25_k1;
  const b = cfg.bm25_b;
  const boosts = FIELDS.map((f) => cfg.field_boost[f]);
  const scores = new Map<number, number>();
  for (const [t, w] of weights) {
    for (const p of index.postings.get(t)!) {
      const lens = index.docs[p.doc].lens;
      let s = 0;
      for (let f = 0; f < FIELDS.length; f++) {
        const tf = p.tf[f];
        if (tf === 0) continue;
        const norm = 1 - b + (b * lens[f]) / index.avgLens[f];
        s += boosts[f] * w * ((tf * (k1 + 1)) / (tf + k1 * norm));
      }
      scores.set(p.doc, (scores.get(p.doc) ?? 0) + s);
    }
  }
  return [...scores]
    .map(([d, s]) => ({ path: index.docs[d].path, score: s + index.docs[d].authorityRank * cfg.authority_weight }))
    .sort((x, y) => y.score - x.score || (x.path < y.path ? -1 : x.path > y.path ? 1 : 0));
}

type CacheDoc = { p: string; h: string; l: [number, number]; t: [Record<string, number>, Record<string, number>] };
type CacheFile = { v: number; docs: CacheDoc[] };

/** 텍스트 파생 색인만 담은 압축 JSON. title·meta·authority는 매 로드 시 다시 계산하므로 넣지 않는다. */
export function serializeIndex(index: Index): string {
  const file: CacheFile = {
    v: INDEX_FORMAT_VERSION,
    docs: index.docs.map((d, i) => {
      const t = index.text[i];
      return { p: d.path, h: t.sha256, l: [t.headingLen, t.bodyLen], t: [Object.fromEntries(t.heading), Object.fromEntries(t.body)] };
    }),
  };
  return JSON.stringify(file);
}

type CacheRead = { cache: Map<string, TextPostings> } | { reset: string };

function isNonNegInt(v: unknown): v is number {
  return Number.isInteger(v) && (v as number) >= 0;
}

function isTfRecord(v: unknown): v is Record<string, number> {
  return typeof v === "object" && v !== null && !Array.isArray(v) && Object.values(v).every(isNonNegInt);
}

function sumOf(tf: Record<string, number>): number {
  let s = 0;
  for (const c of Object.values(tf)) s += c;
  return s;
}

/** 캐시 JSON 형태 검사. 위반은 예외가 아니라 재색인 사유(reset)로 돌려준다. */
function readCache(json: string): CacheRead {
  let file: unknown;
  try {
    file = JSON.parse(json);
  } catch (err) {
    // 캐시는 원문에서 언제든 다시 만들 수 있는 파생물이다. 이 한 곳만 복구 경계로 두고, 사유를 cacheReset으로 표면화한다.
    return { reset: `캐시 JSON 문법 오류: ${(err as Error).message}` };
  }
  if (typeof file !== "object" || file === null || Array.isArray(file)) return { reset: "캐시 최상위가 객체가 아니다" };
  const top = file as Record<string, unknown>;
  if (top["v"] !== INDEX_FORMAT_VERSION) return { reset: `캐시 포맷 버전 불일치: ${JSON.stringify(top["v"])} ≠ ${INDEX_FORMAT_VERSION}` };
  const docs = top["docs"];
  if (!Array.isArray(docs)) return { reset: "캐시 docs가 배열이 아니다" };
  const cache = new Map<string, TextPostings>();
  for (const [i, d] of docs.entries()) {
    const at = `캐시 docs[${i}]`;
    if (typeof d !== "object" || d === null || Array.isArray(d)) return { reset: `${at}: 객체 필요` };
    const { p, h, l, t } = d as Record<string, unknown>;
    if (typeof p !== "string") return { reset: `${at}.p: 문자열 필요` };
    if (cache.has(p)) return { reset: `${at}.p: 경로 중복 ${p}` };
    if (typeof h !== "string" || !SHA256_HEX.test(h)) return { reset: `${at}.h: 64자리 16진수 필요` };
    if (!Array.isArray(t) || t.length !== 2 || !t.every(isTfRecord)) return { reset: `${at}.t: 비음수 정수 tf 객체 2개 필요` };
    if (!Array.isArray(l) || l.length !== 2 || !l.every(isNonNegInt)) return { reset: `${at}.l: 비음수 정수 2개 필요` };
    const [headingTf, bodyTf] = t as [Record<string, number>, Record<string, number>];
    if (l[0] !== sumOf(headingTf) || l[1] !== sumOf(bodyTf)) return { reset: `${at}.l: tf 합계와 다르다` };
    cache.set(p, {
      sha256: h,
      heading: new Map(Object.entries(headingTf)),
      body: new Map(Object.entries(bodyTf)),
      headingLen: l[0],
      bodyLen: l[1],
    });
  }
  return { cache };
}

/**
 * 캐시 JSON(없으면 null)으로 색인 복원. sha256가 같은 문서는 캐시 posting을 재사용하고,
 * 변경·신규 문서만 재색인(rebuilt), 사라진 문서는 버린다. 코퍼스 통계(df·avglen)는 항상 새로 계산.
 * 캐시가 깨졌거나 형태가 틀리면 전량 재색인하고 그 사유를 cacheReset에 담는다(정상이면 null).
 */
export function loadOrRebuild(
  cachedJson: string | null,
  docs: IndexDoc[],
): { index: Index; json: string; rebuilt: string[]; cacheReset: string | null } {
  const read: CacheRead = cachedJson === null ? { cache: new Map() } : readCache(cachedJson);
  const cacheReset = "reset" in read ? read.reset : null;
  const cache = "cache" in read ? read.cache : new Map<string, TextPostings>();
  const { index, rebuilt } = assemble(docs, cache);
  // 재색인·삭제·리셋이 없으면 캐시 문서 집합이 그대로다 → 재직렬화(질의 시간의 대부분)를 생략하고 기존 JSON을 돌려준다
  const unchanged = cachedJson !== null && cacheReset === null && rebuilt.length === 0 && cache.size === docs.length;
  return { index, json: unchanged ? cachedJson : serializeIndex(index), rebuilt, cacheReset };
}
