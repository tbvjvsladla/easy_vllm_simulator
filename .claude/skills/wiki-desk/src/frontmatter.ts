// frontmatter 분리 + YAML 서브셋 파서/에미터. 런타임 의존성 없음.
// 파서는 결과형(YamlError 값)으로 실패를 전파한다 — 내부에 try/catch 없음.
import type { Frontmatter } from "./types.ts";

export type YamlResult = { data: Record<string, unknown> } | { error: string };

const DELIMITER = "---";
const BOM = "\ufeff";
// 에미터 들여쓰기 폭(YAML 관례 2칸)
const EMIT_INDENT = 2;
// 이스케이프 \xXX·\uXXXX·\UXXXXXXXX 의 16진 자릿수
const HEX_DIGITS_X = 2;
const HEX_DIGITS_U = 4;
const HEX_DIGITS_UU = 8;
const HEX_RADIX = 16;
// 이스케이프 접두 길이(`\` + 문자)
const ESCAPE_PREFIX_LEN = 2;

class YamlError {
  message: string;
  constructor(line: number, message: string) {
    this.message = `${line}행: ${message}`;
  }
}

type Line = { no: number; indent: number; text: string; raw: string };
type State = { lines: Line[]; pos: number };

// ---------------------------------------------------------------- frontmatter

const isDelimiter = (line: string): boolean => line.replace(/[ \t\r]+$/, "") === DELIMITER;

export const UNCLOSED_ERROR = "frontmatter 닫힘 구분선(---) 없음";

/** frontmatter 블록 위치. throw하지 않는다 — 닫힘 없음은 kind="unclosed". */
export type Located =
  | { kind: "none"; bodyStartLine: number }
  | { kind: "unclosed"; bodyStartLine: number }
  | { kind: "found"; raw: string; bodyStartLine: number };

export function locate(text: string): Located {
  const lines = text.slice(text.startsWith(BOM) ? BOM.length : 0).split("\n");
  if (!isDelimiter(lines[0])) return { kind: "none", bodyStartLine: 1 };
  const close = lines.findIndex((l, i) => i > 0 && isDelimiter(l));
  if (close === -1) return { kind: "unclosed", bodyStartLine: 1 };
  // raw는 여는 줄 다음부터 닫는 줄 직전까지(각 줄의 개행 포함)
  const raw = lines.slice(1, close).map((l) => l + "\n").join("");
  return { kind: "found", raw, bodyStartLine: close + 2 };
}

/** 첫 줄 `---` ~ 다음 `---` 줄. throw하지 않는다 — 판정 불가는 kind="error". */
export function splitFrontmatter(text: string): Frontmatter {
  const loc = locate(text);
  if (loc.kind === "none") return { kind: "none", bodyStartLine: loc.bodyStartLine };
  if (loc.kind === "unclosed") return { kind: "error", raw: null, error: UNCLOSED_ERROR, bodyStartLine: loc.bodyStartLine };
  const parsed = tryParseYaml(loc.raw);
  if ("error" in parsed) return { kind: "error", raw: loc.raw, error: parsed.error, bodyStartLine: loc.bodyStartLine };
  return { kind: "ok", raw: loc.raw, data: parsed.data, bodyStartLine: loc.bodyStartLine };
}

/** 본문(바이트 그대로): bodyStartLine 줄부터 끝까지. */
export function bodyText(text: string, fm: Frontmatter): string {
  let offset = 0;
  for (let line = 1; line < fm.bodyStartLine; line += 1) {
    const nl = text.indexOf("\n", offset);
    if (nl === -1) return "";
    offset = nl + 1;
  }
  return text.slice(offset);
}

// ---------------------------------------------------------------- 파서

export function tryParseYaml(raw: string): YamlResult {
  const result = parseDocument(raw);
  return result instanceof YamlError ? { error: result.message } : { data: result };
}

