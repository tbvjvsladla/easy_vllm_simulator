// collect part of artifacts a3 (included into a3/ctx.rs) + owner-module adapters.

// ── owner adapters (Python's in-process slave_forward / render_dockerfile modules) ─────────────────
/// The bytes of a selection record (`sel["bytes"]` = `{"__bytes__": hex}`).
fn sel_bytes(sel: &J) -> Vec<u8> {
    _j_bytes(&sel["bytes"]).unwrap_or_default()
}
fn sel_mode(sel: &J) -> Option<i64> {
    sel.get("mode").and_then(J::as_i64)
}
/// OWNER-MIRROR: .claude/skills/upstream-version-watch/scripts/slave_forward.py::NEVER_FORWARD (closed list).
fn owner_never_forward(_c: &_Ctx) -> HR<Vec<String>> {
    Ok(vec!["CONFIG_FILE".to_string()])
}
/// OWNER-MIRROR: render_dockerfile.py::RUNNER_SCRIPTS (closed list).
const RUNNER_SCRIPTS: [&str; 3] = ["serve_runner.sh", "debug-init.sh", "arm_patch.sh"];
fn owner_compose_default(c: &_Ctx, compose: Option<&Path>, key: &str) -> HR<J> {
    Ok(J::from(c.sf("compose_default")?.compose_default(compose.unwrap_or(Path::new("None")), key)?))
}
fn owner_shared_asset_dir(c: &_Ctx) -> HR<PathBuf> {
    Ok(PathBuf::from(c.render()?.SHARED_ASSET_DIR))
}
fn owner_read_env(c: &_Ctx, p: &Path) -> HR<J> {
    _read_env(c.sf("read_env_first")?, Some(p))
}
fn owner_env_shape(c: &_Ctx, p: &Path, kind: &str, measured: Option<&str>) -> HR<String> {
    Ok(_env_shape_d(&c.repo, p, kind, &J::Null, &J::Null, &J::Null, measured, None, &J::Null)?.0)
}
/// `sf.as_sub_recipe(sf.derive(...))` through the owner's native CLI (`slave_forward.sh json` → `sub_recipe`), with the
/// same inputs `_slave_forward_rows` passes to `derive` (cell env · project .env · measurement-time compose · .env.cluster).
fn owner_as_sub_recipe(c: &_Ctx, _forwards: &J) -> HR<J> {
    let compose = _compose_file(c)?;
    let mut args: Vec<String> = vec!["json".into(), "--cell-env".into(), core::posix(&trip_path(c, "env")), "--project-env".into(),
                                     core::posix(&c.out.join(".env")), "--compose".into(),
                                     compose.map(|p| core::posix(&p)).unwrap_or("None".into())];
    args.push("--cluster-env".into());
    args.push(core::posix(&c.out.join("envs").join(".env.cluster")));
    let p = core::run_owner(&c.repo, core::REL_SLAVE_FORWARD, &args, None, &[], false)?;
    if p.returncode != 0 {
        return core::fail(
            "HINT_SUB_RECIPE_DERIVE_FAILED",
            format!("slave_forward.derive 거부: {}", py_strip(&p.stderr)),
            Some("사유 코드의 자리(셀 env 값 · compose 모양 · render_dockerfile.env_tier)를 고친다 — 전달 목록을 손으로 적지 않는다."),
        );
    }
    let doc = crate::value::loads(&p.stdout).map_err(|e| HintError::py("JSONDecodeError", e))?;
    Ok(doc["sub_recipe"].clone())
}

