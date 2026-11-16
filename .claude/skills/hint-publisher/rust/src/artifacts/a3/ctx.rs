// ctx-dependent part of artifacts a3 (included into a3.rs).

// ── memo / ctx accessors (agreed `_Ctx` shape: memo = RefCell<Map>, tuple keys joined by U+001F) ──────────
fn mkey(parts: &[&str]) -> String {
    parts.join("\u{1f}")
}
fn memo_get(c: &_Ctx, key: &str) -> Option<J> {
    c.memo.borrow().get(key).cloned()
}
fn memo_has(c: &_Ctx, key: &str) -> bool {
    c.memo.borrow().contains_key(key)
}
fn memo_set(c: &_Ctx, key: &str, v: J) {
    c.memo.borrow_mut().insert(key, v);
}
/// `c.memo.get(("applied", True)) or c.memo.get(("applied", False))` — the 4-tuple as a J list.
fn memo_applied(c: &_Ctx) -> Option<J> {
    for k in [mkey(&["applied", "True"]), mkey(&["applied", "False"])] {
        if let Some(v) = memo_get(c, &k) {
            if v.truthy() {
                return Some(v);
            }
        }
    }
    None
}
fn memo_dockerfile(c: &_Ctx, name: &str) -> J {
    or_dict(memo_get(c, &mkey(&["dockerfile", name])).unwrap_or(J::Null))
}
fn trip_path(c: &_Ctx, k: &str) -> PathBuf {
    c.triplet.iter().find(|(a, _)| a == k).map(|(_, p)| p.clone()).unwrap_or_default()
}
fn path_j(p: &Path) -> J {
    J::str(core::posix(p))
}
fn hex_enc(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}
fn hex_dec(s: &str) -> Vec<u8> {
    (0..s.len() / 2).filter_map(|k| u8::from_str_radix(&s[2 * k..2 * k + 2], 16).ok()).collect()
}
/// Python `==` on JSON-shaped values (dict equality ignores key order).
fn py_eq(a: &J, b: &J) -> bool {
    match (a, b) {
        (J::Dict(x), J::Dict(y)) => x.len() == y.len() && x.iter().all(|(k, v)| y.get(k).map(|w| py_eq(v, w)).unwrap_or(false)),
        (J::List(x), J::List(y)) => x.len() == y.len() && x.iter().zip(y).all(|(p, q)| py_eq(p, q)),
        (J::Int(_) | J::Float(_) | J::Bool(_), J::Int(_) | J::Float(_) | J::Bool(_)) => {
            let f = |v: &J| match v {
                J::Int(i) => *i as f64,
                J::Float(f) => *f,
                J::Bool(b) => *b as i64 as f64,
                _ => 0.0,
            };
            f(a) == f(b)
        }
        _ => a == b,
    }
}
fn keyset(j: &J) -> std::collections::BTreeSet<String> {
    j.as_dict().map(|m| m.keys().map(str::to_string).collect()).unwrap_or_default()
}
fn tuple_repr(items: &[&str]) -> String {
    let parts: Vec<String> = items.iter().map(|x| crate::value::py_repr(x)).collect();
    if parts.len() == 1 {
        format!("({},)", parts[0])
    } else {
        format!("({})", parts.join(", "))
    }
}
/// `isinstance(ev, CellEvidence)` — the single rule of a1 (bridge `__class__` marker or the dataclass field set; production
/// evidence dicts from `evidence::from_campaign` carry no marker).
fn is_cell_evidence(ev: &J) -> bool {
    crate::artifacts::a1_is_cell_evidence(ev)
}
fn re_escape(s: &str) -> String {
    let mut o = String::new();
    for ch in s.chars() {
        if "\\.+*?()|[]{}^$#&-~".contains(ch) {
            o.push('\\');
        }
        o.push(ch);
    }
    o
}
fn re_env_file_ref() -> &'static Re {
    // OWNER-MIRROR (same module, part a1): artifacts.py:136 _ENV_FILE_REF
    static C: OnceLock<Re> = OnceLock::new();
    re_cached(&C, r"(?<![\w/.-])envs/(\.env\.[A-Za-z0-9_.-]+)", 0)
}

/// `shlex.split(s)` (posix, no comments).
fn shlex_split(s: &str) -> HR<Vec<String>> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut have = false;
    let mut it = s.chars().peekable();
    while let Some(ch) = it.next() {
        match ch {
            ' ' | '\t' | '\n' | '\r' => {
                if have {
                    out.push(std::mem::take(&mut cur));
                    have = false;
                }
            }
            '\\' => match it.next() {
                Some(n) => {
                    cur.push(n);
                    have = true;
                }
                None => return Err(HintError::py("ValueError", "No escaped character")),
            },
            '\'' => {
                have = true;
                loop {
                    match it.next() {
                        Some('\'') => break,
                        Some(x) => cur.push(x),
                        None => return Err(HintError::py("ValueError", "No closing quotation")),
                    }
                }
            }
            '"' => {
                have = true;
                loop {
                    match it.next() {
                        Some('"') => break,
                        Some('\\') => match it.peek().copied() {
                            Some(n) if n == '"' || n == '\\' => {
                                cur.push(n);
                                it.next();
                            }
                            Some(_) => cur.push('\\'),
                            None => return Err(HintError::py("ValueError", "No closing quotation")),
                        },
                        Some(x) => cur.push(x),
                        None => return Err(HintError::py("ValueError", "No closing quotation")),
                    }
                }
            }
            x => {
                cur.push(x);
                have = true;
            }
        }
    }
    if have {
        out.push(cur);
    }
    Ok(out)
}

/// `os.walk(root, followlinks=False)` visiting: callback(dir, dirnames(with is_symlink), filenames).
fn os_walk(root: &Path, f: &mut dyn FnMut(&Path, &[(PathBuf, bool)], &[String]) -> HR<()>) -> HR<()> {
    let Ok(rd) = std::fs::read_dir(root) else { return Ok(()) };
    let mut dirs: Vec<(PathBuf, bool)> = Vec::new();
    let mut names: Vec<String> = Vec::new();
    for e in rd.flatten() {
        let p = e.path();
        let link = std::fs::symlink_metadata(&p).map(|m| m.file_type().is_symlink()).unwrap_or(false);
        if p.is_dir() {
            dirs.push((p, link));
        } else {
            names.push(e.file_name().to_string_lossy().into_owned());
        }
    }
    f(root, &dirs, &names)?;
    for (d, link) in &dirs {
        if !link {
            os_walk(d, f)?;
        }
    }
    Ok(())
}

fn is_symlink(p: &Path) -> bool {
    std::fs::symlink_metadata(p).map(|m| m.file_type().is_symlink()).unwrap_or(false)
}

// ── 서브 레시피 (multi) ─────────────────────────────────────────────────────────────────────
fn re_tier_env() -> &'static Re {
    static C: OnceLock<Re> = OnceLock::new();
    re_cached(&C, r"^([A-Za-z_][A-Za-z0-9_]*)=(\S+)\s+#\s*tier=env\s*$", 0)
}

/// `_sub_recipe` — `shapes` = shipped env shapes (name → text) · `not_shipped` = regenerated rows not shipped.
pub fn _sub_recipe(c: &_Ctx, shapes: &J, not_shipped: &J) -> HR<J> {
    if c.topo != "multi" {
        return core::fail("HINT_SUB_RECIPE_NOT_MULTI", "sub_recipe 는 multi 셀에만 성립한다.", None);
    }
    if c.compose_path.is_none() {
        return core::fail("HINT_SUB_RECIPE_COMPOSE_ABSENT", format!("output/{}/docker-compose.yaml 부재 — 전달 목록을 파생할 수 없다.", c.topo), None);
    }
    let compose_file = _compose_file(c)?;
    let forwards = _slave_forward_rows(c)?;
    let rows = owner_as_sub_recipe(c, &forwards)?;
    let mut env_forward: Vec<J> = Vec::new();
    let mut image_keys: Vec<String> = Vec::new();
    let mut per_node = Map::new();
    for r in rows.items() {
        let key = s(&g(r, "key"));
        let group = s(&g(r, "group"));
        let shaped = if r.get("value_shape").is_some() {
            s(&g(r, "value_shape"))
        } else if group == "mount" {
            crate::pii::manifest_placeholder(&key.to_lowercase())?
        } else {
            let v = g(r, "value");
            if v.truthy() { s(&v) } else { String::new() }
        };
        if group == "mount" {
            per_node.insert(key.clone(), shaped.clone());
        }
        let from = if g(r, "from").truthy() { g(r, "from") } else { g(r, "source") };
        env_forward.push(jd! {"key" => key.clone(), "group" => group.clone(), "from" => from, "compose_ref" => g(r, "compose_ref"),
                              "why" => g(r, "why"), "value" => shaped});
        if group == "image_identity" {
            image_keys.push(key);
        }
    }
    let cluster_shape = g(shapes, ".env.cluster");
    if cluster_shape.truthy() {
        for ln in py_splitlines(&s(&cluster_shape)) {
            if let Some(km) = re_tier_env().match_at_start(ln) {
                per_node.insert(pyre::group(&km, 1).unwrap_or(""), pyre::group(&km, 2).unwrap_or(""));
            }
        }
    }
    if shapes.get(".env.interconnect").is_some() {
        per_node.insert("interconnect", "artifacts/compose/.env.interconnect.template");
    }
    let never = owner_never_forward(c)?;
    let slave_cfg = if never.iter().any(|x| x == "CONFIG_FILE") { owner_compose_default(c, compose_file.as_deref(), "CONFIG_FILE")? } else { J::Null };
    let arm = _arm_roles(c, &forwards)?;
    let am = arm["master"].clone();
    let asl = or_dict(arm["slave"].clone());
    let runner_asset = owner_shared_asset_dir(c)?.join("serve_runner.sh");
    let gpu = if runner_asset.is_file() {
        let sel = _select_tracked(c, &runner_asset)?;
        _runner_gpu_literal(&String::from_utf8_lossy(&sel_bytes(&sel)))?
    } else {
        jd! {"value" => J::Null, "note" => "러너 부재"}
    };
    let att = g(&c.ev, "attestation");
    let mut parity = jd! {"axes" => jl!["vllm_sha", "torch", "driver", "build_ledger"], "source" => _attestation_ref(c)?};
    let role_of = |n: &str| -> String {
        match n {
            "main" | "master" => "main".into(),
            "sub" | "slave" => "sub".into(),
            _ => "<node>".into(),
        }
    };
    let attp = g(&att, "parity");
    let attc = g(&att, "checks");
    if att.is_dict() && attp.is_dict() && attp.truthy() {
        let mut obs = Map::new();
        for axis in ["vllm_sha", "torch", "driver"] {
            let row = g(&attp, axis);
            if row.is_dict() {
                obs.insert(axis, jd! {"verdict" => g(&row, "verdict"), "main" => g(&row, "main"), "sub" => g(&row, "sub")});
            }
        }
        let bl = g(&attp, "build_ledger");
        if bl.is_dict() {
            let mut m = Map::new();
            for k in ["verdict", "main_identity_sha256", "sub_identity_sha256", "identity_exempt_build_args", "compared"] {
                m.insert(k, g(&bl, k));
            }
            obs.insert("build_ledger", J::Dict(m));
        }
        parity.set("observed", J::Dict(obs));
        parity.set("blocking", g(&att, "parity_blocking").truthy());
    } else if att.is_dict() && attc.is_list() && attc.truthy() {
        let mut v = Vec::new();
        for row in attc.items() {
            if row.is_dict() && !(g(row, "observed").is_dict() || g(row, "observed").is_list()) {
                v.push(jd! {"axis" => s(&g(row, "axis")), "node" => role_of(&s(&g(row, "node"))),
                            "observed" => s(&g(row, "observed")), "expected" => s(&g(row, "expected"))});
            }
        }
        parity.set("observed", J::List(v));
    } else {
        parity.set("observed", "unobservable");
        parity.set(
            "reason",
            if !att.is_dict() { "attestation 부재" } else { "attestation 에 checks·parity 가 비었다(K1: 옛 스모크는 wheel 트랙에서만 축을 채웠다)" },
        );
    }
    let master_rp = if g(&am, "armed").truthy() {
        format!("armed — {}={} → /app/configs/{}", s(&g(&am, "key")), g(&am, "stem").repr(), s(&g(&am, "patch")))
    } else {
        format!("none(런타임 패치 파일 없음 — {}={} → {} 부재)", s(&g(&am, "key")), g(&am, "stem").repr(), s(&g(&am, "patch")))
    };
    let akey = g(&asl, "key");
    let akey_in_never = akey.as_str().map(|k| never.iter().any(|x| x == k)).unwrap_or(false);
    let slave_rp = if g(&asl, "armed").truthy() {
        format!(
            "armed — slave {}={}{} → /app/configs/{}",
            s(&akey),
            g(&asl, "stem").repr(),
            if !akey_in_never { "(slave_forward 전달)" } else { "(compose 기본값 · slave_forward.NEVER_FORWARD)" },
            s(&g(&asl, "patch"))
        )
    } else {
        format!(
            "not-armed — slave {}={} → arm_patch.sh 가 /app/configs/{} 를 찾고 없으면 no-op{}",
            s(&akey),
            g(&asl, "stem").repr(),
            s(&g(&asl, "patch")),
            if matches!(akey.as_str(), Some("CONFIG_FILE") | Some("default")) { " (K11)" } else { "" }
        )
    };
    let mut chain: Vec<J> = arm["chain"]["chain"].items().to_vec();
    chain.push(J::str(arm["chain"]["default"].repr()));
    let mut recipe = jd! {
        "schema_version" => 1,
        "role_diff" => jd! {
            "master" => "ray head + vllm serve(트리플렛 source · 모델 러너 실행)",
            "slave" => "ray worker(트리플렛 미수령 — 같은 이미지·같은 compose·같은 serve_runner.sh 가 NODE_ROLE 로 분기)",
            "slave_config_file" => slave_cfg,
            "master_runtime_patch" => master_rp,
            "slave_runtime_patch" => slave_rp,
            "runtime_patch_key_chain" => J::List(chain),
            "runner_gpu_join_literal" => gpu,
            "source" => jd! {
                "slave_config_file" => "compose-default:CONFIG_FILE × slave_forward.NEVER_FORWARD",
                "runtime_patch" => format!("output/{}/configs/<stem>_patch.py 실재 여부 × {}", c.topo, s(&arm["chain"]["source"])),
                "runner_gpu_join_literal" => "render_dockerfile.SHARED_ASSET_DIR/serve_runner.sh(측정 당시 개정 — slots.compose.evidence.selected_revisions)",
            },
        },
        "launch_order" => jl!["build(양 노드 병렬)", "master up(head)", "slave up(join)", "master: GPU 합류 확인 → vllm serve"],
        "launch_order_source" => REL_SMOKE_MULTI,
        "env_forward" => J::List(env_forward),
        "env_forward_source" => format!("{} derive() — 스모크와 같은 함수(AC8)", core::REL_SLAVE_FORWARD),
        "image_identity_args" => image_keys,
        "per_node_values" => J::Dict(per_node),
        "preconditions" => jl!["가중치가 같은 마운트 경로에 실재(메인 .env 의 mount 값이 서브에 전달된다)",
                               "PLE mmap 이면 slave 로컬 스테이징도 실재", "vLLM SHA · torch · driver 동일(동일 이미지가 아니라 동일 ABI)"],
        "parity_attestation" => parity,
    };
    if not_shipped.truthy() {
        let v: Vec<J> = not_shipped
            .items()
            .iter()
            .map(|r| jd! {"file" => g(r, "file"), "reason" => POST_MEASUREMENT_REGENERATED,
                           "replaced_by" => format!("{ENV_OBSERVED_POINTER}(측정 엔진 로그 관측)")})
            .collect();
        recipe.set("env_shapes_not_shipped", J::List(v));
    }
    let terms = crate::pii::require_terms(&c.repo)?;
    let hits = crate::pii::scan_text(&core::dumps(&recipe), Some(&terms), "deploy", &[], false, true, None)?;
    if !hits.is_empty() {
        return core::fail(
            "HINT_SUB_RECIPE_PII",
            format!("sub_recipe 에 배포 금지 패턴:\n  {}", crate::pii::render_hits(&hits, 10)?),
            Some("slave_forward 의 해당 그룹 값을 표지로 바꾸는 규칙을 고친다(덧칠 ✗)."),
        );
    }
    Ok(recipe)
}

