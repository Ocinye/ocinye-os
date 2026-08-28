#!/usr/bin/env python3
"""Nenhuma ligação interna da documentação aponta para o que não existe.

# Porque isto é um portão e não uma revisão

Uma ligação partida não parte nada em produção, e é por isso que sobrevive. Mas
a documentação é parte do contrato do sistema: um documento que aponta para um
ficheiro que já não existe ensina que a documentação não é de confiança, e a
partir daí ninguém a lê.

Ficheiros movidos, secções renomeadas e ADRs consolidados são as três formas
comuns de acontecer, e nenhuma delas dá erro em lado nenhum.

# O que se verifica

Ligações relativas a caminhos do repositório, e âncoras dentro de documentos
Markdown. Ligações externas não: dependeriam da rede, e uma verificação que
falha por o Wi-Fi ter caído deixa de ser lida.
"""

from __future__ import annotations

import os
import re
import subprocess
import sys

LIGACAO = re.compile(r"\[[^\]]*\]\(([^)\s]+)\)")


def raiz() -> str:
    return subprocess.run(
        ["git", "rev-parse", "--show-toplevel"],
        capture_output=True,
        text=True,
        check=True,
    ).stdout.strip()


def ancoras(caminho: str) -> set[str]:
    """As âncoras que um documento Markdown oferece."""
    encontradas: set[str] = set()
    for linha in open(caminho, encoding="utf-8", errors="replace"):
        if not linha.startswith("#"):
            continue
        titulo = linha.lstrip("#").strip().lower()
        # Como o GitHub as constrói: minúsculas, sem pontuação, espaços por hífen.
        limpo = re.sub(r"[^\w\s-]", "", titulo, flags=re.UNICODE)
        encontradas.add(re.sub(r"\s+", "-", limpo).strip("-"))
    return encontradas


def main() -> int:
    base = raiz()

    documentos: list[str] = [os.path.join(base, "README.md")]
    for pasta, _, ficheiros in os.walk(os.path.join(base, "docs")):
        documentos += [
            os.path.join(pasta, f) for f in ficheiros if f.endswith(".md")
        ]
    documentos += [
        os.path.join(base, f)
        for f in ("CLAUDE.md",)
        if os.path.exists(os.path.join(base, f))
    ]

    if len(documentos) < 10:
        print("Ligações da documentação:")
        print()
        print(f"  ZERO OBSERVAÇÕES: só encontrei {len(documentos)} documentos.")
        print("  O caminho está errado, e este guarda aprovaria tudo.")
        return 1

    partidas: list[str] = []
    total = 0

    for documento in documentos:
        pasta = os.path.dirname(documento)
        relativo = os.path.relpath(documento, base)
        for m in LIGACAO.finditer(
            open(documento, encoding="utf-8", errors="replace").read()
        ):
            alvo = m.group(1)
            if alvo.startswith(("http://", "https://", "mailto:")):
                continue
            total += 1

            caminho, _, ancora = alvo.partition("#")
            if caminho:
                destino = os.path.normpath(os.path.join(pasta, caminho))
                if not os.path.exists(destino):
                    partidas.append(f"{relativo} → {alvo} (não existe)")
                    continue
            else:
                destino = documento

            if ancora and destino.endswith(".md"):
                if ancora.lower() not in ancoras(destino):
                    partidas.append(f"{relativo} → {alvo} (âncora não existe)")

    print("Ligações da documentação:")
    print(f"  {len(documentos)} documentos · {total} ligações internas")

    if partidas:
        print()
        print("  LIGAÇÃO PARTIDA:")
        for partida in partidas:
            print(f"      {partida}")
        print()
        print("      A documentação é parte do contrato do sistema. Uma")
        print("      ligação que não abre ensina que não é de confiança.")
        return 1

    print("  Todas resolvem.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
