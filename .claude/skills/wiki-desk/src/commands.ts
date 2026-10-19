// 명령 구현(통합 계층). CLI는 이 함수들을 그대로 호출하고 JSON을 출력한다.
// 쓰기는 Transaction(scope = wiki_dir) 경유 — 원문 경로에는 쓰기 경로 자체가 없다.
// 예외 하나: remove 뒤 비게 된 위키 안 디렉터리 정리(pruneEmptyAncestors)는 rmdir을 직접 쓴다(빈 디렉터리만, wiki_dir 안).
import fs from "node:fs";
import { isUtf8 } from "node:buffer";
import path from "node:path";
import type { Contract, HeadingNode, IndexDoc, QueryResult, CheckIssue, Card } from "./types.ts";
import { Transaction, replaceDerived, readRegular, sha256, safeRelative, isSafeRelative, canonicalRoot, isBelow, SHA256_HEX } from "./fs-safety.ts";
import type { Write } from "./fs-safety.ts";
import { findContract, validateContract, authorityFor, listSources, WIKI_DIRS, CONTRACT_FILE } from "./contract.ts";
import { splitFrontmatter, emitYaml, locate, bodyText } from "./frontmatter.ts";
import { buildCard, inferFromPath } from "./mapping.ts";
import { renderCard, tryParseCard, checkConcept } from "./okf.ts";
import { headingTree } from "./tree.ts";
import { queryTokens } from "./tokenize.ts";
import { loadOrRebuild, scoreQuery, termWeights } from "./index.ts";
import { findSpans } from "./query.ts";
import { parseRef, readSpan } from "./read.ts";
import { INTAKE_MANIFEST, ownerOf, DEFAULT_NODE_MIRROR_ROOT, MAIN_OWNER, readLedger, ledgerRel, expiredLedger, payloadSha, recoverJournal, journalPending, heldLock, EXIT_NEEDS_RECOVERY, LEDGER_DIR, JOURNAL_DIR } from "./concurrency.ts";

// 위키 bundle 안 고정 파일명. OKF §3.1 예약 파일과 운영 파일.
const GITIGNORE_FILE = ".gitignore";
const INDEX_FILE = "index.md";
const LOG_FILE = "log.md";
const RECEIPT_FILE = "receipt.json";
const INDEX_CACHE_FILE = "index.json";
const OKF_VERSION = "0.2";
// OKF §5.2 사람/기계 확인 이력 키 — 확인한 내용에 묶이므로 원문이 바뀌면 옮기지 않는다
const HUMAN_ATTESTATION_KEY = "verified";
const LOG_HEADER = "# 위키 변경 이력\n";
// 카드 파일명 = 원문 상대경로 + 이 접미사. 원문 이름(index.md·log.md·.txt·.MD)과 무관하게 OKF concept(.md)이 되고,
// 접미사를 떼면 원문 경로로 되돌아가는 1:1 대응이 된다.
const CARD_SUFFIX = ".card.md";
const MD_SUFFIX = ".md";
// 날짜 그룹 헤딩(OKF §9) — ISO 날짜 길이
const ISO_DATE_LEN = 10;

type Wiki = { root: string; contract: Contract; wikiDir: string };
type Receipt = { files: Record<string, string> };

const open = (root: string): Wiki => {
  const r = canonicalRoot(root);
  const { wikiDir, contract } = findContract(r);
  return { root: r, contract, wikiDir };
};
const inWiki = (w: Wiki, ...parts: string[]): string => path.posix.join(w.wikiDir, ...parts);
const cardRel = (w: Wiki, src: string): string => inWiki(w, w.contract.layout.cards_dir, src + CARD_SUFFIX);
const archiveRel = (w: Wiki, src: string): string => inWiki(w, w.contract.layout.archive_dir, src + CARD_SUFFIX);
const cacheRel = (w: Wiki, name: string): string => inWiki(w, w.contract.layout.cache_dir, name);
const exists = (root: string, rel: string): boolean => fs.existsSync(path.join(root, rel));
const splitLines = (text: string): string[] => text.split(/\r?\n/);

/** 원문 읽기: 해시는 원 바이트 기준(모든 명령 공통). UTF-8이 아니면 사유를 돌려주고 손실 디코딩본으로 색인한다. */
function readSource(root: string, rel: string): { sha: string; text: string; decodeError: string | null } {
  const buf = readRegular(root, rel);
  return { sha: sha256(buf), text: buf.toString("utf8"), decodeError: isUtf8(buf) ? null : "원문이 UTF-8이 아님(손실 디코딩으로 색인)" };
}
const readText = (root: string, rel: string): string => readRegular(root, rel).toString("utf8");