/// `sub_recipe` — the multi sub recipe (plan §4.6 · SPEC §5.6).
pub fn sub_recipe(repo: &Path, ev: &J, forward_module: &J, render_module: &J) -> HR<J> {
    let c = _ctx(repo, ev, &J::Null, forward_module, render_module)?;
    let mut shapes = Map::new();
    let mut skipped: Vec<J> = Vec::new();
    for name in [".env.cluster", ".env.interconnect"] {
        let p = c.out.join("envs").join(name);
        if !p.is_file() {
            continue;
        }
        let kind = s(&_TIER_ENV_KINDS()[name]);
        if let Some(regen) = _regenerated(&c, &p)? {
            let sh = _shape_regenerated(&c, &p, name, &kind, &regen, &J::Null)?;
            if g(&sh, "ok").truthy() {
                shapes.insert(name, g(&sh, "text"));
            } else {
                skipped.push(jd! {"file" => format!("envs/{name}")});
            }
            continue;
        }
        let measured = _measured_utc(&c)?.0;
        shapes.insert(name, owner_env_shape(&c, &p, &kind, measured.as_deref())?);
    }
    let shapes = J::Dict(shapes);
    let skipped = J::List(skipped);
    let Some(cp) = c.compose_path.clone() else { return _sub_recipe(&c, &shapes, &skipped) };
    let sel = _select_tracked(&c, &cp)?;
    if sel.get_str("shipped") == Some("worktree") {
        return _sub_recipe(&c, &shapes, &skipped);
    }
    let td = core::TempDir::new("hint-compose-rev-");
    let eff = td.path().join(file_name(&cp));
    std::fs::write(&eff, sel_bytes(&sel)).map_err(|e| core::io_err(&e, &eff))?;
    *c.compose_eff.borrow_mut() = Some(eff);
    let r = _sub_recipe(&c, &shapes, &skipped);
    *c.compose_eff.borrow_mut() = None;
    r
}

/// `value_status_candidates` — serving yaml knobs × status candidates (SPEC §5.6).
pub fn value_status_candidates(repo: &Path, ev: &J, forward_module: &J) -> HR<Vec<J>> {
    let c = _ctx(repo, ev, &J::Null, forward_module, &J::Null)?;
    value_status_rows(&c)
}
fn value_status_rows(c: &_Ctx) -> HR<Vec<J>> {
    let lockset = or_dict(g(&c.ev, "lockset"));
    let lockset = if lockset.is_dict() { lockset } else { J::dict() };
    let mut by_knob = Map::new();
    if let Some(m) = LOCKSET_SOURCE_KNOB().as_dict() {
        for (k, v) in m.iter() {
            by_knob.insert(s(v), k);
        }
    }
    const EXCL: [&str; 4] = ["model", "host", "port", "served-model-name"]; // OWNER-MIRROR a1 VALUE_STATUS_EXCLUDED_KNOBS
    let mut rows = Vec::new();
    for (knob, value, comment) in _serving_yaml_knobs(&trip_path(c, "yaml"))? {
        if EXCL.contains(&knob.replace('_', "-").as_str()) {
            continue;
        }
        let mut status = J::Null;
        let mut reason: Option<String> = None;
        let mut note: Option<String> = None;
        let mut source = "serving-yaml".to_string();
        let skey = by_knob.get(&knob).map(s);
        let sval = skey.as_ref().map(|k| g(&lockset, k)).unwrap_or(J::Null);
        if let (Some(sk), Some(sv)) = (&skey, sval.as_str().filter(|x| !x.is_empty())) {
            status = _SOURCE_STATUS().get(sv).cloned().unwrap_or(J::Null);
            source = format!("lockset.{sk}={sv}");
            let prov = g(&lockset, "provenance");
            reason = Some(format!(
                "lockset 출처 {sk}={sv}{}{}",
                if status.truthy() { "" } else { " — 어휘 밖(후보 없음)" },
                if prov.truthy() { format!(" · lockset provenance={}", s(&prov)) } else { String::new() }
            ));
        }
        if status.is_null() {
            if let Some(cm) = comment.as_deref().filter(|x| !x.is_empty()) {
                for pair in _COMMENT_STATUS().items() {
                    let word = s(pair.at(0));
                    let st = s(pair.at(1));
                    if cm.contains(&word) {
                        status = J::str(st.clone());
                        source = "serving-yaml 줄 주석".into();
                        let mut rsn = format!("주석 단어 '{word}': {cm}");
                        if st == "declared-requirement" || st == "tuned" {
                            status = J::str("inherited");
                            let n = if st == "declared-requirement" { COMMENT_ONLY_REQUIREMENT_NOTE } else { COMMENT_ONLY_TUNED_NOTE };
                            rsn = format!("{rsn} · {n}");
                            note = Some(n.to_string());
                        }
                        reason = Some(rsn);
                        break;
                    }
                }
            }
        }
        if status.is_null() && value.to_lowercase() == "auto" {
            status = J::str("engine-default");
            source = "serving-yaml 값 토큰".into();
            reason = Some("값 `auto` = 엔진이 고른다".into());
        }
        let mut row = jd! {"knob" => knob, "value" => value, "candidate_status" => status,
            "reason" => reason.filter(|r| !r.is_empty()).unwrap_or_else(|| "근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다)".into()),
            "source" => source, "comment" => J::from(comment)};
        if let Some(n) = note {
            row.set("note", n);
        }
        rows.push(row);
    }
    Ok(rows)
}

// ── 재현 절차 · 관측 소요 ────────────────────────────────────────────────────────────────────
fn utc_plus(start: &str, ttl: f64) -> HR<String> {
    let base = core::parse_utc(start)?.epoch();
    let micros = (ttl * 1e6).round() as i64;
    let t = (base * 1_000_000 + micros).div_euclid(1_000_000);
    Ok(format!("{}Z", core::Civil::from_epoch(t).iso()))
}

/// `_serve_budget_window` — legacy upper bound only when one observed budget window covers the measurement.
pub fn _serve_budget_window(c: &_Ctx, first: Option<&str>, end: Option<&str>) -> HR<Option<J>> {
    let key = "serve_budget_window_diagnostic";
    let (Some(first), Some(end)) = (first.filter(|x| !x.is_empty()), end.filter(|x| !x.is_empty())) else {
        memo_set(c, key, jd! {"status" => "measurement-window-unavailable", "candidate_count" => 0});
        return Ok(None);
    };
    let ev = jd! {"declaration" => g(&c.ev, "declaration"), "node" => c.node.clone()};
    let mut candidates: Vec<J> = Vec::new();
    for node in crate::evidence::_event_node_dirs(&c.repo, &ev)? {
        let streams = crate::evidence::_node_streams(&c.repo, &node)?;
        let streams_j = J::List(
            streams
                .iter()
                .map(|(k, rows, srcs, bad)| {
                    J::List(vec![
                        J::str(k.clone()),
                        J::List(rows.iter().map(|(t, l, d)| J::List(vec![J::str(t.clone()), J::Int(*l), d.clone()])).collect()),
                        J::from(srcs.clone()),
                        bad.clone(),
                    ])
                })
                .collect(),
        );
        let opens = crate::evidence::_node_opens(&streams_j)?;
        let opens_j = J::List(opens.into_iter().map(|(a, b)| J::List(vec![J::Str(a), J::from(b)])).collect());
        for (i, (_k, rows, sources, bad)) in streams.iter().enumerate() {
            if rows.is_empty() || bad.as_dict().map(|m| m.values().any(J::truthy)).unwrap_or(false) {
                continue;
            }
            let rows_j = streams_j.at(i).at(1).clone();
            for w in crate::evidence::_event_windows(&rows_j, &opens_j)? {
                let fin = g(&w, "end_ts");
                if fin.is_null() {
                    continue;
                }
                let mut finish = s(&fin);
                let ri = w["rows"].at(0).as_i64().unwrap_or(0) as usize;
                let ttl = rows.get(ri).map(|r| g(&r.2, "ttl_s")).unwrap_or(J::Null);
                if let Some(t) = ttl.as_f64() {
                    let exp = utc_plus(&s(&g(&w, "start_ts")), t)?;
                    if exp < finish {
                        finish = exp;
                    }
                }
                let st = s(&g(&w, "start_ts"));
                if w.get_str("open_kind") == Some("budget_declare")
                    && crate::evidence::_label_is_cell(&g(&w, "label"), &c.cell)?
                    && st.as_str() <= first
                    && first <= end
                    && end < finish.as_str()
                {
                    let ln = rows.get(ri).map(|r| r.1).unwrap_or(0);
                    candidates.push(jd! {"start_utc" => st.clone(), "source" => format!(
                        "{}:{} budget_declare(label {}) · 관측된 예산 창 안의 측정", sources.get(ri).cloned().unwrap_or_default(), ln, s(&g(&w, "label")))});
                }
            }
        }
    }
    let status = if candidates.len() == 1 {
        "selected"
    } else if !candidates.is_empty() {
        "multiple-covering-windows"
    } else {
        "no-covering-window"
    };
    let n = candidates.len();
    let chosen = if n == 1 { Some(candidates[0].clone()) } else { None };
    memo_set(c, key, jd! {"status" => status, "candidate_count" => n, "candidates" => J::List(candidates)});
    Ok(chosen)
}

/// `_unselected_serve_window_reason`.
pub fn _unselected_serve_window_reason(c: &_Ctx) -> HR<String> {
    let diag = or_dict(memo_get(c, "serve_budget_window_diagnostic").unwrap_or(J::Null));
    if diag.get_str("status") == Some("multiple-covering-windows") {
        return Ok(format!(
            "관측됨(이 측정 전체를 포함하는 예산 창 {}개 · 복수 후보이므로 단일 상한 선택 안 함; 서빙 세션·하나의 소요시간으로 합성하지 않음)",
            s(&idx(&diag, "candidate_count")?)
        ));
    }
    Ok("미관측(이 측정을 포함하는 serve 세션 또는 단일 예산 창 후보 없음 — 과거 같은 셀 선언으로 대체하지 않음)".into())
}

fn re_build_block() -> &'static Re {
    static C: OnceLock<Re> = OnceLock::new();
    re_cached(&C, r"(?m)^([ \t]*)build:[ \t]*\n((?:\1[ \t]+\S.*\n?)+)", 0)
}
fn re_context() -> &'static Re {
    static C: OnceLock<Re> = OnceLock::new();
    re_cached(&C, r"(?m)^\s*context:\s*(\S+)", 0)
}

/// `_compose_build_context` — compose `build.context` → repository-relative path and source.
pub fn _compose_build_context(c: &_Ctx) -> HR<(Option<String>, String)> {
    let Some(cp) = &c.compose_path else { return Ok((None, "compose 부재".into())) };
    let text = _read(cp)?;
    let raw = re_build_block()
        .search(&text)
        .and_then(|m| {
            let blk = pyre::group(&m, 2).unwrap_or("").to_string();
            re_context().search(&blk).map(|cm| pyre::group(&cm, 1).unwrap_or("").trim_matches(|ch| ch == '\'' || ch == '"').to_string())
        })
        .unwrap_or_default();
    if raw.is_empty() || raw.contains('$') {
        return Ok((None, "compose build.context 미해석".into()));
    }
    let full = core::resolve(&cp.parent().unwrap_or(Path::new(".")).join(&raw));
    let rel = match full.strip_prefix(&c.repo) {
        Ok(r) => {
            let t = core::posix(r);
            if t.is_empty() { ".".to_string() } else { t }
        }
        Err(_) => return Ok((None, "compose build.context 가 저장소 밖".into())),
    };
    Ok((Some(rel), format!("{} build.context={raw}", core::rel(&c.repo, cp)?)))
}

/// `_build_command` — (command, source) of an independently runnable `docker build`.
pub fn _build_command(c: &_Ctx, sel: &J) -> HR<(String, String)> {
    let mut args: Vec<(String, String)> = Vec::new();
    let mut asrc: Option<&str> = None;
    if let Some(app) = memo_applied(c) {
        let applied = app.at(0).clone();
        let ledger = app.at(2).clone();
        if ledger.is_dict() && g(&ledger, "build_args").is_dict() {
            for (k, v) in g(&ledger, "build_args").as_dict().unwrap().iter() {
                if let Some(vs) = v.as_str() {
                    args.push((k.to_string(), vs.to_string()));
                }
            }
            asrc = Some("build-ledger build_args");
        } else if g(&or_dict(applied.clone()), "reconstruction").is_dict() {
            let ba = or_dict(g(&g(&applied, "reconstruction"), "build_args"));
            if let Some(m) = ba.as_dict() {
                for (k, v) in m.iter() {
                    args.push((k.to_string(), s(v)));
                }
            }
            asrc = Some("docker-history build-args");
        }
    }
    let (ctx, _csrc) = _compose_build_context(c)?;
    let out_rel = core::rel(&c.repo, &c.out)?;
    let mut context = ctx.clone().unwrap_or_else(|| out_rel.clone());
    let dname = s(&idx(sel, "dockerfile")?);
    let dmem = memo_dockerfile(c, &dname);
    let origin = g(&dmem, "origin_path");
    let origin_s = if origin.truthy() { Some(s(&origin)) } else { None };
    if let Some(o) = &origin_s {
        if pure_parent(o) != context && context != "." {
            context = out_rel.clone();
        }
    }
    let df = if context == "." { dname.clone() } else { format!("{context}/{dname}") };
    args.sort();
    let mut parts = vec![format!("docker build -f {}", shq(&df))];
    for (k, v) in &args {
        parts.push(format!("  --build-arg {}", shq(&format!("{k}={v}"))));
    }
    if g(sel, "image_tag").truthy() {
        parts.push("  -t \"${REPRO_IMAGE_TAG:?set a separate reproduction image tag}\"".into());
    }
    parts.push(format!("  {}", shq(&context)));
    let shipped0 = g(&dmem, "shipped");
    let mut shipped = if shipped0.truthy() { s(&shipped0) } else { "worktree".into() };
    let hm = g(&dmem, "history_match");
    let otxt = if g(&dmem, "origin_path").truthy() { format!("({})", s(&g(&dmem, "origin_path"))) } else { String::new() };
    shipped.push_str(&otxt);
    let rev_txt = if g(&dmem, "verified").truthy() {
        format!("이미지를 지은 개정: {shipped}")
    } else if hm.is_dict() {
        format!(
            "실린 개정: {shipped} — 이미지를 지은 바이트임을 관측으로 확인 못함(docker history RUN·COPY {}/{})",
            s(&idx(&hm, "matched")?),
            s(&idx(&hm, "total")?)
        )
    } else {
        format!("실린 개정: {shipped} — 이미지를 지은 바이트임은 미검증(history 대조 없음)")
    };
    let mut head = vec![
        format!("# 실린 artifacts/build_recipe/{dname}({rev_txt})를 {df} 자리에 두는 재현 예시다(실제 빌드 컨텍스트 아님)."),
        "# 독립 재현용 태그를 지정한다. 측정 이미지와 동일한 바이트·후속 빌드 층까지 재현되는지는 별도 검증한다.".to_string(),
    ];
    let dtext = g(&dmem, "text");
    let empties = _empty_copy_dirs(c, &if dtext.truthy() { s(&dtext) } else { String::new() }, &J::Null)?;
    if !empties.is_empty() {
        let dirs: Vec<String> = empties.iter().map(|e| shq(&format!("{context}/{}", s(&e["dir"])))).collect();
        let srcs: Vec<String> = empties.iter().map(|e| s(&e["source"])).collect();
        head.push(format!("mkdir -p {}   # {}", dirs.join(" "), srcs.join(" · ")));
    }
    if c.topo == "multi" {
        head.push("# 양 노드에서 각각 짓는다(이미지 save/load 전송 ✗ — 노드마다 자기 이미지 · 요건은 동일 digest 가 아니라 동일 ABI).".into());
    }
    let (lit_kind, lit) = match &c.compose_path {
        Some(p) => crate::evidence::compose_build_dockerfile(&path_j(p))?,
        None => (None, None),
    };
    let tail: Vec<String> = if origin_s.as_ref().map(|o| pure_parent(o) != out_rel).unwrap_or(false) {
        vec![format!(
            "# 실린 레시피의 이력 출처는 {}다. 이 zip 재현 예시는 현재 토폴로지 통로 {out_rel}/를 사용하며 실제 빌드 경로와 동등 여부는 별도 확인한다. 실린 compose 파일만 단독 사용하고 override가 없을 때의 설정을 확인하라. 실제 원본·후속층 실행 경로는 별도 실행 기록에서 확인한다.",
            pure_parent(origin_s.as_ref().unwrap())
        )]
    } else if c.topo == "multi" {
        vec![format!("# 스모크 빌드 사용법(실제 실행·재현 등치는 미검증): bash {REL_SMOKE_MULTI} {} --build-only", c.cell)]
    } else if lit_kind.as_deref() == Some("literal") && lit.as_deref() != Some(dname.as_str()) {
        vec![format!(
            "# 이 zip 재현 예시: 실린 output/{}/docker-compose.yaml 파일만 단독 사용하고 override가 없으면 build.dockerfile 리터럴은 {}이다(실린 레시피 {dname}과 다름). 그 조건의 compose build는 위 예시와 동등 경로가 아니다. 위 docker build는 재현 권장 예시이며 실제 원본·후속층 실행 경로는 별도 실행 기록에서 확인한다.",
            c.topo,
            lit.clone().map(|x| x).unwrap_or_else(|| "None".into())
        )]
    } else {
        vec![format!(
            "# compose 빌드 사용법(실제 실행·재현 등치는 미검증): docker compose -f output/{t}/docker-compose.yaml --env-file output/{t}/envs/.env.{} build",
            c.cell,
            t = c.topo
        )]
    };
    let mut src = if asrc.is_some() && !args.is_empty() {
        format!("reconstructed({} + compose build 선언/설정(실행 관측 아님))", asrc.unwrap())
    } else {
        "reconstructed(compose build 선언/설정(실행 관측 아님) · build-arg 미관측)".to_string()
    };
    if ctx.is_none() {
        src.push_str(&format!("(compose 파일 build.context 미해석 — {context} 로 둔다)"));
    }
    let mut all = head;
    all.push(parts.join(" \\\n"));
    all.extend(tail);
    Ok((all.join("\n"), src))
}

