#!/usr/bin/env python3
"""Structural checks over the ADR library.

# What this does, and what it deliberately does not

It validates **structure**: filenames, unique identifiers, required metadata,
known values, and that every declared dependency and supersession resolves.

It does **not** try to judge architecture. Whether a decision belongs in the
Agentic range rather than the Foundations range is a question for a person, and
a script that guessed at it would be wrong in exactly the cases that matter.
"""

import os
import re
import sys

ADRS = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "docs", "adrs")

STATUS = {"Proposed", "Accepted", "Superseded", "Rejected", "Deprecated"}
IMPACT = {"FOUNDATIONAL", "HIGH", "MEDIUM", "LOCAL"}
DOMAINS = {
    "Foundation", "Security", "Identity", "Data", "Knowledge",
    "Agentic", "AI", "Mail", "Calendar", "Science", "Compute", "Workspace",
    "Operations",
}
FAMILIES = [
    (1, 99, "Foundations"), (100, 199, "Identity/Security"),
    (200, 299, "Knowledge/Data"), (300, 399, "AI/Agentic"),
    (400, 499, "Native modules"), (500, 599, "Compute"),
    (600, 699, "Workspace"), (700, 799, "Operations"),
    (800, 899, "Integrations"), (900, 999, "Reserved"),
]

FILENAME = re.compile(r"^(\d{4})-([a-z0-9]+(?:-[a-z0-9]+)*)\.md$")


def field(text, name):
    match = re.search(rf"- \*\*{name}:\*\* (.+)", text)
    return match.group(1).strip() if match else None


def referenced_ids(value):
    return re.findall(r"ADR-(\d{4})", value or "")


def main():
    problems = []
    seen = {}

    files = sorted(f for f in os.listdir(ADRS) if f.endswith(".md") and f != "README.md")
    if not files:
        print("nenhuma ADR encontrada", file=sys.stderr)
        return 1

    for name in files:
        match = FILENAME.match(name)
        if not match:
            problems.append(f"{name}: nome fora do formato NNNN-kebab-case.md")
            continue

        ident = match.group(1)
        if ident in seen:
            problems.append(f"{name}: identificador {ident} duplicado, já usado por {seen[ident]}")
        seen[ident] = name

        if not any(lo <= int(ident) <= hi for lo, hi, _ in FAMILIES):
            problems.append(f"{name}: {ident} fora de qualquer faixa conhecida")

        with open(os.path.join(ADRS, name), encoding="utf-8") as handle:
            text = handle.read()

        header = text.splitlines()[0] if text else ""
        if f"ADR-{ident}" not in header:
            problems.append(f"{name}: o título não declara ADR-{ident}")

        status = field(text, "Estado")
        if status not in STATUS:
            problems.append(f"{name}: estado «{status}» não é um valor conhecido")

        impact = field(text, "Impacto")
        if impact not in IMPACT:
            problems.append(f"{name}: impacto «{impact}» não é um valor conhecido")

        domain = field(text, "Domínio")
        if domain not in DOMAINS:
            problems.append(f"{name}: domínio «{domain}» não é um valor conhecido")

        # A decision cannot depend on itself, and every reference must resolve.
        for label in ("Depende de", "Substitui", "Substituído por", "Refinado por"):
            for target in referenced_ids(field(text, label)):
                if target == ident:
                    problems.append(f"{name}: «{label}» aponta para si própria")
                elif not any(f.startswith(f"{target}-") for f in files):
                    problems.append(f"{name}: «{label}» aponta para ADR-{target}, que não existe")

        if status == "Superseded" and not field(text, "Substituído por"):
            problems.append(f"{name}: está Superseded e não diz por quê")

    # Supersession is declared in both directions.
    for name in files:
        path = os.path.join(ADRS, name)
        with open(path, encoding="utf-8") as handle:
            text = handle.read()
        ident = FILENAME.match(name).group(1)
        for target in referenced_ids(field(text, "Substituído por")):
            other = seen.get(target)
            if not other:
                continue
            with open(os.path.join(ADRS, other), encoding="utf-8") as handle:
                other_text = handle.read()
            back = referenced_ids(field(other_text, "Substitui"))
            if ident not in back:
                problems.append(
                    f"{other}: substitui ADR-{ident} e não o declara em «Substitui»"
                )

    # Every ADR appears in the index.
    with open(os.path.join(ADRS, "README.md"), encoding="utf-8") as handle:
        index = handle.read()
    for name in files:
        if name not in index:
            problems.append(f"{name}: não aparece no índice")

    if problems:
        for problem in problems:
            print(f"  {problem}", file=sys.stderr)
        print(f"\n{len(problems)} problema(s) na biblioteca de ADRs", file=sys.stderr)
        return 1

    print(f"{len(files)} ADRs, estrutura consistente")
    return 0


if __name__ == "__main__":
    sys.exit(main())
