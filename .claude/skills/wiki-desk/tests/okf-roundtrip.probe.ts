// 1회성 증빙 스크립트(node --test 대상 아님): OKF 공식 예제 전부를 왕복해 무손실을 확인한다.
// 사용: node tests/okf-roundtrip.probe.ts <knowledge-catalog/okf 경로>
import fs from "node:fs";
import path from "node:path";
import assert from "node:assert/strict";
import { splitFrontmatter, bodyText } from "../src/frontmatter.ts";
import { roundTrip, checkConcept } from "../src/okf.ts";

const base = process.argv[2];
const files: string[] = [];
const walk = (d: string): void => {
  for (const e of fs.readdirSync(d, { withFileTypes: true })) {
    const p = path.join(d, e.name);
    if (e.isDirectory()) walk(p);
    else if (e.name.endsWith(".md")) files.push(p);
  }
};
for (const sub of ["bundles", "samples"]) walk(path.join(base, sub));
let ok = 0;
let withFm = 0;
const failures: string[] = [];
const okfIssues: string[] = [];
for (const f of files.sort()) {
  const text = fs.readFileSync(f, "utf8");
  const before = splitFrontmatter(text);
  const out = roundTrip(text);
  const after = splitFrontmatter(out);
  if (before.kind !== "none") withFm++;
  assert.notEqual(before.kind, "error", `파싱 실패: ${f}`);
  const same =
    before.kind === "ok" && after.kind === "ok"
      ? JSON.stringify(after.data) === JSON.stringify(before.data) && bodyText(out, after) === bodyText(text, before)
      : out === text;
  if (same) ok++;
  else failures.push(path.relative(base, f));
  const rel = path.relative(path.join(base, f.includes("/bundles/") ? "bundles" : "samples"), f).split(path.sep).slice(1).join("/");
  for (const i of checkConcept(text, rel)) okfIssues.push(`${path.relative(base, f)}: ${i.code}`);
}
console.log(JSON.stringify({ total: files.length, with_frontmatter: withFm, roundtrip_ok: ok, failures, okf_issues: okfIssues }, null, 2));
process.exitCode = failures.length === 0 ? 0 : 1;
