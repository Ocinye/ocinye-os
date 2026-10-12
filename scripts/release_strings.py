#!/usr/bin/env python3
"""Release-string guard for the graphical surfaces of Ocinye OS.

The installer, the system interface and the D011 Installer window draw every
word from a string catalogue (`window.X = { 'key': [pt, en, fr], ... }`). The
runtime already counts a field left unfilled (`OI_UNRESOLVED`) and the bench
demands zero, but only on the screens the bench happens to visit. This guard
reads the catalogues and their callers without running them, so a template
that would reach a screen nobody opened in the bench still fails here.

What it refuses:

- an entry without exactly three non-empty texts;
- a text with `${`, `%s`, `{0}` or any other template form than `{name}`;
- the three languages of one key disagreeing on their `{name}` fields;
- a literal call `t('key')` without values for a key that has fields, and a
  literal call `t('key', {...})` that leaves one of the key's fields out;
- a keyed field no literal call ever fills (the key can then only reach the
  screen through a dynamic call, which this guard cannot see), unless the key
  is listed in DYNAMIC with the caller that fills it;
- a terminal prompt (`Escolher (1-2) >`, a raw `> ` read prompt) anywhere in
  the files a graphical surface ships.

Exit status: 0 PASS, 1 FAIL, 2 INVALID (a file is missing or unparsable).
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

# Each surface: its catalogues, and the scripts that call `t(...)` on them.
SURFACES = {
    "system-ui": (["apps/system-ui/ui/strings.js"], ["apps/system-ui/ui/app.js"]),
    "image-installer": (
        ["apps/system-ui/ui/installer-strings.js"],
        ["apps/system-ui/ui/installer.js"],
    ),
    "d011-installer": (
        ["apps/installer/ui/strings.js", "apps/installer/ui/strings-code.js"],
        ["apps/installer/ui/app.js"],
    ),
}

# Files a graphical surface ships, scanned for terminal prompts.
SHIPPED = ["apps/system-ui/ui", "apps/installer/ui"]

# Keys with fields that reach `t` through a value no scanner can follow (a
# table, a variable): key -> the caller that fills every field. Empty today.
DYNAMIC: dict[str, str] = {}

FIELD = re.compile(r"\{([a-z_][a-z0-9_]*)\}")
BAD_TEMPLATE = re.compile(r"\$\{|%[sd]|\{\d+\}|\{\{|\}\}|\{(?![a-z_][a-z0-9_]*\})")
TTY = [
    re.compile(r"\(\s*\d+\s*-\s*\d+\s*\)\s*>"),  # Escolher (1-2) >
    re.compile(r"Escolher\s*\("),
    re.compile(r"\[[sSyY]/[nN]\]"),  # [s/N] read prompts
]


class Invalid(Exception):
    pass


def tokens(text: str):
    """Strings, identifiers and punctuation of a catalogue file; no comments."""
    i, n = 0, len(text)
    while i < n:
        c = text[i]
        if c.isspace():
            i += 1
        elif text.startswith("//", i):
            i = text.find("\n", i) if "\n" in text[i:] else n
        elif text.startswith("/*", i):
            j = text.find("*/", i + 2)
            if j < 0:
                raise Invalid("comentário por fechar")
            i = j + 2
        elif c in "'\"`":
            j, buf = i + 1, []
            while j < n and text[j] != c:
                if text[j] == "\\":
                    buf.append(text[j : j + 2])
                    j += 2
                else:
                    buf.append(text[j])
                    j += 1
            if j >= n:
                raise Invalid("texto por fechar")
            yield ("str" if c != "`" else "tpl", "".join(buf))
            i = j + 1
        elif c.isalnum() or c in "_$":
            j = i
            while j < n and (text[j].isalnum() or text[j] in "_$."):
                j += 1
            yield ("id", text[i:j])
            i = j
        else:
            yield ("p", c)
            i += 1


def catalogue(paths: list[Path]) -> dict[str, list[str]]:
    out: dict[str, list[str]] = {}
    for path in paths:
        rel = path.relative_to(ROOT)
        if not path.is_file():
            raise Invalid(f"{rel}: não existe")
        try:
            toks = list(tokens(path.read_text(encoding="utf-8")))
        except Invalid as e:
            raise Invalid(f"{rel}: {e}") from None
        found = 0
        for k in range(len(toks) - 2):
            if toks[k][0] == "str" and toks[k + 1] == ("p", ":") and toks[k + 2] == ("p", "["):
                key, texts, j = toks[k][1], [], k + 3
                while j < len(toks) and toks[j] != ("p", "]"):
                    if toks[j][0] in ("str", "tpl"):
                        texts.append(toks[j][1] if toks[j][0] == "str" else "${" + toks[j][1])
                    elif toks[j] != ("p", ","):
                        raise Invalid(f"{rel}: '{key}' tem um valor que não é texto")
                    j += 1
                if key in out:
                    raise Invalid(f"{rel}: chave repetida {key}")
                out[key] = texts
                found += 1
        if not found:
            raise Invalid(f"{rel}: catálogo vazio")
    return out


def _args(text: str, i: int) -> tuple[list[str], int]:
    """Top-level arguments of the call whose `(` is at text[i]."""
    depth, j, args, cur, quote = 0, i, [], [], None
    while j < len(text):
        c = text[j]
        if quote:
            cur.append(c)
            if c == "\\":
                cur.append(text[j + 1])
                j += 2
                continue
            if c == quote:
                quote = None
        elif c in "'\"`":
            quote = c
            cur.append(c)
        elif c in "([{":
            depth += 1
            if depth > 1:
                cur.append(c)
        elif c in ")]}":
            depth -= 1
            if depth == 0:
                args.append("".join(cur).strip())
                return args, j
            cur.append(c)
        elif c == "," and depth == 1:
            args.append("".join(cur).strip())
            cur = []
        else:
            cur.append(c)
        j += 1
    raise Invalid("chamada por fechar")


LIT = re.compile(r"""^(['"])([^'"]+)\1$""")
QUOTED = re.compile(r"""(['"])([a-zA-Z0-9_.-]+)\1""")
PREFIX = re.compile(r"""^\(?\s*(['"])([a-zA-Z0-9_.-]*\.)\1\s*\+""")
OBJ_KEY = re.compile(r"(?:^|,)\s*([a-z_][a-z0-9_]*)\s*(?=:|,|$)")


def given_names(arg: str | None) -> set[str] | None:
    """Names an object literal supplies; None when there is no second argument."""
    if arg is None:
        return None
    if not (arg.startswith("{") and arg.endswith("}")):
        return {"*"}  # a value the scanner cannot read: trusted, not proven
    inner, depth, top = arg[1:-1], 0, []
    for c in inner:
        if c in "([{":
            depth += 1
        elif c in ")]}":
            depth -= 1
        top.append(c if depth == 0 or c in "([{" and depth == 1 else " ")
    return set(OBJ_KEY.findall("".join(top)))


def calls(paths):
    for p in paths:
        rel = p.relative_to(ROOT)
        if not p.is_file():
            raise Invalid(f"{rel}: não existe")
        text = p.read_text(encoding="utf-8")
        for m in re.finditer(r"(?<![A-Za-z0-9_$.])t\(", text):
            try:
                args, _ = _args(text, m.end() - 1)
            except Invalid:
                raise Invalid(f"{rel}: chamada por fechar") from None
            if not args or not args[0]:
                continue
            line = text.count("\n", 0, m.start()) + 1
            given = given_names(args[1] if len(args) > 1 else None)
            first = args[0]
            lit = LIT.match(first)
            if lit:
                yield rel, line, "key", lit.group(2), given
                continue
            pre = PREFIX.match(first)
            if pre:
                yield rel, line, "prefix", pre.group(2), given
                continue
            for q in QUOTED.finditer(first):  # cond ? 'a' : 'b'
                yield rel, line, "key", q.group(2), given


def check() -> list[str]:
    fails: list[str] = []
    for name, (cats, callers) in SURFACES.items():
        entries = catalogue([ROOT / c for c in cats])
        fields: dict[str, set[str]] = {}
        for key, texts in entries.items():
            where = f"{name} '{key}'"
            if len(texts) != 3 or any(not s.strip() for s in texts):
                fails.append(f"{where}: precisa de três textos não vazios (pt, en, fr)")
                continue
            for lang, s in zip(("pt", "en", "fr"), texts):
                if BAD_TEMPLATE.search(s):
                    fails.append(f"{where} [{lang}]: modelo fora da forma {{nome}}: {s!r}")
            sets = [set(FIELD.findall(s)) for s in texts]
            if not sets[0] == sets[1] == sets[2]:
                fails.append(
                    f"{where}: campos diferentes por língua: "
                    + ", ".join(f"{l}={sorted(x)}" for l, x in zip(("pt", "en", "fr"), sets))
                )
            fields[key] = sets[0]
        filled: set[str] = set()
        # A prefix call without values (`t('server.' + x)`) reaches some keys
        # under the prefix, not all; it fails a key only if nothing fills it.
        pending: list = []
        caller_text = "".join((ROOT / c).read_text(encoding="utf-8") for c in callers if (ROOT / c).is_file())
        for path, line, kind, key, given in calls(ROOT / c for c in callers):
            keys = [key] if kind == "key" else [k for k in fields if k.startswith(key)]
            for k in keys:
                need = fields.get(k)
                if not need:
                    continue  # sem campos, ou em falta (contada em tempo de execução)
                if given is None and kind == "prefix":
                    pending.append((path, line, k, need))
                elif given is None:
                    fails.append(f"{path}:{line}: t('{k}') sem valores para {sorted(need)}")
                elif "*" in given:
                    continue
                elif need - given:
                    fails.append(f"{path}:{line}: t('{k}', …) deixa por preencher {sorted(need - given)}")
                else:
                    filled.add(k)
        for path, line, k, need in pending:
            if k not in filled:
                fails.append(f"{path}:{line}: t('{k}') pode chegar sem valores para {sorted(need)}")
        unused = 0
        for key, need in sorted(fields.items()):
            if not need or key in filled or key in DYNAMIC:
                continue
            if re.search(r"""['"]""" + re.escape(key) + r"""['"]""", caller_text):
                fails.append(f"{name} '{key}': campos {sorted(need)} chegam a t() por um caminho que o guarda não prova")
            else:
                unused += 1
        print(
            f"  {name}: {len(entries)} chaves, {sum(1 for f in fields.values() if f)} com campos, "
            f"{len(filled)} provadas, {unused} com campos sem chamador"
        )
    for d in SHIPPED:
        base = ROOT / d
        if not base.is_dir():
            raise Invalid(f"{d}: não existe")
        for p in sorted(base.rglob("*")):
            if p.suffix not in (".js", ".html", ".css") or not p.is_file():
                continue
            for n, line in enumerate(p.read_text(encoding="utf-8").splitlines(), 1):
                for rx in TTY:
                    if rx.search(line):
                        fails.append(f"{p.relative_to(ROOT)}:{n}: prompt de terminal numa superfície gráfica")
    return fails


def main() -> int:
    try:
        fails = check()
    except Invalid as e:
        print(f"INVALID: {e}")
        return 2
    if fails:
        print("FAIL: textos de release por resolver")
        for f in fails:
            print(f"  {f}")
        return 1
    print("PASS: nenhum texto de release fica por resolver")
    return 0


if __name__ == "__main__":
    sys.exit(main())
