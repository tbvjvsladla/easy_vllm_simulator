// fs-safety 행동 사례: Python fs_safety.py · test_lifecycle.py(FilesystemSafetyTests) 이식.
import { test, describe, beforeEach, afterEach } from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import crypto from "node:crypto";
import { execFileSync } from "node:child_process";
import {
  safeRelative,
  resolveInside,
  readRegular,
  sha256,
  isBelow,
  Transaction,
} from "../src/fs-safety.ts";
import type { Plan } from "../src/fs-safety.ts";

const PERMISSION_BITS = 0o777;
const TEMP_PREFIX = ".wiki-desk-txn-";

let home = "";
let root = "";

beforeEach(() => {
  home = fs.mkdtempSync(path.join(os.tmpdir(), "wiki-desk-fs-"));
  root = path.join(home, "project");
  fs.mkdirSync(root);
});
afterEach(() => {
  fs.rmSync(home, { recursive: true, force: true });
});

function put(rel: string, data: string | Buffer = "contents\n", mode = 0o644): string {
  const p = path.join(root, rel);
  fs.mkdirSync(path.dirname(p), { recursive: true });
  fs.writeFileSync(p, data);
  fs.chmodSync(p, mode);
  return p;
}

/** 심링크를 따르지 않는 독립 관찰자(타입·권한·해시·자식 목록) */
function tree(dir: string): string {
  const out: string[] = [];
  const visit = (p: string): void => {
    const st = fs.lstatSync(p);
    const rel = path.relative(dir, p) || ".";
    const mode = (st.mode & PERMISSION_BITS).toString(8);
    if (st.isSymbolicLink()) out.push(`${rel} link ${fs.readlinkSync(p)}`);
    else if (st.isDirectory()) {
      const kids = fs.readdirSync(p).sort();
      out.push(`${rel} dir ${mode} [${kids.join(",")}]`);
      for (const k of kids) visit(path.join(p, k));
    } else if (st.isFile()) {
      const h = crypto.createHash("sha256").update(fs.readFileSync(p)).digest("hex");
      out.push(`${rel} file ${mode} ${h}`);
    } else out.push(`${rel} special ${mode}`);
  };
  visit(dir);
  return out.join("\n");
}

function refusedUnchanged(fn: () => unknown, message: RegExp): void {
  const before = tree(root);
  assert.throws(fn, message);
  assert.equal(tree(root), before);
}

function modeOf(rel: string): number {
  return fs.statSync(path.join(root, rel)).mode & PERMISSION_BITS;
}

describe("safeRelative", () => {
  test("탈출·비정규 경로를 거부한다", () => {
    const bad = ["", ".", "..", "../outside", "a/../../b", "/etc/passwd", "a//b", "a/./b",
      "a/", "a\\b", "C:/escape", "nul\x00name", "tab\tname", "del\x7fname"];
    for (const rel of bad) assert.throws(() => safeRelative(rel), /안전하지 않은 상대경로/, rel);
  });
  test("정상 경로(한글·공백 포함)는 그대로 반환한다", () => {
    assert.equal(safeRelative("한국어/한 문서.md"), "한국어/한 문서.md");
    assert.equal(safeRelative("docs/a.md"), "docs/a.md");
  });
});

