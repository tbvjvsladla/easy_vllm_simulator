#!/usr/bin/env python3
"""agent_card_contract.py — Agent_Card(A2A v1.0.1) 계약 검증기 + JWS 서명/검증 (결정론 · 사람 개입 0).

왜 이 파일인가 (2026-09-05 · plan_26090516 ② · H1 승인):
    Agent_Card 는 **A2A 평면 규약**이다 — "이 노드가 무엇을 할 수 있나"를 메인이 실측해 발급한 계약이며
    HW 사실("무엇 위에서 도나")은 서브 manifest 가 권위다. 카드는 A2A v1.0.1 `specification/a2a.proto`
    (진실의 원천 · JSON 은 camelCase)의 **표준 필드만 최상위**에 두고, 우리 고유 항목은 표준이 정한
    확장 자리(`capabilities.extensions[]`) 하나에만 둔다. v1.0 에서 JSON Schema 파일은 저장소에서 사라졌으므로
    (`specification/json/README.md` 만 남음) 검증은 proto 필수 필드 집합으로 한다.

서명 (A2A §8.4 · H1 결정 (A) 표준 JWS):
    payload = JCS(RFC 8785) 정규화(카드 − `signatures` − 기본값 필드) · JWS protected 헤더 {alg:"EdDSA",
    typ:"JOSE", kid} · Ed25519. 서명키는 설치 때 에이전트가 생성(패스프레이즈 없음 · 0600 · 비추적)하고
    렌더 시 자동 서명, 서브·메인 게이트가 신뢰 키 저장소(JWK)로 자동 검증한다 — **사람에게 묻는 절차가
    없다**(H1 조건: 매 작업·세션·캠페인마다 묻게 되면 서명을 걷어낸다).

의존: `cryptography`(Ed25519) · `PyJWT`(JWS 컴팩트 직렬화). 둘 다 없으면 fail-loud(설치는 사람 승인 사항).

사용:
    agent_card_contract.py validate --card Agent_Card.json
    agent_card_contract.py keygen   --out-dir output/<topology>/a2a_signing [--force]
    agent_card_contract.py sign     --card Agent_Card.json --key <pem> --trusted-out .claude/a2a/trusted_keys.json
    agent_card_contract.py verify   --card Agent_Card.json --trusted .claude/a2a/trusted_keys.json
    agent_card_contract.py --self-test
종료: 0 ok · 2 인자/의존 · 5 계약 위반 · 6 서명 불일치/부재
"""
from __future__ import annotations

import argparse
import base64
import datetime as _dt
import hashlib
import json
import os
import sys

EXIT_OK = 0
EXIT_USAGE = 2
EXIT_CONTRACT = 5
EXIT_SIGNATURE = 6

# ── A2A v1.0.1 proto → JSON(camelCase) 필드 집합 (message AgentCard) ──
CARD_REQUIRED = ("name", "description", "supportedInterfaces", "version", "capabilities",
                 "defaultInputModes", "defaultOutputModes", "skills")
CARD_OPTIONAL = ("provider", "documentationUrl", "securitySchemes", "securityRequirements",
                 "signatures", "iconUrl")
CARD_FIELDS = frozenset(CARD_REQUIRED + CARD_OPTIONAL)
INTERFACE_REQUIRED = ("url", "protocolBinding", "protocolVersion")
INTERFACE_FIELDS = frozenset(INTERFACE_REQUIRED + ("tenant",))
SKILL_REQUIRED = ("id", "name", "description", "tags")
SKILL_FIELDS = frozenset(SKILL_REQUIRED + ("examples", "inputModes", "outputModes", "securityRequirements"))
CAPABILITY_FIELDS = frozenset(("streaming", "pushNotifications", "extensions", "extendedAgentCard"))
EXTENSION_FIELDS = frozenset(("uri", "description", "required", "params"))
PROVIDER_REQUIRED = ("url", "organization")
SIGNATURE_REQUIRED = ("protected", "signature")
STANDARD_BINDINGS = ("JSONRPC", "GRPC", "HTTP+JSON")

# ── easy-vllm 확장 계약 (표준 확장 자리 하나) ──
NODE_ROLE_EXT_URI = "urn:easy-vllm:ext:node-role:v1"
NODE_ROLE_PARAMS = ("topology", "sub_mode", "sub_mode_source", "rank", "rank_source",
                    "identity_authority", "delivery_plane", "tool_plane", "tool_plane_source")