export function parseYaml(raw: string): Record<string, unknown> {
  const result = tryParseYaml(raw);
  if ("error" in result) throw new Error(`YAML 파싱 실패: ${result.error}`);
  return result.data;
}

function makeLine(no: number, raw: string): Line {
  const indent = raw.length - raw.replace(/^ +/, "").length;
  return { no, indent, text: raw.slice(indent), raw };
}

const isBlank = (l: Line): boolean => l.text.trim() === "";
const isInsignificant = (l: Line): boolean => isBlank(l) || l.text.startsWith("#");
const isSeqLine = (text: string): boolean => /^-( |\t|$)/.test(text);

function skipInsignificant(st: State): void {
  while (st.pos < st.lines.length && isInsignificant(st.lines[st.pos])) st.pos += 1;
}

function parseDocument(raw: string): Record<string, unknown> | YamlError {
  // 마지막 줄바꿈 뒤의 빈 조각은 줄이 아니다(블록 스칼라 keep 계산 왜곡 방지)
  const content = raw.endsWith("\n") ? raw.slice(0, -1) : raw;
  const st: State = {
    lines: content.split("\n").map((l, i) => makeLine(i + 1, l.replace(/\r$/, ""))),
    pos: 0,
  };
  skipInsignificant(st);
  if (st.pos >= st.lines.length) return {};
  const first = st.lines[st.pos];
  if (first.text.startsWith("%")) return new YamlError(first.no, "디렉티브(%) 미지원");
  if (first.indent !== 0) return new YamlError(first.no, "최상위 들여쓰기는 0이어야 함");
  if (splitKey(first) === null) return new YamlError(first.no, "최상위는 매핑(키: 값)이어야 함");
  const node = parseMap(st, 0);
  if (node instanceof YamlError) return node;
  skipInsignificant(st);
  if (st.pos < st.lines.length) return new YamlError(st.lines[st.pos].no, "해석할 수 없는 줄");
  return node;
}

function checkTab(l: Line): YamlError | null {
  return l.text.startsWith("\t") ? new YamlError(l.no, "들여쓰기에 탭 사용 불가") : null;
}

/** 블록 노드(매핑·시퀀스·스칼라). minIndent 미만이면 null(빈 값). */
function parseBlockNode(st: State, minIndent: number, parentIndent: number): unknown {
  skipInsignificant(st);
  if (st.pos >= st.lines.length) return null;
  const line = st.lines[st.pos];
  if (line.indent < minIndent) return null;
  const tab = checkTab(line);
  if (tab) return tab;
  if (isSeqLine(line.text)) return parseSeq(st, line.indent);
  const kv = splitKey(line);
  if (kv instanceof YamlError) return kv;
  if (kv !== null) return parseMap(st, line.indent);
  st.pos += 1;
  return parseInline(st, line.text, parentIndent, line.no);
}

function parseSeq(st: State, indent: number): unknown[] | YamlError {
  const items: unknown[] = [];
  for (;;) {
    skipInsignificant(st);
    if (st.pos >= st.lines.length) break;
    const line = st.lines[st.pos];
    if (line.indent > indent) return new YamlError(line.no, "시퀀스 들여쓰기 불일치");
    if (line.indent < indent || !isSeqLine(line.text)) break;
    const rest = line.text.slice(1);
    const gap = rest.length - rest.replace(/^[ \t]+/, "").length;
    const content = rest.slice(gap);
    let item: unknown;
    if (content === "" || content.startsWith("#")) {
      st.pos += 1;
      item = parseBlockNode(st, indent + 1, indent);
    } else {
      // `- 내용` 을 `  내용` 가상 줄로 바꿔 블록 노드로 해석(콤팩트 표기)
      const virtualIndent = indent + 1 + gap;
      st.lines[st.pos] = { no: line.no, indent: virtualIndent, text: content, raw: " ".repeat(virtualIndent) + content };
      item = parseBlockNode(st, virtualIndent, indent);
    }
    if (item instanceof YamlError) return item;
    items.push(item);
  }
  return items;
}

