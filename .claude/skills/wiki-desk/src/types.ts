// wiki-desk 공통 타입. 모든 모듈이 이 파일의 계약을 공유한다(런타임 코드 없음).

/** `__llm-wiki/contract.json` — 원 wiki-desk 1.x 필드를 계승하고 TS판 확장 필드를 더한다. */
export type Contract = {
  schema_version: 1;
  project_name: string;
  /** 리터럴 그대로 사용. `__llm-wiki`와 `__llm_wiki`는 서로 다른 값이며 암묵 교정하지 않는다. */
  wiki_dir: "__llm-wiki" | "__llm_wiki";
  source_roots: string[];
  excluded_roots: string[];
  source_suffixes: string[];
  authority_rules: AuthorityRule[];
  fallback: AuthorityClass;
  copy_policy: "path_reference";
  /** 원문 frontmatter 키 별칭(계약 frontmatter_aliases): 정본 키 → 동의 키 목록 */
  frontmatter_aliases: Record<string, string[]>;
  /** 파일명 규칙(문서구분_YYMMDDHH_한글제목.md) 검사용 정규식 원문 */
  filename_pattern: string;
  /** wiki_dir 기준 상대 경로. 세 디렉터리는 서로 같거나 포함 관계일 수 없다(validateContract).
   *  archive_dir: 원문 삭제·이동을 수용한 카드의 보존 위치 */
  layout: { cards_dir: string; cache_dir: string; archive_dir: string };
  search: SearchConfig;
  /** 알 수 없는 확장 키는 보존한다 */
  [extension: string]: unknown;
};

export type AuthorityClass = { document_type: string; authority_rank: number; role: string };
export type AuthorityRule = AuthorityClass & { pattern: string };

export type SearchConfig = {
  bm25_k1: number;
  bm25_b: number;
  field_boost: { title: number; heading: number; meta: number; body: number };
  /** authority_rank 1점당 가산 점수(동점 처리·가중) */
  authority_weight: number;
  span_max_lines: number;
  result_limit: number;
  spans_per_result: number;
};

/** 원문 1건(불변) */
export type SourceDoc = {
  path: string; // 프로젝트 루트 상대, '/' 구분
  sha256: string;
  text: string;
};

/**
 * frontmatter 위치·파싱 결과. 판정 불가는 예외가 아니라 kind="error"로 표면화한다(카드 needs_review).
 * bodyStartLine: 본문 시작 줄(1-based). none·닫힘 없음 error는 1.
 */
export type Frontmatter =
  | { kind: "none"; bodyStartLine: number }
  | { kind: "ok"; raw: string; data: Record<string, unknown>; bodyStartLine: number }
  | { kind: "error"; raw: string | null; error: string; bodyStartLine: number };

/** 헤딩 트리 노드. 줄 번호는 1-based, end 포함 */
export type HeadingNode = {
  level: number; // 0..6, 0 = 첫 헤딩 이전 서문
  title: string;
  start: number;
  end: number;
  children: HeadingNode[];
};

/** OKF 0.2 포인터 카드(파싱된 형태) */
export type Card = {
  type: string;
  title: string | null;
  description: string | null;
  resource: string; // 원문 상대 경로
  tags: string[];
  generated: { by: string; at: string };
  authority: AuthorityClass;
  source_meta: Record<string, unknown>;
  source_meta_unknown: Record<string, unknown>;
  /** review_reasons가 비어 있지 않음과 항상 같다(parseCard가 불일치를 거부) */
  needs_review: boolean;
  review_reasons: string[];
  source_sha256: string;
  /** 카드에 사람이 추가한 OKF 키(verified 등) — 원형 보존, 재렌더 시 그대로 다시 쓴다(OKF §4.1) */
  extra: Record<string, unknown>;
};

/** 색인 입력 1건 */
export type IndexDoc = {
  path: string;
  sha256: string;
  title: string;
  /** 카드 메타 값(type·tags·source_meta·source_meta_unknown 스칼라)을 공백으로 이은 문자열 */
  metaText: string;
  lines: string[];
  bodyStartLine: number;
  authorityRank: number;
};

export type Span = { ref: string; heading_path: string[]; start: number; end: number };

export type QueryResult = {
  path: string;
  score: number;
  authority_rank: number;
  title: string;
  spans: Span[];
};

export type CheckIssue = {
  code: string; // okf.* (okf.ts) · card.missing · card.orphan · card.stale · card.invalid · card.unrecognized
  path: string;
  detail: string;
};