describe("resolveInside", () => {
  test("루트 안 경로(미존재 포함)를 절대경로로 반환한다", () => {
    put("docs/a.md");
    assert.equal(resolveInside(root, "docs/a.md"), path.join(root, "docs/a.md"));
    assert.equal(resolveInside(root, "new/deep/x.md"), path.join(root, "new/deep/x.md"));
  });
  test("루트 자체 검증: 심링크·'..'·파일시스템 루트·미존재 거부", () => {
    fs.symlinkSync(root, path.join(home, "alias"));
    assert.throws(() => resolveInside(path.join(home, "alias"), "a.md"), /심볼릭링크/);
    assert.throws(() => resolveInside(path.join(home, "alias", "nested"), "a.md"), /심볼릭링크|존재/);
    assert.throws(() => resolveInside(`${root}/../project`, "a.md"), /'\.\.'/);
    assert.throws(() => resolveInside("/", "a.md"), /파일시스템 루트/);
    assert.throws(() => resolveInside(path.join(home, "absent"), "a.md"), /존재/);
  });
  test("디렉터리·파일 심링크와 그 경유를 거부하고 외부를 건드리지 않는다", () => {
    const outside = path.join(home, "outside");
    fs.mkdirSync(outside);
    put("plain.md");
    fs.symlinkSync(outside, path.join(root, "directory-link"));
    fs.symlinkSync(path.join(root, "plain.md"), path.join(root, "file-link"));
    for (const rel of ["directory-link", "directory-link/new.md", "file-link"]) {
      refusedUnchanged(() => resolveInside(root, rel), /심볼릭링크/);
    }
    assert.deepEqual(fs.readdirSync(outside), []);
  });
  test("특수 파일·파일 부모 충돌·탈출을 거부한다", () => {
    execFileSync("mkfifo", [path.join(root, "pipe")]);
    refusedUnchanged(() => resolveInside(root, "pipe"), /특수 파일/);
    put("file-parent");
    refusedUnchanged(() => resolveInside(root, "file-parent/child"), /디렉터리가 아닌 상위/);
    refusedUnchanged(() => resolveInside(root, "../escape"), /안전하지 않은 상대경로/);
  });
});

describe("readRegular · sha256", () => {
  test("일반 파일 바이트를 읽는다", () => {
    put("docs/a.bin", Buffer.from([0, 255, 1]));
    assert.deepEqual(readRegular(root, "docs/a.bin"), Buffer.from([0, 255, 1]));
  });
  test("디렉터리·심링크·미존재를 거부한다", () => {
    put("docs/a.md");
    fs.symlinkSync(path.join(root, "docs/a.md"), path.join(root, "link.md"));
    assert.throws(() => readRegular(root, "docs"), /일반 파일/);
    assert.throws(() => readRegular(root, "link.md"), /심볼릭링크/);
    assert.throws(() => readRegular(root, "docs/missing.md"), /ENOENT/);
  });
  test("sha256 알려진 값", () => {
    const abc = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
    assert.equal(sha256("abc"), abc);
    assert.equal(sha256(Buffer.from("abc")), abc);
  });
});

describe("Transaction.plan — 변경 전 전수 거부", () => {
  test("탈출·중복·조상 충돌·디렉터리 대상·범위 밖·잘못된 mode·미존재 삭제", () => {
    put("owned/a.md", "before");
    const tx = new Transaction(root, ["owned", "fresh"]);
    const ok = { rel: "owned/a.md", data: Buffer.from("after") };
    const cases: [unknown[], RegExp][] = [
      [[ok, { rel: "../escape", data: Buffer.from("x") }], /안전하지 않은 상대경로/],
      [[ok, { rel: "owned", data: Buffer.from("x") }], /조상|디렉터리/],
      [[ok, { rel: "owned/a.md", data: Buffer.from("dup") }], /중복/],
      [[ok, { rel: "unguarded.md", data: Buffer.from("x") }], /범위 밖/],
      [[{ rel: "fresh/x", data: Buffer.from("x") }, { rel: "fresh/x/y", data: Buffer.from("y") }], /조상/],
      [[{ rel: "owned/b.md", data: Buffer.from("x"), mode: 0o4755 }], /mode/],
      [[{ rel: "owned/b.md", data: Buffer.from("x"), mode: 1.5 }], /mode/],
      [[{ rel: "owned/missing.md", data: null }], /삭제 대상/],
    ];
    for (const [writes, message] of cases) {
      refusedUnchanged(() => tx.plan(writes as never), message);
    }
    fs.mkdirSync(path.join(root, "owned/dir"));
    refusedUnchanged(() => tx.plan([{ rel: "owned/dir", data: Buffer.from("x") }]), /디렉터리/);
  });
  test("범위 자체도 안전 경로여야 한다", () => {
    assert.throws(() => new Transaction(root, ["../up"]), /안전하지 않은 상대경로/);
  });
  test("plan은 쓰기 0건이고 preimage·지문을 담는다", () => {
    put("owned/a.md", "before", 0o600);
    const tx = new Transaction(root, ["owned"]);
    const before = tree(root);
    const plan = tx.plan([{ rel: "owned/a.md", data: Buffer.from("after") }, { rel: "owned/new.md", data: Buffer.from("n") }]);
    assert.equal(tree(root), before);
    assert.match(plan.fingerprint, /^[0-9a-f]{64}$/);
    assert.deepEqual(plan.preimages["owned/a.md"], { kind: "file", sha256: sha256("before"), mode: 0o600, size: 6 });
    assert.deepEqual(plan.preimages["owned/new.md"], { kind: "missing" });
  });
});