/** dir(루트 상대) 아래 파일의 dir 기준 상대 경로 목록(정렬). 없으면 빈 목록. */
function listFiles(root: string, dir: string, skip: Set<string> = new Set()): string[] {
  const out: string[] = [];
  const walk = (rel: string): void => {
    for (const e of fs.readdirSync(path.join(root, dir, rel), { withFileTypes: true })) {
      const child = rel === "" ? e.name : `${rel}/${e.name}`;
      if (skip.has(child)) continue;
      if (e.isSymbolicLink()) throw new Error(`위키 안 심볼릭링크 거부: ${dir}/${child}`);
      if (e.isDirectory()) walk(child);
      else if (e.isFile()) out.push(child);
    }
  };
  if (exists(root, dir)) walk("");
  return out.sort();
}

/** cards/archive 디렉터리의 카드 → 원문 경로. 접미사 규칙 밖 파일은 unrecognized로 따로 돌려준다. */
function listCards(w: Wiki, dirName: string): { sources: string[]; unrecognized: string[] } {
  const dir = inWiki(w, dirName);
  const files = listFiles(w.root, dir);
  return {
    sources: files.filter((f) => f.endsWith(CARD_SUFFIX)).map((f) => f.slice(0, -CARD_SUFFIX.length)),
    unrecognized: files.filter((f) => !f.endsWith(CARD_SUFFIX)).map((f) => `${dir}/${f}`),
  };
}

/** receipt(추적 대상): 위키 생성물의 sha. 입력 계약 — 위키 밖 경로·잘못된 해시면 거부. */
function readReceipt(w: Wiki): Receipt {
  const rel = inWiki(w, RECEIPT_FILE);
  if (!exists(w.root, rel)) throw new Error(`receipt 없음(${rel}) — 생성물 소유 근거가 없어 적용 거부. init으로 만든 위키인지 확인`);
  const raw = parseJsonAt(readText(w.root, rel), rel);
  const files = (raw as { files?: unknown }).files;
  if (typeof files !== "object" || files === null || Array.isArray(files)) throw new Error(`receipt 형식 오류: files 객체 필요 (${rel})`);
  for (const [f, sha] of Object.entries(files as Record<string, unknown>)) {
    if (!isSafeRelative(f) || !isBelow(f, w.wikiDir) || typeof sha !== "string" || !SHA256_HEX.test(sha)) {
      throw new Error(`receipt 형식 오류: 위키 밖 경로이거나 해시가 아님 — ${f} (${rel})`);
    }
  }
  return { files: files as Record<string, string> };
}

/** JSON 문법 오류에 파일 경로를 붙여 다시 던진다(원래 오류는 cause로 보존 — 삼키지 않음) */
function parseJsonAt(text: string, rel: string): unknown {
  try {
    return JSON.parse(text);
  } catch (err) {
    throw new Error(`JSON 형식 오류(${rel}): ${(err as Error).message}`, { cause: err });
  }
}

const gitignoreBytes = (contract: Contract): Buffer => Buffer.from(`${contract.layout.cache_dir}/\n`);

const receiptBytes = (files: Record<string, string>): Buffer => {
  const sorted = Object.fromEntries(Object.entries(files).sort(([a], [b]) => (a < b ? -1 : 1)));
  return Buffer.from(JSON.stringify({ files: sorted }, null, 1) + "\n");
};

/**
 * writes를 receipt 갱신과 함께 하나의 트랜잭션으로 계획·적용.
 * owners: 쓴 바이트의 해시 대신 지정한 소유 기록을 남길 경로(null = 소유 기록 없음).
 * 보존 이동처럼 바이트를 옮기기만 할 때 원래 소유 상태(사람 수정 여부)를 그대로 넘기는 데 쓴다.
 */
function commit(w: Wiki, writes: Write[], receipt: Receipt, owners: Map<string, string | null> = new Map()): void {
  const files = { ...receipt.files };
  for (const wr of writes) {
    if (wr.data === null) delete files[wr.rel];
    else files[wr.rel] = sha256(wr.data);
  }
  for (const [rel, owned] of owners) {
    if (owned === null) delete files[rel];
    else files[rel] = owned;
  }
  const tx = new Transaction(w.root, [w.wikiDir]);
  tx.apply(tx.plan([...writes, { rel: inWiki(w, RECEIPT_FILE), data: receiptBytes(files) }]), { journal: { wikiDir: w.wikiDir }, fault: killFault() });
}

/** 시험 평면 전용 결함 주입: WIKI_DESK_FAULT_KILL_AT=<n> 이면 n번째 변경 직후 자기 자신을 SIGKILL 한다(쓰기 중 kill 재현).
 *  변수가 없으면 아무것도 하지 않는다 — 운영 경로의 동작은 바뀌지 않는다. */
function killFault(): ((count: number) => void) | undefined {
  const at = Number(process.env["WIKI_DESK_FAULT_KILL_AT"]);
  if (!Number.isInteger(at) || at <= 0) return undefined;
  return (count: number) => { if (count >= at) process.kill(process.pid, "SIGKILL"); };
}

