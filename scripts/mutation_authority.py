#!/usr/bin/env python3
"""A escrita institucional acontece no Core, e em mais nenhum sítio.

# A propriedade

> **Core is authority. Experience is presentation.**

Traduzido em algo que se pode medir: nenhum crate fora de `crates/ocinye-core`
escreve no estado institucional. Se um serviço de transporte, um worker ou a
Experience puderem escrever, passa a haver dois sítios a decidir o que é
verdade, e mais cedo ou mais tarde discordam.

`scripts/architecture_boundaries.py` já recusa que a Experience sequer dependa
de persistência. Isto fecha o outro lado: os serviços que **têm** legitimamente
uma ligação à base de dados — o servidor de HTTP e o worker — não a usam para
mutação de domínio.

# As excepções, e porque não são domínio

Duas, ambas declaradas abaixo com a razão. Nenhuma tem principal, nenhuma
autoriza nada, e nenhuma escreve numa tabela que uma Core Operation também
escreva. Uma é configuração do operador a ser reflectida num registo; a outra é
a drenagem do outbox, que publica o que o Core já decidiu e escreveu.

A lista é para encolher. Uma excepção nova é uma decisão institucional, não uma
conveniência.
"""

from __future__ import annotations

import os
import re
import subprocess
import sys

# Ficheiro → porque é que escreve, e porque não é domínio.
EXCEPCOES: dict[str, str] = {
    "services/core-server/src/main.rs": (
        "regista o backend de armazenamento configurado, uma vez no arranque e "
        "de forma idempotente. É configuração do operador reflectida num "
        "registo, sem principal e sem autorização; nenhuma Core Operation "
        "escreve `storage_backends`"
    ),
    "services/worker/src/outbox.rs": (
        "drena o outbox: marca como publicado o que o Core já decidiu e "
        "escreveu na mesma transacção do efeito. Não decide nada"
    ),
}

MUTACOES = re.compile(
    r"\b(INSERT\s+INTO|UPDATE\s+[a-z_\"]+\s+SET|DELETE\s+FROM|TRUNCATE\s+TABLE)\b",
    re.IGNORECASE,
)

# SQL vive dentro de literais de texto, e só aí.
#
# Procurar no ficheiro inteiro dava falsos positivos com cara de verdade: a
# classe CSS `oc-truncate` e o `.truncate(true)` de uma opção de ficheiro
# pareciam `TRUNCATE`. Um guarda que grita por causa de uma classe de estilo
# ensina a ignorá-lo.
LITERAIS = re.compile(r'r#*"(.*?)"#*|"((?:[^"\\]|\\.)*)"', re.DOTALL)


def sql_do_ficheiro(fonte: str) -> str:
    """O texto dos literais, que é onde as consultas podem estar."""
    return "\n".join(
        (m.group(1) or m.group(2) or "") for m in LITERAIS.finditer(fonte)
    )


def raiz() -> str:
    return subprocess.run(
        ["git", "rev-parse", "--show-toplevel"],
        capture_output=True,
        text=True,
        check=True,
    ).stdout.strip()


def main() -> int:
    base = raiz()
    fora: list[str] = []
    excepcoes_usadas: set[str] = set()
    examinados = 0

    for area in ("services", "apps", "crates"):
        for pasta, _, ficheiros in os.walk(os.path.join(base, area)):
            if "target" in pasta:
                continue
            for ficheiro in ficheiros:
                if not ficheiro.endswith(".rs"):
                    continue
                caminho = os.path.join(pasta, ficheiro)
                relativo = os.path.relpath(caminho, base)
                # O Core é onde isto deve acontecer.
                if relativo.startswith("crates/ocinye-core/"):
                    continue
                # As migrations são história, e os testes montam as suas fixtures.
                if "/tests/" in f"/{relativo}":
                    continue
                examinados += 1
                fonte = open(caminho, encoding="utf-8", errors="replace").read()
                if not MUTACOES.search(sql_do_ficheiro(fonte)):
                    continue
                if relativo in EXCEPCOES:
                    excepcoes_usadas.add(relativo)
                    continue
                fora.append(relativo)

    print("Autoridade de escrita institucional:")
    print(
        f"  {examinados} ficheiros fora do Core examinados · "
        f"{len(EXCEPCOES)} excepções declaradas"
    )

    # Ver o cabeçalho de `dependency_consumers.py`.
    if examinados == 0:
        print()
        print("  ZERO OBSERVAÇÕES: nenhum ficheiro fora do Core foi examinado.")
        print("  Não há aqui nada a aprovar; há um caminho errado.")
        return 1

    obsoletas = sorted(set(EXCEPCOES) - excepcoes_usadas)
    if obsoletas:
        print()
        print("  EXCEPÇÃO DECLARADA QUE JÁ NÃO ESCREVE:")
        for nome in obsoletas:
            print(f"      {nome}")
        print("      Deixou de precisar da licença. Remova-a da lista.")
        return 1

    if fora:
        print()
        print("  ESCRITA INSTITUCIONAL FORA DO CORE:")
        for nome in sorted(fora):
            print(f"      {nome}")
        print()
        print("      A autoridade institucional é do Core. Um segundo sítio a")
        print("      escrever é um segundo sítio a decidir o que é verdade, e")
        print("      dois acabam por discordar.")
        print()
        print("      Se for mesmo infraestrutura e não domínio, declare-a em")
        print("      EXCEPCOES neste ficheiro, com a razão escrita.")
        return 1

    print("  Toda a mutação de domínio vive no Core.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
