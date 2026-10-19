// wiki-desk CLI. 사용: node .claude/skills/wiki-desk/src/cli.ts <명령> [옵션]  — 출력은 JSON(stdout)
// 서고 접근자(manifest `library`)가 ssh 면 같은 명령을 메인 게이트웨이로 넘긴다 — 메인과 경로만 다르다(accessor.ts·remote.ts).
// 로컬 서고에서는 flock 아래에서 돈다: 쓰기 = 배타, 읽기 = 공유(concurrency.ts · references/concurrency.md).
import fs from "node:fs";
import path from "node:path";
import { parseArgs } from "node:util";
import { initWiki, syncWiki, queryWiki, readRef, treeOf, checkWiki, statusWiki, removeWiki, enterWiki } from "./commands.ts";
import { findContract, CONTRACT_FILE } from "./contract.ts";
import { resolveAccessor } from "./accessor.ts";
import { forward, intake, EXIT_NOT_INGESTED } from "./remote.ts";
import { reexecUnderLock, assertNodeId, MAIN_OWNER, REQUEST_ID_RE } from "./concurrency.ts";
import type { LockMode } from "./concurrency.ts";

const USAGE = `명령: init --contract <파일> [--apply] | sync [--apply] [--as <main|node_id|all>] [--request-id ID --payload-sha SHA]
      [--accept-removed P]... [--relocate OLD=NEW]... | query --terms <질의>... [--limit N] | read <경로:시작-끝> | tree <경로>
      | check | status | contract | remove [--apply]
공통: --root <프로젝트 루트>(생략 시 현재 디렉터리) · --local(접근자 무시 — 게이트웨이 전용)`;

const { values, positionals } = parseArgs({
  allowPositionals: true,
  options: {
    root: { type: "string" },
    contract: { type: "string" },
    apply: { type: "boolean", default: false },
    terms: { type: "string", multiple: true },
    limit: { type: "string" },
    "accept-removed": { type: "string", multiple: true },
    relocate: { type: "string", multiple: true },
    as: { type: "string" },
    "request-id": { type: "string" },
    "payload-sha": { type: "string" },
    local: { type: "boolean", default: false },
  },
});

const [cmd, arg] = positionals;
const root = path.resolve(values.root ?? process.cwd());
const now = new Date().toISOString();
const need = (v: string | undefined, name: string): string => {
  if (v === undefined || v === "") throw new Error(`${name} 필요\n${USAGE}`);
  return v;
};
const pair = (s: string): [string, string] => {
  const i = s.indexOf("=");
  if (i <= 0 || i === s.length - 1) throw new Error(`--relocate 형식은 OLD=NEW: ${s}`);
  return [s.slice(0, i), s.slice(i + 1)];
};
const limit = values.limit === undefined ? undefined : Number(values.limit);
if (limit !== undefined && !(Number.isInteger(limit) && limit > 0)) throw new Error(`--limit는 양의 정수: ${values.limit}`);
const name = need(cmd, "<명령>");

// ── 접근자: ssh 면 원격(경로만 다름) ──
const accessor = values.local ? null : resolveAccessor(root);
if (accessor !== null && accessor.transport === "ssh") {
  if (name === "sync") {
    if (!values.apply) throw new Error("서브 체크아웃의 sync 는 입고(--apply)만 한다 — 계획은 메인에서 본다");
    const extra = (values["accept-removed"] ?? []).flatMap((p) => ["--accept-removed", p]);
    const rid = values["request-id"];
    if (rid !== undefined && !REQUEST_ID_RE.test(rid)) throw new Error(`--request-id 형식 오류: ${rid}`);
    const r = intake(accessor, root, extra, rid);
    process.stdout.write(JSON.stringify(r, null, 2) + "\n");
    if (r.ingest_status === "not-yet-ingested") process.exitCode = EXIT_NOT_INGESTED;
  } else if (["query", "read", "tree", "check", "status", "contract"].includes(name)) {
    const argv = [name, ...(arg === undefined ? [] : [arg]), ...(values.terms ?? []).flatMap((t) => ["--terms", t]),
      ...(values.limit === undefined ? [] : ["--limit", values.limit])];
    forward(accessor, argv);
  } else {
    throw new Error(`서고가 원격(ssh)인 체크아웃에서는 ${name} 을 실행하지 않는다 — 서고 관리는 메인에서 한다`);
  }
} else {
  runLocal();
}

function runLocal(): void {
  // ── 잠금(init 은 위키가 아직 없으므로 제외 · init 은 기존 위키가 있으면 거부한다) ──
  if (name !== "init") {
    const { wikiDir } = findContract(root);
    const writes = (name === "sync" || name === "remove") && values.apply;
    reexecUnderLock(path.join(root, wikiDir), (writes ? "x" : "s") as LockMode);
  }
  const recovered = name === "init" ? null : enterWiki(root);

  let owner: string | undefined;
  if (name === "sync") {
    const as = values.as ?? MAIN_OWNER;
    owner = as === "all" ? undefined : as === MAIN_OWNER ? MAIN_OWNER : assertNodeId(as);
    const rid = values["request-id"];
    if (rid !== undefined && !REQUEST_ID_RE.test(rid)) throw new Error(`--request-id 형식 오류: ${rid}`);
  }
  const run: Record<string, () => unknown> = {
    init: () => initWiki(root, need(values.contract, "--contract"), { apply: values.apply, now }),
    sync: () => syncWiki(root, {
      apply: values.apply, now, owner,
      acceptRemoved: values["accept-removed"] ?? [],
      relocate: (values.relocate ?? []).map(pair),
      requestId: values["request-id"], payloadSha: values["payload-sha"],
    }),
    query: () => queryWiki(root, values.terms ?? [], limit),
    read: () => readRef(root, need(arg, "<경로:시작-끝>")),
    tree: () => treeOf(root, need(arg, "<경로>")),
    check: () => checkWiki(root),
    status: () => statusWiki(root),
    contract: () => JSON.parse(fs.readFileSync(path.join(root, findContract(root).wikiDir, CONTRACT_FILE), "utf8")),
    remove: () => removeWiki(root, { apply: values.apply }),
  };
  // 상속 속성(toString·constructor 등)이 명령으로 잡히지 않게 자기 키만 허용
  if (!Object.hasOwn(run, name)) throw new Error(`알 수 없는 명령: ${name}\n${USAGE}`);
  const result = run[name]!();
  // 저널 복구는 침묵하지 않는다 — 결과 옆에 싣는다(배열 결과는 감싸지 않고 stderr 로 고지)
  if (recovered !== null) {
    if (typeof result === "object" && result !== null && !Array.isArray(result)) (result as Record<string, unknown>)["journal_recovered"] = recovered;
    else process.stderr.write(`wiki-desk: 저널 복구 ${JSON.stringify(recovered)}\n`);
  }
  process.stdout.write(JSON.stringify(result, null, 2) + "\n");
  // check는 문제가 있으면 exit 1 (기계 판정용)
  if (name === "check" && !(result as { ok: boolean }).ok) process.exitCode = 1;
}
