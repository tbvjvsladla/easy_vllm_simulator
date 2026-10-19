// PII 유출 가드 — 트리(또는 지정한 파일 목록)에서 민감 문자열을 찾는다.
// 리터럴은 **비추적** 공유 용어 파일(기본 <repo>/.claude/pii_terms.txt · 한 줄 하나 · '#' 주석)에서 읽는다 —
// 추적·배포되는 이 스크립트에 PII 리터럴을 굽지 않는다(포인터 원칙). 용어 파일이 없으면 일반 패턴만 가드한다.
// 호출자가 둘이다: 사람이 트리를 훑을 때(이 CLI)와 runtime_selftest tripwire ⑤(추적 배포면 전수 · --files-from).
// 두 호출자가 같은 커널(scanLines)·같은 면제 규칙을 쓴다.
//   node scan-forbidden-strings.ts <root> [--term T]... [--terms-file F]
//   node scan-forbidden-strings.ts --files-from <목록 파일|-> [--json]   (목록 = 한 줄에 경로 하나)
//   node scan-forbidden-strings.ts --stdin-text [--term T]...             (stdin 텍스트 한 덩이 → 적중 JSON · 단위 시험용)
//   node scan-forbidden-strings.ts --meta                                 (면제 마커·크기 상한·기본 용어 파일 JSON)
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { isUtf8 } from "node:buffer";
import { parseArgs } from "node:util";

// 일반 가드(패턴이지 PII 가 아니다) — 용어 파일 없이도 켜진다. RFC1918 사설 대역 접두.
const GENERIC_PATTERNS = [/192\.168\./];
const SKIP_DIRS = new Set([".git", "__pycache__", ".pytest_cache", "dist", "node_modules"]);
// 확장자 allowlist 를 쓰지 않는다(2026-09-01 교정 — `.sh` 사각지대로 운영자 경로가 배포됐다). 바이너리는 UTF-8 판정이 거른다.
const MAX_BYTES = 4 * 1024 * 1024; // 이보다 큰 파일은 스캔하지 않고 **소리내어** 보고한다(침묵 생략 금지)
// 행 단위 면제 마커 — 탐지기 자신의 양성 픽스처를 지우지 않고 그 한 줄만 면제한다(역-오라클). 면제분은 EXEMPT 로 출력한다.
export const EXEMPT_MARK = "pii-scan-fixture";

const HERE = path.dirname(fileURLToPath(import.meta.url));
// scripts/ → wiki-desk → skills → .claude
const DEFAULT_TERMS_FILE = path.resolve(HERE, "..", "..", "..", "pii_terms.txt");

export type Hit = { line: number; what: string; exempt: boolean };

/** 한 텍스트의 모든 적중(행 번호·무엇·면제 여부) — 단일 스캔 커널 */
export function scanLines(text: string, terms: readonly string[]): Hit[] {
  const out: Hit[] = [];
  text.split(/\r\n|\n|\r/).forEach((line, i) => {
    const exempt = line.includes(EXEMPT_MARK);
    const found = terms.filter((t) => t !== "" && line.includes(t)).map((t) => `term:${t}`);
    for (const pat of GENERIC_PATTERNS) {
      const m = pat.exec(line);
      if (m !== null) found.push(`generic:${m[0]}`);
    }
    for (const what of found) out.push({ line: i + 1, what, exempt });
  });
  return out;
}

export function loadTerms(file: string): string[] {
  if (!fs.existsSync(file) || !fs.statSync(file).isFile()) return [];
  return fs.readFileSync(file, "utf8").split(/\r?\n/).map((l) => l.trim()).filter((l) => l !== "" && !l.startsWith("#"));
}

type Report = { hits: (Hit & { path: string })[]; exempted: (Hit & { path: string })[]; skippedLarge: string[]; unreadable: string[]; scanned: number };

