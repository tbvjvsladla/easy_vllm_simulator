// 서고 접근자 — 이 체크아웃의 wiki-desk 가 서고(__llm-wiki)에 닿는 경로. 메인·서브 동일 스키마이며 경로만 다르다.
// 권위 = 이 체크아웃 토폴로지의 manifest(`output/<topology>/manifest.yaml`)의 `library` 블록:
//   library: { transport: local|ssh, host, user, repo_root, wiki_dir, identity_file, known_hosts, node_id }
// 토폴로지는 특화헌법 자기선언 헤더(`**topology: X**`)에서 읽는다 — 브랜치는 어느 manifest 를 읽을지 고르는 필터일 뿐이다.
import fs from "node:fs";
import path from "node:path";
import { parseYaml } from "./frontmatter.ts";
import { WIKI_DIRS } from "./contract.ts";

export type Accessor =
  | { transport: "local"; wiki_dir: string; repo_root: string; source: string }
  | { transport: "ssh"; host: string; user: string; repo_root: string; wiki_dir: string; identity_file: string; known_hosts: string; node_id: string; source: string };

const TOPOLOGY_HEADER = /\*\*topology:\s*([a-z]+)\*\*/;
const SPEC_FILE = ".claude/rules/strategy.topology.md";
// ssh 필드 문법 — 명령줄에 실리므로 닫힌 문자 집합만 허용한다(셸 메타문자 ✗)
const HOST_RE = /^[A-Za-z0-9._-]{1,253}$/;
const USER_RE = /^[a-z_][a-z0-9_-]{0,31}$/;
const ABS_PATH_RE = /^\/[A-Za-z0-9._/-]+$/;

export function manifestPath(root: string): string | null {
  const spec = path.join(root, SPEC_FILE);
  if (!fs.existsSync(spec)) return null;
  const m = TOPOLOGY_HEADER.exec(fs.readFileSync(spec, "utf8"));
  if (m === null) return null;
  return path.join(root, "output", m[1]!, "manifest.yaml");
}

function field(lib: Record<string, unknown>, key: string, re: RegExp, src: string): string {
  const v = lib[key];
  if (typeof v !== "string" || !re.test(v)) throw new Error(`manifest library.${key} 형식 오류(${src}): ${JSON.stringify(v)}`);
  return v;
}

/**
 * 접근자 해소. manifest 에 library 블록이 없으면 local(이 루트의 서고)로 둔다 — 출처를 `source` 에 밝힌다
 * (`default:no-library-block`). 블록이 있는데 형식이 틀리면 대체하지 않고 거부한다(침묵 폴백 ✗).
 */
export function resolveAccessor(root: string): Accessor {
  const mp = manifestPath(root);
  if (mp === null || !fs.existsSync(mp)) return { transport: "local", wiki_dir: WIKI_DIRS[0], repo_root: root, source: "default:no-manifest" };
  const doc = parseYaml(fs.readFileSync(mp, "utf8"));
  const lib = doc["library"];
  const src = path.relative(root, mp);
  if (lib === undefined || lib === null) return { transport: "local", wiki_dir: WIKI_DIRS[0], repo_root: root, source: `default:no-library-block(${src})` };
  if (typeof lib !== "object" || Array.isArray(lib)) throw new Error(`manifest library 는 객체여야 한다(${src})`);
  const l = lib as Record<string, unknown>;
  const wikiDir = l["wiki_dir"] ?? WIKI_DIRS[0];
  if (!WIKI_DIRS.includes(wikiDir as never)) throw new Error(`manifest library.wiki_dir 는 ${WIKI_DIRS.join("|")} 중 하나(${src})`);
  if (l["transport"] === "local") {
    const repoRoot = l["repo_root"] === undefined ? root : field(l, "repo_root", ABS_PATH_RE, src);
    return { transport: "local", wiki_dir: wikiDir as string, repo_root: repoRoot, source: src };
  }
  if (l["transport"] !== "ssh") throw new Error(`manifest library.transport 는 local|ssh(${src}): ${JSON.stringify(l["transport"])}`);
  return {
    transport: "ssh",
    host: field(l, "host", HOST_RE, src), user: field(l, "user", USER_RE, src),
    repo_root: field(l, "repo_root", ABS_PATH_RE, src), wiki_dir: wikiDir as string,
    identity_file: field(l, "identity_file", ABS_PATH_RE, src), known_hosts: field(l, "known_hosts", ABS_PATH_RE, src),
    node_id: field(l, "node_id", /^[a-z0-9][a-z0-9_-]{0,62}$/, src), source: src,
  };
}