describe("Transaction.apply — 원자 쓰기·mode", () => {
  test("새 하위 디렉터리를 만들고 쓰고, 삭제하고, 재실행은 무변경", () => {
    put("owned/existing.md", "old");
    put("owned/gone.md", "bye");
    const tx = new Transaction(root, ["owned"]);
    const writes = [
      { rel: "owned/new/nested.md", data: Buffer.from("new") },
      { rel: "owned/existing.md", data: Buffer.from("updated") },
      { rel: "owned/gone.md", data: null },
    ];
    const result = tx.apply(tx.plan(writes));
    assert.deepEqual(result, { mutations: 4, writes: 2, deletes: 1, createdDirs: 1 });
    assert.equal(fs.readFileSync(path.join(root, "owned/new/nested.md"), "utf8"), "new");
    assert.equal(fs.readFileSync(path.join(root, "owned/existing.md"), "utf8"), "updated");
    assert.equal(fs.existsSync(path.join(root, "owned/gone.md")), false);
    const again = tx.apply(tx.plan(writes.slice(0, 2)));
    assert.equal(again.mutations, 0);
  });
  test("mode 미지정 시 기존 mode 보존, 지정 시 umask와 무관하게 정확히 적용", () => {
    put("owned/keep.md", "k", 0o600);
    const tx = new Transaction(root, ["owned"]);
    const previous = process.umask(0o077);
    tx.apply(tx.plan([
      { rel: "owned/keep.md", data: Buffer.from("k2") },
      { rel: "owned/exec.sh", data: Buffer.from("#!"), mode: 0o755 },
      { rel: "owned/fresh.md", data: Buffer.from("f") },
    ]));
    process.umask(previous);
    assert.equal(modeOf("owned/keep.md"), 0o600);
    assert.equal(modeOf("owned/exec.sh"), 0o755);
    assert.equal(modeOf("owned/fresh.md"), 0o644);
  });
  test("성공 후 임시파일이 남지 않는다", () => {
    const tx = new Transaction(root, ["owned"]);
    tx.apply(tx.plan([{ rel: "owned/a.md", data: Buffer.from("a") }]));
    assert.deepEqual(fs.readdirSync(path.join(root, "owned")), ["a.md"]);
  });
});

