#!/usr/bin/env python3
"""Live Storage Safety Proof bench (D013 Live Mode, L0-S).

Answers one question by experiment: does booting and using a mode of the
Ocinye installation medium write to a disk nobody selected?

  storage_proof.py sentinels --work D
  storage_proof.py run --iso ISO --arch A --work D --name N --mode MODE
                       [--attach overlay|readonly] [--media cdrom|usb] [--traps]

Sentinel disks (never an installation target) are attached as SCSI disks, so
they are `sda`, `sdb`, ... and sort BEFORE the medium (`sr0`) in /sys/block:
that is the order a real machine with internal disks presents, and the order in
which casper scans for its medium.

Three witnesses per sentinel:

  1. content hash    SHA-256 of what the guest could see, before and after
  2. write tracking  the sentinel sits under an empty qcow2 overlay; any guest
                     write, even of identical bytes, allocates a cluster there
  3. read-only run   (--attach readonly) the same boot with the sentinel
                     write-protected by the hypervisor; a write attempt shows as
                     a refusal in the guest's kernel log

Every run ends in one line: RESULT <name> PASS|FAIL|INVALID. INVALID is a bench
failure and never a pass. A FAIL names the sentinels that changed and how.
"""
import argparse
import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from image_e2e import (DONE_OR_FAIL_RX, Vm, base_installed, firmware, installed_root_identity, log, native,  # noqa: E402
                       sh)

MIB = 1 << 20

# name -> (size MiB, what it stands for)
SENTINELS = {
    "A1-ext4-clean": (128, "A: GPT + ext4, cleanly unmounted"),
    "A2-ext4-dirty": (128, "A: GPT + ext4 with an unrecovered journal"),
    "B-esp-fat32": (128, "B: EFI system partition, FAT32"),
    "C1-lvm-pv": (96, "C: LVM physical volume 1 of volume group sentvg"),
    "C2-lvm-pv": (96, "C: LVM physical volume 2 of volume group sentvg"),
    "D1-md-raid1": (96, "D: mdraid RAID1 member 1"),
    "D2-md-raid1": (96, "D: mdraid RAID1 member 2"),
    "E-linux-install": (768, "E: existing Linux installation: ESP, ext4 root with fstab, swap"),
    "F-data-raw": (64, "F: arbitrary data, no partition table"),
    "G-ntfs": (128, "G: GPT + NTFS"),
    "H-luks2": (64, "H: LUKS2 container"),
    "I-swap": (128, "I: GPT with a swap partition"),
    "J-label-traps": (192, "J: partitions labelled writable, casper-rw, OCINYE_OS (ext4)"),
}
# Disks that an unguarded casper would boot from instead of the medium: kept
# out of the default set (they stop an unguarded boot) and attached with --traps.
TRAPS = {
    "K-fake-medium": (96, "K: ext4 partition labelled OCINYE_OS holding casper/fake.squashfs"),
    "L-wholedisk-ext4": (96, "L: ext4 directly on the disk, no partition table"),
}


# The installation target of the install scenarios: 22 GB, sparse.
TARGET_MIB = 22 * 1024
TARGET_SERIAL = "OCY-TARGET-7F3A"


def sha256_stream(cmd):
    p = subprocess.Popen(cmd, stdout=subprocess.PIPE)
    h = hashlib.sha256()
    n = 0
    for chunk in iter(lambda: p.stdout.read(4 * MIB), b""):
        h.update(chunk)
        n += len(chunk)
    if p.wait() != 0:
        raise RuntimeError(f"{cmd[0]} failed")
    return h.hexdigest(), n


def sha256_file(path):
    return sha256_stream(["cat", path])


class Loop:
    """A loop device over an image, with its partitions. Read-only when asked."""

    def __init__(self, path, ro=False):
        self.path, self.ro, self.dev = path, ro, None

    def __enter__(self):
        args = ["losetup", "--show", "-f", "-P"] + (["-r"] if self.ro else []) + [self.path]
        self.dev = sh(*args).stdout.decode().strip()
        sh("udevadm", "settle", check=False)
        return self.dev

    def __exit__(self, *_):
        sh("udevadm", "settle", check=False)
        sh("losetup", "-d", self.dev, check=False)


def _raw(path, mib):
    sh("truncate", "-s", f"{mib}M", path)


def _mounted(dev, mnt, fn, opts=None):
    os.makedirs(mnt, exist_ok=True)
    sh("mount", *(["-o", opts] if opts else []), dev, mnt)
    try:
        fn(mnt)
    finally:
        sh("umount", mnt)


def _files(mnt, n=20):
    for i in range(n):
        with open(os.path.join(mnt, f"sentinel-{i:02d}.txt"), "w") as f:
            f.write(f"ocinye storage sentinel file {i}\n" * 64)


