#!/usr/bin/env python3
"""Propriedades textuais da documentação que envelhecem em silêncio.

# O que este guarda faz, e o que deliberadamente não faz

Guarda **propriedades textuais**: uma frase que tem de existir num sítio e em
mais nenhum, uma afirmação que deixou de ser verdade e não pode voltar. Para isso
uma comparação de texto é a ferramenta certa.

Não tenta validar arquitectura por substring. Já custou uma regressão:
`no_capability_reaches_infrastructure` recusava `science.execution.record` por
«execution» conter «exec». Uma propriedade estrutural mede-se onde a estrutura
existe — no catálogo tipado, no registry, na matriz — e não aqui.

Offline. Só lê.
"""
import pathlib
import re
import sys

RAIZ = pathlib.Path(__file__).resolve().parent.parent

# ── A definição canónica ─────────────────────────────────────────────────
#
# Uma formulação, uma fonte. Três versões ligeiramente diferentes espalhadas
# pelo repositório é como uma instituição perde a sua própria definição: cada
# cópia envelhece à sua maneira e ninguém sabe qual é a boa.
DEFINICAO = (
    "infraestrutura digital institucional através da qual a Ocinye organiza, "
    "governa, preserva e transforma conhecimento, dados, investigação e "
    "engenharia em capacidade tecnológica duradoura"
)
FONTE_DA_DEFINICAO = "docs/architecture/README.md"
# O README cita-a, e citar é o comportamento correcto — a proibição é redefinir.
PODEM_CITAR = {"README.md"}

# ── Afirmações que deixaram de ser verdade ───────────────────────────────
#
# Cada entrada é um par: o padrão, e a razão pela qual não pode voltar. A razão
# vai na mensagem de erro, porque um guarda que só diz «proibido» obriga a
# próxima pessoa a ir descobrir porquê.
OBSOLETAS = [
    (
        r"`?Result`?[^.\n]{0,80}\bnão\b[^.\n]{0,40}\b(têm|tem) tabelas?",
        "`Result` tem tabela desde a migration 0019.",
    ),
    (
        r"\bExperiment\b\s*→",
        "`Experiment` não é uma entidade: o domínio adoptou `Study` com género "
        "fechado (ADR-0412).",
    ),
    (
        r"knowledge::create_result",
        "A operação chama-se `science::create_result` e vive no módulo `science`.",
    ),
    (
        r"nome de utilizador e palavra-passe",
        "O endereço institucional é a credencial única (ADR-0106).",
    ),
]

# `CLAUDE.md` guarda história datada, e uma norma pode citar o que já foi
# verdade. Os documentos históricos de segurança também.
ISENTOS = {"CLAUDE.md"}
PREFIXOS_ISENTOS = ("docs/security/2026-",)


def documentos():
    for md in sorted(RAIZ.glob("*.md")) + sorted(RAIZ.glob("docs/**/*.md")):
        rel = md.relative_to(RAIZ).as_posix()
        if "target/" in rel:
            continue
        # Comparado sem as quebras de linha nem os marcadores de citação: a
        # propriedade é a frase, e não onde o editor decidiu mudar de linha ou
        # se ela está dentro de um bloco `>`. Um guarda que dependesse da
        # largura da coluna falharia na primeira reformatação.
        texto = md.read_text()
        corrido = re.sub(r"^\s*>\s?", "", texto, flags=re.M)
        yield rel, texto, re.sub(r"\s+", " ", corrido)


def main():
    problemas = []
    onde_esta_a_definicao = []

    for rel, texto, corrido in documentos():
        if DEFINICAO in corrido:
            onde_esta_a_definicao.append(rel)

        if rel in ISENTOS or rel.startswith(PREFIXOS_ISENTOS):
            continue
        # As ADRs registam decisões datadas; reescrevê-las seria apagar história.
        if rel.startswith("docs/adrs/") and rel != "docs/adrs/README.md":
            continue

        for padrao, razao in OBSOLETAS:
            achado = re.search(padrao, texto)
            if achado:
                linha = texto[: achado.start()].count("\n") + 1
                problemas.append(f"{rel}:{linha}: «{achado.group(0)}» — {razao}")

    esperados = {FONTE_DA_DEFINICAO} | PODEM_CITAR
    if FONTE_DA_DEFINICAO not in onde_esta_a_definicao:
        problemas.append(
            f"{FONTE_DA_DEFINICAO} deixou de conter a definição canónica do "
            "Ocinye OS. É a fonte; se mudou de sítio, actualizar este guarda."
        )
    for rel in onde_esta_a_definicao:
        if rel not in esperados:
            problemas.append(
                f"{rel} repete a definição canónica. Os outros documentos "
                f"resumem e ligam para {FONTE_DA_DEFINICAO}; não redefinem."
            )

    if problemas:
        print("Factos da documentação:", file=sys.stderr)
        for problema in problemas:
            print(f"  {problema}", file=sys.stderr)
        return 1

    print(
        f"Documentação: definição canónica em {FONTE_DA_DEFINICAO}, "
        f"citada em {len(onde_esta_a_definicao) - 1} documento(s); "
        f"{len(OBSOLETAS)} afirmações obsoletas ausentes"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
