// N개 접근점(메인 로컬 + 서브 게이트웨이)이 한 위키를 쓰는 규정. 정본 설명은 references/concurrency.md.
// 기법 대응(h_report §4 의사결정 순서):
//   C12 키별 단일 소유자 — ownerOf(): 원문 키마다 쓰는 노드가 하나다(메인 원문 = main · 미러 = 그 node_id)
//   C06 상호배제 — 커널 flock(1). 쓰기는 배타, 읽기는 공유. 프로세스가 죽으면 커널이 푼다(lease 없음 → C15 fencing 불요)
//   원자성 — 저널(write-ahead 의도 기록). 쓰다 죽으면 다음 배타 보유자가 저널로 직전 상태로 되돌린다
//   C16 멱등성 — 요청 원장(node_id + request_id + payload sha256). 같은 키·같은 해시 = 저장 결과 재사용, 해시 다름 = 충돌 거부
import fs from "node:fs";
import path from "node:path";
import crypto from "node:crypto";
import { spawnSync } from "node:child_process";
import { readRegular, resolveInside, sha256, isSafeRelative, TEMP_PREFIX } from "./fs-safety.ts";

/** 위키 안 운영 디렉터리·파일 이름(OKF concept 아님 — 전부 .md 가 아니다) */
export const LOCK_FILE = ".lock";
export const JOURNAL_DIR = ".journal";
export const JOURNAL_FILE = "txn.json";
export const LEDGER_DIR = ".requests";
/** 잠금을 이미 쥔 자식임을 알리는 환경변수(값 = x|s) */
export const LOCK_ENV = "WIKI_DESK_LOCK_HELD";
/** 공유 잠금 아래에서 남은 저널을 만난 읽기 자식이 내는 종료코드 — 래퍼가 배타로 승격해 다시 돈다 */
export const EXIT_NEEDS_RECOVERY = 75;
/** 잠금 대기 상한(초). 넘으면 fail-loud 로 끝낸다(무한 대기 ✗) */
export const LOCK_WAIT_SECONDS = 120;
/** 요청 원장 보존 수명(일) — 다음 쓰기 잠금 보유자가 만료분을 정리한다 */
export const LEDGER_RETENTION_DAYS = 30;
const DAY_MS = 86_400_000;
/** 미러 루트 기본값(계약 확장 키 node_mirror_root 가 없을 때) */
export const DEFAULT_NODE_MIRROR_ROOT = "sync_staging/library_mirror";
/** 서브 반입 목록(미러 루트의 파일 · 경로→sha256) — 이번 입고가 보낸 집합. `.md` 가 아니라 원문으로 색인되지 않는다 */
export const INTAKE_MANIFEST = ".wiki-desk-intake.json";
/** 메인 원문의 소유자 이름 */
export const MAIN_OWNER = "main";
/** node_id 문법 — 경로 성분으로 쓰이므로 닫힌 문자 집합 */
export const NODE_ID_RE = /^[a-z0-9][a-z0-9_-]{0,62}$/;
export const REQUEST_ID_RE = /^[A-Za-z0-9_-]{8,80}$/;

export type LockMode = "x" | "s";

// ───────────── C12 소유 ─────────────

/** 원문의 소유 노드. 미러 루트 아래 첫 성분이 node_id 다. */
export function ownerOf(src: string, mirrorRoot: string): string {
  const prefix = mirrorRoot + "/";
  if (!src.startsWith(prefix)) return MAIN_OWNER;
  const node = src.slice(prefix.length).split("/")[0]!;
  return node;
}

export function assertNodeId(node: string): string {
  if (!NODE_ID_RE.test(node) || node === MAIN_OWNER) throw new Error(`node_id 형식 오류(또는 예약어 main): ${JSON.stringify(node)}`);
  return node;
}

// ───────────── C06 상호배제 — flock(1) 재실행 ─────────────

/**
 * 잠금이 없으면 flock(1) 아래에서 같은 명령을 다시 실행하고 그 종료코드로 끝낸다(반환하지 않는다).
 * 잠금을 이미 쥐었으면(LOCK_ENV) 아무것도 하지 않고 돌아온다. 잠금 보유자는 자식 프로세스이며, 자식이 죽으면
 * flock 이 끝나고 커널이 잠금을 푼다 — 그래서 남는 잠금(stale lock)이 구조적으로 없다.
 * 공유로 돌던 자식이 남은 저널을 만나면 EXIT_NEEDS_RECOVERY 로 끝나고, 여기서 배타로 한 번 승격해 다시 돈다.
 */
