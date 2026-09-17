#!/usr/bin/env python3
"""Reciprocal A2A message identity, enrollment, and replay protection.

Identity proves who authenticated an immutable request/report.  It never grants
execution authority.  Production state lives only below git's common directory;
tests may inject a fresh state directory with EASY_VLLM_A2A_TEST_STATE=1.
"""
from __future__ import annotations

import argparse
import base64
import hashlib
import json
import os
import secrets
import sqlite3
import stat
import subprocess
import sys
from dataclasses import asdict, dataclass
from pathlib import Path
from typing import Any

VERSION = 1
DOMAIN = b"easy-vllm/a2a-envelope/v1\0"
STATE_SUBDIR = "easy-vllm-a2a/v1"
TEST_SENTINEL = "EASY_VLLM_A2A_TEST_STATE"
PRIVATE_MODE = 0o600
DIR_MODE = 0o700
ENVELOPE_TYPES = frozenset({"request", "report", "rotation_transition", "rotation_ack", "revoke"})
PEER_ID_RE = __import__("re").compile(r"^[0-9a-f]{32}$")


def validate_peer_id(peer_id: str) -> str:
    if not isinstance(peer_id, str) or not PEER_ID_RE.fullmatch(peer_id):
        raise IdentityError("peer id must be exactly 32 lowercase hex characters")
    return peer_id

class IdentityError(RuntimeError):
    pass

@dataclass(frozen=True)
class IdentityProof:
    sender_peer_id: str
    recipient_peer_id: str
    sender_epoch: int
    attempt_id: str
    envelope_digest: str
    key_fingerprint: str


def _crypto():
    try:
        from cryptography.hazmat.primitives import serialization
        from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PrivateKey, Ed25519PublicKey
    except ImportError as exc:
        raise IdentityError("cryptography Ed25519 support is required") from exc
    return serialization, Ed25519PrivateKey, Ed25519PublicKey


def _b64u(raw: bytes) -> str:
    return base64.urlsafe_b64encode(raw).rstrip(b"=").decode("ascii")


def _b64u_dec(text: str) -> bytes:
    try:
        return base64.urlsafe_b64decode(text + "=" * (-len(text) % 4))
    except Exception as exc:
        raise IdentityError("invalid base64url") from exc


def _no_float(value: Any) -> None:
    if isinstance(value, float):
        raise IdentityError("floating point values are forbidden in signed envelopes")
    if isinstance(value, dict):
        for k, v in value.items():
            if not isinstance(k, str):
                raise IdentityError("object keys must be strings")
            _no_float(v)
    elif isinstance(value, list):
        for v in value:
            _no_float(v)


def canonical_json(value: Any) -> bytes:
    _no_float(value)
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False).encode()


def digest(value: Any) -> str:
    return "sha256:" + hashlib.sha256(canonical_json(value)).hexdigest()


def fingerprint(public_raw: bytes) -> str:
    return "SHA256:" + hashlib.sha256(public_raw).hexdigest()


def resolve_state_dir(repo: str | Path = ".", override: str | None = None) -> Path:
    if override is not None:
        if os.environ.get(TEST_SENTINEL) != "1":
            raise IdentityError("state override is test-only and requires EASY_VLLM_A2A_TEST_STATE=1")
        path = Path(override).resolve()
        if path.exists() and path.is_symlink():
            raise IdentityError("test state directory may not be a symlink")
        return path
    try:
        cp = subprocess.run(
            ["git", "-C", str(repo), "rev-parse", "--path-format=absolute", "--git-common-dir"],
            check=True, capture_output=True, text=True,
        )
    except (OSError, subprocess.CalledProcessError) as exc:
        raise IdentityError("production identity requires a resolvable git common directory") from exc
    common = Path(cp.stdout.strip())
    if not common.is_absolute():
        common = (Path(repo) / common).resolve()
    state = common / STATE_SUBDIR
    if state.exists() and (state.is_symlink() or not state.is_dir()):
        raise IdentityError("identity state root is not a safe directory")
    return state