def build_sentinels(work):
    """Deterministic in structure; random only where the content is the point."""
    d = os.path.join(work, "sentinels")
    if os.path.exists(os.path.join(d, "MANIFEST.json")):
        log("sentinels: already built")
        return d
    shutil.rmtree(d, ignore_errors=True)
    os.makedirs(d)
    mnt = os.path.join(work, "mnt")
    p = lambda n: os.path.join(d, n + ".raw")  # noqa: E731

    # A1: clean ext4.
    _raw(p("A1-ext4-clean"), 128)
    sh("sgdisk", "-n", "1:1MiB:0", "-t", "1:8300", "-c", "1:sent-a", p("A1-ext4-clean"))
    with Loop(p("A1-ext4-clean")) as dev:
        sh("mkfs.ext4", "-q", "-L", "SENT_A1", dev + "p1")
        _mounted(dev + "p1", mnt, _files)

    # A2: the same, copied while mounted read-write with committed but
    # unrecovered journal transactions: `needs_recovery` is set in the copy.
    src = os.path.join(work, "a2-src.raw")
    _raw(src, 128)
    sh("sgdisk", "-n", "1:1MiB:0", "-t", "1:8300", "-c", "1:sent-a2", src)
    with Loop(src) as dev:
        sh("mkfs.ext4", "-q", "-L", "SENT_A2", dev + "p1")
        os.makedirs(mnt, exist_ok=True)
        sh("mount", dev + "p1", mnt)
        try:
            _files(mnt, 40)
            for i in range(200):
                os.makedirs(os.path.join(mnt, f"dir-{i:03d}"), exist_ok=True)
            sh("sync", "-f", mnt)
            sh("blockdev", "--flushbufs", dev, check=False)
            shutil.copyfile(src, p("A2-ext4-dirty"))
        finally:
            sh("umount", mnt)
    os.remove(src)

    # B: an ESP.
    _raw(p("B-esp-fat32"), 128)
    sh("sgdisk", "-n", "1:1MiB:0", "-t", "1:ef00", "-c", "1:EFI System Partition", p("B-esp-fat32"))
    with Loop(p("B-esp-fat32")) as dev:
        sh("mkfs.vfat", "-F", "32", "-n", "SENT_ESP", dev + "p1")

        def esp(m):
            os.makedirs(os.path.join(m, "EFI/BOOT"))
            with open(os.path.join(m, "EFI/BOOT/BOOTX64.EFI"), "wb") as f:
                f.write(b"not a real loader: storage sentinel\n" * 512)
        _mounted(dev + "p1", mnt, esp)

    # C: an LVM volume group over two whole-disk PVs, one LV with ext4.
    for n in ("C1-lvm-pv", "C2-lvm-pv"):
        _raw(p(n), 96)
    with Loop(p("C1-lvm-pv")) as l1, Loop(p("C2-lvm-pv")) as l2:
        try:
            sh("pvcreate", "-q", "-ff", "-y", l1, l2)
            sh("vgcreate", "-q", "sentvg", l1, l2)
            sh("lvcreate", "-q", "-y", "-L", "120M", "-n", "sentlv", "sentvg")
            sh("udevadm", "settle", check=False)
            sh("mkfs.ext4", "-q", "-L", "SENT_LV", "/dev/sentvg/sentlv")
            _mounted("/dev/sentvg/sentlv", mnt, _files)
        finally:
            sh("vgchange", "-an", "sentvg", check=False)
            sh("udevadm", "settle", check=False)

    # D: a RAID1 over two members, synced, with ext4 on it.
    for n in ("D1-md-raid1", "D2-md-raid1"):
        _raw(p(n), 96)
    with Loop(p("D1-md-raid1")) as l1, Loop(p("D2-md-raid1")) as l2:
        md = "/dev/md/ocinye-sentinel"
        try:
            sh("mdadm", "--create", md, "--run", "--quiet", "--level=1", "--raid-devices=2",
               "--metadata=1.2", "--name=ocinye-sentinel", "--homehost=any", l1, l2)
            sh("mdadm", "--wait", md, check=False)
            sh("mkfs.ext4", "-q", "-L", "SENT_MD", md)
            _mounted(md, mnt, _files)
            sh("mdadm", "--wait", md, check=False)
        finally:
            sh("mdadm", "--stop", md, check=False)
            sh("udevadm", "settle", check=False)

    # E: something that looks like an installed Linux.
    _raw(p("E-linux-install"), 768)
    sh("sgdisk", "-n", "1:1MiB:+96MiB", "-t", "1:ef00", "-c", "1:EFI System Partition",
       "-n", "2:0:+512MiB", "-t", "2:8304", "-c", "2:root",
       "-n", "3:0:0", "-t", "3:8200", "-c", "3:swap", p("E-linux-install"))
    with Loop(p("E-linux-install")) as dev:
        sh("mkfs.vfat", "-F", "32", "-n", "SENT_EFI", dev + "p1")
        sh("mkfs.ext4", "-q", "-L", "sentinel-root", dev + "p2")
        sh("mkswap", "-q", "-L", "sentinel-swap", dev + "p3")

        def root(m):
            for sub in ("etc", "boot/grub", "usr/lib", "var/log", "home/user"):
                os.makedirs(os.path.join(m, sub))
            with open(os.path.join(m, "etc/fstab"), "w") as f:
                f.write("LABEL=sentinel-root / ext4 errors=remount-ro 0 1\n"
                        "LABEL=SENT_EFI /boot/efi vfat umask=0077 0 1\n"
                        "LABEL=sentinel-swap none swap sw 0 0\n")
            with open(os.path.join(m, "etc/os-release"), "w") as f:
                f.write('NAME="Sentinel Linux"\nID=sentinel\nVERSION_ID="1"\n')
            with open(os.path.join(m, "boot/grub/grub.cfg"), "w") as f:
                f.write("# sentinel\nmenuentry 'Sentinel Linux' { linux /vmlinuz root=LABEL=sentinel-root }\n")
            _files(os.path.join(m, "home/user"))
        _mounted(dev + "p2", mnt, root)

        def efi(m):
            os.makedirs(os.path.join(m, "EFI/ubuntu"))
            with open(os.path.join(m, "EFI/ubuntu/grub.cfg"), "w") as f:
                f.write("# sentinel\n")
        _mounted(dev + "p1", mnt, efi)

    # F: bytes and nothing else.
    with open(p("F-data-raw"), "wb") as f:
        f.write(os.urandom(64 * MIB))

    # G: NTFS.
    _raw(p("G-ntfs"), 128)
    sh("sgdisk", "-n", "1:1MiB:0", "-t", "1:0700", "-c", "1:Basic data partition", p("G-ntfs"))
    with Loop(p("G-ntfs")) as dev:
        sh("mkfs.ntfs", "-Q", "-q", "-L", "SENT_NTFS", dev + "p1")

    # H: LUKS2 (a throwaway key; nothing ever opens it).
    _raw(p("H-luks2"), 64)
    key = os.path.join(work, "luks.key")
    with open(key, "wb") as f:
        f.write(os.urandom(64))
    sh("cryptsetup", "luksFormat", "--type", "luks2", "--batch-mode", "--pbkdf", "pbkdf2",
       "--pbkdf-force-iterations", "1000", "--key-file", key, p("H-luks2"))
    os.remove(key)

    # I: swap.
    _raw(p("I-swap"), 128)
    sh("sgdisk", "-n", "1:1MiB:0", "-t", "1:8200", "-c", "1:swap", p("I-swap"))
    with Loop(p("I-swap")) as dev:
        sh("mkswap", "-q", "-L", "SENT_SWAP", dev + "p1")

    # J: labels casper gives a meaning to, on a disk that is not the medium.
    _raw(p("J-label-traps"), 192)
    sh("sgdisk", "-n", "1:1MiB:+60MiB", "-t", "1:8300", "-n", "2:0:+60MiB", "-t", "2:8300",
       "-n", "3:0:0", "-t", "3:8300", p("J-label-traps"))
    with Loop(p("J-label-traps")) as dev:
        for part, label in (("p1", "writable"), ("p2", "casper-rw"), ("p3", "OCINYE_OS")):
            sh("mkfs.ext4", "-q", "-L", label, dev + part)
            _mounted(dev + part, mnt, lambda m: _files(m, 3))

    # K, L: disks an unguarded casper would take for its medium.
    _raw(p("K-fake-medium"), 96)
    sh("sgdisk", "-n", "1:1MiB:0", "-t", "1:8300", p("K-fake-medium"))
    with Loop(p("K-fake-medium")) as dev:
        sh("mkfs.ext4", "-q", "-L", "OCINYE_OS", dev + "p1")

        def fake(m):
            os.makedirs(os.path.join(m, "casper"))
            with open(os.path.join(m, "casper/fake.squashfs"), "wb") as f:
                f.write(b"not a squashfs: storage sentinel\n" * 1024)
        _mounted(dev + "p1", mnt, fake)
    _raw(p("L-wholedisk-ext4"), 96)
    sh("mkfs.ext4", "-q", "-F", "-L", "SENT_WHOLE", p("L-wholedisk-ext4"))

    manifest = {}
    for name, (mib, what) in {**SENTINELS, **TRAPS}.items():
        digest, size = sha256_file(p(name))
        os.chmod(p(name), 0o444)
        manifest[name] = {"what": what, "bytes": size, "sha256": digest, "structure": structure(p(name))}
    with open(os.path.join(d, "MANIFEST.json"), "w") as f:
        json.dump(manifest, f, indent=1, sort_keys=True)
    log(f"sentinels: {len(manifest)} built in {d}")
    return d


