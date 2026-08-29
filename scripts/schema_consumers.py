#!/usr/bin/env python3
"""Cada tabela do esquema tem quem a leia ou escreva.

# Porque uma tabela sem leitor não é inofensiva

Uma tabela existe porque alguém decidiu que uma coisa devia ser guardada. Se
nenhum código lhe toca, uma de três coisas é verdade, e todas merecem ser ditas
em voz alta:

- a funcionalidade foi removida e o esquema ficou para trás;
- a funcionalidade nunca foi construída, e o esquema promete-a;
- alguma coisa escreve-lhe por um caminho que ninguém consegue encontrar.

A segunda é a mais cara, porque parece garantia. `mail_outbox` tem estados,
tentativas e razão de falha — a forma exacta de uma fila durável com repetição.
O envio de correio é síncrono e não lhe toca. Quem lesse o esquema concluiria
que uma mensagem sobrevive a um fornecedor em baixo, e não sobrevive.

# As migrations não se apagam

Uma migração aplicada é história (ADR-0002). Este guarda não pede que a tabela
desapareça; pede que o seu estado esteja **declarado**. Uma entrada em
`SEM_LEITOR` é uma afirmação de que se sabe, e porquê.
"""

from __future__ import annotations

import os
import re
import subprocess
import sys

# Tabela → porque é que ninguém lhe toca hoje.
#
# A lista é para encolher: cada entrada é uma promessa por cumprir ou uma
# decisão por tomar, não um sítio confortável para deixar coisas.
SEM_LEITOR: dict[str, str] = {
    "mail_outbox": (
        "fila durável de expedição, com tentativas e razão de falha. O envio "
        "é síncrono contra o fornecedor e nunca passa por aqui — um fornecedor "
        "em baixo devolve erro a quem enviou, e a mensagem não fica em fila. "
        "O esquema promete uma garantia que a implementação não dá"
    ),
    "_sqlx_migrations": "registo do próprio migrador",
}

CRIA = re.compile(r"^CREATE TABLE (?:IF NOT EXISTS )?([a-z_]+)", re.M)


def raiz() -> str:
    return subprocess.run(
        ["git", "rev-parse", "--show-toplevel"],
        capture_output=True,
        text=True,
        check=True,
    ).stdout.strip()


def main() -> int:
    base = raiz()

    tabelas: set[str] = set()
    pasta = os.path.join(base, "migrations")
    if not os.path.isdir(pasta):
        print("Consumidores do esquema:")
        print()
        print(f"  ZERO OBSERVAÇÕES: não encontrei {pasta}.")
        print("  Sem migrations não há esquema para confrontar, e este guarda")
        print("  aprovaria tudo por não ter nada para reprovar.")
        return 1
    for ficheiro in sorted(os.listdir(pasta)):
        if not ficheiro.endswith(".sql"):
            continue
        fonte = open(os.path.join(pasta, ficheiro), encoding="utf-8").read()
        tabelas |= set(CRIA.findall(fonte))

    if not tabelas:
        print("Consumidores do esquema:")
        print()
        print("  ZERO OBSERVAÇÕES: nenhuma tabela encontrada nas migrations.")
        print("  Um esquema vazio faz este guarda aprovar tudo por não ter")
        print("  nada para reprovar. É o caminho que está errado.")
        return 1

    # ── O que não conta como leitor ─────────────────────────────────────
    #
    # `continuity/manifest.rs` nomeia **todas** as tabelas do esquema, por
    # construção: tem de haver uma decisão de continuidade para cada uma. Se
    # contasse como leitor, nenhuma tabela poderia voltar a ficar sem leitor, e
    # este guarda passaria a aprovar tudo sem nunca reprovar nada.
    #
    # Uma decisão sobre uma tabela não é um consumidor dela.
    NAO_E_LEITOR = ("continuity/manifest.rs", "continuity/classification.rs")

    codigo = []
    for area in ("crates", "services", "apps"):
        for onde, _, ficheiros in os.walk(os.path.join(base, area)):
            if "target" in onde:
                continue
            codigo += [
                os.path.join(onde, f) for f in ficheiros if f.endswith(".rs")
            ]
    codigo = [
        f for f in codigo if not any(x in f.replace(os.sep, "/") for x in NAO_E_LEITOR)
    ]
    texto = "\n".join(
        open(f, encoding="utf-8", errors="replace").read() for f in codigo
    )

    sem_leitor = sorted(
        t for t in tabelas if not re.search(rf"\b{re.escape(t)}\b", texto)
    )
    nao_declaradas = [t for t in sem_leitor if t not in SEM_LEITOR]
    declaradas_com_leitor = sorted(
        t for t in SEM_LEITOR if t in tabelas and t not in sem_leitor
    )

    print("Consumidores do esquema:")
    print(
        f"  {len(tabelas)} tabelas · {len(sem_leitor)} sem leitor · "
        f"{len(SEM_LEITOR) - 1} declaradas"
    )

    if declaradas_com_leitor:
        print()
        print("  DECLARADA SEM LEITOR, MAS AGORA TEM UM:")
        for nome in declaradas_com_leitor:
            print(f"      {nome}")
        print("      Boa notícia. Remova-a de SEM_LEITOR.")
        return 1

    if nao_declaradas:
        print()
        print("  TABELA SEM LEITOR:")
        for nome in nao_declaradas:
            print(f"      {nome}")
        print()
        print("      Nenhum código lhe toca. Ou a funcionalidade saiu e o")
        print("      esquema ficou, ou o esquema promete o que não existe.")
        print("      Declare-a em SEM_LEITOR, com a razão — as migrations são")
        print("      história e não se apagam, mas o estado tem de estar dito.")
        return 1

    print("  Todas as tabelas sem leitor estão declaradas.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
