#!/usr/bin/env python3
"""Sub-side authenticated A2A executor.

Order is fixed: identity proof -> independent authorization -> durable replay
consume -> provider invocation -> authenticated report.  This entrypoint never
falls back to a different backend or model.
"""
from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
if str(HERE) not in sys.path:
    sys.path.insert(0, str(HERE))
import a2a_identity as identity

AGENT_CONTROL = HERE / "agent_control.py"

class AuthorizationError(RuntimeError):
    pass


def authorize(payload: dict) -> dict:
    """Validate signed authority facts separately from cryptographic identity."""
    required = ("agent_request", "authorization", "instruction_id")
    if any(k not in payload for k in required) or not isinstance(payload.get("agent_request"), dict) \
            or not isinstance(payload.get("authorization"), dict):
        raise AuthorizationError("signed payload lacks agent_request, authorization, or instruction_id")
    auth = payload["authorization"]
    for field in ("work_manifest_digest", "instruction_id", "capabilities", "scope", "gate", "campaign"):
        if field not in auth:
            raise AuthorizationError(f"authorization lacks {field}")
    request = payload["agent_request"]
    if request.get("capabilities") != auth.get("capabilities"):
        raise AuthorizationError("signed capabilities differ from authorization")
    if payload["instruction_id"] != auth["instruction_id"]:
        raise AuthorizationError("signed instruction differs from authorization")
    scope = auth.get("scope")
    if not isinstance(scope, dict) or scope.get("target_peer_id") != request.get("target", {}).get("peer_id"):
        raise AuthorizationError("signed target peer differs from authorization scope")
    if scope.get("task_digest") != identity.digest(request.get("task")):
        raise AuthorizationError("signed task differs from authorization scope")
    if not isinstance(auth.get("work_manifest_digest"), str) or len(auth["work_manifest_digest"]) != 64:
        raise AuthorizationError("work manifest digest is not a bound SHA-256 value")
    gate = auth.get("gate")
    if not isinstance(gate, dict) or gate.get("action") != "a2a_delegate" \
            or gate.get("authorization_state") != "execution-approved":
        raise AuthorizationError("current HITL authorization gate is not execution-approved")
    campaign = auth.get("campaign")
    expected_campaign = request.get("campaign_id")
    if not isinstance(campaign, dict) or campaign.get("campaign_id") != expected_campaign:
        raise AuthorizationError("signed campaign assignment differs from request")
    if campaign.get("control_variables") != request.get("control_variables"):
        raise AuthorizationError("signed campaign control variables differ from request")
    if auth.get("status") != "authorized":
        raise AuthorizationError("current HITL/campaign authorization is not authorized")
    return {"status": "AUTHORIZED", "instruction_id": auth["instruction_id"],
            "work_manifest_digest": auth["work_manifest_digest"]}


def execute(envelope: dict, state: Path, consumed_utc: str) -> dict:
    proof = identity.verify_envelope(state, envelope, "request")
    payload = envelope["payload"]
    authority = authorize(payload)
    identity.consume_attempt(state, proof, payload, consumed_utc)
    request = dict(payload["agent_request"])
    request["target"] = dict(request["target"])
    request["target"]["role"] = "main"
    request["target"]["transport"] = "local"
    for key in ("host", "ssh_user", "peer_id", "a2a_state_root"):
        request["target"].pop(key, None)
    cp = subprocess.run([sys.executable, str(AGENT_CONTROL), "invoke", "--request", "-"],
                        input=json.dumps(request), capture_output=True, text=True)
    try:
        result = json.loads(cp.stdout)
    except ValueError as exc:
        raise RuntimeError("local provider adapter returned malformed control output") from exc
    report_payload = {
        "request_envelope_digest": proof.envelope_digest,
        "request_attempt_id": proof.attempt_id,
        "instruction_id": authority["instruction_id"],
        "runner": {"backend": request.get("backend", "anthropic"), "model": request["model"]},
        "control_result": result,
        "control_result_digest": identity.digest(result),
    }
    return identity.sign_envelope(state, proof.sender_peer_id, {
        "envelope_type": "report", "attempt_id": proof.attempt_id,
        "payload": report_payload, "payload_digest": identity.digest(report_payload),
    })


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--request-envelope", required=True)
    ap.add_argument("--repo", default="."); ap.add_argument("--state-dir")
    ap.add_argument("--consumed-utc", required=True)
    a = ap.parse_args()
    state = identity.resolve_state_dir(a.repo, a.state_dir)
    if a.request_envelope == "-":
        envelope = json.load(sys.stdin)
    else:
        with open(a.request_envelope, encoding="utf-8") as f:
            envelope = json.load(f)
    print(json.dumps(execute(envelope, state, a.consumed_utc), ensure_ascii=False, sort_keys=True))
    return 0

if __name__ == "__main__":
    try: raise SystemExit(main())
    except (identity.IdentityError, AuthorizationError, RuntimeError) as exc:
        print(f"[a2a-executor] FAIL: {exc}", file=sys.stderr); raise SystemExit(6)