/// `_docker_slots` — docker-plane slots (build_patch_pre/post · build_recipe · compose) → (applied, sel, ledger, env_shapes,
/// sub_recipe). Signature: `missing`/`files`/`slots` are out-parameters (`&mut`); no caller outside this part.
#[allow(clippy::too_many_arguments)]
fn _docker_slots(c: &_Ctx, payload: &Path, measured: Option<&str>, missing: &mut Vec<String>, files: &mut Vec<String>, slots: &mut J,
                 generated: bool) -> HR<(J, J, Option<J>, Vec<J>, Option<J>)> {
    let (mut applied, sel, ledger, _) = _applied(c, true)?;
    if applied.get_str("status") == Some("unobservable") {
        missing.push("HINT_MISSING_BUILD_LEDGER".into());
    }
    let ship = ship_patch(c);
    for (slot, ph) in [("build_patch_pre", "pre"), ("build_patch_post", "post")] {
        let pdir = s(&PATCH_DIRS()[ph]);
        let mut pf: Vec<String> = Vec::new();
        let n = applied["patches"].items().len();
        for i in 0..n {
            let row = applied["patches"].at(i).clone();
            if row.get_str("phase") != Some(ph) || row.get_str("result") == Some("skipped") {
                continue;
            }
            let file = s(&idx(&row, "file")?);
            let src = _patch_base(c)?.join(&pdir).join(&file);
            if let Some((_, (b, m))) = ship.iter().find(|((a, f), _)| a == ph && *f == file) {
                pf.push(_write_bytes(payload, slot, &file, b, Some(*m as i64))?);
                continue;
            }
            if !src.is_file()? {
                missing.push("HINT_MISSING_APPLIED_PATCH_FILE".into());
                if let Some(l) = applied.get_mut("patches").and_then(J::as_list_mut) {
                    l[i].set("file_absent", true);
                }
                continue;
            }
            let want = g(&row, "script_sha256");
            if want.truthy() && J::str(_sha256_of(&src)?) != want {
                return core::fail(
                    "HINT_APPLIED_PATCH_DRIFT",
                    format!("{pdir}/{file} 가 원장 script_sha256 과 다르다."),
                    Some("이미지를 지은 그 스크립트로 되돌리거나 이미지를 다시 짓는다(다른 스크립트를 재현지침으로 배포 ✗)."),
                );
            }
            pf.push(match src.fs() {
                Some(fp) if !src.is_git() => _copy(payload, fp, slot, None)?,
                _ => _write_bytes(payload, slot, &file, &src.read_bytes()?, Some(src.mode()? as i64))?,
            });
        }
        files.extend(pf.iter().cloned());
        let obs = applied.get_str("status") == Some("observed");
        let shipped_rows: Vec<J> =
            applied["patches"].items().iter().filter(|r| r.get_str("phase") == Some(ph) && r.get_str("result") != Some("skipped")).cloned().collect();
        let decided: Vec<&J> = shipped_rows.iter().filter(|r| r.get_str("result") == Some("applied")).collect();
        let mut undecided: Vec<String> = shipped_rows.iter().filter(|r| r.get_str("result") != Some("applied")).map(|r| s(&r["file"])).collect();
        undecided.sort();
        let (kind, note) = if obs {
            ("build-ledger".to_string(), "원장 관측".to_string())
        } else {
            let kind = if !shipped_rows.is_empty() && undecided.is_empty() { "reconstruction" } else { "none" };
            let mut by_src: Vec<(String, i64)> = Vec::new();
            for r in &decided {
                let k = s(&idx(r, "result_source")?);
                match by_src.iter_mut().find(|(a, _)| *a == k) {
                    Some(e) => e.1 += 1,
                    None => by_src.push((k, 1)),
                }
            }
            by_src.sort();
            let mut note = format!("원장 부재 — 라벨된 재구성: applied {}", decided.len());
            if !by_src.is_empty() {
                note.push_str(&format!("({})", by_src.iter().map(|(k, v)| format!("{k} {v}")).collect::<Vec<_>>().join(" · ")));
            }
            if !undecided.is_empty() {
                note.push_str(&format!(" · 적용 미관측 {}", J::from(undecided.clone()).repr()));
            }
            note.push_str(" · skip 판정 = docker-history+gate");
            (kind.to_string(), note)
        };
        let mut excl: Vec<J> = applied["excluded"].items().iter().filter(|x| x.get_str("phase") == Some(ph)).cloned().collect();
        for r in applied["patches"].items() {
            if r.get_str("phase") == Some(ph) && r.get_str("result") == Some("skipped") {
                excl.push(jd! {"phase" => ph, "file" => r["file"].clone(), "why" => format!("skipped({})", s(&g(r, "reason")))});
            }
        }
        let row = _slot(&pf, &jd! {"kind" => kind, "ref" => applied["source"].clone(), "note" => note}, &J::List(excl))?;
        slots.set(slot, row);
    }
    // build recipe
    let dname = s(&idx(&sel, "dockerfile")?);
    let df = c.out.join(&dname);
    if !df.is_file() {
        return core::fail("HINT_DOCKERFILE_ABSENT", format!("쓰인 Dockerfile 이 없다: output/{}/{dname}", c.topo), None);
    }
    let dsel = _select_dockerfile(c, &dname)?;
    let mut rf = vec![_write_bytes(payload, "build_recipe", &file_name(&df), &sel_bytes(&dsel), sel_mode(&dsel))?];
    let mut rf_src = vec![core::rel(&c.repo, &df)?];
    let mut chosen = Map::new();
    chosen.insert(s(&dsel["path"]), dsel.clone());
    let pr = probe_memo(c);
    let pres = pr.as_ref().map(|p| g(&or_dict(p.record.clone()), "result")).unwrap_or(J::Null);
    let probe_note = if pres.truthy() { s(&pres) } else { "이미지 탐침 안 함".to_string() };
    let dtext = s(&dsel["text"]);
    let dests = _copy_dests(&dtext)?;
    let mut notes: Vec<String> = Vec::new();
    let mut not_shipped: Vec<J> = Vec::new();
    let pdirs: Vec<String> = ["pre", "post"].iter().map(|p| s(&PATCH_DIRS()[*p])).collect();
    for src in _dockerfile_copies(&dtext)? {
        if pdirs.contains(&src) {
            continue;
        }
        if src.starts_with('/') || pure_parts(&src).iter().any(|p| p == "..") || src.chars().any(|ch| "*?[".contains(ch)) {
            not_shipped.push(jd! {"source" => src.clone(), "why" => "컨텍스트 밖 또는 글롭 원천 — 파일 하나로 대응 불가"});
            continue;
        }
        let p = c.out.join(&src);
        if p.is_file() {
            let cpath = match dests.get(&src) {
                Some(d) => Some(_dest_path(&src, &s(d), false)?),
                None => None,
            };
            let ran = pr.as_ref().map(|x| x.ran).unwrap_or(false);
            let baked: Option<Vec<u8>> = match (&cpath, ran) {
                (Some(cp), true) if !cp.is_empty() => pr.as_ref().and_then(|x| x.file(cp)).cloned(),
                _ => None,
            };
            let why = match &cpath {
                None => "COPY 목적지를 이미지 경로로 해소하지 못했다(상대 목적지 · 원천 여럿)".to_string(),
                Some(_) if !ran => probe_note.clone(),
                Some(cp) => format!("이미지에 {cp} 없음(뒤 층이 지웠거나 옮겼다)"),
            };
            let led_sha = match &ledger {
                Some(l) if cpath.as_deref() == Some(LEDGER_REQUIREMENTS_IN_IMAGE) => l.get_str("requirements_sha256").map(str::to_string),
                _ => None,
            };
            let baked_j = baked.as_ref().map(|b| _bytes_j(b)).unwrap_or(J::Null);
            let fsel = _select_copied(c, &src, &p, &baked_j, &why, led_sha.as_deref())?;
            chosen.insert(s(&fsel["path"]), fsel.clone());
            rf.push(_write_bytes(payload, "build_recipe", &file_name(&p), &sel_bytes(&fsel), sel_mode(&fsel))?);
            let mut cs = or_dict(memo_get(c, "copied_sel").unwrap_or(J::Null));
            cs.set(rf.last().unwrap().clone(), fsel.clone());
            memo_set(c, "copied_sel", cs);
            rf_src.push(core::rel(&c.repo, &p)?);
            notes.push(format!(
                "{src}: {dname} 이 COPY{}",
                if src == "requirements.txt" && sel.get_str("track") == Some("source-build") { " → /etc/pip/constraint.txt(제약 천장 · wheel 설치 입력 아님)" } else { "" }
            ));
        } else {
            not_shipped.push(jd! {"source" => src.clone(), "why" => "COPY 원천이지만 파일이 아니거나(디렉터리) 부재 — 싣지 않음"});
        }
    }
    files.extend(rf.iter().cloned());
    let led_df = ledger.as_ref().map(|l| l.get_str("dockerfile") == Some(dname.as_str())).unwrap_or(false);
    let created = _image_created(c)?;
    let rvi = _recipe_vs_image(c, &rf_src, created.as_deref(), &J::Dict(chosen.clone()))?;
    let mut keys: Vec<String> = chosen.keys().map(str::to_string).collect();
    keys.sort();
    let mut srevs: Vec<J> = Vec::new();
    for k in &keys {
        let sx = chosen.get(k).unwrap();
        let mut m = Map::new();
        if let Some(sm) = sx.as_dict() {
            for (kk, v) in sm.iter() {
                if ["path", "commit", "method", "history_match", "shipped", "verified", "origin_path", "origin_commit_utc"].contains(&kk) {
                    m.insert(kk, v.clone());
                }
            }
        }
        srevs.push(J::Dict(m));
    }
    let mut br_ev = jd! {"kind" => if led_df { "build-ledger" } else { "file" }, "ref" => g(&sel, "dockerfile_source"),
        "note" => if notes.is_empty() { J::Null } else { J::str(notes.join(" · ")) }, "recipe_vs_image" => J::List(rvi),
        "selected_revision" => jd! {"path" => idx(&dsel, "path")?, "commit" => idx(&dsel, "commit")?, "method" => idx(&dsel, "method")?,
                                    "history_match" => idx(&dsel, "history_match")?, "candidates" => idx(&dsel, "candidates")?},
        "selected_revisions" => J::List(srevs)};
    if !not_shipped.is_empty() {
        br_ev.set("context_not_shipped", J::List(not_shipped));
    }
    slots.set("build_recipe", _slot(&rf, &br_ev, &J::Null)?);
    // compose
    let mut cf: Vec<String> = Vec::new();
    let mut shapes = Map::new();
    let mut env_shapes: Vec<J> = Vec::new();
    let mut regenerated: Vec<String> = Vec::new();
    let mut regen_skipped: Vec<J> = Vec::new();
    let mut compose_excluded: Vec<J> = Vec::new();
    let Some(cpath) = c.compose_path.clone() else {
        return core::fail("HINT_COMPOSE_ABSENT", format!("output/{}/docker-compose.yaml 부재 — docker 평면의 기동 입력(A층).", c.topo), None);
    };
    let csel = _select_tracked(c, &cpath)?;
    cf.push(_write_bytes(payload, "compose", &file_name(&cpath), &sel_bytes(&csel), sel_mode(&csel))?);
    *c.compose_eff.borrow_mut() = Some(payload.join(cf.last().unwrap()));
    let mut tracked_sel: Vec<J> = vec![csel.clone()];
    let shared = owner_shared_asset_dir(c)?;
    let (names, _) = _runner_names(c)?;
    let (armed, armed_why) = _runtime_patch_armed(c)?;
    let mut shipped_runners: Vec<String> = Vec::new();
    for name in names {
        if name == RUNTIME_PATCH_ARMER && !armed {
            compose_excluded.push(jd! {"file" => name.clone(), "why" => format!("not-armed({armed_why})")});
            continue;
        }
        shipped_runners.push(name.clone());
        if !RUNNER_SCRIPTS.contains(&name.as_str()) {
            return core::fail(
                "HINT_RUNNER_UNKNOWN",
                format!("서빙 경로가 부르는 러너가 render_dockerfile.RUNNER_SCRIPTS 밖이다: {name}"),
                Some("러너 정본은 upstream assets/configs 다 — 등재하거나 compose 를 확인한다."),
            );
        }
        let canonical = shared.join(&name);
        let mounted = c.out.join("configs").join(&name);
        if !canonical.is_file() {
            return core::fail("HINT_RUNNER_OWNER_MISSING", format!("러너 정본 부재: {name}"), None);
        }
        let mut rsel = _select_tracked(c, &canonical)?;
        let mut cc: Vec<J> = idx(&rsel, "cross_check")?.items().to_vec();
        if generated {
            cc.push(J::str("native 셀 — 마운트본 대조 생략(이 셀은 컨테이너 러너로 서빙하지 않았다 · generated-unverified)"));
            rsel.set("cross_check", J::List(cc));
        } else if mounted.is_file() {
            let mb = std::fs::read(&mounted).map_err(|e| core::io_err(&e, &mounted))?;
            let mmt = _mtime_utc(&mounted)?;
            let served = measured.is_none() || mmt.as_ref().map(|m| Some(m.as_str()) <= measured).unwrap_or(false);
            if served || rsel.get_str("shipped") == Some("worktree") {
                if mb != sel_bytes(&rsel) {
                    return core::fail(
                        "HINT_RUNNER_RENDER_DRIFT",
                        format!("{}과 실을 러너 바이트가 갈렸다: {name}(실을 바이트 = {})",
                                if served { "서빙한 마운트본(mtime ≤ 측정)" } else { "asset 정본" }, s(&g(&rsel, "shipped"))),
                        Some(&format!("`render_dockerfile.py --materialize-configs --topology {}` 로 다시 materialize 한다(측정 당시 마운트본과 다르면 이 셀의 러너가 아니다 — 발행 대상을 확인한다).", c.topo)),
                    );
                }
                cc.push(J::str(format!(
                    "마운트본 output/{}/configs/{name}{} 바이트 일치",
                    c.topo,
                    if served && measured.is_some() { format!("(mtime {} ≤ 측정 — 마운트본의 현재 mtime 관측)", mmt.clone().unwrap_or("None".into())) } else { String::new() }
                )));
            } else {
                cc.push(J::str(format!(
                    "마운트본 output/{}/configs/{name} 은 측정 뒤(mtime {}) 다시 쓰였다 — 서빙 증거 아님 · 대조 생략",
                    c.topo,
                    mmt.clone().unwrap_or("None".into())
                )));
            }
            rsel.set("cross_check", J::List(cc));
        }
        cf.push(_write_bytes(payload, "compose", &name, &sel_bytes(&rsel), sel_mode(&rsel))?);
        tracked_sel.push(rsel);
    }
    let compose_text = _compose_text(c)?;
    let excluded_regen = |rel_file: &str, regen: &J, out: &mut Vec<J>| {
        let mut row = jd! {"file" => rel_file, "reason" => POST_MEASUREMENT_REGENERATED,
            "why" => format!("{POST_MEASUREMENT_REGENERATED}(재생성 시점 렌더러의 키·값 — 측정 당시 형상 아님 · 측정 당시 env = {ENV_OBSERVED_POINTER})"),
            "mtime_utc" => regen["mtime_utc"].clone(), "measured_utc" => regen["measured_utc"].clone(),
            "measured_source" => regen["measured_source"].clone()};
        if g(regen, "rerender_error").truthy() {
            row.set("rerender_error", regen["rerender_error"].clone());
        }
        out.push(row);
    };
    let spread = |row: &mut J, regen: &J| {
        if let Some(m) = regen.as_dict() {
            for (k, v) in m.iter() {
                row.set(k, v.clone());
            }
        }
    };
    let mut env_names: Vec<String> = re_env_file_ref().findall(&compose_text);
    env_names.sort();
    env_names.dedup();
    for name in env_names {
        if name.contains('$') || name == format!(".env.{}", c.cell) {
            continue;
        }
        let p = c.out.join("envs").join(&name);
        if !p.is_file() {
            missing.push("HINT_MISSING_ENV_SHAPE".into());
            env_shapes.push(jd! {"file" => format!("envs/{name}"), "status" => "absent"});
            continue;
        }
        let kind = _TIER_ENV_KINDS().get(&name).map(s).unwrap_or("generic".into());
        if let Some(mut regen) = _regenerated(c, &p)? {
            regenerated.push(core::rel(&c.repo, &p)?);
            if RERENDER_FUNCS().get(&kind).is_some() {
                let mut derived_r: Vec<J> = Vec::new();
                let sh = _shape_regenerated_d(c, &p, &name, &kind, &regen, Some(&mut derived_r))?;
                if g(&sh, "ok").truthy() {
                    let rr = sh["rr"].clone();
                    let text = s(&sh["text"]);
                    shapes.insert(name.clone(), text.clone());
                    cf.push(_write(payload, "compose", &format!("{name}.template"), &text)?);
                    let mut row = jd! {"file" => format!("envs/{name}"), "kind" => kind.clone(), "rule" => sh["rule"].clone(),
                        "template" => format!("artifacts/compose/{name}.template"), "post_measurement_regenerated" => true,
                        "status" => RERENDER_STATUS, "verification" => "generated-unverified",
                        "generated_by" => rr["generated_by"].clone(), "renderer_rev" => rr["rev"].clone(),
                        "renderer_function" => rr["function"].clone(), "method" => rr["method"].clone(),
                        "manifest" => rr["manifest"].clone(), "manifest_mtime_utc" => rr["manifest_mtime_utc"].clone(),
                        "manifest_basis" => rr["manifest_basis"].clone(), "header_reason" => _rerender_reason(&rr)?};
                    spread(&mut row, &regen);
                    row.set("derived", J::List(derived_r));
                    env_shapes.push(row);
                    continue;
                }
                missing.push("HINT_MISSING_ENV_SHAPE_RERENDER".into());
                regen.set("rerender_error", sh["why"].clone());
            }
            excluded_regen(&format!("envs/{name}"), &regen, &mut compose_excluded);
            regen_skipped.push(jd! {"file" => format!("envs/{name}")});
            let mut row = jd! {"file" => format!("envs/{name}"), "kind" => kind.clone(), "status" => "excluded",
                "reason" => POST_MEASUREMENT_REGENERATED, "post_measurement_regenerated" => true};
            spread(&mut row, &regen);
            env_shapes.push(row);
            continue;
        }
        let mut derived: Vec<J> = Vec::new();
        let (text, rule) = _env_shape_d(&c.repo, &p, &kind, &J::Null, &J::Null, &J::Null, measured, Some(&mut derived), &J::Null)?;
        shapes.insert(name.clone(), text.clone());
        cf.push(_write(payload, "compose", &format!("{name}.template"), &text)?);
        env_shapes.push(jd! {"file" => format!("envs/{name}"), "kind" => kind.clone(), "rule" => rule,
            "template" => format!("artifacts/compose/{name}.template"), "post_measurement_regenerated" => false, "derived" => J::List(derived)});
    }
    let penv = c.out.join(".env");
    if penv.is_file() {
        if let Some(regen) = _regenerated(c, &penv)? {
            regenerated.push(core::rel(&c.repo, &penv)?);
            excluded_regen(".env", &regen, &mut compose_excluded);
            let mut row = jd! {"file" => ".env", "kind" => "project", "status" => "excluded", "reason" => POST_MEASUREMENT_REGENERATED,
                               "post_measurement_regenerated" => true};
            spread(&mut row, &regen);
            env_shapes.push(row);
        } else {
            let (text, rule) = _env_shape_d(&c.repo, &penv, "project", &J::Null, &J::Null, &J::Null, measured, None, &J::Null)?;
            cf.push(_write(payload, "compose", ".env.template", &text)?);
            env_shapes.push(jd! {"file" => ".env", "kind" => "project", "rule" => rule, "template" => "artifacts/compose/.env.template",
                                 "post_measurement_regenerated" => false});
        }
    } else {
        missing.push("HINT_MISSING_ENV_SHAPE".into());
        env_shapes.push(jd! {"file" => ".env", "kind" => "project", "status" => "absent"});
    }
    let mut recipe = None;
    if c.topo == "multi" {
        let r = _sub_recipe(c, &J::Dict(shapes.clone()), &J::List(regen_skipped.clone()))?;
        cf.push(_write(payload, "compose", "sub_recipe.json", &core::dumps(&r))?);
        recipe = Some(r);
    }
    files.extend(cf.iter().cloned());
    let n_shipped = env_shapes.iter().filter(|e| g(e, "template").truthy()).count();
    regenerated.sort();
    let mut note = format!("서빙 러너 {} · env 형상 실림 {n_shipped}/{}", J::from(shipped_runners.clone()).repr(), env_shapes.len());
    if csel.get_str("shipped") != Some("worktree") {
        note.push_str(&format!(" · compose = 측정 전 이력 기반 후보 {}", s(&g(&csel, "shipped"))));
    }
    if !regenerated.is_empty() {
        note.push_str(&format!(
            " · ⚠ 측정({}) 뒤 재생성 {} — 형상 미실림({POST_MEASUREMENT_REGENERATED} · 측정 당시 env 는 {ENV_OBSERVED_POINTER})",
            measured.unwrap_or("None"),
            J::from(regenerated.clone()).repr()
        ));
    }
    let mut ts = tracked_sel.clone();
    ts.sort_by(|a, b| s(&a["path"]).cmp(&s(&b["path"])));
    let mut compose_ev = jd! {"kind" => "file", "ref" => core::rel(&c.repo, &cpath)?, "note" => note,
        "post_measurement_regenerated" => regenerated, "measured_utc" => J::from(measured.map(str::to_string)),
        "selected_revision" => _tracked_row(&csel)?,
        "selected_revisions" => J::List(ts.iter().map(_tracked_row).collect::<HR<Vec<J>>>()?)};
    let warns: Vec<String> = tracked_sel.iter().filter(|x| g(x, "warning").truthy()).map(|x| format!("{}: {}", s(&x["path"]), s(&x["warning"]))).collect();
    if !warns.is_empty() {
        compose_ev.set("revision_warnings", warns);
    }
    slots.set("compose", _slot(&cf, &compose_ev, &if compose_excluded.is_empty() { J::Null } else { J::List(compose_excluded) })?);
    Ok((applied, sel, ledger, env_shapes, recipe))
}