SSH_BINDING_URI = "urn:easy-vllm:a2a-binding:ssh-claude-p:v1"
EXPECTED_SKILL_IDS = ("inspect", "config", "build", "serve", "bench", "publish")

JWS_ALG = "EdDSA"


class ContractViolation(ValueError):
    pass


# ───────────────────────────── 검증 ─────────────────────────────
def _is_uri(s) -> bool:
    return isinstance(s, str) and ":" in s and " " not in s and len(s) >= 3


def _no_floats(obj, path="$"):
    if isinstance(obj, float):
        raise ContractViolation(f"{path}: float 값은 금지(JCS 정규화 결정론 — 정수/문자열로 적는다)")
    if isinstance(obj, dict):
        for k, v in obj.items():
            _no_floats(v, f"{path}.{k}")
    elif isinstance(obj, list):
        for i, v in enumerate(obj):
            _no_floats(v, f"{path}[{i}]")


def validate_card(card: dict) -> list[str]:
    """proto 필수 집합 + 우리 확장 계약. 위반 목록을 돌려준다(빈 목록 = 통과)."""
    v: list[str] = []
    if not isinstance(card, dict):
        return ["카드가 JSON 객체가 아니다"]
    try:
        _no_floats(card)
    except ContractViolation as e:
        v.append(str(e))
    unknown = sorted(k for k in card if k not in CARD_FIELDS)
    if unknown:
        v.append(f"표준 밖 최상위 키 {unknown} — 고유 항목은 capabilities.extensions[] 에만 둔다(A2A 1.0.1 AgentCard 필드 집합)")
    for k in CARD_REQUIRED:
        if k not in card:
            v.append(f"필수 필드 누락: {k}")
    for k in ("name", "description", "version"):
        if k in card and (not isinstance(card[k], str) or (k != "description" and not card[k].strip())):
            v.append(f"{k}: 비어 있지 않은 문자열이어야 한다")
    ifs = card.get("supportedInterfaces")
    if ifs is not None:
        if not isinstance(ifs, list) or not ifs:
            v.append("supportedInterfaces: 비어 있지 않은 배열(첫 항목 = 선호 인터페이스)")
        else:
            for i, it in enumerate(ifs):
                if not isinstance(it, dict):
                    v.append(f"supportedInterfaces[{i}]: 객체가 아니다"); continue
                for k in INTERFACE_REQUIRED:
                    if not isinstance(it.get(k), str) or not it.get(k):
                        v.append(f"supportedInterfaces[{i}].{k}: 필수 문자열")
                bad = sorted(k for k in it if k not in INTERFACE_FIELDS)
                if bad:
                    v.append(f"supportedInterfaces[{i}]: 표준 밖 키 {bad}")
                pb = it.get("protocolBinding")
                if isinstance(pb, str) and pb not in STANDARD_BINDINGS and not _is_uri(pb):
                    v.append(f"supportedInterfaces[{i}].protocolBinding={pb!r}: 표준 3종({', '.join(STANDARD_BINDINGS)}) 아니면 **URI** 여야 한다(§5.8·§12.7)")
    for k in ("defaultInputModes", "defaultOutputModes"):
        val = card.get(k)
        if val is not None and (not isinstance(val, list) or not val or not all(isinstance(x, str) and x for x in val)):
            v.append(f"{k}: 비어 있지 않은 문자열 배열")
    caps = card.get("capabilities")
    if caps is not None:
        if not isinstance(caps, dict):
            v.append("capabilities: 객체여야 한다")
        else:
            bad = sorted(k for k in caps if k not in CAPABILITY_FIELDS)
            if bad:
                v.append(f"capabilities: 표준 밖 키 {bad}")
            for k in ("streaming", "pushNotifications", "extendedAgentCard"):
                if k in caps and not isinstance(caps[k], bool):
                    v.append(f"capabilities.{k}: bool")
            exts = caps.get("extensions", [])
            if not isinstance(exts, list):
                v.append("capabilities.extensions: 배열"); exts = []
            role_exts = []
            for i, ex in enumerate(exts):
                if not isinstance(ex, dict):
                    v.append(f"capabilities.extensions[{i}]: 객체가 아니다"); continue
                bad = sorted(k for k in ex if k not in EXTENSION_FIELDS)
                if bad:
                    v.append(f"capabilities.extensions[{i}]: 표준 밖 키 {bad}")
                if not _is_uri(ex.get("uri")):
                    v.append(f"capabilities.extensions[{i}].uri: URI 여야 한다")
                if "required" in ex and not isinstance(ex["required"], bool):
                    v.append(f"capabilities.extensions[{i}].required: bool")
                if "params" in ex and not isinstance(ex["params"], dict):
                    v.append(f"capabilities.extensions[{i}].params: 객체(Struct)")
                if ex.get("uri") == NODE_ROLE_EXT_URI:
                    role_exts.append(ex)
            if len(role_exts) != 1:
                v.append(f"노드 역할 확장 {NODE_ROLE_EXT_URI} 는 정확히 1개여야 한다(현재 {len(role_exts)})")
            else:
                p = role_exts[0].get("params") or {}
                missing = [k for k in NODE_ROLE_PARAMS if k not in p]
                extra = sorted(k for k in p if k not in NODE_ROLE_PARAMS)
                if missing:
                    v.append(f"node-role 확장 params 누락: {missing}")
                if extra:
                    v.append(f"node-role 확장 params 표준 밖 키: {extra}")
                if "rank" in p and not (p["rank"] is None or isinstance(p["rank"], int)):
                    v.append("node-role.rank: 정수 또는 null(single)")
                if "tool_plane" in p and not (isinstance(p["tool_plane"], list) and all(isinstance(x, str) for x in p["tool_plane"])):
                    v.append("node-role.tool_plane: 문자열 배열")
                for k in ("topology", "sub_mode", "sub_mode_source", "rank_source", "identity_authority", "delivery_plane", "tool_plane_source"):
                    if k in p and (not isinstance(p[k], str) or not p[k]):
                        v.append(f"node-role.{k}: 비어 있지 않은 문자열(계약 해소값 — 빈 값은 미해소)")
                if role_exts[0].get("required") is not True:
                    v.append("node-role 확장은 required: true (이 확장을 모르는 클라이언트는 이 노드를 다룰 수 없다)")
    skills = card.get("skills")
    if skills is not None:
        if not isinstance(skills, list) or not skills:
            v.append("skills: 비어 있지 않은 배열")
        else:
            ids = []
            for i, sk in enumerate(skills):
                if not isinstance(sk, dict):
                    v.append(f"skills[{i}]: 객체가 아니다"); continue
                for k in SKILL_REQUIRED:
                    if k not in sk:
                        v.append(f"skills[{i}].{k}: 필수")
                bad = sorted(k for k in sk if k not in SKILL_FIELDS)
                if bad:
                    v.append(f"skills[{i}]: 표준 밖 키 {bad}")
                if "tags" in sk and (not isinstance(sk["tags"], list) or not sk["tags"]):
                    v.append(f"skills[{i}].tags: 비어 있지 않은 배열(REQUIRED)")
                ids.append(sk.get("id"))
            if len(set(ids)) != len(ids):
                v.append("skills[].id 중복")
    prov = card.get("provider")
    if prov is not None:
        if not isinstance(prov, dict) or any(not isinstance(prov.get(k), str) for k in PROVIDER_REQUIRED):
            v.append("provider: {url, organization} 둘 다 필수 문자열")
    sigs = card.get("signatures")
    if sigs is not None:
        if not isinstance(sigs, list):
            v.append("signatures: 배열")
        else:
            for i, s in enumerate(sigs):
                if not isinstance(s, dict) or any(not isinstance(s.get(k), str) for k in SIGNATURE_REQUIRED):
                    v.append(f"signatures[{i}]: {{protected, signature}} 필수 문자열")
    return v


