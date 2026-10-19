// 한영 혼용 기술 문서 토크나이저. 순수 함수.
//
// 규칙(NFKC → 소문자화 후):
// 1) 한글 음절 연속 구간 → 문자 bigram(겹침). 1글자 구간은 unigram.
//    예) "임베딩을" → 임베·베딩·딩을. 조사가 붙어도 어간 bigram이 남아 매칭된다.
// 2) 그 외 문자/숫자 단어는 `_ - . /`로 이어진 식별자를 하나로 본다.
//    - 구분자로 나뉜 각 부분(part) 토큰을 낸다.
//    - 부분이 2개 이상이면 원형(joined) 토큰도 낸다.
//    예) "agent_config.yaml" → agent·config·yaml·agent_config.yaml
//        "gemini-embedding-2" → gemini·embedding·gemini-embedding-2 ("2"는 길이 미달로 버림)
//    원형 토큰은 희소해 idf가 커지므로 식별자 정확 일치를 우대하고,
//    부분 토큰은 "structured output"처럼 띄어 쓴 질의와도 맞물리게 한다.
// 3) 비한글 토큰은 길이 MIN_WORD_LEN 미만이면 버린다.
// 모든 토큰은 normalize(text)의 부분 문자열이다(query.ts의 줄 사전 필터가 이 성질에 의존).

/** 비한글 토큰 최소 길이. 1글자 영숫자(a, x, 2)는 변별력이 없어 색인 잡음만 늘린다. */
export const MIN_WORD_LEN = 2;

/** 한글 bigram 폭 */
const HANGUL_NGRAM = 2;

// 한글 음절 구간 | 한글 제외 문자·숫자 단어(+ 구분자로 이어진 식별자)
const TOKEN_RUN = /([\uAC00-\uD7A3]+)|([[\p{L}\p{N}]--[\uAC00-\uD7A3]]+(?:[_\-.\/][[\p{L}\p{N}]--[\uAC00-\uD7A3]]+)*)/gv;
const PART_SEP = /[_\-./]/;

/** 토큰화 전 정규화: NFKC + 소문자화(JS에 casefold가 없어 toLowerCase로 근사) */
export function normalize(text: string): string {
  return text.normalize("NFKC").toLowerCase();
}

/** 토큰 스트림(중복 포함, 등장 순서). 빈도 계산용. */
export function tokenize(text: string): string[] {
  const out: string[] = [];
  for (const m of normalize(text).matchAll(TOKEN_RUN)) {
    const hangul = m[1];
    if (hangul !== undefined) {
      if (hangul.length < HANGUL_NGRAM) {
        out.push(hangul);
        continue;
      }
      for (let i = 0; i + HANGUL_NGRAM <= hangul.length; i++) out.push(hangul.slice(i, i + HANGUL_NGRAM));
      continue;
    }
    const word = m[2];
    const parts = word.split(PART_SEP);
    for (const p of parts) if (p.length >= MIN_WORD_LEN) out.push(p);
    if (parts.length > 1) out.push(word);
  }
  return out;
}

/** 질의 term group들을 토큰화해 중복 제거한 합집합(첫 등장 순서). 그룹 간 OR. */
export function queryTokens(termGroups: string[]): string[] {
  const seen = new Set<string>();
  for (const g of termGroups) for (const t of tokenize(g)) seen.add(t);
  return [...seen];
}
