// 원문 frontmatter → OKF 포인터 카드 매핑. 값은 원문 복사만 하며 생성·요약하지 않는다.
import { isDeepStrictEqual } from "node:util";
import type { AuthorityClass, Card, Contract, Frontmatter, SourceDoc } from "./types.ts";
import { ISO_DATETIME_OFFSET_RE } from "./okf.ts";

// 카드 최상위 슬롯으로 올라가는 정본 키(별칭은 계약 frontmatter_aliases가 정한다)
const TITLE_KEY = "title";
const DESCRIPTION_KEY = "description";
const DOC_TYPE_KEY = "document_type";
const SLOT_KEYS = [TITLE_KEY, DESCRIPTION_KEY, DOC_TYPE_KEY];
// 원문 tags 정본 키: 문자열 원소를 카드 tags에 합친다
const TAGS_KEY = "tags";
// 파일명 규칙 정규식이 가져야 할 named group(contract.ts도 로드 시 검사)
export const FILENAME_GROUPS = ["prefix", "yymmddhh", "title"] as const;
// 카드 생산자 actor(OKF §7: 자동 처리는 process: 접두, human: 금지)
export const PRODUCER_ACTOR = "process:wiki-desk-ts";

export type MappedFrontmatter = {
  source_meta: Record<string, unknown>;
  source_meta_unknown: Record<string, unknown>;
  title: string | null;
  description: string | null;
  docType: string | null;
  /** 같은 정본 키에 서로 다른 값이 온 원문 키(또는 비문자열 슬롯 값) — 값은 source_meta_unknown에 원형 보존 */
  conflicts: string[];
  /** 빈/공백 문자열이라 null로 본 슬롯의 원문 키 */
  blanks: string[];
};

/** `__proto__` 같은 키도 일반 데이터로 저장 */
function put(obj: Record<string, unknown>, key: string, value: unknown): void {
  Object.defineProperty(obj, key, { value, enumerable: true, writable: true, configurable: true });
}

/** 별칭 → 정본 역맵(슬롯 키·정본 키는 자기 자신). 한 이름이 두 정본에 걸리면 계약 오류. */
export function buildAliasMap(aliases: Record<string, string[]>): Map<string, string> {
  const rev = new Map<string, string>();
  const claim = (name: string, canonical: string): void => {
    const prev = rev.get(name);
    if (prev !== undefined && prev !== canonical) throw new Error(`frontmatter_aliases 중복 별칭: ${name} (${prev}, ${canonical})`);
    rev.set(name, canonical);
  };
  for (const k of SLOT_KEYS) claim(k, k);
  for (const [canonical, names] of Object.entries(aliases)) {
    claim(canonical, canonical);
    for (const n of names) claim(n, canonical);
  }
  return rev;
}

export function mapFrontmatter(data: Record<string, unknown>, aliases: Record<string, string[]>): MappedFrontmatter {
  const rev = buildAliasMap(aliases);
  const slots: Record<string, string | null> = { [TITLE_KEY]: null, [DESCRIPTION_KEY]: null, [DOC_TYPE_KEY]: null };
  const source_meta: Record<string, unknown> = {};
  const source_meta_unknown: Record<string, unknown> = {};
  const conflicts: string[] = [];
  const blanks: string[] = [];
  for (const [key, value] of Object.entries(data)) {
    const canonical = rev.get(key);
    if (canonical === undefined) {
      put(source_meta_unknown, key, value);
      continue;
    }
    if (SLOT_KEYS.includes(canonical)) {
      if (value === null) continue; // null = 값 없음
      if (typeof value === "string" && value.trim() === "") {
        blanks.push(key);
        continue;
      }
      const current = slots[canonical];
      if (typeof value === "string" && (current === null || current === value)) {
        slots[canonical] = value;
        continue;
      }
      put(source_meta_unknown, key, value);
      conflicts.push(key);
      continue;
    }
    if (!Object.prototype.hasOwnProperty.call(source_meta, canonical)) put(source_meta, canonical, value);
    else if (!isDeepStrictEqual(source_meta[canonical], value)) {
      put(source_meta_unknown, key, value);
      conflicts.push(key);
    }
  }
  return {
    source_meta,
    source_meta_unknown,
    title: slots[TITLE_KEY],
    description: slots[DESCRIPTION_KEY],
    docType: slots[DOC_TYPE_KEY],
    conflicts,
    blanks,
  };
}

export type PathInference =
  | { matches: false }
  | { matches: true; prefix: string; yymmddhh: string; koTitle: string };

