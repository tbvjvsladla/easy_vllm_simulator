// contract 행동 사례: Python wiki_runtime.validate_contract · _authority · _paths 이식.
import { test, describe, beforeEach, afterEach } from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { findContract, validateContract, authorityFor, listSources, WIKI_DIRS, CONTRACT_FILE, RESERVED_WIKI_FILES } from "../src/contract.ts";
import type { Contract } from "../src/types.ts";

const FIXTURE = path.join(import.meta.dirname, "fixtures", "contract", "valid.json");
const valid = (): Record<string, unknown> => JSON.parse(fs.readFileSync(FIXTURE, "utf8"));

let root = "";
beforeEach(() => {
  root = fs.realpathSync(fs.mkdtempSync(path.join(os.tmpdir(), "wiki-desk-contract-")));
});
afterEach(() => {
  fs.rmSync(root, { recursive: true, force: true });
});

function put(rel: string, data = "contents\n"): void {
  const p = path.join(root, rel);
  fs.mkdirSync(path.dirname(p), { recursive: true });
  fs.writeFileSync(p, data);
}

function withChange(mutate: (c: Record<string, any>) => void): Record<string, unknown> {
  const c = valid() as Record<string, any>;
  mutate(c);
  return c;
}

describe("validateContract", () => {
  test("유효 계약을 통과시키고 확장 키를 보존하며 원본과 분리된 사본을 반환한다", () => {
    const raw = withChange((c) => { c.x_extra = { nested: [1, 2] }; });
    const contract = validateContract(raw);
    assert.equal(contract.project_name, "t");
    assert.deepEqual(contract["x_extra"], { nested: [1, 2] });
    (raw.source_roots as string[]).push("other");
    assert.deepEqual(contract.source_roots, ["docs"]);
  });

  test("필수 필드 누락을 거부한다(기본값 채움 없음)", () => {
    const required = ["schema_version", "project_name", "wiki_dir", "source_roots", "excluded_roots",
      "source_suffixes", "authority_rules", "fallback", "copy_policy", "frontmatter_aliases",
      "filename_pattern", "layout", "search"];
    for (const key of required) {
      assert.throws(() => validateContract(withChange((c) => { delete c[key]; })), new RegExp(key), key);
    }
    assert.throws(() => validateContract(null), /객체/);
    assert.throws(() => validateContract([]), /객체/);
  });

  test("원 스키마 필드 값 위반을 거부한다", () => {
    const bad: [string, (c: Record<string, any>) => void][] = [
      ["schema_version", (c) => { c.schema_version = 2; }],
      ["schema_version", (c) => { c.schema_version = "1"; }],
      ["project_name", (c) => { c.project_name = "  "; }],
      ["wiki_dir", (c) => { c.wiki_dir = "wiki"; }],
      ["copy_policy", (c) => { c.copy_policy = "copy"; }],
      ["source_roots", (c) => { c.source_roots = []; }],
      ["source_roots", (c) => { c.source_roots = ["docs", "docs"]; }],
      ["source_roots", (c) => { c.source_roots = ["../up"]; }],
      ["source_roots", (c) => { c.source_roots = ["/abs"]; }],
      ["source_roots", (c) => { c.source_roots = ["docs "]; }],
      ["source_roots", (c) => { c.source_roots = ["C:x"]; }],
      ["source_roots", (c) => { c.source_roots = ["__llm-wiki/x"]; }],
      ["source_roots", (c) => { c.source_roots = ["docs/secrets"]; }],
      ["excluded_roots", (c) => { c.excluded_roots = ["."]; }],
      ["excluded_roots", (c) => { c.excluded_roots = "docs"; }],
      ["source_suffixes", (c) => { c.source_suffixes = []; }],
      ["source_suffixes", (c) => { c.source_suffixes = ["md"]; }],
      ["source_suffixes", (c) => { c.source_suffixes = [".md", ".MD"]; }],
      ["authority_rules", (c) => { c.authority_rules = {}; }],
      ["authority_rules\\[0\\].pattern", (c) => { c.authority_rules[0].pattern = "/docs/**"; }],
      ["authority_rules\\[0\\].pattern", (c) => { c.authority_rules[0].pattern = "docs/../x"; }],
      ["authority_rules\\[0\\].pattern", (c) => { c.authority_rules[0].pattern = "docs\\x"; }],
      ["authority_rules\\[0\\].authority_rank", (c) => { c.authority_rules[0].authority_rank = 1.5; }],
      ["authority_rules\\[0\\].role", (c) => { c.authority_rules[0].role = ""; }],
      ["fallback.document_type", (c) => { delete c.fallback.document_type; }],
    ];
    for (const [label, mutate] of bad) {
      assert.throws(() => validateContract(withChange(mutate)), new RegExp(label), label);
    }
  });

  test("TS 확장 필드 위반을 거부한다", () => {
    const bad: [string, (c: Record<string, any>) => void][] = [
      ["frontmatter_aliases", (c) => { c.frontmatter_aliases = { related_plan: "plan" }; }],
      ["frontmatter_aliases", (c) => { c.frontmatter_aliases = { related_plan: [""] }; }],
      ["filename_pattern", (c) => { c.filename_pattern = ""; }],
      ["filename_pattern", (c) => { c.filename_pattern = "(unclosed"; }],
      ["filename_pattern", (c) => { c.filename_pattern = "^(?<prefix>[a-z]+)_(?<stamp>\\d{8})_(?<title>.+)$"; }],
      ["layout.cards_dir", (c) => { c.layout.cards_dir = "../out"; }],
      ["layout.cache_dir", (c) => { delete c.layout.cache_dir; }],
      ["layout.archive_dir", (c) => { delete c.layout.archive_dir; }],
      ["search.bm25_k1", (c) => { c.search.bm25_k1 = 0; }],
      ["search.bm25_k1", (c) => { c.search.bm25_k1 = Infinity; }],
      ["search.bm25_b", (c) => { c.search.bm25_b = 1.5; }],
      ["search.bm25_b", (c) => { c.search.bm25_b = "0.75"; }],
      ["search.field_boost.title", (c) => { c.search.field_boost.title = -1; }],
      ["search.field_boost.body", (c) => { delete c.search.field_boost.body; }],
      ["search.authority_weight", (c) => { c.search.authority_weight = Number.NaN; }],
      ["search.span_max_lines", (c) => { c.search.span_max_lines = 2.5; }],
      ["search.result_limit", (c) => { c.search.result_limit = 0; }],
      ["search.spans_per_result", (c) => { c.search.spans_per_result = -2; }],
    ];
    for (const [label, mutate] of bad) {
      assert.throws(() => validateContract(withChange(mutate)), new RegExp(label), label);
    }
  });
});

