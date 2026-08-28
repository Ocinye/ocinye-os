#!/usr/bin/env python3
"""The GitHub Advisory Database gate.

# Why this exists next to `cargo audit`

`cargo audit` reads the RustSec advisory database. Dependabot reads the GitHub
Advisory Database. They are different collections, and an advisory can live in
one for a long time before it appears in the other — or never appear at all.

`GHSA-h395-gr6q-cpjc` (`CVE-2026-25537`, `jsonwebtoken`) was exactly that: the
GitHub alert was open on this repository while `cargo audit` reported zero
vulnerabilities. Both tools were telling the truth about the universe they can
see. What was wrong was the conclusion drawn from a single green tick.

So this asks the *other* database the same question, against the same lockfile,
and reports under its own name. Nothing here is a copy of an advisory database:
the answer always comes from GitHub at the moment of the query.

# Fetching and judging are separate on purpose

`fetch` talks to the network and writes what GitHub said. `evaluate` is pure and
takes that file. Only the second half decides whether the build fails, which is
what makes the policy testable against fixtures instead of against whatever
vulnerabilities happen to exist today.
"""

import argparse
import json
import re
import subprocess
import sys

# GitHub reports severity two ways: `LOW|MODERATE|HIGH|CRITICAL` on the GraphQL
# advisory API, `low|medium|high|critical` on the REST alerts API. Same ladder,
# different words for the middle rung.
SEVERITY = {
    "low": 1,
    "moderate": 2,
    "medium": 2,
    "high": 3,
    "critical": 4,
}

UNKNOWN = 2  # exit code that means "could not verify", never "nothing found"


class Unavailable(Exception):
    """GitHub could not be consulted. This is not the same as a clean result."""


# ── Lockfile ────────────────────────────────────────────────────────────────


def parse_lock(text):
    """Every (name, version) in a `Cargo.lock`.

    Deliberately a small regex reader rather than a TOML parse: the lockfile
    grammar for this is three fixed lines, and this way the gate runs on any
    Python the runner happens to ship.
    """
    packages = []
    name = None
    for line in text.splitlines():
        line = line.strip()
        if line == "[[package]]":
            name = None
        elif line.startswith("name = "):
            name = line[len("name = ") :].strip().strip('"')
        elif line.startswith("version = ") and name is not None:
            version = line[len("version = ") :].strip().strip('"')
            packages.append((name, version))
            name = None
    return packages


# ── Versions ────────────────────────────────────────────────────────────────


def parse_version(text):
    """A comparable key for a semantic version.

    A release outranks any pre-release of the same numbers, which is what makes
    `1.0.0-rc.1 < 1.0.0` come out right. Build metadata is ignored, as semver
    says it must be.
    """
    text = text.strip().split("+", 1)[0]
    core, _, pre = text.partition("-")
    numbers = []
    for part in core.split("."):
        if not part.isdigit():
            # Refused rather than coerced to zero. A range this reader cannot
            # understand must stop the gate, because the alternative is an
            # advisory that quietly matches nothing.
            raise ValueError("unreadable version: " + text)
        numbers.append(int(part))
    while len(numbers) < 3:
        numbers.append(0)

    if not pre:
        # 1 sorts above the 0 given to pre-releases below.
        return (tuple(numbers[:3]), 1, ())

    identifiers = []
    for part in pre.split("."):
        if part.isdigit():
            identifiers.append((0, int(part), ""))
        else:
            identifiers.append((1, 0, part))
    return (tuple(numbers[:3]), 0, tuple(identifiers))


def satisfies(version, constraint):
    """Whether `version` satisfies one `<op> <version>` constraint."""
    match = re.match(r"^\s*(<=|>=|<|>|=|==)?\s*(\S+)\s*$", constraint)
    if not match:
        raise ValueError("unreadable version constraint: " + constraint)
    operator = match.group(1) or "="
    left, right = parse_version(version), parse_version(match.group(2))
    if operator in ("=", "=="):
        return left == right
    if operator == "<":
        return left < right
    if operator == "<=":
        return left <= right
    if operator == ">":
        return left > right
    return left >= right


def in_range(version, vulnerable_range):
    """Whether a version falls in a GitHub `vulnerableVersionRange`.

    The ranges GitHub publishes are conjunctions: `< 10.3.0`, or
    `>= 1.0.0, < 1.2.3`. Every part has to hold.
    """
    parts = [part for part in vulnerable_range.split(",") if part.strip()]
    if not parts:
        raise ValueError("empty version range")
    return all(satisfies(version, part) for part in parts)


# ── Policy ──────────────────────────────────────────────────────────────────


def evaluate(packages, advisories, minimum="moderate"):
    """Which locked packages are hit by which advisories.

    `packages` is what `parse_lock` returned; `advisories` is what `fetch`
    wrote. Pure: no network, no clock, no environment.
    """
    if minimum.lower() not in SEVERITY:
        raise ValueError("unknown severity: " + minimum)
    floor = SEVERITY[minimum.lower()]

    by_package = {}
    for advisory in advisories:
        by_package.setdefault(advisory["package"], []).append(advisory)

    findings = []
    for name, version in packages:
        for advisory in by_package.get(name, []):
            severity = SEVERITY.get(advisory["severity"].lower())
            if severity is None:
                raise ValueError("unknown severity: " + advisory["severity"])
            if severity < floor:
                continue
            if not in_range(version, advisory["range"]):
                continue
            findings.append(
                {
                    "package": name,
                    "version": version,
                    "ghsa": advisory["ghsa"],
                    "cve": advisory.get("cve"),
                    "severity": advisory["severity"].lower(),
                    "range": advisory["range"],
                    "patched": advisory.get("patched"),
                }
            )
    findings.sort(key=lambda f: (-SEVERITY[f["severity"]], f["package"], f["ghsa"]))
    return findings


