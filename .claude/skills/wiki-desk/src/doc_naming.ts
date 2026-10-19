// docs/ 산문 문서 명명 SSOT(.claude/rules/docs.md §명명 SSOT). 순수 함수 — 시계·난수·파일시스템을 읽지 않는다.
// 호출자가 UTC 'YYYY-MM-DDTHH:MM:SSZ' 와 이미 점유된 이름 집합을 넘긴다. 충돌 시에만 `_MM_SS` 를 붙이고,
// 두 형태가 모두 점유되면 NamingCollisionExhausted 로 fail-closed 한다(세 번째 형태를 발명하지 않는다).
// 벤치 명명(bench_report·benchmark·max_envelope)은 adversarial-benchmark 가 소유한다 — 여기서 다시 적지 않는다.
// 사용(파이썬 호출부의 shim 이 부른다): node doc_naming.ts <함수> '<JSON 인자 배열>' → stdout JSON
//   node doc_naming.ts --patterns    → 정규식 원문(파이썬 shim 이 같은 패턴을 컴파일한다)
//   node doc_naming.ts --self-test
import path from "node:path";
import { fileURLToPath } from "node:url";

export const DATED_DOC_TYPES = ["plan", "devlog", "testlog", "request", "checklist"] as const;
export type DatedDocType = (typeof DATED_DOC_TYPES)[number];

// 패턴 원문은 JS·파이썬 공통 문법만 쓴다. 이름 그룹 `(?<x>` 는 파이썬 shim 이 `(?P<x>` 로만 바꿔 같은 문자열을 컴파일한다
// (hint-publisher lineage 가 그룹 hour·topic 을 읽는다). 전후방 탐색 `(?<=`·`(?<!` 은 쓰지 않는다.
export const PATTERNS = {
  topic: "^[A-Za-z0-9_.가-힣]+$",
  dated_doc: "^(?<type>[a-z]+)_(?<hour>\\d{8})(?:_\\d{2}_\\d{2})?_(?<topic>.+)\\.md$",
  report: "^[a-z][a-z0-9]*_\\d{8}_.+$",
  utc: "^(\\d{4})-(\\d{2})-(\\d{2})T(\\d{2}):(\\d{2}):(\\d{2})Z$",
} as const;

const TOPIC_RE = new RegExp(PATTERNS.topic, "u");
const DATED_DOC_RE = new RegExp(PATTERNS.dated_doc, "u");
const REPORT_RE = new RegExp(PATTERNS.report, "u");
const UTC_RE = new RegExp(PATTERNS.utc);
const KST_OFFSET_HOURS = 9;
const NA = ["NA", "00", "00"] as const;

export class NamingCollisionExhausted extends Error {
  constructor(message: string) { super(message); this.name = "NamingCollisionExhausted"; }
}

const pad2 = (n: number): string => String(n).padStart(2, "0");

/** UTC → (YYMMDDHH, MM, SS) KST. 형태 불일치·달력에 없는 날짜·null 은 전부 ('NA','00','00') — raise 하지 않는다.
 *  벤치 명명 SSOT(adversarial-benchmark/scripts/doc_naming.py kst_tokens)와 같은 계약이며, 두 구현의 일치는
 *  tests/doc_naming.test.ts 가 교차검증한다(단일 소유가 불가능한 자리의 차선 — workflow.md §결정론 규율). */
export function kstTokens(generatedUtc: unknown): [string, string, string] {
  if (typeof generatedUtc !== "string" || generatedUtc === "") return [...NA];
  const m = UTC_RE.exec(generatedUtc);
  if (m === null) return [...NA];
  const [y, mo, d, h, mi, s] = m.slice(1).map(Number) as [number, number, number, number, number, number];
  const t = Date.UTC(y, mo - 1, d, h, mi, s);
  const back = new Date(t);
  // Date.UTC 는 넘치는 필드를 굴려 받는다 — 되읽어 같은 필드인지 확인해 달력 밖 날짜를 거부한다
  if (back.getUTCFullYear() !== y || back.getUTCMonth() !== mo - 1 || back.getUTCDate() !== d ||
    back.getUTCHours() !== h || back.getUTCMinutes() !== mi || back.getUTCSeconds() !== s) return [...NA];
  const k = new Date(t + KST_OFFSET_HOURS * 3600_000);
  const yymmddhh = pad2(k.getUTCFullYear() % 100) + pad2(k.getUTCMonth() + 1) + pad2(k.getUTCDate()) + pad2(k.getUTCHours());
  return [yymmddhh, pad2(k.getUTCMinutes()), pad2(k.getUTCSeconds())];
}

export function validateTopic(topic: unknown): asserts topic is string {
  if (typeof topic !== "string" || topic === "" || !TOPIC_RE.test(topic) || topic === "." || topic === "..") {
    throw new Error(`doc_naming: topic must be a non-empty run of [A-Za-z0-9_. 한글] with no path separators/whitespace, got ${JSON.stringify(topic)}`);
  }
}

export function isDatedDocBasename(docType: string, basename: string): boolean {
  if (!(DATED_DOC_TYPES as readonly string[]).includes(docType) || !basename || basename.includes("/")) return false;
  const m = DATED_DOC_RE.exec(basename);
  return m !== null && m.groups!["type"] === docType;
}

function suffixed(kind: string, base: string, withSuffix: string, prefix: string, existing: Iterable<string>): string {
  const taken = new Set(existing);
  if (![...taken].some((n) => n.startsWith(prefix))) return base;
  if (taken.has(withSuffix)) {
    throw new NamingCollisionExhausted(`${kind}: both ${JSON.stringify(base)} and ${JSON.stringify(withSuffix)} are already occupied by another publication -- canon permits only unsuffixed or a single _MM_SS suffix`);
  }
  return withSuffix;
}