function setKey(obj: Record<string, unknown>, key: string, value: unknown): void {
  // `__proto__` 키도 일반 데이터로 저장
  Object.defineProperty(obj, key, { value, enumerable: true, writable: true, configurable: true });
}

function parseMap(st: State, indent: number): Record<string, unknown> | YamlError {
  const obj: Record<string, unknown> = {};
  for (;;) {
    skipInsignificant(st);
    if (st.pos >= st.lines.length) break;
    const line = st.lines[st.pos];
    if (line.indent < indent) break;
    if (line.indent > indent) return new YamlError(line.no, "예상치 못한 들여쓰기");
    const tab = checkTab(line);
    if (tab) return tab;
    if (isSeqLine(line.text)) return new YamlError(line.no, "매핑 안에 시퀀스 항목");
    if (line.text === "---" || line.text === "...") return new YamlError(line.no, "멀티문서 미지원");
    const kv = splitKey(line);
    if (kv instanceof YamlError) return kv;
    if (kv === null) return new YamlError(line.no, "'키: 값' 형식이 아님");
    if (Object.prototype.hasOwnProperty.call(obj, kv.key)) return new YamlError(line.no, `중복 키: ${kv.key}`);
    st.pos += 1;
    const value = parseMapValue(st, indent, kv.rest, line.no);
    if (value instanceof YamlError) return value;
    setKey(obj, kv.key, value);
  }
  return obj;
}

function parseMapValue(st: State, indent: number, rest: string, lineNo: number): unknown {
  if (rest === "" || rest.startsWith("#")) {
    skipInsignificant(st);
    const next = st.lines[st.pos];
    if (next !== undefined && next.indent === indent && isSeqLine(next.text)) return parseSeq(st, indent);
    return parseBlockNode(st, indent + 1, indent);
  }
  return parseInline(st, rest, indent, lineNo);
}