# ── Fetching ────────────────────────────────────────────────────────────────

QUERY_BATCH = 40


def _graphql(query):
    result = subprocess.run(
        ["gh", "api", "graphql", "-f", "query=" + query],
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        raise Unavailable("gh api graphql failed: " + result.stderr.strip()[:400])
    try:
        payload = json.loads(result.stdout)
    except json.JSONDecodeError as error:
        raise Unavailable("GitHub returned something that is not JSON: %s" % error)
    if payload.get("errors"):
        raise Unavailable("GitHub returned errors: " + json.dumps(payload["errors"])[:400])
    if "data" not in payload:
        raise Unavailable("GitHub returned no data")
    return payload["data"]


def fetch(names):
    """Ask the GitHub Advisory Database about these crates.

    Batched with GraphQL aliases: a lockfile holds several hundred crates, and
    one request each would be both slow and rude to the rate limit.
    """
    advisories = []
    ordered = sorted(set(names))
    for start in range(0, len(ordered), QUERY_BATCH):
        batch = ordered[start : start + QUERY_BATCH]
        fields = "\n".join(
            'a%d: securityVulnerabilities(ecosystem: RUST, package: "%s", first: 100) '
            "{ nodes { advisory { ghsaId identifiers { type value } severity withdrawnAt } "
            "vulnerableVersionRange firstPatchedVersion { identifier } } }" % (index, name)
            for index, name in enumerate(batch)
        )
        data = _graphql("{ %s }" % fields)
        for index, name in enumerate(batch):
            node = data.get("a%d" % index)
            if node is None:
                raise Unavailable("no answer for crate " + name)
            for entry in node["nodes"]:
                advisory = entry["advisory"]
                if advisory.get("withdrawnAt"):
                    continue
                cve = next(
                    (
                        identifier["value"]
                        for identifier in advisory["identifiers"]
                        if identifier["type"] == "CVE"
                    ),
                    None,
                )
                patched = entry.get("firstPatchedVersion")
                advisories.append(
                    {
                        "package": name,
                        "ghsa": advisory["ghsaId"],
                        "cve": cve,
                        "severity": advisory["severity"],
                        "range": entry["vulnerableVersionRange"],
                        "patched": patched["identifier"] if patched else None,
                    }
                )
    return advisories


# ── Command line ────────────────────────────────────────────────────────────


def _load_advisories(path):
    with open(path, encoding="utf-8") as handle:
        payload = json.load(handle)
    if not isinstance(payload, list):
        # A failed fetch that produced an error object must never be read as an
        # empty list of problems.
        raise Unavailable("advisory file is not a list of advisories")
    return payload


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--lock", default="Cargo.lock")
    parser.add_argument("--advisories", help="read this file instead of asking GitHub")
    parser.add_argument("--write", help="save what GitHub said to this file")
    parser.add_argument("--minimum", default="moderate")
    arguments = parser.parse_args(argv)

    with open(arguments.lock, encoding="utf-8") as handle:
        packages = parse_lock(handle.read())

    try:
        if arguments.advisories:
            advisories = _load_advisories(arguments.advisories)
            source = arguments.advisories
        else:
            advisories = fetch(name for name, _ in packages)
            source = "the GitHub Advisory Database"
        if arguments.write:
            with open(arguments.write, "w", encoding="utf-8") as handle:
                json.dump(advisories, handle, indent=2, sort_keys=True)
        findings = evaluate(packages, advisories, arguments.minimum)
    except Unavailable as error:
        # Not a clean bill of health. Say so, and fail differently.
        print("NAO VERIFICADO: %s" % error, file=sys.stderr)
        print(
            "A base de advisories do GitHub nao pôde ser consultada. Isto nao é "
            "o mesmo que zero vulnerabilidades.",
            file=sys.stderr,
        )
        return UNKNOWN

    print(
        "%d crates no %s, confrontados com %s."
        % (len(packages), arguments.lock, source)
    )
    if not findings:
        print(
            "Nenhum advisory do GitHub de severidade %s ou acima atinge uma "
            "versão presente." % arguments.minimum
        )
        return 0

    print("", file=sys.stderr)
    for finding in findings:
        print(
            "%s %s %s (%s)\n    %s%s\n    afectado: %s%s"
            % (
                finding["severity"].upper(),
                finding["package"],
                finding["version"],
                finding["ghsa"],
                "https://github.com/advisories/" + finding["ghsa"],
                "\n    " + finding["cve"] if finding["cve"] else "",
                finding["range"],
                "  → corrigido em " + finding["patched"] if finding["patched"] else "",
            ),
            file=sys.stderr,
        )
    print(
        "\n%d advisory(s) do GitHub atingem esta árvore de dependências."
        % len(findings),
        file=sys.stderr,
    )
    return 1


if __name__ == "__main__":
    sys.exit(main())
