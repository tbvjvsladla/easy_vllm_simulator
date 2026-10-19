// N-writer 동시성 규정(references/concurrency.md)을 실제 CLI 프로세스로 핀한다 — h_report §6.1 실패 순서:
// 동시 입고(유실·중복 0) · 재전송(no-op · receipt 바이트 동일) · 같은 키 다른 내용(충돌 거부) · 쓰기 중 kill(중간 상태 비관측 · 잔재 0)
// · 남의 카드 비접촉(C12). 잠금은 flock(1) 재실행 경로를 그대로 탄다(시험용 우회 ✗).
import { test, describe, beforeEach, afterEach } from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { spawn, spawnSync } from "node:child_process";
import { payloadSha, INTAKE_MANIFEST } from "../src/concurrency.ts";
import { sha256 } from "../src/fs-safety.ts";
import { vet } from "../src/gateway.ts";

const CLI = path.join(import.meta.dirname, "..", "src", "cli.ts");
const BASE = JSON.parse(fs.readFileSync(path.join(import.meta.dirname, "fixtures", "contract", "valid.json"), "utf8"));
const NODES = ["sub-a", "sub-b", "sub-c"];
const DOCS_PER_OWNER = 12;

let root = "";
const put = (rel: string, data: string): void => {
  fs.mkdirSync(path.dirname(path.join(root, rel)), { recursive: true });
  fs.writeFileSync(path.join(root, rel), data);
};
const cli = (...args: string[]) => spawnSync(process.execPath, [CLI, ...args, "--root", root, "--local"], { encoding: "utf8" });
const cliAsync = (...args: string[]): Promise<{ code: number | null; out: string; err: string }> => new Promise((resolve) => {
  const p = spawn(process.execPath, [CLI, ...args, "--root", root, "--local"]);
  let out = ""; let err = "";
  p.stdout.on("data", (d) => (out += d)); p.stderr.on("data", (d) => (err += d));
  p.on("close", (code) => resolve({ code, out, err }));
});
const doc = (title: string): string => `---\ntitle: ${title}\n---\n# ${title}\n\n본문 ${title} 동시성 시험\n`;
const mirrorDocs = (node: string): string[] => fs.readdirSync(path.join(root, "mirror", node, "docs", "devlog")).map((f) => `docs/devlog/${f}`);
// 클라이언트가 하는 일을 재현한다: 이번에 보낸 집합을 반입 목록(INTAKE_MANIFEST)으로 미러에 두고 그 payload 해시를 낸다
const nodePayload = (node: string): string => {
  const entries = mirrorDocs(node).map((f) => [f, sha256(fs.readFileSync(path.join(root, "mirror", node, f)))] as [string, string]);
  fs.writeFileSync(path.join(root, "mirror", node, INTAKE_MANIFEST), JSON.stringify({ entries }));
  return payloadSha(entries);
};
const snapshot = (): Record<string, string> => {
  const out: Record<string, string> = {};
  const walk = (d: string): void => {
    for (const e of fs.readdirSync(path.join(root, d), { withFileTypes: true })) {
      const rel = `${d}/${e.name}`;
      if (e.name === ".cache" || e.name === ".lock") continue;
      if (e.isDirectory()) walk(rel); else out[rel] = sha256(fs.readFileSync(path.join(root, rel)));
    }
  };
  walk("__llm-wiki");
  return out;
};

beforeEach(() => {
  root = fs.mkdtempSync(path.join(os.tmpdir(), "wd-conc-"));
  const contract = { ...BASE, source_roots: ["docs", "mirror"], node_mirror_root: "mirror" };
  fs.writeFileSync(path.join(root, "contract.json"), JSON.stringify(contract));
  for (let i = 0; i < DOCS_PER_OWNER; i++) put(`docs/devlog/devlog_2610010${i % 10}_메인${i}.md`, doc(`메인${i}`));
  for (const n of NODES) for (let i = 0; i < DOCS_PER_OWNER; i++) put(`mirror/${n}/docs/devlog/devlog_2610020${i % 10}_${n}${i}.md`, doc(`${n}-${i}`));
  assert.equal(cli("init", "--contract", path.join(root, "contract.json"), "--apply").status, 0);
});
afterEach(() => fs.rmSync(root, { recursive: true, force: true }));

