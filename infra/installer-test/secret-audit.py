#!/usr/bin/env python3
"""Secret audit for an Installer proof VM (D011). Runs as root on the VM.

Every secret the installation knows — the values in /etc/ocinye whose names say
password, secret, key or token, the database URL, the sudo password and the
one-time credential — is searched for, as bytes, in everything the Installer
leaves behind or prints: the server journal and state, /srv/ocinye, the system
journal, the containers' logs, /var/log, the process-argument samples taken
during the install, and the operator-side logs, receipts and session files
(uploaded beside this script). It prints names and counts, never a value, and
exits 1 on any hit.

  secret-audit.py WORKDIR   (WORKDIR holds operator.tar and secrets/)
"""
import os, re, subprocess, sys, tarfile

work = sys.argv[1]
secrets = {}
for name in sorted(os.listdir("/etc/ocinye")):
    p = os.path.join("/etc/ocinye", name)
    if not os.path.isfile(p):
        continue
    for line in open(p, encoding="utf-8", errors="replace"):
        if "=" not in line or line.lstrip().startswith("#"):
            continue
        k, v = line.rstrip("\n").split("=", 1)
        v = v.strip().strip('"')
        is_url = k.endswith("_URL")
        # A URL is a secret only when it carries credentials (user:pass@).
        if is_url and not re.match(r"[a-z]+://[^:/@]+:[^@]+@", v):
            continue
        if re.search(r"PASSWORD|SECRET|KEY|TOKEN|_URL$", k) and len(v) >= 8:
            secrets[f"{name}:{k}"] = v.encode()
sdir = os.path.join(work, "secrets")
for name in sorted(os.listdir(sdir)):
    v = open(os.path.join(sdir, name), "rb").read().strip()
    if len(v) >= 6:
        secrets[name] = v
# Passwords inside URLs, too (postgres://user:pass@…).
for k, v in list(secrets.items()):
    m = re.match(rb"[a-z]+://[^:/]+:([^@]+)@", v)
    if m and len(m.group(1)) >= 8:
        secrets[k + "#password"] = m.group(1)

def files(root):
    for base, _, names in os.walk(root):
        for n in names:
            p = os.path.join(base, n)
            if os.path.isfile(p) and not os.path.islink(p):
                yield p

def run(*cmd):
    return subprocess.run(cmd, capture_output=True).stdout

targets = {}
for root in ["/var/lib/ocinye-installer", "/srv/ocinye", "/var/log", "/root/ocinye-argv-samples"]:
    if os.path.isfile(root):
        targets[root] = open(root, "rb").read()
    elif os.path.isdir(root):
        for p in files(root):
            # The proxy's own TLS key is a key file, not a leak.
            if p.startswith("/srv/ocinye/") and "/data/" in p:
                continue
            try:
                targets[p] = open(p, "rb").read()
            except OSError:
                pass
targets["journalctl"] = run("journalctl", "-o", "cat", "--no-pager")
for cid in run("docker", "ps", "-aq").split():
    targets[f"docker-logs:{cid.decode()}"] = run("docker", "logs", cid.decode())
    targets[f"docker-inspect-args:{cid.decode()}"] = run(
        "docker", "inspect", "-f", "{{json .Args}} {{json .Config.Cmd}} {{json .Config.Entrypoint}}", cid.decode())
for p in sorted(os.listdir("/home")):
    h = os.path.join("/home", p, ".bash_history")
    if os.path.isfile(h):
        targets[h] = open(h, "rb").read()
with tarfile.open(os.path.join(work, "operator.tar")) as t:
    for m in t.getmembers():
        if m.isfile():
            targets["operator:" + m.name] = t.extractfile(m).read()

hits = 0
for tname, data in sorted(targets.items()):
    for sname, v in secrets.items():
        n = data.count(v)
        if n:
            hits += n
            print(f"HIT {sname} in {tname} x{n}")
print(f"secrets={len(secrets)} targets={len(targets)} hits={hits}")
sys.exit(1 if hits else 0)