/** 파일명(경로의 마지막 조각)을 계약 정규식으로 해석. 필수 group이 빠진 매치는 불일치. 한글제목의 `_`는 공백으로. */
export function inferFromPath(rel: string, filenamePattern: string): PathInference {
  const name = rel.slice(rel.lastIndexOf("/") + 1);
  const m = new RegExp(filenamePattern).exec(name);
  if (m === null) return { matches: false };
  const groups = m.groups ?? {};
  const missing = FILENAME_GROUPS.filter((g) => !(g in groups));
  if (missing.length > 0) throw new Error(`filename_pattern에 named group 필요: ${missing.join(", ")}`);
  const { prefix, yymmddhh, title } = groups;
  if (prefix === undefined || yymmddhh === undefined || title === undefined) return { matches: false };
  return { matches: true, prefix, yymmddhh, koTitle: title.replace(/_/g, " ") };
}

const nonEmpty = (v: string | null): v is string => v !== null && v.trim() !== "";

/** 원문 tags 값 → 문자열 tag 목록 + 문제 사유(없으면 null). null·부재는 값 없음. */
function readTags(v: unknown): { tags: string[]; problem: string | null } {
  if (v === undefined || v === null) return { tags: [], problem: null };
  if (!Array.isArray(v)) return { tags: [], problem: `tags가 리스트가 아님: ${JSON.stringify(v)}` };
  const tags = v.filter((t): t is string => typeof t === "string");
  const bad = v.filter((t) => typeof t !== "string");
  return { tags, problem: bad.length === 0 ? null : `tags 원소가 문자열이 아님: ${bad.map((t) => JSON.stringify(t)).join(", ")}` };
}

export function buildCard(
  src: SourceDoc,
  fm: Frontmatter,
  contract: Contract,
  authority: AuthorityClass,
  nowIso: string,
  decodeError: string | null,
): Card {
  if (!ISO_DATETIME_OFFSET_RE.test(nowIso)) throw new Error(`nowIso는 UTC offset 포함 ISO 8601이어야 함: ${nowIso}`);
  const inferred = inferFromPath(src.path, contract.filename_pattern);
  const reasons: string[] = [];
  if (decodeError !== null) reasons.push(`원문 UTF-8 디코딩 불가: ${decodeError}`);
  if (!inferred.matches) reasons.push(`파일명 규칙 위반: ${src.path}`);
  if (fm.kind === "error") reasons.push(`frontmatter 파싱 불가: ${fm.error}`);

  // 파싱 불가·frontmatter 없음이면 빈 data → source_meta/source_meta_unknown 빈 객체, 제목은 파일명 추론
  const data = fm.kind === "ok" ? fm.data : {};
  const mapped = mapFrontmatter(data, contract.frontmatter_aliases);
  for (const key of mapped.conflicts) reasons.push(`frontmatter 키 충돌(원형은 source_meta_unknown): ${key}`);
  for (const key of mapped.blanks) reasons.push(`frontmatter 값이 비어 있음: ${key}`);
  // tags는 별칭 등록 여부와 무관: 등록됐으면 source_meta(별칭 흡수 결과), 아니면 원문 키
  const sourceTags = readTags(Object.prototype.hasOwnProperty.call(mapped.source_meta, TAGS_KEY) ? mapped.source_meta[TAGS_KEY] : data[TAGS_KEY]);
  if (sourceTags.problem !== null) reasons.push(sourceTags.problem);

  // OKF type: 원문 document_type → 파일명 접두사 → 계약 authority.document_type
  const type = [mapped.docType, inferred.matches ? inferred.prefix : null, authority.document_type].find(nonEmpty);
  if (type === undefined) throw new Error(`type 도출 불가(계약 authority.document_type 비어 있음): ${src.path}`);

  return {
    type,
    title: mapped.title ?? (inferred.matches && inferred.koTitle.trim() !== "" ? inferred.koTitle : null),
    description: mapped.description,
    resource: src.path,
    tags: [...new Set([type, ...sourceTags.tags])],
    generated: { by: PRODUCER_ACTOR, at: nowIso },
    authority: { document_type: authority.document_type, authority_rank: authority.authority_rank, role: authority.role },
    source_meta: mapped.source_meta,
    source_meta_unknown: mapped.source_meta_unknown,
    needs_review: reasons.length > 0,
    review_reasons: reasons,
    source_sha256: src.sha256,
    extra: {},
  };
}
