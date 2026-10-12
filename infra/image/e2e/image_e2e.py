#!/usr/bin/env python3
"""Ocinye OS image certification harness (D013, phase A).

Runs inside the builder VM (scripts/image-e2e.sh starts it there). Boots the
produced artifacts in QEMU and drives them the way an operator would: the
machine's own serial console (the pairing code is read off the screen after
pressing P, never from a file) and the SSH claim channel. Every scenario
prints one RESULT line: PASS, FAIL or INVALID (harness/infrastructure).

  image_e2e.py virt  --qcow2 F --arch A --work D [--keys D]
  image_e2e.py raw   --raw-zst F --raw-sha256 H --arch A --work D
  image_e2e.py iso   --iso F --arch A --work D
  image_e2e.py seat  --qcow2 F --arch A --work D [--memory MiB]
  image_e2e.py window --iso F --arch A --work D

Stdlib only. Test keys are generated per run under --work; no secret is
printed (codes are redacted in the log, compared in memory).
"""
import argparse
import hashlib
import json
import os
import re
import shutil
import socket
import subprocess
import sys
import threading
import time

CODE_RE = re.compile(r"\b([0-9A-HJKMNP-TV-Z]{5}(?:-[0-9A-HJKMNP-TV-Z]{5}){4})\b")
PORTS = iter(range(2300, 2400))


def log(msg):
    print(f"[e2e {time.strftime('%H:%M:%S')}] {msg}", flush=True)


def redact(text):
    return CODE_RE.sub("<code>", text)


def sh(*args, check=True, capture=True, input_=None, timeout=None):
    r = subprocess.run(args, check=False, capture_output=capture, input=input_, timeout=timeout)
    if check and r.returncode != 0:
        raise RuntimeError(f"{args[0]} failed ({r.returncode}): {r.stderr.decode(errors='replace')[-400:] if capture else ''}")
    return r