def ensure_target(sdir):
    """T: an installation target that is not blank: a stale RAID member and a
    stale LVM volume group. Installing onto it must still work, and only after
    the confirmation. Sparse; never hashed (it is meant to be written)."""
    path = os.path.join(sdir, "T-stale-target.raw")
    if os.path.exists(path):
        return path
    p = lambda n: os.path.join(sdir, n + ".raw")  # noqa: E731
    _raw(p("T-stale-target"), TARGET_MIB)
    sh("sgdisk", "-n", "1:1MiB:+512MiB", "-t", "1:fd00", "-n", "2:0:+1024MiB", "-t", "2:8e00", p("T-stale-target"))
    with Loop(p("T-stale-target")) as dev:
        md = "/dev/md/ocinye-stale"
        try:
            sh("mdadm", "--create", md, "--run", "--quiet", "--force", "--level=1", "--raid-devices=1",
               "--metadata=1.2", "--name=ocinye-stale", "--homehost=any", dev + "p1")
            sh("mkfs.ext4", "-q", "-L", "STALE_MD", md)
        finally:
            sh("mdadm", "--stop", md, check=False)
        try:
            sh("pvcreate", "-q", "-ff", "-y", dev + "p2")
            sh("vgcreate", "-q", "stalevg", dev + "p2")
            sh("lvcreate", "-q", "-y", "-L", "512M", "-n", "stalelv", "stalevg")
            sh("udevadm", "settle", check=False)
            sh("mkfs.ext4", "-q", "-L", "STALE_LV", "/dev/stalevg/stalelv")
        finally:
            sh("vgchange", "-an", "stalevg", check=False)
            sh("udevadm", "settle", check=False)
    os.chmod(p("T-stale-target"), 0o444)

    return path


