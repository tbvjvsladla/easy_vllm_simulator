// 접근자가 ssh 인 체크아웃(서브)의 클라이언트. 같은 CLI 명령을 메인 게이트웨이로 넘긴다 — 메인과 경로만 다르다.
// 셸을 거치지 않는다: argv 를 JSON → base64url 한 토큰으로 실어 보내고, 게이트웨이가 그것만 해독한다(인용·주입 ✗).
// 입고(sync --apply) = ① 계약 조회 ② 입고 루트의 원문을 rsync 로 메인의 자기 미러에 반입 ③ 멱등 요청으로 원격 sync.
// 반출(query/read/tree)은 메인 stdout 을 그대로 내보낸다 — 이 디스크에 사본을 남기지 않는다.
import fs from "node:fs";
import path from "node:path";
import crypto from "node:crypto";
import { spawnSync } from "node:child_process";
import type { Accessor } from "./accessor.ts";
import { payloadSha } from "./concurrency.ts";
import { sha256 } from "./fs-safety.ts";

export const PROTOCOL_TOKEN = "wiki-desk-v1";
/** ssh 자체 실패(접속·인증·호스트키) 종료코드 — 원격 명령의 실패와 구분된다 */
const SSH_TRANSPORT_FAILURE = 255;
/** 원격 입고 재전송 횟수(같은 request_id) — 응답 유실·일시 단절의 재전송이 멱등 키로 no-op 이 된다 */
const RETRIES = 3;
/** 입고 루트 기본값(계약 확장 키 node_intake_roots 가 없을 때) */
const DEFAULT_INTAKE_ROOTS = ["docs/plan", "docs/devlog", "docs/testlog", "docs/simlog"];
/** 서고에 닿지 못해 입고가 미뤄졌다 — 호출자는 발행을 막지 않고 not-yet-ingested 로 기록한다 */
export const EXIT_NOT_INGESTED = 4;
const CONNECT_TIMEOUT_S = 10;

type Ssh = Extract<Accessor, { transport: "ssh" }>;

export function sshArgs(a: Ssh): string[] {
  return ["-F", "/dev/null", "-o", "BatchMode=yes", "-o", "StrictHostKeyChecking=yes", "-o", `UserKnownHostsFile=${a.known_hosts}`,
    "-o", "IdentitiesOnly=yes", "-o", `ConnectTimeout=${CONNECT_TIMEOUT_S}`, "-i", a.identity_file, `${a.user}@${a.host}`];
}

export const encodeArgv = (argv: readonly string[]): string => Buffer.from(JSON.stringify(argv)).toString("base64url");

export type Remote = { status: number; stdout: string; stderr: string };

export function remoteCall(a: Ssh, argv: readonly string[]): Remote {
  const r = spawnSync("ssh", [...sshArgs(a), `${PROTOCOL_TOKEN} ${encodeArgv(argv)}`], { encoding: "utf8", maxBuffer: 64 * 1024 * 1024 });
  if (r.error !== undefined) throw new Error(`ssh 실행 실패: ${r.error.message}`);
  return { status: r.status ?? SSH_TRANSPORT_FAILURE, stdout: r.stdout, stderr: r.stderr };
}

/** 반출 명령: 원격 결과를 그대로 흘려보내고 같은 종료코드로 끝난다 */
export function forward(a: Ssh, argv: readonly string[]): never {
  const r = remoteCall(a, argv);
  process.stdout.write(r.stdout);
  process.stderr.write(r.stderr);
  process.exit(r.status);
}

function walkMd(root: string, rel: string, suffixes: readonly string[], out: string[]): void {
  const abs = path.join(root, rel);
  if (!fs.existsSync(abs)) return;
  for (const e of fs.readdirSync(abs, { withFileTypes: true })) {
    const child = `${rel}/${e.name}`;
    if (e.isSymbolicLink()) continue; // 원문 심링크는 서고가 거부한다 — 반입하지 않는다
    if (e.isDirectory()) walkMd(root, child, suffixes, out);
    else if (e.isFile() && suffixes.includes(path.extname(e.name).toLowerCase())) out.push(child);
  }
}

export type IntakeResult = Record<string, unknown> & { ingest_status: "ingested" | "not-yet-ingested" };

/** 입고: 반입 → 원격 멱등 sync. 서고 미도달은 예외가 아니라 not-yet-ingested 로 돌려준다(fail-loud 표시). */
export function intake(a: Ssh, localRoot: string, extra: readonly string[] = [], fixedRequestId?: string): IntakeResult {
  const notIngested = (stage: string, r: Remote): IntakeResult =>
    ({ ingest_status: "not-yet-ingested", stage, status: r.status, stderr: r.stderr.trim().slice(-2000) });
  const c = remoteCall(a, ["contract"]);
  if (c.status === SSH_TRANSPORT_FAILURE) return notIngested("contract", c);
  if (c.status !== 0) throw new Error(`원격 계약 조회 실패(exit ${c.status}): ${c.stderr.trim()}`);
  const contract = JSON.parse(c.stdout) as { node_intake_roots?: string[]; source_suffixes: string[] };
  const roots = (contract.node_intake_roots ?? DEFAULT_INTAKE_ROOTS).filter((r) => fs.existsSync(path.join(localRoot, r)));
  const suffixes = contract.source_suffixes.map((s) => s.toLowerCase());
  const files: string[] = [];
  for (const r of roots) walkMd(localRoot, r, suffixes, files);
  const sha = payloadSha(files.map((f) => [f, sha256(fs.readFileSync(path.join(localRoot, f)))] as const));
  if (roots.length > 0) {
    // 접미 필터는 클라이언트가 건다(프로토콜로 전달) — 서버 쪽 rrsync 는 자기 미러 밖 경로를 거부한다
    const filters = ["--include=*/", ...suffixes.flatMap((s) => [`--include=*${s}`, `--include=*${s.toUpperCase()}`]), "--exclude=*"];
    const r = spawnSync("rsync", ["-a", "--delete", "--relative", "--prune-empty-dirs", ...filters,
      "-e", ["ssh", ...sshArgs(a).slice(0, -1)].join(" "), ...roots, `${a.user}@${a.host}:.`], { cwd: localRoot, encoding: "utf8" });
    if (r.error !== undefined) throw new Error(`rsync 실행 실패: ${r.error.message}`);
    if (r.status === SSH_TRANSPORT_FAILURE || r.status === 12 || r.status === 10) return notIngested("rsync", { status: r.status ?? 1, stdout: r.stdout, stderr: r.stderr });
    if (r.status !== 0) throw new Error(`rsync 반입 실패(exit ${r.status}): ${r.stderr.trim()}`);
  }
  // 재전송: 호출자가 같은 request_id 를 주면 같은 논리적 요청으로 다시 보낸다(서버가 원장으로 no-op 처리 · C16)
  const requestId = fixedRequestId ?? crypto.randomUUID().replaceAll("-", "");
  let last: Remote | null = null;
  for (let attempt = 0; attempt < RETRIES; attempt++) {
    last = remoteCall(a, ["sync", "--apply", "--request-id", requestId, "--payload-sha", sha, ...extra]);
    if (last.status !== SSH_TRANSPORT_FAILURE) break;
  }
  if (last!.status === SSH_TRANSPORT_FAILURE) return notIngested("sync", last!);
  if (last!.status !== 0) throw new Error(`원격 sync 실패(exit ${last!.status}): ${last!.stderr.trim()}`);
  return { ...(JSON.parse(last!.stdout) as Record<string, unknown>), ingest_status: "ingested", request_id: requestId, payload_sha256: sha, files: files.length };
}