def sha256(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def native(arch):
    m = os.uname().machine
    return ((arch == "amd64" and m == "x86_64") or (arch == "arm64" and m == "aarch64")) and os.path.exists("/dev/kvm")


def firmware(arch):
    if arch == "amd64":
        return "/usr/share/OVMF/OVMF_CODE_4M.fd", "/usr/share/OVMF/OVMF_VARS_4M.fd"
    return "/usr/share/AAVMF/AAVMF_CODE.fd", "/usr/share/AAVMF/AAVMF_VARS.fd"


RUNNING = []

# Seconds between two keys the bench presses (see `Vm.press`).
KEY_GAP_NATIVE = 0.04
KEY_GAP_EMULATED = 0.3


class Vm:
    """One QEMU guest with its serial console on a unix socket."""

    def __init__(self, name, arch, work, memory=2048, cpus=2):
        self.name, self.arch, self.work = name, arch, work
        self.memory, self.cpus = memory, cpus
        self.vars = os.path.join(work, f"{name}-vars.fd")
        if not os.path.exists(self.vars):
            shutil.copy(firmware(arch)[1], self.vars)
        self.sock = os.path.join(work, f"{name}.serial")
        self.transcript = open(os.path.join(work, f"{name}-serial.log"), "a", encoding="utf-8")
        self.proc = None
        self.conn = None
        self.buf = ""
        self.port = None

    def start(self, disks, cdrom=None, net=True, seed=None, screen=False):
        kvm = native(self.arch)
        code, _ = firmware(self.arch)
        if self.arch == "amd64":
            cmd = ["qemu-system-x86_64", "-machine", "q35", "-cpu", "host" if kvm else "max"]
        else:
            cmd = ["qemu-system-aarch64", "-machine", "virt", "-cpu", "host" if kvm else "max"]
        cmd += ["-accel", "kvm" if kvm else "tcg,thread=multi", "-m", str(self.memory), "-smp", str(self.cpus),
                "-drive", f"if=pflash,format=raw,unit=0,readonly=on,file={code}",
                "-drive", f"if=pflash,format=raw,unit=1,file={self.vars}",
                "-device", "virtio-rng-pci", "-display", "none", "-monitor", "none",
                "-serial", f"unix:{self.sock},server=on,wait=off",
                "-device", "virtio-scsi-pci,id=scsi0"]
        for i, d in enumerate(disks):
            fmt = d.get("format", "qcow2")
            serial = d.get("serial")
            cmd += ["-drive", f"if=none,id=d{i},file={d['file']},format={fmt}"]
            dev = f"virtio-blk-pci,drive=d{i}" + (f",serial={serial}" if serial else "")
            if d.get("boot"):
                dev += ",bootindex=2"
            cmd += ["-device", dev]
        if cdrom:
            cmd += ["-drive", f"if=none,id=cd,media=cdrom,readonly=on,file={cdrom}",
                    "-device", "scsi-cd,drive=cd,bus=scsi0.0,bootindex=1"]
        if seed:
            cmd += ["-drive", f"if=none,id=seed,format=raw,readonly=on,file={seed}", "-device", "virtio-blk-pci,drive=seed"]
        if screen:
            # A display, a keyboard and a pointer, and the monitor socket the
            # screen is read and the keys are pressed through.
            self.qmp_sock = os.path.join(self.work, f"{self.name}.qmp")
            cmd += ["-device", "virtio-gpu-pci", "-device", "qemu-xhci", "-device", "usb-kbd", "-device", "usb-tablet",
                    "-qmp", f"unix:{self.qmp_sock},server=on,wait=off"]
        if net:
            self.port = next(PORTS)
            cmd += ["-netdev", f"user,id=n0,hostfwd=tcp:127.0.0.1:{self.port}-:22", "-device", "virtio-net-pci,netdev=n0"]
        else:
            cmd += ["-nic", "none"]
        log(f"{self.name}: start ({'kvm' if kvm else 'tcg'}, net={'yes' if net else 'none'})")
        RUNNING.append(self)
        self.proc = subprocess.Popen(cmd, stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=open(os.path.join(self.work, f"{self.name}-qemu.err"), "a"))
        for _ in range(100):
            try:
                self.conn = socket.socket(socket.AF_UNIX)
                self.conn.connect(self.sock)
                self.conn.setblocking(False)
                break
            except OSError:
                time.sleep(0.2)
        else:
            raise RuntimeError("serial socket")
        # Drain the console all the time: a guest whose serial port backs up
        # stalls (sshd's banner included).
        self.lock = threading.Lock()
        threading.Thread(target=self._reader, daemon=True).start()

    def _reader(self):
        while self.proc and self.proc.poll() is None:
            try:
                data = self.conn.recv(65536)
            except BlockingIOError:
                time.sleep(0.05)
                continue
            except OSError:
                return
            if not data:
                return
            text = data.decode("utf-8", errors="replace").replace("\r", "")
            with self.lock:
                self.buf += text
            self.transcript.write(redact(text))
            self.transcript.flush()

    def _pump(self):
        # The reader thread does the reading; this only yields.
        return False

    def expect(self, pattern, timeout):
        """Wait for a regex in the console output since the last expect."""
        rx = re.compile(pattern)
        end = time.time() + timeout
        while time.time() < end:
            with self.lock:
                m = rx.search(self.buf)
                if m:
                    self.buf = self.buf[m.end():]
                    return m
            if self.proc.poll() is not None:
                raise RuntimeError(f"{self.name}: qemu exited")
            if not self._pump():
                time.sleep(0.2)
        raise TimeoutError(f"{self.name}: no {pattern!r} after {timeout}s")

    def send(self, text):
        self.conn.sendall(text.encode())

    def drain(self, seconds=1.0):
        end = time.time() + seconds
        while time.time() < end:
            if not self._pump():
                time.sleep(0.1)

    def qmp(self, *commands):
        """Run monitor commands; raise on a refusal."""
        c = socket.socket(socket.AF_UNIX)
        c.settimeout(30)
        c.connect(self.qmp_sock)
        f = c.makefile("rw")
        f.readline()
        out = []
        for command in ({"execute": "qmp_capabilities"},) + commands:
            f.write(json.dumps(command) + "\n")
            f.flush()
            while True:
                answer = json.loads(f.readline())
                if "return" in answer or "error" in answer:
                    break
            if "error" in answer:
                raise RuntimeError(f"qmp {command['execute']}: {answer['error']}")
            out.append(answer)
        c.close()
        return out

    def screenshot(self, path):
        """The guest's screen, as the display device shows it, into a PNG."""
        if os.path.exists(path):
            os.remove(path)
        self.qmp({"execute": "screendump", "arguments": {"filename": path, "format": "png"}})
        return path

    def press(self, qcode, shift=False):
        k = lambda code, down: {"type": "key", "data": {"down": down, "key": {"type": "qcode", "data": code}}}
        events = [k(qcode, True), k(qcode, False)]
        if shift:
            events = [k("shift", True)] + events + [k("shift", False)]
        self.qmp({"execute": "input-send-event", "arguments": {"events": events}})
        # One key at a time, at a pace the guest keeps up with. Key repeat is
        # the client's (Wayland): a press whose release reaches it more than
        # the compositor's repeat delay later is repeated. Under full
        # emulation a guest that falls behind a burst of keys delivers them
        # that late, and the amd64 set-up form received extra Tabs and spaces
        # (boxes ticked, a name typed into the wrong field).
        time.sleep(KEY_GAP_NATIVE if native(self.arch) else KEY_GAP_EMULATED)

    def type(self, text):
        """Type on the guest's keyboard (US layout). The text is never logged."""
        for ch in text:
            if ch in TYPE_SHIFTED:
                self.press(TYPE_SHIFTED[ch], shift=True)
            elif ch.isalpha() and ch.isupper():
                self.press(ch.lower(), shift=True)
            elif ch in TYPE_PLAIN:
                self.press(TYPE_PLAIN[ch])
            elif ch.isalnum() and ch.isascii():
                self.press(ch)
            else:
                raise RuntimeError("a character the bench keyboard cannot type")

    def stop(self):
        if self.proc and self.proc.poll() is None:
            self.proc.terminate()
            try:
                self.proc.wait(30)
            except subprocess.TimeoutExpired:
                self.proc.kill()
        if self.conn:
            self.conn.close()

    def wait_exit(self, timeout):
        self.proc.wait(timeout)


TYPE_PLAIN = {" ": "spc", "-": "minus", ".": "dot", ",": "comma", "/": "slash", "=": "equal", ";": "semicolon",
              "'": "apostrophe", "[": "bracket_left", "]": "bracket_right", "\\": "backslash", "`": "grave_accent"}
TYPE_SHIFTED = {"!": "1", "@": "2", "#": "3", "$": "4", "%": "5", "^": "6", "&": "7", "*": "8", "(": "9", ")": "0",
                "_": "minus", "+": "equal", ":": "semicolon", "?": "slash", "<": "comma", ">": "dot", '"': "apostrophe",
                "{": "bracket_left", "}": "bracket_right", "|": "backslash", "~": "grave_accent"}


def keygen(path):
    if not os.path.exists(path):
        sh("ssh-keygen", "-q", "-t", "ed25519", "-N", "", "-C", "ocinye-e2e", "-f", path)
    return path


LAST_CONNECT = {}
# The image's own firewall allows at most 6 connections per 30 s per source
# on 22/tcp while it is unclaimed (`ufw limit`): stay under it.
MIN_GAP = 6.5

# Confirming a claim enables the runtime at CLAIMED — `systemctl enable --now`
# of containerd/docker/… runs synchronously inside the confirm command before
# it returns (firstboot `on_claimed`). On an emulated amd64 guest (TCG on an
# arm64 host) bringing docker up legitimately takes minutes, far past the
# default 120 s SSH budget, so the step that triggers it waits longer. This is
# test tolerance for emulation speed; the machine reaches CLAIMED either way.
CLAIM_CONFIRM_TIMEOUT = 600


def throttle(port):
    gap = time.time() - LAST_CONNECT.get(port, 0)
    if gap < MIN_GAP:
        time.sleep(MIN_GAP - gap)
    LAST_CONNECT[port] = time.time()


def ssh(port, user, key, remote=None, input_=None, timeout=120):
    throttle(port)
    args = ["ssh", "-i", key, "-p", str(port), "-o", "StrictHostKeyChecking=no", "-o", "UserKnownHostsFile=/dev/null",
            "-o", "LogLevel=ERROR", "-o", "ConnectTimeout=10", "-o", "BatchMode=yes", f"{user}@127.0.0.1"]
    if remote:
        args.append(remote)
    return subprocess.run(args, input=input_, capture_output=True, timeout=timeout)


def claim_session(port, key, messages, timeout=120):
    """Send JSON lines to the claim channel; return parsed events (or the ssh error)."""
    payload = "".join(json.dumps(m) + "\n" for m in messages).encode()
    r = ssh(port, "ocinye-claim", key, None, payload, timeout)
    events = [json.loads(l) for l in r.stdout.decode().splitlines() if l.strip().startswith("{")]
    return r.returncode, events, r.stderr.decode(errors="replace")


def wait_ssh_port(port, timeout):
    end = time.time() + timeout
    while time.time() < end:
        throttle(port)
        try:
            with socket.create_connection(("127.0.0.1", port), 5) as s:
                s.settimeout(5)
                if s.recv(64).startswith(b"SSH-"):
                    return
        except OSError:
            pass
        time.sleep(10)
    raise TimeoutError("ssh port")


def result_status(items, invalid):
    """The single predicate behind every verdict. The failure-evidence gate and
    the reported RESULT must agree, so both read it here (§59)."""
    return "INVALID" if invalid else ("PASS" if items and all(i["ok"] for i in items) else "FAIL")


class Results:
    def __init__(self, scenario):
        self.scenario, self.items = scenario, []

    def check(self, name, ok, detail=""):
        self.items.append({"check": name, "ok": bool(ok), "detail": str(detail)[:300]})
        log(f"{'PASS' if ok else 'FAIL'} {name} {redact(str(detail))[:200]}")
        return ok

    def finish(self, work, invalid=None):
        status = result_status(self.items, invalid)
        out = {"scenario": self.scenario, "status": status, "invalid": invalid, "checks": self.items}
        with open(os.path.join(work, f"result-{self.scenario}.json"), "w") as f:
            json.dump(out, f, indent=1)
        print(f"RESULT {self.scenario} {status}", flush=True)
        return 0 if status == "PASS" else 1


UNCLAIMED_RX = r"N[AÃ]O RECLAMADO"
BOOT_BUDGET = 900

# The OIE prints one of BASE_DONE when the base install succeeds, one of
# BASE_FAIL when it stops with an error — in the medium's language (pt/en/fr,
# services/oie/src/strings.rs `c13.done.t` / `c13.err.t`). The console waits for
# either, and `base_installed` decides which it was. Both are derived from the
# same lists so an English or French success can never be read as failure again.
BASE_DONE = ("Base instalada", "Base installed", "Base installée")
BASE_FAIL_RX = (r"A instala[cç][aã]o parou", r"Installation stopped", r"L.installation s.est arr")
DONE_OR_FAIL_RX = "(" + "|".join([re.escape(s) for s in BASE_DONE] + list(BASE_FAIL_RX)) + ")"


def base_installed(text):
    """True iff the serial shows the base-installed marker in any product
    language. Kept in lockstep with DONE_OR_FAIL_RX (same source list)."""
    return any(marker in text for marker in BASE_DONE)


def curtin_subreason(stderr_lower):
    """A bounded machine token for why a curtin install failed, derived from the
    saved log. Mirrors the OIE's own classifier (services/oie/src/install.rs):
    the missing-package case is tested first, because an offline
    `apt-get install grub-efi-amd64` fails with 'no installation candidate',
    whose text names 'grub'."""
    if any(s in stderr_lower for s in ("no installation candidate", "unable to locate package")):
        return "missing_offline_package"
    if "efibootmgr" in stderr_lower:
        return "efibootmgr"
    if "shim" in stderr_lower:
        return "shim"
    if "grub" in stderr_lower:
        return "grub_install"
    return "other"


ROOT_RE = re.compile(r"\broot=(PARTUUID|UUID)=(\S+)", re.IGNORECASE)


def grub_root_refs(grub_cfg_text):
    """Every `root=PARTUUID=…` / `root=UUID=…` on a `linux` line of a grub.cfg,
    as lowercase `(kind, value)` pairs."""
    refs = []
    for line in grub_cfg_text.splitlines():
        s = line.strip()
        if s.startswith("linux") and "root=" in s:
            m = ROOT_RE.search(s)
            if m:
                refs.append((m.group(1).upper(), m.group(2).lower()))
    return refs


def evaluate_root_identity(refs, forced, target_partuuid, target_fsuuid):
    """Decide whether the installed boot config resolves root to the actual
    target. Pure: the I/O lives in `installed_root_identity`. Returns
    `(ok, detail)`; a mismatch leads with a typed token so the harness can fail
    in seconds instead of waiting out a 900 s second-boot timeout."""
    pu = (target_partuuid or "").lower()
    fu = (target_fsuuid or "").lower()
    if forced:
        return False, f"INSTALLED_ROOT_IDENTITY_MISMATCH: GRUB_FORCE_PARTUUID in {','.join(sorted(forced))}"
    if not refs:
        return False, "INSTALLED_ROOT_IDENTITY_UNKNOWN: no root= on any linux line"

    def resolves(kind, val):
        return (kind == "PARTUUID" and val == pu and pu) or (kind == "UUID" and val == fu and fu)

    shown = ",".join(f"{k}={v}" for k, v in refs)
    if all(resolves(k, v) for k, v in refs):
        return True, f"root={shown} == target(PARTUUID={pu},UUID={fu})"
    return False, f"INSTALLED_ROOT_IDENTITY_MISMATCH: grub root={shown} != target(PARTUUID={pu},UUID={fu})"


def read_code(vm):
    """Press P on the console and read the code off the screen; hide it again."""
    vm.drain(2)
    vm.buf = ""
    vm.send("p")
    m = vm.expect(CODE_RE.pattern, 60)
    vm.send("p")
    return m.group(1)


def claim_by_code(r, vm, key, other_key, label):
    """The full pairing-code claim with its negative paths. Returns claim facts."""
    rc, ev, err = claim_session(vm.port, key, [{"cmd": "hello", "protocol": 1}])
    welcome = ev[0]["challenge"] if ev and ev[0].get("event") == "Welcome" else None
    r.check(f"{label}: welcome", welcome and welcome["state"]["state"] == "UNCLAIMED", ev or err)
    if not welcome:
        return None
    r.check(f"{label}: two host keys, ed25519 first", len(welcome["host_keys"]) == 2 and welcome["host_keys"][0]["algorithm"] == "ssh-ed25519")
    rc, ev, err = claim_session(vm.port, key, [{"cmd": "hello", "protocol": 1}, {"cmd": "enroll", "code": "00000-00000-00000-00000-00000"}])
    r.check(f"{label}: wrong code refused", len(ev) > 1 and ev[1].get("reason", {}).get("code") == "INVALID_CODE", ev)
    rc, ev, err = claim_session(vm.port, key, [{"cmd": "hello", "protocol": 2}])
    r.check(f"{label}: unknown protocol refused", ev and ev[0].get("reason", {}).get("code") == "PROTOCOL_UNSUPPORTED", ev)
    code = read_code(vm)
    rc, ev, err = claim_session(vm.port, key, [{"cmd": "hello", "protocol": 1}, {"cmd": "enroll", "code": code}])
    enrolled = ev[1] if len(ev) > 1 and ev[1].get("event") == "Enrolled" else None
    r.check(f"{label}: enrolled with the console code", enrolled, ev)
    if not enrolled:
        return None
    rc, ev, err = claim_session(vm.port, other_key, [{"cmd": "hello", "protocol": 1}, {"cmd": "enroll", "code": code}])
    r.check(f"{label}: second claimant cannot even authenticate", rc != 0 and not ev, err.strip())
    rc, ev, err = claim_session(vm.port, key, [{"cmd": "hello", "protocol": 1}, {"cmd": "enroll", "code": code}])
    r.check(f"{label}: replay of the consumed code refused", rc != 0 and not ev, err.strip())
    bad = ssh(vm.port, "ocinye", key, "sudo -n /usr/lib/ocinye/ocinye-firstboot confirm --claim " + "0" * 32)
    r.check(f"{label}: wrong claim id refused", b"CLAIM_ID_MISMATCH" in bad.stdout, bad.stdout)
    ok = ssh(vm.port, "ocinye", key, "sudo -n /usr/lib/ocinye/ocinye-firstboot confirm --claim " + enrolled["claim_id"], timeout=CLAIM_CONFIRM_TIMEOUT)
    confirmed = ok.returncode == 0 and b'"Confirmed"' in ok.stdout
    r.check(f"{label}: confirmed by the enrolled key", confirmed, ok.stdout + ok.stderr)
    r.check(f"{label}: confirm prints only the protocol JSON", ok.stdout.strip().count(b"\n") == 0 and ok.stdout.strip().startswith(b"{"), ok.stdout)
    again = ssh(vm.port, "ocinye", key, "sudo -n /usr/lib/ocinye/ocinye-firstboot confirm --claim " + enrolled["claim_id"])
    r.check(f"{label}: confirm replay refused (ALREADY_CLAIMED)", b"ALREADY_CLAIMED" in again.stdout, again.stdout)
    rc, ev, err = claim_session(vm.port, other_key, [{"cmd": "hello", "protocol": 1}])
    r.check(f"{label}: claim account closed after CLAIMED", rc != 0 and not ev, err.strip())
    return {"welcome": welcome, "claim_id_prefix": enrolled["claim_id"][:8]}


def machine_facts(port, key):
    r = ssh(port, "ocinye", key, "cat /etc/machine-id; sudo -n cat /var/lib/ocinye-firstboot/identity.json; echo; systemctl is-enabled docker.service; systemctl is-active docker.service; sudo -n ufw status | head -1")
    lines = r.stdout.decode().splitlines()
    return {"machine_id": lines[0] if lines else "", "identity": json.loads(lines[1]) if len(lines) > 1 else {}, "docker": lines[2:4], "ufw": lines[4:5]}


def measure(port, key):
    r = ssh(port, "ocinye", key, "df -B1 --output=used / | tail -1; free -b | awk '/Mem:/{print $3}'; systemctl list-units --type=service --state=running --no-legend | wc -l; dpkg-query -W | wc -l; swapon --noheadings | wc -l")
    v = r.stdout.decode().split()
    keys = ["disk_used_bytes", "ram_used_bytes", "running_services", "packages", "swap_devices"]
    return dict(zip(keys, v))


def seed_iso(work, name, pubkey):
    d = os.path.join(work, f"{name}-seed")
    os.makedirs(d, exist_ok=True)
    with open(os.path.join(d, "user-data"), "w") as f:
        f.write(f"#cloud-config\nusers:\n  - default\nssh_authorized_keys:\n  - {open(pubkey).read().strip()}\nhostname: ocinye-e2e-b\n")
    with open(os.path.join(d, "meta-data"), "w") as f:
        f.write(f"instance-id: e2e-{name}-{int(time.time())}\n")
    out = os.path.join(work, f"{name}-seed.img")
    sh("cloud-localds", out, os.path.join(d, "user-data"), os.path.join(d, "meta-data"))
    return out


def scenario_virt(a):
    r = Results(f"virt-{a.arch}")
    work = a.work
    k1, k2 = keygen(os.path.join(work, "operator_a")), keygen(os.path.join(work, "operator_x"))
    kp = keygen(os.path.join(work, "operator_b"))
    disks = {}
    for n in ("a", "b"):
        disks[n] = os.path.join(work, f"clone-{n}.qcow2")
        sh("qemu-img", "create", "-q", "-f", "qcow2", "-F", "qcow2", "-b", a.qcow2, disks[n], "30G")
    try:
        a_vm = Vm("clone-a", a.arch, work)
        a_vm.start([{"file": disks["a"], "boot": True}])
        a_vm.expect(UNCLAIMED_RX, BOOT_BUDGET)
        wait_ssh_port(a_vm.port, 300)
        r.check("A: console shows UNCLAIMED without a code on screen", not CODE_RE.search(a_vm.buf))
        boot_log = open(a_vm.transcript.name, encoding="utf-8", errors="replace").read()
        r.check("A: boot has no systemd ordering cycle", "Ordering cycle" not in boot_log)
        facts = claim_by_code(r, a_vm, k1, k2, "A")
        fa = machine_facts(a_vm.port, k1) if facts else {}
        r.check("A: docker enabled and running only after CLAIMED", fa.get("docker") == ["enabled", "active"], fa.get("docker"))
        r.check("A: firewall active", fa.get("ufw") == ["Status: active"], fa.get("ufw"))
        res = measure(a_vm.port, k1)
        r.check("A: no swap", res.get("swap_devices") == "0", res)
        log(f"A resources after claim: {json.dumps(res)}")
        a_vm.stop()
        # B: a clone of the same image, claimed through a platform-provisioned key.
        b_vm = Vm("clone-b", a.arch, work)
        b_vm.start([{"file": disks["b"], "boot": True}], seed=seed_iso(work, "clone-b", kp + ".pub"))
        b_vm.expect(UNCLAIMED_RX, BOOT_BUDGET)
        wait_ssh_port(b_vm.port, 300)
        rc, ev, err = claim_session(b_vm.port, k2, [{"cmd": "hello", "protocol": 1}])
        wb = ev[0]["challenge"] if ev and ev[0].get("event") == "Welcome" else None
        r.check("B: UNCLAIMED after A was claimed", wb and wb["state"]["state"] == "UNCLAIMED", ev or err)
        if facts and wb:
            wa = facts["welcome"]
            r.check("clones: different bootstrap ids", wa["bootstrap_id"] != wb["bootstrap_id"])
            r.check("clones: different host keys", {k["fingerprint"] for k in wa["host_keys"]}.isdisjoint({k["fingerprint"] for k in wb["host_keys"]}))
        fp = sh("ssh-keygen", "-l", "-E", "sha256", "-f", kp + ".pub").stdout.decode().split()[1]
        p = ssh(b_vm.port, "ocinye", kp, f"sudo -n /usr/lib/ocinye/ocinye-firstboot claim --provisioned --key {fp}", timeout=CLAIM_CONFIRM_TIMEOUT)
        r.check("B: claimed with the provisioned key", p.returncode == 0 and b'"Confirmed"' in p.stdout, p.stdout + p.stderr)
        fb = machine_facts(b_vm.port, kp)
        if fa:
            r.check("clones: different machine-id", fa["machine_id"] and fb["machine_id"] and fa["machine_id"] != fb["machine_id"], (fa["machine_id"][:6], fb["machine_id"][:6]))
        b_vm.stop()
        # A again: CLAIMED survives a reboot, and the code is never back.
        a2 = Vm("clone-a", a.arch, work)
        a2.start([{"file": disks["a"], "boot": True}])
        a2.expect(r"RECLAMADO", BOOT_BUDGET)
        wait_ssh_port(a2.port, 300)
        st = ssh(a2.port, "ocinye", k1, "sudo -n /usr/lib/ocinye/ocinye-firstboot status")
        r.check("A: still CLAIMED after reboot", b'"CLAIMED"' in st.stdout, st.stdout)
        a2.stop()
    except Exception as e:  # noqa: BLE001 — the harness reports, never hides
        return r.finish(work, invalid=f"{type(e).__name__}: {e}")
    return r.finish(work)


SESSION_PROBE = r'''
import json, socket, struct
def frame(o):
    b = json.dumps(o).encode(); return struct.pack(">I", len(b)) + b
def read(s):
    n = struct.unpack(">I", s.recv(4, socket.MSG_WAITALL))[0]; return json.loads(s.recv(n, socket.MSG_WAITALL))
s = socket.socket(socket.AF_UNIX); s.settimeout(20); s.connect("/run/ocinye/session.sock")
s.sendall(frame({"frame": "hello", "contract": 1})); read(s)
s.sendall(frame(REQUEST)); print(json.dumps(read(s)))
'''


def session_ask(port, key, request):
    """One request to the Session Service, as root on the machine; its reply."""
    probe = SESSION_PROBE.replace("REQUEST", repr(request))
    r = ssh(port, "ocinye", key, "sudo -n python3 -", probe.encode(), timeout=60)
    try:
        return json.loads(r.stdout.decode().strip().splitlines()[-1])
    except (ValueError, IndexError):
        return {"reply": "none", "detail": (r.stdout + r.stderr).decode(errors="replace")[-300:]}


def totp(secret_base32, at=None):
    """The six digits an authenticator shows for this key now (RFC 6238)."""
    import base64
    import hmac
    import struct
    key = base64.b32decode(secret_base32.upper() + "=" * (-len(secret_base32) % 8))
    counter = int((at if at is not None else time.time()) // 30)
    digest = hmac.new(key, struct.pack(">Q", counter), "sha1").digest()
    offset = digest[-1] & 0x0F
    return f"{(struct.unpack('>I', digest[offset:offset + 4])[0] & 0x7FFFFFFF) % 1000000:06d}"


def session_state(port, key):
    """The local session, as the Session Service tells root on the machine."""
    reply = session_ask(port, key, {"request": "state"})
    return reply.get("state") or {"state": "UNKNOWN", "detail": reply}


def png_is_a_picture(path):
    """Whether a screenshot shows something: more than one colour in it."""
    import zlib
    data = open(path, "rb").read()
    if data[:8] != b"\x89PNG\r\n\x1a\n":
        return False
    raw, pos = b"", 8
    while pos < len(data):
        n = int.from_bytes(data[pos:pos + 4], "big")
        if data[pos + 4:pos + 8] == b"IDAT":
            raw += data[pos + 8:pos + 8 + n]
        pos += 12 + n
    pixels = zlib.decompress(raw)
    return len(set(pixels[1000:200000:97])) > 8


def scenario_seat(a):
    """The seat of an installed machine (ADR-0031), end to end on the image.

    Everything a person does happens at the machine's own screen, with the
    guest's keyboard: the first-start set-up creates the Instance from the
    release the image carries, the first administrator's credential is shown
    once, and the administrator signs in — replacing that credential — locks,
    unlocks and restarts. The machine is also claimed, only so that the bench
    has a way in to look: what it asserts comes from the services and the
    units, never from pixels.
    """
    r = Results(f"seat-{a.arch}")
    work = a.work
    k1, k2 = keygen(os.path.join(work, "operator_a")), keygen(os.path.join(work, "operator_x"))
    disk = os.path.join(work, "seat.qcow2")
    sh("qemu-img", "create", "-q", "-f", "qcow2", "-F", "qcow2", "-b", a.qcow2, disk, "40G")
    shots = os.path.join(work, "screens")
    os.makedirs(shots, exist_ok=True)
    chosen = "a escolha da bancada 2026"
    try:
        vm = Vm("seat", a.arch, work, memory=a.memory, cpus=3)
        vm.start([{"file": disk, "boot": True}], screen=True)
        vm.expect(UNCLAIMED_RX, BOOT_BUDGET)
        wait_ssh_port(vm.port, 300)
        facts = claim_by_code(r, vm, k1, k2, "seat")
        if not facts:
            return r.finish(work)
        run = lambda command, timeout=120: ssh(vm.port, "ocinye", k1, command, timeout=timeout)
        units = run("systemctl is-active ocinye-system.service ocinye-session.service ocinye-display.service ocinye-console@tty2.service; systemctl is-enabled ocinye-seat-link.path; systemctl is-active ocinye-console@tty1.service").stdout.decode().split()
        r.check("seat: the two services run before any Instance exists", units[:2] == ["active", "active"], units)
        r.check("seat: the screen is up on a machine with a display, and the text console is on tty2", units[2:4] == ["active", "active"], units)
        r.check("seat: the link waits for an Instance", units[4:5] == ["enabled"], units)
        r.check("seat: tty1 belongs to the screen", units[5:6] == ["inactive"], units)
        st = session_state(vm.port, k1)
        r.check("seat: nobody is signed in", st.get("state") == "SIGNED_OUT", st)

        def activation():
            out = run("sudo -n ocinye-system --json activation status").stdout.decode(errors="replace")
            try:
                return json.loads(out)["value"]
            except (ValueError, KeyError):
                return {"phase": "UNKNOWN", "detail": out[-200:]}

        ready = activation()
        r.check("seat: the machine offers to create its Instance from the release it carries", ready.get("phase") == "READY" and ready.get("release_id"), ready)
        time.sleep(20)
        shown = png_is_a_picture(vm.screenshot(os.path.join(shots, "1-set-up.png")))
        r.check("seat: the screen shows the first-start set-up", shown)
        if not shown:
            # What the machine says of its own screen: the devices, the
            # drivers, and the display unit's journal. Evidence, not a check.
            why = run("ls -l /dev/dri 2>&1; lsmod | grep -E 'drm|gpu|bochs|virtio' 2>&1; "
                      "sudo -n journalctl -b -u ocinye-display.service -u seatd.service --no-pager -o cat 2>&1 | tail -n 80; "
                      "pgrep -a -f 'ocinye-|WebKit' 2>&1; id ocinye-ui 2>&1; sudo -n dmesg | grep -iE 'segfault|traps:|killed process|oom' | tail -n 10").stdout.decode(errors="replace")
            log("seat: display diagnosis\n" + why)
        # The whole set-up, at the keyboard: the form's fields in order, the
        # four distribution boxes passed over (research is ticked), Enter.
        vm.type("Bancada do posto")
        vm.press("tab")
        vm.type("ocinye-seat.test")
        for _ in range(5):
            vm.press("tab")
        vm.type("Pessoa de Prova")
        vm.press("tab")
        vm.type("pessoa@ocinye-seat.test")
        vm.press("tab")
        vm.type("Admin de Prova")
        vm.press("tab")
        vm.type("admin@ocinye-seat.test")
        # The picture of the filled form is the one taken after the guest has
        # drawn the last field, and Enter follows it.
        time.sleep(1 if native(a.arch) else 15)
        vm.screenshot(os.path.join(shots, "2-set-up-filled.png"))
        vm.press("ret")
        time.sleep(15)
        started = activation()
        r.check("seat: the set-up started the installation", started.get("phase") in ("RUNNING", "ACTIVATED"), started)
        vm.screenshot(os.path.join(shots, "3-creating.png"))
        done = started
        for _ in range(240):
            if done.get("phase") != "RUNNING":
                break
            time.sleep(15)
            done = activation()
        log_tail = run("sudo -n tail -c 1500 /var/lib/ocinye/system/activation/install.log").stdout.decode(errors="replace")
        open(os.path.join(work, "install.log"), "w").write(redact(log_tail))
        r.check("seat: the Instance was created on the machine, by the machine", done.get("phase") == "ACTIVATED" and done.get("credential_waiting") is True, (done, log_tail[-500:]))
        if done.get("phase") != "ACTIVATED":
            return r.finish(work)
        audit = run("sudo -n grep -c activation.start /var/log/ocinye/system-audit.jsonl; sudo -n grep -c 'pessoa@' /var/log/ocinye/system-audit.jsonl").stdout.decode().split()
        r.check("seat: the activation is in the audit record, by its administrator and not by the person's address", audit[:1] == ["1"] and audit[1:2] == ["0"], audit)
        # The link: the key, the address, the name.
        linked = False
        for _ in range(60):
            got = run("test -s /var/lib/ocinye/seat/instance-session.pub && echo linked").stdout.decode().strip()
            if got == "linked":
                linked = True
                break
            time.sleep(10)
        link = run("cat /var/lib/ocinye/seat/workspace-url; grep -c 'ocinye-seat$' /etc/hosts; sudo -n journalctl -u ocinye-seat-link --no-pager -n 20").stdout.decode(errors="replace")
        r.check("seat: linked to the Instance it created", linked, link[-900:])
        r.check("seat: the Workspace address is the Instance's own name", link.startswith("https://ocinye-seat.test/"), link[:80])
        r.check("seat: the name resolves to this machine", link.splitlines()[1:2] == ["1"], link[:120])
        codes = [run(f"curl -s -o /dev/null -w '%{{http_code}}' http://127.0.0.1:8480{path}").stdout.decode().strip()
                 for path in ("/api/v1/auth/machine-session/key", "/api/v1/people", "/api/v1/members", "/")]
        r.check("seat: the loopback door answers for the session key and for nothing else", codes == ["200", "404", "404", "404"], codes)
        listening = run("ss -ltnH 'sport = :8480' | awk '{print $4}'").stdout.decode().split()
        r.check("seat: the door listens on the loopback only", listening == ["127.0.0.1:8480"], listening)
        # The first administrator's credential: on the screen once, then gone.
        # The bench cannot read a screen, so it reads the file the screen is
        # about to be given — as root, over the claim's SSH — and then checks
        # that showing it removed it.
        secret = run("sudo -n cat /var/lib/ocinye/system/activation/credential").stdout.decode().strip()
        r.check("seat: a temporary credential is waiting to be shown", len(secret) >= 12)
        time.sleep(8)
        vm.screenshot(os.path.join(shots, "4-instance-created.png"))
        vm.press("ret")
        time.sleep(4)
        vm.screenshot(os.path.join(shots, "5-credential.png"))
        gone = run("sudo -n test -e /var/lib/ocinye/system/activation/credential && echo there || echo gone").stdout.decode().strip()
        r.check("seat: once shown, the credential is no longer on the machine", gone == "gone" and activation().get("credential_waiting") is False, gone)
        vm.press("ret")
        time.sleep(3)
        first = vm.screenshot(os.path.join(shots, "6-sign-in.png"))
        r.check("seat: the screen shows the sign-in", png_is_a_picture(first))
        # Sign in at the keyboard, with the temporary credential, and replace
        # it. The address is already in its field: the screen put it there.
        vm.press("tab")
        vm.type(secret)
        vm.press("ret")
        time.sleep(12)
        vm.screenshot(os.path.join(shots, "7-first-access.png"))
        st = session_state(vm.port, k1)
        r.check("seat: a temporary credential opens no session", st.get("state") == "SIGNED_OUT", st)
        vm.type(chosen)
        vm.press("tab")
        vm.type(chosen)
        vm.press("ret")
        time.sleep(12)
        vm.screenshot(os.path.join(shots, "7b-second-factor.png"))
        r.check("seat: an administrator without a second factor still has no session", session_state(vm.port, k1).get("state") == "SIGNED_OUT")
        # The screen shows the authenticator key; the bench cannot read a
        # screen. It asks the Session Service to begin the enrolment again —
        # which replaces the key on the Core's side — and plays the
        # authenticator for the key it was given. The code typed at the
        # keyboard is therefore a real one, for the key the Core now holds.
        begun = session_ask(vm.port, k1, {"request": "begin_second_factor_enrollment", "username": "admin@ocinye-seat.test", "password": chosen})
        r.check("seat: the Core hands out an authenticator key for the enrolment", begun.get("reply") == "second_factor_secret", {k: v for k, v in begun.items() if k != "secret_base32"})
        if begun.get("reply") != "second_factor_secret":
            return r.finish(work)
        if time.time() % 30 > 22:
            time.sleep(9)
        vm.type(totp(begun["secret_base32"]))
        vm.press("ret")
        active = {}
        for _ in range(12):
            time.sleep(5)
            active = session_state(vm.port, k1)
            if active.get("state") == "ACTIVE":
                break
        r.check("seat: signed in at the machine's own screen", active.get("state") == "ACTIVE" and active.get("core") == "ONLINE", active)
        r.check("seat: the member is the Instance's administrator", active.get("member_name") == "Admin de Prova", active.get("member_name"))
        r.check("seat: the administrator, with the second factor satisfied, may operate the machine", active.get("operator") is True, active.get("operator"))
        time.sleep(4)
        vm.screenshot(os.path.join(shots, "7c-recovery-codes.png"))
        # `[o]`: the pattern must not match the command line that carries it.
        waiting = run("pgrep -fc '[o]cinye-system-ui --workspace'").stdout.decode().split()
        r.check("seat: the Workspace waits while the recovery codes are on the screen", waiting == ["0"], waiting)
        vm.press("ret")
        time.sleep(25)
        vm.screenshot(os.path.join(shots, "8-session.png"))
        procs = run("pgrep -fc '[o]cinye-system-ui --workspace'; pgrep -u ocinye-ui -fc '[o]cinye-compositor -- '").stdout.decode().split()
        r.check("seat: the Workspace has its own window process", procs[:1] == ["1"], procs)
        r.check("seat: the compositor runs as the unprivileged screen account", procs[1:2] == ["1"], procs)
        # Lock, then unlock with the chosen password — the temporary one is gone.
        lock = session_ask(vm.port, k1, {"request": "lock"})
        r.check("seat: locked", (lock.get("state") or {}).get("state") == "LOCKED", lock)
        time.sleep(6)
        vm.screenshot(os.path.join(shots, "9-locked.png"))
        vm.type(secret)
        vm.press("ret")
        time.sleep(6)
        r.check("seat: the temporary credential does not unlock", session_state(vm.port, k1).get("state") == "LOCKED")
        vm.type(chosen)
        vm.press("ret")
        time.sleep(8)
        r.check("seat: unlocked with the chosen password", session_state(vm.port, k1).get("state") == "ACTIVE")
        vm.screenshot(os.path.join(shots, "10-unlocked.png"))
        # A restart ends the session and brings the screen back by itself.
        run("sudo -n systemctl reboot", timeout=30)
        time.sleep(20)
        wait_ssh_port(vm.port, 600)
        back = {}
        for _ in range(60):
            got = run("systemctl is-active ocinye-display.service").stdout.decode().strip()
            if got == "active":
                back = session_state(vm.port, k1)
                break
            time.sleep(10)
        r.check("seat: after a restart the screen is back and nobody is signed in", back.get("state") == "SIGNED_OUT", back)
        r.check("seat: the previous session is said to have ended with the restart", back.get("ended") == "RESTARTED", back)
        time.sleep(20)
        vm.screenshot(os.path.join(shots, "11-after-restart.png"))
        vm.stop()
    except Exception as e:  # noqa: BLE001 — the harness reports, never hides
        return r.finish(work, invalid=f"{type(e).__name__}: {e}")
    return r.finish(work)


def scenario_raw(a):
    r = Results(f"raw-{a.arch}")
    raw = os.path.join(a.work, "image.raw")
    try:
        sh("zstd", "-q", "-d", "-f", a.raw_zst, "-o", raw)
        r.check("RAW: uncompressed digest matches the manifest", sha256(raw) == a.raw_sha256)
        vm = Vm("raw", a.arch, a.work)
        vm.start([{"file": raw, "format": "raw", "boot": True}])
        vm.expect(UNCLAIMED_RX, BOOT_BUDGET)
        wait_ssh_port(vm.port, 300)
        rc, ev, err = claim_session(vm.port, keygen(os.path.join(a.work, "operator_r")), [{"cmd": "hello", "protocol": 1}])
        r.check("RAW: boots to UNCLAIMED and answers the claim channel", ev and ev[0]["challenge"]["state"]["state"] == "UNCLAIMED", ev or err)
        vm.stop()
    except Exception as e:  # noqa: BLE001
        return r.finish(a.work, invalid=f"{type(e).__name__}: {e}")
    finally:
        if os.path.exists(raw):
            os.remove(raw)
    return r.finish(a.work)


def make_data_disk(path, size):
    """A disk with a GPT, one ext4 partition and a file: must survive the install."""
    sh("qemu-img", "create", "-q", "-f", "raw", path, size)
    sh("parted", "-s", path, "mklabel", "gpt", "mkpart", "data", "ext4", "1MiB", "100%")
    dev = sh("losetup", "--show", "-f", "-P", path).stdout.decode().strip()
    try:
        sh("mkfs.ext4", "-q", "-L", "data", dev + "p1")
    finally:
        sh("losetup", "-d", dev, check=False)


# --- Failure evidence for the ISO install -------------------------------------
# A real BOOTLOADER_INSTALL_FAILED can only be diagnosed from the disk curtin
# wrote and the logs it saved on the target root (`save_install_log`,
# `save_install_config`). The live installer VM's own /run/ocinye-oie/curtin.out
# lives on tmpfs and dies with the VM, so the recoverable copies are on the
# target. On FAIL/INVALID we keep the disk and read it OFFLINE — never booted,
# always mounted read-only, qemu-nbd always disconnected. This runs inside the
# disposable builder VM as root; nothing here runs on an operator's machine.


def _run_cmd(args, check=True):
    return subprocess.run(args, capture_output=True, check=check)


def _free_nbd():
    for i in range(16):
        try:
            if open(f"/sys/block/nbd{i}/size").read().strip() == "0":
                return f"/dev/nbd{i}"
        except OSError:
            continue
    return None


class NbdAttach:
    """A disk image attached READ-ONLY on an nbd device; detached on exit even
    when the body raises. Every shell-out goes through `run` so the read-only
    flag and the guaranteed disconnect are testable without root."""

    def __init__(self, disk, run=None, pick_dev=None):
        self.disk, self.run, self.pick_dev = disk, run or _run_cmd, pick_dev or _free_nbd
        self.dev = None

    def __enter__(self):
        self.run(["modprobe", "nbd", "max_part=16"], check=False)
        self.dev = self.pick_dev()
        if not self.dev:
            raise RuntimeError("no free nbd device")
        # -r is mandatory: the target is evidence and must not be written to.
        self.run(["qemu-nbd", "--fork", "-r", "-c", self.dev, self.disk])
        self.run(["partprobe", self.dev], check=False)
        # A freshly attached nbd device needs udev to settle before its
        # partition nodes and blkid data exist; querying too soon returns empty
        # (seen as a false identity mismatch) or an unmountable partition.
        self.run(["udevadm", "settle"], check=False)
        if self.run is _run_cmd:
            for _ in range(40):
                if os.path.exists(self.dev + "p2"):
                    break
                time.sleep(0.25)
        return self

    def __exit__(self, *exc):
        if self.dev:
            self.run(["qemu-nbd", "-d", self.dev], check=False)
            self.dev = None
        return False


class RoMount:
    """A filesystem mounted read-only; unmounted on exit even when the body
    raises. Read-only is not negotiable here."""

    def __init__(self, source, mnt, run=None):
        self.source, self.mnt, self.run = source, mnt, run or _run_cmd

    def __enter__(self):
        os.makedirs(self.mnt, exist_ok=True)
        self.run(["mount", "-o", "ro", self.source, self.mnt])
        return self

    def __exit__(self, *exc):
        self.run(["umount", self.mnt], check=False)
        return False


def _capture(run, args, dest, collected):
    try:
        r = run(args, check=False)
        with open(dest, "wb") as f:
            f.write(r.stdout or b"")
            if getattr(r, "stderr", None):
                f.write(b"\n--- stderr ---\n")
                f.write(r.stderr)
        collected.append(os.path.basename(dest))
    except Exception as e:  # noqa: BLE001 — a missing tool is a gap, not a crash
        log(f"diag: {args[0]} failed: {e}")


def _copy_if(src, dest, collected):
    if os.path.isfile(src):
        shutil.copy2(src, dest)
        collected.append(os.path.basename(dest))


def _copy_tree_if(src, dest, collected):
    if os.path.isdir(src):
        shutil.copytree(src, dest, dirs_exist_ok=True)
        collected.append(os.path.basename(dest) + "/")


def _listing(root, dest, collected):
    """A plain recursive listing of a tree (no bytes copied), read-only."""
    if not os.path.isdir(root):
        return
    lines = []
    for dirpath, dirs, files in os.walk(root):
        dirs.sort()
        rel = os.path.relpath(dirpath, root)
        for name in sorted(files):
            p = os.path.join(dirpath, name)
            try:
                size = os.path.getsize(p)
            except OSError:
                size = -1
            lines.append(f"{size:>12}  {os.path.join(rel, name) if rel != '.' else name}")
    with open(dest, "w") as f:
        f.write("\n".join(lines) + ("\n" if lines else ""))
    collected.append(os.path.basename(dest))


def _grub_shim_packages(status_path, dest, collected):
    """Which grub/shim/efibootmgr packages the target root has, read offline
    from dpkg's status file (no chroot, no execution)."""
    if not os.path.isfile(status_path):
        return
    out = []
    pkg = ver = None
    for line in open(status_path, encoding="utf-8", errors="replace"):
        if line.startswith("Package: "):
            pkg = line[9:].strip()
        elif line.startswith("Version: "):
            ver = line[9:].strip()
        elif line.strip() == "" and pkg:
            if any(t in pkg for t in ("grub", "shim", "efibootmgr", "efivar")):
                out.append(f"{pkg} {ver or '?'}")
            pkg = ver = None
    with open(dest, "w") as f:
        f.write("\n".join(sorted(out)) + ("\n" if out else ""))
    collected.append(os.path.basename(dest))


def collect_target_diagnostics(disk, diag, run=None, pick_dev=None):
    """Attach the preserved disk read-only and copy the installation's own
    evidence into `diag`. The OIE lays the target out with the ESP as GPT
    partition 1 and the root as partition 2 (`curtin_config`)."""
    run = run or _run_cmd
    collected = []
    with NbdAttach(disk, run=run, pick_dev=pick_dev) as nbd:
        dev = nbd.dev
        _capture(run, ["sfdisk", "-d", dev], os.path.join(diag, "partition-table.sfdisk"), collected)
        _capture(run, ["lsblk", "-J", "-o", "NAME,LABEL,FSTYPE,SIZE,PARTTYPE,PARTTYPENAME,PARTFLAGS,RO", dev],
                 os.path.join(diag, "lsblk.json"), collected)
        esp, root = f"{dev}p1", f"{dev}p2"
        # Target root: curtin's saved log and config, installer/ocinye logs,
        # fstab, the boot trees, and the bootloader packages that are present.
        try:
            rootmnt = os.path.join(diag, "_root")
            with RoMount(root, rootmnt, run=run):
                _copy_if(os.path.join(rootmnt, "root/curtin-install.log"),
                         os.path.join(diag, "curtin-install.log"), collected)
                saved_log = os.path.join(diag, "curtin-install.log")
                if os.path.isfile(saved_log):
                    low = open(saved_log, encoding="utf-8", errors="replace").read().lower()
                    with open(os.path.join(diag, "curtin-failure-subreason.txt"), "w") as f:
                        f.write(curtin_subreason(low) + "\n")
                    collected.append("curtin-failure-subreason.txt")
                _copy_if(os.path.join(rootmnt, "root/curtin-install-cfg.yaml"),
                         os.path.join(diag, "curtin-install-cfg.yaml"), collected)
                _copy_if(os.path.join(rootmnt, "etc/fstab"), os.path.join(diag, "fstab"), collected)
                # The generated boot config and the grub defaults that shape its
                # root= identity (the force-partuuid class of defect lives here).
                _copy_if(os.path.join(rootmnt, "boot/grub/grub.cfg"),
                         os.path.join(diag, "grub.cfg"), collected)
                _copy_if(os.path.join(rootmnt, "etc/default/grub"),
                         os.path.join(diag, "default-grub"), collected)
                _copy_tree_if(os.path.join(rootmnt, "etc/default/grub.d"),
                              os.path.join(diag, "default-grub.d"), collected)
                _copy_tree_if(os.path.join(rootmnt, "var/log/curtin"), os.path.join(diag, "var-log-curtin"), collected)
                _copy_tree_if(os.path.join(rootmnt, "var/log/installer"), os.path.join(diag, "var-log-installer"), collected)
                _copy_tree_if(os.path.join(rootmnt, "var/log/ocinye"), os.path.join(diag, "var-log-ocinye"), collected)
                _listing(os.path.join(rootmnt, "boot"), os.path.join(diag, "boot-tree.txt"), collected)
                _listing(os.path.join(rootmnt, "boot/efi"), os.path.join(diag, "boot-efi-tree.txt"), collected)
                _grub_shim_packages(os.path.join(rootmnt, "var/lib/dpkg/status"),
                                    os.path.join(diag, "grub-shim-packages.txt"), collected)
        except Exception as e:  # noqa: BLE001
            _capture(run, ["true"], os.path.join(diag, "root-mount-error.txt"), [])
            with open(os.path.join(diag, "root-mount-error.txt"), "w") as f:
                f.write(f"{type(e).__name__}: {e}\n")
            log(f"diag: target root not mountable: {e}")
        # ESP: whether it mounts at all is itself a diagnostic, then its EFI tree
        # (EFI/ubuntu/shimx64.efi, EFI/ubuntu/grubx64.efi, EFI/BOOT/BOOTX64.EFI).
        espmnt = os.path.join(diag, "_esp")
        try:
            with RoMount(esp, espmnt, run=run):
                _listing(espmnt, os.path.join(diag, "esp-tree.txt"), collected)
                with open(os.path.join(diag, "esp-mountable.txt"), "w") as f:
                    f.write("yes\n")
        except Exception as e:  # noqa: BLE001
            with open(os.path.join(diag, "esp-mountable.txt"), "w") as f:
                f.write(f"no: {type(e).__name__}: {e}\n")
            log(f"diag: ESP not mountable: {e}")
        collected.append("esp-mountable.txt")
    return collected


def _blkid_value(run, dev, field, tries=24):
    """A blkid field (lowercased), retried while udev and the blkid cache
    settle after a fresh nbd attach — an unsettled device answers empty, not an
    error, so an empty read is retried rather than taken as a real mismatch.
    With an injected (test) runner there is nothing to settle, so it is read
    once."""
    for i in range(tries):
        v = run(["blkid", "-s", field, "-o", "value", dev], check=False).stdout.decode().strip().lower()
        if v or run is not _run_cmd:
            return v
        run(["udevadm", "settle"], check=False)
        time.sleep(0.25)
    return ""


def installed_root_identity(disk, run=None, pick_dev=None):
    """Attach the freshly installed disk READ-ONLY and prove that its generated
    boot config resolves root to this target's own root partition/filesystem,
    not a build-time identity. Returns a facts dict with `ok`/`detail`. Never
    raises — an attach/mount failure is reported as INVALID, not a crash."""
    run = run or _run_cmd
    facts = {
        "ok": False,
        "detail": "",
        "target_root_partuuid": None,
        "target_root_fsuuid": None,
        "grub_root_refs": [],
        "forced_partuuid_files": [],
    }
    try:
        with NbdAttach(disk, run=run, pick_dev=pick_dev) as nbd:
            p2 = f"{nbd.dev}p2"
            facts["target_root_partuuid"] = _blkid_value(run, p2, "PARTUUID")
            facts["target_root_fsuuid"] = _blkid_value(run, p2, "UUID")
            mnt = disk + ".idmnt"
            with RoMount(p2, mnt, run=run):
                cfg = os.path.join(mnt, "boot/grub/grub.cfg")
                refs = grub_root_refs(
                    open(cfg, encoding="utf-8", errors="replace").read()
                ) if os.path.isfile(cfg) else []
                gd = os.path.join(mnt, "etc/default/grub.d")
                forced = []
                if os.path.isdir(gd):
                    for fn in sorted(os.listdir(gd)):
                        try:
                            txt = open(os.path.join(gd, fn), encoding="utf-8", errors="replace").read()
                        except OSError:
                            continue
                        if any(l.lstrip().startswith("GRUB_FORCE_PARTUUID=") for l in txt.splitlines()):
                            forced.append(fn)
                facts["grub_root_refs"] = [f"{k}={v}" for k, v in refs]
                facts["forced_partuuid_files"] = forced
                facts["ok"], facts["detail"] = evaluate_root_identity(
                    refs, forced, facts["target_root_partuuid"], facts["target_root_fsuuid"]
                )
    except Exception as e:  # noqa: BLE001
        facts["detail"] = f"INSTALLED_ROOT_IDENTITY_INVALID: {type(e).__name__}: {e}"
    return facts


def installed_settings(disk, run=None):
    """Read, offline and read-only, what the installer wrote from the
    window's settings: the keyboard layout and the machine's name."""
    run = run or _run_cmd
    facts = {}
    try:
        with NbdAttach(disk, run=run) as nbd:
            mnt = disk + ".setmnt"
            with RoMount(f"{nbd.dev}p2", mnt, run=run):
                kb = open(os.path.join(mnt, "etc/default/keyboard"), encoding="utf-8").read()
                m = re.search(r'^XKBLAYOUT="([a-z]+)"', kb, re.M)
                facts["keyboard"] = m.group(1) if m else None
                facts["hostname"] = open(os.path.join(mnt, "etc/hostname"), encoding="utf-8").read().strip()
    except Exception as e:  # noqa: BLE001
        facts["error"] = f"{type(e).__name__}: {e}"
    return facts


def _unique_dir(base):
    """`base`, or `base-2`, `base-3`… so repeated failures never overwrite
    earlier evidence silently."""
    cand, n = base, 2
    while os.path.exists(cand):
        cand, n = f"{base}-{n}", n + 1
    return cand


def preserve_iso_failure(work, target, collector=None):
    """On ISO FAIL/INVALID, keep the disk curtin wrote under the work directory
    and gather offline evidence beside it. Never raises: preserving evidence
    must not change the verdict (§59, H)."""
    collector = collector or collect_target_diagnostics
    try:
        failure = _unique_dir(os.path.join(work, "failure"))
        diag = os.path.join(failure, "diagnostics")
        os.makedirs(diag, exist_ok=True)
        summary = {"preserved_disk": None, "diagnostics": [], "collector_error": None}
        if os.path.exists(target):
            preserved = os.path.join(failure, "target.qcow2")
            # A move keeps the actual disk curtin wrote (same filesystem, no copy).
            os.replace(target, preserved)
            summary["preserved_disk"] = preserved
            try:
                summary["diagnostics"] = collector(preserved, diag)
            except Exception as e:  # noqa: BLE001 — evidence collection is best-effort
                summary["collector_error"] = f"{type(e).__name__}: {e}"
                log(f"ISO: offline evidence collection failed: {e}")
        else:
            log(f"ISO: no target disk at {target} to preserve")
        with open(os.path.join(diag, "COLLECTION.json"), "w") as f:
            json.dump(summary, f, indent=1)
        log(f"ISO: failure evidence under {failure}")
        return failure
    except Exception as e:  # noqa: BLE001 — never let preservation break the run
        log(f"ISO: could not preserve failure evidence: {e}")
        return None


def esp_boot_artifacts(files, suffix):
    """Shim and grub of one architecture in one vendor directory, and the
    removable fallback loader."""
    vendors = {f.split("/")[1] for f in files if f.startswith("EFI/") and f.count("/") == 2} - {"BOOT"}
    paired = any(f"EFI/{v}/shim{suffix}.efi" in files and f"EFI/{v}/grub{suffix}.efi" in files for v in vendors)
    return paired and f"EFI/BOOT/BOOT{suffix.upper()}.EFI" in files


def scenario_iso(a):
    r = Results(f"iso-{a.arch}")
    work = a.work
    target = os.path.join(work, "target.qcow2")
    data = os.path.join(work, "data.raw")
    small = os.path.join(work, "small.raw")
    invalid = None
    try:
        sh("qemu-img", "create", "-q", "-f", "qcow2", target, "40G")
        make_data_disk(data, "24G")
        sh("qemu-img", "create", "-q", "-f", "raw", small, "8G")
        data_before, small_before = sha256(data), sha256(small)
        vm = Vm("iso", a.arch, work, memory=2560)
        # No network at all: the base install is offline.
        vm.start([{"file": target, "serial": "OCY-TARGET-7F3A"}, {"file": data, "format": "raw", "serial": "DATA0001"},
                  {"file": small, "format": "raw", "serial": "SMALL01"}], cdrom=a.iso, net=False)
        # The boot menu waits for the operator: nothing may start on its own,
        # least of all the installer. Then Install is chosen explicitly.
        vm.expect(r"Experimentar o Ocinye OS", 600)
        time.sleep(40)
        early = open(vm.transcript.name, encoding="utf-8", errors="replace").read()
        r.check("ISO: the boot menu waits (no mode starts without a key)",
                not re.search(r"(Linux version|EFI stub|neste computador|OCINYE-SESSION-READY)", early))
        vm.send("i")
        vm.expect(r"Instalar o Ocinye OS neste computador", 1800)
        vm.send("\r")
        vm.expect(r"Escolher \(1", 300)
        text = open(vm.transcript.name, encoding="utf-8", errors="replace").read()
        rows = {}
        for line in text.splitlines()[-60:]:
            mm = re.match(r"^(\[\d+\]\??| - )\s+(\S+)\s", line)
            if mm:
                rows[mm.group(2)] = (mm.group(1), line)
        log(f"OIE disk rows: { {k: v[0] for k, v in rows.items()} }")
        media = [d for d, (st, line) in rows.items() if st == " - " and "SUPORTE" in line]
        r.check("ISO: the installation medium is listed and protected", media == ["sr0"], rows.get("sr0"))
        r.check("ISO: the 8 GB disk is listed as too small", rows.get("vdc", ("", ""))[0] == " - " and "pequeno" in rows.get("vdc", ("", ""))[1], rows.get("vdc"))
        r.check("ISO: the data disk shows its partition", "ext4" in rows.get("vdb", ("", ""))[1], rows.get("vdb"))
        tgt = rows.get("vda", ("", ""))[0]
        r.check("ISO: the blank target is selectable", tgt.startswith("["), rows.get("vda"))
        num = tgt.strip("[]?")
        # The medium's own row cannot be chosen: typing a protected number re-lists.
        vm.send(str(len(rows) + 5) + "\r")
        vm.expect(r"Escolher \(1", 120)
        # Wrong confirmation first: nothing may be written.
        vm.send(num + "\r")
        vm.expect(r"(4 caracteres|4 characters)", 120)
        vm.send("0000\r")
        vm.expect(r"(n[aã]o foi alterado|not changed)", 120)
        info = json.loads(sh("qemu-img", "info", "-U", "--output", "json", target).stdout)
        r.check("ISO: wrong confirmation writes nothing (target still unallocated)", info.get("actual-size", 1 << 30) < (1 << 20), info.get("actual-size"))
        vm.send("\r")
        # The service restarts the flow; this time the right token.
        vm.expect(r"Instalar o Ocinye OS neste computador", 300)
        vm.send("\r")
        vm.expect(r"Escolher \(1", 300)
        vm.send(num + "\r")
        vm.expect(r"(4 caracteres|4 characters)", 120)
        vm.send("7F3A\r")
        vm.expect(r"(Chave SSH do operador|Operator SSH key)", 120)
        vm.expect(DONE_OR_FAIL_RX, 5400)
        installed = base_installed(open(vm.transcript.name, encoding="utf-8", errors="replace").read())
        r.check("ISO: base installed offline (no NIC attached)", installed)
        vm.send("\r")
        try:
            vm.wait_exit(300)
        except subprocess.TimeoutExpired:
            vm.stop()
        else:
            vm.stop()
        r.check("ISO: the data disk is byte-for-byte unchanged", sha256(data) == data_before)
        r.check("ISO: the small disk is byte-for-byte unchanged", sha256(small) == small_before)
        # Before the (slow) second boot, prove OFFLINE that the installed boot
        # config resolves root to THIS target. A stale build-time PARTUUID is a
        # typed INSTALLED_ROOT_IDENTITY_MISMATCH in seconds, not a 900 s timeout.
        ident = installed_root_identity(target)
        log(f"ISO installed root identity: {json.dumps(ident)}")
        id_ok = r.check("ISO: installed boot config resolves to the actual target root", ident["ok"], ident["detail"])
        if id_ok:
            # First boot from the installed disk (same UEFI variables: the boot entry curtin wrote).
            vm2 = Vm("iso", a.arch, work, memory=2048)
            vm2.start([{"file": target, "serial": "OCY-TARGET-7F3A", "boot": True}])
            vm2.expect(UNCLAIMED_RX, BOOT_BUDGET)
            wait_ssh_port(vm2.port, 300)
            k1, k2 = keygen(os.path.join(work, "operator_i")), keygen(os.path.join(work, "operator_j"))
            facts = claim_by_code(r, vm2, k1, k2, "ISO")
            if facts:
                j = ssh(vm2.port, "ocinye", k1, "sudo -n cat /var/log/ocinye/oie-install.json; findmnt -n -o FSTYPE /; findmnt -n -o FSTYPE /boot/efi; sudo -n sfdisk -J /dev/vda; swapon --noheadings | wc -l")
                out = j.stdout.decode()
                r.check("ISO: OIE journal on the installed disk", '"steps"' in out and '"media_check"' in out, out[:200])
                r.check("ISO: ext4 root, vfat ESP, GPT, no swap", "ext4" in out and "vfat" in out and '"label":"gpt"' in out.replace(" ", "") and out.strip().endswith("0"), out[-300:])
                # The ESP carries this architecture's UEFI boot path (shim → grub
                # under EFI/grub, the bootloader-id this image uses) and the
                # removable fallback EFI/BOOT; and the installed disk's saved
                # curtin log must show the offline install fetched nothing — a
                # zero count over the fetch/resolution-failure and in-target
                # install signatures proves no network package op was needed.
                # The vendor directory is the distributor's name as grub
                # derives it, and it is not the same on the two architectures
                # (`grub` on amd64, `ubuntu` on arm64): what is asked is the
                # shim and grub of this architecture in one vendor directory,
                # and the removable fallback.
                efi_suffix = {"amd64": "x64", "arm64": "aa64"}[a.arch]
                p = ssh(vm2.port, "ocinye", k1,
                        "sudo -n find /boot/efi -maxdepth 3 -type f -printf '%P\\n' 2>/dev/null; "
                        "echo '---LOG---'; "
                        "sudo -n grep -ciE "
                        "'no installation candidate|unable to locate package|temporary failure resolving|could not resolve host|installing packages on target system' "
                        "/root/curtin-install.log 2>/dev/null")
                esp_part, _, log_part = p.stdout.decode().partition("---LOG---")
                esp_files = set(esp_part.split())
                r.check("ISO: ESP carries the UEFI boot artifacts for the architecture",
                        esp_boot_artifacts(esp_files, efi_suffix), sorted(esp_files))
                hit = re.search(r"\b(\d+)\b", log_part)
                r.check("ISO: offline install required no package fetch (empty curtin apt)",
                        hit is not None and hit.group(1) == "0", log_part.strip()[:200])
                log(f"ISO resources after claim: {json.dumps(measure(vm2.port, k1))}")
            vm2.stop()
    except Exception as e:  # noqa: BLE001
        invalid = f"{type(e).__name__}: {e}"
    # The same predicate decides the verdict and whether to keep the evidence,
    # so they can never disagree. Preservation runs with every guest already
    # stopped (main()'s finally also stops them), so the disk is quiescent.
    if result_status(r.items, invalid) != "PASS":
        for vm in RUNNING:
            vm.stop()
        preserve_iso_failure(work, target)
    return r.finish(work, invalid=invalid)


def scenario_window(a):
    """The graphical installer (ADR-0031): an installation done with controls.

    The medium is started in a guest with a display; Install is chosen at the
    keyboard, and from there every step is a control of the installer's own
    window, reached on the machine's keyboard: Start, the language, the
    compatibility summary, a disk card, the review, the confirmation dialog and
    Restart. Nothing is answered on a console: the serial line is tried and is
    told the installation belongs elsewhere. What the bench waits on is the
    engine's own trace of the typed exchange (`OCINYE-WINDOW …`), which only
    the window's host writes — so a step observed there was reached through
    the window. The engine is the one the text installer uses: this scenario
    is about the window driving it, not about what it does to a disk, which
    `iso` proves.
    """
    r = Results(f"window-{a.arch}")
    work = a.work
    target = os.path.join(work, "target.qcow2")
    shots = os.path.join(work, "screens")
    os.makedirs(shots, exist_ok=True)
    invalid = None

    def reached(vm, what, timeout):
        try:
            vm.expect(rf"OCINYE-WINDOW {what}\b", timeout)
            return True
        except TimeoutError:
            return False

    class Stopped(Exception):
        """A step the window did not reach: what follows cannot be asked."""

    def need(name, ok, detail=""):
        r.check(name, ok, detail)
        if not ok:
            raise Stopped(name)

    def size():
        return json.loads(sh("qemu-img", "info", "-U", "--output", "json", target).stdout).get("actual-size", 0)

    try:
        sh("qemu-img", "create", "-q", "-f", "qcow2", target, "40G")
        vm = Vm("window", a.arch, work, memory=3072)
        vm.start([{"file": target, "serial": "OCY-TARGET-7F3A"}], cdrom=a.iso, net=False, screen=True)
        vm.expect(r"Experimentar o Ocinye OS", 600)
        time.sleep(10)
        # Install, chosen on the machine's own keyboard.
        vm.press("i")
        need("window: the installer's window reached the engine", reached(vm, "step=WELCOME", 1800))
        time.sleep(20)
        first = vm.screenshot(os.path.join(shots, "1-welcome.png"))
        r.check("window: the screen shows the installer", png_is_a_picture(first))
        # Start, then the language page's Continue: both are the focused control.
        vm.press("ret")
        time.sleep(4)
        vm.screenshot(os.path.join(shots, "2-language.png"))
        vm.press("ret")
        need("window: Continue on the language step asked the engine to begin", reached(vm, "step=COMPATIBILITY", 300))
        # The window now holds the installation. The serial line is the other
        # console: an answer there is refused, in words.
        vm.send("\r")
        try:
            vm.expect(r"(noutra consola|another console)", 30)
            other = True
        except TimeoutError:
            other = False
        r.check("window: the serial console cannot drive the installation and is told so", other)
        time.sleep(4)
        vm.screenshot(os.path.join(shots, "3-compatibility.png"))
        vm.press("ret")
        need("window: Continue on the compatibility summary opened the keyboard step", reached(vm, "step=KEYBOARD", 300))
        settle = 4 if native(a.arch) else 20
        time.sleep(settle)
        # The layouts are cards in order (pt, br, us, gb, fr…): four Tabs from
        # the chosen one reach French; Enter chooses it and moves to Continue.
        for _ in range(4):
            vm.press("tab")
        vm.press("ret")
        time.sleep(settle)
        vm.screenshot(os.path.join(shots, "4-keyboard.png"))
        vm.press("ret")
        need("window: Continue on the keyboard step opened the network step", reached(vm, "step=NETWORK", 120))
        time.sleep(settle)
        vm.screenshot(os.path.join(shots, "5-network.png"))
        # Automatic (DHCP) is chosen: Enter keeps it and moves to Continue.
        vm.press("ret")
        time.sleep(settle)
        vm.press("ret")
        need("window: the network settings were accepted and storage opened", reached(vm, "step=STORAGE", 120))
        time.sleep(settle)
        vm.screenshot(os.path.join(shots, "6-storage.png"))
        # No disk is chosen until a card is: Enter on the focused card chooses
        # it and moves to Continue; Enter there sends the choice.
        vm.press("ret")
        time.sleep(3)
        vm.press("ret")
        need("window: a disk card was chosen with the keyboard, and the engine selected it", reached(vm, "selected=vd[a-z]", 120))
        need("window: the options step followed the disk", reached(vm, "step=ADVANCED", 60))
        time.sleep(settle)
        vm.type("posto-e2e")
        time.sleep(settle)
        vm.screenshot(os.path.join(shots, "7-options.png"))
        vm.press("ret")
        need("window: the engine showed the review", reached(vm, "step=REVIEW", 120))
        time.sleep(4)
        vm.screenshot(os.path.join(shots, "8-review.png"))
        # The destructive action opens the confirmation dialog.
        vm.press("ret")
        time.sleep(4)
        vm.screenshot(os.path.join(shots, "9-confirm-empty.png"))
        # A wrong confirmation: the action stays disabled, and Enter does
        # nothing — the engine is never asked to confirm, and nothing is written.
        before = size()
        vm.type("0000")
        # The same wait as for the right confirmation below: Enter must reach
        # a dialog the engine has answered for the wrong value, or the check
        # proves nothing.
        time.sleep(4 if native(a.arch) else 45)
        vm.press("ret")
        time.sleep(12)
        vm.screenshot(os.path.join(shots, "10-confirm-wrong.png"))
        with vm.lock:
            asked = "OCINYE-WINDOW confirmation=" in vm.buf or "OCINYE-WINDOW step=INSTALLING" in vm.buf
        r.check("window: a wrong confirmation leaves the action disabled; the engine is not asked", not asked)
        r.check("window: nothing was written after the wrong confirmation", size() == before, f"{before} -> {size()}")
        for _ in range(4):
            vm.press("backspace")
            time.sleep(0.1)
        vm.type("7F3A")
        # Enter is pressed once, after the engine has checked what was typed:
        # the window never decides a confirmation, it asks the engine
        # (`check_confirmation`) and enables the action only on its answer, so
        # an earlier Enter is rightly ignored, and a second Enter after the
        # engine accepted would land on the next step. Under full emulation
        # (amd64 on this arm64 bench) that round trip took more than 5 s.
        time.sleep(5 if native(a.arch) else 45)
        vm.screenshot(os.path.join(shots, "11-confirm-right.png"))
        vm.press("ret")
        need("window: the confirmation typed in the dialog was accepted by the engine", reached(vm, "confirmation=accepted", 120))
        need("window: the installation started", reached(vm, "step=INSTALLING", 120))
        time.sleep(60)
        vm.screenshot(os.path.join(shots, "12-installing.png"))
        done = reached(vm, "step=INSTALLED", 5400)
        r.check("window: the installation wrote the chosen disk", size() > (64 << 20), size())
        r.check("window: the engine finished and the window was told", done)
        time.sleep(6)
        vm.screenshot(os.path.join(shots, "13-installed.png"))
        # Restart is the focused control: the machine goes down and its
        # firmware starts again. The window says to remove the medium when it
        # does; the bench removes it, and the installed disk starts on its own,
        # its console saying it is unclaimed.
        restarted = False
        if done:
            vm.press("ret")
            try:
                vm.expect(r"(Stopped target graphical\.target|Reached target reboot\.target|reboot: Restarting system)", 600)
                vm.expect(r"(GNU GRUB|BdsDxe|UEFI firmware|EFI stub)", 900)
                restarted = True
            except TimeoutError:
                restarted = False
        r.check("window: Restart was pressed in the window, and the machine restarted", restarted)
        vm.stop()
        booted = False
        if restarted:
            # Offline, before the installed disk is started and its journal
            # is in use: the boot configuration names the target's own root.
            ident = installed_root_identity(target)
            r.check("window: the installed boot configuration resolves to the target's own root", ident["ok"], ident["detail"])
            chosen = installed_settings(target)
            r.check("window: the keyboard and the name chosen in the window are on the installed disk",
                    chosen.get("keyboard") == "fr" and chosen.get("hostname") == "posto-e2e", chosen)
            vm2 = Vm("window", a.arch, work, memory=2048)
            vm2.start([{"file": target, "serial": "OCY-TARGET-7F3A", "boot": True}])
            try:
                vm2.expect(UNCLAIMED_RX, BOOT_BUDGET)
                booted = True
            except TimeoutError:
                booted = False
            vm2.stop()
        r.check("window: with the medium removed, the installed system started", booted)
    except Stopped:
        vm.screenshot(os.path.join(shots, "stopped.png"))
        vm.stop()
    except Exception as e:  # noqa: BLE001
        invalid = f"{type(e).__name__}: {e}"
    return r.finish(work, invalid=invalid)


def main():
    p = argparse.ArgumentParser()
    p.add_argument("scenario", choices=["virt", "raw", "iso", "seat", "window"])
    p.add_argument("--arch", required=True, choices=["amd64", "arm64"])
    p.add_argument("--work", required=True)
    p.add_argument("--qcow2")
    p.add_argument("--raw-zst")
    p.add_argument("--raw-sha256")
    p.add_argument("--iso")
    p.add_argument("--memory", type=int, default=4096, help="seat: the guest's memory, MiB (an Instance needs 3500)")
    a = p.parse_args()
    os.makedirs(a.work, exist_ok=True)
    try:
        return {"virt": scenario_virt, "raw": scenario_raw, "iso": scenario_iso, "seat": scenario_seat, "window": scenario_window}[a.scenario](a)
    finally:
        # No guest outlives its scenario, whatever happened.
        for vm in RUNNING:
            vm.stop()


if __name__ == "__main__":
    sys.exit(main())