def _ensure_dir(path: Path, create: bool = False) -> None:
    if create:
        path.mkdir(parents=True, exist_ok=True, mode=DIR_MODE)
        os.chmod(path, DIR_MODE)
    if not path.is_dir() or path.is_symlink():
        raise IdentityError(f"unsafe state directory: {path}")
    if stat.S_IMODE(path.stat().st_mode) & 0o077:
        raise IdentityError(f"state directory permissions are too broad: {path}")


def _read_json(path: Path) -> dict:
    if not path.is_file() or path.is_symlink():
        raise IdentityError(f"missing or unsafe state file: {path}")
    with path.open(encoding="utf-8") as f:
        data = json.load(f, parse_float=lambda _x: (_ for _ in ()).throw(IdentityError("floats forbidden")))
    if not isinstance(data, dict):
        raise IdentityError(f"state file is not an object: {path}")
    return data


def _write_new(path: Path, data: bytes, mode: int = PRIVATE_MODE) -> None:
    path.parent.mkdir(parents=True, exist_ok=True, mode=DIR_MODE)
    flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL
    fd = os.open(path, flags, mode)
    try:
        os.write(fd, data)
        os.fsync(fd)
    finally:
        os.close(fd)


def _atomic_json(path: Path, data: dict, mode: int = PRIVATE_MODE) -> None:
    path.parent.mkdir(parents=True, exist_ok=True, mode=DIR_MODE)
    tmp = path.with_name(path.name + ".tmp-" + secrets.token_hex(8))
    fd = os.open(tmp, os.O_WRONLY | os.O_CREAT | os.O_EXCL, mode)
    try:
        with os.fdopen(fd, "wb", closefd=False) as f:
            f.write(canonical_json(data) + b"\n")
            f.flush(); os.fsync(f.fileno())
    finally:
        os.close(fd)
    os.replace(tmp, path)


def init_endpoint(state: Path, apply: bool = False) -> dict:
    plan = {"action": "init-endpoint", "state_root": str(state), "creates": ["endpoint.json", "replay.sqlite3"]}
    if not apply:
        return {"applied": False, "plan": plan}
    if state.exists():
        raise IdentityError("identity state already exists; automatic replacement is forbidden")
    state.mkdir(parents=True, mode=DIR_MODE)
    endpoint = {"schema_version": VERSION, "peer_id": secrets.token_hex(16), "status": "initialized"}
    _write_new(state / "endpoint.json", canonical_json(endpoint) + b"\n")
    _init_replay(state / "replay.sqlite3")
    return {"applied": True, "plan": plan, "peer_id": endpoint["peer_id"]}


def _init_replay(path: Path) -> None:
    con = sqlite3.connect(path)
    try:
        con.executescript("""
        PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL;
        CREATE TABLE IF NOT EXISTS consumed_attempt(
          sender_peer_id TEXT NOT NULL, sender_epoch INTEGER NOT NULL,
          message_kind TEXT NOT NULL, attempt_id TEXT NOT NULL,
          envelope_digest TEXT NOT NULL UNIQUE,
          predecessor_attempt_id TEXT, predecessor_request_digest TEXT,
          consumed_utc TEXT NOT NULL, state TEXT NOT NULL,
          PRIMARY KEY(sender_peer_id, sender_epoch, message_kind, attempt_id));
        CREATE TABLE IF NOT EXISTS epoch_revocations(
          peer_id TEXT NOT NULL, epoch INTEGER NOT NULL, reason_code TEXT NOT NULL,
          revoked_utc TEXT NOT NULL, PRIMARY KEY(peer_id, epoch));
        """)
        con.commit()
    finally:
        con.close()
    os.chmod(path, PRIVATE_MODE)


def generate_peer_key(state: Path, remote_peer_id: str, apply: bool = False) -> dict:
    """Create this endpoint's peer-scoped key; never creates or accepts remote trust."""
    plan = {"action": "generate-peer-key", "remote_peer_id": remote_peer_id}
    if not apply:
        return {"applied": False, "plan": plan}
    _ensure_dir(state)
    peer_dir = state / "peers" / validate_peer_id(remote_peer_id)
    if peer_dir.exists():
        raise IdentityError("peer state already exists")
    peer_dir.mkdir(parents=True, mode=DIR_MODE)
    serialization, Private, _ = _crypto()
    private = Private.generate()
    private_bytes = private.private_bytes(serialization.Encoding.PEM, serialization.PrivateFormat.PKCS8,
                                          serialization.NoEncryption())
    public_raw = private.public_key().public_bytes(serialization.Encoding.Raw, serialization.PublicFormat.Raw)
    _write_new(peer_dir / "signing.pem", private_bytes)
    local = {"schema_version": VERSION, "epoch": 1, "public_key": _b64u(public_raw),
             "fingerprint": fingerprint(public_raw), "status": "staged"}
    _write_new(peer_dir / "public.json", canonical_json(local) + b"\n")
    return {"applied": True, "plan": plan, "public": local}


