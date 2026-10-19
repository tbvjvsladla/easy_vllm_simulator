// 메인의 ssh 강제명령 게이트웨이 — 서브 전용 키의 authorized_keys `command=` 가 이것만 실행한다:
//   command="node <repo>/.claude/skills/wiki-desk/src/gateway.ts --node <node_id>",restrict <서브 공개키>
// node_id 는 이 인자(= 메인이 키에 묶은 값)가 정한다. 서브는 자기 node_id 를 주장할 수 없다.
// 허용하는 것은 두 모양뿐이고 나머지는 전부 거부한다(셸·임의 명령 ✗):
//   ① `wiki-desk-v1 <base64url(JSON argv)>` — 닫힌 명령·플래그 목록만. 쓰기(sync)는 `--as <node_id>` 를 강제한다
//   ② `rsync --server …` 수신 — rrsync -wo 로 그 노드의 미러 디렉터리 하나에만 쓴다(배타 잠금 아래)
import fs from "node:fs";
import path from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { PROTOCOL_TOKEN } from "./remote.ts";
import { assertNodeId, LOCK_FILE, LOCK_WAIT_SECONDS, DEFAULT_NODE_MIRROR_ROOT } from "./concurrency.ts";
import { findContract } from "./contract.ts";
import { isSafeRelative } from "./fs-safety.ts";

const HERE = path.dirname(fileURLToPath(import.meta.url));
// src → wiki-desk → skills → .claude → 저장소 루트
const REPO = path.resolve(HERE, "..", "..", "..", "..");
const CLI = path.join(HERE, "cli.ts");
const EXIT_REFUSED = 126;

/** 명령별 허용 플래그(값 있음 = true) · 위치 인자 수. 이 표 밖은 거부한다. */
const ALLOWED: Record<string, { flags: Record<string, boolean>; positionals: number }> = {
  query: { flags: { "--terms": true, "--limit": true }, positionals: 0 },
  read: { flags: {}, positionals: 1 },
  tree: { flags: {}, positionals: 1 },
  check: { flags: {}, positionals: 0 },
  status: { flags: {}, positionals: 0 },
  contract: { flags: {}, positionals: 0 },
  sync: { flags: { "--apply": false, "--request-id": true, "--payload-sha": true, "--accept-removed": true }, positionals: 0 },
};

function refuse(why: string): never {
  process.stderr.write(`wiki-desk gateway: 거부 — ${why}\n`);
  process.exit(EXIT_REFUSED);
}

function decodeArgv(token: string): string[] {
  let v: unknown;
  try { v = JSON.parse(Buffer.from(token, "base64url").toString("utf8")); } catch { refuse("argv 해독 실패"); }
  if (!Array.isArray(v) || v.length === 0 || !v.every((x) => typeof x === "string")) refuse("argv 는 비어 있지 않은 문자열 배열");
  return v as string[];
}

/** argv 를 표에 대조하고 로컬 CLI 인자로 바꾼다. 쓰기는 소유자를 이 키의 node_id 로 못박는다. */
export function vet(argv: readonly string[], node: string, mirrorRoot: string): string[] {
  const [cmd, ...rest] = argv;
  if (cmd === undefined || !Object.hasOwn(ALLOWED, cmd)) refuse(`허용되지 않은 명령: ${JSON.stringify(cmd)}`);
  const spec = ALLOWED[cmd]!;
  const out: string[] = [cmd];
  let positionals = 0;
  for (let i = 0; i < rest.length; i++) {
    const a = rest[i]!;
    if (a.startsWith("--")) {
      if (!Object.hasOwn(spec.flags, a)) refuse(`${cmd} 에 허용되지 않은 플래그: ${a}`);
      if (spec.flags[a]) {
        const val = rest[++i];
        if (val === undefined) refuse(`${a} 값 누락`);
        if (a === "--accept-removed" && !(isSafeRelative(val) && val.startsWith(`${mirrorRoot}/${node}/`))) refuse(`자기 미러 밖 경로 수용 지정: ${val}`);
        out.push(a, val);
      } else out.push(a);
    } else {
      if (++positionals > spec.positionals) refuse(`${cmd} 위치 인자 초과`);
      out.push(a);
    }
  }
  if (cmd === "sync") {
    if (!out.includes("--apply")) refuse("게이트웨이 sync 는 입고(--apply)만 받는다");
    if (!out.includes("--request-id")) refuse("게이트웨이 입고는 멱등 키(--request-id)가 필요하다");
    out.push("--as", node);
  }
  return [...out, "--root", REPO, "--local"];
}

function main(): void {
  const i = process.argv.indexOf("--node");
  const node = assertNodeId(process.argv[i + 1] ?? refuse("--node 누락(authorized_keys 설정 오류)"));
  const original = process.env["SSH_ORIGINAL_COMMAND"] ?? refuse("SSH_ORIGINAL_COMMAND 없음 — 대화형 셸은 열지 않는다");
  const { wikiDir, contract } = findContract(REPO);
  const mirrorRoot = typeof contract["node_mirror_root"] === "string" ? contract["node_mirror_root"] as string : DEFAULT_NODE_MIRROR_ROOT;
  if (original.startsWith(`${PROTOCOL_TOKEN} `)) {
    const parts = original.split(" ");
    if (parts.length !== 2) refuse("프로토콜 토큰 뒤에는 인코딩된 argv 하나만 온다");
    const argv = vet(decodeArgv(parts[1]!), node, mirrorRoot);
    const r = spawnSync(process.execPath, [CLI, ...argv], { stdio: "inherit" });
    process.exit(r.status ?? 1);
  }
  if (/^rsync --server( |$)/.test(original)) {
    if (/(^| )--sender( |$)/.test(original)) refuse("반출용 rsync(--sender)는 열지 않는다 — 반출은 질의 결과뿐이다");
    const mirror = path.join(REPO, mirrorRoot, node);
    fs.mkdirSync(mirror, { recursive: true });
    // rrsync -wo: 쓰기 전용 · 그 디렉터리 밖 경로 거부. 배타 잠금 아래에서 받아 sync 와 직렬화한다.
    const r = spawnSync("flock", ["-x", "-w", String(LOCK_WAIT_SECONDS), path.join(REPO, wikiDir, LOCK_FILE), "rrsync", "-wo", mirror], { stdio: "inherit" });
    if (r.error !== undefined) refuse(`rrsync/flock 실행 실패: ${r.error.message}`);
    process.exit(r.status ?? 1);
  }
  refuse("허용 목록 밖 명령");
}

if (process.argv[1] !== undefined && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) main();
