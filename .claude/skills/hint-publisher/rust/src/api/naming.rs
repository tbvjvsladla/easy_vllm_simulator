//! hint_api bridge for `naming`.
use crate::api::{Call, Res};
use crate::core::HintError;
use crate::naming::{self as n, Axis, DerivedName, TagName};
use crate::value::{Map, J};

pub fn enc_tag(t: &TagName) -> J {
    crate::jd! {"__class__" => "TagName", "vllm" => t.vllm.clone(), "model" => t.model.clone(), "arch" => t.arch.clone(),
        "recipe" => t.recipe.clone(), "grammar" => t.grammar.clone(), "base_recipe" => t.base_recipe.clone(),
        "tail" => t.tail.clone(), "timestamp" => t.timestamp.clone()}
}

pub fn enc_axis(a: &Axis) -> J {
    crate::jd! {"__class__" => "Axis", "value" => a.value.clone(), "source" => a.source.clone(), "token" => a.token.clone()}
}

fn enc_axes(v: &[(String, Axis)]) -> J {
    let mut m = Map::new();
    for (k, a) in v {
        m.insert(k.clone(), enc_axis(a));
    }
    J::Dict(m)
}

pub fn enc_dn(d: &DerivedName) -> J {
    crate::jd! {"__class__" => "DerivedName", "tag" => d.tag.clone(), "segments" => enc_axes(&d.segments),
        "axes" => enc_axes(&d.axes), "vllm_build_input" => d.vllm_build_input.clone()}
}

fn dec_axis(j: &J) -> Axis {
    Axis {
        value: j["value"].display(),
        source: j["source"].display(),
        token: j.get_str("token").map(str::to_string),
    }
}

fn dec_axes(j: &J) -> Vec<(String, Axis)> {
    j.as_dict().map(|m| m.iter().map(|(k, v)| (k.to_string(), dec_axis(v))).collect()).unwrap_or_default()
}

fn dec_dn(j: &J) -> DerivedName {
    DerivedName {
        tag: j["tag"].display(),
        segments: dec_axes(&j["segments"]),
        axes: dec_axes(&j["axes"]),
        vllm_build_input: j["vllm_build_input"].clone(),
    }
}

fn opt_str(v: Option<J>) -> J {
    J::from(v)
}

fn pairs(v: Vec<(String, String)>) -> J {
    J::List(v.into_iter().map(|(a, b)| crate::jl![a, b]).collect())
}