/** 계약 확장 키 node_mirror_root(서브 원문 미러 루트) — 없으면 기본값 */
export const mirrorRootOf = (contract: Contract): string =>
  typeof contract["node_mirror_root"] === "string" ? contract["node_mirror_root"] as string : DEFAULT_NODE_MIRROR_ROOT;

/**
 * 모든 명령의 진입 전처리. 배타 잠금이면 남은 저널을 복구하고 무엇을 했는지 돌려준다.
 * 공유 잠금인데 저널이 남아 있으면(쓰던 writer 가 죽었다) 읽지 않고 EXIT_NEEDS_RECOVERY 로 끝낸다 —
 * 래퍼가 배타로 승격해 복구한 뒤 같은 읽기를 다시 돈다(중간 상태 관측 ✗).
 */
export function enterWiki(root: string): ReturnType<typeof recoverJournal> {
  const w = open(root);
  const mode = heldLock();
  if (mode === "x") return recoverJournal(w.root, w.wikiDir);
  if (mode === "s" && journalPending(w.root, w.wikiDir)) process.exit(EXIT_NEEDS_RECOVERY);
  return null;
}

/**
 * 카드 소유 상태.
 * - owned: receipt 해시와 바이트가 같다(도구가 마지막에 쓴 그대로)
 * - owned-with-human-keys: 사람이 frontmatter에 OKF 키(verified 등)만 더했다 — 카드 키·본문은 도구 것 그대로.
 *   receipt에는 사람 키를 뺀 렌더의 해시를 둔다. 원문이 바뀌면 갱신하되 사람 키는 다시 쓴다.
 * - edited: 그 밖의 수정(본문·카드 키 변경·형식 오류) — 덮어쓰지 않는다(protected)
 */
type Ownership =
  | { state: "absent" }
  | { state: "owned" | "owned-with-human-keys"; text: string; card: Card | null }
  | { state: "edited"; text: string; card: Card | null };

const pureSha = (card: Card): string => sha256(renderCard({ ...card, extra: {} }));

/**
 * 사람이 더한 내용이 도구 렌더로 손실 없이 다시 쓰이는가: 카드 frontmatter의 모든 줄이 도구 렌더(사람 키 포함)에 그대로 있어야 한다.
 * 주석·다른 따옴표·들여쓰기 등 도구가 재현하지 못하는 모양은 하나라도 있으면 false — 사람 글로 보고 보호한다.
 * 키의 위치는 상관없다(줄 집합 비교).
 */
function rerenderLossless(text: string, card: Card): boolean {
  const located = locate(text);
  if (located.kind !== "found") return false;
  // 비교 대상은 도구 렌더의 frontmatter 줄만 — 본문 줄(`# 제목` 등)과 우연히 같은 주석이 통과하지 않도록
  const rendered = locate(renderCard(card));
  const canon = new Set(rendered.kind === "found" ? rendered.raw.split("\n") : []);
  return located.raw.split("\n").every((line) => canon.has(line));
}

function cardOwnership(w: Wiki, receipt: Receipt, rel: string): Ownership {
  if (!exists(w.root, rel)) return { state: "absent" };
  const text = readText(w.root, rel);
  const parsed = tryParseCard(text, rel);
  const card = "card" in parsed ? parsed.card : null;
  if (receipt.files[rel] === sha256(text)) return { state: "owned", text, card };
  // 사람 키만 더했는지: 사람 키를 뺀 렌더가 소유 기록과 같고, 본문(frontmatter 아래)이 도구 렌더와 같아야 한다
  // (키 위치는 어디에 더했든 상관없다 — 다시 쓸 때 도구 키 뒤로 정렬된다)
  const sameBody = card !== null && bodyText(text, splitFrontmatter(text)) === bodyText(renderCard(card), splitFrontmatter(renderCard(card)));
  if (card !== null && Object.keys(card.extra).length > 0 && sameBody && rerenderLossless(text, card) && pureSha(card) === receipt.files[rel]) {
    return { state: "owned-with-human-keys", text, card };
  }
  return { state: "edited", text, card };
}

/** 위키 생성물이 마지막 기록 이후 사람이 고쳤는가(receipt sha ≠ 현재 sha). 없는 파일은 false. */
function userEdited(w: Wiki, receipt: Receipt, rel: string): boolean {
  if (!exists(w.root, rel)) return false;
  return receipt.files[rel] !== sha256(readRegular(w.root, rel));
}

// ───────────────────────── init ─────────────────────────