export function reexecUnderLock(wikiAbs: string, mode: LockMode): void {
  if (process.env[LOCK_ENV] !== undefined) return;
  const lockPath = path.join(wikiAbs, LOCK_FILE);
  const run = (m: LockMode): number => {
    const r = spawnSync("flock", [m === "x" ? "-x" : "-s", "-w", String(LOCK_WAIT_SECONDS), lockPath,
      process.execPath, ...process.execArgv, ...process.argv.slice(1)], { stdio: "inherit", env: { ...process.env, [LOCK_ENV]: m } });
    if (r.error !== undefined) {
      // flock(1)(util-linux) 부재는 조용히 무잠금으로 대체하지 않는다 — 동시성 규정이 집행 불가가 된다
      throw new Error(`flock(1) 실행 실패 — util-linux 가 필요하다(무잠금 대체 ✗): ${r.error.message}`);
    }
    if (r.signal !== null) return 128 + (({ SIGKILL: 9, SIGTERM: 15, SIGINT: 2 } as Record<string, number>)[r.signal] ?? 1);
    return r.status ?? 1;
  };
  let code = run(mode);
  if (code === EXIT_NEEDS_RECOVERY && mode === "s") code = run("x");
  if (code === 1 && !fs.existsSync(lockPath)) process.stderr.write(`잠금 파일을 만들지 못했다: ${lockPath}\n`);
  process.exit(code);
}

export function heldLock(): LockMode | null {
  const v = process.env[LOCK_ENV];
  return v === "x" || v === "s" ? v : null;
}

// ───────────── 원자성 — 저널 ─────────────

type JournalEntry = { rel: string; pre: { kind: "missing" } | { kind: "file"; mode: number; b64: string } };
type Journal = { version: 1; fingerprint: string; entries: JournalEntry[]; created_dirs: string[] };

const journalRel = (wikiDir: string): string => `${wikiDir}/${JOURNAL_DIR}/${JOURNAL_FILE}`;

function fsyncDir(dir: string): void {
  const fd = fs.openSync(dir, "r");
  try { fs.fsyncSync(fd); } finally { fs.closeSync(fd); }
}

function writeDurable(abs: string, data: Buffer, mode: number): void {
  fs.mkdirSync(path.dirname(abs), { recursive: true, mode: 0o755 });
  const tmp = path.join(path.dirname(abs), TEMP_PREFIX + crypto.randomBytes(8).toString("hex"));
  try {
    const fd = fs.openSync(tmp, "wx", mode);
    try { fs.fchmodSync(fd, mode); fs.writeFileSync(fd, data); fs.fsyncSync(fd); } finally { fs.closeSync(fd); }
    fs.renameSync(tmp, abs);
    fsyncDir(path.dirname(abs));
  } finally {
    fs.rmSync(tmp, { force: true });
  }
}

/** 변경 직전 호출: 대상 원본 바이트와 새로 만들 디렉터리를 기록한다(fsync 후 반환). */
export function writeJournal(root: string, wikiDir: string, fingerprint: string, rels: readonly string[], createdDirs: readonly string[]): void {
  const entries: JournalEntry[] = rels.map((rel) => {
    const st = fs.lstatSync(resolveInside(root, rel), { throwIfNoEntry: false });
    if (st === undefined) return { rel, pre: { kind: "missing" } };
    return { rel, pre: { kind: "file", mode: st.mode & 0o777, b64: readRegular(root, rel).toString("base64") } };
  });
  const j: Journal = { version: 1, fingerprint, entries, created_dirs: [...createdDirs] };
  writeDurable(resolveInside(root, journalRel(wikiDir)), Buffer.from(JSON.stringify(j)), 0o644);
}

export function clearJournal(root: string, wikiDir: string): void {
  const abs = resolveInside(root, journalRel(wikiDir));
  if (fs.existsSync(abs)) { fs.unlinkSync(abs); fsyncDir(path.dirname(abs)); }
  // 빈 저널 디렉터리도 남기지 않는다(잔재 0)
  const dir = path.dirname(abs);
  if (fs.existsSync(dir) && fs.readdirSync(dir).length === 0) fs.rmdirSync(dir);
}

export function journalPending(root: string, wikiDir: string): boolean {
  return fs.existsSync(path.join(root, journalRel(wikiDir)));
}