def _ext(dev):
    out = sh("dumpe2fs", "-h", dev, check=False).stdout.decode(errors="replace")
    keep = ("Filesystem state", "Mount count", "Last mount time", "Last write time", "Filesystem features",
            "Journal sequence", "Journal start", "Filesystem UUID", "Last mounted on")
    return {k.strip(): v.strip() for k, v in (line.split(":", 1) for line in out.splitlines() if ":" in line)
            if k.strip() in keep}


def structure(path):
    """What the metadata says, read through a READ-ONLY loop device (so that
    taking the evidence cannot be the mutation)."""
    ev = {}
    with Loop(path, ro=True) as dev:
        probe = sh("lsblk", "-J", "-b", "-o", "NAME,SIZE,FSTYPE,LABEL,UUID,PARTTYPE,PARTLABEL", dev, check=False)
        try:
            nodes = json.loads(probe.stdout)["blockdevices"]
        except (ValueError, KeyError):
            nodes = []
        flat = []
        for n in nodes:
            flat.append(n)
            flat.extend(n.get("children", []))
        for n in flat:
            name, fst = "/dev/" + n["name"], n.get("fstype") or ""
            # blkid low-level probe: lsblk asks udev, which may not have looked yet.
            b = sh("blkid", "-p", "-o", "export", name, check=False).stdout.decode(errors="replace")
            kv = dict(line.split("=", 1) for line in b.splitlines() if "=" in line)
            fst = kv.get("TYPE", fst)
            item = {"type": fst, "label": kv.get("LABEL", ""), "uuid": kv.get("UUID", "")}
            if fst in ("ext2", "ext3", "ext4"):
                item["ext"] = _ext(name)
            elif fst == "linux_raid_member":
                x = sh("mdadm", "--examine", name, check=False).stdout.decode(errors="replace")
                item["md"] = {k.strip(): v.strip() for k, v in
                              (line.split(":", 1) for line in x.splitlines() if " : " in line)
                              if k.strip() in ("Events", "Update Time", "State", "Array State", "Checksum")}
            elif fst == "LVM2_member":
                head = sh("dd", f"if={name}", "bs=1M", "count=2", "status=none", check=False).stdout
                item["lvm_seqno"] = sorted(set(re.findall(rb"seqno = (\d+)", head)))[-1:]
                item["lvm_seqno"] = [s.decode() for s in item["lvm_seqno"]]
            elif fst == "crypto_LUKS":
                x = sh("cryptsetup", "luksDump", name, check=False).stdout.decode(errors="replace")
                item["luks"] = {k.strip(): v.strip() for k, v in
                                (line.split(":", 1) for line in x.splitlines() if ":" in line)
                                if k.strip() in ("Epoch", "UUID", "Version")}
            elif fst in ("vfat", "ntfs", "swap"):
                head = sh("dd", f"if={name}", "bs=4096", "count=2", "status=none", check=False).stdout
                item["head_sha256"] = hashlib.sha256(head).hexdigest()
            ev[n["name"].replace(os.path.basename(dev), "disk")] = item
    return ev