def stage_remote_peer(state: Path, remote_peer_id: str, public_key: str,
                      expected_fingerprint: str, apply: bool = False) -> dict:
    raw = _b64u_dec(public_key)
    actual = fingerprint(raw)
    if not secrets.compare_digest(actual, expected_fingerprint):
        raise IdentityError("human-approved fingerprint does not match supplied public key")
    plan = {"action": "enroll-stage", "peer_id": remote_peer_id, "fingerprint": actual}
    if not apply:
        return {"applied": False, "plan": plan}
    _ensure_dir(state)
    path = state / "peers" / validate_peer_id(remote_peer_id) / "peer.json"
    if path.exists():
        raise IdentityError("peer trust already exists; use the rotation protocol")
    record = {"schema_version": VERSION, "peer_id": remote_peer_id, "epoch": 1,
              "public_key": public_key, "fingerprint": actual, "status": "staged"}
    _write_new(path, canonical_json(record) + b"\n")
    return {"applied": True, "plan": plan}


def activate_peer(state: Path, remote_peer_id: str, approved_fingerprint: str,
                  apply: bool = False) -> dict:
    path = state / "peers" / validate_peer_id(remote_peer_id) / "peer.json"
    peer = _read_json(path)
    local = _read_json(path.parent / "public.json")
    if peer.get("status") != "staged" or local.get("status") != "staged":
        raise IdentityError("reciprocal enrollment is not staged")
    if not secrets.compare_digest(str(peer.get("fingerprint")), approved_fingerprint):
        raise IdentityError("activation fingerprint mismatch")
    plan = {"action": "enroll-activate", "peer_id": remote_peer_id,
            "remote_fingerprint": approved_fingerprint}
    if not apply:
        return {"applied": False, "plan": plan}
    peer["status"] = "active"; local["status"] = "active"
    _atomic_json(path, peer); _atomic_json(path.parent / "public.json", local)
    return {"applied": True, "plan": plan}


def _load_private(path: Path):
    if not path.is_file() or path.is_symlink() or stat.S_IMODE(path.stat().st_mode) != PRIVATE_MODE:
        raise IdentityError("private key is missing or has unsafe permissions")
    serialization, _, _ = _crypto()
    return serialization.load_pem_private_key(path.read_bytes(), password=None)


def envelope_payload(envelope: dict) -> dict:
    return {k: v for k, v in envelope.items() if k != "signature"}


def sign_envelope(state: Path, remote_peer_id: str, envelope: dict) -> dict:
    peer_dir = state / "peers" / validate_peer_id(remote_peer_id)
    endpoint = _read_json(state / "endpoint.json")
    local = _read_json(peer_dir / "public.json")
    if local.get("status") != "active":
        raise IdentityError("local peer-scoped key is not active")
    doc = dict(envelope)
    doc.setdefault("schema_version", VERSION)
    doc["sender_peer_id"] = endpoint["peer_id"]
    doc["recipient_peer_id"] = remote_peer_id
    doc["sender_epoch"] = local["epoch"]
    _validate_unsigned(doc)
    message = DOMAIN + doc["envelope_type"].encode() + b"\0" + canonical_json(doc)
    doc["signature"] = _b64u(_load_private(peer_dir / "signing.pem").sign(message))
    return doc


