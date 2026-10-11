#!/usr/bin/env python3
"""Tests for the ISO E2E failure-evidence machinery and the language detector.

These run against the harness functions directly, never against a real QEMU
guest or a real nbd device: the shell-outs are injected so the read-only flag,
the guaranteed qemu-nbd disconnect, and the PASS/FAIL independence can be proven
without root. The one thing a real run would add — that curtin actually wrote
the disk — is exactly what the preservation exists to keep for later, and is not
something a unit test can summon.
"""

import json
import os
import re
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

import image_e2e as m


class Ran:
    """A recording stand-in for a CompletedProcess."""

    def __init__(self, returncode=0, stdout=b"", stderr=b""):
        self.returncode, self.stdout, self.stderr = returncode, stdout, stderr


class Recorder:
    """Records every shell-out and answers them, so a test can assert on the
    exact commands the collector issued."""

    def __init__(self, fail_on=None):
        self.calls = []
        self.fail_on = fail_on or ()

    def __call__(self, args, check=True):
        self.calls.append(list(args))
        if args and args[0] in self.fail_on:
            raise RuntimeError(f"injected failure for {args[0]}")
        return Ran()

    def with_tool(self, tool):
        return [c for c in self.calls if c and c[0] == tool]


class LanguageDetector(unittest.TestCase):
    # E, F, G: a success in any product language is a success; the error marker
    # is not a success.
    def test_portuguese_base_instalada_is_installed(self):
        self.assertTrue(m.base_installed("... A instalar ...\nBase instalada\nSeguinte"))

    def test_english_base_installed_is_installed(self):
        self.assertTrue(m.base_installed("... Installing ...\nBase installed\nNext"))

    def test_french_base_installee_is_installed(self):
        self.assertTrue(m.base_installed("... Installation ...\nBase installée\nSuivant"))

    def test_stopped_is_not_installed(self):
        self.assertFalse(m.base_installed("media_check OK\nA instalacao parou\nCodigo: X"))
        self.assertFalse(m.base_installed("Installation stopped"))

    def test_expect_pattern_waits_for_every_marker(self):
        # The console pattern must match the exact strings base_installed reads,
        # or expect() would time out on an English/French success.
        for marker in m.BASE_DONE:
            self.assertIsNotNone(re.search(m.DONE_OR_FAIL_RX, f"x {marker} y"), marker)
        self.assertIsNotNone(re.search(m.DONE_OR_FAIL_RX, "A instalação parou"))


class CurtinSubreason(unittest.TestCase):
    # Section 8: the preserved log is classified into a bounded token, and a
    # missing offline package is not hidden behind a bootloader failure.
    def test_missing_package_detected_first(self):
        self.assertEqual(
            m.curtin_subreason("e: package 'grub-efi-amd64' has no installation candidate"),
            "missing_offline_package",
        )
        self.assertEqual(
            m.curtin_subreason("e: unable to locate package grub-efi-amd64"),
            "missing_offline_package",
        )

    def test_real_bootloader_failures_stay_distinct(self):
        self.assertEqual(m.curtin_subreason("running command grub-install failed"), "grub_install")
        self.assertEqual(m.curtin_subreason("efibootmgr: could not write entry"), "efibootmgr")
        self.assertEqual(m.curtin_subreason("shim-signed postinst error"), "shim")
        self.assertEqual(m.curtin_subreason("disk write error"), "other")


