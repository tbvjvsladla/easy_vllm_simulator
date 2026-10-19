// 범위 제한 파일 쓰기 트랜잭션.
// 파일 단위 원자 교체 + 예외 시 전체 롤백. 프로세스 kill 원자성은 `journal` 옵션(concurrency.ts 저널)을 줄 때만 성립한다 —
// 쓰기 전 원본을 저널에 남기고, 다음 배타 잠금 보유자가 남은 저널로 되돌린다. 단일 writer 전제는 flock 이 집행한다.
import fs from "node:fs";
import { writeJournal, clearJournal } from "./concurrency.ts";
import path from "node:path";
import crypto from "node:crypto";

/** 권한 비트만 허용(setuid·setgid·sticky 거부) */
const PERMISSION_BITS = 0o777;
/** 새 파일 기본 권한: 소유자 쓰기, 그 외 읽기 */
const DEFAULT_FILE_MODE = 0o644;
/** 새 디렉터리 기본 권한: 소유자 쓰기, 그 외 읽기·탐색 */
const DEFAULT_DIR_MODE = 0o755;
/** 원자 쓰기 임시파일 접두사(대상과 같은 디렉터리에 생성) */
export const TEMP_PREFIX = ".wiki-desk-txn-";
/** 임시파일 이름 충돌 방지용 난수 바이트 수 */
const TEMP_RANDOM_BYTES = 8;
/** 제어문자(C0·DEL) */
const CONTROL_CHARS = /[\u0000-\u001f\u007f]/;

/** 쓰기 요청. data=null은 삭제이며 mode를 받지 않는다. */
export type Write =
  | { rel: string; data: Buffer; mode?: number }
  | { rel: string; data: null };

export type Preimage =
  | Readonly<{ kind: "missing" }>
  | Readonly<{ kind: "file"; sha256: string; mode: number; size: number }>;

/** plan()이 확정한 쓰기 1건: 쓰기는 mode가 반드시 정해져 있다. */
export type PlannedWrite =
  | Readonly<{ rel: string; data: Buffer; mode: number }>
  | Readonly<{ rel: string; data: null }>;

/** plan()이 발행한 계획(동결). 쓰기 대상의 preimage와 지문을 묶는다. */
export type Plan = Readonly<{
  fingerprint: string;
  /** rel 정렬 */
  writes: readonly PlannedWrite[];
  preimages: Readonly<Record<string, Preimage>>;
}>;

export type ApplyResult = { mutations: number; writes: number; deletes: number; createdDirs: number };

/** fault: 실제 변경 1건마다 호출되는 테스트 훅(롤백 검증용). CLI는 넘기지 않는다. */
export type ApplyOptions = { fault?: (count: number, rel: string) => void; journal?: { wikiDir: string } };

/** sha256 16진 표기(64자) */
export const SHA256_HEX = /^[0-9a-f]{64}$/;

export function sha256(data: Buffer | string): string {
  return crypto.createHash("sha256").update(data).digest("hex");
}

/** 절대경로·'..'·'.'·빈 segment·백슬래시·제어문자·드라이브(첫 segment ':')가 없는 '/' 구분 상대경로인가 */
export function isSafeRelative(rel: unknown): rel is string {
  if (typeof rel !== "string") return false;
  const parts = rel.split("/");
  return !(rel === "" || rel.includes("\\") || CONTROL_CHARS.test(rel) || rel.startsWith("/") ||
    parts.some((p) => p === "" || p === "." || p === "..") || parts[0]!.includes(":"));
}

/** rel이 scope 자신이거나 그 하위인가('/' 경계 기준 — 이름 접두만 같은 형제는 아님) */
export function isBelow(rel: string, scope: string): boolean {
  return rel === scope || rel.startsWith(scope + "/");
}

export function safeRelative(rel: string): string {
  if (!isSafeRelative(rel)) throw new Error(`안전하지 않은 상대경로: ${JSON.stringify(rel)}`);
  return rel;
}

/** 루트 자신과 조상이 모두 실제 디렉터리(심링크 아님)인지 확인하고 절대경로를 돌려준다. */
export function canonicalRoot(root: string): string {
  if (root.split(/[\\/]/).includes("..")) throw new Error(`루트에 '..' 금지: ${root}`);
  const abs = path.resolve(root);
  if (abs === path.parse(abs).root) throw new Error("파일시스템 루트는 프로젝트 루트가 아니다");
  let cursor = path.parse(abs).root;
  for (const part of abs.slice(cursor.length).split(path.sep)) {
    cursor = path.join(cursor, part);
    const st = fs.lstatSync(cursor, { throwIfNoEntry: false });
    if (st === undefined) throw new Error(`프로젝트 루트가 존재해야 한다: ${abs}`);
    if (st.isSymbolicLink()) throw new Error(`루트/조상 심볼릭링크 거부: ${cursor}`);
    if (!st.isDirectory()) throw new Error(`루트/조상이 디렉터리가 아니다: ${cursor}`);
  }
  return abs;
}

