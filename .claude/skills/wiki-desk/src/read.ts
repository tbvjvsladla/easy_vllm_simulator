// span ref 파싱·읽기. 순수 함수.

// "<경로>:<시작>-<끝>". 경로에 ':'가 있어도 마지막 ':' 뒤만 범위로 본다.
const REF_PATTERN = /^(.+):(\d+)-(\d+)$/;

/** ref → {path, start, end}. 형식 오류·start<1·end<start는 throw. 경로 안전성 검사는 fs-safety 몫. */
export function parseRef(ref: string): { path: string; start: number; end: number } {
  const m = REF_PATTERN.exec(ref);
  if (m === null) throw new Error(`ref 형식 오류(경로:시작-끝): ${ref}`);
  const start = Number(m[2]);
  const end = Number(m[3]);
  if (start < 1 || end < start) throw new Error(`ref 범위 오류: ${ref}`);
  return { path: m[1], start, end };
}

/** 1-based 포함 범위의 줄들을 '\n'으로 이어 반환. 범위가 파일 밖이면 throw. */
export function readSpan(lines: string[], start: number, end: number): string {
  if (!Number.isInteger(start) || !Number.isInteger(end) || start < 1 || end < start || end > lines.length) {
    throw new Error(`span 범위 오류: ${start}-${end} (줄 수 ${lines.length})`);
  }
  return lines.slice(start - 1, end).join("\n");
}