def _validate_unsigned(doc: dict) -> None:
    required = {"schema_version", "envelope_type", "sender_peer_id", "recipient_peer_id",
                "sender_epoch", "attempt_id", "payload", "payload_digest"}
    if set(doc) != required:
        raise IdentityError(f"unsigned envelope fields mismatch: {sorted(set(doc) ^ required)}")
    if doc["schema_version"] != VERSION or doc["envelope_type"] not in ENVELOPE_TYPES:
        raise IdentityError("unsupported envelope version/type")
    for field in ("sender_peer_id", "recipient_peer_id", "attempt_id"):
        if not isinstance(doc[field], str) or not doc[field]:
            raise IdentityError(f"invalid {field}")
    if not isinstance(doc["sender_epoch"], int) or isinstance(doc["sender_epoch"], bool) or doc["sender_epoch"] < 1:
        raise IdentityError("invalid sender_epoch")
    if doc["payload_digest"] != digest(doc["payload"]):
        raise IdentityError("payload digest mismatch")
    _no_float(doc)


def verify_envelope(state: Path, envelope: dict, expected_type: str,
                    expected_recipient: str | None = None) -> IdentityProof:
    if not isinstance(envelope, dict) or set(envelope) != {
        "schema_version", "envelope_type", "sender_peer_id", "recipient_peer_id", "sender_epoch",
        "attempt_id", "payload", "payload_digest", "signature"}:
        raise IdentityError("signed envelope shape mismatch")
    unsigned = envelope_payload(envelope); _validate_unsigned(unsigned)
    if unsigned["envelope_type"] != expected_type:
        raise IdentityError("envelope type mismatch")
    endpoint = _read_json(state / "endpoint.json")
    recipient = validate_peer_id(expected_recipient or endpoint.get("peer_id"))
    if unsigned["recipient_peer_id"] != recipient:
        raise IdentityError("wrong envelope recipient")
    peer = _read_json(state / "peers" / validate_peer_id(unsigned["sender_peer_id"]) / "peer.json")
    if peer.get("status") != "active" or peer.get("epoch") != unsigned["sender_epoch"]:
        raise IdentityError("unknown, inactive, or stale peer epoch")
    raw = _b64u_dec(peer["public_key"])
    _, _, Public = _crypto()
    message = DOMAIN + expected_type.encode() + b"\0" + canonical_json(unsigned)
    try:
        Public.from_public_bytes(raw).verify(_b64u_dec(envelope["signature"]), message)
    except Exception as exc:
        raise IdentityError("signature verification failed") from exc
    return IdentityProof(unsigned["sender_peer_id"], recipient, unsigned["sender_epoch"],
                         unsigned["attempt_id"], digest(envelope), fingerprint(raw))


def consume_attempt(state: Path, proof: IdentityProof, payload: dict, consumed_utc: str,
                    message_kind: str = "request") -> None:
    predecessor_id = payload.get("predecessor_attempt_id")
    predecessor_digest = payload.get("predecessor_request_digest")
    db = state / "replay.sqlite3"
    if not db.is_file() or db.is_symlink():
        raise IdentityError("replay state is missing; re-enrollment is required")
    con = sqlite3.connect(db, timeout=30, isolation_level=None)
    try:
        con.execute("PRAGMA synchronous=FULL")
        con.execute("BEGIN IMMEDIATE")
        revoked = con.execute("SELECT 1 FROM epoch_revocations WHERE peer_id=? AND epoch=?",
                              (proof.sender_peer_id, proof.sender_epoch)).fetchone()
        if revoked:
            raise IdentityError("sender epoch is revoked")
        if predecessor_id or predecessor_digest:
            if not (predecessor_id and predecessor_digest):
                raise IdentityError("predecessor id and digest must appear together")
            prev = con.execute(
                "SELECT envelope_digest FROM consumed_attempt WHERE sender_peer_id=? AND sender_epoch=? "
                "AND message_kind='request' AND attempt_id=?",
                (proof.sender_peer_id, proof.sender_epoch, predecessor_id)).fetchone()
            if not prev or prev[0] != predecessor_digest:
                raise IdentityError("predecessor binding mismatch")
        try:
            con.execute("INSERT INTO consumed_attempt VALUES(?,?,?,?,?,?,?,?,?)",
                        (proof.sender_peer_id, proof.sender_epoch, message_kind, proof.attempt_id,
                         proof.envelope_digest, predecessor_id, predecessor_digest,
                         consumed_utc, "consumed"))
        except sqlite3.IntegrityError as exc:
            raise IdentityError("duplicate attempt or envelope digest") from exc
        con.commit()
    except Exception:
        con.rollback()
        raise
    finally:
        con.close()


