// 프로젝트 계약(`<wiki_dir>/contract.json`) 탐색·검증, 권위 분류, 원문 목록.
// 기본값을 채우지 않는다 — 누락·위반은 호출자가 고칠 외부 입력 오류로 throw.
import fs from "node:fs";
import path from "node:path";
import { isSafeRelative, isBelow, canonicalRoot, resolveInside, readRegular } from "./fs-safety.ts";
import { buildAliasMap, FILENAME_GROUPS } from "./mapping.ts";
import type { AuthorityClass, Contract } from "./types.ts";

/** types.ts `Contract.wiki_dir` 리터럴과 동일. 서로 교정하지 않는다. */
export const WIKI_DIRS = ["__llm-wiki", "__llm_wiki"] as const;
export const CONTRACT_FILE = "contract.json";
/** 위키 루트에 놓이는 고정 파일명. layout 디렉터리 이름(첫 segment)으로 쓸 수 없다. */
export const RESERVED_WIKI_FILES: ReadonlySet<string> = new Set([CONTRACT_FILE, "index.md", "log.md", ".gitignore", "receipt.json"]);
const LAYOUT_KEYS = ["cards_dir", "cache_dir", "archive_dir"] as const;
const SCHEMA_VERSION = 1;
const COPY_POLICY = "path_reference";
/** 원 스키마 source_suffixes 패턴 */
const SUFFIX_PATTERN = /^\.[A-Za-z0-9]+$/;
/** BM25 길이 정규화 계수 상한(정의상 0~1) */
const BM25_B_MAX = 1;
/** 원 wiki_runtime `_BLOCKED_PARTS`: 제어·생성·비밀 디렉터리명(대소문자 무시). 본문을 열지 않는다. */
const BLOCKED_PARTS = new Set([".git", ".wiki-desk", ".agents", ".claude", ".hermes", ".codex", ".ssh",
  ".aws", "__pycache__", ".pytest_cache", ".cache", ".venv", "venv", "node_modules", "secret", "secrets",
  "generated", "build", "dist"]);
/** 원 wiki_runtime 비밀 파일명·확장자 */
const SECRET_NAMES = new Set(["credentials", "credentials.json", "secrets.json", "secrets.yaml", "secrets.yml"]);
const SECRET_SUFFIXES = new Set([".pem", ".key", ".p12", ".pfx", ".keystore"]);
const ENV_FILE = ".env";

const REQUIRED = ["schema_version", "project_name", "wiki_dir", "source_roots", "excluded_roots",
  "source_suffixes", "authority_rules", "fallback", "copy_policy", "frontmatter_aliases",
  "filename_pattern", "layout", "search"] as const;
const FIELD_BOOST_KEYS = ["title", "heading", "meta", "body"] as const;
const POSITIVE_REALS = ["bm25_k1", "bm25_b", "authority_weight"] as const;
const POSITIVE_INTEGERS = ["span_max_lines", "result_limit", "spans_per_result"] as const;

type Obj = Record<string, unknown>;

function isObj(v: unknown): v is Obj {
  return typeof v === "object" && v !== null && !Array.isArray(v);
}

function fail(label: string, why: string): never {
  throw new Error(`계약 ${label}: ${why}`);
}

function nonEmptyString(v: unknown, label: string): string {
  if (typeof v !== "string" || v.trim() === "") fail(label, "비어 있지 않은 문자열 필요");
  return v;
}

/** 프로젝트 상대 canonical 경로(스키마 relativePath): safeRelative + ':' 금지 + 양끝 공백 금지 */
function relativePath(v: unknown, label: string): string {
  if (!isSafeRelative(v) || v !== v.trim() || v.includes(":")) fail(label, `안전하지 않은 상대경로 ${JSON.stringify(v)}`);
  return v;
}

function uniqueList(v: unknown, label: string, minItems: number): unknown[] {
  if (!Array.isArray(v) || v.length < minItems) fail(label, minItems > 0 ? "비어 있지 않은 목록 필요" : "목록 필요");
  if (new Set(v).size !== v.length) fail(label, "중복 항목");
  return v;
}

function hasBlockedPart(rel: string): boolean {
  return rel.split("/").some((p) => BLOCKED_PARTS.has(p.toLowerCase()));
}

function authorityClass(v: unknown, label: string, withPattern: boolean): void {
  if (!isObj(v)) fail(label, "객체 필요");
  // glob도 프로젝트 상대경로 규칙을 그대로 따른다('*'는 segment 안의 일반 문자)
  if (withPattern) relativePath(v["pattern"], `${label}.pattern`);
  nonEmptyString(v["document_type"], `${label}.document_type`);
  nonEmptyString(v["role"], `${label}.role`);
  if (!Number.isInteger(v["authority_rank"])) fail(`${label}.authority_rank`, "정수 필요");
}