/** 루트 안 경로로 해석. 심링크·특수파일·파일 상위를 거부한다(미존재 꼬리는 허용). */
export function resolveInside(root: string, rel: string): string {
  const base = canonicalRoot(root);
  const parts = safeRelative(rel).split("/");
  let cursor = base;
  let deepestExisting = base;
  for (const [i, part] of parts.entries()) {
    cursor = path.join(cursor, part);
    const st = fs.lstatSync(cursor, { throwIfNoEntry: false });
    if (st === undefined) break;
    if (st.isSymbolicLink()) throw new Error(`심볼릭링크 거부: ${rel}`);
    if (!st.isFile() && !st.isDirectory()) throw new Error(`특수 파일 거부: ${rel}`);
    if (i < parts.length - 1 && !st.isDirectory()) throw new Error(`디렉터리가 아닌 상위 경로: ${rel}`);
    deepestExisting = cursor;
  }
  // 이중 확인: 실제로 존재하는 가장 깊은 지점의 realpath가 루트 안이어야 한다.
  const real = fs.realpathSync(deepestExisting);
  if (real !== base && !real.startsWith(base + path.sep)) throw new Error(`루트 밖 경로: ${rel}`);
  return path.join(base, ...parts);
}

export function readRegular(root: string, rel: string): Buffer {
  const target = resolveInside(root, rel);
  // 검사 이후 바뀐 최종 심링크도 따라가지 않는다.
  const fd = fs.openSync(target, fs.constants.O_RDONLY | fs.constants.O_NOFOLLOW);
  try {
    if (!fs.fstatSync(fd).isFile()) throw new Error(`일반 파일이 아니다: ${rel}`);
    return fs.readFileSync(fd);
  } finally {
    fs.closeSync(fd);
  }
}

function preimageOf(root: string, rel: string): Preimage {
  const st = fs.lstatSync(resolveInside(root, rel), { throwIfNoEntry: false });
  if (st === undefined) return { kind: "missing" };
  if (!st.isFile()) throw new Error(`쓰기 대상이 디렉터리다: ${rel}`);
  const data = readRegular(root, rel);
  return { kind: "file", sha256: sha256(data), mode: st.mode & PERMISSION_BITS, size: data.length };
}

function samePreimage(a: Preimage, b: Preimage): boolean {
  return JSON.stringify(a) === JSON.stringify(b);
}

function fingerprintOf(writes: readonly PlannedWrite[], preimages: Readonly<Record<string, Preimage>>): string {
  const body = writes.map((w) => (w.data === null ? [w.rel, null, null] : [w.rel, sha256(w.data), w.mode]));
  return sha256(JSON.stringify([body, preimages]));
}

function fsyncDir(dir: string): void {
  const fd = fs.openSync(dir, "r");
  try {
    fs.fsyncSync(fd);
  } finally {
    fs.closeSync(fd);
  }
}

/** 임시파일 쓰기 → fsync → rename → 상위 fsync. 임시파일은 성공·실패 모두 정리한다. */
function atomicWrite(target: string, data: Buffer, mode: number): void {
  const dir = path.dirname(target);
  const temporary = path.join(dir, TEMP_PREFIX + crypto.randomBytes(TEMP_RANDOM_BYTES).toString("hex"));
  try {
    const fd = fs.openSync(temporary, "wx", mode);
    try {
      fs.fchmodSync(fd, mode); // umask 무관하게 정확한 mode
      fs.writeFileSync(fd, data);
      fs.fsyncSync(fd);
    } finally {
      fs.closeSync(fd);
    }
    fs.renameSync(temporary, target);
    fsyncDir(dir);
  } finally {
    fs.rmSync(temporary, { force: true }); // rename 성공 시 이미 없음
  }
}


/**
 * 파생물(색인 캐시) 한 파일의 원자 교체 — preimage 대조 없이 마지막 쓰기가 이긴다. 공유 잠금 아래 동시 질의가
 * 같은 파생물을 쓰므로 경합 검출이 오히려 질의를 실패시킨다(내용은 같은 입력의 결정론 산출이다). 원문·카드에는 쓰지 않는다.
 */