def revoke_epoch(state: Path, peer_id: str, epoch: int, reason: str, utc: str, apply: bool = False) -> dict:
    plan = {"action": "revoke-epoch", "peer_id": peer_id, "epoch": epoch, "reason": reason}
    if not apply:
        return {"applied": False, "plan": plan}
    db = state / "replay.sqlite3"
    con = sqlite3.connect(db)
    try:
        con.execute("INSERT INTO epoch_revocations VALUES(?,?,?,?)", (peer_id, epoch, reason, utc)); con.commit()
    finally:
        con.close()
    peer_path = state / "peers" / validate_peer_id(peer_id) / "peer.json"
    peer = _read_json(peer_path); peer["status"] = "revoked"; _atomic_json(peer_path, peer)
    return {"applied": True, "plan": plan}


def rotate_prepare(state: Path, remote_peer_id: str, apply: bool) -> dict:
    """Stage a new peer-scoped key and old-key-signed transition; do not activate it."""
    peer_dir = state / "peers" / validate_peer_id(remote_peer_id)
    current = _read_json(peer_dir / "public.json")
    if current.get("status") != "active":
        raise IdentityError("rotation requires an active local key")
    _, Private, _ = _crypto()
    private = Private.generate()
    serialization, _, _ = _crypto()
    public = private.public_key().public_bytes(
        serialization.Encoding.Raw, serialization.PublicFormat.Raw)
    next_epoch = int(current["epoch"]) + 1
    pop_statement = {"peer_id": remote_peer_id, "new_epoch": next_epoch,
                     "new_public_key": _b64u(public)}
    transition = {
        "peer_id": remote_peer_id,
        "old_epoch": current["epoch"],
        "new_epoch": next_epoch,
        "new_public_key": _b64u(public),
        "new_fingerprint": fingerprint(public),
        "new_key_pop": _b64u(private.sign(DOMAIN + b"rotation-pop\0" + canonical_json(pop_statement))),
    }
    signed = sign_envelope(state, remote_peer_id, {
        "envelope_type": "rotation_transition",
        "attempt_id": secrets.token_hex(16),
        "payload": transition,
        "payload_digest": digest(transition),
    })
    result = {"action": "rotate-prepare", "apply": apply, "transition": signed}
    if not apply:
        return result
    pending_key = peer_dir / "signing.pending.pem"
    pending_meta = peer_dir / "rotation.json"
    if pending_key.exists() or pending_meta.exists():
        raise IdentityError("a rotation is already pending")
    serialization, _, _ = _crypto()
    _write_new(pending_key, private.private_bytes(
        serialization.Encoding.PEM, serialization.PrivateFormat.PKCS8,
        serialization.NoEncryption()), PRIVATE_MODE)
    _write_new(pending_meta, (json.dumps({
        "status": "prepared", "new_epoch": next_epoch,
        "new_public_key": _b64u(public), "new_fingerprint": fingerprint(public),
        "transition_digest": digest(signed),
    }, ensure_ascii=False, sort_keys=True) + "\n").encode(), PRIVATE_MODE)
    return result