describe("layout 불변식 (TDA-1, CR-8)", () => {
  test("세 디렉터리가 같거나 포함 관계면 거부한다", () => {
    const bad: [string, string, string][] = [
      ["cards", "cards", "archive"],
      ["cards", ".cache", "cards"],
      ["cards", "cards/.cache", "archive"],
      ["store/cards", ".cache", "store"],
      ["cards", ".cache", ".cache/old"],
    ];
    for (const [cards, cache, archive] of bad) {
      const raw = withChange((c) => { c.layout = { cards_dir: cards, cache_dir: cache, archive_dir: archive }; });
      assert.throws(() => validateContract(raw), /계약 layout: .*겹친다/, `${cards}|${cache}|${archive}`);
    }
  });
  test("이름 접두만 같은 형제 디렉터리는 허용한다", () => {
    const raw = withChange((c) => { c.layout = { cards_dir: "cards", cache_dir: "cards-cache", archive_dir: "cards_archive" }; });
    assert.deepEqual(validateContract(raw).layout, { cards_dir: "cards", cache_dir: "cards-cache", archive_dir: "cards_archive" });
  });
  test("위키 루트 예약 파일명과 같은 이름을 거부한다", () => {
    assert.deepEqual([...RESERVED_WIKI_FILES].sort(), [".gitignore", "contract.json", "index.md", "log.md", "receipt.json"]);
    for (const name of RESERVED_WIKI_FILES) {
      for (const key of ["cards_dir", "cache_dir", "archive_dir"]) {
        const raw = withChange((c) => { c.layout[key] = name; });
        assert.throws(() => validateContract(raw), new RegExp(`layout\\.${key}: 예약 파일명`), `${key}=${name}`);
        const nested = withChange((c) => { c.layout[key] = `${name}/sub`; });
        assert.throws(() => validateContract(nested), new RegExp(`layout\\.${key}: 예약 파일명`), `${key}=${name}/sub`);
      }
    }
  });
});