describe("동시 입고 — 유실·중복 0 (C06 + C12)", () => {
  test("메인과 서브 3개가 동시에 sync --apply 해도 원문당 카드 1장 · receipt 가 전 카드 집합과 일치", async () => {
    const runs = await Promise.all([
      cliAsync("sync", "--apply", "--as", "main"),
      ...NODES.map((n) => cliAsync("sync", "--apply", "--as", n, "--request-id", `req-${n}-0001`, "--payload-sha", nodePayload(n))),
    ]);
    for (const r of runs) assert.equal(r.code, 0, r.err);
    const check = JSON.parse(cli("check").stdout);
    assert.equal(check.ok, true, JSON.stringify(check.issues));
    assert.equal(check.counts.cards, DOCS_PER_OWNER * (NODES.length + 1));
    const receipt = JSON.parse(fs.readFileSync(path.join(root, "__llm-wiki", "receipt.json"), "utf8")).files as Record<string, string>;
    const cardFiles = Object.keys(receipt).filter((f) => f.endsWith(".card.md"));
    assert.equal(cardFiles.length, DOCS_PER_OWNER * (NODES.length + 1), "receipt 가 카드 집합과 다르다(갱신 유실)");
    for (const f of cardFiles) assert.equal(receipt[f], sha256(fs.readFileSync(path.join(root, f))), `receipt 해시 불일치: ${f}`);
    assert.ok(!Object.keys(receipt).some((f) => f.includes("/.requests/")), "원장이 소유 기록(receipt)에 섞였다");
    // index 는 누가 마지막에 돌렸든 전 카드를 싣는다
    const index = fs.readFileSync(path.join(root, "__llm-wiki", "index.md"), "utf8");
    assert.equal((index.match(/\.card\.md\)/g) ?? []).length, DOCS_PER_OWNER * (NODES.length + 1));
  });

  test("서브는 남의 카드를 만들지도 고치지도 않는다(키별 단일 소유)", () => {
    assert.equal(cli("sync", "--apply", "--as", "sub-a", "--request-id", "req-own-00001", "--payload-sha", nodePayload("sub-a")).status, 0);
    const cards = fs.readdirSync(path.join(root, "__llm-wiki", "cards"), { recursive: true }).map(String).filter((f) => f.endsWith(".card.md"));
    assert.equal(cards.length, DOCS_PER_OWNER);
    assert.ok(cards.every((c) => c.startsWith(path.join("mirror", "sub-a"))), cards.join(","));
  });
});

describe("재전송 — 멱등 키 (C16)", () => {
  test("같은 request_id·같은 payload 재전송은 no-op 이고 receipt 가 바이트 동일", () => {
    const args = ["sync", "--apply", "--as", "sub-b", "--request-id", "req-retry-0001", "--payload-sha", nodePayload("sub-b")];
    assert.equal(cli(...args).status, 0);
    const before = fs.readFileSync(path.join(root, "__llm-wiki", "receipt.json"));
    const snap = snapshot();
    const again = cli(...args);
    assert.equal(again.status, 0, again.stderr);
    assert.equal(JSON.parse(again.stdout).replayed, true);
    assert.deepEqual(fs.readFileSync(path.join(root, "__llm-wiki", "receipt.json")), before);
    assert.deepEqual(snapshot(), snap);
  });

  test("같은 request_id 에 다른 payload 는 충돌로 거부", () => {
    assert.equal(cli("sync", "--apply", "--as", "sub-b", "--request-id", "req-conf-0001", "--payload-sha", nodePayload("sub-b")).status, 0);
    const r = cli("sync", "--apply", "--as", "sub-b", "--request-id", "req-conf-0001", "--payload-sha", "0".repeat(64));
    assert.notEqual(r.status, 0);
    assert.match(r.stderr, /멱등 키 충돌/);
  });

  test("미러가 보낸 집합과 다르면(반입 미완) 쓰지 않는다", () => {
    nodePayload("sub-c"); // 목록은 있으나 클라이언트가 보낸 해시가 다르다
    const r = cli("sync", "--apply", "--as", "sub-c", "--request-id", "req-mism-0001", "--payload-sha", "f".repeat(64));
    assert.notEqual(r.status, 0);
    assert.match(r.stderr, /미러 payload 불일치/);
  });
});

describe("입고는 더하기만 한다 — 서브 문서 집합이 바뀌어도 기존 입고가 막히지 않는다", () => {
  test("반입 목록 밖의 이전 입고분은 대조하지 않고 그대로 둔다(고아 카드로 입고를 막지 않는다)", () => {
    assert.equal(cli("sync", "--apply", "--as", "sub-a", "--request-id", "req-app-00001", "--payload-sha", nodePayload("sub-a")).status, 0);
    // 서브가 다른 체크아웃으로 바뀌어 보낸 집합이 줄었다 — 미러에는 이전 원문이 남고(반입은 지우지 않는다) 목록만 줄어든다
    const all = mirrorDocs("sub-a");
    const entries = all.slice(1).map((f) => [f, sha256(fs.readFileSync(path.join(root, "mirror", "sub-a", f)))] as [string, string]);
    fs.writeFileSync(path.join(root, "mirror", "sub-a", INTAKE_MANIFEST), JSON.stringify({ entries }));
    put(`mirror/sub-a/docs/devlog/devlog_26100301_새문서.md`, doc("새 문서"));
    const withNew = [...entries, ["docs/devlog/devlog_26100301_새문서.md", sha256(fs.readFileSync(path.join(root, "mirror/sub-a/docs/devlog/devlog_26100301_새문서.md")))] as [string, string]];
    fs.writeFileSync(path.join(root, "mirror", "sub-a", INTAKE_MANIFEST), JSON.stringify({ entries: withNew }));
    const r = cli("sync", "--apply", "--as", "sub-a", "--request-id", "req-app-00002", "--payload-sha", payloadSha(withNew));
    assert.equal(r.status, 0, r.stderr);
    assert.deepEqual(JSON.parse(r.stdout).created, ["mirror/sub-a/docs/devlog/devlog_26100301_새문서.md"]);
  });
  test("목록의 파일이 미러와 다르면(반입 미완) 거부한다", () => {
    const entries = [["docs/devlog/없는.md", "0".repeat(64)]] as [string, string][];
    fs.mkdirSync(path.join(root, "mirror", "sub-b"), { recursive: true });
    fs.writeFileSync(path.join(root, "mirror", "sub-b", INTAKE_MANIFEST), JSON.stringify({ entries }));
    const r = cli("sync", "--apply", "--as", "sub-b", "--request-id", "req-app-00003", "--payload-sha", payloadSha(entries));
    assert.notEqual(r.status, 0);
    assert.match(r.stderr, /반입 미완 또는 손상/);
  });
});