def rotate_ack(state: Path, transition: dict, apply: bool) -> dict:
    """Verify an old-key-signed transition and stage the remote epoch with new-key PoP."""
    proof = verify_envelope(state, transition, "rotation_transition")
    payload = transition["payload"]
    if payload.get("peer_id") != proof.recipient_peer_id:
        raise IdentityError("rotation transition peer mismatch")
    result = {"action": "rotate-ack", "apply": apply,
              "remote_peer_id": proof.sender_peer_id,
              "new_epoch": payload.get("new_epoch"),
              "transition_digest": proof.envelope_digest}
    ack_payload = {"transition_digest": proof.envelope_digest,
                   "new_epoch": payload.get("new_epoch"),
                   "initiator_peer_id": proof.sender_peer_id}
    result["ack"] = sign_envelope(state, proof.sender_peer_id, {
        "envelope_type": "rotation_ack", "attempt_id": secrets.token_hex(16),
        "payload": ack_payload, "payload_digest": digest(ack_payload),
    })
    if not apply:
        return result
    peer_path = state / "peers" / validate_peer_id(proof.sender_peer_id) / "peer.json"
    peer = _read_json(peer_path)
    if payload.get("old_epoch") != peer.get("epoch") or payload.get("new_epoch") != int(peer["epoch"]) + 1:
        raise IdentityError("rotation epoch transition mismatch")
    raw = _b64u_dec(payload.get("new_public_key", ""))
    if fingerprint(raw) != payload.get("new_fingerprint"):
        raise IdentityError("rotation new-key fingerprint mismatch")
    _, _, Public = _crypto()
    pop_statement = {"peer_id": payload["peer_id"], "new_epoch": payload["new_epoch"],
                     "new_public_key": payload["new_public_key"]}
    try:
        Public.from_public_bytes(raw).verify(
            _b64u_dec(payload.get("new_key_pop", "")),
            DOMAIN + b"rotation-pop\0" + canonical_json(pop_statement))
    except Exception as exc:
        raise IdentityError("rotation new-key proof-of-possession failed") from exc
    peer.update({"pending_epoch": payload["new_epoch"],
                 "pending_public_key": payload["new_public_key"],
                 "pending_fingerprint": payload["new_fingerprint"],
                 "pending_transition_digest": proof.envelope_digest})
    _atomic_json(peer_path, peer, PRIVATE_MODE)
    return result


def rotate_finalize(state: Path, remote_peer_id: str, ack: dict, apply: bool) -> dict:
    """Activate a prepared key only after a signed reciprocal acknowledgement."""
    peer_dir = state / "peers" / validate_peer_id(remote_peer_id)
    rotation = _read_json(peer_dir / "rotation.json")
    proof = verify_envelope(state, ack, "rotation_ack")
    if proof.sender_peer_id != remote_peer_id:
        raise IdentityError("rotation acknowledgement signer mismatch")
    ack_payload = ack["payload"]
    if (rotation.get("status") != "prepared"
            or ack_payload.get("transition_digest") != rotation.get("transition_digest")
            or ack_payload.get("new_epoch") != rotation.get("new_epoch")):
        raise IdentityError("rotation acknowledgement does not match the pending transition")
    result = {"action": "rotate-finalize", "apply": apply,
              "peer_id": remote_peer_id, "new_epoch": rotation["new_epoch"]}
    if not apply:
        return result
    consume_attempt(state, proof, {}, "rotation-finalize", message_kind="rotation_ack")
    pending = peer_dir / "signing.pending.pem"
    if not pending.is_file() or pending.is_symlink():
        raise IdentityError("pending rotation key is missing or unsafe")
    os.replace(pending, peer_dir / "signing.pem")
    _atomic_json(peer_dir / "public.json", {
        "epoch": rotation["new_epoch"], "public_key": rotation["new_public_key"],
        "fingerprint": rotation["new_fingerprint"], "status": "active",
    }, PRIVATE_MODE)
    rotation["status"] = "finalized"
    _atomic_json(peer_dir / "rotation.json", rotation, PRIVATE_MODE)
    return result


