#!/usr/bin/env python3
"""Cada configuração suportada é lida por alguém, e está documentada.

# As duas perguntas

- **Uma variável lida e não documentada** é uma configuração que só existe para
  quem leu o código. Quem instala não a encontra, e descobre-a quando o valor
  por omissão não serve.
- **Uma variável documentada e não lida** é pior: promete um controlo que não
  existe. Alguém escreve-a no `.env`, reinicia, e o comportamento não muda.

Havia uma de cada quando isto foi escrito. `OCINYE_WORKSPACE_STATIC_DIR` era
lida pela configuração do Workspace e não aparecia em lado nenhum;
`OCINYE_PUBLIC_URL` era definida por uma fixture de teste e não era lida por
ninguém em todo o repositório.

# O que conta como lida

Um literal `"OCINYE_..."` no código de produção ou de teste. As variáveis
`OCINYE_TEST_*` são de harness e não pertencem ao `.env.example` de uma
instalação — estão isentas da segunda pergunta, não da primeira.
"""

from __future__ import annotations

import os
import re
import subprocess
import sys

# Literais que parecem nomes de variável e não são.
#
# `Residency` guarda os locais institucionais como `OCINYE_CAMAMA` e
# `OCINYE_COLOCATION`: são valores de um enum, e não configuração.
NAO_SAO_VARIAVEIS = {"OCINYE_CAMAMA", "OCINYE_COLOCATION"}


def raiz() -> str:
    return subprocess.run(
        ["git", "rev-parse", "--show-toplevel"],
        capture_output=True,
        text=True,
        check=True,
    ).stdout.strip()


def main() -> int:
    base = raiz()

    lidas: set[str] = set()
    for area in ("crates", "services", "apps"):
        for pasta, _, ficheiros in os.walk(os.path.join(base, area)):
            if "target" in pasta:
                continue
            for ficheiro in ficheiros:
                if not ficheiro.endswith(".rs"):
                    continue
                caminho = os.path.join(pasta, ficheiro)
                fonte = open(caminho, encoding="utf-8", errors="replace").read()
                lidas |= set(re.findall(r'"(OCINYE_[A-Z0-9_]+)"', fonte))
    lidas -= NAO_SAO_VARIAVEIS

    caminho_exemplo = os.path.join(base, ".env.example")
    if not os.path.exists(caminho_exemplo):
        print("Superfície de configuração:")
        print()
        print(f"  ZERO OBSERVAÇÕES: não encontrei {caminho_exemplo}.")
        print("  Sem o lado documentado, a comparação aprova tudo.")
        return 1
    exemplo = open(caminho_exemplo, encoding="utf-8").read()
    documentadas = set(re.findall(r"(OCINYE_[A-Z0-9_]+)", exemplo)) - NAO_SAO_VARIAVEIS

    de_harness = {v for v in lidas if v.startswith("OCINYE_TEST_")}
    sem_documento = sorted(lidas - documentadas - de_harness)
    sem_leitor = sorted(documentadas - lidas)

    print("Superfície de configuração:")
    print(
        f"  {len(lidas)} variáveis lidas ({len(de_harness)} de harness) · "
        f"{len(documentadas)} documentadas"
    )

    # Ver o cabeçalho de `dependency_consumers.py`: zero de um lado ou do outro
    # é o verificador a falhar, e não o sistema a estar limpo.
    if not lidas or not documentadas:
        print()
        print("  ZERO OBSERVAÇÕES: não encontrei variáveis lidas, ou não")
        print("  encontrei o `.env.example`. Um lado vazio faz a comparação")
        print("  passar sem ter comparado coisa nenhuma.")
        return 1

    if sem_documento or sem_leitor:
        print()
        if sem_documento:
            print("  LIDA E NÃO DOCUMENTADA:")
            for nome in sem_documento:
                print(f"      {nome}")
            print("      Quem instala não a encontra. Acrescente-a a `.env.example`.")
        if sem_leitor:
            print("  DOCUMENTADA E NÃO LIDA:")
            for nome in sem_leitor:
                print(f"      {nome}")
            print("      Promete um controlo que não existe. Remova-a, ou ligue-a.")
        return 1

    print("  Todas têm leitor e documentação.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
