// 결정론 markdown 헤딩 트리(PageIndex md 트리 방식, LLM 요약 없음). 순수 함수.
import type { HeadingNode } from "./types.ts";

/** 헤딩 이전 서문 노드의 level(헤딩 level 1..6과 구분) */
export const PREAMBLE_LEVEL = 0;

// ATX 헤딩: 0~3칸 들여쓰기, '#'×1~6, 그 뒤 공백 또는 줄 끝(CommonMark)
const ATX_HEADING = /^ {0,3}(#{1,6})(?:[ \t]+(.*?))?[ \t]*$/;
// 닫는 '#' 열(앞에 공백 필요) 제거
const ATX_CLOSING = /(?:^|[ \t]+)#+$/;
// 펜스 여는 줄: 0~3칸 들여쓰기 + ``` 또는 ~~~ 3개 이상
const FENCE_OPEN = /^ {0,3}(`{3,}|~{3,})(.*)$/;

export type LineKind = "heading" | "fence" | "code" | "blank" | "text";

/** 줄 분류 결과. kinds[i]는 1-based 줄 i+1. frontmatter 구간(bodyStartLine 이전)은 null */
export type LineScan = {
  kinds: (LineKind | null)[];
  /** heading 줄의 level·제목(그 외 줄은 undefined) */
  headings: Map<number, { level: number; title: string }>;
};

function checkBodyStart(lines: string[], bodyStartLine: number): void {
  if (!Number.isInteger(bodyStartLine) || bodyStartLine < 1 || bodyStartLine > lines.length + 1) {
    throw new Error(`bodyStartLine 범위 오류: ${bodyStartLine} (줄 수 ${lines.length})`);
  }
}

function stripCr(line: string): string {
  return line.endsWith("\r") ? line.slice(0, -1) : line;
}

/** 본문 각 줄을 heading/펜스/코드/빈줄/텍스트로 분류한다. 펜스 내부의 '#'은 헤딩이 아니다. */
export function scanLines(lines: string[], bodyStartLine: number): LineScan {
  checkBodyStart(lines, bodyStartLine);
  const kinds: (LineKind | null)[] = new Array(lines.length).fill(null);
  const headings = new Map<number, { level: number; title: string }>();
  let fence: { char: string; len: number } | null = null;
  for (let i = bodyStartLine - 1; i < lines.length; i++) {
    const line = stripCr(lines[i]);
    const lineNo = i + 1;
    if (fence !== null) {
      const close = FENCE_OPEN.exec(line);
      // 닫는 펜스: 같은 문자, 여는 길이 이상, 정보 문자열 없음
      if (close !== null && close[1][0] === fence.char && close[1].length >= fence.len && close[2].trim() === "") {
        kinds[i] = "fence";
        fence = null;
      } else {
        kinds[i] = "code";
      }
      continue;
    }
    const open = FENCE_OPEN.exec(line);
    // 백틱 펜스의 정보 문자열에는 백틱이 올 수 없다(CommonMark)
    if (open !== null && !(open[1][0] === "`" && open[2].includes("`"))) {
      kinds[i] = "fence";
      fence = { char: open[1][0], len: open[1].length };
      continue;
    }
    const h = ATX_HEADING.exec(line);
    if (h !== null) {
      kinds[i] = "heading";
      headings.set(lineNo, { level: h[1].length, title: (h[2] ?? "").replace(ATX_CLOSING, "").trim() });
      continue;
    }
    kinds[i] = line.trim() === "" ? "blank" : "text";
  }
  return { kinds, headings };
}

/**
 * ATX 헤딩 트리. 각 노드 end = 다음 동급/상위 헤딩 직전 줄(없으면 파일 끝).
 * 첫 헤딩 이전에 비어 있지 않은 서문이 있으면 level 0 노드(title "")를 맨 앞에 둔다.
 */
export function headingTree(lines: string[], bodyStartLine: number): HeadingNode[] {
  const { kinds, headings } = scanLines(lines, bodyStartLine);
  const roots: HeadingNode[] = [];
  // headings는 줄 순서대로 채워지므로 첫 키가 첫 헤딩 줄이다
  const firstHeading = headings.keys().next().value ?? lines.length + 1;
  const preambleHasContent = kinds.slice(bodyStartLine - 1, firstHeading - 1).some((k) => k !== "blank");
  if (preambleHasContent) {
    roots.push({ level: PREAMBLE_LEVEL, title: "", start: bodyStartLine, end: firstHeading - 1, children: [] });
  }
  const stack: HeadingNode[] = [];
  for (const [lineNo, { level, title }] of headings) {
    while (stack.length > 0 && stack[stack.length - 1].level >= level) {
      stack.pop()!.end = lineNo - 1;
    }
    const node: HeadingNode = { level, title, start: lineNo, end: lines.length, children: [] };
    (stack.length > 0 ? stack[stack.length - 1].children : roots).push(node);
    stack.push(node);
  }
  return roots;
}

/** 줄 번호를 포함하는 헤딩 제목 경로(루트→가장 깊은 헤딩). 서문(level 0)은 경로에 넣지 않는다. */
export function headingPathAt(tree: HeadingNode[], line: number): string[] {
  const path: string[] = [];
  let level: HeadingNode[] = tree;
  for (;;) {
    const hit = level.find((n) => n.start <= line && line <= n.end);
    if (hit === undefined) return path;
    if (hit.level !== PREAMBLE_LEVEL) path.push(hit.title);
    level = hit.children;
  }
}