class RootIdentity(unittest.TestCase):
    # The force-partuuid defect and its guard: the installed boot config must
    # resolve root to the actual target, not a build-time identity.
    TPU = "f62eda4e-48ab-45da-b7a9-f18c15f667b6"
    TFU = "e42c50f0-ca81-4d09-9c6f-f3a031c6c0c0"

    def test_grub_root_refs_reads_linux_lines(self):
        cfg = (
            "menuentry 'x' {\n"
            "  search --no-floppy --fs-uuid --set=root e42c50f0-ca81-4d09-9c6f-f3a031c6c0c0\n"
            "  linux /boot/vmlinuz root=PARTUUID=bd907978-e121-479a-9598-288628fcfd21 ro console=ttyS0\n"
            "}\n"
        )
        self.assertEqual(
            m.grub_root_refs(cfg),
            [("PARTUUID", "bd907978-e121-479a-9598-288628fcfd21")],
        )

    def test_stale_forced_partuuid_is_a_typed_mismatch(self):
        # The exact shipped defect: a forced cloud-image PARTUUID.
        refs = [("PARTUUID", "bd907978-e121-479a-9598-288628fcfd21")]
        ok, detail = m.evaluate_root_identity(refs, ["40-force-partuuid.cfg"], self.TPU, self.TFU)
        self.assertFalse(ok)
        self.assertIn("INSTALLED_ROOT_IDENTITY_MISMATCH", detail)
        self.assertIn("GRUB_FORCE_PARTUUID", detail)

    def test_wrong_partuuid_without_force_is_still_a_mismatch(self):
        refs = [("PARTUUID", "bd907978-e121-479a-9598-288628fcfd21")]
        ok, detail = m.evaluate_root_identity(refs, [], self.TPU, self.TFU)
        self.assertFalse(ok)
        self.assertIn("INSTALLED_ROOT_IDENTITY_MISMATCH", detail)

    def test_root_resolving_to_the_target_fs_uuid_is_ok(self):
        refs = [("UUID", self.TFU)]
        ok, _ = m.evaluate_root_identity(refs, [], self.TPU, self.TFU)
        self.assertTrue(ok)

    def test_root_resolving_to_the_target_partuuid_is_ok(self):
        refs = [("PARTUUID", self.TPU)]
        ok, _ = m.evaluate_root_identity(refs, [], self.TPU, self.TFU)
        self.assertTrue(ok)

    def test_no_root_reference_is_unknown_not_pass(self):
        ok, detail = m.evaluate_root_identity([], [], self.TPU, self.TFU)
        self.assertFalse(ok)
        self.assertIn("INSTALLED_ROOT_IDENTITY_UNKNOWN", detail)

    def test_empty_target_identity_never_passes(self):
        # A blank target identity must not vacuously match a blank ref.
        ok, _ = m.evaluate_root_identity([("UUID", "")], [], "", "")
        self.assertFalse(ok)


class StatusPredicate(unittest.TestCase):
    # B, H: the verdict is a pure function of the checks and the invalid reason.
    def test_pass_only_when_every_check_passed(self):
        self.assertEqual(m.result_status([{"ok": True}, {"ok": True}], None), "PASS")

    def test_any_failed_check_is_fail(self):
        self.assertEqual(m.result_status([{"ok": True}, {"ok": False}], None), "FAIL")

    def test_no_checks_is_fail_not_pass(self):
        self.assertEqual(m.result_status([], None), "FAIL")

    def test_invalid_overrides_everything(self):
        self.assertEqual(m.result_status([{"ok": True}], "RuntimeError: boom"), "INVALID")


class NbdLifecycle(unittest.TestCase):
    # C, D: attach is read-only and disconnect always happens.
    def test_attach_is_read_only(self):
        rec = Recorder()
        with m.NbdAttach("/x/target.qcow2", run=rec, pick_dev=lambda: "/dev/nbd3"):
            pass
        attach = [c for c in rec.with_tool("qemu-nbd") if "-c" in c]
        self.assertEqual(len(attach), 1)
        self.assertIn("-r", attach[0], "qemu-nbd attach must be read-only")

    def test_disconnect_runs_even_when_body_raises(self):
        rec = Recorder()
        with self.assertRaises(ValueError):
            with m.NbdAttach("/x/target.qcow2", run=rec, pick_dev=lambda: "/dev/nbd3"):
                raise ValueError("collector blew up mid-flight")
        disconnect = [c for c in rec.with_tool("qemu-nbd") if "-d" in c]
        self.assertEqual(disconnect, [["qemu-nbd", "-d", "/dev/nbd3"]])

    def test_no_free_device_raises_before_attaching(self):
        rec = Recorder()
        with self.assertRaises(RuntimeError):
            with m.NbdAttach("/x/target.qcow2", run=rec, pick_dev=lambda: None):
                pass
        self.assertEqual([c for c in rec.with_tool("qemu-nbd") if "-c" in c], [])

    def test_mount_is_read_only_and_unmounts_on_raise(self):
        rec = Recorder()
        with tempfile.TemporaryDirectory() as d:
            mnt = os.path.join(d, "m")
            with self.assertRaises(ValueError):
                with m.RoMount("/dev/nbd3p2", mnt, run=rec):
                    raise ValueError("x")
        mounts = rec.with_tool("mount")
        self.assertEqual(len(mounts), 1)
        self.assertEqual(mounts[0][:3], ["mount", "-o", "ro"], "every mount must be read-only")
        self.assertEqual(rec.with_tool("umount"), [["umount", mnt]])