export function initWiki(root: string, contractFile: string, opts: { apply: boolean; now: string }) {
  const r = canonicalRoot(root);
  const raw = fs.readFileSync(contractFile);
  const contract = validateContract(parseJsonAt(raw.toString("utf8"), contractFile));
  const present = WIKI_DIRS.filter((d) => exists(r, d));
  if (present.length > 0) throw new Error(`위키가 이미 존재한다(덮어쓰기 없음): ${present.join(", ")}`);
  const w: Wiki = { root: r, contract, wikiDir: contract.wiki_dir };
  const writes: Write[] = [
    { rel: inWiki(w, CONTRACT_FILE), data: raw },
    { rel: inWiki(w, GITIGNORE_FILE), data: gitignoreBytes(contract) },
    { rel: inWiki(w, INDEX_FILE), data: Buffer.from(renderIndex(w, [])) },
    { rel: inWiki(w, LOG_FILE), data: Buffer.from(appendLog(LOG_HEADER, opts.now, ["init: 위키 생성"])) },
  ];
  if (opts.apply) commit(w, writes, { files: {} });
  return { applied: opts.apply, wiki_dir: w.wikiDir, writes: [...writes.map((x) => x.rel), inWiki(w, RECEIPT_FILE)] };
}

// ───────────────────────── 카드 생성 공통 ─────────────────────────

function buildFor(w: Wiki, src: string, now: string): Card {
  const s = readSource(w.root, src);
  return buildCard({ path: src, sha256: s.sha, text: s.text }, splitFrontmatter(s.text), w.contract, authorityFor(w.contract, src), now, s.decodeError);
}

/**
 * 기존 카드와 generated.at·사람 키(extra)를 제외하고 같으면 기존 바이트 유지(고정점 — 사람 키의 위치도 그대로).
 * 다르면 새로 렌더하며 기존 카드의 extra(사람이 더한 OKF 키)를 도구 키 뒤에 다시 쓴다.
 */
function reconcile(built: Card, prev: Card | null, prevText: string | null): { text: string; changed: boolean } {
  if (prev === null || prevText === null) return { text: renderCard(built), changed: true };
  const sameToolPart = renderCard({ ...built, extra: {}, generated: { ...built.generated, at: prev.generated.at } }) === renderCard({ ...prev, extra: {} });
  return sameToolPart ? { text: prevText, changed: false } : { text: renderCard({ ...built, extra: prev.extra }), changed: true };
}

function renderIndex(w: Wiki, cards: { src: string; card: Card }[]): string {
  const byType = new Map<string, { src: string; card: Card }[]>();
  for (const c of cards) {
    const list = byType.get(c.card.type);
    if (list === undefined) byType.set(c.card.type, [c]);
    else list.push(c);
  }
  const sections = [...byType.keys()].sort().map((t) => {
    const items = byType.get(t)!.map(({ src, card }) => {
      const label = (card.title ?? card.resource).replace(/[\[\]\n]/g, " ");
      const href = path.posix.relative(w.wikiDir, cardRel(w, src));
      return `* [${label}](${href})${card.description === null ? "" : ` - ${card.description.replace(/\n/g, " ")}`}`;
    });
    return `# ${t}\n\n${items.join("\n")}\n`;
  });
  const head = `---\n${emitYaml({ okf_version: OKF_VERSION })}---\n\n`;
  return head + (sections.length === 0 ? `# ${w.contract.project_name}\n\n카드 없음\n` : sections.join("\n"));
}