def firmware_vars(path):
    """UEFI variables in a variable store: name -> digest of its value. The
    firmware and shim create several on every boot; what matters is whether a
    mode adds or changes one beyond what a boot that only reaches the menu
    does (the `menu` mode is that control)."""
    out = path + ".json"
    r = sh("virt-fw-vars", "-i", path, "--output-json", out, check=False)
    if r.returncode != 0 or not os.path.exists(out):
        return None
    try:
        doc = json.load(open(out))
    finally:
        os.remove(out)
    return {v["name"]: hashlib.sha256(json.dumps(v, sort_keys=True).encode()).hexdigest()[:16]
            for v in doc.get("variables", [])}


def overlay_writes(overlay):
    """Bytes of guest data the overlay holds: 0 means no write ever reached the
    sentinel. (`qemu-img map` reports each extent with the layer it comes from:
    depth 0 is the overlay itself.)"""
    m = json.loads(sh("qemu-img", "map", "-U", "--output=json", overlay).stdout)
    return sum(e["length"] for e in m if e.get("depth") == 0 and e.get("data"))


WRITE_RX = re.compile(
    r"(recovery complete|recovering journal|EXT4-fs \([a-z0-9]+\): mounted|EXT4-fs \([a-z0-9-]+\): mounting"
    r"|XFS \([a-z0-9]+\): Mounting|FAT-fs \([a-z0-9]+\)|ntfs3?[:(]|NTFS|Adding \d+k swap|md/raid1:md|md\d+: detected"
    r"|md: md\d+|device-mapper: |write access unavailable|recovery required on readonly"
    r"|I/O error|Buffer I/O error|critical target error|Write Protect|Medium not present|EXT4-fs error|read-only"
    r"|Remounting filesystem|blk_update_request|print_req_error)")


class Machine(Vm):
    """A guest whose data disks are SCSI (sda, sdb, ...) and sort before the
    medium, with the medium as a virtual CD or as a USB stick."""

    def start_bench(self, disks, medium, media="cdrom", net=True):
        # scsi-id 0.. in the order given: that is the order the kernel names them.
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
            ro = ",readonly=on" if d.get("readonly") else ""
            cmd += ["-drive", f"if=none,id=d{i},file={d['file']},format={d['format']}{ro}",
                    "-device", f"scsi-hd,drive=d{i},bus=scsi0.0,scsi-id={i},serial={d['serial']}"]
        if media == "cdrom":
            cmd += ["-drive", f"if=none,id=cd,media=cdrom,readonly=on,format=raw,file={medium['file']}",
                    "-device", f"scsi-cd,drive=cd,bus=scsi0.0,scsi-id={len(disks)},bootindex=1"]
        else:
            # A USB stick: writable as far as the guest knows, so that anything
            # written to it lands in the overlay and is seen.
            cmd += ["-device", "qemu-xhci,id=xhci",
                    "-drive", f"if=none,id=stick,format={medium['format']},file={medium['file']}",
                    "-device", "usb-storage,drive=stick,bus=xhci.0,removable=on,bootindex=1"]
        if net:
            cmd += ["-netdev", "user,id=n0", "-device", "virtio-net-pci,netdev=n0"]
        else:
            cmd += ["-nic", "none"]
        log(f"{self.name}: start ({'kvm' if kvm else 'tcg'}, {len(disks)} sentinels, medium={media})")
        self._launch(cmd)

    def _launch(self, cmd):
        import socket
        import threading
        from image_e2e import RUNNING
        RUNNING.append(self)
        self.proc = subprocess.Popen(cmd, stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL,
                                     stderr=open(os.path.join(self.work, f"{self.name}-qemu.err"), "a"))
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
        self.lock = threading.Lock()
        threading.Thread(target=self._reader, daemon=True).start()

    def text(self):
        return open(self.transcript.name, encoding="utf-8", errors="replace").read()


# GRUB hotkeys of the normative menu (ocinye_image_contracts::bootmode::BOOT_MENU).
MENU_KEYS = {"live": "t", "install": "i", "install-full": "i", "hardware-check": "h", "recovery": "ad",
             "restricted": "as", "menu": ""}
MENU_RX = r"(Experimentar o Ocinye OS|Try Ocinye OS)"
SESSION_RX = r"OCINYE-SESSION-READY"
LEGACY_RX = r"Instalar o Ocinye OS neste computador"


def medium_fact(vm):
    m = re.findall(r"OCINYE-MEDIUM (\{.*\})", vm.text())
    if not m:
        return {}
    try:
        return {"medium": json.loads(m[-1])}
    except ValueError:
        return {"medium_unparsed": m[-1][:300]}


