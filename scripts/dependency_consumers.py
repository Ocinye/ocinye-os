#!/usr/bin/env python3
"""Cada dependência directa de produção tem um consumidor conhecido.

# O que isto recusa

Uma dependência declarada em `[dependencies]` que nenhum ficheiro de `src/` do
próprio crate menciona. São três coisas diferentes, e todas custam:

- **Lixo.** Ficou de uma implementação que já não existe. Compila-se, faz-se
  download, audita-se e revê-se por nada.
- **Promoção silenciosa de teste para produção.** A dependência é usada só pelas
  suites, mas está declarada como de produção, e por isso entra no binário que
  vai para o servidor. `scripts/architecture_boundaries.py` recusa exactamente
  isto entre crates internos; nada o recusava para dependências externas — e foi
  assim que `tower` esteve em `[dependencies]` do `ocinye-core-server` a servir
  apenas `ServiceExt::oneshot` dentro de `tests/`.
- **Intenção por cumprir.** `subtle` esteve declarado no `ocinye-core` sem uma
  única comparação em tempo constante escrita. Uma dependência de segurança que
  ninguém chama não é uma defesa; é a memória de uma que se pensou fazer.

# Porque a referência textual chega

Depois da limpeza de 2026-08-25 não há uma única dependência directa de produção
que o `src` do seu crate não mencione pelo nome — nem sequer as que só entram por
macro de derivação, porque essas mencionam o nome do trait e o crate a seguir.
Enquanto isso for verdade, a busca textual não tem falsos positivos, e é barata
o suficiente para correr em todas as verificações.

Se um dia existir uma dependência legítima que o código não nomeia — um crate que
só existe para uma feature, ou por exigência do linker — acrescenta-se a
`SEM_MENCAO` com a razão escrita. A lista é para encolher.
"""

from __future__ import annotations

import json
import os
import re
import subprocess
import sys

# Dependências de produção que o código legitimamente não nomeia.
#
# Cada entrada precisa de uma razão. Vazia de propósito: hoje não existe
# nenhuma, e é assim que se quer.
SEM_MENCAO: dict[tuple[str, str], str] = {}


def raiz() -> str:
    return subprocess.run(
        ["git", "rev-parse", "--show-toplevel"],
        capture_output=True,
        text=True,
        check=True,
    ).stdout.strip()


def main() -> int:
    base = raiz()
    metadata = subprocess.run(
        ["cargo", "metadata", "--no-deps", "--format-version", "1"],
        capture_output=True,
        text=True,
        cwd=base,
        check=True,
    ).stdout
    pacotes = json.loads(metadata)["packages"]
    internos = {p["name"] for p in pacotes}

    orfas: list[str] = []
    contadas = 0

    for pacote in sorted(pacotes, key=lambda p: p["name"]):
        directorio = os.path.join(os.path.dirname(pacote["manifest_path"]), "src")
        fontes: list[str] = []
        for pasta, _, ficheiros in os.walk(directorio):
            fontes += [
                os.path.join(pasta, f) for f in ficheiros if f.endswith(".rs")
            ]
        texto = "\n".join(
            open(f, encoding="utf-8", errors="replace").read() for f in fontes
        )

        for dependencia in pacote["dependencies"]:
            if dependencia["kind"] is not None:
                continue
            nome = dependencia["name"]
            if nome in internos:
                continue
            contadas += 1
            if (pacote["name"], nome) in SEM_MENCAO:
                continue
            identificador = nome.replace("-", "_")
            if re.search(rf"\b{re.escape(identificador)}\b", texto):
                continue
            orfas.append(f"{pacote['name']} → {nome}")

    print("Consumidores das dependências directas de produção:")
    print(f"  {contadas} arestas directas de produção")

    # Zero observações não é «tudo bem»: é um verificador que não encontrou o
    # que devia examinar. `cargo metadata` a devolver um workspace vazio, ou um
    # caminho errado, dariam verde sem terem olhado para nada.
    if contadas == 0:
        print()
        print("  ZERO OBSERVAÇÕES: nenhuma dependência directa de produção foi")
        print("  examinada. Isto não é um repositório sem dependências; é este")
        print("  verificador a não estar a ver o que devia.")
        return 1

    if orfas:
        print()
        print("  DEPENDÊNCIA DE PRODUÇÃO SEM CONSUMIDOR:")
        for orfa in orfas:
            print(f"      {orfa}")
        print()
        print("      O `src` deste crate não menciona esta dependência.")
        print("      Ou é lixo — e remove-se; ou só serve os testes — e passa")
        print("      para `[dev-dependencies]`, porque em `[dependencies]` entra")
        print("      no binário que vai para o servidor; ou é uma intenção por")
        print("      cumprir — e então falta o código que a devia usar.")
        print()
        print("      Se for legítima e o código não a nomear, acrescente-a a")
        print("      SEM_MENCAO neste ficheiro, com a razão escrita.")
        return 1

    print("  Todas têm consumidor no `src` do seu crate.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