fn is_lite_leg(rel: &str) -> bool {
    let n = pure_name(rel);
    n.starts_with("lite_cold_") || n.starts_with("lite_warm_")
}
fn plain_int(j: &J) -> Option<i64> {
    j.as_i64()
}

/// `_bench_commands` — commands rebuilt from the sweep's bench JSON fields only.
pub fn _bench_commands(c: &_Ctx) -> HR<(String, String)> {
    let sweep = or_dict(g(&c.ev, "sweep"));
    let sdir = if sweep.is_dict() { g(&sweep, "dir") } else { J::Null };
    let binding = if sweep.is_dict() { g(&sweep, "binding") } else { J::Null };
    if !binding.is_null() && binding.as_str() != Some("output") {
        return Ok((
            String::new(),
            format!("unobserved(스윕 binding={} — 출력 평면에 묶인 스윕만 재구성한다 · evidence.bench_command 와 같은 규칙)", binding.repr()),
        ));
    }
    let mut rows: Vec<(String, String, J)> = Vec::new();
    if let Some(sd) = sdir.as_str() {
        if c.repo.join(sd).is_dir() {
            for f in rglob_suffix(&c.repo.join(sd), ".json") {
                if !core::level_raw_is_measured_tool(&f, &c.cell) {
                    continue;
                }
                let Some(d) = _json_file(&f)? else { continue };
                if !d.truthy() || !_BENCH_JSON_KEYS.iter().all(|k| d.get(k).is_some()) {
                    continue;
                }
                let ds = if g(&d, "date").truthy() { s(&g(&d, "date")) } else { String::new() };
                let key = re_local_date().match_at_start(&ds).map(|m| (1..=6).map(|i| pyre::group(&m, i).unwrap_or("")).collect::<String>()).unwrap_or("~".into());
                rows.push((key, rel_posix(&c.repo, &f)?, d));
            }
        }
    }
    let mut lw: Option<J> = None;
    if rows.is_empty() && !sdir.truthy() {
        lw = _lite_window(c)?;
        for leg in ["cold", "warm"] {
            let x = lw.as_ref().map(|l| g(&g(l, "legs"), leg)).unwrap_or(J::Null);
            if x.truthy() && _BENCH_JSON_KEYS.iter().all(|k| x["doc"].get(k).is_some()) {
                let ds = if g(&x["doc"], "date").truthy() { s(&g(&x["doc"], "date")) } else { String::new() };
                let key = re_local_date().match_at_start(&ds).map(|m| (1..=6).map(|i| pyre::group(&m, i).unwrap_or("")).collect::<String>()).unwrap_or("~".into());
                rows.push((key, s(&x["path"]), x["doc"].clone()));
            }
        }
    }
    if rows.is_empty() {
        let lite = g(&c.ev, "report_kind").as_str() == Some("lite");
        return Ok((
            String::new(),
            if sdir.truthy() || !lite {
                "unobserved(스윕 디렉터리에 vllm bench serve JSON 없음)".into()
            } else {
                "unobserved(lite-only 셀 — lite_raw 조인 불성립 또는 레그 JSON 부재)".into()
            },
        ));
    }
    rows.sort_by(|a, b| (&a.0, &a.1).cmp(&(&b.0, &b.1)));
    let tool = if rows.iter().any(|(_, r, _)| is_lite_leg(r)) { _lite_tool_args(c)? } else { None };
    let mut lines: Vec<String> = vec![
        "# bench JSON 필드로 복원: backend · model_id→--model · tokenizer_id→--tokenizer · num_prompts · max_concurrency · request_rate · burstiness · 요청당 입출력 길이 = total_*_tokens ÷ completed(나누어떨어질 때만 — 평균이다 · 요청별 길이는 JSON 에 없다) · 나누어떨어지면 `--dataset-name random` 으로 추정했다(측정 도구가 그 꼴로 부른다 — 아래 도구 원문).".into(),
        "# JSON 에 없는 인자(--endpoint · --base-url · --ignore-eos · --num-warmups · --temperature · --seed · --random-range-ratio · --trust-remote-code)는 측정 도구가 정했다 — level_* = run_bench.sh · lite_cold/lite_warm = lite_bench.sh(.claude/skills/adversarial-benchmark/scripts/) · 셀 스윕 순서는 sweep_bench.sh.".into(),
    ];
    let mut tflags: Vec<(String, J)> = Vec::new();
    if let Some(t) = &tool {
        for p in or_list(g(t, "flags")).items() {
            let f = s(p.at(0));
            if _LITE_TOOL_LOAD_FLAGS.contains(&f.as_str()) {
                tflags.push((f, p.at(1).clone()));
            }
        }
    }
    let have_t = tool.as_ref().map(J::truthy).unwrap_or(false) && !tflags.is_empty();
    if have_t {
        lines.push(format!(
            "# lite_cold/lite_warm 의 데이터셋 · 부하 인자 = 측정 시점 도구 원문 {}(리터럴 인자만 · `$변수` 인자는 도구가 값을 정하지 않아 JSON 필드로 채운다) — `total_*_tokens ÷ completed` 는 실측 토큰(chat 템플릿 포함)이지 인자가 아니다.",
            s(&tool.as_ref().unwrap()["source"])
        ));
    }
    if !lw.as_ref().map(J::truthy).unwrap_or(false) {
        lw = if !rows.is_empty() && !sdir.truthy() { _lite_window(c)? } else { None };
    }
    if let Some(l) = lw.as_ref().filter(|l| l.truthy()) {
        let raw = or_dict(g(l, "raw_doc"));
        let cn = g(&raw, "config_name");
        let mut inv = vec![format!(
            "bash .claude/skills/adversarial-benchmark/scripts/lite_bench.sh {}",
            shq(&if cn.truthy() { s(&cn) } else { c.cell.clone() })
        )];
        if g(&raw, "topology").truthy() {
            inv.push(format!("--topology {}", shq(&s(&raw["topology"]))));
        }
        if g(&raw, "backend").truthy() {
            inv.push(format!("--backend {}", shq(&s(&raw["backend"]))));
        }
        if g(&c.ev, "bench_report_path").truthy() {
            inv.push("--publish-report".into());
        }
        lines.push(format!(
            "# 호출(재구성 — lite_raw config_name · topology · backend + 바인딩된 경량 리포트 = --publish-report · 원문 호출 줄 아님): {}",
            inv.join(" ")
        ));
        let eu = g(l, "end_utc");
        let es = g(l, "end_source");
        lines.push(format!(
            "# 측정 창: 시작 {}({}) → 끝 {}({})",
            s(&idx(l, "start_utc")?),
            s(&idx(l, "start_source")?),
            if eu.truthy() { s(&eu) } else { "미관측".into() },
            if es.truthy() { s(&es) } else { "레그 JSON date · lite_raw mtime 모두 없음".into() }
        ));
    }
    for (_, rel, d) in &rows {
        let mut flags = vec!["vllm bench serve".to_string(), format!("--backend {}", shq(&s(&idx(d, "backend")?)))];
        for (key, flag) in [("model_id", "--model"), ("tokenizer_id", "--tokenizer")] {
            if g(d, key).truthy() {
                flags.push(format!("{flag} {}", shq(&s(&d[key]))));
            }
        }
        let (n, ti, to) = (plain_int(&g(d, "completed")), plain_int(&g(d, "total_input_tokens")), plain_int(&g(d, "total_output_tokens")));
        let ints = match (n, ti, to) {
            (Some(n), Some(ti), Some(to)) if n > 0 => Some((n, ti, to)),
            _ => None,
        };
        let uniform = ints.map(|(n, ti, to)| ti.rem_euclid(n) == 0 && to.rem_euclid(n) == 0).unwrap_or(false);
        let is_lite = is_lite_leg(rel);
        let mut mean_note = String::new();
        if is_lite && !tflags.is_empty() {
            for (f_, v_) in &tflags {
                if f_ == "--max-concurrency" || f_ == "--request-rate" {
                    continue;
                }
                flags.push(if v_.is_null() { f_.clone() } else { format!("{f_} {}", s(v_)) });
            }
            if let Some((n, ti, to)) = ints {
                mean_note = format!(
                    " · 실측 토큰(chat 템플릿 포함 · total ÷ completed · 인자 아님): 입력 {ti}/{n} = {} · 출력 {to}/{n} = {}",
                    fmt_g(ti as f64 / n as f64),
                    fmt_g(to as f64 / n as f64)
                );
            }
        } else if uniform {
            let (n, ti, to) = ints.unwrap();
            flags.push("--dataset-name random".into());
            flags.push(format!("--random-input-len {}", ti.div_euclid(n)));
            flags.push(format!("--random-output-len {}", to.div_euclid(n)));
            if is_lite {
                mean_note = " · ⚠ --random-input-len/--random-output-len = total ÷ completed 평균(chat 템플릿 포함 실측 토큰 · 측정 도구 원문 인자 미관측 — lite_bench.sh 원문 인자와 다를 수 있다)".into();
            }
        }
        flags.push(format!("--num-prompts {}", s(&idx(d, "num_prompts")?)));
        flags.push(format!("--max-concurrency {}", s(&idx(d, "max_concurrency")?)));
        if !g(d, "request_rate").is_null() {
            flags.push(format!("--request-rate {}", shq(&s(&d["request_rate"]))));
        }
        let b = g(d, "burstiness");
        if !(b.is_null() || eq_int(&b, 1)) {
            flags.push(format!("--burstiness {}", s(&b)));
        }
        let acc = g(d, "spec_decode_acceptance_length");
        if is_lite && acc.is_number() {
            mean_note.push_str(&format!(" · spec_decode_acceptance_length {}(이 레그 JSON)", fmt_g(acc.as_f64().unwrap())));
        }
        lines.push(format!(
            "# {rel} (date {}){mean_note}{}",
            s(&g(d, "date")),
            if uniform || (is_lite && !tflags.is_empty()) {
                ""
            } else {
                " · total 토큰이 completed 로 나누어떨어지지 않는다(요청 길이 불균일) — 데이터셋 인자 복원 ✗"
            }
        ));
        lines.push(flags.join(" "));
    }
    let man = crate::evidence::output_manifest(&c.repo, &c.topo)?;
    let table = crate::pii::substitution_table(&c.repo, &J::from(man))?;
    Ok((
        crate::pii::substitute(&lines.join("\n"), &table, true)?,
        if have_t { "reconstructed(bench json + 측정 도구 원문 lite 인자)".into() } else { "reconstructed(bench json)".into() },
    ))
}

const RESTART_MARKS: [&str; 2] = ["Initializing a V1 LLM engine", "Starting vLLM server"];
fn is_restart(line: &str) -> bool {
    RESTART_MARKS.iter().any(|m| line.contains(m))
}

/// `_pre_sweep_requests` — bounded observed access lines before the final sweep.
pub fn _pre_sweep_requests(c: &_Ctx, measure_start: Option<&str>, session: &J) -> HR<Option<J>> {
    let sweep = or_dict(g(&c.ev, "sweep"));
    let sdir = if sweep.is_dict() { sweep.get_str("dir").map(str::to_string) } else { None };
    let Some(ms) = measure_start.filter(|x| !x.is_empty()) else { return Ok(None) };
    if !session.truthy() {
        return Ok(None);
    }
    let Some(sdir) = sdir else { return Ok(None) };
    if !c.repo.join(&sdir).is_dir() {
        return Ok(None);
    }
    let y0 = match core::parse_utc(ms) {
        Ok(v) => v,
        Err(e) if !e.is_py() => return Ok(None),
        Err(e) => return Err(e),
    };
    let timestamp = |line: &str| -> Option<String> {
        let tm = re_engine_ts().search(line)?;
        let n = |i| pyre::group(&tm, i).unwrap_or("0").parse::<i64>().unwrap_or(0);
        let (mo, d, h, mi, se) = (n(1), n(2), n(3), n(4), n(5));
        let y = y0.year - if (mo, d) > (y0.month as i64, y0.day as i64) { 1 } else { 0 };
        if !valid_dt(y, mo, d, h, mi, se) {
            return None;
        }
        Some(format!("{y:04}-{mo:02}-{d:02}T{h:02}:{mi:02}:{se:02}Z"))
    };
    let mut captures: Vec<(PathBuf, Vec<String>)> = Vec::new();
    let mut epoch_start = s(&idx(session, "start_utc")?);
    for name in [format!("lite_engine_head_{}.log", c.cell), format!("lite_engine_{}.log", c.cell)] {
        let f = c.repo.join(&sdir).join(&name);
        if f.is_file() {
            let body = String::from_utf8_lossy(&std::fs::read(&f).map_err(|e| core::io_err(&e, &f))?).into_owned();
            let lines: Vec<String> = body.split('\n').map(|l| re_ansi().sub_literal(l, "")).collect();
            for line in &lines {
                if is_restart(line) {
                    if let Some(t) = timestamp(line) {
                        if epoch_start.as_str() <= t.as_str() && t.as_str() < ms {
                            epoch_start = t;
                        }
                    }
                }
            }
            captures.push((f, lines));
        }
    }
    let mut seen: Vec<((String, String), i64)> = Vec::new();
    let mut srcs: Vec<String> = Vec::new();
    for (f, lines) in &captures {
        let mut pending: Vec<String> = Vec::new();
        let mut lower: Option<String> = None;
        let mut counts: Vec<((String, String), i64)> = Vec::new();
        let mut hit = false;
        for clean in lines {
            if re_post_req().is_search(clean) {
                if lower.is_some() {
                    pending.push(py_strip(clean).to_string());
                }
                continue;
            }
            let Some(ub) = timestamp(clean) else {
                if re_engine_ts().is_search(clean) {
                    pending.clear();
                    lower = None;
                }
                continue;
            };
            let restart = is_restart(clean);
            if let Some(lo) = &lower {
                if !restart && epoch_start.as_str() <= lo.as_str() && lo.as_str() <= ub.as_str() && ub.as_str() < ms {
                    for p_ in &pending {
                        let k = (p_.clone(), ub.clone());
                        match counts.iter_mut().find(|(a, _)| *a == k) {
                            Some(e) => e.1 += 1,
                            None => counts.push((k, 1)),
                        }
                    }
                    hit = hit || !pending.is_empty();
                }
            }
            pending.clear();
            lower = if epoch_start.as_str() <= ub.as_str() && ub.as_str() < ms { Some(ub) } else { None };
        }
        if hit {
            srcs.push(rel_posix(&c.repo, f)?);
        }
        for (k, v) in counts {
            match seen.iter_mut().find(|(a, _)| *a == k) {
                Some(e) => {
                    if v > e.1 {
                        e.1 = v;
                    }
                }
                None => seen.push((k, v)),
            }
        }
    }
    let count: i64 = seen.iter().map(|x| x.1).sum();
    if count <= SMOKE_INFERENCE_REQUESTS {
        return Ok(None);
    }
    let mut ubs: Vec<String> = seen.iter().map(|((_, u), _)| u.clone()).collect();
    ubs.sort();
    Ok(Some(jd! {"count" => count, "first_ub" => ubs[0].clone(), "last_ub" => ubs[ubs.len() - 1].clone(), "sources" => srcs,
        "session_id" => idx(session, "session_id")?, "session_source" => idx(session, "source")?,
        "epoch_start_utc" => epoch_start,
        "scope" => "current-session timestamp-bounded access lines; not total requests"}))
}