def drive_install(vm, boot_budget, facts, checkpoints):
    """The installer, as an operator: look, choose the target, mistype the
    confirmation, then confirm. `checkpoints(label)` records what has been
    written to every disk at that moment."""
    vm.expect(LEGACY_RX, boot_budget)
    facts.update(medium_fact(vm))
    time.sleep(30)
    checkpoints("first screen, nothing selected")
    vm.send("\r")
    vm.expect(r"Escolher \(1", 300)
    rows = {}
    for line in vm.text().splitlines()[-80:]:
        mm = re.match(r"^(\[\d+\]\??| - )\s+(\S+)\s", line)
        if mm:
            rows[mm.group(2)] = (mm.group(1), line.strip())
    facts["rows"] = {k: v[1][:110] for k, v in rows.items()}
    target = [(st, dev) for dev, (st, line) in rows.items() if "7F3A" in line]
    selectable = [dev for dev, (st, _) in rows.items() if st.startswith("[")]
    facts["selectable"] = selectable
    if len(target) != 1 or not target[0][0].startswith("["):
        raise RuntimeError(f"target row not found or not selectable: {target}")
    num = target[0][0].strip("[]?")
    time.sleep(20)
    checkpoints("disk list shown, nothing selected")
    vm.send(num + "\r")
    vm.expect(r"(4 caracteres|4 characters)", 120)
    time.sleep(20)
    checkpoints("target selected, not confirmed")
    vm.send("0000\r")
    vm.expect(r"(n[aã]o foi alterado|not changed)", 120)
    time.sleep(20)
    checkpoints("wrong confirmation")
    vm.send("\r")
    vm.expect(LEGACY_RX, 300)
    vm.send("\r")
    vm.expect(r"Escolher \(1", 300)
    vm.send(num + "\r")
    vm.expect(r"(4 caracteres|4 characters)", 120)
    vm.send("7F3A\r")
    vm.expect(r"(Chave SSH do operador|Operator SSH key)", 300)
    vm.expect(DONE_OR_FAIL_RX, 5400)
    facts["installed"] = base_installed(vm.text())
    time.sleep(20)
    checkpoints("installation finished")
    vm.send("\r")
    time.sleep(60)
    vm.stop()
    return ("installed" if facts["installed"] else "INSTALL DID NOT COMPLETE"), facts


def drive(vm, mode, boot_budget, checkpoints=lambda label: None):
    """Bring the guest to its first screen, use it, and stop it. Returns
    (how it stopped, facts the guest printed about itself)."""
    facts = {}
    if mode == "legacy":
        # The frozen Phase A medium: one entry, booted by GRUB after 10 s.
        vm.expect(LEGACY_RX, boot_budget)
        time.sleep(90)
        vm.stop()
        return "stopped at the installer's first screen after 90 s (no shutdown path exists there)", facts
    vm.expect(MENU_RX, 600)
    # The menu must wait: nothing may start without a key.
    waited = 45
    time.sleep(waited)
    if re.search(r"(Linux version|EFI stub|Loading Linux|OCINYE-SESSION-READY|" + LEGACY_RX + ")", vm.text()):
        raise RuntimeError("the boot menu started something without a key")
    facts["menu_waited_seconds"] = waited
    if mode == "menu":
        # Control: the firmware, shim and GRUB only. No kernel is loaded.
        vm.stop()
        return "stopped at the boot menu (control: no kernel loaded)", facts
    for key in MENU_KEYS[mode]:
        vm.send(key)
        time.sleep(3)
    if mode == "install":
        vm.expect(LEGACY_RX, boot_budget)
        time.sleep(90)
        facts.update(medium_fact(vm))
        vm.stop()
        return "stopped at the installer's first screen after 90 s", facts
    if mode == "install-full":
        return drive_install(vm, boot_budget, facts, checkpoints)
    vm.expect(SESSION_RX, boot_budget)
    # Normal flows of the session: inventory again, the report, the install
    # action (which must only explain a restart) and its refusal.
    for key, rx in (("d", r"OCINYE-INVENTORY-END"), ("h", r"OCINYE-REPORT-END"), ("n", r"OCINYE-NETWORK-END"),
                    ("i", r"OCINYE-INSTALL-NOTICE"), ("", SESSION_RX), ("d", r"OCINYE-INVENTORY-END")):
        vm.expect(SESSION_RX, 300) if key in ("h", "n", "i") else None
        vm.send(key + "\r")
        vm.expect(rx, 300)
    vm.expect(SESSION_RX, 300)
    time.sleep(45)
    m = re.findall(r"OCINYE-POLICY (\{.*\})", vm.text())
    if m:
        try:
            facts["policy"] = json.loads(m[-1])
        except ValueError:
            facts["policy_unparsed"] = m[-1][:400]
    vm.send("p\r")
    vm.expect(r"OCINYE-POWEROFF", 120)
    try:
        vm.wait_exit(600)
        how = "powered off from the session"
    except subprocess.TimeoutExpired:
        vm.stop()
        how = "INVALID: power-off did not complete"
    return how, facts


