#!/usr/bin/env python3
"""The security posture of the canonical branch.

# The question this answers

`advisory_gate.py` asks whether the lockfile in front of it contains a known
vulnerable version. This asks something different and complementary: does GitHub
itself still consider this repository to have an open finding?

They are not the same question. GitHub knows things the lockfile does not — an
advisory published this morning against a version that has been in `main` for a
month raises an alert without a single line of the repository changing.

# Why this is not a pull-request gate

A rule of the form "fail while any alert is open" would fail the very branch that
fixes the alert, because the alert stays open until the fix reaches the default
branch. So this belongs to `main` and to the schedule, and never to the pull
request. Pull requests are judged on what they change; `main` is judged on what
it still carries.

# Failing to ask is not an answer

If GitHub cannot be consulted — no permission, an outage, a rate limit — this
reports `NAO VERIFICADO` and exits distinctly. A security telemetry failure that
turns into an empty list of findings is how a repository comes to believe it is
clean. See `evaluate`, which refuses any payload that is not a list of alerts.
"""

import argparse
import json
import subprocess
import sys

SEVERITY = {"low": 1, "moderate": 2, "medium": 2, "high": 3, "critical": 4}

# Anything else — `fixed`, `dismissed`, `auto_dismissed` — is not an open
# finding. `fixed` is the only one this milestone will accept for its own CVE.
OPEN = "open"

UNKNOWN = 2


class Unavailable(Exception):
    """GitHub could not be consulted. Not the same as a clean result."""


def evaluate(alerts, minimum="moderate", scopes=("runtime",)):
    """The alerts that this policy refuses to live with.

    Pure. `alerts` is what the REST API returns, verbatim.
    """
    if not isinstance(alerts, list):
        raise Unavailable("the alert payload is not a list of alerts")
    if minimum.lower() not in SEVERITY:
        raise ValueError("unknown severity: " + minimum)
    floor = SEVERITY[minimum.lower()]

    findings = []
    for alert in alerts:
        if alert.get("state") != OPEN:
            continue
        advisory = alert.get("security_advisory") or {}
        raw = (advisory.get("severity") or "").lower()
        severity = SEVERITY.get(raw)
        if severity is None:
            raise ValueError("unknown severity: " + repr(advisory.get("severity")))
        if severity < floor:
            continue
        dependency = alert.get("dependency") or {}
        scope = dependency.get("scope")
        # An unknown scope is treated as runtime: the conservative reading is
        # the one that does not let a finding through by omission.
        if scopes and scope is not None and scope not in scopes:
            continue
        findings.append(
            {
                "number": alert.get("number"),
                "package": (dependency.get("package") or {}).get("name"),
                "scope": scope,
                "severity": raw,
                "ghsa": advisory.get("ghsa_id"),
                "summary": advisory.get("summary"),
            }
        )
    findings.sort(key=lambda f: (-SEVERITY[f["severity"]], f["number"] or 0))
    return findings


def fetch(repository):
    """Every Dependabot alert GitHub holds for this repository."""
    result = subprocess.run(
        [
            "gh",
            "api",
            "--paginate",
            "repos/%s/dependabot/alerts?per_page=100&state=open" % repository,
        ],
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        raise Unavailable("gh api failed: " + result.stderr.strip()[:400])
    try:
        payload = json.loads(result.stdout)
    except json.JSONDecodeError as error:
        raise Unavailable("GitHub returned something that is not JSON: %s" % error)
    if not isinstance(payload, list):
        raise Unavailable("GitHub returned %s, not a list of alerts" % type(payload).__name__)
    return payload


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--repository", default="Ocinye/ocinye-os")
    parser.add_argument("--alerts", help="read this file instead of asking GitHub")
    parser.add_argument("--minimum", default="moderate")
    arguments = parser.parse_args(argv)

    try:
        if arguments.alerts:
            with open(arguments.alerts, encoding="utf-8") as handle:
                alerts = json.load(handle)
            source = arguments.alerts
        else:
            alerts = fetch(arguments.repository)
            source = "GitHub"
        findings = evaluate(alerts, arguments.minimum)
    except Unavailable as error:
        print("NAO VERIFICADO: %s" % error, file=sys.stderr)
        print(
            "Os alertas do Dependabot nao puderam ser lidos. Isto nao é o mesmo "
            "que zero alertas, e por isso nao conta como verde.",
            file=sys.stderr,
        )
        return UNKNOWN

    if not findings:
        print(
            "%s nao tem alertas Dependabot abertos de severidade %s ou acima em "
            "dependências de runtime (fonte: %s)."
            % (arguments.repository, arguments.minimum, source)
        )
        return 0

    for finding in findings:
        print(
            "%s #%s %s (%s)\n    %s"
            % (
                finding["severity"].upper(),
                finding["number"],
                finding["package"],
                finding["ghsa"],
                finding["summary"] or "",
            ),
            file=sys.stderr,
        )
    print(
        "\n%d alerta(s) Dependabot abertos no branch canónico." % len(findings),
        file=sys.stderr,
    )
    return 1


if __name__ == "__main__":
    sys.exit(main())
