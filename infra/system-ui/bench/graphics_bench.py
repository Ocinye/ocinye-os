#!/usr/bin/env python3
"""Graphics bench for the native system surfaces (ADR-0031).

Boots an Ubuntu 24.04 cloud image as a QEMU guest with a virtual GPU, keyboard
and tablet, copies binaries in, runs a command, and takes a screenshot of the
guest's screen through the QEMU monitor. It answers one question by looking:
does the compositor take the screen and show a client?

  graphics_bench.py prepare --arch A --base IMAGE --work D
        build the guest disk (packages installed once, from the network)
  graphics_bench.py run --arch A --work D --copy SRC:DST … --exec COMMAND --shot OUT.png [--wait S]
        [--step wait:S|shot:NAME|click:X,Y|type:TEXT|key:QCODE|sh:COMMAND …]

Runs inside the builder VM, as root. Nothing here ships.
"""
import argparse
import json
import os
import shutil
import socket
import subprocess
import sys
import time

PACKAGES = ["seatd", "libseat1", "libinput10", "libgbm1", "libegl1", "libgles2", "libgl1-mesa-dri",
            "libxkbcommon0", "libwayland-server0", "weston", "fonts-dejavu-core",
            "libwebkit2gtk-4.1-0", "libgtk-3-0"]
KEY = "bench_ed25519"
PORT = 2231


def sh(*cmd, **kw):
    return subprocess.run(cmd, check=True, **kw)


def firmware(arch):
    if arch == "amd64":
        return "/usr/share/OVMF/OVMF_CODE_4M.fd", "/usr/share/OVMF/OVMF_VARS_4M.fd"
    return "/usr/share/AAVMF/AAVMF_CODE.fd", "/usr/share/AAVMF/AAVMF_VARS.fd"


def native(arch):
    m = os.uname().machine
    return ((arch == "amd64" and m == "x86_64") or (arch == "arm64" and m == "aarch64")) and os.path.exists("/dev/kvm")


def qemu(arch, work, disk, seed=None):
    code, vars_src = firmware(arch)
    vars_ = os.path.join(work, "vars.fd")
    if not os.path.exists(vars_):
        shutil.copy(vars_src, vars_)
    binary = "qemu-system-x86_64" if arch == "amd64" else "qemu-system-aarch64"
    cmd = [binary, "-m", "3072", "-smp", "3", "-nodefaults", "-display", "none",
           "-drive", f"if=pflash,format=raw,unit=0,readonly=on,file={code}",
           "-drive", f"if=pflash,format=raw,unit=1,file={vars_}",
           "-drive", f"if=virtio,format=qcow2,file={disk}",
           "-device", "virtio-gpu-pci", "-device", "qemu-xhci", "-device", "usb-kbd", "-device", "usb-tablet",
           "-netdev", f"user,id=n0,hostfwd=tcp:127.0.0.1:{PORT}-:22", "-device", "virtio-net-pci,netdev=n0",
           "-device", "virtio-rng-pci",
           "-qmp", f"unix:{work}/qmp.sock,server=on,wait=off",
           "-serial", f"file:{work}/serial.log"]
    if arch == "amd64":
        cmd += ["-machine", "q35"] + (["-accel", "kvm", "-cpu", "host"] if native(arch) else ["-accel", "tcg,thread=multi", "-cpu", "max"])
    else:
        cmd += ["-machine", "virt"] + (["-accel", "kvm", "-cpu", "host"] if native(arch) else ["-accel", "tcg,thread=multi", "-cpu", "max"])
    if seed:
        cmd += ["-drive", f"if=virtio,format=raw,readonly=on,file={seed}"]
    return subprocess.Popen(cmd, stdout=subprocess.DEVNULL, stderr=open(os.path.join(work, "qemu.err"), "w"))