def require_skills(card: dict, expected=EXPECTED_SKILL_IDS) -> list[str]:
    """easy-vllm 계약: 6 skill 이 모두 선언돼야 publish Phase 까지 위임 가능하다."""
    have = {s.get("id") for s in card.get("skills") or [] if isinstance(s, dict)}
    return [f"skill 누락: {k}" for k in expected if k not in have]


# ───────────────────────────── 정규화 · 서명 ─────────────────────────────
def _strip_defaults(obj):
    """A2A §8.4.1: 서명 전 기본값 필드 제거 — 비-REQUIRED 의 빈 배열/빈 객체/빈 문자열 은 뺀다.
    명시된 bool(false 포함)은 남긴다(스펙 예시와 동일). 우리 카드는 float 를 금지하므로 수 표현은 결정론이다."""
    if isinstance(obj, dict):
        out = {}
        for k, v in obj.items():
            if k == "signatures":
                continue
            v2 = _strip_defaults(v)
            if v2 in ([], {}, ""):
                if k in CARD_REQUIRED and isinstance(obj, dict):
                    out[k] = v2   # REQUIRED 는 값이 기본값이어도 남긴다
                continue
            out[k] = v2
        return out
    if isinstance(obj, list):
        return [_strip_defaults(x) for x in obj]
    return obj


