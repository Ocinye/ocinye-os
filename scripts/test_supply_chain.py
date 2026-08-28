#!/usr/bin/env python3
"""Tests for the two supply-chain policy evaluators.

These run against fixtures, never against the live databases. A gate whose only
proof is "it went green against today's advisories" proves nothing about the day
an advisory appears: the interesting cases — a moderate runtime finding, an
alert that was dismissed rather than fixed, an API that returned an error — are
precisely the ones that cannot be summoned on demand.

So the fetching halves are not exercised here. What is exercised is every
decision made after the answer arrives.
"""

import json
import os
import sys
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

import advisory_gate
import dependabot_posture


class Lockfile(unittest.TestCase):
    def test_reads_name_and_version_pairs(self):
        packages = advisory_gate.parse_lock(
            '# generated\nversion = 4\n\n'
            '[[package]]\nname = "jsonwebtoken"\nversion = "10.3.0"\n'
            'source = "registry+https://github.com/rust-lang/crates.io-index"\n\n'
            '[[package]]\nname = "serde"\nversion = "1.0.228"\n'
            'dependencies = [\n "serde_core",\n]\n'
        )
        self.assertEqual(
            packages, [("jsonwebtoken", "10.3.0"), ("serde", "1.0.228")]
        )

    def test_the_lockfile_format_version_is_not_a_package(self):
        # `version = 4` sits at the top of every lockfile, before any package.
        self.assertEqual(advisory_gate.parse_lock("version = 4\n"), [])

    def test_the_real_lockfile_parses(self):
        root = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")
        with open(os.path.join(root, "Cargo.lock"), encoding="utf-8") as handle:
            packages = advisory_gate.parse_lock(handle.read())
        self.assertGreater(len(packages), 100)
        self.assertIn("jsonwebtoken", [name for name, _ in packages])


class Versions(unittest.TestCase):
    def test_ordering(self):
        self.assertLess(
            advisory_gate.parse_version("9.3.1"), advisory_gate.parse_version("10.3.0")
        )
        self.assertLess(
            advisory_gate.parse_version("1.9.0"), advisory_gate.parse_version("1.10.0")
        )

    def test_a_pre_release_is_below_its_release(self):
        self.assertLess(
            advisory_gate.parse_version("1.0.0-rc.1"),
            advisory_gate.parse_version("1.0.0"),
        )

    def test_build_metadata_is_ignored(self):
        self.assertEqual(
            advisory_gate.parse_version("1.0.0+build.7"),
            advisory_gate.parse_version("1.0.0"),
        )

    def test_open_upper_bound(self):
        self.assertTrue(advisory_gate.in_range("9.3.1", "< 10.3.0"))
        self.assertFalse(advisory_gate.in_range("10.3.0", "< 10.3.0"))
        self.assertFalse(advisory_gate.in_range("10.4.0", "< 10.3.0"))

    def test_closed_upper_bound(self):
        self.assertTrue(advisory_gate.in_range("0.9.6", "<= 0.9.6"))
        self.assertFalse(advisory_gate.in_range("0.9.7", "<= 0.9.6"))

    def test_a_conjunction_needs_every_part(self):
        self.assertTrue(advisory_gate.in_range("1.1.0", ">= 1.0.0, < 1.2.3"))
        self.assertFalse(advisory_gate.in_range("0.9.0", ">= 1.0.0, < 1.2.3"))
        self.assertFalse(advisory_gate.in_range("1.2.3", ">= 1.0.0, < 1.2.3"))

    def test_an_exact_range(self):
        self.assertTrue(advisory_gate.in_range("1.2.3", "= 1.2.3"))
        self.assertFalse(advisory_gate.in_range("1.2.4", "= 1.2.3"))

    def test_an_unreadable_range_is_refused_rather_than_ignored(self):
        with self.assertRaises(ValueError):
            advisory_gate.in_range("1.0.0", "sometimes")


THE_CVE = {
    "package": "jsonwebtoken",
    "ghsa": "GHSA-h395-gr6q-cpjc",
    "cve": "CVE-2026-25537",
    "severity": "MODERATE",
    "range": "< 10.3.0",
    "patched": "10.3.0",
}