function positiveFinite(v: unknown, label: string): number {
  if (typeof v !== "number" || !Number.isFinite(v) || v <= 0) fail(label, "유한한 양수 필요");
  return v;
}

export function validateContract(raw: unknown): Contract {
  if (!isObj(raw)) throw new Error("계약은 JSON 객체여야 한다");
  for (const key of REQUIRED) if (!(key in raw)) fail(key, "필수 필드 누락");

  if (raw["schema_version"] !== SCHEMA_VERSION) fail("schema_version", `정수 ${SCHEMA_VERSION}만 지원`);
  nonEmptyString(raw["project_name"], "project_name");
  const wiki = raw["wiki_dir"];
  if (!WIKI_DIRS.includes(wiki as never)) fail("wiki_dir", `${WIKI_DIRS.join(" | ")} 중 하나`);
  if (raw["copy_policy"] !== COPY_POLICY) fail("copy_policy", `${COPY_POLICY}만 지원`);

  for (const root of uniqueList(raw["source_roots"], "source_roots", 1)) {
    if (root !== ".") relativePath(root, "source_roots");
    const rel = root as string;
    if (rel !== "." && (isBelow(rel, wiki as string) || hasBlockedPart(rel))) {
      fail("source_roots", `위키·제어·생성·비밀 경로는 원문 root가 될 수 없다: ${rel}`);
    }
  }
  for (const root of uniqueList(raw["excluded_roots"], "excluded_roots", 0)) relativePath(root, "excluded_roots");

  const suffixes = uniqueList(raw["source_suffixes"], "source_suffixes", 1);
  if (suffixes.some((s) => typeof s !== "string" || !SUFFIX_PATTERN.test(s))) fail("source_suffixes", '".md" 형식 필요');
  if (new Set(suffixes.map((s) => (s as string).toLowerCase())).size !== suffixes.length) fail("source_suffixes", "대소문자만 다른 중복");

  const rules = raw["authority_rules"];
  if (!Array.isArray(rules)) fail("authority_rules", "순서 있는 목록 필요");
  rules.forEach((rule, i) => authorityClass(rule, `authority_rules[${i}]`, true));
  authorityClass(raw["fallback"], "fallback", false);

  const aliases = raw["frontmatter_aliases"];
  if (!isObj(aliases)) fail("frontmatter_aliases", "객체 필요");
  for (const [key, list] of Object.entries(aliases)) {
    if (!Array.isArray(list) || list.some((x) => typeof x !== "string" || x.trim() === "")) {
      fail(`frontmatter_aliases.${key}`, "비어 있지 않은 문자열 목록 필요");
    }
  }
  try {
    buildAliasMap(aliases as Record<string, string[]>); // 중복 별칭을 로드 시점에 거부
  } catch (err) {
    // 외부 입력 오류에 필드명을 붙여 다시 던진다(삼키지 않음).
    throw new Error(`계약 frontmatter_aliases: ${(err as Error).message}`, { cause: err });
  }

  const pattern = nonEmptyString(raw["filename_pattern"], "filename_pattern");
  try {
    new RegExp(pattern);
  } catch (err) {
    // 외부 입력 오류에 필드명을 붙여 다시 던진다(삼키지 않음).
    throw new Error(`계약 filename_pattern: 정규식 오류 — ${(err as Error).message}`, { cause: err });
  }
  const groups = new Set([...pattern.matchAll(/\(\?<([A-Za-z_][A-Za-z0-9_]*)>/g)].map((m) => m[1]));
  const missingGroups = FILENAME_GROUPS.filter((g) => !groups.has(g));
  if (missingGroups.length > 0) fail("filename_pattern", `named group 필요: ${missingGroups.join(", ")}`);

  const layout = raw["layout"];
  if (!isObj(layout)) fail("layout", "객체 필요");
  const dirs = LAYOUT_KEYS.map((key) => {
    const dir = relativePath(layout[key], `layout.${key}`);
    const head = dir.split("/")[0]!;
    // 대소문자를 구분하지 않는 파일시스템(macOS·NTFS)에서도 같은 이름이 되지 않도록 소문자로 비교
    if ([...RESERVED_WIKI_FILES].some((r) => r.toLowerCase() === head.toLowerCase())) fail(`layout.${key}`, `예약 파일명 ${head}와 같은 이름`);
    return { key, dir: dir.toLowerCase(), shown: dir };
  });
  // 카드·캐시·보관 영역은 서로 독립이어야 한다(한쪽 정리가 다른 쪽을 지우지 않도록)
  for (const [i, a] of dirs.entries()) {
    for (const b of dirs.slice(i + 1)) {
      if (isBelow(a.dir, b.dir) || isBelow(b.dir, a.dir)) {
        fail("layout", `${a.key}(${a.shown})와 ${b.key}(${b.shown})가 겹친다(대소문자 무시)`);
      }
    }
  }

  const search = raw["search"];
  if (!isObj(search)) fail("search", "객체 필요");
  for (const key of POSITIVE_REALS) positiveFinite(search[key], `search.${key}`);
  if ((search["bm25_b"] as number) > BM25_B_MAX) fail("search.bm25_b", `${BM25_B_MAX} 이하 필요`);
  for (const key of POSITIVE_INTEGERS) {
    if (!Number.isInteger(positiveFinite(search[key], `search.${key}`))) fail(`search.${key}`, "양의 정수 필요");
  }
  const boost = search["field_boost"];
  if (!isObj(boost)) fail("search.field_boost", "객체 필요");
  for (const key of FIELD_BOOST_KEYS) positiveFinite(boost[key], `search.field_boost.${key}`);

  // 알 수 없는 확장 키 포함 전체 보존, 호출자 원본과 분리.
  return structuredClone(raw) as Contract;
}

/** `<root>/__llm-wiki|__llm_wiki/contract.json` 중 정확히 하나. wikiDir은 루트 상대 폴더명. */
export function findContract(root: string): { wikiDir: string; contract: Contract } {
  const present = WIKI_DIRS.filter((dir) => {
    const target = resolveInside(root, `${dir}/${CONTRACT_FILE}`);
    return fs.lstatSync(target, { throwIfNoEntry: false }) !== undefined;
  });
  if (present.length !== 1) {
    throw new Error(`계약 파일은 정확히 하나여야 한다(${WIKI_DIRS.join(", ")}): 발견 ${present.length}개`);
  }
  const wikiDir = present[0]!;
  const contract = validateContract(JSON.parse(readRegular(root, `${wikiDir}/${CONTRACT_FILE}`).toString("utf8")));
  if (contract.wiki_dir !== wikiDir) fail("wiki_dir", `폴더명 ${wikiDir}와 다름: ${contract.wiki_dir}`);
  return { wikiDir, contract };
}

/** 자체 glob: `**`는 '/' 포함 임의 문자열, `**​/`는 0개 이상 디렉터리, `*`는 '/' 제외. 그 외는 리터럴. */
function globToRegExp(glob: string): RegExp {
  let out = "";
  for (let i = 0; i < glob.length; i++) {
    const ch = glob[i]!;
    if (ch === "*" && glob[i + 1] === "*") {
      if (glob[i + 2] === "/") {
        out += "(?:.*/)?";
        i += 2;
      } else {
        out += ".*";
        i += 1;
      }
    } else if (ch === "*") out += "[^/]*";
    else out += ch.replace(/[.+?^${}()|[\]\\]/g, "\\$&");
  }
  return new RegExp(`^${out}$`, "u");
}

export function authorityFor(contract: Contract, rel: string): AuthorityClass {
  const rule = contract.authority_rules.find((r) => globToRegExp(r.pattern).test(rel)) ?? contract.fallback;
  return { document_type: rule.document_type, authority_rank: rule.authority_rank, role: rule.role };
}

function excluded(rel: string, contract: Contract): boolean {
  const name = path.posix.basename(rel).toLowerCase();
  return isBelow(rel, contract.wiki_dir) || contract.excluded_roots.some((e) => isBelow(rel, e)) ||
    hasBlockedPart(rel) || name === ENV_FILE || name.startsWith(ENV_FILE + ".") ||
    SECRET_NAMES.has(name) || SECRET_SUFFIXES.has(path.posix.extname(name));
}

function suffixOk(rel: string, contract: Contract): boolean {
  const ext = path.posix.extname(rel).toLowerCase();
  return contract.source_suffixes.some((s) => s.toLowerCase() === ext);
}

/** source_roots 하위 원문 상대경로(마지막에 한 번 정렬). 제외 경로는 내려가지도·열지도 않는다. 보이는 심링크는 거부. */
export function listSources(root: string, contract: Contract): string[] {
  const collected = new Set<string>();
  const walk = (dirRel: string): void => {
    const dir = dirRel === "." ? canonicalRoot(root) : resolveInside(root, dirRel);
    for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
      const rel = dirRel === "." ? entry.name : `${dirRel}/${entry.name}`;
      if (entry.isSymbolicLink()) throw new Error(`원문 심볼릭링크 거부: ${rel}`);
      if (excluded(rel, contract)) continue;
      if (entry.isDirectory()) walk(rel);
      else if (!entry.isFile()) throw new Error(`특수 파일 원문 거부: ${rel}`);
      else if (suffixOk(rel, contract)) collected.add(rel);
    }
  };
  for (const sourceRoot of contract.source_roots) {
    if (sourceRoot === ".") {
      walk(".");
      continue;
    }
    const st = fs.statSync(resolveInside(root, sourceRoot)); // 미존재 root는 자연 raise
    if (excluded(sourceRoot, contract)) continue;
    if (st.isDirectory()) walk(sourceRoot);
    else if (suffixOk(sourceRoot, contract)) collected.add(sourceRoot);
  }
  return [...collected].sort();
}