/** '<type>_<YYMMDDHH>[_MM_SS]_<topic>.<ext>'. 충돌 키는 (type, hour) 뿐이다(topic 무관 · P2-A04). */
export function datedDocBasename(docType: string, generatedUtc: unknown, topic: unknown, existingBasenames: Iterable<string> = [], ext = "md"): string {
  if (!(DATED_DOC_TYPES as readonly string[]).includes(docType)) {
    throw new Error(`dated_doc_basename: doc_type must be one of ${JSON.stringify(DATED_DOC_TYPES)}, got ${JSON.stringify(docType)}`);
  }
  validateTopic(topic);
  const [hour, mm, ss] = kstTokens(generatedUtc);
  return suffixed("dated_doc_basename", `${docType}_${hour}_${topic}.${ext}`, `${docType}_${hour}_${mm}_${ss}_${topic}.${ext}`,
    `${docType}_${hour}_`, existingBasenames);
}

/** simlog run 디렉터리 '<YYMMDDHH>[_MM_SS]_<topic>' — '<type>_' 접두 없음. 충돌 키는 hour 뿐이다. */
export function simlogDirname(generatedUtc: unknown, topic: unknown, existingDirnames: Iterable<string> = []): string {
  validateTopic(topic);
  const [hour, mm, ss] = kstTokens(generatedUtc);
  return suffixed("simlog_dirname", `${hour}_${topic}`, `${hour}_${mm}_${ss}_${topic}`, `${hour}_`, existingDirnames);
}

/** docs/report/ — '<분류>_<YYMMDDHH>[_MM_SS]_<한글제목>.<ext>'. 발행 시점 고정. */
export function reportBasename(name: unknown, ext = "html"): string {
  if (typeof name !== "string" || name === "" || name.includes("/") || name.includes("\\") || !REPORT_RE.test(name)) {
    throw new Error(`report_basename: name must be <분류>_<YYMMDDHH>[_MM_SS]_<한글제목> (분류=영문 키워드, 예: perf_26082421_qwen3.8-27b_레시피별_성능), got ${JSON.stringify(name)}`);
  }
  return `${name}.${ext}`;
}

// ───────────── CLI (파이썬 shim 통로) ─────────────

const FUNCS: Record<string, (...a: never[]) => unknown> = {
  kst_tokens: kstTokens as never,
  validate_topic: ((t: unknown) => { validateTopic(t); return null; }) as never,
  is_dated_doc_basename: isDatedDocBasename as never,
  dated_doc_basename: datedDocBasename as never,
  simlog_dirname: simlogDirname as never,
  report_basename: reportBasename as never,
};

function selfTest(): void {
  const eq = (a: unknown, b: unknown, what: string): void => {
    if (JSON.stringify(a) !== JSON.stringify(b)) throw new Error(`self-test ${what}: ${JSON.stringify(a)} != ${JSON.stringify(b)}`);
  };
  eq(kstTokens("2026-07-24T16:22:29Z"), ["26072501", "22", "29"], "kst 1");
  eq(kstTokens("2026-07-15T21:21:33Z"), ["26071606", "21", "33"], "kst 2");
  eq(kstTokens(null), [...NA], "kst null");
  eq(kstTokens("2026-99-99T99:99:99Z"), [...NA], "kst calendar");
  eq(datedDocBasename("plan", "2026-07-25T06:58:00Z", "하네스_루프_구조개선"), "plan_26072515_하네스_루프_구조개선.md", "plan");
  eq(datedDocBasename("testlog", "2026-07-25T06:58:29Z", "구조개선", ["testlog_26072515_구조개선.md"]), "testlog_26072515_58_29_구조개선.md", "collision");
  let threw = false;
  try { datedDocBasename("testlog", "2026-07-25T06:58:29Z", "구조개선", ["testlog_26072515_구조개선.md", "testlog_26072515_58_29_구조개선.md"]); } catch (e) { threw = e instanceof NamingCollisionExhausted; }
  eq(threw, true, "exhausted");
  eq(simlogDirname("2026-06-21T12:21:00Z", "vLLM0.22.1_KV클램프_시뮬"), "26062121_vLLM0.22.1_KV클램프_시뮬", "simlog");
  eq(reportBasename("perf_26082421_qwen3.8-27b_레시피별_성능"), "perf_26082421_qwen3.8-27b_레시피별_성능.html", "report");
  eq(isDatedDocBasename("plan", "testlog_26072515_구조개선.md"), false, "dated type");
  process.stdout.write("[doc_naming] self-test PASS\n");
}

if (process.argv[1] !== undefined && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const [fn, rawArgs] = process.argv.slice(2);
  if (fn === "--self-test") selfTest();
  else if (fn === "--patterns") process.stdout.write(JSON.stringify({ ...PATTERNS, dated_doc_types: DATED_DOC_TYPES }) + "\n");
  else if (fn !== undefined && Object.hasOwn(FUNCS, fn)) {
    const args = JSON.parse(rawArgs ?? "[]") as never[];
    try {
      process.stdout.write(JSON.stringify({ ok: true, value: FUNCS[fn]!(...args) }) + "\n");
    } catch (e) {
      // 오류 종류를 이름으로 넘긴다 — shim 이 같은 예외 클래스로 다시 던진다(삼키지 않는다)
      process.stdout.write(JSON.stringify({ ok: false, error: (e as Error).name, message: (e as Error).message }) + "\n");
      process.exitCode = 3;
    }
  } else {
    process.stderr.write(`사용: node doc_naming.ts <${Object.keys(FUNCS).join("|")}> '<JSON 인자 배열>' | --patterns | --self-test\n`);
    process.exitCode = 2;
  }
}
