// 배포 위생 검사(배포자용): 패키지 트리에 사이트 식별자·개인 경로·사설 IP·특정 프로젝트 식별자가 없는지 본다.
// 사용: node scripts/sanitize-check.ts [패키지 루트]  — 종료코드 0 = 0건, 1 = 발견. 금지 패턴은 FORBIDDEN_PATTERNS 한 곳에 둔다.
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

export const DEFAULT_ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const EXCLUDED_DIRS: ReadonlySet<string> = new Set([".git", "node_modules", ".cache"]);
// 이 파일 자신이 검사에 걸리지 않도록 사이트 이름은 유니코드 이스케이프·조각으로 적는다
const SITE_NAMES = ["\uacbd\ub9ac\ub098\ub77c", "kyung" + "rinara"];

export const FORBIDDEN_PATTERNS: Readonly<Record<string, RegExp>> = {
  site_name: new RegExp(SITE_NAMES.join("|"), "i"),
  home_path: /\/home\/[a-z_][a-z0-9_-]*\//,
  private_ip: /(?<![0-9.])(?:10\.\d{1,3}|172\.(?:1[6-9]|2\d|3[01])|192\.168)\.\d{1,3}\.\d{1,3}(?![0-9])/,
  // 워크스페이스 이름 형식(두 자리 번호_대문자로 시작하는 이름)과 그 번호를 딴 카드 namespace 키.
  // 키 패턴 앞 `(?<!\\)`는 `\x07` 같은 문자열 이스케이프를 제외한다
  project_identifier: new RegExp("\\b\\d{2}_[A-Z][A-Za-z]+_[A-Za-z_]+\\b|(?<!\\\\)\\bx" + "\\d{2}(?:_unknown)?\\b"),
};

export type Hit = { rel: string; line: number; pattern: string };

export function scan(root: string = DEFAULT_ROOT): Hit[] {
  const hits: Hit[] = [];
  const walk = (dir: string): void => {
    for (const e of fs.readdirSync(dir, { withFileTypes: true }).sort((a, b) => a.name.localeCompare(b.name))) {
      if (EXCLUDED_DIRS.has(e.name)) continue;
      const p = path.join(dir, e.name);
      if (e.isDirectory()) walk(p);
      else if (e.isFile()) {
        const buf = fs.readFileSync(p);
        if (buf.includes(0)) continue; // 바이너리는 검사 대상이 아니다
        buf.toString("utf8").split("\n").forEach((text, i) => {
          for (const [name, re] of Object.entries(FORBIDDEN_PATTERNS)) {
            if (re.test(text)) hits.push({ rel: path.relative(root, p), line: i + 1, pattern: name });
          }
        });
      }
    }
  };
  walk(root);
  return hits;
}

if (process.argv[1] !== undefined && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const hits = scan(process.argv[2] ?? DEFAULT_ROOT);
  for (const h of hits) console.log(`${h.rel}:${h.line}: ${h.pattern}`);
  console.log(`SANITIZE ${hits.length === 0 ? "OK" : "FAIL"} (${hits.length} hits)`);
  process.exit(hits.length === 0 ? 0 : 1);
}