/** OKF §9: 최신 날짜 그룹이 위. 같은 날짜면 그 그룹 끝에 덧붙인다. */
function appendLog(current: string, now: string, entries: string[]): string {
  const date = now.slice(0, ISO_DATE_LEN);
  const lines = current.replace(/\n+$/, "").split("\n");
  const bullets = entries.map((e) => `- ${e}`);
  const first = lines.findIndex((l) => /^##\s/.test(l));
  if (first !== -1 && lines[first].trim() === `## ${date}`) {
    let end = first + 1;
    while (end < lines.length && !/^##\s/.test(lines[end])) end++;
    while (end > first + 1 && lines[end - 1].trim() === "") end--;
    lines.splice(end, 0, ...bullets);
  } else {
    const at = first === -1 ? lines.length : first;
    const lead = at > 0 && lines[at - 1].trim() !== "" ? [""] : [];
    const trail = first === -1 ? [] : [""];
    lines.splice(at, 0, ...lead, `## ${date}`, ...bullets, ...trail);
  }
  return lines.join("\n") + "\n";
}

// ───────────────────────── sync ─────────────────────────

/**
 * owner: 이 실행이 카드를 쓸 수 있는 원문 소유자(C12). 생략 = 전체(메인 로컬 관리자 재구축 전용 — 게이트웨이는 늘 지정한다).
 * requestId·payloadSha: 멱등 요청(C16). requestId 가 있으면 owner 도 있어야 한다.
 */
export type SyncOptions = {
  apply: boolean; now: string; acceptRemoved?: string[]; relocate?: [string, string][];
  owner?: string; requestId?: string; payloadSha?: string;
};

/**
 * owner 의 이번 반입 목록(INTAKE_MANIFEST)을 미러와 대조해 payload 해시를 돌려준다. 목록의 모든 파일이 미러에 같은 sha 로
 * 있어야 한다 — 하나라도 다르면 반입이 덜 끝났거나 손상된 것이다(거부). 목록에 없는 미러 파일(이전 입고분)은 대조하지 않는다.
 */
export function mirrorPayloadSha(root: string, owner: string): string {
  const w = open(root);
  const base = `${mirrorRootOf(w.contract)}/${owner}`;
  const manRel = `${base}/${INTAKE_MANIFEST}`;
  if (!exists(w.root, manRel)) throw new Error(`반입 목록 없음(${manRel}) — 반입이 끝나지 않았다`);
  const doc = parseJsonAt(readText(w.root, manRel), manRel) as { entries?: unknown };
  if (!Array.isArray(doc.entries)) throw new Error(`반입 목록 형식 오류: ${manRel}`);
  const entries = doc.entries as [string, string][];
  for (const [rel, want] of entries) {
    if (!isSafeRelative(rel)) throw new Error(`반입 목록에 안전하지 않은 경로: ${rel}`);
    const src = `${base}/${rel}`;
    if (!exists(w.root, src) || readSource(w.root, src).sha !== want) throw new Error(`미러 payload 불일치(반입 미완 또는 손상): ${src}`);
  }
  return payloadSha(entries);
}

export function syncWiki(root: string, opts: SyncOptions) {
  const w = open(root);
  const mirrorRoot = mirrorRootOf(w.contract);
  const owns = (src: string): boolean => opts.owner === undefined || ownerOf(src, mirrorRoot) === opts.owner;
  if (opts.requestId !== undefined) {
    if (opts.owner === undefined) throw new Error("멱등 요청(--request-id)은 소유자(--as)와 함께만 쓴다");
    if (opts.payloadSha === undefined) throw new Error("멱등 요청은 payload 해시(--payload-sha)가 필요하다");
    const prior = readLedger(w.root, w.wikiDir, opts.owner, opts.requestId);
    if (prior !== null) {
      if (prior.payload_sha256 !== opts.payloadSha) {
        throw new Error(`멱등 키 충돌: 같은 request_id(${opts.requestId})에 다른 payload — 재전송이 아니라 다른 요청이다(거부)`);
      }
      return { ...(prior.result as Record<string, unknown>), replayed: true };
    }
    if (opts.owner !== MAIN_OWNER) {
      const actual = mirrorPayloadSha(root, opts.owner);
      if (actual !== opts.payloadSha) throw new Error(`미러 payload 불일치(반입 미완 또는 손상): 보낸 ${opts.payloadSha} ≠ 미러 ${actual}`);
    }
  }
  const receipt = readReceipt(w);
  const allSources = listSources(w.root, w.contract);
  const sources = allSources.filter(owns);
  const sourceSet = new Set(sources);
  const existing = new Set(listCards(w, w.contract.layout.cards_dir).sources.filter(owns));
  const accept = (opts.acceptRemoved ?? []).map(safeRelative);
  const relocate = (opts.relocate ?? []).map(([a, b]) => [safeRelative(a), safeRelative(b)] as [string, string]);

  const orphans = [...existing].filter((s) => !sourceSet.has(s));
  const resolved = new Set<string>();
  for (const old of [...accept, ...relocate.map(([o]) => o)]) {
    if (resolved.has(old)) throw new Error(`수용 지정 중복: ${old}`);
    if (!orphans.includes(old)) throw new Error(`수용 지정이 고아가 아님(원문 존재 또는 카드 없음): ${old}`);
    resolved.add(old);
  }
  for (const [, neu] of relocate) if (!sourceSet.has(neu)) throw new Error(`이동 목적지가 원문 범위에 없음: ${neu}`);
  const blocked = orphans.filter((o) => !resolved.has(o));

  const writes: Write[] = [];
  const created: string[] = [];
  const updated: string[] = [];
  const needsReview: string[] = [];
  const protectedFiles: string[] = [];
  const invalid: string[] = [];
  const reverify: string[] = [];
  const owners = new Map<string, string | null>();
  const indexCards: { src: string; card: Card }[] = [];
  // 남의 카드는 쓰지 않되 index 에는 그대로 싣는다(누가 돌려도 index 가 같은 바이트가 되게)
  const foreign = new Map<string, Card>();
  for (const src of listCards(w, w.contract.layout.cards_dir).sources.filter((x) => !owns(x))) {
    const parsed = tryParseCard(readText(w.root, cardRel(w, src)), cardRel(w, src));
    if ("card" in parsed) foreign.set(src, parsed.card);
  }
  for (const src of allSources) {
    if (!owns(src)) {
      const card = foreign.get(src);
      if (card !== undefined) indexCards.push({ src, card });
      continue;
    }
    if (!sourceSet.has(src)) continue;
    const built = buildFor(w, src, opts.now);
    if (built.needs_review) needsReview.push(src);
    const rel = cardRel(w, src);
    const own = cardOwnership(w, receipt, rel);
    if (own.state === "edited") {
      // 사람이 본문·카드 키를 고친 카드는 덮어쓰지 않는다(원문이 바뀌었어도) — 보고만 한다
      protectedFiles.push(rel);
      if (own.card === null) invalid.push(rel);
      indexCards.push({ src, card: own.card ?? built });
      continue;
    }
    const prev = own.state === "absent" ? null : own.card;
    const prevText = own.state === "absent" ? null : own.text;
    const { text, changed } = reconcile(built, prev, prevText);
    if (changed && prev !== null && Object.hasOwn(prev.extra, HUMAN_ATTESTATION_KEY)) {
      // 사람 확인(OKF §5.2 verified)은 확인한 내용에 묶인다 — 바뀐 원문에 옮기지 않고 재확인을 요청한다
      reverify.push(src);
      indexCards.push({ src, card: prev });
      continue;
    }
    const merged: Card = { ...built, extra: prev?.extra ?? {} };
    if (changed) {
      writes.push({ rel, data: Buffer.from(text) });
      // 사람 키가 실린 카드는 사람 키를 뺀 렌더 해시를 소유 기록으로 둔다(remove는 사람 키 카드를 보존)
      if (Object.keys(merged.extra).length > 0) owners.set(rel, pureSha(merged));
      (prevText === null ? created : updated).push(src);
    }
    indexCards.push({ src, card: merged });
  }

  const archived: string[] = [];
  const logEntries: string[] = [];
  const archive = (old: string, note: string): void => {
    const target = archiveRel(w, old);
    if (exists(w.root, target)) throw new Error(`보존 위치에 이미 파일이 있다(덮어쓰기 없음): ${target}`);
    writes.push({ rel: target, data: readRegular(w.root, cardRel(w, old)) }, { rel: cardRel(w, old), data: null });
    // 옮긴 바이트를 새로 소유하지 않는다: 원래 카드의 소유 기록을 그대로 넘겨 사람 수정분이 계속 보호되게
    owners.set(target, receipt.files[cardRel(w, old)] ?? null);
    archived.push(old);
    logEntries.push(`보존이동: ${note}`);
  };
  for (const old of accept) archive(old, `${old} (원문 삭제 수용)`);
  for (const [old, neu] of relocate) archive(old, `${old} → ${neu} (이동)`);

  const logRel = inWiki(w, LOG_FILE);
  if (created.length + updated.length + archived.length > 0) {
    logEntries.unshift(`sync: 생성 ${created.length} · 갱신 ${updated.length} · 보존이동 ${archived.length}`);
    if (userEdited(w, receipt, logRel)) protectedFiles.push(logRel);
    else writes.push({ rel: logRel, data: Buffer.from(appendLog(exists(w.root, logRel) ? readText(w.root, logRel) : LOG_HEADER, opts.now, logEntries)) });
  }
  // remove 뒤에는 .gitignore가 없을 수 있다 → 캐시가 git에 드러나지 않도록 다시 만든다
  const ignoreRel = inWiki(w, GITIGNORE_FILE);
  if (!exists(w.root, ignoreRel)) writes.push({ rel: ignoreRel, data: gitignoreBytes(w.contract) });
  const indexRel = inWiki(w, INDEX_FILE);
  const index = renderIndex(w, indexCards);
  // remove 뒤에는 index.md가 없을 수 있다 → 새로 만든다
  if (!exists(w.root, indexRel) || index !== readText(w.root, indexRel)) {
    if (userEdited(w, receipt, indexRel)) protectedFiles.push(indexRel);
    else writes.push({ rel: indexRel, data: Buffer.from(index) });
  }

  const result = {
    applied: opts.apply, owner: opts.owner ?? "all", sources: sources.length, created, updated, archived,
    unchanged: sources.length - created.length - updated.length - reverify.length - protectedFiles.filter((p) => p.endsWith(CARD_SUFFIX)).length,
    protected: protectedFiles, invalid, reverify, orphans_blocked: blocked, needs_review: needsReview, writes: writes.map((x) => x.rel),
  };
  if (opts.apply) {
    if (blocked.length > 0) throw new Error(`원문 없는 고아 카드가 있어 적용 거부(--accept-removed 또는 --relocate로 명시 조정): ${blocked.join(", ")}`);
    // 원장(만료 정리·이번 요청 기록)은 같은 트랜잭션에 싣는다 — 효과와 기록이 함께 서거나 함께 사라진다. 소유 기록(receipt) 밖이다.
    const ops: Write[] = [];
    for (const rel of expiredLedger(w.root, w.wikiDir, opts.now)) { ops.push({ rel, data: null }); owners.set(rel, null); }
    if (opts.requestId !== undefined) {
      const rel = ledgerRel(w.wikiDir, opts.owner!, opts.requestId);
      const entry = { node: opts.owner!, request_id: opts.requestId, payload_sha256: opts.payloadSha!, recorded_at: opts.now, result };
      ops.push({ rel, data: Buffer.from(JSON.stringify(entry, null, 1) + "\n") });
      owners.set(rel, null);
    }
    if (writes.length + ops.length > 0) commit(w, [...writes, ...ops], receipt, owners);
  }
  return result;
}

// ───────────────────────── query·read·tree ─────────────────────────

type Loaded = { doc: IndexDoc; card: Card | null; invalid: boolean };

/** 메타 색인용: 중첩 값에서 스칼라만 문자열로 펼친다(null 제외) */
function scalars(v: unknown): string[] {
  if (v === null || v === undefined) return [];
  if (Array.isArray(v)) return v.flatMap(scalars);
  if (typeof v === "object") return Object.values(v as Record<string, unknown>).flatMap(scalars);
  return [String(v)];
}

function loadDocs(w: Wiki): Loaded[] {
  return listSources(w.root, w.contract).map((src) => {
    const s = readSource(w.root, src);
    const rel = cardRel(w, src);
    const parsed = exists(w.root, rel) ? tryParseCard(readText(w.root, rel), rel) : null;
    const card = parsed !== null && "card" in parsed ? parsed.card : null;
    // 카드 메타: type·tags·관례 키(source_meta)·미매핑 원문 키(source_meta_unknown) 값 — frontmatter에만 있는 식별자도 검색되게
    const meta = card === null ? [] : [card.type, ...card.tags, ...scalars(card.source_meta), ...scalars(card.source_meta_unknown)];
    const inferred = inferFromPath(src, w.contract.filename_pattern);
    const title = card?.title ?? (inferred.matches ? inferred.koTitle : path.posix.basename(src));
    return {
      card,
      invalid: parsed !== null && "error" in parsed,
      doc: {
        path: src, sha256: s.sha, title, metaText: meta.join(" "),
        // 질의에는 본문 시작 줄만 필요 — YAML 파싱 없이 구분선 위치만 찾는다
        lines: splitLines(s.text), bodyStartLine: locate(s.text).bodyStartLine,
        authorityRank: authorityFor(w.contract, src).authority_rank,
      },
    };
  });
}

export function queryWiki(root: string, termGroups: string[], limit?: number) {
  if (limit !== undefined && !(Number.isInteger(limit) && limit > 0)) throw new Error(`limit는 양의 정수: ${limit}`);
  const tokens = queryTokens(termGroups);
  if (tokens.length === 0) throw new Error(`질의어가 색인 가능한 토큰을 만들지 못함(1글자 영숫자·기호만): ${JSON.stringify(termGroups)}`);
  const w = open(root);
  const started = performance.now();
  const loaded = loadDocs(w);
  const docs = loaded.map((l) => l.doc);
  const cacheFile = cacheRel(w, INDEX_CACHE_FILE);
  const cached = exists(w.root, cacheFile) ? readText(w.root, cacheFile) : null;
  const { index, json, rebuilt, cacheReset } = loadOrRebuild(cached, docs);
  if (json !== cached) {
    // 파생 캐시 — 카드가 아니므로 --apply 대상이 아니다. 동시 질의(공유 잠금)가 함께 쓰므로 경합 검출 없이 원자 교체한다
    replaceDerived(w.root, cacheFile, Buffer.from(json));
  }
  const cfg = w.contract.search;
  const byPath = new Map(docs.map((d) => [d.path, d]));
  const weights = termWeights(index, tokens);
  const results: QueryResult[] = scoreQuery(index, weights, cfg).slice(0, limit ?? cfg.result_limit).map(({ path: p, score }) => {
    const doc = byPath.get(p)!;
    const tree = headingTree(doc.lines, doc.bodyStartLine);
    return { path: p, score, authority_rank: doc.authorityRank, title: doc.title, spans: findSpans(doc, tree, weights, cfg) };
  });
  return {
    terms: termGroups, query_tokens: tokens, unmatched_tokens: tokens.filter((t) => !weights.has(t)),
    results,
    stale: loaded.filter((l) => l.card !== null && l.card.source_sha256 !== l.doc.sha256).map((l) => l.doc.path),
    missing_cards: loaded.filter((l) => l.card === null && !l.invalid).map((l) => l.doc.path),
    invalid_cards: loaded.filter((l) => l.invalid).map((l) => l.doc.path),
    corpus: docs.length, index_rebuilt: rebuilt.length, index_cache_reset: cacheReset,
    elapsed_ms: Math.round(performance.now() - started),
  };
}

function sourceLines(w: Wiki, rel: string): { lines: string[]; bodyStartLine: number } {
  const src = safeRelative(rel);
  if (!listSources(w.root, w.contract).includes(src)) throw new Error(`원문 범위 밖 경로: ${rel}`);
  const text = readSource(w.root, src).text;
  return { lines: splitLines(text), bodyStartLine: locate(text).bodyStartLine };
}

export function readRef(root: string, ref: string) {
  const w = open(root);
  const { path: p, start, end } = parseRef(ref);
  return { ref, text: readSpan(sourceLines(w, p).lines, start, end) };
}

export function treeOf(root: string, rel: string): HeadingNode[] {
  const w = open(root);
  const { lines, bodyStartLine } = sourceLines(w, rel);
  return headingTree(lines, bodyStartLine);
}

// ───────────────────────── check·status ─────────────────────────

export function checkWiki(root: string) {
  const w = open(root);
  const issues: CheckIssue[] = [];
  const skip = new Set([w.contract.layout.cache_dir, LEDGER_DIR, JOURNAL_DIR]);
  for (const rel of listFiles(w.root, w.wikiDir, skip).filter((f) => f.endsWith(MD_SUFFIX))) {
    for (const i of checkConcept(readText(w.root, inWiki(w, rel)), rel)) issues.push({ ...i, path: inWiki(w, rel) });
  }
  const sources = listSources(w.root, w.contract);
  const { sources: cardSources, unrecognized } = listCards(w, w.contract.layout.cards_dir);
  for (const u of unrecognized) issues.push({ code: "card.unrecognized", path: u, detail: `카드 이름 규칙(*${CARD_SUFFIX}) 밖 파일` });
  const cards = new Set(cardSources);
  let needsReview = 0;
  for (const src of sources) {
    if (!cards.has(src)) { issues.push({ code: "card.missing", path: src, detail: "원문은 있으나 카드 없음" }); continue; }
    const parsed = tryParseCard(readText(w.root, cardRel(w, src)), cardRel(w, src));
    if ("error" in parsed) { issues.push({ code: "card.invalid", path: src, detail: parsed.error }); continue; }
    if (parsed.card.needs_review) needsReview++;
    if (parsed.card.source_sha256 !== readSource(w.root, src).sha) issues.push({ code: "card.stale", path: src, detail: "원문 해시가 카드와 다름" });
  }
  const srcSet = new Set(sources);
  for (const c of cards) if (!srcSet.has(c)) issues.push({ code: "card.orphan", path: c, detail: "카드는 있으나 원문 없음" });
  const archived = listCards(w, w.contract.layout.archive_dir).sources.length;
  return { ok: issues.length === 0, issues, counts: { sources: sources.length, cards: cards.size, archived, needs_review: needsReview } };
}

export function statusWiki(root: string) {
  const w = open(root);
  return {
    wiki_dir: w.wikiDir, project_name: w.contract.project_name,
    sources: listSources(w.root, w.contract).length,
    cards: listCards(w, w.contract.layout.cards_dir).sources.length,
    archived: listCards(w, w.contract.layout.archive_dir).sources.length,
    index_cache: exists(w.root, cacheRel(w, INDEX_CACHE_FILE)),
  };
}

// ───────────────────────── remove ─────────────────────────

export function removeWiki(root: string, opts: { apply: boolean }) {
  const w = open(root);
  const receipt = readReceipt(w);
  const deletes: string[] = [];
  const preserved: string[] = [];
  // 남는 receipt에 계속 둘 소유 기록: 사람 키만 더한 카드는 remove 뒤에도 원문 변경 시 갱신되게 순수 렌더 해시를 유지
  const remaining: Record<string, string> = {};
  const keep = inWiki(w, CONTRACT_FILE);
  for (const [rel, sha] of Object.entries(receipt.files).sort()) {
    if (rel === keep || !exists(w.root, rel)) continue;
    if (sha256(readRegular(w.root, rel)) === sha) { deletes.push(rel); continue; }
    preserved.push(rel);
    if (rel.endsWith(CARD_SUFFIX) && cardOwnership(w, receipt, rel).state === "owned-with-human-keys") remaining[rel] = sha;
  }
  const cacheFile = cacheRel(w, INDEX_CACHE_FILE);
  if (exists(w.root, cacheFile)) deletes.push(cacheFile);
  if (opts.apply) {
    // 남는 receipt = 사람 키 카드의 소유 기록만. 이후 sync --apply가 위키를 다시 채우며, 그 밖의 남은 파일(사람 수정분)은 보호한다.
    const tx = new Transaction(w.root, [w.wikiDir]);
    tx.apply(tx.plan([...deletes.map((rel) => ({ rel, data: null })), { rel: inWiki(w, RECEIPT_FILE), data: receiptBytes(remaining) }]));
    pruneEmptyAncestors(w, deletes);
  }
  return { applied: opts.apply, deletes, preserved, kept: [keep, inWiki(w, RECEIPT_FILE)] };
}

/** 지운 파일의 상위 디렉터리 중 비게 된 것만 wiki_dir 경계 안에서 제거 */
function pruneEmptyAncestors(w: Wiki, deleted: string[]): void {
  const dirs = new Set<string>();
  for (const rel of deleted) {
    for (let d = path.posix.dirname(rel); d !== w.wikiDir && isBelow(d, w.wikiDir); d = path.posix.dirname(d)) dirs.add(d);
  }
  // 깊은 디렉터리부터
  for (const d of [...dirs].sort((a, b) => b.split("/").length - a.split("/").length)) {
    const abs = path.join(w.root, d);
    if (fs.existsSync(abs) && fs.readdirSync(abs).length === 0) fs.rmdirSync(abs);
  }
}
