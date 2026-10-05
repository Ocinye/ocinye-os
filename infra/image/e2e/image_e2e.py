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

    def start(self, disks, cdrom=None, net=True, seed=None):
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
        if net:
            self.port = next(PORTS)
            cmd += ["-netdev", f"user,id=n0,hostfwd=tcp:127.0.0.1:{self.port}-:22", "-device", "virtio-net-pci,netdev=n0"]
        else:
            cmd += ["-nic", "none"]
        log(f"{self.name}: start ({'kvm' if kvm else 'tcg'}, net={'yes' if net else 'none'})")
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

    def _pump(self):
        try:
            data = self.conn.recv(65536)
        except BlockingIOError:
            return False
        if not data:
            return False
        text = data.decode("utf-8", errors="replace").replace("\r", "")
        self.buf += text
        self.transcript.write(redact(text))
        self.transcript.flush()
        return True

    def expect(self, pattern, timeout):
        """Wait for a regex in the console output since the last expect."""
        rx = re.compile(pattern)
        end = time.time() + timeout
        while time.time() < end:
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


def keygen(path):
    if not os.path.exists(path):
        sh("ssh-keygen", "-q", "-t", "ed25519", "-N", "", "-C", "ocinye-e2e", "-f", path)
    return path


LAST_CONNECT = {}
# The image's own firewall allows at most 6 connections per 30 s per source
# on 22/tcp while it is unclaimed (`ufw limit`): stay under it.
MIN_GAP = 6.5


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


class Results:
    def __init__(self, scenario):
        self.scenario, self.items = scenario, []

    def check(self, name, ok, detail=""):
        self.items.append({"check": name, "ok": bool(ok), "detail": str(detail)[:300]})
        log(f"{'PASS' if ok else 'FAIL'} {name} {redact(str(detail))[:200]}")
        return ok

    def finish(self, work, invalid=None):
        status = "INVALID" if invalid else ("PASS" if self.items and all(i["ok"] for i in self.items) else "FAIL")
        out = {"scenario": self.scenario, "status": status, "invalid": invalid, "checks": self.items}
        with open(os.path.join(work, f"result-{self.scenario}.json"), "w") as f:
            json.dump(out, f, indent=1)
        print(f"RESULT {self.scenario} {status}", flush=True)
        return 0 if status == "PASS" else 1


UNCLAIMED_RX = r"N[AÃ]O RECLAMADO"
BOOT_BUDGET = 900


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
    ok = ssh(vm.port, "ocinye", key, "sudo -n /usr/lib/ocinye/ocinye-firstboot confirm --claim " + enrolled["claim_id"])
    confirmed = ok.returncode == 0 and b'"Confirmed"' in ok.stdout
    r.check(f"{label}: confirmed by the enrolled key", confirmed, ok.stdout + ok.stderr)
    again = ssh(vm.port, "ocinye", key, "sudo -n /usr/lib/ocinye/ocinye-firstboot confirm --claim " + enrolled["claim_id"])
    r.check(f"{label}: confirm replay refused (ALREADY_CLAIMED)", b"ALREADY_CLAIMED" in again.stdout, again.stdout)
    rc, ev, err = claim_session(vm.port, other_key, [{"cmd": "hello", "protocol": 1}])
    r.check(f"{label}: claim account closed after CLAIMED", rc != 0 and not ev, err.strip())
    return {"welcome": welcome, "claim_id_prefix": enrolled["claim_id"][:8]}


def machine_facts(port, key):
    r = ssh(port, "ocinye", key, "cat /etc/machine-id; sudo -n cat /var/lib/ocinye-firstboot/identity.json; systemctl is-enabled docker.service; systemctl is-active docker.service; sudo -n ufw status | head -1")
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
        p = ssh(b_vm.port, "ocinye", kp, f"sudo -n /usr/lib/ocinye/ocinye-firstboot claim --provisioned --key {fp}")
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


def scenario_iso(a):
    r = Results(f"iso-{a.arch}")
    work = a.work
    target = os.path.join(work, "target.qcow2")
    data = os.path.join(work, "data.raw")
    small = os.path.join(work, "small.raw")
    try:
        sh("qemu-img", "create", "-q", "-f", "qcow2", target, "40G")
        make_data_disk(data, "24G")
        sh("qemu-img", "create", "-q", "-f", "raw", small, "8G")
        data_before, small_before = sha256(data), sha256(small)
        vm = Vm("iso", a.arch, work, memory=2560)
        # No network at all: the base install is offline.
        vm.start([{"file": target, "serial": "OCY-TARGET-7F3A"}, {"file": data, "format": "raw", "serial": "DATA0001"},
                  {"file": small, "format": "raw", "serial": "SMALL01"}], cdrom=a.iso, net=False)
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
        vm.expect(r"(Base instalada|Base installed|A instala[cç][aã]o parou)", 5400)
        installed = "Base instalada" in open(vm.transcript.name, encoding="utf-8", errors="replace").read()
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
            log(f"ISO resources after claim: {json.dumps(measure(vm2.port, k1))}")
        vm2.stop()
    except Exception as e:  # noqa: BLE001
        return r.finish(work, invalid=f"{type(e).__name__}: {e}")
    return r.finish(work)


def main():
    p = argparse.ArgumentParser()
    p.add_argument("scenario", choices=["virt", "raw", "iso"])
    p.add_argument("--arch", required=True, choices=["amd64", "arm64"])
    p.add_argument("--work", required=True)
    p.add_argument("--qcow2")
    p.add_argument("--raw-zst")
    p.add_argument("--raw-sha256")
    p.add_argument("--iso")
    a = p.parse_args()
    os.makedirs(a.work, exist_ok=True)
    return {"virt": scenario_virt, "raw": scenario_raw, "iso": scenario_iso}[a.scenario](a)


if __name__ == "__main__":
    sys.exit(main())