struct Steps(Vec<J>);
impl Steps {
    #[allow(clippy::too_many_arguments)]
    fn step(&mut self, name: &str, command: &str, success: &str, start: Option<String>, end: Option<String>, bound: &str, source: &str,
            command_source: &str, extra: Option<J>) -> HR<()> {
        let secs = _secs_between(start.as_deref(), end.as_deref())?;
        let mut obs = jd! {"start_utc" => J::from(start.clone()), "end_utc" => J::from(end.clone()), "seconds" => J::from(secs),
                           "bound" => if secs.is_some() { bound } else { "none" }};
        if let Some(J::Dict(m)) = extra {
            for (k, v) in m.iter() {
                obs.set(k, v.clone());
            }
        }
        let duration = match secs {
            Some(_) => _fmt_duration(secs, bound)?,
            None => match (nz(start.clone()), nz(end.clone())) {
                (None, None) => "미관측".into(),
                (a, b) => format!("미관측(관측 시각: {})", a.or(b).unwrap()),
            },
        };
        self.0.push(jd! {"order" => (self.0.len() + 1) as i64, "step" => name, "phase" => name, "command" => command,
            "command_source" => command_source, "observed" => obs, "duration" => duration, "success" => success, "source" => source});
        Ok(())
    }
}
fn nz(x: Option<String>) -> Option<String> {
    x.filter(|v| !v.is_empty())
}

/// `_reproduce` — command order + per-step observed durations.
pub fn _reproduce(c: &_Ctx, sel: &J, plane: &str, src_ctx: Option<&_Ctx>) -> HR<Vec<J>> {
    let repo = &c.repo;
    let cell = c.cell.clone();
    let topo = c.topo.clone();
    let mut st = Steps(Vec::new());
    let rd_rel = core::REL_RENDER_DOCKERFILE;
    let (mut measure_start, mut measure_src) = _first_measure_utc(repo, &c.ev)?;
    let sweep = or_dict(g(&c.ev, "sweep"));
    let mut sweep_end: Option<String> = if sweep.is_dict() { sweep.get_str("generated_utc").map(str::to_string) } else { None };
    let gen_raw = if sweep.is_dict() { g(&sweep, "generated_utc") } else { J::Null };
    if let (Some(ms), Some(se)) = (&measure_start, gen_raw.as_str()) {
        if ms.as_str() > se {
            measure_src = Some(format!("{}(date 를 UTC 로 읽으면 generated_utc 뒤 — 시계 가정 불성립 · 버림)", measure_src.clone().map(|x| x).unwrap_or("None".into())));
            measure_start = None;
        }
    }
    let host_start = measure_src.as_ref().map(|m| m.contains(HOST_START_MARK)).unwrap_or(false);
    if measure_src.as_ref().map(|m| !m.is_empty()).unwrap_or(false) && nz(measure_start.clone()).is_some() && !host_start {
        measure_src = Some(format!("{} · date=컨테이너 시계(UTC 가정)", measure_src.unwrap()));
    }
    let mut bench_bound = if host_start { "exact".to_string() } else { CLOCK_MIXED.to_string() };
    let (mut lite_end, mut lite_src, mut lite_start): (Option<String>, Option<String>, bool) = (None, None, false);
    if !gen_raw.truthy() {
        if let Some(lw) = _lite_window(c)?.filter(J::truthy) {
            measure_start = lw.get_str("start_utc").map(str::to_string);
            measure_src = Some(s(&g(&lw, "start_source")));
            lite_start = true;
            if g(&lw, "end_utc").truthy() {
                sweep_end = Some(s(&lw["end_utc"]));
                lite_end = sweep_end.clone();
                lite_src = Some(s(&g(&lw, "end_source")));
                let es = g(&lw, "end_source");
                bench_bound = if es.truthy() && s(&es).contains("컨테이너 시계") { CLOCK_MIXED.into() } else { "exact".into() };
            }
        } else {
            let (ls_, lsrc_) = _lite_measured_utc(repo, &c.ev)?;
            if nz(ls_.clone()).is_some() {
                measure_start = ls_;
                measure_src = Some(lsrc_);
                lite_start = true;
            }
        }
    } else if host_start {
        lite_start = true;
    }
    let session = current_serve_window(repo, &c.ev, measure_start.as_deref(), sweep_end.as_deref())?;
    if plane == "docker" {
        let serve_cmd;
        if topo == "multi" {
            let rrev = _renderer_revision(c)?;
            let mut l = _renderer_drift_lines(&J::from(rrev.clone()))?;
            l.push(format!("bash {rd_rel} --materialize-configs --topology multi"));
            l.push(format!("bash {rd_rel} --materialize-env --topology multi --manifest output/multi/manifest.yaml"));
            l.push(format!("bash {rd_rel} --cluster-envfile --topology multi --manifest output/multi/manifest.yaml -o output/multi/envs/.env.cluster"));
            l.push(format!("bash {rd_rel} --nccl-envfile --manifest output/multi/manifest.yaml -o output/multi/envs/.env.interconnect"));
            st.step("render", &l.join("\n"), "러너 3종·.env·.env.cluster·.env.interconnect 생성(렌더러 fail-loud)", None, None, "none",
                    &format!("{REL_SMOKE_MULTI} 선행조건 안내 · {rd_rel} CLI"), "derived(렌더러 CLI)", None)?;
            if let Some(r) = rrev {
                let row = _tracked_row(&r)?;
                st.0.last_mut().unwrap().set("renderer_revision", row);
            }
            serve_cmd = format!("bash {REL_SMOKE_MULTI} {cell} --keep-up");
        } else {
            serve_cmd = format!("bash {REL_SMOKE_SINGLE} {cell}");
        }
        let (mut build_cmd, mut build_src) = if sel.truthy() { _build_command(c, sel)? } else { (String::new(), "unobserved(빌드 선택자 없음)".into()) };
        if let Some(parent) = _parent_build(c)? {
            let hs = s(&parent["doc"]["historical_scope"]);
            let pref = s(&parent["ref"]);
            for pstep in ["parent_source_core", "alias"] {
                st.step(pstep, &s(&parent["doc"]["commands"][pstep]), "generated-unverified: 새 양노드 local reproduction 예시 · 미실행",
                        None, None, "none", &hs, &format!("generated-unverified(explicit manifest {pref})"), None)?;
            }
            if parent["extension"].truthy() {
                st.step("intermediate_deps2", &s(&parent["extension"]["intermediate"]["command"]),
                        "generated-unverified: explicit observed intermediate inputs; new local build not run", None, None, "none", &hs,
                        &format!("generated-unverified(explicit manifest {pref})"), None)?;
            }
            build_cmd = s(&parent["doc"]["commands"]["leaf"]);
            build_src = format!("generated-unverified(explicit manifest {pref} · leaf ledger args; full original CLI unavailable)");
        }
        let (p_start, p_end, p_src) = _phase_span(repo, &c.ev, &c.node, "build", &cell)?;
        let created = _image_created(c)?;
        let span = if sel.truthy() { memo_dockerfile(c, &s(&sel["dockerfile"]))["layer_span"].clone() } else { J::Null };
        let extra = if span.truthy() { Some(jd! {"layer_span" => span.clone()}) } else { None };
        if nz(p_start.clone()).is_some() && nz(p_end.clone()).is_some() {
            st.step("build", &build_cmd, "재현 예시 · 측정 이미지와 동일 여부 별도 확인", p_start, p_end, "exact",
                    &format!("{} · 기록된 phase 구간(이 재구성 명령 또는 측정 이미지의 전체 빌드 소요로 결속되지 않음)", p_src.unwrap_or("None".into())),
                    &build_src, extra)?;
        } else if span.truthy() && g(&span, "oldest_utc").truthy() && g(&span, "newest_utc").truthy() {
            st.step("build", &build_cmd, "빌드 OK · 이미지 실재", Some(s(&span["oldest_utc"])), Some(s(&span["newest_utc"])), LAYER_WINDOW,
                    &format!("{} · 우리 Dockerfile 층 {}개의 CreatedAt 첫 층 → 끝 층(캐시 재사용 층은 이전 빌드 시각 — 빌드 소요 아님)",
                             s(&g(&span, "source")), s(&g(&span, "layers"))),
                    &build_src, extra)?;
        } else {
            let src = if nz(created.clone()).is_some() {
                format!("docker image inspect {} .Created(종료만 관측)", _image_ref(c)?.0.unwrap_or("None".into()))
            } else {
                "미관측".into()
            };
            st.step("build", &build_cmd, "빌드 OK · 이미지 실재", None, created, "none", &src, &build_src, extra)?;
        }
        let (s_end, s_end_src) = if nz(measure_start.clone()).is_some() {
            let ms = measure_src.clone().unwrap_or("None".into());
            (measure_start.clone(), Some(if lite_start { format!("첫 측정 시각({ms})") } else { format!("첫 측정 date({ms})") }))
        } else {
            (None, None)
        };
        let bound = match &session {
            Some(sv) => Some(sv.clone()),
            None => _serve_budget_window(c, measure_start.as_deref(), sweep_end.as_deref())?,
        };
        let have_b = bound.is_some() && s_end.is_some();
        let src = if have_b {
            format!(
                "{} {} → {}",
                s(&bound.as_ref().unwrap()["source"]),
                if session.is_some() { "started_utc(측정을 포함하는 세션)" } else { "" },
                s_end_src.clone().unwrap()
            )
        } else {
            _unselected_serve_window_reason(c)?
        };
        let diag = memo_get(c, "serve_budget_window_diagnostic");
        let extra = if bound.is_none() && diag.as_ref().map(J::truthy).unwrap_or(false) {
            Some(jd! {"budget_window_selection" => diag.unwrap()})
        } else {
            None
        };
        st.step("serve", &serve_cmd, "health 200 + 추론 1회(스모크 판정)", bound.as_ref().map(|b| s(&b["start_utc"])), s_end,
                if have_b { "upper" } else { "none" }, &src, "derived(스모크 사용법)", extra)?;
    } else {
        if let (Some(sc), true) = (src_ctx, sel.truthy()) {
            let (b_cmd, b_src) = _build_command(sc, sel)?;
            let cr = _image_created(sc)?;
            let src = if nz(cr.clone()).is_some() {
                format!("docker image inspect {} .Created(종료만 관측)", _image_ref(sc)?.0.unwrap_or("None".into()))
            } else {
                "미관측".into()
            };
            st.step("build", &b_cmd, "원천 이미지 빌드 OK(각 노드 로컬 · 이미지 전송 ✗)", None, cr, "none", &src, &format!("{b_src} · 원천 셀 {}", sc.cell), None)?;
        }
        let nt = _native_run_timing(c)?;
        let ns = |k: &str| -> Option<String> { nt.get(k).filter(|v| v.truthy()).map(s) };
        let isrc = ns("install_src").unwrap_or("native producer preserved build_recipe".into());
        st.step("install", "artifacts/build_recipe/native-install.sh + pip-freeze-main.txt + pip-freeze-sub.txt",
                "venv 에서 `python -c 'import vllm'` 성공", ns("created"), ns("serve_start"),
                if ns("created").is_some() && ns("serve_start").is_some() { "upper" } else { "none" }, &isrc, "shipped-file",
                if nt.get("wheelhouse").map(J::truthy).unwrap_or(false) { Some(jd! {"wheelhouse_build_seconds" => nt["wheelhouse"].clone()}) } else { None })?;
        let (serve_cmd, serve_src) = _native_serve_command(c)?;
        st.step("serve", &serve_cmd, "health 200 + 추론 1회 + cleanup attestation PASS", ns("serve_start"), ns("up_completed"),
                if ns("serve_start").is_some() && ns("up_completed").is_some() { "exact" } else { "none" },
                &ns("serve_src").unwrap_or("미관측".into()), &serve_src, None)?;
        if nz(measure_start.clone()).is_none() {
            if let (Some(up), Some(se)) = (ns("up_completed"), nz(sweep_end.clone())) {
                if up <= se {
                    measure_start = Some(up);
                    measure_src = Some(format!("run state up_completed_utc({} · bench 는 up 완료 뒤 — 상한 시작)", s(&g(&nt, "state_rel"))));
                    bench_bound = "upper".into();
                }
            }
        }
    }
    let (bench_cmd, bench_src) = _bench_commands(c)?;
    let ms_ok = nz(measure_start.clone()).is_some();
    let se_ok = nz(sweep_end.clone()).is_some();
    let msrc = measure_src.clone().unwrap_or("None".into());
    let bsrc = if nz(lite_end.clone()).is_some() {
        format!("시작 = {msrc} → 끝 = {}", lite_src.clone().unwrap_or("None".into()))
    } else if ms_ok && se_ok {
        if bench_bound == "upper" {
            format!("{msrc} → sweep_index.generated_utc(측정 JSON date 는 호스트 로컬 시계 — UTC 가정 불성립으로 쓰지 않는다)")
        } else if host_start {
            format!("{msrc} → sweep_index.generated_utc(호스트 UTC · 두 끝 같은 시계)")
        } else {
            format!("{msrc} date → sweep_index.generated_utc(호스트 UTC — 시작과 시계 다름 · 근사)")
        }
    } else if ms_ok {
        format!("시작만 관측 — {msrc}")
    } else {
        "미관측".into()
    };
    st.step("bench", &bench_cmd, "리포트 발행(루브릭 권한 explicit ∧ PASS일 때만 full 인증서)", measure_start.clone(), sweep_end.clone(),
            if ms_ok && se_ok { &bench_bound } else { "none" }, &bsrc, &bench_src, None)?;
    let pre = _pre_sweep_requests(c, measure_start.as_deref(), &J::from(session.clone()))?;
    if let Some(pre) = pre {
        let note = pre_sweep_note(&pre)?;
        for stp in st.0.iter_mut() {
            let name = s(&stp["step"]);
            if (name == "bench" || name == "serve") && !stp["observed"]["seconds"].is_null() {
                let head = if name == "bench" { "최종 스윕 기준 · " } else { "끝 = 최종 스윕 첫 측정 · " };
                let d = format!("{}({head}그 전 요청 {}건 — 소요 출처 칸)", s(&stp["duration"]), s(&pre["count"]));
                stp.set("duration", d);
                let src = format!("{} · ⚠ {note}", s(&stp["source"]));
                stp.set("source", src);
                stp.get_mut("observed").unwrap().set("pre_final_sweep_requests", pre.clone());
            }
        }
    }
    Ok(st.0)
}