class AdvisoryPolicy(unittest.TestCase):
    def test_the_vulnerable_version_is_caught(self):
        findings = advisory_gate.evaluate([("jsonwebtoken", "9.3.1")], [THE_CVE])
        self.assertEqual(len(findings), 1)
        self.assertEqual(findings[0]["ghsa"], "GHSA-h395-gr6q-cpjc")
        self.assertEqual(findings[0]["cve"], "CVE-2026-25537")

    def test_the_patched_version_is_not(self):
        # The positive control for the test above: same advisory, same policy,
        # only the version differs.
        self.assertEqual(
            advisory_gate.evaluate([("jsonwebtoken", "10.3.0")], [THE_CVE]), []
        )

    def test_an_advisory_for_another_crate_does_not_match_by_version_alone(self):
        self.assertEqual(advisory_gate.evaluate([("serde", "9.3.1")], [THE_CVE]), [])

    def test_a_low_finding_is_below_the_floor(self):
        low = dict(THE_CVE, severity="LOW")
        self.assertEqual(advisory_gate.evaluate([("jsonwebtoken", "9.3.1")], [low]), [])
        # ...but the same finding is caught when the floor is lowered, which is
        # what proves the version matching was never the reason.
        self.assertEqual(
            len(advisory_gate.evaluate([("jsonwebtoken", "9.3.1")], [low], "low")), 1
        )

    def test_high_and_critical_are_above_a_moderate_floor(self):
        for severity in ("HIGH", "CRITICAL"):
            finding = dict(THE_CVE, severity=severity)
            self.assertEqual(
                len(advisory_gate.evaluate([("jsonwebtoken", "9.3.1")], [finding])),
                1,
                severity,
            )

    def test_an_unknown_severity_is_refused_rather_than_skipped(self):
        with self.assertRaises(ValueError):
            advisory_gate.evaluate(
                [("jsonwebtoken", "9.3.1")], [dict(THE_CVE, severity="spicy")]
            )

    def test_an_error_payload_is_not_read_as_an_empty_list(self):
        path = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")
        del path
        with self.assertRaises(advisory_gate.Unavailable):
            advisory_gate._load_advisories(_write_temp(json.dumps({"message": "Bad credentials"})))


def _write_temp(text):
    import tempfile

    handle = tempfile.NamedTemporaryFile("w", suffix=".json", delete=False)
    handle.write(text)
    handle.close()
    return handle.name


def alert(number=1, state="open", severity="medium", scope="runtime", package="jsonwebtoken"):
    return {
        "number": number,
        "state": state,
        "dependency": {"scope": scope, "package": {"name": package}},
        "security_advisory": {
            "severity": severity,
            "ghsa_id": "GHSA-h395-gr6q-cpjc",
            "summary": "Type confusion",
        },
    }


class PosturePolicy(unittest.TestCase):
    def test_no_alerts_is_clean(self):
        self.assertEqual(dependabot_posture.evaluate([]), [])

    def test_an_open_moderate_runtime_alert_fails(self):
        findings = dependabot_posture.evaluate([alert()])
        self.assertEqual(len(findings), 1)
        self.assertEqual(findings[0]["number"], 1)

    def test_the_same_alert_once_fixed_does_not(self):
        # This is the shape alert #1 must take after the fix reaches main.
        self.assertEqual(dependabot_posture.evaluate([alert(state="fixed")]), [])

    def test_a_dismissed_alert_does_not_fail_the_gate(self):
        # Dismissal is a person's decision and the gate honours it. That it is
        # honoured here is exactly why dismissing this milestone's own alert
        # would have been dishonest.
        self.assertEqual(dependabot_posture.evaluate([alert(state="dismissed")]), [])
        self.assertEqual(
            dependabot_posture.evaluate([alert(state="auto_dismissed")]), []
        )

    def test_a_low_alert_is_below_the_floor(self):
        self.assertEqual(dependabot_posture.evaluate([alert(severity="low")]), [])
        self.assertEqual(
            len(dependabot_posture.evaluate([alert(severity="low")], "low")), 1
        )

    def test_high_and_critical_fail(self):
        for severity in ("high", "critical"):
            self.assertEqual(
                len(dependabot_posture.evaluate([alert(severity=severity)])),
                1,
                severity,
            )

    def test_a_development_dependency_is_judged_separately(self):
        self.assertEqual(dependabot_posture.evaluate([alert(scope="development")]), [])

    def test_an_unstated_scope_is_treated_as_runtime(self):
        # Letting a finding through because GitHub did not say where it lives
        # would be the wrong way to be wrong.
        self.assertEqual(len(dependabot_posture.evaluate([alert(scope=None)])), 1)

    def test_findings_come_worst_first(self):
        findings = dependabot_posture.evaluate(
            [alert(number=1, severity="medium"), alert(number=2, severity="critical")]
        )
        self.assertEqual([f["number"] for f in findings], [2, 1])

    def test_an_unknown_severity_is_refused_rather_than_skipped(self):
        with self.assertRaises(ValueError):
            dependabot_posture.evaluate([alert(severity="spicy")])

    def test_a_failed_query_is_never_read_as_zero_alerts(self):
        # GitHub answers a permission failure with an object, not a list. If
        # that object were iterated as if it were a list of alerts, the gate
        # would report a clean repository at exactly the moment it cannot see.
        with self.assertRaises(dependabot_posture.Unavailable):
            dependabot_posture.evaluate({"message": "Resource not accessible"})
        with self.assertRaises(dependabot_posture.Unavailable):
            dependabot_posture.evaluate(None)


if __name__ == "__main__":
    unittest.main(verbosity=2)