/** `키: 나머지` 분해. 키 줄이 아니면 null. */
function splitKey(line: Line): { key: string; rest: string } | null | YamlError {
  const text = line.text;
  const first = text[0];
  if (first === '"' || first === "'") {
    const q = readQuoted(text, 0, line.no);
    if (q instanceof YamlError) return q;
    const after = text.slice(q.end);
    const m = /^[ \t]*:([ \t]|$)/.exec(after);
    if (m === null) return null;
    return { key: q.value, rest: after.slice(m[0].length).trim() };
  }
  if (/^[\[\]{}#&*!|>%@`,?]/.test(text)) return null;
  for (let i = 0; i < text.length; i += 1) {
    const c = text[i];
    if (c === "#" && i > 0 && /[ \t]/.test(text[i - 1])) return null;
    if (c === ":" && (i + 1 === text.length || /[ \t]/.test(text[i + 1]))) {
      const key = text.slice(0, i).replace(/[ \t]+$/, "");
      if (key === "") return new YamlError(line.no, "빈 키");
      return { key, rest: text.slice(i + 1).trim() };
    }
  }
  return null;
}

/** 줄 안(따옴표 밖) 주석 제거. 주석은 공백 뒤 `#`. */
function stripComment(text: string): { value: string; hadComment: boolean } {
  const m = /(^|[ \t])#/.exec(text);
  if (m === null) return { value: text.trim(), hadComment: false };
  return { value: text.slice(0, m.index).trim(), hadComment: true };
}

function hasMappingIndicator(plain: string): boolean {
  return /:([ \t]|$)/.test(plain);
}

/** 한 줄에서 시작하는 인라인 값: 블록 스칼라·흐름·따옴표·plain(다중 줄 접힘). */
function parseInline(st: State, text: string, parentIndent: number, lineNo: number): unknown {
  const c = text[0];
  if (c === "&" || c === "*" || c === "!") return new YamlError(lineNo, "앵커·별칭·태그 미지원");
  if (c === "%" || c === "@" || c === "`") return new YamlError(lineNo, `예약 문자 '${c}'로 시작하는 값`);
  if (c === "?" && (text.length === 1 || /[ \t]/.test(text[1]))) return new YamlError(lineNo, "복합 키(?) 미지원");
  if (c === "|" || c === ">") return parseBlockScalar(st, text, parentIndent, lineNo);
  if (c === "[" || c === "{") return parseFlowText(st, text, parentIndent, lineNo);
  if (c === '"' || c === "'") {
    const q = readQuoted(text, 0, lineNo);
    if (q instanceof YamlError) return q;
    const tail = text.slice(q.end).trim();
    if (tail !== "" && !tail.startsWith("#")) return new YamlError(lineNo, "따옴표 스칼라 뒤 잔여 내용");
    return q.value;
  }
  return parsePlain(st, text, parentIndent, lineNo);
}

function parsePlain(st: State, text: string, parentIndent: number, lineNo: number): unknown {
  const head = stripComment(text);
  if (hasMappingIndicator(head.value)) return new YamlError(lineNo, "plain 값 안의 ': '는 따옴표 필요");
  let value = head.value;
  let multiline = false;
  let closed = head.hadComment;
  while (!closed) {
    // 빈 줄을 건너뛴 다음 줄이 더 깊이 들여쓰였으면 연속 줄
    let j = st.pos;
    let blanks = 0;
    while (j < st.lines.length && isBlank(st.lines[j])) {
      blanks += 1;
      j += 1;
    }
    const next = st.lines[j];
    if (next === undefined || next.indent <= parentIndent || next.text.startsWith("#")) break;
    const part = stripComment(next.text);
    if (hasMappingIndicator(part.value)) return new YamlError(next.no, "plain 연속 줄 안의 ': '");
    value += blanks > 0 ? "\n".repeat(blanks) + part.value : " " + part.value;
    multiline = true;
    closed = part.hadComment;
    st.pos = j + 1;
  }
  return multiline ? value : resolvePlain(value);
}

const NULLS = new Set(["", "~", "null", "Null", "NULL"]);
const TRUES = new Set(["true", "True", "TRUE"]);
const FALSES = new Set(["false", "False", "FALSE"]);
const INT_RE = /^[-+]?[0-9]+$/;
const FLOAT_RE = /^[-+]?(\.[0-9]+|[0-9]+(\.[0-9]*)?)([eE][-+]?[0-9]+)?$/;

/** plain 스칼라 해석(YAML 1.2 core 서브셋). 날짜·yes/no 등은 문자열 유지. */
export function resolvePlain(s: string): unknown {
  if (NULLS.has(s)) return null;
  if (TRUES.has(s)) return true;
  if (FALSES.has(s)) return false;
  if (INT_RE.test(s)) {
    const n = Number(s);
    // 정밀도를 잃는 큰 정수는 원문 문자열로 둔다
    return Number.isSafeInteger(n) ? n : s;
  }
  if (FLOAT_RE.test(s)) {
    const f = Number(s);
    // 넘치는 실수(1e999 → Infinity)는 출력할 수 없으므로 원문 문자열로 둔다
    return Number.isFinite(f) ? f : s;
  }
  return s;
}

const ESCAPES: Record<string, string> = {
  "0": "\0", a: "\x07", b: "\b", t: "\t", "\t": "\t", n: "\n", v: "\v", f: "\f", r: "\r",
  e: "\x1b", " ": " ", '"': '"', "/": "/", "\\": "\\", N: "\x85", _: "\xa0", L: "\u2028", P: "\u2029",
};
const HEX_ESCAPES: Record<string, number> = { x: HEX_DIGITS_X, u: HEX_DIGITS_U, U: HEX_DIGITS_UU };

/** text[start]의 따옴표부터 닫는 따옴표까지. 흐름 안의 줄바꿈은 공백으로 접는다. */
function readQuoted(text: string, start: number, lineNo: number): { value: string; end: number } | YamlError {
  const quote = text[start];
  let out = "";
  let i = start + 1;
  while (i < text.length) {
    const c = text[i];
    if (c === quote) {
      if (quote === "'" && text[i + 1] === "'") {
        out += "'";
        i += 2;
        continue;
      }
      return { value: out, end: i + 1 };
    }
    if (c === "\n") {
      out = out.replace(/[ \t]+$/, "") + " ";
      i += 1;
      while (i < text.length && /[ \t]/.test(text[i])) i += 1;
      continue;
    }
    if (c === "\\" && quote === '"') {
      const e = text[i + 1];
      const width = HEX_ESCAPES[e];
      if (width !== undefined) {
        const hex = text.slice(i + ESCAPE_PREFIX_LEN, i + ESCAPE_PREFIX_LEN + width);
        if (!new RegExp(`^[0-9a-fA-F]{${width}}$`).test(hex)) return new YamlError(lineNo, `잘못된 이스케이프 \\${e}${hex}`);
        out += String.fromCodePoint(parseInt(hex, HEX_RADIX));
        i += ESCAPE_PREFIX_LEN + width;
        continue;
      }
      const mapped = ESCAPES[e];
      if (mapped === undefined) return new YamlError(lineNo, `알 수 없는 이스케이프 \\${e}`);
      out += mapped;
      i += ESCAPE_PREFIX_LEN;
      continue;
    }
    out += c;
    i += 1;
  }
  return new YamlError(lineNo, "닫히지 않은 따옴표(여러 줄 따옴표 스칼라 미지원)");
}

function parseBlockScalar(st: State, header: string, parentIndent: number, lineNo: number): string | YamlError {
  const m = /^([|>])([-+]?)([1-9]?)([-+]?)([ \t]+#.*)?[ \t]*$/.exec(header);
  if (m === null || (m[2] !== "" && m[4] !== "")) return new YamlError(lineNo, `블록 스칼라 헤더 오류: ${header}`);
  const folded = m[1] === ">";
  const chomp = m[2] || m[4];
  let contentIndent = m[3] === "" ? -1 : parentIndent + Number(m[3]);
  const lines: string[] = [];
  while (st.pos < st.lines.length) {
    const l = st.lines[st.pos];
    if (isBlank(l)) {
      lines.push(contentIndent >= 0 && l.raw.length > contentIndent ? l.raw.slice(contentIndent) : "");
      st.pos += 1;
      continue;
    }
    if (contentIndent < 0) {
      if (l.indent <= parentIndent) break;
      contentIndent = l.indent;
    }
    if (l.indent < contentIndent) break;
    lines.push(l.raw.slice(contentIndent));
    st.pos += 1;
  }
  let trailing = 0;
  while (lines.length > 0 && lines[lines.length - 1].trim() === "") {
    lines.pop();
    trailing += 1;
  }
  const core = folded ? foldLines(lines) : lines.join("\n");
  if (chomp === "-") return core;
  if (chomp === "+") return lines.length === 0 ? "\n".repeat(trailing) : core + "\n" + "\n".repeat(trailing);
  return lines.length === 0 ? "" : core + "\n";
}

/** 접힘에서 앞 줄(prev)과 이번 줄 사이에 들어갈 구분자. */
function foldBreak(prev: string | null, line: string, blanks: number): string {
  if (prev === null) return "\n".repeat(blanks);
  // 더 들여쓴 줄 주변 개행은 보존
  if (/^[ \t]/.test(prev) || /^[ \t]/.test(line)) return "\n".repeat(blanks + 1);
  if (blanks === 0) return " ";
  return "\n".repeat(blanks);
}

/** `>` 접힘: 일반 줄 사이 단일 개행은 공백, 빈 줄은 개행, 더 들여쓴 줄 주변 개행은 보존. */
function foldLines(lines: string[]): string {
  let out = "";
  let blanks = 0;
  let prev: string | null = null;
  for (const l of lines) {
    if (l === "") {
      blanks += 1;
      continue;
    }
    out += foldBreak(prev, l, blanks) + l;
    prev = l;
    blanks = 0;
  }
  return out;
}

// ---------------------------------------------------------------- 흐름 컬렉션

function bracketDepth(text: string): number {
  let depth = 0;
  let quote: string | null = null;
  for (let i = 0; i < text.length; i += 1) {
    const c = text[i];
    if (quote !== null) {
      if (c === "\\" && quote === '"') i += 1;
      else if (c === quote) quote = null;
      continue;
    }
    if (c === "#" && (i === 0 || /[ \t\n]/.test(text[i - 1]))) {
      const nl = text.indexOf("\n", i);
      if (nl === -1) break;
      i = nl;
      continue;
    }
    if (c === '"' || c === "'") quote = c;
    else if (c === "[" || c === "{") depth += 1;
    else if (c === "]" || c === "}") depth -= 1;
  }
  return depth;
}

function parseFlowText(st: State, text: string, parentIndent: number, lineNo: number): unknown {
  let joined = text;
  while (bracketDepth(joined) > 0) {
    const next = st.lines[st.pos];
    if (next === undefined || (!isBlank(next) && next.indent <= parentIndent)) {
      return new YamlError(lineNo, "닫히지 않은 흐름 컬렉션");
    }
    joined += "\n" + next.text;
    st.pos += 1;
  }
  const fp = { s: joined, i: 0, lineNo };
  const value = flowValue(fp);
  if (value instanceof YamlError) return value;
  flowSkip(fp);
  if (fp.i < fp.s.length) return new YamlError(lineNo, "흐름 컬렉션 뒤 잔여 내용");
  return value;
}

type Flow = { s: string; i: number; lineNo: number };

function flowSkip(fp: Flow): void {
  while (fp.i < fp.s.length) {
    const c = fp.s[fp.i];
    if (c === " " || c === "\t" || c === "\n") fp.i += 1;
    else if (c === "#" && (fp.i === 0 || /[ \t\n]/.test(fp.s[fp.i - 1]))) {
      const nl = fp.s.indexOf("\n", fp.i);
      fp.i = nl === -1 ? fp.s.length : nl;
    } else break;
  }
}

function flowValue(fp: Flow): unknown {
  flowSkip(fp);
  const c = fp.s[fp.i];
  if (c === "[") return flowSeq(fp);
  if (c === "{") return flowMap(fp);
  if (c === '"' || c === "'") {
    const q = readQuoted(fp.s, fp.i, fp.lineNo);
    if (q instanceof YamlError) return q;
    fp.i = q.end;
    return q.value;
  }
  if (c === "&" || c === "*" || c === "!") return new YamlError(fp.lineNo, "앵커·별칭·태그 미지원");
  const plain = flowPlain(fp);
  return plain instanceof YamlError ? plain : resolvePlain(plain);
}

/** 흐름 plain: `,[]{}`, `: `/`:` + 구분자, 주석 전까지. 줄바꿈은 공백으로 접는다. */
function flowPlain(fp: Flow): string | YamlError {
  const start = fp.i;
  while (fp.i < fp.s.length) {
    const c = fp.s[fp.i];
    const n = fp.s[fp.i + 1];
    if (",[]{}".includes(c)) break;
    if (c === ":" && (n === undefined || /[ \t\n,\[\]{}]/.test(n))) break;
    if (c === "#" && /[ \t\n]/.test(fp.s[fp.i - 1] ?? "")) break;
    fp.i += 1;
  }
  const text = fp.s.slice(start, fp.i).split("\n").map((p) => p.trim()).filter((p) => p !== "").join(" ");
  if (text === "" && fp.s[fp.i] !== ":") return new YamlError(fp.lineNo, "흐름 컬렉션의 빈 항목");
  return text;
}

function flowSeq(fp: Flow): unknown[] | YamlError {
  fp.i += 1;
  const items: unknown[] = [];
  for (;;) {
    flowSkip(fp);
    if (fp.s[fp.i] === "]") {
      fp.i += 1;
      return items;
    }
    if (fp.i >= fp.s.length) return new YamlError(fp.lineNo, "닫히지 않은 '['");
    const v = flowValue(fp);
    if (v instanceof YamlError) return v;
    flowSkip(fp);
    if (fp.s[fp.i] === ":") return new YamlError(fp.lineNo, "흐름 시퀀스 안 단일쌍 매핑 미지원");
    items.push(v);
    if (fp.s[fp.i] === ",") fp.i += 1;
    else if (fp.s[fp.i] !== "]") return new YamlError(fp.lineNo, "흐름 시퀀스 구분자(,) 누락");
  }
}

function flowMap(fp: Flow): Record<string, unknown> | YamlError {
  fp.i += 1;
  const obj: Record<string, unknown> = {};
  for (;;) {
    flowSkip(fp);
    if (fp.s[fp.i] === "}") {
      fp.i += 1;
      return obj;
    }
    if (fp.i >= fp.s.length) return new YamlError(fp.lineNo, "닫히지 않은 '{'");
    const c = fp.s[fp.i];
    let key: string;
    if (c === '"' || c === "'") {
      const q = readQuoted(fp.s, fp.i, fp.lineNo);
      if (q instanceof YamlError) return q;
      fp.i = q.end;
      key = q.value;
    } else {
      if (c === "[" || c === "{") return new YamlError(fp.lineNo, "흐름 매핑의 컬렉션 키 미지원");
      const plain = flowPlain(fp);
      if (plain instanceof YamlError) return plain;
      key = plain;
    }
    if (key === "") return new YamlError(fp.lineNo, "빈 키");
    flowSkip(fp);
    let value: unknown = null;
    if (fp.s[fp.i] === ":") {
      fp.i += 1;
      flowSkip(fp);
      if (fp.s[fp.i] !== "," && fp.s[fp.i] !== "}") {
        value = flowValue(fp);
        if (value instanceof YamlError) return value;
      }
    }
    if (Object.prototype.hasOwnProperty.call(obj, key)) return new YamlError(fp.lineNo, `중복 키: ${key}`);
    setKey(obj, key, value);
    flowSkip(fp);
    if (fp.s[fp.i] === ",") fp.i += 1;
    else if (fp.s[fp.i] !== "}") return new YamlError(fp.lineNo, "흐름 매핑 구분자(,) 누락");
  }
}

// ---------------------------------------------------------------- 에미터

const isPlainObject = (v: unknown): v is Record<string, unknown> =>
  typeof v === "object" && v !== null && !Array.isArray(v) && Object.getPrototypeOf(v) === Object.prototype;

function plainSafe(s: string): boolean {
  return (
    s !== "" &&
    s === s.trim() &&
    !/[\x00-\x1f\x7f\u0085\u2028\u2029\ufeff]/.test(s) &&
    !/^[-?:,\[\]{}#&*!|>'"%@`]/.test(s) &&
    !/:([ \t]|$)/.test(s) &&
    !/[ \t]#/.test(s)
  );
}

// YAML 1.1(PyYAML 등)·1.2 core 어느 쪽에서든 비문자열로 해석될 수 있는 plain 형태.
// 숫자 계열(16·8·2진·`_`·60진·날짜·시각)은 모두 부호/점 뒤 숫자로 시작하므로 그 시작만으로 판정한다(보수적).
const NUMBER_LIKE_RE = /^[-+]?\.?[0-9]/;
const SPECIAL_FLOAT_RE = /^[-+]?\.(inf|nan)$/i;
const BOOL_NULL_RE = /^(y|n|yes|no|on|off|true|false|null|~)$/i;
// YAML 1.1 value(`=`)·merge(`<<`) 키 태그
const YAML11_SPECIAL = new Set(["=", "<<"]);

function ambiguousPlain(s: string): boolean {
  return NUMBER_LIKE_RE.test(s) || SPECIAL_FLOAT_RE.test(s) || BOOL_NULL_RE.test(s) || YAML11_SPECIAL.has(s);
}

/** plain으로 써도 어떤 YAML 소비자든 같은 문자열로 읽는가. */
const plainString = (s: string): boolean => plainSafe(s) && !ambiguousPlain(s) && resolvePlain(s) === s;

// YAML 1.1 소비자(PyYAML)가 줄바꿈으로 읽는 문자 — JSON.stringify가 이스케이프하지 않으므로 직접 \u 표기
const YAML_LINE_BREAKS = /[\u0085\u2028\u2029]/g;
const emitString = (s: string): string => (plainString(s)
  ? s
  : JSON.stringify(s).replace(YAML_LINE_BREAKS, (c) => `\\u${c.charCodeAt(0).toString(16).padStart(4, "0")}`));

const emitKey = emitString;

function emitScalar(v: unknown, path: string): string {
  if (v === null) return "null";
  if (typeof v === "boolean") return String(v);
  if (typeof v === "number") {
    if (!Number.isFinite(v)) throw new Error(`YAML 출력 불가(비유한 수): ${path}`);
    return String(v);
  }
  if (typeof v === "string") return emitString(v);
  throw new Error(`YAML 출력 불가 값(${typeof v}): ${path}`);
}

/** 빈 컬렉션은 흐름 표기, 그 외 컬렉션은 null 반환(블록으로 출력). */
function inlineForm(v: unknown, path: string): string | null {
  if (Array.isArray(v)) return v.length === 0 ? "[]" : null;
  if (isPlainObject(v)) return Object.keys(v).length === 0 ? "{}" : null;
  if (typeof v === "object" && v !== null) throw new Error(`YAML 출력 불가 객체: ${path}`);
  return emitScalar(v, path);
}

function emitMapLines(obj: Record<string, unknown>, indent: number, path: string): string[] {
  const pad = " ".repeat(indent);
  const out: string[] = [];
  for (const [k, v] of Object.entries(obj)) {
    const p = `${path}.${k}`;
    const inline = inlineForm(v, p);
    if (inline !== null) out.push(`${pad}${emitKey(k)}: ${inline}`);
    else {
      out.push(`${pad}${emitKey(k)}:`);
      out.push(...emitBlock(v, indent + EMIT_INDENT, p));
    }
  }
  return out;
}

function emitSeqLines(arr: unknown[], indent: number, path: string): string[] {
  const pad = " ".repeat(indent);
  const out: string[] = [];
  arr.forEach((v, i) => {
    const p = `${path}[${i}]`;
    const inline = inlineForm(v, p);
    if (inline !== null) out.push(`${pad}- ${inline}`);
    else if (Array.isArray(v)) {
      out.push(`${pad}-`);
      out.push(...emitSeqLines(v, indent + EMIT_INDENT, p));
    } else {
      // `- 첫키: 값` 콤팩트 매핑: 나머지 키는 대시 뒤 열에 맞춘다
      const lines = emitMapLines(v as Record<string, unknown>, indent + EMIT_INDENT, p);
      out.push(`${pad}- ${lines[0].slice(indent + EMIT_INDENT)}`);
      out.push(...lines.slice(1));
    }
  });
  return out;
}

function emitBlock(v: unknown, indent: number, path: string): string[] {
  return Array.isArray(v) ? emitSeqLines(v, indent, path) : emitMapLines(v as Record<string, unknown>, indent, path);
}

/** 결정론적 YAML(키 삽입 순서 유지). 빈 매핑은 빈 문자열. */
export function emitYaml(obj: Record<string, unknown>): string {
  if (!isPlainObject(obj)) throw new Error("emitYaml: 최상위는 일반 객체여야 함");
  return emitMapLines(obj, 0, "$").map((l) => l + "\n").join("");
}
