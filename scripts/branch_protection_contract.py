#!/usr/bin/env python3
"""Os nomes que a protecção de `main` exige continuam a existir na CI.

# O defeito que isto guarda

A protecção de `main` lista *required status checks* **por nome**. O GitHub não
verifica que esses nomes correspondem a alguma coisa: se um job da CI for
renomeado, o check exigido deixa de ser reportado, e um Pull Request fica à
espera de um resultado que ninguém vai produzir.

Isso não falha — **espera**. Uma renomeação inocente bloqueia todos os merges
para `main`, indefinidamente, e o sintoma é um PR verde que não deixa carregar
no botão. Quem o vê conclui que a CI está lenta.

# Porque isto é offline

Porque um portão que perguntasse ao GitHub deixaria de correr sem rede e
falharia num *fork* sem credenciais — e um portão que só é verde onde há
credenciais deixa de ser um portão, que é o argumento já escrito no
`.gitleaks.toml` para o `target/`.

Portanto o contrato vive **aqui**, ao lado do ficheiro que ele descreve, e
compara-se com o ficheiro. Alterar a protecção no GitHub obriga a alterar esta
lista, e alterar esta lista sem alterar a protecção é o que o comentário no
topo pede que não se faça.

# O que isto **não** prova

Não prova que a protecção remota é a que aqui está escrita: isso é uma
propriedade do servidor, e verifica-se contra o servidor. Prova a metade que se
pode provar sem rede — que os nomes exigidos correspondem a jobs que existem.
"""

from __future__ import annotations

import pathlib
import re
import sys

REPO = pathlib.Path(__file__).resolve().parent.parent
WORKFLOW = REPO / ".github" / "workflows" / "ci.yml"

# Os cinco *required status checks* de `main`, tal como a API os devolveu em
# 2026-08-27. Alterar a protecção no GitHub obriga a alterar esta lista.
EXIGIDOS = (
    "Testes",
    "Stack local",
    "Formatação, lint e segredos",
    "Advisories RustSec (cargo audit)",
    "Advisories do GitHub (Cargo.lock)",
)

# O portão canónico corre **depois** do merge, e não é exigido antes.
#
# Não é um esquecimento: ele confronta o `Cargo.lock` do branch canónico com a
# base de advisories, e um advisory continua aberto até a correcção chegar a
# `main`. Exigi-lo num PR reprovaria justamente o PR que o vem fechar.
DEPOIS_DO_MERGE = ("Postura do branch canónico",)


def nomes_de_job(workflow: str) -> list[str]:
    """Os nomes dos jobs, que são os que o GitHub reporta como *checks*.

    Um job declara `name:` com quatro espaços de indentação; os passos dentro
    dele declaram o seu com seis ou mais. Distinguir pela indentação é o que
    impede este portão de contar «cargo fmt» como um check.
    """
    return re.findall(r"^    name: (.+)$", workflow, re.MULTILINE)


def main() -> int:
    if not WORKFLOW.exists():
        print(f"o workflow não existe em {WORKFLOW}", file=sys.stderr)
        return 1

    jobs = nomes_de_job(WORKFLOW.read_text(encoding="utf-8"))
    problemas: list[str] = []

    for exigido in EXIGIDOS:
        if exigido not in jobs:
            problemas.append(
                f"«{exigido}» é exigido pela protecção de `main` e nenhum job da "
                f"CI tem esse nome. Um Pull Request ficará à espera de um "
                f"resultado que ninguém produz — e isso não falha, espera."
            )

    for tardio in DEPOIS_DO_MERGE:
        if tardio not in jobs:
            problemas.append(
                f"«{tardio}» desapareceu da CI. É o portão que corre sobre o "
                f"branch canónico depois do merge, e sem ele um advisory novo "
                f"deixa de ser encontrado contra código que já cá está."
            )
        if tardio in EXIGIDOS:
            problemas.append(
                f"«{tardio}» está na lista de exigidos antes do merge. Ele só "
                f"pode correr no estado que ainda não existe."
            )

    if problemas:
        print("Contrato da protecção de `main`:", file=sys.stderr)
        for problema in problemas:
            print(f"  ✗ {problema}", file=sys.stderr)
        return 1

    print("Contrato da protecção de `main`:")
    print(f"  {len(EXIGIDOS)} checks exigidos, todos declarados na CI")
    print(f"  {len(DEPOIS_DO_MERGE)} portão canónico, depois do merge")
    return 0


if __name__ == "__main__":
    sys.exit(main())