def run(a):
    work = os.path.join(a.work, a.name)
    shutil.rmtree(work, ignore_errors=True)
    os.makedirs(work)
    sdir = build_sentinels(a.work)
    manifest = json.load(open(os.path.join(sdir, "MANIFEST.json")))
    names = list(SENTINELS) + (list(TRAPS) if a.traps else [])
    disks, before = [], {}
    for name in names:
        base = os.path.join(sdir, name + ".raw")
        digest, _ = sha256_file(base)
        if digest != manifest[name]["sha256"]:
            print(f"RESULT {a.name} INVALID sentinel {name} differs from its manifest before the run", flush=True)
            return 2
        before[name] = {"sha256": digest, "structure": manifest[name]["structure"]}
        if a.attach == "overlay":
            ov = os.path.join(work, name + ".overlay.qcow2")
            sh("qemu-img", "create", "-q", "-f", "qcow2", "-F", "raw", "-b", base, ov)
            disks.append({"file": ov, "format": "qcow2", "serial": name[:20]})
        else:
            disks.append({"file": base, "format": "raw", "serial": name[:20], "readonly": True})
    target = None
    if a.mode == "install-full":
        target = os.path.join(work, "target.overlay.qcow2")
        sh("qemu-img", "create", "-q", "-f", "qcow2", "-F", "raw", "-b",
           ensure_target(sdir), target)
    copy = None
    if a.iso_copy:
        # An exact copy of the medium on an "internal" disk, attached FIRST so
        # the kernel enumerates it before every other disk and before the
        # medium. It must not become the medium.
        copy = os.path.join(work, "iso-copy.overlay.qcow2")
        sh("qemu-img", "create", "-q", "-f", "qcow2", "-F", "raw", "-b", a.iso, copy)
        disks.insert(0, {"file": copy, "format": "qcow2", "serial": "M-ISO-COPY"})
        names.insert(0, None)
    if target:
        disks.append({"file": target, "format": "qcow2", "serial": TARGET_SERIAL})
        names.append(None)
    iso_digest, _ = sha256_file(a.iso)
    if a.media == "usb":
        # A stick larger than the image, as real ones are: the hybrid ISO at
        # the start and free space after it (casper may want that space). The
        # stick is a copy; the ISO file itself is only ever read.
        base = os.path.join(work, "stick.raw")
        sh("cp", "--sparse=always", a.iso, base)
        sh("truncate", "-s", str(os.path.getsize(a.iso) + a.stick_free_mib * MIB), base)
        os.chmod(base, 0o444)
        stick = os.path.join(work, "medium.overlay.qcow2")
        sh("qemu-img", "create", "-q", "-f", "qcow2", "-F", "raw", "-b", base, stick)
        medium = {"file": stick, "format": "qcow2"}
    else:
        medium = {"file": a.iso}
    vm = Machine(a.name, a.arch, work, memory=3072)
    vars_before = sha256_file(vm.vars)[0]
    invalid, how, facts = None, "", {}
    progress = []

    def checkpoints(label):
        progress.append({
            "at": label,
            "sentinels_written_bytes": sum(overlay_writes(d["file"]) for n, d in zip(names, disks)
                                           if n and d["format"] == "qcow2"),
            "target_written_bytes": overlay_writes(target) if target else None,
            "iso_copy_written_bytes": overlay_writes(copy) if copy else None,
        })
        log(f"{a.name}: {progress[-1]}")

    try:
        vm.start_bench(disks, medium, media=a.media, net=a.mode != "install-full")
        how, facts = drive(vm, a.mode, a.boot_budget, checkpoints)
        if how.startswith("INSTALL DID NOT"):
            invalid = None
        if how.startswith("INVALID"):
            invalid = how
    except Exception as e:  # bench failure, not a finding
        invalid = f"{type(e).__name__}: {e}"
    finally:
        vm.stop()
    serial = vm.text()
    kernel = sorted(set(line.strip()[:200] for line in serial.splitlines() if WRITE_RX.search(line)))
    out = {"name": a.name, "mode": a.mode, "attach": a.attach, "media": a.media, "traps": a.traps, "arch": a.arch,
           "iso": os.path.basename(a.iso), "iso_sha256": iso_digest, "how_stopped": how, "guest_facts": facts,
           "kernel_log_storage_lines": kernel, "sentinels": {}, "invalid": invalid}
    changed = []
    for name, d in zip(names, disks):
        if name is None:
            continue
        base = os.path.join(sdir, name + ".raw")
        item = {"what": {**SENTINELS, **TRAPS}[name][1], "before_sha256": before[name]["sha256"]}
        if a.attach == "overlay":
            flat = os.path.join(work, name + ".after.raw")
            sh("qemu-img", "convert", "-O", "raw", d["file"], flat)
            item["after_sha256"] = sha256_file(flat)[0]
            item["overlay_written_bytes"] = overlay_writes(d["file"])
            item["structure_before"] = before[name]["structure"]
            if item["after_sha256"] != item["before_sha256"] or item["overlay_written_bytes"]:
                item["structure_after"] = structure(flat)
            os.remove(flat)
            item["mutated"] = item["after_sha256"] != item["before_sha256"]
            item["written"] = item["overlay_written_bytes"] > 0
        else:
            item["after_sha256"] = sha256_file(base)[0]
            item["mutated"] = item["after_sha256"] != item["before_sha256"]
            item["written"] = False
        if item["mutated"] or item["written"]:
            changed.append(name)
        out["sentinels"][name] = item
    out["firmware_vars_changed"] = sha256_file(vm.vars)[0] != vars_before
    out["firmware_vars"] = firmware_vars(vm.vars)
    if a.media == "usb":
        out["medium_written_bytes"] = overlay_writes(medium["file"])
    out["iso_sha256_after"] = sha256_file(a.iso)[0]
    if a.media == "usb":
        os.chmod(os.path.join(work, "stick.raw"), 0o644)
        os.remove(os.path.join(work, "stick.raw"))
    out["changed"] = changed
    out["checkpoints"] = progress
    failures = list(changed)
    if out.get("medium_written_bytes"):
        failures.append("medium written")
    med = (facts.get("medium") or {}).get("decision") or {}
    out["medium"] = med
    if copy:
        out["iso_copy_written_bytes"] = overlay_writes(copy)
        if out["iso_copy_written_bytes"]:
            failures.append("iso copy written")
        # The copy is a SCSI disk; the medium is the CD drive or the USB stick
        # the firmware started from, and only the firmware's word tells them apart.
        if a.mode not in ("menu", "legacy"):
            name = med.get("name") or ""
            booted = name.startswith("sr") if a.media == "cdrom" else bool(name)
            if med.get("decision") != "MEDIUM" or med.get("binding") != "FIRMWARE_BOOT_ENTRY" or not booted:
                failures.append(f"medium is not bound to the booted device: {med}")
    if a.mode not in ("menu", "legacy") and a.expect_binding and med.get("binding") != a.expect_binding:
        failures.append(f"binding {med.get('binding')} != {a.expect_binding}")
    if target:
        out["target_written_bytes"] = overlay_writes(target)
        early = [c for c in progress if c["at"] != "installation finished"]
        if any(c["target_written_bytes"] or c["sentinels_written_bytes"] or c["iso_copy_written_bytes"] for c in early):
            failures.append("a disk was written before the confirmation was accepted")
        if len(early) != 4:
            failures.append(f"only {len(early)} of 4 pre-confirmation checkpoints were taken")
        if not facts.get("installed"):
            failures.append("installation did not complete")
        elif not out["target_written_bytes"]:
            failures.append("installed, but the target shows no writes")
        else:
            ident = installed_root_identity(target)
            out["installed_root_identity"] = ident
            if not ident.get("ok"):
                failures.append("installed root identity mismatch")
    out["failures"] = failures
    status = "INVALID" if invalid else ("FAIL" if failures else "PASS")
    out["status"] = status
    with open(os.path.join(work, "result.json"), "w") as f:
        json.dump(out, f, indent=1, sort_keys=True)
    print(f"RESULT {a.name} {status} failures={failures} medium={med.get('name')}/{med.get('binding')} "
          f"invalid={invalid}", flush=True)
    return {"PASS": 0, "FAIL": 1, "INVALID": 2}[status]


