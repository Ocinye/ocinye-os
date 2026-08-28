#!/usr/bin/env python3
"""A Secção 1 do `CLAUDE.md` diz o que a árvore diz.

# Porque isto existe

Porque a Secção 1 é a única parte daquele ficheiro que descreve estado real, e
todos os seus números tinham envelhecido em silêncio: 62 caminhos contra 131,
12 migrations contra 19, 45 tabelas contra 63, 64 permissões contra 72, 40 ADRs
contra 49. Nada falhava, porque nada os comparava.

`repository-facts.sh` passou a derivá-los. Derivar não chega: um número
derivado que ninguém confronta com o documento é um número que o documento pode
contradizer à vontade.

Offline, sem rede e sem GitHub. Só lê.
"""
import re
import pathlib
import subprocess
import sys

RAIZ = pathlib.Path(__file__).resolve().parent.parent

# O que cada facto derivado tem de encontrar na Secção 1, e como se procura.
#
# O padrão nomeia o número com um grupo, para a mensagem poder dizer o que o
# documento afirma **e** o que a árvore diz. Um teste que só diz «diverge»
# obriga a próxima pessoa a ir procurar as duas metades.
CONFRONTOS = [
    ("caminhos-core", r"(\d+) caminhos e \d+ operações", "caminhos sob /api/v1"),
    ("operacoes-core", r"\d+ caminhos e (\d+) operações", "operações HTTP"),
    ("ecras-workspace", r"\*\* (\d+) ecrãs em Leptos SSR", "ecrãs do Workspace"),
    ("migrations", r"\*\*(\d+) migrations\*\*", "migrations"),
    ("tabelas", r"aplicáveis de base vazia; (\d+) tabelas", "tabelas"),
    ("permissoes", r"`IMPLEMENTED`\.\*\* (\d+) permissões", "permissões"),
    ("adrs", r"\*\*(\d+) ADRs\*\*", "ADRs"),
    ("runbooks", r"\*\*(\d+) runbooks\*\*", "runbooks"),
    ("readmes", r"\*\*(\d+) READMEs\*\*", "READMEs"),
    ("testes-com-postgres", r"\*\*(\d+) deles não correm sem base de dados\*\*",
     "testes que exigem PostgreSQL"),
]


def derivados():
    saida = subprocess.run(
        [str(RAIZ / "scripts" / "repository-facts.sh")],
        capture_output=True, text=True, check=True,
    ).stdout
    return {
        linha.split()[0]: linha.split()[1]
        for linha in saida.splitlines() if linha.strip()
    }


def main():
    factos = derivados()
    claude = (RAIZ / "CLAUDE.md").read_text()
    # Só a Secção 1: o resto do ficheiro é norma, e um número numa norma é um
    # exemplo, não uma afirmação sobre o estado.
    inicio = claude.index("## 1. Estado real do repositório")
    fim = claude.index("Todo o restante conteúdo deste ficheiro")
    seccao = claude[inicio:fim]

    problemas = []
    for chave, padrao, o_que in CONFRONTOS:
        esperado = factos.get(chave)
        if esperado is None:
            problemas.append(f"{o_que}: `repository-facts.sh` não deriva «{chave}»")
            continue
        encontrado = re.search(padrao, seccao)
        if not encontrado:
            problemas.append(
                f"{o_que}: a Secção 1 não afirma nada que este contrato reconheça.\n"
                f"      A árvore diz {esperado}. Se a frase mudou, actualizar o padrão aqui."
            )
            continue
        if encontrado.group(1) != esperado:
            problemas.append(
                f"{o_que}: a Secção 1 diz {encontrado.group(1)}, a árvore diz {esperado}"
            )

    if problemas:
        print("A Secção 1 do CLAUDE.md diverge da árvore:", file=sys.stderr)
        for problema in problemas:
            print(f"  {problema}", file=sys.stderr)
        print(file=sys.stderr)
        print("  Os números saem de ./scripts/repository-facts.sh.", file=sys.stderr)
        return 1

    print(f"Secção 1: {len(CONFRONTOS)} factos, todos de acordo com a árvore")
    return 0


if __name__ == "__main__":
    sys.exit(main())