/// `_native_evidence_dir`.
pub fn _native_evidence_dir(c: &_Ctx) -> HR<Option<PathBuf>> {
    let sp = or_dict(g(&c.ev, "serve_proof"));
    let raw = if sp.is_dict() { g(&sp, "evidence_dir") } else { J::Null };
    let Some(raw) = raw.as_str() else { return Ok(None) };
    if py_strip(raw).is_empty() || raw.starts_with('/') || pure_parts(raw).iter().any(|p| p == "..") {
        return Ok(None);
    }
    let d = c.repo.join(py_strip(raw));
    Ok(if d.is_dir() { Some(d) } else { None })
}

/// `_native_run_timing` — observed times of the native run.
pub fn _native_run_timing(c: &_Ctx) -> HR<J> {
    let Some(d) = _native_evidence_dir(c)? else { return Ok(J::dict()) };
    let mut out = Map::new();
    let stp = d.join("logs").join("state.json");
    let st = _json_file(&stp)?.unwrap_or(J::dict());
    let sp = or_dict(g(&c.ev, "serve_proof"));
    let created = _utc_z(st.get("created_utc").and_then(J::as_str))?;
    let upc = _utc_z(st.get("up_completed_utc").and_then(J::as_str))?;
    let ready = if g(&sp, "health").is_dict() { g(&sp["health"], "ready_after_s") } else { J::Null };
    let state_rel = core::rel(&c.repo, &stp)?;
    out.insert("created", J::from(created.clone()));
    out.insert("up_completed", J::from(upc.clone()));
    out.insert("state_rel", state_rel.clone());
    let mut wh = Map::new();
    for n in ["main", "sub"] {
        let w = _json_file(&d.join(format!("wheelhouse-build-{n}.json")))?.unwrap_or(J::dict());
        let e = g(&w, "elapsed_s_monotonic");
        if e.is_number() || e.is_bool() {
            wh.insert(n, e);
        }
    }
    if !wh.is_empty() {
        out.insert("wheelhouse", J::Dict(wh.clone()));
    }
    if let (Some(u), Some(r)) = (&upc, ready.as_f64()) {
        if r >= 0.0 {
            let ri = r.trunc() as i64;
            let t = core::parse_civil(u).map(|cv| cv.epoch()).unwrap_or(0) - ri;
            out.insert("serve_start", format!("{}Z", core::Civil::from_epoch(t).iso()));
            out.insert("serve_src", format!(
                "serve proof health.ready_after_s={ri}(health 200 까지 · 관측) · 끝 = {state_rel} up_completed_utc(추론 1회 · proof 포함) · 시작 = 끝 − ready_after_s(파생)"));
        }
    }
    if created.as_ref().map(|x| !x.is_empty()).unwrap_or(false) && out.get("serve_start").map(J::truthy).unwrap_or(false) {
        let mut ks: Vec<(&str, &J)> = wh.iter().collect();
        ks.sort_by(|a, b| a.0.cmp(b.0));
        let whs = if wh.is_empty() {
            String::new()
        } else {
            format!(
                " · wheelhouse 재포장 {}(wheelhouse-build-<노드>.json elapsed_s_monotonic)",
                ks.iter().map(|(n, v)| format!("{n} {}s", s(v))).collect::<Vec<_>>().join(" · ")
            )
        };
        out.insert("install_src", format!(
            "{state_rel} created_utc → serve 시작(파생) — 예산·RAM 게이트 · wheelhouse 재포장 · venv 설치 · Ray 기동을 합친 상한{whs}"));
    }
    Ok(J::Dict(out))
}

/// `_native_serve_command`.
pub fn _native_serve_command(c: &_Ctx) -> HR<(String, String)> {
    let sp = or_dict(g(&c.ev, "serve_proof"));
    let sp = if sp.is_dict() { sp } else { J::dict() };
    let cell = if g(&sp, "cell").truthy() { s(&sp["cell"]) } else if g(&sp, "config").truthy() { s(&sp["config"]) } else { c.cell.clone() };
    let run_id = g(&sp, "run_id");
    let src_cell = g(&sp, "source_cell");
    if !run_id.truthy() {
        return Ok((
            format!("bash {REL_NATIVE_SERVE} up --cell {cell} --run-id <run-id> --apply  # run_id 미관측"),
            "derived(native_multinode_serve.py CLI · serve proof run_id 없음)".into(),
        ));
    }
    let rid = s(&run_id);
    let up = format!(
        "bash {REL_NATIVE_SERVE} up --cell {cell} --run-id {rid}{} --apply",
        if src_cell.truthy() { format!(" --source-cell {}", s(&src_cell)) } else { String::new() }
    );
    let lines = [
        "# native 정문 up: 양 노드 RAM 게이트 → 예산 → 각 노드가 자기 이미지에서 wheelhouse 재포장 → 오프라인 venv 설치 → Ray head/worker → vllm serve(native 트리플렛) → health · 추론 1회 → serve proof(서빙을 띄운 채 exit 0)".to_string(),
        up,
        format!("# 측정 뒤 정리(cleanup attestation): bash {REL_NATIVE_SERVE} down --cell {cell} --run-id {rid} --apply"),
    ];
    Ok((lines.join("\n"), "reconstructed(serve proof cell · run_id · source_cell + native_multinode_serve.py up/down CLI)".into()))
}

/// `reproduce_steps` — command order + observed durations per step (SPEC §5.6).
pub fn reproduce_steps(repo: &Path, ev: &J, runner: &J, forward_module: &J) -> HR<Vec<J>> {
    let c = _ctx(repo, ev, runner, forward_module, &J::Null)?;
    let plane = _plane(&c)?.0;
    let sel = if plane == "docker" { _applied(&c, false)?.1 } else { J::Null };
    _reproduce(&c, &sel, &plane, None)
}

// ── 파일 단위 검증 · 관련성 표시 · native 원천 이미지 ─────────────────────────────────────────
/// `_slot_machine_reasons` — machine-derived per-slot reasons (`machine_reason`); mutates `slots`.
pub fn _slot_machine_reasons(c: &_Ctx, slots: &mut J, applied: &J, sel: &J, plane: &str, vctx: &_Ctx) -> HR<()> {
    let rows: Vec<J> = or_dict(applied.clone())["patches"].items().iter().filter(|r| r.is_dict()).cloned().collect();
    let Some(m) = slots.as_dict_mut() else { return Ok(()) };
    for (name, row) in m.0.iter_mut() {
        if !row.is_dict() {
            continue;
        }
        let ev = or_dict(g(row, "evidence"));
        let files = g(row, "files").truthy();
        let mut why: Option<String> = None;
        match name.as_str() {
            "fork_pin" => {
                let repo_ = vctx.env.get("VLLM_REPO").filter(|v| v.truthy()).map(s);
                why = Some(if !files {
                    format!("셀 env 에 VARIANT 줄 없음 = stock(workflow.md §변종 좌표의 거처){}", repo_.map(|r| format!(" · VLLM_REPO={r}")).unwrap_or_default())
                } else {
                    format!("셀 env VARIANT → {} 변종 좌표", s(&g(&ev, "ref")))
                });
            }
            "runtime_patch" => {
                why = Some(if !files {
                    format!("{} 없음 — arm_patch.sh 는 no-op(런타임 패치 불해당)", s(&g(&ev, "ref")))
                } else {
                    format!("{} 실재 — 셀 이름의 런타임 패치", s(&g(&ev, "ref")))
                });
            }
            "build_patch_pre" | "build_patch_post" => {
                let ph = if name.ends_with("pre") { "pre" } else { "post" };
                let mine: Vec<&J> = rows.iter().filter(|r| r.get_str("phase") == Some(ph)).collect();
                if !mine.is_empty() {
                    let mut cnt: Vec<(String, i64)> = Vec::new();
                    for r in &mine {
                        let k = s(&g(r, "result"));
                        match cnt.iter_mut().find(|(a, _)| *a == k) {
                            Some(e) => e.1 += 1,
                            None => cnt.push((k, 1)),
                        }
                    }
                    cnt.sort();
                    why = Some(format!(
                        "{}/ — 적용 판정 {}: {}{}",
                        s(&PATCH_DIRS()[ph]),
                        s(&g(&ev, "kind")),
                        cnt.iter().map(|(k, v)| format!("{k} {v}")).collect::<Vec<_>>().join(" · "),
                        if files { " · 실린 것 = skip 아닌 패치" } else { "" }
                    ));
                } else if !files {
                    let n = g(&ev, "note");
                    let r = g(&ev, "ref");
                    why = Some(format!("적용 판정 없음 — {}", if n.truthy() { s(&n) } else if r.truthy() { s(&r) } else { "원천 미관측".into() }));
                }
                let asym = or_dict(applied.clone())["node_asymmetry"].clone();
                if let Some(t) = node_asymmetry_phase_text(&asym, ph)? {
                    why = Some(format!("{} · {t} — 실림·판정은 대표 원장 기준", why.unwrap_or("대표 원장 기준".into())));
                }
            }
            "build_recipe" => {
                if sel.truthy() && g(sel, "dockerfile").truthy() {
                    let mut w = format!("쓰인 Dockerfile 선택자 {}({}) + 그것이 COPY 하는 파일", s(&sel["dockerfile"]), s(&g(sel, "dockerfile_source")));
                    if plane == "native" {
                        w = format!("원천 이미지 {w} · native 설치 입력(serve proof 보존 파일)");
                    }
                    why = Some(w);
                }
            }
            "compose" => {
                if plane == "native" {
                    let nsc = g(&ev, "native_source_cell");
                    why = Some(format!(
                        "Docker 형 참고(원천 셀 {} · 이 셀 run 아님 · generated-unverified) — 이 셀은 native 정문으로 기동했다",
                        if nsc.truthy() { s(&nsc) } else { "미관측".into() }
                    ));
                } else if files {
                    why = Some(format!("{} 서빙 경로(compose · 러너 · env 형상) — 측정 당시 개정(selected_revisions)", c.topo));
                }
            }
            "triplet" => {
                if files {
                    why = Some(format!("셀 id {cl} = 트리플렛 이름(configs/{cl}.yaml · .sh · envs/.env.{cl})", cl = c.cell));
                }
            }
            _ => {}
        }
        if let Some(w) = why.filter(|w| !w.is_empty()) {
            row.set("machine_reason", w);
        }
    }
    Ok(())
}

/// `_native_launch_path` — the native launch record (serve proof `native_launch_path`).
pub fn _native_launch_path(c: &_Ctx) -> HR<Option<(PathBuf, String, String)>> {
    let sp = g(&c.ev, "serve_proof");
    let r0 = g(&c.ev, "native_launch_path");
    let raw = if r0.truthy() { r0 } else if sp.is_dict() { g(&sp, "native_launch_path") } else { J::Null };
    let Some(raw) = raw.as_str() else { return Ok(None) };
    if py_strip(raw).is_empty() || Path::new(raw).is_absolute() {
        return Ok(None);
    }
    let fp = c.repo.join(core::rel(&c.repo, Path::new(raw))?);
    if !fp.is_file() || is_symlink(&fp) {
        return Ok(None);
    }
    let suf = pure_suffix(&core::posix(&fp));
    let name = if suf == ".md" { NATIVE_LAUNCH_NAME.to_string() } else { format!("native-launch{}", if suf.is_empty() { ".txt".into() } else { suf }) };
    Ok(Some((fp, name, "native_launch_path".into())))
}

/// `_native_source` — the native cell's source Docker cell · image.
pub fn _native_source(c: &_Ctx) -> HR<J> {
    let sp = or_dict(g(&c.ev, "serve_proof"));
    let sp = if sp.is_dict() { sp } else { J::dict() };
    let wh = if g(&sp, "wheelhouse").is_dict() { g(&sp, "wheelhouse") } else { J::dict() };
    let src = g(&sp, "source_cell");
    let img = g(&wh, "source_image_tag");
    let mut out = jd! {"ok" => false, "cell" => if src.is_str() { src.clone() } else { J::Null },
        "image_tag" => if img.is_str() { img.clone() } else { J::Null },
        "source" => "serve_proof.source_cell × serve_proof.wheelhouse.source_image_tag"};
    let src_s = src.as_str().unwrap_or("").to_string();
    let img_s = img.as_str().unwrap_or("").to_string();
    if src_s.is_empty() || src_s.contains('/') || src_s.starts_with('.') || img_s.is_empty() {
        out.set("why", "serve proof 에 source_cell·wheelhouse.source_image_tag 가 없다(원천 이미지 미관측)");
        return Ok(out);
    }
    let trip = [
        ("yaml", c.out.join("configs").join(format!("{src_s}.yaml"))),
        ("sh", c.out.join("configs").join(format!("{src_s}.sh"))),
        ("env", c.out.join("envs").join(format!(".env.{src_s}"))),
    ];
    let absent: Vec<&str> = trip.iter().filter(|(_, p)| !p.is_file()).map(|(k, _)| *k).collect();
    if !absent.is_empty() {
        out.set("why", format!("원천 셀 {src_s} 트리플렛 부재 {}", J::from(absent).repr()));
        return Ok(out);
    }
    let tag = owner_read_env(c, &trip[2].1)?.get("IMAGE_TAG").cloned().unwrap_or(J::Null);
    if tag.as_str() != Some(img_s.as_str()) {
        out.set("why", format!("원천 셀 env IMAGE_TAG={} ≠ wheelhouse 원천 이미지 {}", tag.repr(), img.repr()));
        return Ok(out);
    }
    let bi = or_dict(g(&c.ev, "build_identity"));
    out.set("evidence_marker", if bi.is_dict() { g(&bi, "native_wheelhouse_source").truthy() } else { false });
    if bi.is_dict() && g(&bi, "image_tag").truthy() && g(&bi, "image_tag").as_str() != Some(img_s.as_str()) {
        out.set("why", format!("evidence build_identity.image_tag={} ≠ wheelhouse 원천 이미지 {}", g(&bi, "image_tag").repr(), img.repr()));
        return Ok(out);
    }
    let pat = format!(r"envs/\.env\.{} \(sha256=([0-9a-f]{{8,64}})\)", re_escape(&src_s));
    let rx = Re::new(&pat, 0);
    let env_text = _read(&trip_path(c, "env"))?;
    let mm = rx.search(&env_text).map(|m| pyre::group(&m, 1).unwrap_or("").to_string());
    out.set("render_header_sha_match", match mm {
        Some(h) => J::Bool(core::sha256_file(&trip[2].1)?.starts_with(&h)),
        None => J::Null,
    });
    out.set("ok", true);
    out.set("why", J::Null);
    let mut tr = Map::new();
    let mut tp = Map::new();
    for (k, p) in &trip {
        tr.insert(*k, core::rel(&c.repo, p)?);
        tp.insert(*k, path_j(p));
    }
    out.set("triplet", J::Dict(tr));
    out.set("triplet_paths", J::Dict(tp));
    Ok(out)
}

/// `_source_ctx` — the source Docker cell context (cell · triplet · plane · build identity replaced).
pub fn _source_ctx(c: &_Ctx, ns: &J) -> HR<_Ctx> {
    let bi = or_dict(g(&c.ev, "build_identity"));
    let mut bim = Map::new();
    if let Some(m) = bi.as_dict() {
        for (k, v) in m.iter() {
            if k != "native_wheelhouse_source" {
                bim.insert(k, v.clone());
            }
        }
    }
    let mut over = Map::new();
    over.insert("cell", idx(ns, "cell")?);
    over.insert("triplet", idx(ns, "triplet")?);
    over.insert("plane", "docker");
    over.insert("build_identity", J::Dict(bim));
    let ev2 = _EvOverlay::new(&c.ev, &over).view();
    let sc = _ctx(&c.repo, &ev2, &J::Null, &J::Null, &J::Null)?;
    *sc.runner.borrow_mut() = c.runner.borrow().clone();
    sc.default_runner.set(c.default_runner.get());
    *sc.rd_loaded.borrow_mut() = *c.rd_loaded.borrow();
    Ok(sc)
}