describe("frontmatter_aliases 로드 시 검증 (TDA-2)", () => {
  test("두 정본 키가 같은 별칭을 쓰면 계약 오류", () => {
    const raw = withChange((c) => { c.frontmatter_aliases = { related_plan: ["plan"], source_plan: ["plan"] }; });
    assert.throws(() => validateContract(raw), /계약 frontmatter_aliases/);
  });
  test("서로 다른 별칭은 통과한다", () => {
    const raw = withChange((c) => { c.frontmatter_aliases = { related_plan: ["plan"], source_plan: ["origin"] }; });
    assert.deepEqual(validateContract(raw).frontmatter_aliases, { related_plan: ["plan"], source_plan: ["origin"] });
  });
});

describe("공유 상수 (CR-9, cs-9)", () => {
  test("WIKI_DIRS·CONTRACT_FILE은 두 리터럴과 계약 파일명을 노출한다", () => {
    assert.deepEqual([...WIKI_DIRS], ["__llm-wiki", "__llm_wiki"]);
    assert.equal(CONTRACT_FILE, "contract.json");
  });
  test("authority pattern은 fs-safety 경로 규칙을 따른다(빈 segment·'.' segment·제어문자 거부)", () => {
    for (const p of ["docs//x.md", "./docs/**", "docs/./x", "docs/**/", "docs/\u0007x", " docs/**"]) {
      const raw = withChange((c) => { c.authority_rules[0].pattern = p; });
      assert.throws(() => validateContract(raw), /authority_rules\[0\]\.pattern/, JSON.stringify(p));
    }
    const ok = withChange((c) => { c.authority_rules[0].pattern = "**/README.md"; });
    assert.equal(validateContract(ok).authority_rules[0]!.pattern, "**/README.md");
  });
});

describe("findContract", () => {
  test("정확히 하나의 계약을 찾아 검증한다", () => {
    put("__llm-wiki/contract.json", JSON.stringify(valid()));
    const found = findContract(root);
    assert.equal(found.wikiDir, "__llm-wiki");
    assert.equal(found.contract.wiki_dir, "__llm-wiki");
  });
  test("밑줄 표기 폴더도 리터럴 그대로 인정한다", () => {
    put("__llm_wiki/contract.json", JSON.stringify(withChange((c) => { c.wiki_dir = "__llm_wiki"; })));
    assert.equal(findContract(root).wikiDir, "__llm_wiki");
  });
  test("0개·2개 모두 거부한다", () => {
    assert.throws(() => findContract(root), /정확히 하나/);
    put("__llm-wiki/contract.json", JSON.stringify(valid()));
    put("__llm_wiki/contract.json", JSON.stringify(withChange((c) => { c.wiki_dir = "__llm_wiki"; })));
    assert.throws(() => findContract(root), /정확히 하나/);
  });
  test("폴더명과 wiki_dir이 다르면 거부한다(암묵 교정 없음)", () => {
    put("__llm_wiki/contract.json", JSON.stringify(valid()));
    assert.throws(() => findContract(root), /wiki_dir/);
  });
  test("심링크 계약 파일을 거부한다", () => {
    put("elsewhere.json", JSON.stringify(valid()));
    fs.mkdirSync(path.join(root, "__llm-wiki"));
    fs.symlinkSync(path.join(root, "elsewhere.json"), path.join(root, "__llm-wiki/contract.json"));
    assert.throws(() => findContract(root), /심볼릭링크/);
  });
});