/** 원자 쓰기가 kill 로 남긴 임시파일(위키 안 어디든) */
function tempDebris(root: string, wikiDir: string): string[] {
  const out: string[] = [];
  const walk = (rel: string): void => {
    const abs = path.join(root, rel);
    if (!fs.existsSync(abs)) return;
    for (const e of fs.readdirSync(abs, { withFileTypes: true })) {
      const child = `${rel}/${e.name}`;
      if (e.isDirectory()) walk(child);
      else if (e.name.startsWith(TEMP_PREFIX)) out.push(child);
    }
  };
  walk(wikiDir);
  return out;
}

/**
 * 배타 잠금 보유자만 부른다. 남은 저널이 있으면 기록된 원본으로 되돌리고(직전 커밋 상태), 원자 쓰기 임시파일
 * 잔재를 지운다. 무엇을 했는지 돌려준다(침묵 복구 ✗ — 호출자가 출력에 싣는다).
 */
export function recoverJournal(root: string, wikiDir: string): { rolled_back: string[]; debris_removed: string[] } | null {
  if (heldLock() !== "x") throw new Error("저널 복구는 배타 잠금 아래에서만 한다");
  const rel = journalRel(wikiDir);
  const debris = tempDebris(root, wikiDir);
  const hadJournal = journalPending(root, wikiDir);
  if (!hadJournal && debris.length === 0) return null;
  const rolled: string[] = [];
  if (hadJournal) {
    const j = JSON.parse(readRegular(root, rel).toString("utf8")) as Journal;
    for (const e of j.entries) {
      if (!isSafeRelative(e.rel) || !(e.rel === wikiDir || e.rel.startsWith(wikiDir + "/"))) throw new Error(`저널 항목이 위키 밖: ${e.rel}`);
      const abs = resolveInside(root, e.rel);
      if (e.pre.kind === "file") {
        const want = Buffer.from(e.pre.b64, "base64");
        if (!fs.existsSync(abs) || sha256(fs.readFileSync(abs)) !== sha256(want)) { writeDurable(abs, want, e.pre.mode); rolled.push(e.rel); }
      } else if (fs.existsSync(abs)) {
        fs.unlinkSync(abs); fsyncDir(path.dirname(abs)); rolled.push(e.rel);
      }
    }
    for (const d of [...j.created_dirs].sort((a, b) => b.split("/").length - a.split("/").length)) {
      const abs = path.join(root, d);
      if (fs.existsSync(abs) && fs.readdirSync(abs).length === 0) fs.rmdirSync(abs);
    }
  }
  for (const d of debris) fs.rmSync(path.join(root, d), { force: true });
  if (hadJournal) clearJournal(root, wikiDir);
  return { rolled_back: rolled, debris_removed: debris };
}

// ───────────── C16 멱등성 — 요청 원장 ─────────────

export type LedgerEntry = { node: string; request_id: string; payload_sha256: string; recorded_at: string; result: unknown };

export const ledgerRel = (wikiDir: string, node: string, requestId: string): string => `${wikiDir}/${LEDGER_DIR}/${node}/${requestId}.json`;

export function readLedger(root: string, wikiDir: string, node: string, requestId: string): LedgerEntry | null {
  const rel = ledgerRel(wikiDir, node, requestId);
  if (!fs.existsSync(path.join(root, rel))) return null;
  return JSON.parse(readRegular(root, rel).toString("utf8")) as LedgerEntry;
}

/** 보존 수명이 지난 원장 파일(루트 상대) — 다음 쓰기 트랜잭션이 삭제로 싣는다 */
export function expiredLedger(root: string, wikiDir: string, now: string): string[] {
  const base = path.join(root, wikiDir, LEDGER_DIR);
  if (!fs.existsSync(base)) return [];
  const cutoff = Date.parse(now) - LEDGER_RETENTION_DAYS * DAY_MS;
  const out: string[] = [];
  for (const node of fs.readdirSync(base)) {
    const dir = path.join(base, node);
    if (!fs.statSync(dir).isDirectory()) continue;
    for (const f of fs.readdirSync(dir)) {
      const rel = `${wikiDir}/${LEDGER_DIR}/${node}/${f}`;
      try {
        const e = JSON.parse(fs.readFileSync(path.join(dir, f), "utf8")) as LedgerEntry;
        if (Date.parse(e.recorded_at) < cutoff) out.push(rel);
      } catch {
        // 깨진 원장 파일은 만료 판정 대상이 아니다 — 지우지 않고 남겨 사람이 보게 한다(침묵 삭제 ✗)
      }
    }
  }
  return out.sort();
}

/** 미러 원문 집합의 payload 해시: (미러 루트 기준 경로, 원문 sha256) 정렬 목록의 sha256 */
export function payloadSha(entries: readonly (readonly [string, string])[]): string {
  return sha256(JSON.stringify([...entries].sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0))));
}
