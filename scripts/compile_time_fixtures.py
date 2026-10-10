#!/usr/bin/env python3
"""Every file a Rust source includes at compile time is versioned.

`include_bytes!` and `include_str!` read a file while compiling. If that file
is ignored by git, the tree builds on the machine that has it and nowhere else:
a fresh clone fails in the compiler, far from the cause. It happened with three
test certificates that `*.pem` in `.gitignore` kept out of the repository.

Read-only. Exit 1 names each include whose target is not tracked.
"""
import os
import re
import subprocess
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
INCLUDE = re.compile(r'include_(?:bytes|str)!\(\s*"([^"]+)"\s*\)')
ANY = re.compile(r'include_(?:bytes|str)!\(')


def tracked():
    out = subprocess.run(["git", "-C", ROOT, "ls-files", "-z"], check=True, capture_output=True).stdout
    return {p for p in out.decode().split("\0") if p}


def main():
    files = tracked()
    literal = other = 0
    missing = []
    for path in sorted(p for p in files if p.endswith(".rs")):
        try:
            text = open(os.path.join(ROOT, path), encoding="utf-8").read()
        except OSError:
            continue
        found = INCLUDE.findall(text)
        other += len(ANY.findall(text)) - len(found)
        for rel in found:
            literal += 1
            target = os.path.normpath(os.path.join(os.path.dirname(path), rel))
            if target not in files:
                missing.append((path, rel, target))
    if missing:
        print("Ficheiros incluídos na compilação que não estão versionados:", file=sys.stderr)
        for path, rel, target in missing:
            print(f"  {path}: include de \"{rel}\" -> {target}", file=sys.stderr)
        print("Um clone novo não compila. Versione o ficheiro (com uma excepção\n"
              "específica no .gitignore, se for o caso) ou retire o include.", file=sys.stderr)
        return 1
    print(f"Includes de compilação: {literal} com caminho literal, todos versionados"
          + (f"; {other} com caminho construído, fora deste guarda" if other else ""))
    return 0


if __name__ == "__main__":
    sys.exit(main())
