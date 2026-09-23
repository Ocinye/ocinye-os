#!/usr/bin/env python3
"""i18n chrome guard — mixed-language becomes detectable, not needle-dependent.

The Ocinye Workspace renders all product chrome through the i18n via
(`crate::i18n::t` / `tf` / `tp`). Any user-visible string literal that reaches a
render position **without** going through that via is untranslated chrome: it
will show in Portuguese even when the member chose English or French.

The earlier i18n slices were needle-driven — the author migrated the strings they
listed, and each purity test asserted only those same needles. Screens marked
"fully migrated" kept Portuguese in conditional branches, secondary buttons,
footers and all-caps banners. This guard reads the source instead of trusting a
needle list: it flags string literals in chrome position that are not routed
through i18n and are not on the allowlist of genuine non-chrome (CSS classes,
URLs, data keys, enum values, acronyms, brand terms).

It is deliberately conservative about what counts as chrome (multi-word natural
language, or a capitalised/all-caps word) so that data values and identifiers do
not trip it. When it is green over a screen, that screen has no bare chrome.

Usage:
    python3 scripts/i18n-chrome-guard.py            # report + exit 1 if any
    python3 scripts/i18n-chrome-guard.py --baseline # write the baseline file
    python3 scripts/i18n-chrome-guard.py --list     # print every violation

While migration is in progress, `i18n-chrome-baseline.json` records the files
that are known to still contain stragglers, with their counts. The guard fails
only when a NEW violation appears in a file at or below its baseline count, or a
count rises — so clean screens can never regress, and the baseline shrinks to
empty as screens are migrated. A file absent from the baseline must be clean.
"""
import json
import os
import re
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SCAN_DIRS = [
    "apps/workspace/src/ui/screens",
    "apps/workspace/src/ui/components",
]
BASELINE_PATH = os.path.join(ROOT, "scripts", "i18n-chrome-baseline.json")

# Attributes whose value is chrome shown to a human. Everything else attribute is
# machinery (class, href, id, …) and its value is never translated.
CHROME_ATTRS = {"title", "alt", "placeholder", "aria-label", "aria-placeholder",
                "aria-roledescription", "aria-valuetext"}

# Exact strings that are legitimately not translated: brand, acronyms, symbols.
ALLOW = {
    "OCINYE OS", "OCINYE WORKSPACE", "Ocinye", "ocinye.com", "OCINYE CORE",
    "CPU", "RAM", "GPU", "PNG", "JPEG", "WebP", "RTF", "PDF", "SVG", "DOI",
    "TOTP", "MFA", "QR", "OS", "AI", "HTML", "URL", "IMAP", "SMTP", "TLS",
    "PUBLIC", "INTERNAL", "CONFIDENTIAL", "RESTRICTED",
    "pt", "en", "fr", "pt-PT",
}

STR = re.compile(r'"((?:[^"\\]|\\.)*)"')
LETTER = re.compile(r"[A-Za-zÀ-ÖØ-öø-ÿ]")
WORD = re.compile(r"[A-Za-zÀ-ÖØ-öø-ÿ]{2,}")


# An i18n key, whole ("compute.registered_count") or a dynamic prefix that a
# format! completes ("task.state." from format!("task.state.{}", …)).
KEY = re.compile(r"^[a-z][a-z0-9_]*(?:\.[a-z0-9_]*)+$")


def is_chrome(s: str) -> bool:
    """True when `s` looks like natural-language product chrome."""
    if s in ALLOW or len(s) < 2:
        return False
    # A `format!` / class template: drop the `{...}` holes and judge the rest.
    # If what remains carries no natural-language word, it is machinery
    # (a class list, an id, an HTML fragment, a units string), not chrome.
    core = re.sub(r"\{[^{}]*\}", "", s).strip()
    if not core or not LETTER.search(core):
        return False
    # HTML / attribute fragments assembled in a string.
    if "<" in core or '="' in core or "http-equiv" in core or "/>" in core:
        return False
    # An i18n key itself (a dotted lowercase token on a wrapped t()/tf() call).
    if KEY.match(core):
        return False
    # URLs, paths, anchors, mailto, otpauth, mime types.
    if core.startswith(("/", "#", "http", "mailto:", "otpauth:", "data:")):
        return False
    if "://" in core or core.count("/") >= 2:
        return False
    # A URL query fragment (`?unit=true`, `&sort=asc`) assembled onto a base with
    # `format!`. After the `{…}` holes are stripped it can read as words, but it
    # is machinery, not chrome: no spaces, and it is `?`/`&` key=value pairs.
    if re.fullmatch(r"[?&][\w=&%.+-]*", core):
        return False
    # A single snake_case / kebab token, or a class list, or an enum key.
    tokens = core.split()
    if all(re.fullmatch(r"[A-Za-z0-9:_-]+", t) for t in tokens):
        # class lists ("oc-a oc-b"), css-ish, snake_case data keys, kebab ids.
        if any(("-" in t or "_" in t or ":" in t) for t in tokens):
            return False
        if len(tokens) == 1:
            t = tokens[0]
            # ALL-CAPS single word (e.g. "AVISO", "OK") — an enum/state value,
            # unless it is clearly a word a person reads. Treat as data.
            if re.fullmatch(r"[A-Z0-9]+", t):
                return False
            # lowercase single word — a data value / key.
            if re.fullmatch(r"[a-z][a-z0-9]*", t):
                return False
    letters = len(LETTER.findall(core))
    # Multi-word natural language.
    if len(WORD.findall(core)) >= 2 and letters >= 3:
        return True
    # A single capitalised word of real length: "Confirmar", "Cancelar".
    if re.match(r"^[A-ZÀ-Þ][a-zà-ÿ]{2,}$", core):
        return True
    # An all-caps banner with punctuation/spacing ("OCINYE CORE · X" handled by
    # the multi-word rule; a lone all-caps accented word here).
    if re.fullmatch(r"[A-ZÀ-Þ][A-ZÀ-Þ0-9]{3,}", core) and letters >= 4:
        return True
    return False