/// `collect` — build `payload_dir/artifacts/` and return the PAYLOAD.slots · applied_set facts (SPEC §5.6). On failure the
/// `artifacts/` this call created is removed.
pub fn collect(repo: &Path, ev: &J, payload_dir: &Path, forward_module: &J, render_module: &J, runner: &J) -> HR<J> {
    let c = _ctx(repo, ev, runner, forward_module, render_module)?;
    collect_ctx(&c, payload_dir)
}
/// `collect` with an already built context (typed hooks via `_ctx_with`).
pub fn collect_ctx(c: &_Ctx, payload_dir: &Path) -> HR<J> {
    if is_symlink(payload_dir) {
        return core::fail("HINT_ARTIFACTS_OUTPUT_SYMLINK", format!("payload 출력 루트가 symlink다: {}", core::posix(payload_dir)), None);
    }
    let payload = core::resolve(payload_dir);
    let art = payload.join("artifacts");
    let lexists = std::fs::symlink_metadata(&art).is_ok();
    if lexists && is_symlink(&art) {
        return core::fail("HINT_ARTIFACTS_OUTPUT_SYMLINK", format!("artifacts 출력 루트가 symlink다: {}", core::posix(&art)), None);
    }
    if art.exists() && !art.is_dir() {
        return core::fail("HINT_ARTIFACTS_OUTPUT_NOT_DIR", format!("artifacts 출력 루트가 디렉터리가 아니다: {}", core::posix(&art)), None);
    }
    if art.exists() && std::fs::read_dir(&art).map(|mut r| r.next().is_some()).unwrap_or(false) {
        return core::fail(
            "HINT_ARTIFACTS_DIR_NOT_EMPTY",
            format!("artifacts/ 가 비어 있지 않다: {}", core::posix(&art)),
            Some("새 draft 디렉터리에서 publish 한다(기존 저작물을 덮지 않는다)."),
        );
    }
    let mut made = false;
    if !art.exists() {
        std::fs::create_dir_all(&art).map_err(|e| core::io_err(&e, &art))?;
        made = true;
    }
    let r = _collect(c, &payload, &art);
    if r.is_err() && made && std::fs::symlink_metadata(&art).is_ok() && !is_symlink(&art) {
        let _ = std::fs::remove_dir_all(&art);
    }
    r
}

