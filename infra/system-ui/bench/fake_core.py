#!/usr/bin/env python3
"""A stand-in for the Core, for the graphics bench only (ADR-0031).

Speaks the four endpoints the Session Service uses — sign-in, the signed
machine-session policy, its public key, sign-out — with one known account and
a signing seed that is public by being written here. It proves the path
between the screen and the Session Service; it proves nothing about the Core,
whose own endpoint is tested in `services/core-server/tests/machine_session_http.rs`.

  fake_core.py PORT PUBLIC_KEY_FILE [IDLE_LOCK_SECS]

Nothing here ships.
"""
import http.server
import json
import struct
import sys
import time

from cryptography.hazmat.primitives import serialization
from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PrivateKey

ACCOUNT = ("pessoa@ocinye.test", "bancada-de-prova")
KEY = Ed25519PrivateKey.from_private_bytes(bytes([7]) * 32)
IDLE = int(sys.argv[3]) if len(sys.argv) > 3 else 300
SESSIONS = set()


def signed_bytes(p):
    """`SessionPolicy::signed_bytes`, field for field."""
    out = b"ocinye/session-policy/v1\0"
    for name in ("member_id", "member_name", "session_id", "machine_id"):
        raw = p[name].encode()
        out += struct.pack(">Q", len(raw)) + raw
    out += struct.pack(">Q", p["revocation_epoch"]) + bytes([1 if p["offline_unlock"] else 0])
    out += b"\0" if p["max_offline_secs"] is None else b"\1" + struct.pack(">Q", p["max_offline_secs"])
    out += struct.pack(">I", p["max_unlock_attempts"]) + struct.pack(">I", p["idle_lock_secs"])
    return out + struct.pack(">q", p["issued_at"])


class Core(http.server.BaseHTTPRequestHandler):
    def answer(self, status, body=None):
        raw = b"" if body is None else json.dumps(body).encode()
        self.send_response(status)
        self.send_header("content-type", "application/json")
        self.send_header("content-length", str(len(raw)))
        self.end_headers()
        self.wfile.write(raw)

    def token(self):
        for part in self.headers.get("cookie", "").split(";"):
            name, _, value = part.strip().partition("=")
            if name == "ocinye_core_session":
                return value
        return None

    def do_GET(self):
        if self.path == "/api/v1/auth/machine-session/key":
            public = KEY.public_key().public_bytes(serialization.Encoding.Raw, serialization.PublicFormat.Raw)
            return self.answer(200, {"public_key": public.hex()})
        self.answer(404, {})

    def do_POST(self):
        body = json.loads(self.rfile.read(int(self.headers.get("content-length", "0"))) or b"{}")
        if self.path == "/api/v1/auth/login":
            if (body.get("email"), body.get("password")) != ACCOUNT:
                return self.answer(401, {})
            token = "t%d" % time.time_ns()
            SESSIONS.add(token)
            return self.answer(200, {"session_token": token, "state": "active", "display_name": "Pessoa de Prova", "must_change_password": False})
        if self.path == "/api/v1/auth/machine-session":
            token = self.token()
            if token not in SESSIONS:
                return self.answer(401, {})
            policy = {"member_id": "m-bancada", "member_name": "Pessoa de Prova", "session_id": token,
                      "machine_id": body["machine_id"], "revocation_epoch": 0, "offline_unlock": True,
                      "max_offline_secs": 3600, "max_unlock_attempts": 5, "idle_lock_secs": IDLE,
                      "issued_at": int(time.time())}
            return self.answer(200, {"policy": policy, "signature": KEY.sign(signed_bytes(policy)).hex()})
        if self.path == "/api/v1/auth/logout":
            SESSIONS.discard(self.token())
            return self.answer(204)
        self.answer(404, {})

    def log_message(self, *a):
        pass


public = KEY.public_key().public_bytes(serialization.Encoding.Raw, serialization.PublicFormat.Raw)
with open(sys.argv[2], "w") as f:
    f.write(public.hex() + "\n")
http.server.ThreadingHTTPServer(("127.0.0.1", int(sys.argv[1])), Core).serve_forever()