export function scanFiles(files: Iterable<string>, terms: readonly string[], termsFile: string): Report {
  const r: Report = { hits: [], exempted: [], skippedLarge: [], unreadable: [], scanned: 0 };
  const termsReal = fs.existsSync(termsFile) ? fs.realpathSync(termsFile) : null;
  for (const p of files) {
    let st: fs.Stats;
    try { st = fs.statSync(p); } catch (e) { r.unreadable.push(`${p}: ${(e as Error).message}`); continue; }
    if (!st.isFile()) continue;
    if (termsReal !== null && fs.realpathSync(p) === termsReal) continue; // 비추적 용어 원본 자신은 유출이 아니다
    if (st.size > MAX_BYTES) { r.skippedLarge.push(p); continue; }
    let buf: Buffer;
    try { buf = fs.readFileSync(p); } catch (e) { r.unreadable.push(`${p}: ${(e as Error).message}`); continue; }
    if (!isUtf8(buf)) continue; // 바이너리 — 확장자가 아니라 내용으로 판정한다
    r.scanned++;
    for (const h of scanLines(buf.toString("utf8"), terms)) (h.exempt ? r.exempted : r.hits).push({ ...h, path: p });
  }
  return r;
}

function walk(root: string): string[] {
  const out: string[] = [];
  const rec = (dir: string): void => {
    for (const e of fs.readdirSync(dir, { withFileTypes: true })) {
      if (SKIP_DIRS.has(e.name)) continue;
      const p = path.join(dir, e.name);
      if (e.isDirectory()) rec(p);
      else if (e.isFile()) out.push(p);
    }
  };
  rec(root);
  return out.sort();
}

if (process.argv[1] !== undefined && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const { values, positionals } = parseArgs({
    allowPositionals: true,
    options: { term: { type: "string", multiple: true }, "terms-file": { type: "string" }, "files-from": { type: "string" }, json: { type: "boolean", default: false },
      "stdin-text": { type: "boolean", default: false }, meta: { type: "boolean", default: false } },
  });
  if (values.meta) {
    process.stdout.write(JSON.stringify({ exempt_mark: EXEMPT_MARK, max_bytes: MAX_BYTES, default_terms_file: DEFAULT_TERMS_FILE }) + "\n");
    process.exit(0);
  }
  if (values["stdin-text"]) {
    process.stdout.write(JSON.stringify(scanLines(fs.readFileSync(0, "utf8"), values.term ?? [])) + "\n");
    process.exit(0);
  }
  const termsFile = values["terms-file"] ?? DEFAULT_TERMS_FILE;
  const terms = [...loadTerms(termsFile), ...(values.term ?? [])];
  let files: string[];
  if (values["files-from"] !== undefined) {
    const src = values["files-from"] === "-" ? fs.readFileSync(0, "utf8") : fs.readFileSync(values["files-from"], "utf8");
    files = src.split(/\r?\n/).filter((l) => l !== "");
  } else if (positionals[0] !== undefined) {
    files = walk(positionals[0]);
  } else {
    process.stderr.write("사용: scan-forbidden-strings.ts <root> | --files-from <목록|-> [--term T] [--terms-file F] [--json]\n");
    process.exit(2);
  }
  const r = scanFiles(files, terms, termsFile);
  if (values.json) {
    process.stdout.write(JSON.stringify(r) + "\n");
  } else {
    // 스캔하지 못한 것을 먼저 고지한다 — 조용히 건너뛴 파일은 "깨끗한 것"과 구분되지 않는다
    for (const p of r.skippedLarge) process.stdout.write(`SKIP(too-large>${MAX_BYTES}B) ${p}\n`);
    for (const p of r.unreadable) process.stdout.write(`SKIP(unreadable) ${p}\n`);
    for (const h of r.exempted) process.stdout.write(`EXEMPT(${EXEMPT_MARK}) ${h.path}:${h.line}: ${h.what}\n`);
    for (const h of r.hits) process.stdout.write(`HIT ${h.path}:${h.line}: ${h.what}\n`);
    if (r.hits.length === 0) {
      process.stdout.write(`PASS forbidden string scan: 0 hits (면제 ${r.exempted.length} · 미스캔 large=${r.skippedLarge.length} unreadable=${r.unreadable.length})\n`);
    }
  }
  process.exitCode = r.hits.length > 0 ? 1 : 0;
}