/// `_model_facts` — the cell model's `architectures` · `model_type`.
pub fn _model_facts(c: &_Ctx) -> HR<J> {
    if let Some(v) = memo_get(c, "model_facts") {
        return Ok(v);
    }
    let archs = g(&c.ev, "model_architectures");
    let mtype = g(&c.ev, "model_type");
    let mut out = jd! {"observed" => false, "architectures" => J::Null, "model_type" => J::Null, "source" => "미관측"};
    if archs.is_list() && archs.truthy() {
        out.set("observed", true);
        out.set("architectures", J::from(strs_of(&archs)));
        out.set("model_type", if mtype.truthy() { J::str(s(&mtype)) } else { J::Null });
        out.set("source", "evidence.model_architectures");
    } else if is_cell_evidence(&c.ev) {
        let ck = match crate::evidence::_checkpoint(&c.repo, &c.ev) {
            Ok(v) => v,
            Err(e) => {
                let ty = if !e.is_py() { "HintError".to_string() } else { e.code.trim_start_matches("py:").to_string() };
                const OK: [&str; 9] = ["HintError", "AttributeError", "TypeError", "OSError", "ValueError", "FileNotFoundError",
                                       "PermissionError", "JSONDecodeError", "UnicodeDecodeError"];
                if !OK.contains(&ty.as_str()) {
                    return Err(e);
                }
                jd! {"observed" => false, "reason" => format!("체크포인트 관측 실패({ty})")}
            }
        };
        let cfg = if g(&ck, "observed").truthy() { g(&ck, "config") } else { J::Null };
        if cfg.is_dict() && g(&cfg, "architectures").is_list() {
            out.set("observed", true);
            out.set("architectures", J::from(strs_of(&cfg["architectures"])));
            out.set("model_type", if g(&cfg, "model_type").truthy() { J::str(s(&cfg["model_type"])) } else { J::Null });
            let lab = if g(&ck, "model_path").truthy() { s(&ck["model_path"]) } else if g(&ck, "host_label").truthy() { s(&ck["host_label"]) } else { "관측".into() };
            out.set("source", format!("체크포인트 config.json({lab})"));
        } else {
            let r = g(&ck, "reason");
            out.set("source", format!("미관측({})", if r.truthy() { s(&r) } else { "config.json architectures 없음".into() }));
        }
    }
    memo_set(c, "model_facts", out.clone());
    Ok(out)
}

/// `_engine_log_paths` — this cell's engine logs (non-empty regular files inside the repository).
pub fn _engine_log_paths(c: &_Ctx) -> HR<Vec<PathBuf>> {
    let mut rels: Vec<String> = Vec::new();
    if is_cell_evidence(&c.ev) {
        for q in crate::evidence::_measured_engine_logs_ex(&c.repo, &c.ev)?.0 {
            rels.push(core::rel(&c.repo, &q)?);
        }
    }
    let seeds = or_dict(g(&c.ev, "lineage_seeds"));
    if seeds.is_dict() {
        for x in or_list(g(&seeds, "evidence_candidates")).items() {
            if x.is_dict() && x.get_str("kind") == Some("engine_log") {
                if let Some(p) = x.get_str("path") {
                    rels.push(p.to_string());
                }
            }
        }
    }
    let extra = g(&c.ev, "engine_logs");
    if extra.is_list() {
        rels.extend(extra.items().iter().filter_map(|x| x.as_str().map(str::to_string)));
    }
    let sp = g(&c.ev, "serve_proof");
    let ed = if sp.is_dict() { g(&sp, "evidence_dir") } else { J::Null };
    if let Some(ed) = ed.as_str().filter(|e| !e.is_empty() && !Path::new(e).is_absolute()) {
        let d = c.repo.join(core::rel(&c.repo, Path::new(ed))?).join("logs");
        if d.is_dir() {
            let mut found: Vec<PathBuf> = std::fs::read_dir(&d)
                .map(|rd| rd.flatten().map(|e| e.path()).filter(|p| file_name(p).ends_with("vllm-serve.log")).collect())
                .unwrap_or_default();
            found.sort_by_key(|p| path_key(p));
            for q in found {
                if !file_name(&q).contains("watchdog") {
                    rels.push(core::rel(&c.repo, &q)?);
                }
            }
        }
    }
    let mut out = Vec::new();
    let mut seen: Vec<String> = Vec::new();
    for r in rels {
        if seen.contains(&r) {
            continue;
        }
        seen.push(r.clone());
        if Path::new(&r).is_absolute() || pure_name(&r).contains("watchdog") {
            continue;
        }
        let q = c.repo.join(core::rel(&c.repo, Path::new(&r))?);
        if q.is_file() && !is_symlink(&q) && std::fs::metadata(&q).map(|m| m.len() > 0).unwrap_or(false) {
            out.push(q);
        }
    }
    Ok(out)
}

/// `_relevance_context` — relevance inputs at once (`_logs` = absolute paths · `_per` = rel → {row, phase}).
pub fn _relevance_context(c: &_Ctx, slots: &J, applied: &J) -> HR<J> {
    let model = _model_facts(c)?;
    let prow: Vec<J> = or_dict(applied.clone())["patches"].items().iter().filter(|r| r.is_dict()).cloned().collect();
    let mut per = Map::new();
    for (slot, ph) in [("build_patch_pre", "pre"), ("build_patch_post", "post")] {
        for rel_f in or_dict(g(slots, slot))["files"].items() {
            let rel_f = s(rel_f);
            let name = pure_name(&rel_f);
            let row = prow
                .iter()
                .rev()
                .find(|r| r.get_str("phase") == Some(ph) && g(r, "file").as_str() == Some(name.as_str()))
                .cloned()
                .unwrap_or(J::dict());
            per.insert(rel_f, jd! {"row" => row, "phase" => ph});
        }
    }
    let logs = if !per.is_empty() { _engine_log_paths(c)? } else { Vec::new() };
    let mut read = Vec::new();
    for q in &logs {
        read.push(core::rel(&c.repo, q)?);
    }
    let mut mm = Map::new();
    for k in ["observed", "architectures", "model_type", "source"] {
        mm.insert(k, g(&model, k));
    }
    Ok(jd! {"model" => J::Dict(mm), "engine_logs" => jd! {"read" => read, "count" => logs.len()},
            "_logs" => J::List(logs.iter().map(|p| path_j(p)).collect()), "_per" => J::Dict(per)})
}

/// `_file_records` — `slots[<slot>]["file_records"]` per shipped file (mutates `slots`).
pub fn _file_records(c: &_Ctx, payload: &Path, slots: &mut J, marks: &J, applied: &J, rctx: &J) -> HR<()> {
    let _ = applied;
    let per = rctx["_per"].clone();
    let logs: Vec<PathBuf> = rctx["_logs"].items().iter().map(|p| PathBuf::from(s(p))).collect();
    let model = rctx["model"].clone();
    let mut texts = Map::new();
    let mut all_sigs: Vec<String> = Vec::new();
    if let Some(pm) = per.as_dict() {
        for rel_f in pm.keys() {
            let t = read_lossy(&payload.join(rel_f))?;
            for sg in _log_signatures(&t)? {
                if !all_sigs.contains(&sg) {
                    all_sigs.push(sg);
                }
            }
            texts.insert(rel_f, t);
        }
    }
    let mut hits: Vec<String> = Vec::new();
    let mut counts: Vec<(String, i64)> = Vec::new();
    for q in &logs {
        let body = String::from_utf8_lossy(&std::fs::read(q).map_err(|e| core::io_err(&e, q))?).into_owned();
        for sg in &all_sigs {
            let n = body.matches(sg.as_str()).count() as i64;
            if n > 0 {
                match counts.iter_mut().find(|(a, _)| a == sg) {
                    Some(e) => e.1 += n,
                    None => counts.push((sg.clone(), n)),
                }
                if !hits.contains(sg) {
                    hits.push(sg.clone());
                }
            }
        }
    }
    let cnt = |k: &str| counts.iter().find(|(a, _)| a == k).map(|x| x.1).unwrap_or(0);
    let log_rels: Vec<String> = logs.iter().map(|q| core::rel(&c.repo, q)).collect::<HR<_>>()?;
    let Some(sm) = slots.as_dict_mut() else { return Ok(()) };
    for (slot, row) in sm.0.iter_mut() {
        let ev = or_dict(g(row, "evidence"));
        let mut sel_by: Vec<(String, J)> = Vec::new();
        for sx in or_list(g(&ev, "selected_revisions")).items() {
            if sx.is_dict() {
                let n = pure_name(&s(&g(sx, "path")));
                match sel_by.iter_mut().find(|(a, _)| *a == n) {
                    Some(e) => e.1 = sx.clone(),
                    None => sel_by.push((n, sx.clone())),
                }
            }
        }
        let mut recs: Vec<J> = Vec::new();
        for rel_f in or_list(g(row, "files")).items().iter().map(s) {
            let m = or_dict(g(marks, &rel_f));
            let name = pure_name(&rel_f);
            let vb = g(&m, "verification_basis");
            let mut rec = jd! {"path" => rel_f.clone(), "sha256" => core::sha256_file(&payload.join(&rel_f))?,
                "verification" => m.get("verification").cloned().unwrap_or(J::str("verified")),
                "generated_by" => m.get("generated_by").cloned().unwrap_or(J::str("cell-run")),
                "verification_basis" => if vb.truthy() { vb } else { J::str(_default_basis(slot, row, &name)?) }};
            if g(&m, "header").truthy() {
                rec.set("header", m["header"].clone());
            }
            if m.get("frozen_source_sha256").is_some() {
                rec.set("frozen_source_sha256", m["frozen_source_sha256"].clone());
                rec.set("source_identity_basis", "raw frozen source validated before annotation; sha256 hashes shipped bytes including any recorded warning");
            }
            if let Some((_, sx)) = sel_by.iter().find(|(a, _)| *a == name) {
                rec.set("bytes_observed", g(sx, "verified").truthy());
            }
            if m.get("parent_source").is_some() {
                rec.set("parent_source", m["parent_source"].clone());
                let ps = m["parent_source"].as_str().unwrap_or("");
                rec.set("bytes_observed", !(ps == "README.md" || ps == "manifest.json") || !m["parent_source"].is_str());
            }
            if per.get(&rel_f).is_some() {
                let prow = per[rel_f.as_str()]["row"].clone();
                let text = texts.get(&rel_f).map(s).unwrap_or_default();
                let dmt = g(&prow, "declared_model_trigger");
                let trig = if dmt.truthy() { Some(s(&dmt)) } else { _patch_trigger(&text)? };
                let decl = _patch_declared(&text, trig.as_deref())?;
                let sigs = _log_signatures(&text)?;
                let fired = if !sigs.is_empty() && !logs.is_empty() { Some(sigs.iter().any(|x| hits.contains(x))) } else { None };
                let mut mine = Map::new();
                for sg in &sigs {
                    if cnt(sg) != 0 {
                        mine.insert(sg.clone(), cnt(sg));
                    }
                }
                let (rel, mut basis) = _relevance(&decl, &model, &sigs, fired, logs.len() as i64, &J::Dict(mine.clone()))?;
                if decl["shared_files"].truthy() {
                    basis.push_str(&format!(" · 편집하는 공유(모델 디렉터리 밖) 파일 {}", decl["shared_files"].repr()));
                }
                rec.set("relevance", rel);
                rec.set("relevance_basis", basis);
                rec.set("relevance_evidence", jd! {
                    "declared" => jd! {"archs" => decl["archs"].clone(), "modules" => decl["modules"].clone(),
                                      "general" => decl["general"].clone(), "shared_files" => decl["shared_files"].clone()},
                    "signatures" => sigs.iter().take(3).cloned().collect::<Vec<_>>(), "fired" => J::from(fired),
                    "engine_logs" => logs.len(), "fired_signatures" => J::Dict(mine), "engine_log_paths" => log_rels.clone(),
                });
                let w0 = g(&prow, "script_sha256");
                let want = if w0.truthy() { w0 } else { g(&prow, "image_script_sha256") };
                if want.truthy() {
                    rec.set("script_sha256_ledger", want.clone());
                    let eq = rec["sha256"] == want;
                    rec.set("bytes_observed", eq);
                }
                rec.set("applied_result", g(&prow, "result"));
            } else if m.get("verification").cloned().unwrap_or(J::str("verified")).as_str() == Some("verified") {
                rec.set("relevance", "required");
                rec.set("relevance_basis", "재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일)");
            } else {
                rec.set("relevance", "unknown");
                rec.set("relevance_basis", format!(
                    "파일의 실행 당시 바이트가 확인되지 않았다(generated-unverified · 생성 {}) — 이 표시만으로 경로 실행 여부나 필요 여부를 판정하지 않는다(참고 형상)",
                    s(&rec["generated_by"])));
            }
            for (key, vocab) in [("verification", &VERIFICATION_VALUES[..]), ("generated_by", &GENERATED_BY_VALUES[..]), ("relevance", &RELEVANCE_VALUES[..])] {
                if key == "generated_by" && generated_by_ok(&rec[key])? {
                    continue;
                }
                if !rec[key].as_str().map(|v| vocab.contains(&v)).unwrap_or(false) {
                    return core::fail("HINT_ARTIFACT_MARK_INVALID", format!("{rel_f}: {key}={} 가 어휘 {} 밖이다.", rec[key].repr(), tuple_repr(vocab)), None);
                }
            }
            if rec["verification"].as_str() == Some("generated-unverified") && rec.get("header").is_none() {
                return core::fail("HINT_ARTIFACT_MARK_INVALID", format!("{rel_f}: generated-unverified 인데 파일 헤더 경고가 없다(표시 3자리 불일치)."), None);
            }
            recs.push(rec);
        }
        row.set("file_records", J::List(recs));
    }
    Ok(())
}

/// `_empty_copy_dirs` — COPY'd patch dirs with no shipped patch file. `slots`: `J::Null` = None.
pub fn _empty_copy_dirs(c: &_Ctx, df_text: &str, slots: &J) -> HR<Vec<J>> {
    if df_text.is_empty() {
        return Ok(Vec::new());
    }
    let copies = _dockerfile_copies(df_text)?;
    let app = memo_applied(c);
    let ledger = app.as_ref().map(|a| a.at(2).clone()).unwrap_or(J::Null);
    let inv_all = if ledger.is_dict() && g(&ledger, "context_inventory").is_dict() { Some(g(&ledger, "context_inventory")) } else { None };
    let mut out = Vec::new();
    for ph in ["pre", "post"] {
        let d = s(&PATCH_DIRS()[ph]);
        let slot = format!("build_patch_{ph}");
        if !copies.contains(&d) {
            continue;
        }
        let shipped = app
            .as_ref()
            .map(|a| or_dict(a.at(0).clone())["patches"].items().iter().any(|r| r.is_dict() && r.get_str("phase") == Some(ph) && r.get_str("result") != Some("skipped")))
            .unwrap_or(false);
        if (!slots.is_null() && or_dict(g(slots, &slot))["files"].truthy()) || (slots.is_null() && shipped) {
            continue;
        }
        let inv = inv_all.as_ref().map(|i| g(i, &slot)).filter(|v| v.is_list());
        out.push(jd! {"dir" => d.clone(), "slot" => slot.clone(), "inventory" => J::from(inv.clone()),
            "source" => match &inv {
                Some(v) => format!("빌드 원장 context_inventory.{slot} = {}", v.repr()),
                None => "빌드 원장 context_inventory 미관측 — 빌드 때 내용 모름(실린 패치 없음)".into(),
            }});
    }
    Ok(out)
}

fn re_part() -> &'static Re {
    static C: OnceLock<Re> = OnceLock::new();
    re_cached(&C, r"[A-Za-z0-9_.-]+", 0)
}
fn re_sha() -> &'static Re {
    static C: OnceLock<Re> = OnceLock::new();
    re_cached(&C, r"sha256:[0-9a-f]{64}", 0)
}
fn invalid<T>(msg: impl Into<String>) -> HR<T> {
    core::fail("HINT_ARTIFACT_MARK_INVALID", msg, None)
}