export function replaceDerived(root: string, rel: string, data: Buffer): void {
  const target = resolveInside(root, rel);
  fs.mkdirSync(path.dirname(target), { recursive: true, mode: DEFAULT_DIR_MODE });
  atomicWrite(target, data, DEFAULT_FILE_MODE);
}

export class Transaction {
  readonly root: string;
  readonly scopes: readonly string[];
  /** 이 트랜잭션이 발행한 계획만 적용 가능(직렬화·복제 계획 거부) */
  #issued = new WeakMap<Plan, string>();

  /** scopes: 쓰기 허용 상대경로 접두(이 밖의 쓰기는 plan 단계에서 거부) */
  constructor(root: string, scopes: string[]) {
    this.root = canonicalRoot(root);
    this.scopes = scopes.map(safeRelative);
  }

  /** 변경 0건. 모든 사전조건을 확인하고 preimage를 기록한다. */
  plan(writes: Write[]): Plan {
    const sorted = [...writes].sort((a, b) => (a.rel < b.rel ? -1 : a.rel > b.rel ? 1 : 0));
    const rels = new Set<string>();
    for (const w of sorted) {
      safeRelative(w.rel);
      if (rels.has(w.rel)) throw new Error(`중복 경로: ${w.rel}`);
      rels.add(w.rel);
      if (!this.scopes.some((s) => isBelow(w.rel, s))) throw new Error(`쓰기 범위 밖 경로: ${w.rel}`);
      const mode = "mode" in w ? w.mode : undefined;
      if (mode !== undefined && !(Number.isInteger(mode) && mode >= 0 && mode <= PERMISSION_BITS)) {
        throw new Error(`잘못된 mode: ${w.rel}`);
      }
    }
    for (const rel of rels) {
      const ancestor = [...rels].find((other) => other !== rel && rel.startsWith(other + "/"));
      if (ancestor !== undefined) throw new Error(`파일/조상 경로 충돌: ${ancestor} ↔ ${rel}`);
    }
    const preimages: Record<string, Preimage> = {};
    const planned: PlannedWrite[] = [];
    for (const w of sorted) {
      const pre = preimageOf(this.root, w.rel);
      if (w.data === null && pre.kind === "missing") throw new Error(`삭제 대상이 없다: ${w.rel}`);
      preimages[w.rel] = pre;
      planned.push(w.data === null
        ? { rel: w.rel, data: null }
        : { rel: w.rel, data: Buffer.from(w.data), mode: w.mode ?? (pre.kind === "file" ? pre.mode : DEFAULT_FILE_MODE) });
    }
    const fingerprint = fingerprintOf(planned, preimages);
    const plan: Plan = { fingerprint, writes: planned, preimages };
    Object.freeze(plan);
    Object.freeze(planned);
    planned.forEach((w) => Object.freeze(w));
    Object.freeze(preimages);
    this.#issued.set(plan, fingerprint);
    return plan;
  }