describe("Transaction.apply — preimage 경합 감지(변경 전 거부)", () => {
  test("대상 내용·mode 변경, 미존재였던 경로 생성 시 거부", () => {
    put("owned/a.md", "before");
    const tx = new Transaction(root, ["owned", "fresh"]);
    const plan = tx.plan([{ rel: "owned/a.md", data: Buffer.from("after") }, { rel: "fresh/x.md", data: Buffer.from("new") }]);
    put("owned/a.md", "user edit");
    refusedUnchanged(() => tx.apply(plan), /경합/);
    put("owned/a.md", "before", 0o600);
    refusedUnchanged(() => tx.apply(plan), /경합/);
    put("owned/a.md", "before", 0o644);
    put("fresh/x.md", "someone else");
    refusedUnchanged(() => tx.apply(plan), /경합/);
  });
  test("계획 후 상위가 심링크로 바뀌면 거부하고 외부를 건드리지 않는다", () => {
    const outside = path.join(home, "outside");
    fs.mkdirSync(outside);
    fs.mkdirSync(path.join(root, "owned"));
    const tx = new Transaction(root, ["owned"]);
    const plan = tx.plan([{ rel: "owned/a.md", data: Buffer.from("x") }]);
    fs.rmdirSync(path.join(root, "owned"));
    fs.symlinkSync(outside, path.join(root, "owned"));
    refusedUnchanged(() => tx.apply(plan), /심볼릭링크/);
    assert.deepEqual(fs.readdirSync(outside), []);
  });
  test("발행되지 않은(직렬화·복제) 계획과 변조된 계획을 거부한다", () => {
    put("owned/a.md", "before");
    const tx = new Transaction(root, ["owned"]);
    const plan = tx.plan([{ rel: "owned/a.md", data: Buffer.from("after") }]);
    const forged = { ...plan } as Plan;
    refusedUnchanged(() => tx.apply(forged), /발행/);
    refusedUnchanged(() => new Transaction(root, ["owned"]).apply(plan), /발행/);
    (plan.writes[0]!.data as Buffer).write("AFTER");
    refusedUnchanged(() => tx.apply(plan), /변조/);
  });
});

describe("Transaction.apply — 실패 시 전체 롤백", () => {
  test("모든 변경 지점마다 예외 주입 → 원래 트리로 정확히 복원, 원 예외 재던짐", () => {
    const seed = (): void => {
      fs.rmSync(root, { recursive: true, force: true });
      fs.mkdirSync(root);
      put("owned/a.bin", Buffer.from([0, 255, 0x6f]), 0o600);
      put("owned/gone.md", "delete", 0o640);
      put("owned/keep.md", "same", 0o644);
    };
    const writes = [
      { rel: "owned/a.bin", data: Buffer.from([255, 1]), mode: 0o755 },
      { rel: "new/nested/b.md", data: Buffer.from("fresh") },
      { rel: "owned/keep.md", data: Buffer.from("same") },
      { rel: "owned/gone.md", data: null },
    ];
    seed();
    const initial = tree(root);
    const seen: string[] = [];
    const tx0 = new Transaction(root, ["owned", "new"]);
    const { mutations } = tx0.apply(tx0.plan(writes), { fault: (n, rel) => { seen.push(`${n}:${rel}`); } });
    assert.equal(mutations, 5); // new · new/nested · b.md · a.bin · gone.md (keep.md는 동일 → 생략)
    assert.equal(seen.length, mutations);
    for (let failing = 1; failing <= mutations; failing++) {
      seed();
      const tx = new Transaction(root, ["owned", "new"]);
      const plan = tx.plan(writes);
      const injected = new Error(`injected after ${failing}`);
      assert.throws(
        () => tx.apply(plan, { fault: (n) => { if (n === failing) throw injected; } }),
        (err: unknown) => err === injected,
      );
      assert.equal(tree(root), initial, `failing=${failing}`);
      assert.equal(fs.readdirSync(path.join(root, "owned")).some((n) => n.startsWith(TEMP_PREFIX)), false);
    }
  });
});

describe("isBelow (cs-7: contract.ts와 공유)", () => {
  test("자신·하위는 참, 이름 접두만 같은 형제·상위는 거짓", () => {
    assert.equal(isBelow("cards", "cards"), true);
    assert.equal(isBelow("cards/a/b.md", "cards"), true);
    assert.equal(isBelow("cards-cache", "cards"), false);
    assert.equal(isBelow("cardsx/a.md", "cards"), false);
    assert.equal(isBelow("cards", "cards/a"), false);
  });
});