/// `_parent_input` — one explicit parent build input (signature: `bytes` → `Vec<u8>`).
pub fn _parent_input(c: &_Ctx, rel: &str) -> HR<(PathBuf, Vec<u8>)> {
    let bad = rel.is_empty()
        || Path::new(rel).is_absolute()
        || rel.split('/').any(|p| p.is_empty() || p == "." || p == ".." || !re_part().is_full(p))
        || !rel.starts_with(&format!("output/{}/", c.topo));
    if bad {
        return invalid(format!("parent build input escapes the selected topology: {}", crate::value::py_repr(rel)));
    }
    let mut q = c.repo.clone();
    for part in pure_parts(rel) {
        q = q.join(part);
        if is_symlink(&q) {
            return invalid(format!("parent build input symlink: {rel}"));
        }
    }
    use std::os::unix::fs::MetadataExt;
    let ok = q.is_file()
        && std::fs::metadata(&q).map(|m| m.file_type().is_file() && m.nlink() == 1 && m.len() <= 2 * 1024 * 1024).unwrap_or(false);
    if !ok {
        return invalid(format!("parent build input must be a bounded regular UTF8 file: {rel}"));
    }
    let data = std::fs::read(&q).map_err(|e| core::io_err(&e, &q))?;
    if std::str::from_utf8(&data).is_err() {
        return invalid(format!("parent build input is not UTF8: {rel}"));
    }
    Ok((q, data))
}
fn jstr_input(c: &_Ctx, v: &J) -> HR<(PathBuf, Vec<u8>)> {
    match v.as_str() {
        Some(x) => _parent_input(c, x),
        None => invalid(format!("parent build input escapes the selected topology: {}", v.repr())),
    }
}
fn loads_b(b: &[u8]) -> HR<J> {
    crate::value::loads(&String::from_utf8_lossy(b)).map_err(|e| HintError::py("JSONDecodeError", e))
}
fn data_bytes(data: &J, k: &str) -> HR<Vec<u8>> {
    Ok(hex_dec(&s(&idx(data, k)?)))
}

/// `_parent_intermediate` — one explicit deps2 intermediate and the exact final leaf inventory (`data`: name → hex).
fn _parent_intermediate(c: &_Ctx, doc: &J, leaf: &J, alias: &J, mp: &Path, data: &mut J) -> HR<Option<J>> {
    let mid = g(doc, "intermediate_layer");
    let lc = g(doc, "leaf_context");
    if mid.is_null() && lc.is_null() {
        return Ok(None);
    }
    if !mid.is_dict() || !lc.is_dict() {
        return invalid("one intermediate requires its explicit final leaf inventory");
    }
    let inv = g(&mid, "files");
    let allowed: std::collections::BTreeSet<String> =
        ["Dockerfile.source-build-deps2", "build_patches/06-b12x-optional032.sh"].iter().map(|x| x.to_string()).collect();
    let cmd = g(&mid, "command");
    if !inv.is_dict() || keyset(&inv) != allowed || !re_sha().is_full(&s(&g(&mid, "image_digest")))
        || !cmd.as_str().map(|x| x.starts_with("# generated-unverified") && clen(x) <= 8192).unwrap_or(false)
    {
        return invalid("intermediate must declare exactly its deps2 recipe/POST06 and generated command");
    }
    let observed = or_dict(g(leaf, "parent_ledger"));
    let args = or_dict(g(&observed, "build_args"));
    let leaf_ba = or_dict(g(leaf, "ledger")).get("build_args").cloned().unwrap_or(J::dict());
    if !py_eq(&args, &g(&mid, "build_args")) || !py_eq(&g(&args, "BASE_IMAGE"), &g(alias, "alias")) || !py_eq(&g(&leaf_ba, "BASE_IMAGE"), &g(&mid, "image_tag")) {
        return invalid("intermediate BASE_IMAGE chain/observed ledger args differ");
    }
    for (target, item) in inv.as_dict().unwrap().iter() {
        if !item.is_dict() || !s(&g(item, "source")).starts_with(&format!("output/{}/cache/", c.topo)) {
            return invalid("intermediate source must be an explicit frozen topology cache file");
        }
        let (_, body) = jstr_input(c, &item["source"])?;
        if g(item, "sha256").as_str() != Some(core::sha256_bytes(&body).as_str()) {
            return invalid("intermediate source sha256 mismatch");
        }
        data.set(format!("intermediate-deps2/context/{target}"), hex_enc(&body));
    }
    let mp_parent = rel_posix(&c.repo, mp.parent().unwrap_or(Path::new(".")))?;
    for (name, field) in [("leaf-result-evidence.json", "identity_evidence"), ("leaf-build-command.txt", "build_command")] {
        let rec = g(&lc, field);
        if !rec.is_dict() || g(&rec, "source").as_str() != Some(format!("{mp_parent}/{name}").as_str()) {
            return invalid("leaf identity/argv requires explicit sibling evidence");
        }
        let (_, body) = jstr_input(c, &rec["source"])?;
        if g(&rec, "sha256").as_str() != Some(core::sha256_bytes(&body).as_str()) {
            return invalid("leaf identity/argv sha256 mismatch");
        }
        data.set(name, hex_enc(&body));
    }
    let ident = loads_b(&data_bytes(data, "leaf-result-evidence.json")?)?;
    let par = or_dict(g(&ident, "parent"));
    if !py_eq(&or_dict(g(&ident, "image"))["id"], &idx(doc, "selected_image_digest")?)
        || !py_eq(&g(&par, "id"), &idx(&mid, "image_digest")?)
        || !py_eq(&g(&par, "tag"), &g(&mid, "image_tag"))
    {
        return invalid("observed leaf/parent image identity chain differs");
    }
    let root = g(&lc, "root");
    let inventory = g(&lc, "files");
    let argv = shlex_split(&String::from_utf8_lossy(&data_bytes(data, "leaf-build-command.txt")?))?;
    let root_s = root.as_str().unwrap_or("").to_string();
    let fpos = argv.iter().position(|a| a == "-f");
    let bad = !root.is_str()
        || !root_s.starts_with(&format!("output/{}/cache/", c.topo))
        || argv.is_empty()
        || argv[argv.len() - 1] != root_s
        || fpos.is_none()
        || argv.get(fpos.unwrap() + 1).map(|x| x != &format!("{root_s}/Dockerfile")).unwrap_or(true)
        || !inventory.is_dict()
        || inventory.len() != 24;
    if bad {
        if fpos.is_some() && argv.get(fpos.unwrap() + 1).is_none() && root.is_str() && root_s.starts_with(&format!("output/{}/cache/", c.topo)) && !argv.is_empty() && argv[argv.len() - 1] == root_s {
            return Err(HintError::py("IndexError", "list index out of range"));
        }
        return invalid("leaf inventory must bind the actual final build argv/context");
    }
    let ledger = or_dict(g(leaf, "ledger"));
    let mut patches: Vec<((String, String), J)> = Vec::new();
    for r in or_list(g(&ledger, "patches")).items() {
        if r.is_dict() {
            let k = (s(&g(r, "phase")), s(&g(r, "file")));
            match patches.iter_mut().find(|(a, _)| *a == k) {
                Some(e) => e.1 = r.clone(),
                None => patches.push((k, r.clone())),
            }
        }
    }
    let required = ["Dockerfile", "build_patches_src/PROVENANCE.json", "build_patches_src/exl3_port_lib.py", "build_patches_src/.gitkeep",
                    "build_patches/.gitkeep", "build_patches/Dockerfile.source-build-deps2"];
    let mut expected: std::collections::BTreeSet<String> = required.iter().map(|x| x.to_string()).collect();
    for ((ph, name), _) in &patches {
        if ph == "pre" || ph == "post" {
            expected.insert(format!("{}{name}", if ph == "pre" { "build_patches_src/" } else { "build_patches/" }));
        }
    }
    if keyset(&inventory) != expected || patches.len() != 18 {
        return invalid("leaf inventory is not the observed 18 scripts plus six exact context files");
    }
    let mut reuse = Map::new();
    let mut support: Vec<(String, Vec<u8>)> = Vec::new();
    let mut frozen: Vec<(String, Vec<u8>)> = Vec::new();
    for (target, item) in inventory.as_dict().unwrap().iter() {
        if !item.is_dict() || g(item, "source").as_str() != Some(format!("{root_s}/{target}").as_str()) {
            return invalid("leaf inventory path escape/location mismatch");
        }
        let (_, body) = jstr_input(c, &item["source"])?;
        if g(item, "sha256").as_str() != Some(core::sha256_bytes(&body).as_str()) || (target.ends_with("/.gitkeep") && !body.is_empty()) {
            return invalid("leaf frozen input sha256/scaffold mismatch");
        }
        frozen.push((target.to_string(), body.clone()));
        let slot: Option<String> = if target == "Dockerfile" {
            Some(format!("artifacts/build_recipe/{}", s(&g(&ledger, "dockerfile"))))
        } else if target.ends_with(".sh") {
            let ph = if target.starts_with("build_patches_src/") { "pre" } else { "post" };
            let nm = pure_name(target);
            let row = patches.iter().find(|((a, b), _)| a == ph && *b == nm).map(|x| x.1.clone()).unwrap_or(J::dict());
            if g(&row, "result").as_str() != Some("applied") || !py_eq(&g(&row, "script_sha256"), &g(item, "sha256")) {
                return invalid("leaf script differs from observed applied ledger");
            }
            Some(format!("artifacts/build_patch_{ph}/{nm}"))
        } else {
            None
        };
        if !py_eq(&g(item, "slot_path"), &J::from(slot.clone())) {
            return invalid("leaf reuse must reference its existing recipe/patch slot");
        }
        match slot {
            Some(sl) => reuse.insert(sl, jd! {"target" => target, "bytes" => hex_enc(&body)}),
            None => support.push((format!("leaf-support/{target}"), body)),
        }
    }
    let rootp = c.repo.join(&root_s);
    let mut actual: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    os_walk(&rootp, &mut |base, dirs, names| {
        for (q, link) in dirs {
            let r = rel_posix(&rootp, q)?;
            if *link || !(r == "build_patches" || r == "build_patches_src") {
                return invalid("undeclared leaf directory");
            }
        }
        for n in names {
            actual.insert(rel_posix(&rootp, &base.join(n))?);
        }
        Ok(())
    })?;
    if actual != keyset(&inventory) || reuse.len() != 19 || support.len() != 5 {
        return invalid("leaf exact inventory/reuse/support count mismatch");
    }
    let prov_b = frozen.iter().find(|(k, _)| k == "build_patches_src/PROVENANCE.json").map(|x| x.1.clone()).ok_or_else(|| key_err("build_patches_src/PROVENANCE.json"))?;
    let prov = loads_b(&prov_b)?;
    let ms = or_dict(g(&prov, "source_payload")).get("modules_sha256").cloned().unwrap_or(J::dict());
    if let Some(m) = ms.as_dict() {
        for (name, digest) in m.iter() {
            let b = frozen.iter().find(|(k, _)| *k == format!("build_patches_src/{name}")).map(|x| x.1.clone()).unwrap_or_default();
            if J::str(core::sha256_bytes(&b)) != *digest {
                return invalid("leaf PRE helper/script provenance disagrees");
            }
        }
    }
    for (k, b) in support {
        data.set(k, hex_enc(&b));
    }
    Ok(Some(jd! {"reuse" => J::Dict(reuse), "root" => root_s, "inventory" => inventory, "intermediate" => mid}))
}

/// `_parent_build` — the explicit parent source manifest (`HINT_PARENT_BUILD_MANIFEST`); memoized. `data` values = hex.
pub fn _parent_build(c: &_Ctx) -> HR<Option<J>> {
    let key = "explicit_parent_build";
    if memo_has(c, key) {
        return Ok(memo_get(c, key).filter(|v| !v.is_null()));
    }
    let rf = c.env.get("HINT_PARENT_BUILD_MANIFEST").cloned().unwrap_or(J::Null);
    if !rf.truthy() {
        memo_set(c, key, J::Null);
        return Ok(None);
    }
    let rf = s(&rf);
    let (mp, raw) = _parent_input(c, &rf)?;
    if !rf.starts_with(&format!("output/{}/parent-build/", c.topo)) {
        return invalid("parent manifest must be under topology parent-build/");
    }
    let doc = match crate::value::loads(&String::from_utf8_lossy(&raw)) {
        Ok(d) => d,
        Err(_) => return invalid("invalid parent build manifest JSON"),
    };
    if !doc.is_dict() || !eq_int(&g(&doc, "schema_version"), 1) || g(&doc, "kind").as_str() != Some("hint-parent-source-core") {
        return invalid("unsupported explicit parent source manifest");
    }
    let digest = g(&doc, "selected_image_digest");
    if !digest.as_str().map(|d| re_sha().is_full(d)).unwrap_or(false) || digest.as_str().map(str::to_string) != _image_ref(c)?.0 {
        return invalid("parent manifest does not bind the selected measured image digest");
    }
    let root = g(&doc, "context_root");
    if !root.as_str().map(|r| r.starts_with(&format!("output/{}/cache/", c.topo))).unwrap_or(false) {
        return invalid("parent source context must be an explicit topology cache snapshot");
    }
    let root_s = s(&root);
    let declared = g(&doc, "files");
    let commands = g(&doc, "commands");
    let cmd_keys: std::collections::BTreeSet<String> = ["parent_source_core", "alias", "leaf"].iter().map(|x| x.to_string()).collect();
    if !declared.is_dict() || declared.len() != 9 || !commands.is_dict() || keyset(&commands) != cmd_keys
        || commands.as_dict().unwrap().values().any(|v| !v.as_str().map(|x| x.starts_with("# generated-unverified") && clen(x) <= 8192).unwrap_or(false))
        || !g(&doc, "historical_scope").is_str() || !g(&doc, "generated_reason").is_str()
    {
        return invalid("parent manifest requires exact bounded inputs, generated commands and historical scope");
    }
    let metadata = ["alias-evidence.json", "core-build-command.txt", "leaf-ledger-evidence.json", "README.md"];
    let context: std::collections::BTreeSet<String> = keyset(&declared).iter().filter_map(|n| n.strip_prefix("context/").map(str::to_string)).collect();
    let required = ["Dockerfile.source-build", "requirements.txt", "build_patches/.gitkeep", "build_patches_src/.gitkeep"];
    let mut want: std::collections::BTreeSet<String> = context.iter().map(|n| format!("context/{n}")).collect();
    want.extend(metadata.iter().map(|x| x.to_string()));
    let post_re = {
        static C: OnceLock<Re> = OnceLock::new();
        re_cached(&C, r"build_patches/[0-9]{2}-[A-Za-z0-9_.-]+\.sh", 0)
    };
    if keyset(&declared) != want || context.len() != 5 || !required.iter().all(|r| context.contains(*r))
        || context.iter().filter(|n| !required.contains(&n.as_str())).any(|n| !post_re.is_full(n))
    {
        return invalid("parent source context must contain only the declared core/requirements/one POST/empty PRE scaffolds");
    }
    let mut data = jd! {"manifest.json" => hex_enc(&raw)};
    let mp_parent = rel_posix(&c.repo, mp.parent().unwrap_or(Path::new(".")))?;
    for (target, item) in declared.as_dict().unwrap().iter() {
        if !item.is_dict() {
            return invalid("invalid parent input record");
        }
        let source = g(item, "source");
        let expected = match target.strip_prefix("context/") {
            Some(rest) => format!("{root_s}/{rest}"),
            None => format!("{mp_parent}/{target}"),
        };
        if source.as_str() != Some(expected.as_str()) {
            return invalid(format!("parent source path differs from explicit frozen input location: {target}"));
        }
        let (_, body) = jstr_input(c, &source)?;
        if g(item, "sha256").as_str() != Some(core::sha256_bytes(&body).as_str()) {
            return invalid(format!("parent input sha256 mismatch: {target}"));
        }
        if target.ends_with("/.gitkeep") && !body.is_empty() {
            return invalid("parent PRE/POST scaffolds must be empty");
        }
        data.set(target, hex_enc(&body));
    }
    let rootp = c.repo.join(&root_s);
    let mut actual: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    os_walk(&rootp, &mut |base, dirs, names| {
        for (q, link) in dirs {
            let r = rel_posix(&rootp, q)?;
            if *link || !(r == "build_patches" || r == "build_patches_src") {
                return invalid("undeclared/symlink parent context directory");
            }
        }
        for n in names {
            actual.insert(rel_posix(&rootp, &base.join(n))?);
            if !actual.is_subset(&context) {
                return invalid("undeclared parent context file (active PRE/POST contamination)");
            }
        }
        Ok(())
    })?;
    if actual != context {
        return invalid("parent context inventory mismatch");
    }
    let alias = loads_b(&data_bytes(&data, "alias-evidence.json")?)?;
    let leaf = loads_b(&data_bytes(&data, "leaf-ledger-evidence.json")?)?;
    let leaf_ba = or_dict(g(&leaf, "ledger")).get("build_args").cloned().unwrap_or(J::dict());
    if g(&alias, "build_performed") != J::Bool(false)
        || !py_eq(&g(&alias, "image_id"), &g(&alias, "alias_image_id"))
        || !re_sha().is_full(&s(&g(&alias, "image_id")))
        || (!g(&doc, "intermediate_layer").truthy() && !py_eq(&g(&leaf_ba, "BASE_IMAGE"), &g(&alias, "alias")))
    {
        return invalid("main parent alias/leaf base evidence does not agree");
    }
    let extension = _parent_intermediate(c, &doc, &leaf, &alias, &mp, &mut data)?;
    let header = format!("# {}", unverified_header(&s(&doc["generated_reason"]), "agent")?);
    let readme = utf8_strict(data_bytes(&data, "README.md")?)?;
    let hdr_tail: String = header.chars().skip(2).collect();
    if g(&doc, UNVERIFIED_HEADER_KEY).as_str() != Some(hdr_tail.as_str())
        || !readme.starts_with(&format!("{header}\n"))
        || commands.as_dict().unwrap().values().any(|v| !readme.contains(&s(v)))
        || extension.as_ref().map(|e| !readme.contains(&s(&e["intermediate"]["command"]))).unwrap_or(false)
    {
        return invalid("parent README generated-unverified header/commands mismatch");
    }
    let out = jd! {"ref" => rf, "doc" => doc, "data" => data, "header" => header, "extension" => J::from(extension)};
    memo_set(c, key, out.clone());
    Ok(Some(out))
}