describe("authorityFor", () => {
  const contract = (): Contract => validateContract(withChange((c) => {
    c.authority_rules = [
      { pattern: "docs/devlog/**", document_type: "devlog", authority_rank: 60, role: "result-evidence" },
      { pattern: "docs/*.md", document_type: "top", authority_rank: 10, role: "top" },
      { pattern: "**/README.md", document_type: "readme", authority_rank: 5, role: "readme" },
      { pattern: "docs/plan/plan_*_설계.md", document_type: "design", authority_rank: 50, role: "intent", extra: 1 },
      { pattern: "docs/**", document_type: "any", authority_rank: 1, role: "any" },
    ];
  }));
  test("첫 일치 규칙을 고르고 3개 키만 반환한다", () => {
    const c = contract();
    assert.deepEqual(authorityFor(c, "docs/devlog/a.md"), { document_type: "devlog", authority_rank: 60, role: "result-evidence" });
    assert.equal(authorityFor(c, "docs/devlog/deep/b.md").document_type, "devlog");
    assert.equal(authorityFor(c, "docs/top.md").document_type, "top");
    assert.deepEqual(authorityFor(c, "docs/plan/plan_26101409_위키_설계.md"), { document_type: "design", authority_rank: 50, role: "intent" });
  });
  test("'*'는 '/'를 넘지 않고 '**/'는 0개 이상 디렉터리에 일치한다", () => {
    const c = contract();
    assert.equal(authorityFor(c, "docs/sub/x.md").document_type, "any");
    assert.equal(authorityFor(c, "README.md").document_type, "readme");
    assert.equal(authorityFor(c, "a/b/README.md").document_type, "readme");
    assert.equal(authorityFor(c, "docs/plan/plan_x_설계.mdx").document_type, "any");
  });
  test("정규식 메타문자는 리터럴로 취급하고, 불일치는 fallback", () => {
    const c = validateContract(withChange((c) => {
      c.authority_rules = [{ pattern: "docs/a+b(1).md", document_type: "lit", authority_rank: 1, role: "r" }];
    }));
    assert.equal(authorityFor(c, "docs/a+b(1).md").document_type, "lit");
    assert.deepEqual(authorityFor(c, "docs/aab1.md"), { document_type: "reference", authority_rank: 0, role: "reference" });
  });
});

describe("listSources", () => {
  test("source_roots 하위·suffix(대소문자 무시)·제외·위키·비밀 경로를 반영한 정렬 목록", () => {
    const contract = validateContract(withChange((c) => {
      c.source_roots = ["."];
      c.excluded_roots = ["docs/private"];
    }));
    for (const rel of ["docs/b.md", "docs/a.MD", "docs/sub/c.md", "docs/data.txt", "top.md",
      "docs/private/x.md", "__llm-wiki/cards/card.md", "docs/secrets/password.md",
      "docs/generated/hidden.md", "docs/.cache/hidden.md", ".git/HEAD.md", "node_modules/p/readme.md"]) put(rel);
    put("__llm-wiki/contract.json", "{}");
    assert.deepEqual(listSources(root, contract), ["docs/a.MD", "docs/b.md", "docs/sub/c.md", "top.md"]);
  });
  test("파일 root·다중 root는 중복 없이 합친다", () => {
    put("docs/a.md");
    put("notes/n.md");
    put("notes/skip.txt");
    const contract = validateContract(withChange((c) => { c.source_roots = ["docs", "docs/a.md", "notes"]; }));
    assert.deepEqual(listSources(root, contract), ["docs/a.md", "notes/n.md"]);
  });
  test("심링크는 제외 경로여도 거부, 없는 root는 자연 raise", () => {
    put("docs/a.md");
    fs.symlinkSync(path.join(root, "docs/a.md"), path.join(root, "docs/link.md"));
    const contract = validateContract(valid());
    assert.throws(() => listSources(root, contract), /심볼릭링크/);
    fs.unlinkSync(path.join(root, "docs/link.md"));
    fs.mkdirSync(path.join(root, "docs/private"));
    fs.symlinkSync(root, path.join(root, "docs/private/loop"));
    const excluding = validateContract(withChange((c) => { c.excluded_roots = ["docs/private"]; }));
    assert.deepEqual(listSources(root, excluding), ["docs/a.md"]);
    fs.symlinkSync(root, path.join(root, "docs/loop"));
    assert.throws(() => listSources(root, excluding), /심볼릭링크/);
    const missing = validateContract(withChange((c) => { c.source_roots = ["nope"]; }));
    assert.throws(() => listSources(root, missing), /ENOENT/);
  });
});

test("layout: 대소문자만 다른 디렉터리·예약 파일명도 겹침으로 거부", () => {
  const base = JSON.parse(fs.readFileSync(FIXTURE, "utf8"));
  assert.throws(() => validateContract({ ...base, layout: { cards_dir: "Cards", cache_dir: ".cache", archive_dir: "cards" } }), /겹친다/);
  assert.throws(() => validateContract({ ...base, layout: { cards_dir: "cards", cache_dir: "Index.md", archive_dir: "archive" } }), /예약 파일명/);
});