def ssh(work, *command, check=True, timeout=600):
    return subprocess.run(["ssh", "-i", os.path.join(work, KEY), "-p", str(PORT), "-o", "StrictHostKeyChecking=no",
                           "-o", "UserKnownHostsFile=/dev/null", "-o", "LogLevel=ERROR", "-o", "ConnectTimeout=10",
                           "root@127.0.0.1", *command], check=check, timeout=timeout, capture_output=True, text=True)


def wait_ssh(work, budget):
    deadline = time.time() + budget
    while time.time() < deadline:
        try:
            if ssh(work, "true", check=False, timeout=20).returncode == 0:
                return
        except subprocess.TimeoutExpired:
            pass
        time.sleep(5)
    raise SystemExit("INVALID: the guest did not answer on SSH")


def qmp(work, *commands):
    s = socket.socket(socket.AF_UNIX)
    s.connect(os.path.join(work, "qmp.sock"))
    f = s.makefile("rw")
    f.readline()
    out = []
    for c in ({"execute": "qmp_capabilities"},) + commands:
        f.write(json.dumps(c) + "\n")
        f.flush()
        while True:
            r = json.loads(f.readline())
            if "return" in r or "error" in r:
                out.append(r)
                break
    s.close()
    return out[1:]


QCODES = {" ": "spc", "-": "minus", ".": "dot", ",": "comma", "/": "slash", "=": "equal", ";": "semicolon",
          "\n": "ret", "\t": "tab"}
SHIFTED = {"@": "2", "_": "minus", "!": "1", "#": "3", "$": "4", "%": "5", ":": "semicolon", "?": "slash", "+": "equal"}


def key_events(qcode, shift=False):
    k = lambda code, down: {"type": "key", "data": {"down": down, "key": {"type": "qcode", "data": code}}}
    events = [k(qcode, True), k(qcode, False)]
    return [k("shift", True)] + events + [k("shift", False)] if shift else events


def send(work, events):
    r = qmp(work, {"execute": "input-send-event", "arguments": {"events": events}})
    if any("error" in x for x in r):
        raise SystemExit(f"INVALID: the guest refused input: {r}")