fn _collect(c: &_Ctx, payload: &Path, art: &Path) -> HR<J> {
    let absent: Vec<&str> = ["yaml", "sh", "env"].into_iter().filter(|k| !trip_path(c, k).is_file()).collect();
    if !absent.is_empty() {
        return core::fail(
            "HINT_TRIPLET_ABSENT",
            format!("면제 불가 트리플렛 부재: {} (셀 {})", J::from(absent).repr(), c.cell),
            Some("트리플렛은 서빙 직후의 작업트리에만 있다 — 셀 서빙 산출물을 복원하거나 발행 대상을 확인한다."),
        );
    }
    let (plane, plane_source) = _plane(c)?;
    let mut missing: Vec<String> = Vec::new();
    let mut files: Vec<String> = Vec::new();
    let mut slots = J::dict();
    let mut docker_env_sub = false;
    let trip_files: Vec<String>;
    if plane == "native" {
        let man = crate::evidence::output_manifest(&c.repo, &c.topo)?;
        let table = crate::pii::substitution_table(&c.repo, &J::from(man))?;
        let mut tf = Vec::new();
        for k in ["yaml", "sh", "env"] {
            let tp = trip_path(c, k);
            let rel = _write(payload, "triplet", &file_name(&tp), &crate::pii::substitute(&read_strict(&tp)?, &table, true)?)?;
            chmod(&payload.join(&rel), mode_of(&tp)?)?;
            tf.push(rel);
        }
        trip_files = tf;
    } else {
        let mut tf = vec![_copy(payload, &trip_path(c, "yaml"), "triplet", None)?, _copy(payload, &trip_path(c, "sh"), "triplet", None)?];
        let ep = trip_path(c, "env");
        let (env_text, sub) = _docker_triplet_env_shape(&read_strict(&ep)?)?;
        docker_env_sub = sub;
        let env_rel = _write(payload, "triplet", &file_name(&ep), &env_text)?;
        chmod(&payload.join(&env_rel), mode_of(&ep)?)?;
        tf.push(env_rel);
        trip_files = tf;
    }
    files.extend(trip_files.iter().cloned());
    let sweep = or_dict(g(&c.ev, "sweep"));
    let sidx = if sweep.is_dict() { g(&sweep, "index") } else { J::Null };
    let served = sidx.is_dict() && sidx.get_str("config") == Some(c.cell.as_str());
    let mut trip_ev = jd! {"kind" => "file", "ref" => core::rel(&c.repo, &trip_path(c, "yaml"))?,
        "observed" => if served { J::str(format!("{}/sweep_index.json#config", s(&g(&sweep, "dir")))) } else { J::Null },
        "note" => if served { "측정이 이 셀 config 로 돌았다(스윕 색인)" } else { "측정 대조 없음" }};
    let measured = _measured_utc(c)?.0;
    let mut trip_regen: Vec<String> = Vec::new();
    for k in ["yaml", "sh", "env"] {
        if _regenerated(c, &trip_path(c, k))?.is_some() {
            trip_regen.push(core::rel(&c.repo, &trip_path(c, k))?);
        }
    }
    trip_ev.set("post_measurement_regenerated", trip_regen.clone());
    trip_ev.set("measured_utc", J::from(measured.clone()));
    if !trip_regen.is_empty() {
        let n = format!("{} · ⚠ 측정({}) 뒤 mtime: {} — 실린 값이 측정 당시 값과 같다는 보장 없음", s(&trip_ev["note"]),
                        measured.clone().unwrap_or("None".into()), J::from(trip_regen.clone()).repr());
        trip_ev.set("note", n);
    }
    if plane == "native" {
        trip_ev.set("substituted", "pii.substitution_table(manifest) — 운영자 경로 → 자리표시(싣는 사본만)");
    } else if docker_env_sub {
        trip_ev.set("substituted", "QUANT_MODEL_PATH → manifest.quant_model_path (host bind source; original env unchanged)");
    }
    slots.set("triplet", _slot(&trip_files, &trip_ev, &J::Null)?);

    let rp = c.out.join("configs").join(format!("{}_patch.py", c.cell));
    let rp_files: Vec<String> = if rp.is_file() { vec![_copy(payload, &rp, "runtime_patch", None)?] } else { Vec::new() };
    files.extend(rp_files.iter().cloned());
    let mut rp_ev = jd! {"kind" => "none", "ref" => core::rel(&c.repo, &rp)?};
    if !rp_files.is_empty() {
        let arm_obs = _arm_log_observations(c)?;
        rp_ev.set("arming", arm_obs.clone());
        let meas = or_list(g(&arm_obs, "measured"));
        if meas.truthy() {
            let parts: Vec<String> = meas
                .items()
                .iter()
                .map(|x| format!("{}:{}", s(&x["path"]), x["lines"].items().iter().map(s).collect::<Vec<_>>().join(",")))
                .collect();
            rp_ev.set("observed", parts.join(" · "));
        }
        rp_ev.set("note", format!("{}{}", arm_log_text(&arm_obs)?,
                                  if c.topo == "multi" { " · multi slave 무장 판정은 sub_recipe.role_diff(키 체인 × slave env)" } else { "" }));
    } else {
        rp_ev.set("note", "런타임 패치 파일 없음");
    }
    slots.set("runtime_patch", _slot(&rp_files, &rp_ev, &J::Null)?);

    let mut applied = J::Null;
    let mut sel = J::Null;
    let mut ledger: Option<J> = None;
    let mut marks = J::dict();
    let mut agent_requests: Vec<J> = Vec::new();
    let mut native_meta = J::Null;
    let mut src_ctx: Option<_Ctx> = None;
    let env_shapes: Vec<J>;
    let recipe: Option<J>;
    if plane == "docker" {
        let (a, sl, l, es, r) = _docker_slots(c, payload, measured.as_deref(), &mut missing, &mut files, &mut slots, false)?;
        applied = a;
        sel = sl;
        ledger = l;
        env_shapes = es;
        recipe = r;
        _append_parent_build(c, payload, &mut files, &mut slots, &mut marks)?;
        let cfiles: Vec<String> = slots["compose"]["files"].items().iter().map(s).collect();
        for tracked in or_list(slots["compose"]["evidence"]["selected_revisions"].clone()).items() {
            let pth = g(tracked, "path");
            let name = pure_name(&if pth.truthy() { s(&pth) } else { String::new() });
            for rel_f in &cfiles {
                if pure_name(rel_f) == name && marks.get(rel_f).is_none() && !g(tracked, "verified").truthy() {
                    let why = format!("{name}: {} — 측정 당시 실행 바이트는 별도 관측되지 않았다", s(&g(tracked, "method")));
                    let hdr = _insert_header(payload, rel_f, &why, "renderer")?;
                    marks.set(rel_f.clone(), jd! {"verification" => "generated-unverified", "generated_by" => "renderer", "header" => hdr,
                                                   "verification_basis" => why});
                }
            }
        }
        let dname = if sel.is_dict() && g(&sel, "dockerfile").truthy() { Some(s(&sel["dockerfile"])) } else { None };
        let dsel_ = dname.as_ref().and_then(|n| memo_get(c, &mkey(&["dockerfile", n]))).filter(J::truthy);
        if let (Some(d), Some(dn)) = (&dsel_, &dname) {
            let hm3 = g(d, "history_match");
            let hm3 = if hm3.is_dict() { Some(hm3) } else { None };
            if !g(d, "verified").truthy() {
                let bfiles: Vec<String> = slots["build_recipe"]["files"].items().iter().map(s).collect();
                for rel_f in bfiles {
                    if pure_name(&rel_f) == *dn && marks.get(&rel_f).is_none() {
                        let comparison = match &hm3 {
                            Some(h) if g(h, "total").truthy() => format!("docker history RUN·COPY {}/{} 일치", s(&h["matched"]), s(&h["total"])),
                            _ => "docker history 대조 미관측".into(),
                        };
                        let why3 = format!("실린 {dn}({}) · {comparison} — Dockerfile 전체 원본 바이트의 동일성은 확인하지 못했다", s(&g(d, "shipped")));
                        let hdr = _insert_header(payload, &rel_f, &why3, "renderer")?;
                        marks.set(rel_f.clone(), jd! {"verification" => "generated-unverified", "generated_by" => "renderer", "header" => hdr,
                            "verification_basis" => format!("{why3}(_select_dockerfile · {})", s(&g(d, "method")))});
                    }
                }
            }
        }
        let cs = or_dict(memo_get(c, "copied_sel").unwrap_or(J::Null));
        let mut items: Vec<(String, J)> = cs.as_dict().map(|m| m.iter().map(|(k, v)| (k.to_string(), v.clone())).collect()).unwrap_or_default();
        items.sort_by(|a, b| a.0.cmp(&b.0));
        let bfiles: Vec<String> = slots["build_recipe"]["files"].items().iter().map(s).collect();
        for (rel_f, fsel) in items {
            if bfiles.contains(&rel_f) && marks.get(&rel_f).is_none()
                && (g(&fsel, "image_bytes_differ").truthy() || g(&fsel, "ledger_sha256_match") == J::Bool(false))
                && !g(&fsel, "verified").truthy()
            {
                let led = g(&fsel, "ledger_sha256");
                let why7 = format!(
                    "실린 {}({} · sha256 {})이 {} — 이 이미지를 지은 바이트임을 관측으로 확인하지 못했다(참고 형상)",
                    pure_name(&rel_f),
                    s(&g(&fsel, "shipped")),
                    core::head_chars(&core::sha256_bytes(&sel_bytes(&fsel)), 16),
                    if led.truthy() { format!("빌드 원장 sha256 {} 과 다르다", core::head_chars(&s(&led), 16)) } else { "측정 이미지 안 COPY 대상 바이트와 다르다".into() }
                );
                let hdr = _insert_header(payload, &rel_f, &why7, "renderer")?;
                marks.set(rel_f.clone(), jd! {"verification" => "generated-unverified", "generated_by" => "renderer", "header" => hdr,
                    "verification_basis" => format!("{why7}(_select_copied · {})", s(&g(&fsel, "method")))});
            }
        }
    } else {
        let freezes = g(&c.ev, "pip_freeze_paths");
        let fk = keyset(&freezes);
        if !freezes.is_dict() || fk != ["main", "sub"].iter().map(|x| x.to_string()).collect() {
            return core::fail("HINT_NATIVE_EVIDENCE_MISSING", "native artifacts의 pip_freeze_paths는 정확히 main·sub 보존 경로여야 한다.", None);
        }
        let native_paths = [
            ("native_install_path", g(&c.ev, "native_install_path"), "native-install.sh"),
            ("pip_freeze_paths.main", g(&freezes, "main"), "pip-freeze-main.txt"),
            ("pip_freeze_paths.sub", g(&freezes, "sub"), "pip-freeze-sub.txt"),
        ];
        let mut checked: Vec<(PathBuf, &str, &str)> = Vec::new();
        for (key, raw, name) in &native_paths {
            let rs = raw.as_str().unwrap_or("");
            if !raw.is_str() || py_strip(rs).is_empty() || Path::new(rs).is_absolute() {
                return core::fail("HINT_NATIVE_EVIDENCE_MISSING", format!("native artifacts에 필요한 {key} 보존 경로가 없다."), None);
            }
            let fp = c.repo.join(core::rel(&c.repo, Path::new(rs))?);
            if !fp.is_file() || is_symlink(&fp) {
                return core::fail("HINT_NATIVE_EVIDENCE_UNREADABLE", format!("native artifacts의 {key} 가 regular file이 아니다: {rs}"), None);
            }
            checked.push((fp, name, key));
        }
        let launch = _native_launch_path(c)?;
        let ns = _native_source(c)?;
        let mut nm = Map::new();
        for (k, v) in ns.as_dict().unwrap().iter() {
            if k != "triplet_paths" {
                nm.insert(k, v.clone());
            }
        }
        native_meta = J::Dict(nm);
        if g(&ns, "ok").truthy() {
            let sc = _source_ctx(c, &ns)?;
            let (a, sl, l, es, mut r) = _docker_slots(&sc, payload, measured.as_deref(), &mut missing, &mut files, &mut slots, true)?;
            applied = a;
            sel = sl;
            ledger = l;
            let asrc = s(&g(&applied, "source"));
            let led_kind = if asrc.starts_with("attestation:") {
                "원천 셀 attestation"
            } else if applied.get_str("status") == Some("observed") {
                "원천 이미지 원장"
            } else {
                "원천 이미지 재구성(원장 미관측)"
            };
            let src_basis = format!(
                "원천 이미지 기준({led_kind} {}) — 원천 이미지 {} · 원천 셀 {} · 이 셀의 wheelhouse 가 그 이미지에서 재포장됐다(serve proof wheelhouse.source_image_tag)",
                s(&g(&applied, "source")), s(&ns["image_tag"]), s(&ns["cell"])
            );
            for slot in ["build_recipe", "build_patch_pre", "build_patch_post"] {
                for rel_f in slots[slot]["files"].items().iter().map(s) {
                    marks.set(rel_f, jd! {"verification" => "verified", "generated_by" => "cell-run", "verification_basis" => src_basis.clone()});
                }
            }
            let mut trip_names: Vec<String> = Vec::new();
            for k in ["yaml", "sh", "env"] {
                let tp = PathBuf::from(s(&ns["triplet_paths"][k]));
                trip_names.push(file_name(&tp));
                let rel = _copy(payload, &tp, "compose", None)?;
                files.push(rel.clone());
                let mut cfl: Vec<String> = slots["compose"]["files"].items().iter().map(s).collect();
                cfl.push(rel);
                cfl.sort();
                slots.get_mut("compose").unwrap().set("files", cfl);
            }
            let rerendered: Vec<String> =
                es.iter().filter(|e| e.get_str("status") == Some(RERENDER_STATUS) && g(e, "template").truthy()).map(|e| s(&e["template"])).collect();
            for rel_f in slots["compose"]["files"].items().iter().map(s).collect::<Vec<_>>() {
                if rerendered.contains(&rel_f) {
                    continue;
                }
                let hdr = _insert_header(payload, &rel_f, NATIVE_UNVERIFIED_REASON, "renderer")?;
                let name = pure_name(&rel_f);
                let basis = if trip_names.contains(&name) {
                    format!("원천 셀 {} 트리플렛(render_native_triplet 의 입력 = 이 셀 트리플렛의 Docker 형) — 원천 셀에서는 실행됐지만 이 셀은 이 파일로 실행되지 않았다(native)", s(&ns["cell"]))
                } else {
                    format!("upstream-version-watch 렌더러 산출물(docker-compose · 러너 정본 · env 형상 · slave_forward.derive)을 원천 셀 {} 트리플렛으로 실었다 — 이 셀은 Docker 로 실행되지 않았다(native)", s(&ns["cell"]))
                };
                marks.set(rel_f, jd! {"verification" => "generated-unverified", "generated_by" => "renderer", "verification_basis" => basis, "header" => hdr});
            }
            if let Some(rc) = r.as_mut() {
                rc.set(UNVERIFIED_HEADER_KEY, unverified_header(NATIVE_UNVERIFIED_REASON, "renderer")?);
            }
            let cev = slots.get_mut("compose").unwrap().get_mut("evidence").unwrap();
            let old = g(cev, "note");
            cev.set("note", format!("⚠ generated-unverified(renderer) — native 셀 · 원천 셀 {} 트리플렛으로 렌더러 산출물을 실었다 · {}",
                                    s(&ns["cell"]), if old.truthy() { s(&old) } else { String::new() }));
            cev.set("native_source_cell", ns["cell"].clone());
            if let Some(r0) = cev.get_mut("selected_revision") {
                if r0.is_dict() {
                    r0.set("executed", false);
                    r0.set("verified", J::Null);
                }
            }
            if let Some(J::List(l)) = cev.get_mut("selected_revisions") {
                for r0 in l.iter_mut() {
                    if r0.is_dict() {
                        r0.set("executed", false);
                        r0.set("verified", J::Null);
                    }
                }
            }
            env_shapes = es;
            recipe = r;
            src_ctx = Some(sc);
        } else {
            missing.push("HINT_MISSING_APPLIED_SET".into());
            let why = s(&ns["why"]);
            for s_name in ["build_patch_pre", "build_patch_post"] {
                slots.set(s_name, _slot(&[], &jd! {"kind" => "none", "ref" => J::Null, "note" => format!("native — 원천 이미지 미관측({why}) · 적용 집합 미관측")}, &J::Null)?);
            }
            slots.set("build_recipe", _slot(&[], &jd! {"kind" => "none", "ref" => J::Null, "note" => format!("native — 원천 이미지 미관측({why})")}, &J::Null)?);
            slots.set("compose", _slot(&[], &jd! {"kind" => "none", "ref" => J::Null, "note" => format!("native — 렌더러 입력(원천 셀 트리플렛) 없음({why}) · agent_requests")}, &J::Null)?);
            for (slot_name, name, w) in [("build_recipe", "Dockerfile", "원천 이미지 Dockerfile(원천 이미지 미관측)"),
                                         ("compose", "docker-compose.yaml", "Docker 형 기동(렌더러 입력 없음)"),
                                         ("compose", "serve_runner.sh", "Docker 형 러너(렌더러 입력 없음)")] {
                agent_requests.push(_agent_request(slot_name, name, &format!("{w} — {why}"),
                    &["artifacts/triplet/".into(), "artifacts/build_recipe/pip-freeze-main.txt".into(), "PAYLOAD.build".into()])?);
            }
            env_shapes = Vec::new();
            recipe = None;
        }
        let mut rf = Vec::new();
        for (fp, name, key) in &checked {
            let rel = _copy(payload, fp, "build_recipe", Some(name))?;
            marks.set(rel.clone(), jd! {"verification" => "verified", "generated_by" => "cell-run",
                "verification_basis" => format!("native serve proof {key} — native_multinode_serve 가 이 셀 실행에서 보존한 파일")});
            rf.push(rel);
        }
        files.extend(rf.iter().cloned());
        {
            let br = slots.get_mut("build_recipe").unwrap();
            let mut bf: Vec<String> = br["files"].items().iter().map(s).collect();
            bf.extend(rf.iter().cloned());
            bf.sort();
            br.set("applicable", !bf.is_empty());
            br.set("files", bf);
            let mut rfs = rf.clone();
            rfs.sort();
            let ev = br.get_mut("evidence").unwrap();
            ev.set("native_files", rfs);
            let old = g(ev, "note");
            ev.set("note", format!("{}native 설치 입력(native-install.sh · pip freeze main/sub = serve proof 보존 파일)",
                                   if old.truthy() { format!("{} · ", s(&old)) } else { String::new() }));
            let conf = slot_confidence(br)?;
            br.set("confidence", conf);
        }
        if let Some((lp, lname, lkey)) = &launch {
            let lr = _copy(payload, lp, "triplet", Some(lname))?;
            files.push(lr.clone());
            let tr = slots.get_mut("triplet").unwrap();
            let mut tf: Vec<String> = tr["files"].items().iter().map(s).collect();
            tf.push(lr.clone());
            tf.sort();
            tr.set("files", tf);
            marks.set(lr, jd! {"verification" => "verified", "generated_by" => "cell-run",
                "verification_basis" => format!("native serve proof {lkey} — 이 셀 기동이 보존한 기동 절차 기록")});
        } else {
            missing.push("HINT_MISSING_NATIVE_LAUNCH".into());
            agent_requests.push(_agent_request("triplet", NATIVE_LAUNCH_NAME,
                "native 기동 절차 기록 부재 — serve proof 에 native_launch_path 가 없다(ray head/worker · vllm serve 순서 · 노드별 env)",
                &["artifacts/triplet/".into(), "artifacts/build_recipe/native-install.sh".into(), "docs/simlog 의 native 실행 로그(발췌만)".into()])?);
        }
        if c.topo == "multi" && launch.is_none() {
            missing.push("HINT_MISSING_SUB_RECIPE".into());
        }
        for s_name in ["triplet", "compose", "build_recipe"] {
            let row = slots.get_mut(s_name).unwrap();
            let ap = row["files"].truthy();
            row.set("applicable", ap);
            let conf = slot_confidence(row)?;
            row.set("confidence", conf);
        }
    }
    let vctx: &_Ctx = src_ctx.as_ref().unwrap_or(c);
    // fork_pin
    let mut variants: Vec<String> = Vec::new();
    for ln in py_splitlines(&_read(&trip_path(vctx, "env"))?) {
        if py_strip(ln).starts_with("VARIANT=") {
            let v = py_strip(ln.split_once('=').unwrap().1).to_string();
            if !variants.contains(&v) {
                variants.push(v);
            }
        }
    }
    variants.sort();
    let mut ff: Vec<String> = Vec::new();
    if variants.len() > 1 {
        return core::fail("HINT_VARIANT_AMBIGUOUS", format!("셀 env 에 VARIANT 가 여럿이다: {}", J::from(variants).repr()), None);
    }
    if let Some(vid) = variants.first().filter(|v| !v.is_empty()) {
        let led = _json_file(&c.repo.join(REL_ARCH_VARIANT_LEDGER))?.unwrap_or(J::dict());
        let table = if g(&led, "source_build_variants").is_dict() { g(&led, "source_build_variants") } else { J::dict() };
        let mut rec = if !vid.starts_with('_') && g(&table, vid).is_dict() { Some(g(&table, vid)) } else { None };
        if rec.is_none() {
            let mut keys: Vec<(&str, &J)> = table.as_dict().unwrap().iter().collect();
            keys.sort_by(|a, b| a.0.cmp(b.0));
            let hits: Vec<&J> = keys.iter().filter(|(k, v)| !k.starts_with('_') && v.is_dict() && v.get_str("variant_id") == Some(vid.as_str())).map(|x| x.1).collect();
            if hits.len() == 1 {
                rec = Some(hits[0].clone());
            }
        }
        let Some(rec) = rec else {
            return core::fail(
                "HINT_FORK_PIN_UNBOUND",
                format!("VARIANT={vid} 가 {REL_ARCH_VARIANT_LEDGER} 에 없다(키·유일 variant_id 모두 불일치)."),
                Some("변종 좌표는 Band2 원장에 등재한다(workflow.md §변종 좌표의 거처) — 등재 없이 발행하지 않는다."),
            );
        };
        let rf_ = vctx.env.get("VLLM_REF").cloned().unwrap_or(J::Null);
        if g(&rec, "vllm_ref").truthy() && rf_.truthy() && g(&rec, "vllm_ref") != rf_ {
            return core::fail("HINT_FORK_PIN_MISMATCH", format!("변종 {vid} 의 vllm_ref={} ≠ 셀 env VLLM_REF={}", rec["vllm_ref"].repr(), rf_.repr()), None);
        }
        let mut vd = jd! {"variant_id" => vid.clone()};
        if let Some(m) = rec.as_dict() {
            for (k, v) in m.iter() {
                vd.set(k, v.clone());
            }
        }
        ff.push(_write(payload, "fork_pin", "variant.json", &core::dumps(&vd))?);
    }
    files.extend(ff.iter().cloned());
    slots.set("fork_pin", _slot(&ff, &jd! {"kind" => if ff.is_empty() { "none" } else { "file" }, "ref" => REL_ARCH_VARIANT_LEDGER,
        "note" => if ff.is_empty() { "VARIANT 줄 부재 = stock" } else { "변종 좌표 = Band2 원장" }}, &J::Null)?);
    // rerendered shapes
    for e in &env_shapes {
        if e.get_str("status") == Some(RERENDER_STATUS) && g(e, "template").truthy() {
            let t = s(&e["template"]);
            let hdr = _insert_header(payload, &t, &s(&e["header_reason"]), &s(&e["generated_by"]))?;
            marks.set(t, jd! {"verification" => "generated-unverified", "generated_by" => e["generated_by"].clone(), "header" => hdr,
                "verification_basis" => format!("측정 당시 판본 렌더러 {}({}) · {} — 실행 바이트 아님(작업트리 파일은 측정 뒤 {} 재생성)",
                    s(&e["generated_by"]), s(&e["method"]), s(&e["manifest_basis"]), s(&e["mtime_utc"]))});
        }
    }
    // PII backstop
    let terms = crate::pii::load_pii_terms(&c.repo)?;
    if terms.is_none() {
        missing.push("HINT_MISSING_PII_TERMS".into());
    }
    let mut exempt: Vec<J> = Vec::new();
    if art.is_dir() {
        let hits = crate::pii::scan_tree(payload, terms.as_deref(), "deploy", &[], true, true, Some(&["artifacts".to_string()]), Some(&mut exempt))?;
        if !hits.is_empty() {
            return core::fail(
                "HINT_ARTIFACT_PII",
                format!("artifacts/ 에 배포 금지 패턴:\n  {}", crate::pii::render_located_hits(&hits, 10)?),
                Some("해당 파일의 출처(트리플렛·compose·패치)를 고치거나 형상 규칙을 고친다(덧칠 ✗)."),
            );
        }
    }
    let ext = if plane == "docker" { _parent_build(c)?.map(|p| p["extension"].clone()).unwrap_or(J::Null) } else { J::Null };
    if let Some(m) = g(&ext, "reuse").as_dict() {
        for (rel_f, item) in m.iter() {
            let shipped = std::fs::read(payload.join(rel_f)).map_err(|e| core::io_err(&e, &payload.join(rel_f)))?;
            let fb = hex_dec(&s(&item["bytes"]));
            if !_marked_leaf_matches_frozen(&shipped, &fb, &s(&item["target"]), &or_dict(g(&marks, rel_f)))? {
                return invalid(format!("final marked leaf slot differs from frozen context: {rel_f}"));
            }
            let mut mk = or_dict(g(&marks, rel_f));
            mk.set("frozen_source_sha256", core::sha256_bytes(&fb));
            marks.set(rel_f, mk);
        }
    }
    _slot_machine_reasons(c, &mut slots, &applied, &sel, &plane, vctx)?;
    let rel_ctx = _relevance_context(c, &slots, &applied)?;
    _file_records(c, payload, &mut slots, &marks, &applied, &rel_ctx)?;
    if applied.is_dict()
        && applied["patches"].items().iter().any(|r| matches!(r.get_str("phase"), Some("pre") | Some("post")) && !matches!(r.get_str("result"), Some("applied") | Some("skipped")))
    {
        missing.push("HINT_MISSING_APPLIED_SET".into());
    }
    _require_registered(&c.repo, &missing)?;
    missing.sort();
    missing.dedup();
    files.sort();
    files.dedup();
    let track = if sel.truthy() && plane == "docker" { sel["track"].clone() } else { J::str("native") };
    let build_ledger = match (&ledger, applied.truthy()) {
        (Some(l), true) if l.is_dict() => jd! {"source" => applied["source"].clone(), "doc" => l.clone()},
        _ => J::Null,
    };
    let native_trip = if plane == "native" {
        let mut m = Map::new();
        for (k, p) in &c.triplet {
            m.insert(k.clone(), path_j(p));
        }
        J::Dict(m)
    } else {
        J::Null
    };
    let reproduce = _reproduce(c, &sel, &plane, src_ctx.as_ref())?;
    let bcm = _build_context_map(vctx, &slots, &sel, &native_trip)?;
    let mut exempt_s = Vec::new();
    for x in &exempt {
        exempt_s.push(J::str(_exempt_record(&s(x))?));
    }
    let mut ri = Map::new();
    for k in ["model", "engine_logs"] {
        ri.insert(k, rel_ctx[k].clone());
    }
    Ok(jd! {
        "plane" => plane.clone(), "plane_source" => plane_source, "track" => track, "build" => sel.clone(), "slots" => slots,
        "applied_set" => applied, "build_ledger_vllm" => J::from(_ledger_vllm_view(c)?), "build_ledger" => build_ledger,
        "missing" => missing, "files" => files, "sub_recipe" => J::from(recipe),
        "value_status_candidates" => J::List(value_status_rows(c)?), "reproduce_steps" => J::List(reproduce),
        "env_shapes" => J::List(env_shapes), "pii_exempt" => J::List(exempt_s), "native_source" => native_meta,
        "agent_requests" => J::List(agent_requests), "build_context_map" => J::List(bcm), "relevance_inputs" => J::Dict(ri),
    })
}
