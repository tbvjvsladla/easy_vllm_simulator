#!/usr/bin/env python3
import importlib.util
import json
import os
import secrets
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
PATH = ROOT / ".claude/policies/runtime/a2a_identity.py"
spec = importlib.util.spec_from_file_location("a2a_identity", PATH)
identity = importlib.util.module_from_spec(spec)
import sys
sys.modules["a2a_identity"] = identity
spec.loader.exec_module(identity)

class IdentityContractTest(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(); self.addCleanup(self.temp.cleanup)
        self.old = os.environ.get(identity.TEST_SENTINEL)
        os.environ[identity.TEST_SENTINEL] = "1"
        self.addCleanup(self._restore)
        root = Path(self.temp.name); self.a=root/"a"; self.b=root/"b"
        identity.init_endpoint(self.a, True); identity.init_endpoint(self.b, True)
        self.aid=identity._read_json(self.a/"endpoint.json")["peer_id"]
        self.bid=identity._read_json(self.b/"endpoint.json")["peer_id"]
        ap=identity.generate_peer_key(self.a,self.bid,True)["public"]
        bp=identity.generate_peer_key(self.b,self.aid,True)["public"]
        identity.stage_remote_peer(self.a,self.bid,bp["public_key"],bp["fingerprint"],True)
        identity.stage_remote_peer(self.b,self.aid,ap["public_key"],ap["fingerprint"],True)
        identity.activate_peer(self.a,self.bid,bp["fingerprint"],True)
        identity.activate_peer(self.b,self.aid,ap["fingerprint"],True)
    def _restore(self):
        if self.old is None: os.environ.pop(identity.TEST_SENTINEL,None)
        else: os.environ[identity.TEST_SENTINEL]=self.old
    def request(self, payload=None):
        payload=payload or {"instruction_id":"i", "authorization_ref":"fixture"}
        return identity.sign_envelope(self.a,self.bid,{"envelope_type":"request",
            "attempt_id":secrets.token_hex(16),"payload":payload,"payload_digest":identity.digest(payload)})
    def test_proof_has_no_authority_and_replay_is_rejected(self):
        req=self.request(); proof=identity.verify_envelope(self.b,req,"request")
        self.assertFalse(hasattr(proof,"allowed"))
        identity.consume_attempt(self.b,proof,req["payload"],"2026-09-17T00:00:00Z")
        with self.assertRaises(identity.IdentityError):
            identity.consume_attempt(self.b,proof,req["payload"],"2026-09-17T00:00:01Z")
    def test_tamper_wrong_recipient_and_float_rejected(self):
        req=self.request(); bad=json.loads(json.dumps(req)); bad["payload"]["instruction_id"]="x"
        with self.assertRaises(identity.IdentityError): identity.verify_envelope(self.b,bad,"request")
        bad=json.loads(json.dumps(req)); bad["recipient_peer_id"]="0"*32
        with self.assertRaises(identity.IdentityError): identity.verify_envelope(self.b,bad,"request")
        with self.assertRaises(identity.IdentityError): identity.digest({"x":1.2})
    def test_predecessor_binding(self):
        first=self.request(); p1=identity.verify_envelope(self.b,first,"request")
        identity.consume_attempt(self.b,p1,first["payload"],"2026-09-17T00:00:00Z")
        payload={"instruction_id":"i","predecessor_attempt_id":p1.attempt_id,
                 "predecessor_request_digest":p1.envelope_digest}
        second=self.request(payload); p2=identity.verify_envelope(self.b,second,"request")
        identity.consume_attempt(self.b,p2,payload,"2026-09-17T00:00:01Z")
    def test_report_replay_and_wrong_peer_are_rejected(self):
        req = self.request()
        request_proof = identity.verify_envelope(self.b, req, "request")
        identity.consume_attempt(self.b, request_proof, req["payload"], "2026-09-17T00:00:00Z")
        report_payload = {"request_envelope_digest": request_proof.envelope_digest,
                          "request_attempt_id": request_proof.attempt_id}
        report = identity.sign_envelope(self.b, self.aid, {
            "envelope_type": "report", "attempt_id": request_proof.attempt_id,
            "payload": report_payload, "payload_digest": identity.digest(report_payload)})
        report_proof = identity.verify_envelope(self.a, report, "report")
        identity.consume_attempt(self.a, report_proof, {}, "2026-09-17T00:00:01Z", message_kind="report")
        with self.assertRaises(identity.IdentityError):
            identity.consume_attempt(self.a, report_proof, {}, "2026-09-17T00:00:02Z", message_kind="report")
        with self.assertRaises(identity.IdentityError):
            identity.generate_peer_key(self.a, "../outside", True)

    def test_authorization_binding(self):
        executor_path = ROOT / ".claude/policies/runtime/a2a_executor.py"
        executor_spec = importlib.util.spec_from_file_location("a2a_executor", executor_path)
        executor = importlib.util.module_from_spec(executor_spec)
        sys.modules["a2a_executor"] = executor
        executor_spec.loader.exec_module(executor)
        request = {"task": "bounded task", "capabilities": ["read"],
                   "target": {"peer_id": self.bid}, "campaign_id": None,
                   "control_variables": None}
        auth = {"status": "authorized", "work_manifest_digest": "a" * 64,
                "instruction_id": "i", "capabilities": ["read"],
                "scope": {"target_peer_id": self.bid,
                          "task_digest": identity.digest("bounded task")},
                "gate": {"action": "a2a_delegate", "authorization_state": "execution-approved"},
                "campaign": {"campaign_id": None, "control_variables": None}}
        self.assertEqual(executor.authorize({"instruction_id": "i", "agent_request": request,
                                             "authorization": auth})["status"], "AUTHORIZED")
        bad = json.loads(json.dumps(auth)); bad["scope"]["task_digest"] = "0" * 64
        with self.assertRaises(executor.AuthorizationError):
            executor.authorize({"instruction_id": "i", "agent_request": request, "authorization": bad})

    def test_test_override_requires_sentinel(self):
        os.environ.pop(identity.TEST_SENTINEL,None)
        with self.assertRaises(identity.IdentityError): identity.resolve_state_dir(".", self.temp.name)

if __name__ == "__main__": unittest.main()