pub fn call(k: &Call) -> Option<Res> {
    let s = |i: usize, name: &str| k.str(i, name);
    Some(match k.func.as_str() {
        "parse_tag" => n::parse_tag(&s(0, "name")).map(|t| enc_tag(&t)),
        "base_name" => n::base_name(&s(0, "name")).map(J::Str),
        "grammar_of" => n::grammar_of(&s(0, "name")).map(J::Str),
        "plane_of_token" => {
            let v = k.arg(1, "vocab").filter(|v| !v.is_null()).cloned();
            n::plane_of_token(&s(0, "token"), v.as_ref()).map(J::from)
        }
        "parse_arch" => match k.arg(0, "arch") {
            Some(J::Str(a)) => {
                let v = k.arg(1, "vocab").filter(|v| !v.is_null()).cloned();
                n::parse_arch(a, v.as_ref()).map(opt_str)
            }
            _ => Ok(J::Null),
        },
        "parse_recipe" => n::parse_recipe(&k.j(0, "recipe")).map(opt_str),
        "split_recipe" => match k.arg(0, "recipe") {
            Some(J::Str(r)) => n::split_recipe(r).map(opt_str),
            _ => Ok(J::Null),
        },
        "_split_recipe_raw" => {
            let (a, b) = n::_split_recipe_raw(&k.j(0, "recipe"));
            Ok(crate::jl![J::from(a), J::from(b)])
        }
        "arch_violation" => match k.arg(0, "arch") {
            Some(J::Str(a)) => n::arch_violation(a).map(J::from),
            _ => Ok(J::from("HINT_ARCH_ABSENT")),
        },
        "recipe_violation" => Ok(J::from(n::_split_recipe_raw(&k.j(0, "recipe")).1)),
        "validate_new_name" => n::validate_new_name(&s(0, "tag")).map(|_| J::Null),
        "build_arch" => {
            let pt = k.opt_str(5, "plane_token").unwrap_or_default();
            match (k.arg(1, "gpus_per_node").and_then(J::as_i64), k.arg(2, "nodes").and_then(J::as_i64)) {
                (Some(g), Some(nn)) => n::build_arch(&s(0, "hw"), g, nn, &s(3, "role"), &s(4, "target"), &pt).map(J::Str),
                _ => Err(HintError::py("TypeError", "build_arch bridge takes int G/N")),
            }
        }
        "build_recipe" => n::build_recipe(&s(0, "q"), &k.j(1, "length"), &s(2, "kv"), &s(3, "ple"), &k.j(4, "spec"), &s(5, "graph")).map(J::Str),
        "build_recipe_base" => n::build_recipe_base(&s(0, "q"), &k.j(1, "length"), &s(2, "kv")).map(J::Str),
        "compose_from_payload" => n::compose_from_payload(&k.j(0, "naming")).map(J::Str),
        "_tail_token_problems" => Ok(J::from(n::_tail_token_problems(k.j(0, "tokens").items()))),
        "_norm_value" => Ok(J::Str(n::_norm_value(&k.j(0, "v")))),
        "validate_tail" => n::validate_tail(&k.j(0, "tail"), &k.j(1, "sources")).map(pairs),
        "compose_name" => n::compose_name(&s(0, "base_tag"), &k.j(1, "tail")).map(J::Str),
        "timestamp_token" => n::timestamp_token(&s(0, "generated_utc")).map(J::Str),
        "with_timestamp" => n::with_timestamp(&s(0, "name"), &s(1, "generated_utc")).map(J::Str),
        "apply_tail" => {
            let ts = k.arg(2, "timestamp").and_then(J::as_str).map(str::to_string);
            n::apply_tail(&k.j(0, "naming"), &k.j(1, "tail"), ts.as_deref())
        }
        "tail_candidates" => n::tail_candidates(&k.j(0, "facts"), &k.j(1, "vocab")).map(J::List),
        "derive_slug" => {
            let h = k.arg(0, "hf_repo").and_then(J::as_str).map(str::to_string);
            let m = k.arg(1, "model_path").and_then(J::as_str).map(str::to_string);
            n::derive_slug(h.as_deref(), m.as_deref()).map(J::Str)
        }
        "base_slug" => n::base_slug(&k.arg(0, "slug").map(J::display).unwrap_or_default(), &k.j(1, "vocab")).map(J::Str),
        // Python `norm_slug(None) == ""` (None is the empty key); the Rust signature takes `&str`.
        "norm_slug" => match k.arg(0, "s") {
            None | Some(J::Null) => Ok(J::from("")),
            Some(v) => n::norm_slug(&v.display()).map(J::Str),
        },
        "version_key" => n::version_key(&k.arg(0, "v").map(J::display).unwrap_or_default()).map(|t| {
            let ints = [t.0, t.1, t.2, t.3, t.4, t.5, t.6, t.7];
            let mut v: Vec<J> = ints.iter().map(|x| J::Int(*x as i64)).collect();
            v.push(J::Str(t.8));
            J::List(v)
        }),
        "_vnorm" => Ok(J::Str(n::_vnorm(&k.j(0, "text")))),
        "validate_vocab" => n::validate_vocab(&k.j(0, "vocab")).map(J::from),
        "_plane_vocab_problems" => n::_plane_vocab_problems(k.arg(0, "table")).map(J::from),
        "plane_token" => n::plane_token(&k.j(0, "plane"), &k.j(1, "vocab")).map(J::Str),
        "load_vocab" => n::load_vocab(&k.path(0, "repo")),
        "normalize" => n::normalize(&s(0, "axis"), &k.j(1, "raw"), &k.j(2, "vocab")).map(J::Str),
        "_describe_release" => n::_describe_release(&k.j(0, "text")).map(J::from),
        "release_of" => n::release_of(&k.j(0, "text")).map(J::from),
        "vllm_repo_kind" => n::vllm_repo_kind(&k.j(0, "url")).map(J::from),
        "vllm_segment" => n::vllm_segment(&k.j(0, "build_input")).map(|a| enc_axis(&a)),
        "derive_name" => n::derive_name(&k.j(0, "facts"), &k.j(1, "vocab")).map(|d| enc_dn(&d)),
        "DerivedName.to_payload" => Ok(dec_dn(&k.j(0, "self")).to_payload()),
        "Axis.as_dict" => Ok(dec_axis(&k.j(0, "self")).as_dict()),
        "_fixture_vocab" => Ok(n::_fixture_vocab()),
        "_tag2_facts" => Ok(n::_tag2_facts()),
        "selftest" => Ok(J::from(n::selftest())),
        _ => return None,
    })
}