def step(a, text):
    """One step of a scripted session with the guest's screen, keyboard and pointer.

    wait:S · shot:NAME · click:X,Y (pixels of a WxH screen) · type:TEXT · key:QCODE · sh:COMMAND
    """
    kind, _, arg = text.partition(":")
    if kind == "wait":
        time.sleep(float(arg))
    elif kind == "shot":
        shot = os.path.join(a.work, "shot.png")
        if os.path.exists(shot):
            os.remove(shot)
        qmp(a.work, {"execute": "screendump", "arguments": {"filename": shot, "format": "png"}})
        shutil.copy(shot, os.path.join(os.path.dirname(a.shot), arg + ".png"))
        print("SHOT", arg)
    elif kind == "click":
        x, y = (int(v) for v in arg.split(","))
        w, h = (int(v) for v in a.screen.split("x"))
        at = [{"type": "abs", "data": {"axis": "x", "value": x * 32767 // w}},
              {"type": "abs", "data": {"axis": "y", "value": y * 32767 // h}}]
        send(a.work, at)
        time.sleep(0.2)
        send(a.work, [{"type": "btn", "data": {"down": True, "button": "left"}}])
        time.sleep(0.1)
        send(a.work, [{"type": "btn", "data": {"down": False, "button": "left"}}])
    elif kind == "type":
        for ch in arg:
            if ch in SHIFTED:
                send(a.work, key_events(SHIFTED[ch], shift=True))
            elif ch.isupper():
                send(a.work, key_events(ch.lower(), shift=True))
            else:
                send(a.work, key_events(QCODES.get(ch, ch)))
            time.sleep(0.03)
    elif kind == "key":
        send(a.work, key_events(arg))
    elif kind == "sh":
        r = ssh(a.work, arg, check=False)
        print("SH", arg, "->", r.returncode)
        print((r.stdout + r.stderr)[-2500:])
    else:
        raise SystemExit(f"unknown step: {text}")


def prepare(a):
    os.makedirs(a.work, exist_ok=True)
    disk = os.path.join(a.work, "guest.qcow2")
    key = os.path.join(a.work, KEY)
    if not os.path.exists(key):
        sh("ssh-keygen", "-q", "-t", "ed25519", "-N", "", "-f", key)
    sh("qemu-img", "create", "-q", "-f", "qcow2", "-F", "qcow2", "-b", os.path.abspath(a.base), disk, "12G")
    user = os.path.join(a.work, "user-data")
    with open(user, "w") as f:
        f.write("#cloud-config\ndisable_root: false\nssh_pwauth: false\nusers:\n  - name: root\n    ssh_authorized_keys:\n      - "
                + open(key + ".pub").read().strip() + "\n")
    seed = os.path.join(a.work, "seed.img")
    sh("cloud-localds", seed, user)
    p = qemu(a.arch, a.work, disk, seed)
    try:
        wait_ssh(a.work, 900 if native(a.arch) else 2400)
        ssh(a.work, "cloud-init status --wait >/dev/null 2>&1; apt-get update -q && DEBIAN_FRONTEND=noninteractive apt-get install -y -q --no-install-recommends "
            + " ".join(PACKAGES), timeout=3600)
        ssh(a.work, "sync; poweroff", check=False)
        p.wait(timeout=180)
    finally:
        if p.poll() is None:
            p.kill()
    print("PREPARED", disk)


def run(a):
    disk = os.path.join(a.work, "guest.qcow2")
    p = qemu(a.arch, a.work, disk)
    try:
        wait_ssh(a.work, 600 if native(a.arch) else 1800)
        for pair in a.copy or []:
            src, dst = pair.split(":", 1)
            sh("scp", "-q", "-i", os.path.join(a.work, KEY), "-P", str(PORT), "-o", "StrictHostKeyChecking=no",
               "-o", "UserKnownHostsFile=/dev/null", "-o", "LogLevel=ERROR", "-r", src, f"root@127.0.0.1:{dst}")
        # The command runs detached; its output stays in the guest for reading back.
        ssh(a.work, f"nohup sh -c {json.dumps(a.exec)} > /root/bench.log 2>&1 &")
        time.sleep(a.wait)
        for text in a.step or []:
            step(a, text)
        shot = os.path.join(a.work, "shot.png")
        r = qmp(a.work, {"execute": "screendump", "arguments": {"filename": shot, "format": "png"}})
        print("SCREENDUMP", json.dumps(r))
        if os.path.exists(shot):
            shutil.copy(shot, a.shot)
        log = ssh(a.work, "cat /root/bench.log; echo ---; ps -eo comm | sort | uniq -c | sort -rn | head -12", check=False)
        print(log.stdout[-3000:])
        ssh(a.work, "poweroff", check=False)
        try:
            p.wait(timeout=120)
        except subprocess.TimeoutExpired:
            pass
    finally:
        if p.poll() is None:
            p.kill()


def main():
    ap = argparse.ArgumentParser()
    sub = ap.add_subparsers(dest="cmd", required=True)
    pr = sub.add_parser("prepare")
    pr.add_argument("--arch", required=True, choices=["amd64", "arm64"])
    pr.add_argument("--base", required=True)
    pr.add_argument("--work", required=True)
    rn = sub.add_parser("run")
    rn.add_argument("--arch", required=True, choices=["amd64", "arm64"])
    rn.add_argument("--work", required=True)
    rn.add_argument("--copy", action="append")
    rn.add_argument("--exec", required=True)
    rn.add_argument("--shot", required=True)
    rn.add_argument("--wait", type=int, default=25)
    rn.add_argument("--step", action="append", help=step.__doc__)
    rn.add_argument("--screen", default="1280x800")
    a = ap.parse_args()
    {"prepare": prepare, "run": run}[a.cmd](a)


if __name__ == "__main__":
    sys.exit(main())