def canonical_payload(card: dict) -> bytes:
    """RFC 8785(JCS): 키 사전순 · 공백 없음 · UTF-8. float 없음이 전제(validate 가 강제)."""
    _no_floats(card)
    body = _strip_defaults(card)
    return json.dumps(body, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode("utf-8")


def _b64u(b: bytes) -> str:
    return base64.urlsafe_b64encode(b).rstrip(b"=").decode("ascii")


def _b64u_dec(s: str) -> bytes:
    return base64.urlsafe_b64decode(s + "=" * (-len(s) % 4))


def _crypto():
    try:
        from cryptography.hazmat.primitives import serialization
        from cryptography.hazmat.primitives.asymmetric import ed25519
    except ImportError as e:  # 설치는 사람 승인 사항 — 여기서 몰래 깔지 않는다
        raise SystemExit(f"[agent-card] FAIL: `cryptography` 부재({e}) — 서명/검증 불가. 설치는 사용자 승인 후(헌법 §안전 경계).")
    return serialization, ed25519


def jwk_from_public(pub_bytes: bytes) -> dict:
    jwk = {"kty": "OKP", "crv": "Ed25519", "x": _b64u(pub_bytes)}
    thumb = hashlib.sha256(json.dumps({"crv": jwk["crv"], "kty": jwk["kty"], "x": jwk["x"]},
                                      sort_keys=True, separators=(",", ":")).encode()).digest()
    jwk["kid"] = _b64u(thumb)[:24]   # RFC 7638 thumbprint 축약 — 키 식별용
    return jwk


def keygen(out_dir: str, force: bool = False) -> dict:
    serialization, ed25519 = _crypto()
    os.makedirs(out_dir, exist_ok=True)
    priv_path = os.path.join(out_dir, "main_ed25519.pem")
    pub_path = os.path.join(out_dir, "main_ed25519.jwk.json")
    if os.path.exists(priv_path) and not force:
        raise SystemExit(f"[agent-card] FAIL: 서명키가 이미 있다({priv_path}) — 재생성은 --force(기존 카드 서명이 전부 무효가 된다)")
    key = ed25519.Ed25519PrivateKey.generate()
    pem = key.private_bytes(serialization.Encoding.PEM, serialization.PrivateFormat.PKCS8,
                            serialization.NoEncryption())   # 패스프레이즈 없음 = 사람 개입 0(H1 조건)
    fd = os.open(priv_path, os.O_WRONLY | os.O_CREAT | os.O_TRUNC, 0o600)
    with os.fdopen(fd, "wb") as f:
        f.write(pem)
    pub = key.public_key().public_bytes(serialization.Encoding.Raw, serialization.PublicFormat.Raw)
    jwk = jwk_from_public(pub)
    jwk["issued_by"] = "main"
    jwk["created_utc"] = _dt.datetime.now(_dt.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
    with open(pub_path, "w", encoding="utf-8") as f:
        json.dump(jwk, f, ensure_ascii=False, indent=2)
    return {"private": priv_path, "public_jwk": pub_path, "kid": jwk["kid"]}


def _load_private(pem_path: str):
    serialization, _ = _crypto()
    with open(pem_path, "rb") as f:
        return serialization.load_pem_private_key(f.read(), password=None)


def sign_card(card: dict, pem_path: str) -> dict:
    """카드에 signatures[] 를 (재)부여한 사본을 돌려준다. 기존 서명은 갈아 끼운다(키 회전은 keygen --force)."""
    violations = validate_card({k: v for k, v in card.items() if k != "signatures"})
    if violations:
        raise ContractViolation("서명 전 계약 위반: " + "; ".join(violations))
    serialization, _ = _crypto()
    key = _load_private(pem_path)
    pub = key.public_key().public_bytes(serialization.Encoding.Raw, serialization.PublicFormat.Raw)
    kid = jwk_from_public(pub)["kid"]
    header = {"alg": JWS_ALG, "typ": "JOSE", "kid": kid}
    protected = _b64u(json.dumps(header, sort_keys=True, separators=(",", ":")).encode())
    payload = canonical_payload(card)
    signing_input = (protected + "." + _b64u(payload)).encode("ascii")
    sig = key.sign(signing_input)
    out = dict(card)
    out["signatures"] = [{"protected": protected, "signature": _b64u(sig)}]
    return out


def load_trusted(trusted_path: str) -> dict:
    with open(trusted_path, encoding="utf-8") as f:
        doc = json.load(f)
    keys = doc.get("keys") if isinstance(doc, dict) else None
    if not isinstance(keys, list) or not keys:
        raise SystemExit(f"[agent-card] FAIL: 신뢰 키 저장소 형식 오류({trusted_path}) — {{keys:[JWK...]}}")
    return {k["kid"]: k for k in keys if isinstance(k, dict) and k.get("kid")}


def verify_card(card: dict, trusted_path: str) -> str:
    """서명 검증 — 통과 시 kid 를 돌려주고, 부재/불일치/미신뢰는 예외(fail-closed)."""
    serialization, ed25519 = _crypto()
    sigs = card.get("signatures")
    if not isinstance(sigs, list) or not sigs:
        raise ContractViolation("signatures 부재 — 서명 없는 카드는 메인 발급을 증명하지 못한다(fail-closed)")
    trusted = load_trusted(trusted_path)
    payload = canonical_payload(card)
    errors = []
    for s in sigs:
        try:
            header = json.loads(_b64u_dec(s["protected"]))
            kid = header.get("kid")
            if header.get("alg") != JWS_ALG:
                errors.append(f"alg={header.get('alg')!r} 미지원"); continue
            jwk = trusted.get(kid)
            if not jwk:
                errors.append(f"kid={kid!r} 신뢰 저장소에 없음"); continue
            pub = ed25519.Ed25519PublicKey.from_public_bytes(_b64u_dec(jwk["x"]))
            signing_input = (s["protected"] + "." + _b64u(payload)).encode("ascii")
            pub.verify(_b64u_dec(s["signature"]), signing_input)
            return kid
        except Exception as e:  # noqa: BLE001 — 개별 서명 실패는 모아서 보고(전부 실패 시 fail-closed)
            errors.append(f"{type(e).__name__}: {e}")
    raise ContractViolation("서명 검증 실패: " + "; ".join(errors))


# ───────────────────────────── self-test ─────────────────────────────
def _fixture_card() -> dict:
    return {
        "name": "easy-vllm sub agent @ testhost",
        "description": "fixture",
        "supportedInterfaces": [{"url": "ssh://tester@testhost", "protocolBinding": SSH_BINDING_URI, "protocolVersion": "1.0"}],
        "provider": {"organization": "easy-vllm (self-hosted)", "url": "https://example.invalid/easy-vllm"},
        "version": "2.0.0",
        "documentationUrl": ".claude/rules/comms.md",
        "capabilities": {"streaming": False, "pushNotifications": False, "extendedAgentCard": False,
                          "extensions": [{"uri": NODE_ROLE_EXT_URI, "description": "node role (resolved by node_role_contract.py)",
                                          "required": True,
                                          "params": {"topology": "single", "sub_mode": "a2a-agent", "sub_mode_source": "contract",
                                                     "rank": None, "rank_source": "not-applicable:single-a2a-agent",
                                                     "identity_authority": "agent-card", "delivery_plane": "dormant",
                                                     "tool_plane": ["vllm-recipe-explorer"], "tool_plane_source": "contract"}}]},
        "defaultInputModes": ["application/json", "text/plain"],
        "defaultOutputModes": ["application/json"],
        "skills": [{"id": k, "name": k, "description": k, "tags": [k]} for k in EXPECTED_SKILL_IDS],
    }


def _self_test() -> int:
    import copy, tempfile
    ok = True
    def chk(name, cond, extra=""):
        nonlocal ok
        print(f"  [{'PASS' if cond else 'FAIL'}] {name}{(' — ' + extra) if extra else ''}")
        ok &= bool(cond)
    card = _fixture_card()
    chk("fixture 카드 계약 통과", validate_card(card) == [] and require_skills(card) == [])
    # 음성대조: 표준 밖 최상위 키 · 비-URI 바인딩 · 확장 params 누락 · float · skill 누락
    bad = copy.deepcopy(card); bad["node_identity"] = {"gpu_model": "X"}
    chk("표준 밖 최상위 키(node_identity) → 위반", any("표준 밖 최상위" in v for v in validate_card(bad)))
    bad = copy.deepcopy(card); bad["supportedInterfaces"][0]["protocolBinding"] = "ssh-claude-p"
    chk("비-URI 커스텀 바인딩 → 위반", any("URI" in v for v in validate_card(bad)))
    bad = copy.deepcopy(card); del bad["capabilities"]["extensions"][0]["params"]["gpu" if False else "tool_plane"]
    chk("node-role params 누락 → 위반", any("params 누락" in v for v in validate_card(bad)))
    bad = copy.deepcopy(card); bad["version"] = 1.5
    chk("float → 위반", any("float" in v for v in validate_card(bad)))
    bad = copy.deepcopy(card); bad["skills"] = bad["skills"][:4]
    chk("skill 6종 미만 → 누락 보고", require_skills(bad) == ["skill 누락: bench", "skill 누락: publish"])
    # 정규화 결정론 + 서명 왕복 + 위조 감지
    c1 = canonical_payload(card); c2 = canonical_payload(json.loads(json.dumps(card)))
    chk("JCS 정규화 결정론(같은 카드 → 같은 바이트)", c1 == c2 and b" " not in c1[:40])
    chk("signatures 는 정규화에서 제외", canonical_payload({**card, "signatures": [{"protected": "x", "signature": "y"}]}) == c1)
    with tempfile.TemporaryDirectory() as tmp:
        k = keygen(tmp)
        signed = sign_card(card, k["private"])
        trusted = os.path.join(tmp, "trusted.json")
        with open(k["public_jwk"], encoding="utf-8") as f:
            jwk = json.load(f)
        with open(trusted, "w", encoding="utf-8") as f:
            json.dump({"keys": [jwk]}, f)
        chk("서명 → 검증 통과(kid 일치)", verify_card(signed, trusted) == k["kid"])
        forged = copy.deepcopy(signed); forged["skills"][0]["description"] = "tampered"
        try:
            verify_card(forged, trusted); chk("위조(내용 변경) → 검증 실패", False)
        except ContractViolation as e:
            chk("위조(내용 변경) → 검증 실패", "검증 실패" in str(e))
        try:
            verify_card(card, trusted); chk("서명 부재 → fail-closed", False)
        except ContractViolation as e:
            chk("서명 부재 → fail-closed", "부재" in str(e))
        k2 = keygen(os.path.join(tmp, "other"))
        signed_other = sign_card(card, k2["private"])
        try:
            verify_card(signed_other, trusted); chk("미신뢰 키 서명 → 검증 실패", False)
        except ContractViolation as e:
            chk("미신뢰 키 서명 → 검증 실패", "신뢰 저장소에 없음" in str(e))
        chk("키 파일 권한 0600", oct(os.stat(k["private"]).st_mode & 0o777) == "0o600")
        try:
            keygen(tmp); chk("기존 키 덮어쓰기 거부(--force 없이)", False)
        except SystemExit as e:
            chk("기존 키 덮어쓰기 거부(--force 없이)", "이미 있다" in str(e))
    print(f"self-test: {'PASS' if ok else 'FAIL'}")
    return 0 if ok else 1


# ── 정체성 증명(2026-09-05 · plan_26090516 ③ 3-9 · G-E1) ─────────────────────────
#   위임 키(`.claude/a2a_delegation.json`)를 대체한다. 종전 규약은 "메인이 발급한 **실행 허가**가
#   없으면 서브는 아무것도 못 한다" 였고, 감사는 그것을 R3(에이전트 자율성 부정)로 판정했다.
#   지금 서브는 ② 이후 **자기 manifest**(main 이 발급한 Flag)와 **서명된 Agent Card** 를 갖는다 —
#   그러므로 필요한 것은 허가가 아니라 **"이 노드는 메인이 프로비저닝했다"는 증명**이다.
#   손상 키 fail-closed 규율은 그대로 이식한다: 카드·신뢰저장소가 없거나 서명이 깨지면 거부한다.
CARD_REL = "Agent_Card.json"
TRUSTED_REL = os.path.join(".claude", "a2a", "trusted_keys.json")


def _manifest_flag(repo_root: str, topology: str) -> dict:
    """서브가 **자기 manifest** 의 완수 Flag 를 읽는다(최소 파서 · 2026-09-05 · G-E1).

    왜 여기인가: 서브에는 `manifest_contract.py` 가 없다(terraforming 은 메인 전용이라 배달되지
    않는다). 종전에는 그 부재를 **위임 키 면제**로 메웠고, 그것이 R3 구조의 뿌리였다. 이제 서브는
    메인이 발급한 자기 manifest 를 가지므로, 그것을 읽어 정규 통과한다. 파서는 이 파일이 이미
    양 평면에 배달되므로 여기 둔다(새 배달 대상을 만들지 않는다).
    """
    path = os.path.join(repo_root, "output", topology or "single", "manifest.yaml")
    if not os.path.isfile(path):
        raise ContractViolation("manifest 부재(%s) — 완수 Flag 를 확인할 수 없다" % path)
    flags, in_terra = {}, False
    with open(path, encoding="utf-8") as f:
        for line in f:
            if not line.strip() or line.lstrip().startswith("#"):
                continue
            if not line[:1].isspace():
                in_terra = line.startswith("terraforming:")
                continue
            if in_terra and ":" in line:
                k, _, v = line.strip().partition(":")
                flags[k.strip()] = v.split("#", 1)[0].strip().strip('"').lower()
    if flags.get("complete") != "true" or flags.get("branch_verified") != "true":
        raise ContractViolation(
            "완수 Flag 미발급(complete=%r branch_verified=%r · %s) — 메인의 terraforming 이 "
            "발급한다(issued_by: main)" % (flags.get("complete"), flags.get("branch_verified"), path))
    return {"manifest": os.path.relpath(path, repo_root), "issued_by": flags.get("issued_by")}

def prove_identity(repo_root: str, require_flag: bool = False) -> dict:
    """워크스페이스의 정체성 증명을 검증한다. 실패는 예외(ContractViolation)다.

    반환: {kid, role, topology, self_role} — 호출부(recipe·bench 게이트)가 그대로 로그에 적는다.
    """
    card_path = os.path.join(repo_root, CARD_REL)
    trusted_path = os.path.join(repo_root, TRUSTED_REL)
    if not os.path.isfile(card_path):
        raise ContractViolation("Agent_Card.json 부재(%s) — 정체성 증명 없음(fail-closed)" % card_path)
    if not os.path.isfile(trusted_path):
        raise ContractViolation("신뢰키 저장소 부재(%s) — 서명을 검증할 수 없다(fail-closed)" % trusted_path)
    with open(card_path, encoding="utf-8") as f:
        card = json.load(f)
    kid = verify_card(card, trusted_path)
    v = validate_card(card)
    if v:
        raise ContractViolation("서명은 유효하나 카드 계약 위반: " + "; ".join(v))
    params = {}
    for ext in ((card.get("capabilities") or {}).get("extensions") or []):
        if str(ext.get("uri", "")).startswith("urn:easy-vllm:ext:node-role"):
            params = ext.get("params") or {}
            break
    out = {"kid": kid, "role": params.get("sub_mode"), "topology": params.get("topology"),
           "rank": params.get("rank")}
    if require_flag:
        out.update(_manifest_flag(repo_root, out["topology"]))
    return out

def main(argv=None) -> int:
    ap = argparse.ArgumentParser(description="Agent_Card(A2A v1.0.1) 계약 검증 · JWS 서명/검증")
    ap.add_argument("--self-test", action="store_true")
    sub = ap.add_subparsers(dest="cmd")
    p = sub.add_parser("validate"); p.add_argument("--card", required=True); p.add_argument("--require-skills", action="store_true", default=True)
    p = sub.add_parser("keygen"); p.add_argument("--out-dir", required=True); p.add_argument("--force", action="store_true")
    p = sub.add_parser("sign"); p.add_argument("--card", required=True); p.add_argument("--key", required=True)
    p.add_argument("--trusted-out", help="서명 공개키(JWK)를 신뢰 저장소 파일로 기록(서브 오버레이 배달용)")
    p.add_argument("--out", help="서명된 카드 출력 경로(기본 = --card 덮어쓰기)")
    p = sub.add_parser("verify"); p.add_argument("--card", required=True); p.add_argument("--trusted", required=True)
    p = sub.add_parser("prove-identity",
                       help="워크스페이스의 정체성 증명(서명된 카드 + 신뢰저장소)을 검증한다 — 위임 키 대체")
    p.add_argument("--repo-root", required=True)
    p.add_argument("--require-flag", action="store_true",
                   help="정체성에 더해 **자기 manifest 의 완수 Flag** 까지 요구(서브의 정규 통과 경로)")
    a = ap.parse_args(argv)
    if a.self_test:
        return _self_test()
    if not a.cmd:
        ap.print_help(); return EXIT_USAGE
    if a.cmd == "prove-identity":
        try:
            r = prove_identity(a.repo_root, getattr(a, "require_flag", False))
        except (ContractViolation, ValueError, OSError) as e:
            print("[agent-card] IDENTITY FAIL: %s" % e, file=sys.stderr); return EXIT_SIGNATURE
        print(json.dumps(r, ensure_ascii=False)); return EXIT_OK
    if a.cmd == "keygen":
        r = keygen(a.out_dir, a.force)
        print(json.dumps(r, ensure_ascii=False)); return EXIT_OK
    with open(a.card, encoding="utf-8") as f:
        card = json.load(f)
    if a.cmd == "validate":
        v = validate_card(card) + require_skills(card)
        if v:
            print("[agent-card] 계약 위반:\n  - " + "\n  - ".join(v), file=sys.stderr); return EXIT_CONTRACT
        print(f"[agent-card] OK: 표준 필드 {len(card)}개 · skills {len(card['skills'])} · signatures {len(card.get('signatures') or [])}")
        return EXIT_OK
    if a.cmd == "sign":
        try:
            signed = sign_card(card, a.key)
        except ContractViolation as e:
            print(f"[agent-card] FAIL: {e}", file=sys.stderr); return EXIT_CONTRACT
        out = a.out or a.card
        with open(out, "w", encoding="utf-8") as f:
            json.dump(signed, f, ensure_ascii=False, indent=2); f.write("\n")
        if a.trusted_out:
            serialization, _ = _crypto()
            key = _load_private(a.key)
            pub = key.public_key().public_bytes(serialization.Encoding.Raw, serialization.PublicFormat.Raw)
            jwk = jwk_from_public(pub); jwk["issued_by"] = "main"
            os.makedirs(os.path.dirname(os.path.abspath(a.trusted_out)), exist_ok=True)
            with open(a.trusted_out, "w", encoding="utf-8") as f:
                json.dump({"schema_version": 1, "keys": [jwk]}, f, ensure_ascii=False, indent=2); f.write("\n")
        print(f"[agent-card] signed: {out} kid={json.loads(_b64u_dec(signed['signatures'][0]['protected']))['kid']}")
        return EXIT_OK
    if a.cmd == "verify":
        try:
            kid = verify_card(card, a.trusted)
        except ContractViolation as e:
            print(f"[agent-card] FAIL: {e}", file=sys.stderr); return EXIT_SIGNATURE
        v = validate_card(card)
        if v:
            print("[agent-card] 서명은 유효하나 계약 위반:\n  - " + "\n  - ".join(v), file=sys.stderr); return EXIT_CONTRACT
        print(f"[agent-card] verified: kid={kid}"); return EXIT_OK
    return EXIT_USAGE


if __name__ == "__main__":
    sys.exit(main())