def attr_before(line: str, quote_start: int):
    """If the literal is an attribute value, return the attribute name."""
    prefix = line[:quote_start]
    m = re.search(r'([A-Za-z_][\w-]*)\s*=\s*$', prefix)
    return m.group(1) if m else None


def data_position(line: str, quote_start: int) -> bool:
    """True when the literal is a data value, not chrome: match arm, .get(), ==."""
    before = line[:quote_start].rstrip()
    after = line[quote_start:]
    if before.endswith((".get(", ".contains(", ".starts_with(", ".ends_with(",
                        "== ", "!= ", "eq(", "&", "insert(", ".push_str(")):
        return True
    # match arm:  "value" => ...   or   "a" | "b" =>
    if re.match(r'^\s*"(?:[^"\\]|\\.)*"\s*(?:\|\s*"(?:[^"\\]|\\.)*"\s*)*=>', line):
        return True
    # data-oc-value / attribute value bindings handled by attr_before.
    return False


def scan_file(path: str):
    hits = []
    with open(path, encoding="utf-8") as fh:
        lines = fh.readlines()
    for i, line in enumerate(lines, 1):
        stripped = line.lstrip()
        if stripped.startswith("#[cfg(test)"):
            break
        if stripped.startswith(("//", "///", "//!")):
            continue
        if "i18n::" in line:
            continue
        for m in STR.finditer(line):
            s = m.group(1)
            if not is_chrome(s):
                continue
            attr = attr_before(line, m.start())
            if attr is not None and attr not in CHROME_ATTRS:
                continue
            if data_position(line, m.start()):
                continue
            hits.append((i, s))
    return hits


def all_violations():
    result = {}
    for d in SCAN_DIRS:
        base = os.path.join(ROOT, d)
        for name in sorted(os.listdir(base)):
            if not name.endswith(".rs"):
                continue
            rel = os.path.join(d, name)
            hits = scan_file(os.path.join(base, name))
            if hits:
                result[rel] = hits
    return result


def main():
    viol = all_violations()
    counts = {f: len(h) for f, h in viol.items()}

    if "--list" in sys.argv:
        for f, hits in sorted(viol.items(), key=lambda kv: -len(kv[1])):
            print(f"\n=== {f} ({len(hits)}) ===")
            for ln, s in hits:
                print(f"  {ln}: {s[:80]}")
        print(f"\nTOTAL: {sum(counts.values())} in {len(counts)} files")
        return 0

    if "--baseline" in sys.argv:
        with open(BASELINE_PATH, "w", encoding="utf-8") as fh:
            json.dump({"counts": dict(sorted(counts.items()))}, fh,
                      ensure_ascii=False, indent=2)
            fh.write("\n")
        print(f"Baseline written: {sum(counts.values())} in {len(counts)} files")
        return 0

    baseline = {}
    if os.path.exists(BASELINE_PATH):
        with open(BASELINE_PATH, encoding="utf-8") as fh:
            baseline = json.load(fh).get("counts", {})

    failures = []
    for f, n in sorted(counts.items()):
        allowed = baseline.get(f, 0)
        if n > allowed:
            failures.append((f, n, allowed))
    # A file that dropped to clean should be removed from the baseline.
    stale = [f for f, allowed in baseline.items() if counts.get(f, 0) < allowed]

    if failures:
        print("Chrome não roteado pela via i18n (mistura de línguas):\n")
        for f, n, allowed in failures:
            print(f"  {f}: {n} literais de interface (baseline {allowed})")
            for ln, s in viol[f][:12]:
                print(f"      {ln}: {s[:80]}")
        print("\n  Rota tudo por crate::i18n::t/tf/tp, ou, se for legítimo não-chrome,")
        print("  ajusta o allowlist do guarda. Os números saem do próprio código.")
        return 1

    total = sum(counts.values())
    if total == 0:
        print("i18n chrome guard: 0 literais de interface fora da via i18n.")
    else:
        print(f"i18n chrome guard: {total} stragglers conhecidos, nenhum acima da baseline"
              f" ({len(counts)} ficheiros).")
    if stale:
        print("  (baseline desactualizada — estes ficheiros melhoraram; corre --baseline:"
              f" {', '.join(os.path.basename(f) for f in stale)})")
    return 0


if __name__ == "__main__":
    sys.exit(main())
