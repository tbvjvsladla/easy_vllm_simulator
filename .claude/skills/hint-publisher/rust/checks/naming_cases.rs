//! `naming` checks (owner: the naming port). Byte parity with the Python original is the `hp_naming` golden stage;
//! these are the Rust-side facts: the ported selftest and a few focused contracts other modules rely on.
use hintlib::naming::{self as n, _fixture_vocab, _tag2_facts};
use hintlib::value::J;
use hintlib::{jd, jl};

const T2: &str = "hint/0.29.0rc6/qwen3.8-flash-next-nvfp4/gb10-1g2n-cluster-native/qnvfp4-len262144-kvauto";

#[test]
fn selftest_is_green() {
    assert_eq!(n::selftest(), Vec::<String>::new());
}

#[test]
fn parse_tag_v6_v7_legacy() {
    let t = n::parse_tag(&format!("{T2}-mmap-spec3-eager-t2609290823")).unwrap();
    assert_eq!(t.grammar, n::GRAMMAR_V7);
    assert_eq!(t.tail, ["mmap", "spec3", "eager"]);
    assert_eq!(t.timestamp.as_deref(), Some("t2609290823"));
    assert_eq!(t.base_tag().as_deref(), Some(T2));
    assert_eq!(t.tag(), format!("{T2}-mmap-spec3-eager-t2609290823"));
    let v6 = n::parse_tag(n::_TAG2_V6).unwrap();
    assert_eq!(v6.grammar, "v6");
    let old = n::parse_tag("hint/0.18.0/gpt-oss-20b/gb10-sim-h100/qmxfp4-len131072-kvfp8").unwrap();
    assert_eq!(old.grammar, "legacy-5seg");
    assert!(old.base_tag().is_none() && old.tail.is_empty());
    assert_eq!(n::parse_tag("hint/a/b").unwrap_err().code, "HINT_NAME_SHAPE");
    assert_eq!(n::base_name(&format!("{T2}-mmap")).unwrap(), T2);
}

#[test]
fn derive_compose_roundtrip() {
    let v = _fixture_vocab();
    let dn = n::derive_name(&_tag2_facts(), &v).unwrap();
    assert_eq!(dn.tag, T2);
    let order: Vec<&str> = dn.axes.iter().map(|(k, _)| k.as_str()).collect();
    assert_eq!(order, ["hw", "gpus_per_node", "nodes", "role", "target", "plane", "q", "len", "kv"]);
    assert_eq!(dn.axis("plane").unwrap().token.as_deref(), Some(""));
    let pay = dn.to_payload();
    assert_eq!(n::compose_from_payload(&pay).unwrap(), T2);
    let fin = n::apply_tail(&pay, &jl!["mmap"], Some("t2609290823")).unwrap();
    assert_eq!(n::compose_from_payload(&fin).unwrap(), format!("{T2}-mmap-t2609290823"));
    assert_eq!(n::compose_name(T2, &jl!["mmap"]).unwrap(), format!("{T2}-mmap"));
    assert_eq!(n::with_timestamp(T2, "2026-09-28T23:23:13Z").unwrap(), format!("{T2}-t2609290823"));
    assert_eq!(n::timestamp_token("2026-12-31T15:00:00Z").unwrap(), "t2701010000");
    assert_eq!(n::validate_new_name(&T2.replace("/0.29.0rc6/", "/v0.29.0rc6/")).unwrap_err().code, "HINT_VLLM_SEGMENT_SHAPE");
}

#[test]
fn tail_and_vocab_contracts() {
    let v = _fixture_vocab();
    let srcs = jd! {"e" => jd!{"K" => "1"}};
    let ok = jl![jd! {"token" => "mmap", "meaning" => "m", "evidence" => jd!{"file" => "e", "key" => "K", "value" => 1}}];
    assert!(n::validate_tail(&ok, &srcs).unwrap().is_empty());
    let lie = jl![jd! {"token" => "mmap", "meaning" => "m", "evidence" => jd!{"file" => "e", "key" => "K", "value" => "0"}}];
    assert_eq!(n::validate_tail(&lie, &srcs).unwrap()[0].0, "HINT_TAIL_UNGROUNDED");
    assert_eq!(n::normalize("kv", &J::from("fp8_e4m3"), &v).unwrap(), "fp8");
    assert_eq!(n::normalize("hw", &J::from("NVIDIA RTX PRO 6000"), &v).unwrap_err().code, "HINT_VOCAB_UNKNOWN");
    assert_eq!(n::base_slug("hy3-nvfp4-w4a16", &v).unwrap(), "hy3");
    assert_eq!(n::norm_slug("Gemma-4_E2B").unwrap(), "gemma4e2b");
    assert_eq!(n::release_of(&J::from(" v0.9.2.post1 ")).unwrap().as_deref(), Some("0.9.2.post1"));
    assert_eq!(n::_describe_release(&J::from("v0.29.0rc5-37-g0123456")).unwrap().as_deref(), Some("0.29.0rc5"));
    assert_eq!(n::vllm_repo_kind(&J::from("git@github.com:vllm-project/vllm.git")).unwrap().as_deref(), Some("upstream"));
    assert_eq!(n::vllm_repo_kind(&J::Null).unwrap(), None);
    let cands = n::tail_candidates(&_tag2_facts(), &v).unwrap();
    let toks: Vec<String> = cands.iter().map(|c| c["token"].display()).collect();
    assert_eq!(toks, ["plemmap", "spec3", "eager"]);
    assert!(n::version_key("0.29.0rc5-g0123456789ab").unwrap() < n::version_key("0.29.0rc6").unwrap());
    let parsed = n::parse_arch("gb10-1g2n-cluster-native-bare", Some(&v)).unwrap().unwrap();
    assert_eq!(parsed["plane"].as_str(), Some("native"));
    assert_eq!(n::split_recipe("x").unwrap(), None);
    let td = hintlib::core::TempDir::new("naming-cases-");
    assert_eq!(n::load_vocab(td.path()).unwrap_err().code, "HINT_VOCAB_ABSENT");
}
