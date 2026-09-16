#!/usr/bin/env python3
"""Cada serviço do Compose nomeia o stage do Dockerfile que constrói.

# O incidente que isto impede

Um `docker build` sem `target` explícito constrói o **último** stage do
Dockerfile. Enquanto o `runtime` era o último, os serviços `core`/`worker`/
`workspace` construíam-se sem `target` e funcionava. No dia em que se
acrescentaram stages novos (o conversor, o runner) **depois** do `runtime`, o
último stage passou a ser outro, e os três serviços saíram do stage errado — sem
`curl`, sem os estáticos. O healthcheck do core falhou, a workspace não arrancou,
e produção ficou em baixo. Só se descobriu no deploy, porque a CI não constrói
imagens.

# O que este guarda exige

> Um serviço que construa de um Dockerfile **multi-stage** tem de declarar
> `build.target`, e esse stage tem de existir no Dockerfile.

Um Dockerfile de um só stage não tem ambiguidade e não precisa de `target`. Um
`target` que aponte para um stage inexistente é um erro de escrita, e apanha-se
aqui em vez de no servidor.
"""

from __future__ import annotations

import os
import re
import sys

RAIZ = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))

# Os ficheiros de Compose a verificar. Um que não exista é ignorado.
COMPOSES = [
    "infra/compose/docker-compose.yml",
    "infra/compose/docker-compose.production.yml",
]

FROM_AS = re.compile(r"^\s*FROM\s+\S+(?:\s+.*)?\s+AS\s+(\S+)\s*$", re.IGNORECASE | re.MULTILINE)
FROM_ANY = re.compile(r"^\s*FROM\s+", re.IGNORECASE | re.MULTILINE)


def stages_do_dockerfile(caminho: str) -> tuple[set[str], int]:
    """Os nomes dos stages e o número total de `FROM` de um Dockerfile."""
    with open(caminho, encoding="utf-8") as handle:
        texto = handle.read()
    nomes = {m.group(1) for m in FROM_AS.finditer(texto)}
    total = len(FROM_ANY.findall(texto))
    return nomes, total


def main() -> int:
    try:
        import yaml
    except ImportError:
        print("compose_build_targets: falta o PyYAML", file=sys.stderr)
        return 1

    problemas: list[str] = []
    servicos_vistos = 0

    for rel in COMPOSES:
        caminho = os.path.join(RAIZ, rel)
        if not os.path.exists(caminho):
            continue
        with open(caminho, encoding="utf-8") as handle:
            doc = yaml.safe_load(handle)
        for nome, servico in (doc.get("services") or {}).items():
            build = servico.get("build") if isinstance(servico, dict) else None
            if not isinstance(build, dict):
                continue  # imagem pré-construída, sem build
            dockerfile = build.get("dockerfile")
            if not dockerfile:
                continue
            df = os.path.join(RAIZ, dockerfile)
            if not os.path.exists(df):
                # O `context` pode reposicionar; tenta relativo ao contexto.
                contexto = build.get("context", ".")
                df = os.path.normpath(os.path.join(RAIZ, "infra", "compose", contexto, dockerfile))
            if not os.path.exists(df):
                problemas.append(f"{rel}: serviço «{nome}» aponta para um Dockerfile inexistente ({dockerfile})")
                continue

            servicos_vistos += 1
            stages, total = stages_do_dockerfile(df)
            target = build.get("target")

            if total >= 2:
                if not target:
                    problemas.append(
                        f"{rel}: serviço «{nome}» constrói um Dockerfile multi-stage "
                        f"({dockerfile}, {total} stages) sem `target` explícito — "
                        f"um build sem target escolhe o ÚLTIMO stage, e isso deriva "
                        f"quando se acrescenta um stage novo."
                    )
                elif target not in stages:
                    problemas.append(
                        f"{rel}: serviço «{nome}» tem `target: {target}`, que não é "
                        f"um stage de {dockerfile} (stages: {', '.join(sorted(stages)) or 'nenhum nomeado'})"
                    )

    print("Alvos de construção do Compose:")
    print(f"  {servicos_vistos} serviços com build examinados")
    if problemas:
        print()
        for p in problemas:
            print(f"  {p}", file=sys.stderr)
        print(f"\n{len(problemas)} problema(s) de alvo de construção", file=sys.stderr)
        return 1
    print("  Cada serviço multi-stage nomeia o stage que constrói.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