  /** 적용 직전 preimage 재검증 → 원자 쓰기 → 읽기 재확인. 실패 시 전체 롤백 후 원 예외를 다시 던진다. */
  apply(plan: Plan, options: ApplyOptions = {}): ApplyResult {
    const sealed = this.#issued.get(plan);
    if (sealed === undefined) throw new Error("이 트랜잭션이 발행한 계획만 적용할 수 있다");
    if (fingerprintOf(plan.writes, plan.preimages) !== sealed || plan.fingerprint !== sealed) {
      throw new Error("계획 변조 감지");
    }
    // 변경 전 전수 재검증(경합 감지). 롤백용 원본 바이트도 이때 확보한다.
    const original = new Map<string, Buffer>();
    for (const w of plan.writes) {
      const now = preimageOf(this.root, w.rel);
      if (!samePreimage(now, plan.preimages[w.rel]!)) throw new Error(`계획 이후 경합 변경: ${w.rel}`);
      if (now.kind === "file") original.set(w.rel, readRegular(this.root, w.rel));
    }

    if (options.journal !== undefined) {
      // 새로 생길 디렉터리(얕은 것부터) — kill 뒤 복구가 빈 디렉터리 잔재까지 지운다
      const willCreate = new Set<string>();
      for (const w of plan.writes) {
        if (w.data === null) continue;
        const parts = w.rel.split("/");
        for (let i = 1; i < parts.length; i++) {
          const dirRel = parts.slice(0, i).join("/");
          if (fs.lstatSync(resolveInside(this.root, dirRel), { throwIfNoEntry: false }) === undefined) willCreate.add(dirRel);
        }
      }
      writeJournal(this.root, options.journal.wikiDir, plan.fingerprint, plan.writes.map((w) => w.rel), [...willCreate]);
    }
    const created: string[] = [];
    const touched: string[] = [];
    const result: ApplyResult = { mutations: 0, writes: 0, deletes: 0, createdDirs: 0 };
    const step = (rel: string): void => {
      result.mutations += 1;
      options.fault?.(result.mutations, rel);
    };
    const ensureParents = (rel: string): void => {
      const parts = rel.split("/");
      for (let i = 1; i < parts.length; i++) {
        const dirRel = parts.slice(0, i).join("/");
        const dir = resolveInside(this.root, dirRel);
        if (fs.lstatSync(dir, { throwIfNoEntry: false }) !== undefined) continue;
        fs.mkdirSync(dir, { mode: DEFAULT_DIR_MODE });
        created.push(dirRel);
        fs.chmodSync(dir, DEFAULT_DIR_MODE);
        fsyncDir(path.dirname(dir));
        result.createdDirs += 1;
        step(dirRel);
      }
    };

    try {
      for (const w of plan.writes) {
        if (w.data === null) continue; // 삭제는 쓰기 다음
        const want: Preimage = { kind: "file", sha256: sha256(w.data), mode: w.mode, size: w.data.length };
        if (samePreimage(plan.preimages[w.rel]!, want)) continue; // 동일 → 변경 없음
        ensureParents(w.rel);
        touched.push(w.rel);
        atomicWrite(resolveInside(this.root, w.rel), w.data, w.mode);
        result.writes += 1;
        step(w.rel);
      }
      for (const w of plan.writes.filter((x) => x.data === null)) {
        const target = resolveInside(this.root, w.rel);
        touched.push(w.rel);
        fs.unlinkSync(target);
        fsyncDir(path.dirname(target));
        result.deletes += 1;
        step(w.rel);
      }
      // 읽기 재확인: 실제 결과가 계획과 다르면 롤백한다.
      for (const w of plan.writes) {
        const want: Preimage = w.data === null
          ? { kind: "missing" }
          : { kind: "file", sha256: sha256(w.data), mode: w.mode, size: w.data.length };
        if (!samePreimage(preimageOf(this.root, w.rel), want)) throw new Error(`읽기 재확인 불일치: ${w.rel}`);
      }
    } catch (failure) {
      const errors = this.#rollback(plan, original, touched, created);
      // 롤백이 불완전하면 저널을 남긴다 — 다음 배타 보유자가 마저 되돌린다
      if (errors.length > 0) throw new Error(`롤백 불완전: ${errors.join("; ")}`, { cause: failure });
      if (options.journal !== undefined) clearJournal(this.root, options.journal.wikiDir);
      throw failure;
    }
    if (options.journal !== undefined) clearJournal(this.root, options.journal.wikiDir);
    return result;
  }

  /** 필수 롤백 경로: 단계별 실패를 모아 끝까지 진행하고, 남은 불일치를 보고한다. */
  #rollback(plan: Plan, original: Map<string, Buffer>, touched: string[], created: string[]): string[] {
    const errors: string[] = [];
    const attempt = (label: string, action: () => void): void => {
      try {
        action();
      } catch (err) {
        errors.push(`${label}: ${(err as Error).message}`);
      }
    };
    for (const rel of [...touched].reverse()) {
      const pre = plan.preimages[rel]!;
      attempt(rel, () => {
        const target = resolveInside(this.root, rel);
        if (pre.kind === "file") atomicWrite(target, original.get(rel)!, pre.mode);
        else if (fs.lstatSync(target, { throwIfNoEntry: false }) !== undefined) {
          fs.unlinkSync(target);
          fsyncDir(path.dirname(target));
        }
      });
    }
    for (const rel of [...created].reverse()) {
      attempt(rel, () => {
        const dir = resolveInside(this.root, rel);
        fs.rmdirSync(dir);
        fsyncDir(path.dirname(dir));
      });
    }
    for (const w of plan.writes) {
      attempt(`복원 확인 ${w.rel}`, () => {
        if (!samePreimage(preimageOf(this.root, w.rel), plan.preimages[w.rel]!)) throw new Error("preimage 불일치");
      });
    }
    return errors;
  }
}