/// `_append_parent_build` — ship the explicit parent source inputs (mutates files · slots · marks).
fn _append_parent_build(c: &_Ctx, payload: &Path, files: &mut Vec<String>, slots: &mut J, marks: &mut J) -> HR<()> {
    let Some(parent) = _parent_build(c)? else { return Ok(()) };
    if let Some(m) = parent["extension"]["reuse"].as_dict() {
        for (rel, item) in m.iter() {
            let p = payload.join(rel);
            if !p.is_file() || std::fs::read(&p).ok() != Some(hex_dec(&s(&item["bytes"]))) {
                return invalid(format!("existing leaf slot does not equal final frozen attempt: {rel}"));
            }
        }
    }
    let mut items: Vec<(String, String)> = parent["data"].as_dict().map(|m| m.iter().map(|(k, v)| (k.to_string(), s(v))).collect()).unwrap_or_default();
    items.sort();
    let pref = s(&parent["ref"]);
    let hdr = s(&parent["header"]);
    for (name, hx) in items {
        let rel = _write_bytes(payload, "build_recipe", &format!("parent-source-core/{name}"), &hex_dec(&hx), None)?;
        files.push(rel.clone());
        if let Some(l) = slots.get_mut("build_recipe").and_then(|r| r.get_mut("files")) {
            l.push(rel.clone());
        }
        let generated = name == "README.md" || name == "manifest.json";
        let mut mk = jd! {"verification" => if generated { "generated-unverified" } else { "verified" },
            "generated_by" => if generated || name == "manifest.json" { "agent" } else { "cell-run" },
            "verification_basis" => if generated { s(&parent["doc"]["generated_reason"]) } else {
                format!("explicit manifest {pref} · selected image digest bound · frozen source sha256 verified · main historical scope only") },
            "parent_source" => name.clone()};
        if generated {
            let h = if name == "manifest.json" {
                jd! {"style" => "json-key", "key" => UNVERIFIED_HEADER_KEY, "text" => hdr.chars().skip(2).collect::<String>()}
            } else {
                jd! {"style" => "comment", "line" => 1, "text" => hdr.clone()}
            };
            mk.set("header", h);
        }
        marks.set(rel, mk);
    }
    let mut ctxi: Vec<String> = keyset(&parent["doc"]["files"]).into_iter().filter(|n| n.starts_with("context/")).collect();
    ctxi.sort();
    let mut psc = jd! {"manifest" => pref.clone(), "selected_image_digest" => parent["doc"]["selected_image_digest"].clone(),
        "context_inventory" => ctxi, "historical_scope" => parent["doc"]["historical_scope"].clone()};
    if parent["extension"].truthy() {
        let ext = &parent["extension"];
        let mut fl: Vec<String> = keyset(&ext["intermediate"]["files"]).into_iter().collect();
        fl.sort();
        psc.set("intermediate_layer", jd! {"image_digest" => ext["intermediate"]["image_digest"].clone(), "files" => fl});
        let mut inv = Map::new();
        if let Some(m) = ext["inventory"].as_dict() {
            for (k, v) in m.iter() {
                inv.insert(k, v["sha256"].clone());
            }
        }
        psc.set("leaf_context", jd! {"frozen_root" => ext["root"].clone(), "inventory" => J::Dict(inv),
            "reused_existing_slots" => ext["reuse"].len(), "additional_support_files" => 5});
    }
    if let Some(ev) = slots.get_mut("build_recipe").and_then(|r| r.get_mut("evidence")) {
        ev.set("parent_source_core", psc);
    }
    Ok(())
}

/// `_build_context_map` — zip slot path → build-context path. `native_triplet` = dict of paths (or `J::Null`).
pub fn _build_context_map(c: &_Ctx, slots: &J, sel: &J, native_triplet: &J) -> HR<Vec<J>> {
    let df_name = g(&or_dict(sel.clone()), "dockerfile");
    let dfn = if df_name.truthy() { Some(s(&df_name)) } else { None };
    let dsel = match &dfn {
        Some(n) if c.out.join(n).is_file() => _select_dockerfile(c, n)?,
        _ => J::dict(),
    };
    let df_text = if g(&dsel, "text").truthy() { s(&dsel["text"]) } else { String::new() };
    let copies = if !df_text.is_empty() { _dockerfile_copies(&df_text)? } else { Vec::new() };
    let origin = g(&dsel, "origin_path");
    let out_rel = core::rel(&c.repo, &c.out)?;
    let rroot = if origin.truthy() && pure_parent(&s(&origin)) != out_rel { Some(out_rel.clone()) } else { None };
    let ctext = _compose_text(c)?;
    let vols = _compose_volumes(&ctext)?;
    let env_refs: Vec<String> = re_env_file_ref().findall(&ctext);
    let cell_env_ref = {
        static C: OnceLock<Re> = OnceLock::new();
        re_cached(&C, r"envs/\.env\.\$\{?CONFIG_FILE", 0).is_search(&ctext)
    };
    let cfg_dir = vols.as_dict().and_then(|m| m.iter().find(|(_, v)| v.as_str() == Some("/app/configs")).map(|(k, _)| k.to_string()));
    let et = g(&c.ev, "triplet");
    let trip_names: Vec<String> = if et.is_dict() { or_dict(et.clone()).as_dict().unwrap().values().map(|v| pure_name(&s(v))).collect() } else { Vec::new() };
    let mut native_at: Vec<(String, String)> = Vec::new();
    if let Some(m) = native_triplet.as_dict() {
        let pre = format!("output/{}/", c.topo);
        for v in m.values() {
            let r = core::rel(&c.repo, Path::new(&s(v)))?;
            if let Some(rest) = r.strip_prefix(&pre) {
                let n = pure_name(&s(v));
                match native_at.iter_mut().find(|(a, _)| *a == n) {
                    Some(e) => e.1 = rest.to_string(),
                    None => native_at.push((n, rest.to_string())),
                }
            }
        }
    }
    let patch_dirs: Vec<String> = ["pre", "post"].iter().map(|p| s(&PATCH_DIRS()[*p])).collect();
    let dfn_s = dfn.clone().unwrap_or("None".into());
    let mut rows: Vec<J> = Vec::new();
    for slot in SLOTS {
        for rel_f in or_dict(g(slots, slot))["files"].items().iter().map(s) {
            let name = pure_name(&rel_f);
            let mut ctx_path: Option<String> = None;
            let mut basis = "빌드 컨텍스트 입력 아님(Dockerfile COPY · compose 참조 밖)".to_string();
            let ext = _parent_build(c)?.map(|p| p["extension"].clone()).unwrap_or(J::Null);
            let reused = g(&or_dict(g(&ext, "reuse")), &rel_f);
            if reused.truthy() {
                rows.push(jd! {"slot_path" => rel_f.clone(), "context_path" => reused["target"].clone(), "context_root" => "leaf-sm12x/context",
                    "stage" => "leaf-sm12x", "context_scope" => "reconstruction_example",
                    "frozen_source_sha256" => core::sha256_bytes(&hex_dec(&s(&reused["bytes"]))),
                    "basis" => "raw slot equals explicit final frozen source before annotation; selected Dockerfile may carry the recorded generated-unverified warning; file_records.sha256 hashes shipped bytes; no duplicate artifact"});
                continue;
            }
            if let Some(part) = rel_f.strip_prefix(_PARENT_PREFIX) {
                let _ = _parent_build(c)?;
                let mut cp = part.strip_prefix("context/").map(str::to_string);
                let mut croot: Option<String> = cp.as_ref().map(|_| "parent-source-core/context".to_string());
                let mut stage = "parent-source-core".to_string();
                if let Some(r) = part.strip_prefix("intermediate-deps2/context/") {
                    cp = Some(r.to_string());
                    croot = Some("parent-source-core/intermediate-deps2/context".into());
                    stage = "intermediate-deps2".into();
                } else if let Some(r) = part.strip_prefix("leaf-support/") {
                    cp = Some(r.to_string());
                    croot = Some("leaf-sm12x/context".into());
                    stage = "leaf-sm12x".into();
                }
                rows.push(jd! {"slot_path" => rel_f.clone(), "context_path" => J::from(cp), "context_root" => J::from(croot),
                    "stage" => stage, "context_scope" => "reconstruction_example",
                    "basis" => "explicit selected-image-bound parent manifest; main frozen context only; metadata is not build context"});
                continue;
            }
            if slot == "triplet" && native_at.iter().any(|(a, _)| *a == name) {
                ctx_path = native_at.iter().find(|(a, _)| *a == name).map(|x| x.1.clone());
                basis = "native 트리플렛 원래 자리(render_native_triplet 산출 · native 정문이 읽는 곳)".into();
            } else if slot == "build_patch_pre" || slot == "build_patch_post" {
                let d = &patch_dirs[if slot == "build_patch_pre" { 0 } else { 1 }];
                if copies.contains(d) || copies.contains(&format!("{d}/{name}")) {
                    ctx_path = Some(format!("{d}/{name}"));
                    basis = format!("{dfn_s} COPY {d}/{name} (or whole directory)");
                }
            } else if slot == "build_recipe" && !df_text.is_empty() {
                if Some(&name) == dfn.as_ref() && rroot.is_some() {
                    let rr = rroot.clone().unwrap();
                    ctx_path = Some(name.clone());
                    basis = format!(
                        "이 zip 재현 예시: docker build -f {rr}/{dfn_s} · 컨텍스트 {rr}(현재 토폴로지 통로; 실린 바이트 이력 출처 {}). 실린 compose 파일만 단독 사용하고 override가 없는 구성은 {out_rel}/의 기본 통로와 대조한다. 실제 원본·후속층 실행 경로는 별도 실행 기록에서 확인한다.",
                        s(&origin)
                    );
                } else if Some(&name) == dfn.as_ref() {
                    let (lk, lv) = match &c.compose_path {
                        Some(p) => crate::evidence::compose_build_dockerfile(&path_j(p))?,
                        None => (None, None),
                    };
                    let lvs = lv.clone().unwrap_or("None".into());
                    basis = if lk.as_deref() != Some("literal") {
                        "compose build.dockerfile ${BUILD_DOCKERFILE} · build.context .".into()
                    } else if lv.as_deref() == Some(name.as_str()) {
                        format!("compose build.dockerfile {lvs}(리터럴) · build.context .")
                    } else {
                        format!("이 zip 재현 예시의 docker build -f {name} 입력 · build.context .(실린 compose 파일만 단독 사용하고 override가 없으면 build.dockerfile 리터럴은 {lvs}이므로 이 파일과 다름; 실제 원본·후속층 실행 경로는 별도 실행 기록에서 확인)")
                    };
                    ctx_path = Some(name.clone());
                } else if let Some(src) = copies.iter().find(|x| pure_name(x) == name && !patch_dirs.contains(x)) {
                    ctx_path = Some(src.clone());
                    basis = format!("{dfn_s} COPY {src}");
                }
            } else if slot == "compose" || slot == "triplet" || slot == "runtime_patch" {
                let stem_t = name.strip_suffix(".template").map(str::to_string);
                if name == "docker-compose.yaml" || name == "docker-compose.yml" {
                    ctx_path = Some(name.clone());
                    basis = "compose 파일(빌드 컨텍스트 루트)".into();
                } else if stem_t.as_ref().map(|t| env_refs.contains(t)).unwrap_or(false) {
                    let t = stem_t.unwrap();
                    ctx_path = Some(format!("envs/{t}"));
                    basis = format!("compose env_file envs/{t}");
                } else if name == ".env.template" && ctext.contains("${") {
                    ctx_path = Some(".env".into());
                    basis = "compose 변수치환 프로젝트 .env(${…} 참조)".into();
                } else if name.starts_with(".env.") && !name.ends_with(".template") && cell_env_ref {
                    ctx_path = Some(format!("envs/{name}"));
                    basis = "compose env_file envs/.env.${CONFIG_FILE}(셀 env)".into();
                } else if let Some(cd) = &cfg_dir {
                    if (name.ends_with(".sh") || name.ends_with(".yaml") || name.ends_with(".py"))
                        && (ctext.contains(&format!("/app/configs/{name}")) || trip_names.contains(&name) || name.ends_with("_patch.py") || slot == "compose")
                    {
                        ctx_path = Some(format!("{cd}/{name}"));
                        basis = format!("compose volumes ./{cd}:/app/configs");
                    }
                }
            }
            let mut row = jd! {"slot_path" => rel_f.clone(), "context_path" => J::from(ctx_path.clone()), "basis" => basis,
                               "context_scope" => "reconstruction_example"};
            if let Some(rr) = &rroot {
                if ctx_path.is_some() && (slot == "build_recipe" || slot == "build_patch_pre" || slot == "build_patch_post") {
                    row.set("context_root", rr.clone());
                }
            }
            rows.push(row);
        }
    }
    if _parent_build(c)?.is_some() {
        rows.push(jd! {"slot_path" => J::Null, "context_path" => "build_patches_src/", "context_root" => "parent-source-core/context",
            "stage" => "parent-source-core", "create" => "empty-dir", "inventory" => jl![".gitkeep"], "context_scope" => "reconstruction_example",
            "basis" => "parent frozen inventory: empty PRE scaffold only; no current i5 PRE modules"});
    }
    for e in _empty_copy_dirs(c, &df_text, slots)? {
        let mut row = jd! {"slot_path" => J::Null, "context_path" => format!("{}/", s(&e["dir"])), "create" => "empty-dir",
            "inventory" => e["inventory"].clone(), "context_scope" => "reconstruction_example",
            "basis" => format!("{dfn_s} COPY {}/ · 실린 패치 없음 — 빈 디렉터리를 만든다(없으면 COPY 단계에서 빌드가 멈춘다) · {}", s(&e["dir"]), s(&e["source"]))};
        if let Some(rr) = &rroot {
            row.set("context_root", rr.clone());
        }
        rows.push(row);
    }
    Ok(rows)
}

include!("collect.rs");