describe("쓰기 중 kill — 엄격 원자성(저널)", () => {
  test("kill 직후 읽기는 중간 상태를 보지 않고(복구 후 읽음), 위키는 직전 커밋과 같으며 잔재가 없다", () => {
    assert.equal(cli("sync", "--apply", "--as", "main").status, 0);
    const before = snapshot();
    const r = spawnSync(process.execPath, [CLI, "sync", "--apply", "--as", "sub-a", "--request-id", "req-kill-0001",
      "--payload-sha", nodePayload("sub-a"), "--root", root, "--local"], { encoding: "utf8", env: { ...process.env, WIKI_DESK_FAULT_KILL_AT: "5" } });
    assert.notEqual(r.status, 0, "결함 주입이 발화하지 않았다");
    assert.ok(fs.existsSync(path.join(root, "__llm-wiki", ".journal", "txn.json")), "kill 뒤 저널이 남아야 한다(복구 근거)");
    // 공유 잠금 읽기가 저널을 만나면 배타로 승격해 복구한 뒤 읽는다
    const st = cli("status");
    assert.equal(st.status, 0, st.stderr);
    const status = JSON.parse(st.stdout);
    assert.ok(status.journal_recovered && status.journal_recovered.rolled_back.length > 0, JSON.stringify(status));
    assert.deepEqual(snapshot(), before, "복구 뒤 위키가 직전 커밋 상태와 다르다");
    assert.ok(!fs.existsSync(path.join(root, "__llm-wiki", ".journal")), "저널 잔재");
    const debris = fs.readdirSync(path.join(root, "__llm-wiki"), { recursive: true }).map(String).filter((f) => path.basename(f).startsWith(".wiki-desk-txn-"));
    assert.deepEqual(debris, [], "임시파일 잔재");
    // 다음 입고는 남은 잠금 없이 바로 성공한다
    const next = cli("sync", "--apply", "--as", "sub-a", "--request-id", "req-kill-0002", "--payload-sha", nodePayload("sub-a"));
    assert.equal(next.status, 0, next.stderr);
    assert.equal(JSON.parse(cli("check").stdout).ok, false); // sub-b·sub-c 는 아직 입고 전이다(card.missing) — 정상
  });
});

describe("게이트웨이 허용 목록", () => {
  const exitOf = (fn: () => unknown): number | string => {
    const orig = process.exit;
    let code: number | string = "returned";
    (process as { exit: unknown }).exit = (c: number) => { code = c; throw new Error("exit"); };
    const origErr = process.stderr.write.bind(process.stderr);
    (process.stderr as { write: unknown }).write = () => true;
    try { fn(); } catch { /* exit 신호 */ } finally { (process as { exit: unknown }).exit = orig; (process.stderr as { write: unknown }).write = origErr; }
    return code;
  };
  test("쓰기는 키의 node_id 로 못박힌다", () => {
    const argv = vet(["sync", "--apply", "--request-id", "req-gw-00001", "--payload-sha", "a".repeat(64)], "sub-a", "mirror");
    assert.deepEqual(argv.slice(argv.indexOf("--as"), argv.indexOf("--as") + 2), ["--as", "sub-a"]);
    assert.ok(argv.includes("--local"));
  });
  test("목록 밖 명령·플래그·남의 미러 수용 지정·멱등 키 없는 입고는 거부", () => {
    assert.equal(exitOf(() => vet(["remove", "--apply"], "sub-a", "mirror")), 126);
    assert.equal(exitOf(() => vet(["sync", "--apply", "--as", "main", "--request-id", "x".repeat(10)], "sub-a", "mirror")), 126);
    assert.equal(exitOf(() => vet(["sync", "--apply", "--request-id", "x".repeat(10), "--accept-removed", "mirror/sub-b/docs/x.md"], "sub-a", "mirror")), 126);
    assert.equal(exitOf(() => vet(["sync", "--apply"], "sub-a", "mirror")), 126);
    assert.equal(exitOf(() => vet(["query", "--root", "/etc"], "sub-a", "mirror")), 126);
  });
});