class CollectReadOnly(unittest.TestCase):
    # C: the whole collection never issues a writing command.
    WRITERS = {"mkfs", "mkfs.ext4", "mkfs.vfat", "fsck", "e2fsck", "dd", "parted",
               "sfdisk", "sgdisk", "wipefs"}

    def test_collection_issues_no_writing_command(self):
        rec = Recorder()
        with tempfile.TemporaryDirectory() as diag:
            # sfdisk -d is a read-only dump, but prove nothing *writes*: the
            # dangerous forms (sfdisk <dev> with a script) never appear.
            m.collect_target_diagnostics("/x/target.qcow2", diag, run=rec,
                                         pick_dev=lambda: "/dev/nbd3")
        for c in rec.calls:
            if not c:
                continue
            if c[0] == "mount":
                self.assertIn("ro", c, c)
            if c[0] == "sfdisk":
                self.assertEqual(c, ["sfdisk", "-d", "/dev/nbd3"], "only the read-only dump")
            self.assertNotIn(c[0], self.WRITERS - {"sfdisk"}, f"writing command issued: {c}")
        # The disk was attached read-only and disconnected.
        self.assertTrue(any("-r" in c for c in rec.with_tool("qemu-nbd") if "-c" in c))
        self.assertTrue(any("-d" in c for c in rec.with_tool("qemu-nbd")))


class Preservation(unittest.TestCase):
    # A, B, H.
    def test_failed_run_preserves_disk_and_collects(self):
        with tempfile.TemporaryDirectory() as work:
            target = os.path.join(work, "target.qcow2")
            Path(target).write_bytes(b"the disk curtin wrote")
            seen = {}

            def collector(disk, diag):
                seen["disk"], seen["diag"] = disk, diag
                Path(diag, "curtin-install.log").write_text("evidence")
                return ["curtin-install.log"]

            failure = m.preserve_iso_failure(work, target, collector=collector)
            self.assertIsNotNone(failure)
            preserved = os.path.join(failure, "target.qcow2")
            self.assertTrue(os.path.isfile(preserved), "the disk is kept under failure/")
            self.assertFalse(os.path.exists(target), "and moved, not left in place")
            self.assertEqual(Path(preserved).read_bytes(), b"the disk curtin wrote")
            # The collector saw the preserved disk, not the vanished original.
            self.assertEqual(seen["disk"], preserved)
            summary = json.loads(Path(failure, "diagnostics", "COLLECTION.json").read_text())
            self.assertEqual(summary["preserved_disk"], preserved)
            self.assertEqual(summary["diagnostics"], ["curtin-install.log"])
            self.assertIsNone(summary["collector_error"])

    def test_evidence_never_overwritten_silently(self):
        with tempfile.TemporaryDirectory() as work:
            t1 = os.path.join(work, "target.qcow2")
            Path(t1).write_bytes(b"first")
            f1 = m.preserve_iso_failure(work, t1, collector=lambda d, g: [])
            Path(t1).write_bytes(b"second")
            f2 = m.preserve_iso_failure(work, t1, collector=lambda d, g: [])
            self.assertNotEqual(f1, f2, "a second failure gets its own directory")
            self.assertEqual(Path(f1, "target.qcow2").read_bytes(), b"first")
            self.assertEqual(Path(f2, "target.qcow2").read_bytes(), b"second")

    def test_collector_failure_does_not_propagate(self):
        # H: preservation is best-effort. A collector that raises must not turn a
        # FAIL into a crash or change the verdict.
        with tempfile.TemporaryDirectory() as work:
            target = os.path.join(work, "target.qcow2")
            Path(target).write_bytes(b"disk")

            def boom(disk, diag):
                raise RuntimeError("nbd not available in CI")

            failure = m.preserve_iso_failure(work, target, collector=boom)
            self.assertIsNotNone(failure)
            summary = json.loads(Path(failure, "diagnostics", "COLLECTION.json").read_text())
            self.assertIn("nbd not available", summary["collector_error"])
            # The disk was still preserved before the collector ran.
            self.assertTrue(os.path.isfile(os.path.join(failure, "target.qcow2")))

    def test_missing_disk_is_recorded_not_fatal(self):
        with tempfile.TemporaryDirectory() as work:
            failure = m.preserve_iso_failure(work, os.path.join(work, "nope.qcow2"),
                                             collector=lambda d, g: [])
            self.assertIsNotNone(failure)
            summary = json.loads(Path(failure, "diagnostics", "COLLECTION.json").read_text())
            self.assertIsNone(summary["preserved_disk"])


if __name__ == "__main__":
    unittest.main()