def _self_test() -> int:
    import tempfile
    old = os.environ.get(TEST_SENTINEL); os.environ[TEST_SENTINEL] = "1"
    try:
        with tempfile.TemporaryDirectory() as td:
            root = Path(td); a = root/"a"; b = root/"b"
            init_endpoint(a, True); init_endpoint(b, True)
            aid = _read_json(a/"endpoint.json")["peer_id"]; bid = _read_json(b/"endpoint.json")["peer_id"]
            apub = generate_peer_key(a, bid, True)["public"]
            bpub = generate_peer_key(b, aid, True)["public"]
            stage_remote_peer(a, bid, bpub["public_key"], bpub["fingerprint"], True)
            stage_remote_peer(b, aid, apub["public_key"], apub["fingerprint"], True)
            activate_peer(a, bid, bpub["fingerprint"], True); activate_peer(b, aid, apub["fingerprint"], True)
            payload = {"instruction_id":"i1", "authorization_ref":"fixture", "capabilities":["read"]}
            req = sign_envelope(a, bid, {"envelope_type":"request", "attempt_id":secrets.token_hex(16),
                "payload":payload, "payload_digest":digest(payload)})
            proof = verify_envelope(b, req, "request")
            if hasattr(proof, "allowed"):
                raise AssertionError("identity proof must not carry authorization")
            consume_attempt(b, proof, payload, "2026-09-17T00:00:00Z")
            try: consume_attempt(b, proof, payload, "2026-09-17T00:00:01Z")
            except IdentityError: pass
            else: raise AssertionError("duplicate replay accepted")
            bad = json.loads(json.dumps(req)); bad["payload"]["instruction_id"] = "evil"
            try: verify_envelope(b, bad, "request")
            except IdentityError: pass
            else: raise AssertionError("tamper accepted")
            print("[a2a-identity] self-test PASS")
            return 0
    finally:
        if old is None: os.environ.pop(TEST_SENTINEL, None)
        else: os.environ[TEST_SENTINEL] = old


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--repo", default=".")
    ap.add_argument("--state-dir")
    ap.add_argument("--self-test", action="store_true")
    sub = ap.add_subparsers(dest="cmd")
    init = sub.add_parser("init-endpoint")
    init.add_argument("--apply", action="store_true")
    gen = sub.add_parser("generate-peer-key")
    gen.add_argument("--peer-id", required=True)
    gen.add_argument("--apply", action="store_true")
    stage = sub.add_parser("enroll-stage")
    stage.add_argument("--peer-id", required=True)
    stage.add_argument("--public-key", required=True)
    stage.add_argument("--expected-fingerprint", required=True)
    stage.add_argument("--apply", action="store_true")
    act = sub.add_parser("enroll-activate")
    act.add_argument("--peer-id", required=True)
    act.add_argument("--approved-fingerprint", required=True)
    act.add_argument("--apply", action="store_true")
    prep = sub.add_parser("rotate-prepare")
    prep.add_argument("--peer-id", required=True)
    prep.add_argument("--apply", action="store_true")
    ack = sub.add_parser("rotate-ack")
    ack.add_argument("--transition", required=True)
    ack.add_argument("--apply", action="store_true")
    fin = sub.add_parser("rotate-finalize")
    fin.add_argument("--peer-id", required=True)
    fin.add_argument("--ack", required=True)
    fin.add_argument("--apply", action="store_true")
    rev = sub.add_parser("revoke-epoch")
    rev.add_argument("--peer-id", required=True)
    rev.add_argument("--epoch", required=True, type=int)
    rev.add_argument("--reason", required=True)
    rev.add_argument("--utc", required=True)
    rev.add_argument("--apply", action="store_true")
    a = ap.parse_args()
    if a.self_test:
        return _self_test()
    state = resolve_state_dir(a.repo, a.state_dir)
    if a.cmd == "init-endpoint":
        out = init_endpoint(state, a.apply)
    elif a.cmd == "generate-peer-key":
        out = generate_peer_key(state, a.peer_id, a.apply)
    elif a.cmd == "enroll-stage":
        out = stage_remote_peer(state, a.peer_id, a.public_key, a.expected_fingerprint, a.apply)
    elif a.cmd == "enroll-activate":
        out = activate_peer(state, a.peer_id, a.approved_fingerprint, a.apply)
    elif a.cmd == "rotate-prepare":
        out = rotate_prepare(state, a.peer_id, a.apply)
    elif a.cmd == "rotate-ack":
        with open(a.transition, encoding="utf-8") as f:
            out = rotate_ack(state, json.load(f), a.apply)
    elif a.cmd == "rotate-finalize":
        with open(a.ack, encoding="utf-8") as f:
            out = rotate_finalize(state, a.peer_id, json.load(f), a.apply)
    elif a.cmd == "revoke-epoch":
        out = revoke_epoch(state, a.peer_id, a.epoch, a.reason, a.utc, a.apply)
    else:
        ap.error("a command is required")
    print(json.dumps(out, ensure_ascii=False, indent=2))
    return 0

if __name__ == "__main__":
    try: raise SystemExit(main())
    except IdentityError as exc:
        print(f"[a2a-identity] FAIL: {exc}", file=sys.stderr); raise SystemExit(6)