def main():
    p = argparse.ArgumentParser()
    sub = p.add_subparsers(dest="cmd", required=True)
    s = sub.add_parser("sentinels")
    s.add_argument("--work", required=True)
    r = sub.add_parser("run")
    r.add_argument("--iso", required=True)
    r.add_argument("--arch", required=True, choices=["amd64", "arm64"])
    r.add_argument("--work", required=True)
    r.add_argument("--name", required=True)
    r.add_argument("--mode", required=True, choices=["legacy"] + list(MENU_KEYS))
    r.add_argument("--attach", default="overlay", choices=["overlay", "readonly"])
    r.add_argument("--media", default="cdrom", choices=["cdrom", "usb"])
    r.add_argument("--traps", action="store_true")
    r.add_argument("--stick-free-mib", type=int, default=0)
    r.add_argument("--iso-copy", action="store_true")
    r.add_argument("--expect-binding", default="")
    r.add_argument("--boot-budget", type=int, default=2400)
    a = p.parse_args()
    os.makedirs(a.work, exist_ok=True)
    if a.cmd == "sentinels":
        build_sentinels(a.work)
        return 0
    try:
        return run(a)
    finally:
        from image_e2e import RUNNING
        for vm in RUNNING:
            vm.stop()


if __name__ == "__main__":
    sys.exit(main())
