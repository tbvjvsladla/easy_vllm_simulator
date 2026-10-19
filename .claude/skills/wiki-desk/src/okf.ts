// OKF 0.2 규칙 검사·무손실 왕복·포인터 카드 렌더/파싱. 원문은 읽기만 한다.
import type { Card, CheckIssue } from "./types.ts";
import { bodyText, emitYaml, splitFrontmatter } from "./frontmatter.ts";

// §3.1 예약 파일명
const INDEX_FILE = "index.md";
const LOG_FILE = "log.md";
// §8: 번들 루트 index.md에 허용되는 유일한 frontmatter 키(§12)
const ROOT_INDEX_KEYS = new Set(["okf_version"]);
// §10.2: runtime이 필수인 type
const ATTESTED_TYPE = "Attested Computation";
// §7 actor: 사람 확인은 `human:` 접두
const HUMAN_PREFIX = "human:";
// §9 로그 날짜 헤딩(ATX 2단계)
const LOG_HEADING_RE = /^##[ \t]+(.*?)[ \t]*#*[ \t]*$/;
const ISO_DATE_RE = /^(\d{4})-(\d{2})-(\d{2})$/;
const FENCE_RE = /^ {0,3}(`{3,}|~{3,})/;
// OKF §5: 명시 UTC offset을 가진 ISO 8601 datetime
export const ISO_DATETIME_OFFSET_RE = /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(\.\d+)?(Z|[+-]\d{2}:\d{2})$/;

export type TrustTier = "unverified" | "machine-confirmed" | "human-reviewed";

const isMapping = (v: unknown): v is Record<string, unknown> =>
  typeof v === "object" && v !== null && !Array.isArray(v);
const nonEmptyString = (v: unknown): v is string => typeof v === "string" && v.trim() !== "";
const issue = (code: string, path: string, detail: string): CheckIssue => ({ code, path, detail });
const baseName = (rel: string): string => rel.slice(rel.lastIndexOf("/") + 1);

/** §5.2: verified 이벤트 목록. bare mapping은 1원소 리스트, 부재는 빈 리스트. */
export function verifiedEvents(data: Record<string, unknown>): unknown[] {
  if (!("verified" in data)) return [];
  const v = data.verified;
  return Array.isArray(v) ? [...v] : [v];
}

/** §5.3: 신뢰 계층(권고 신호, 접근 제어 아님). */
export function trustTier(data: Record<string, unknown>): TrustTier {
  const events = verifiedEvents(data);
  if (events.length === 0) return "unverified";
  const human = events.some((e) => isMapping(e) && typeof e.by === "string" && e.by.startsWith(HUMAN_PREFIX));
  return human ? "human-reviewed" : "machine-confirmed";
}

function validIsoDate(s: string): boolean {
  const m = ISO_DATE_RE.exec(s);
  if (m === null) return false;
  const [y, mo, d] = [Number(m[1]), Number(m[2]), Number(m[3])];
  const date = new Date(Date.UTC(y, mo - 1, d));
  return date.getUTCFullYear() === y && date.getUTCMonth() === mo - 1 && date.getUTCDate() === d;
}

/** 펜스 코드 밖의 `## ` 헤딩 텍스트 목록. */
function level2Headings(body: string): string[] {
  const out: string[] = [];
  let fence: string | null = null;
  for (const line of body.split("\n").map((l) => l.replace(/\r$/, ""))) {
    const f = FENCE_RE.exec(line);
    if (fence !== null) {
      if (f !== null && f[1][0] === fence[0] && f[1].length >= fence.length && line.trim() === f[1]) fence = null;
      continue;
    }
    if (f !== null) {
      fence = f[1];
      continue;
    }
    const h = LOG_HEADING_RE.exec(line);
    if (h !== null) out.push(h[1]);
  }
  return out;
}

function checkIndex(rel: string, data: Record<string, unknown> | null): CheckIssue[] {
  if (data === null) return [];
  const isRoot = !rel.includes("/");
  const extra = Object.keys(data).filter((k) => !(isRoot && ROOT_INDEX_KEYS.has(k)));
  return extra.length === 0
    ? []
    : [issue("okf.index_frontmatter", rel, `index.md frontmatter 금지 키: ${extra.join(", ")}`)];
}

function checkLog(rel: string, body: string): CheckIssue[] {
  return level2Headings(body)
    .filter((h) => !validIsoDate(h))
    .map((h) => issue("okf.log_date", rel, `로그 날짜 헤딩은 YYYY-MM-DD여야 함: ${h}`));
}

function checkFamilies(rel: string, data: Record<string, unknown>): CheckIssue[] {
  const out: CheckIssue[] = [];
  if (!nonEmptyString(data.type)) out.push(issue("okf.missing_type", rel, "type(비어있지 않은 문자열) 필수"));
  if ("generated" in data) {
    const g = data.generated;
    if (!isMapping(g) || !nonEmptyString(g.by)) out.push(issue("okf.generated_by", rel, "generated.by(actor) 필수"));
  }
  verifiedEvents(data).forEach((e, i) => {
    if (!isMapping(e) || !nonEmptyString(e.by)) out.push(issue("okf.verified_by", rel, `verified[${i}].by(actor) 필수`));
  });
  if ("sources" in data) {
    const sources = data.sources;
    if (!Array.isArray(sources)) out.push(issue("okf.sources_shape", rel, "sources는 리스트여야 함"));
    else sources.forEach((s, i) => {
      if (!isMapping(s) || !nonEmptyString(s.resource)) out.push(issue("okf.source_resource", rel, `sources[${i}].resource 필수`));
    });
  }
  if (data.type === ATTESTED_TYPE && !nonEmptyString(data.runtime)) {
    out.push(issue("okf.runtime_missing", rel, "Attested Computation은 runtime 필수"));
  }
  return out;
}

/** OKF 0.2 규칙 검사. unknown 키·type, 선택 필드 부재, 깨진 링크는 문제로 보지 않는다. */
export function checkConcept(text: string, rel: string): CheckIssue[] {
  const fm = splitFrontmatter(text);
  if (fm.kind === "error") return [issue("okf.frontmatter_parse", rel, fm.error)];
  const data = fm.kind === "ok" ? fm.data : null;
  const name = baseName(rel);
  if (name === INDEX_FILE) return checkIndex(rel, data);
  if (name === LOG_FILE) return checkLog(rel, bodyText(text, fm));
  if (data === null) return [issue("okf.missing_frontmatter", rel, "concept 문서는 YAML frontmatter 필요")];
  return checkFamilies(rel, data);
}

/** 파싱→재출력. frontmatter는 의미 동등, 본문은 바이트 동일. frontmatter 없으면 원문 그대로. */
export function roundTrip(text: string): string {
  const fm = splitFrontmatter(text);
  if (fm.kind === "none") return text;
  if (fm.kind === "error") throw new Error(`YAML 파싱 실패: ${fm.error}`);
  return "---\n" + emitYaml(fm.data) + "---\n" + bodyText(text, fm);
}

// ---------------------------------------------------------------- 카드

/** 카드 frontmatter 키 순서(설계 §6). 이 밖의 최상위 키는 Card.extra. */
const CARD_KEYS = [
  "type", "title", "description", "resource", "tags", "generated", "authority",
  "source_meta", "source_meta_unknown", "needs_review", "review_reasons", "source_sha256",
] as const;
const CARD_KEY_SET: ReadonlySet<string> = new Set(CARD_KEYS);

export function renderCard(card: Card): string {
  const clash = Object.keys(card.extra).filter((k) => CARD_KEY_SET.has(k));
  if (clash.length > 0) throw new Error(`extra 키가 카드 키와 겹침: ${clash.join(", ")}`);
  // fromEntries: `__proto__` 같은 키도 일반 데이터로 둔다
  const data = Object.fromEntries([...CARD_KEYS.map((k) => [k, card[k]]), ...Object.entries(card.extra)]);
  // 제목이 없으면 원문 경로를 표시용 헤딩으로 쓴다(값 생성 아님)
  const heading = (card.title ?? card.resource).replace(/\s*\n\s*/g, " ");
  return `---\n${emitYaml(data)}---\n\n# ${heading}\n\n- 원문: \`${card.resource}\`\n`;
}

const isString = (v: unknown): boolean => typeof v === "string";
const isStringOrNull = (v: unknown): boolean => v === null || isString(v);
const isStringList = (v: unknown): boolean => Array.isArray(v) && v.every(isString);
const isBoolean = (v: unknown): boolean => typeof v === "boolean";
const isIsoOffset = (v: unknown): boolean => typeof v === "string" && ISO_DATETIME_OFFSET_RE.test(v);

type Shape = [key: string, ok: (v: unknown) => boolean][];

const CARD_SHAPE: Shape = [
  ["type", nonEmptyString], ["title", isStringOrNull], ["description", isStringOrNull], ["resource", nonEmptyString],
  ["tags", isStringList], ["generated", isMapping], ["authority", isMapping], ["source_meta", isMapping],
  ["source_meta_unknown", isMapping], ["needs_review", isBoolean], ["review_reasons", isStringList], ["source_sha256", nonEmptyString],
];
const GENERATED_SHAPE: Shape = [["by", nonEmptyString], ["at", isIsoOffset]];
const AUTHORITY_SHAPE: Shape = [["document_type", nonEmptyString], ["authority_rank", Number.isSafeInteger], ["role", nonEmptyString]];

/** 형식에 어긋난 첫 키(prefix 포함). 모두 맞으면 null. */
function shapeViolation(data: Record<string, unknown>, shape: Shape, prefix: string): string | null {
  const bad = shape.find(([key, ok]) => !ok(data[key]));
  return bad === undefined ? null : prefix + bad[0];
}

/** 형식은 맞지만 있을 수 없는 카드인가(불변식). 사유 또는 null. */
function cardInvariantViolation(c: Card): string | null {
  if (c.needs_review !== c.review_reasons.length > 0) {
    return `카드 불변식 위반: needs_review(${c.needs_review})와 review_reasons(${c.review_reasons.length}건) 불일치`;
  }
  if (!c.tags.includes(c.type)) return `카드 불변식 위반: type(${c.type})이 tags에 없음`;
  return null;
}

/** 형식 검사를 통과한 data → Card(복사). */
function toCard(data: Record<string, unknown>): Card {
  const generated = data.generated as Record<string, unknown>;
  const authority = data.authority as Record<string, unknown>;
  return {
    type: data.type as string,
    title: data.title as string | null,
    description: data.description as string | null,
    resource: data.resource as string,
    tags: data.tags as string[],
    generated: { by: generated.by as string, at: generated.at as string },
    authority: {
      document_type: authority.document_type as string,
      authority_rank: authority.authority_rank as number,
      role: authority.role as string,
    },
    source_meta: data.source_meta as Record<string, unknown>,
    source_meta_unknown: data.source_meta_unknown as Record<string, unknown>,
    needs_review: data.needs_review as boolean,
    review_reasons: data.review_reasons as string[],
    source_sha256: data.source_sha256 as string,
    extra: Object.fromEntries(Object.entries(data).filter(([k]) => !CARD_KEY_SET.has(k))),
  };
}

export type CardResult = { card: Card } | { error: string };

/** 카드 파싱(결과형). 모든 오류 메시지는 `${rel}: `로 시작한다. */
export function tryParseCard(text: string, rel: string): CardResult {
  const fail = (detail: string): CardResult => ({ error: `${rel}: ${detail}` });
  const fm = splitFrontmatter(text);
  if (fm.kind === "none") return fail("카드에 frontmatter 없음");
  if (fm.kind === "error") return fail(`카드 frontmatter 파싱 불가: ${fm.error}`);
  const data = fm.data;
  const bad =
    shapeViolation(data, CARD_SHAPE, "") ??
    shapeViolation(data.generated as Record<string, unknown>, GENERATED_SHAPE, "generated.") ??
    shapeViolation(data.authority as Record<string, unknown>, AUTHORITY_SHAPE, "authority.");
  if (bad !== null) return fail(`카드 필드 형식 오류: ${bad}`);
  const card = toCard(data);
  const broken = cardInvariantViolation(card);
  return broken === null ? { card } : fail(broken);
}

export function parseCard(text: string, rel: string): Card {
  const result = tryParseCard(text, rel);
  if ("error" in result) throw new Error(result.error);
  return result.card;
}
