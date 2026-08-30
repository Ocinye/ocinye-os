#!/usr/bin/env python3
"""As fronteiras arquitecturais do Ocinye OS, impostas pelo grafo real.

# O princípio

    O Core detém a verdade institucional. A Experience detém a sua apresentação.

Isso não é uma preferência de arrumação. É uma fronteira de confiança e uma
fronteira de dependências, e as duas metades dizem-se assim:

    O Core tem de continuar utilizável sem o Workspace.
    O Workspace tem de continuar incapaz de se tornar autoridade institucional.

A segunda metade é a que precisa de guarda. Um Workspace que possa importar
`ocinye-core` pode chamar serviços, políticas e repositórios directamente — e a
partir daí a autoridade deixa de estar num sítio só, sem que ninguém tenha
decidido isso.

# Porquê uma lista de permitidos, e não de proibidos

Uma lista de proibidos só apanha o que alguém já imaginou. Esta declara as
arestas que existem, e recusa qualquer outra. Uma dependência nova entre crates
do workspace tem de passar por aqui, o que a torna uma decisão em vez de um
efeito secundário de um `use`.

# Produção e teste são classificações, não nomes

    Arestas de produção estão fechadas por omissão. Arestas só de teste podem
    existir apenas quando explicitamente classificadas e justificadas.

O `ocinye-workspace` depende do `ocinye-core-server` **em testes**: o harness de
browser levanta um Core a sério, em processo, para provar que uma pessoa
consegue usar o produto. Isso é legítimo e não atravessa fronteira nenhuma — o
binário enviado não leva o Core consigo.

O que este guarda recusa é a promoção silenciosa. Se alguém mover
`ocinye-core` de `dev-dependencies` para `dependencies`, o nome da aresta
continua exactamente o mesmo e a fronteira mudou por completo. Por isso a
classificação é comparada antes do nome: uma aresta declarada como de teste que
apareça em produção é uma violação, e é dita como tal.
"""

import json
import pathlib
import re
import subprocess
import sys

# ── As arestas permitidas ───────────────────────────────────────────────────
#
# Cada entrada é `crate -> {dependências internas permitidas}`, e a leitura
# vertical desta tabela é o diagrama da arquitectura:
#
#     contracts, observability   nada dependem — são a base
#     domain                     conhece contratos, e mais nada
#     core                       conhece domínio; não conhece serviço nem UI
#     core-server, worker        montam o Core sobre transporte
#     workspace                  conhece **contratos**, e nada do Core
#
# A linha do `workspace` é a fronteira. Tudo o que ela não tem é o que a
# Experience não pode alcançar.
NORMAIS = {
    "ocinye-contracts": set(),
    "ocinye-observability": set(),
    "ocinye-domain": {"ocinye-contracts"},
    "ocinye-capabilities": {"ocinye-contracts"},
    # `ocinye-capabilities` entrou aqui a 2026-08-26, e só aqui.
    #
    # O Core delega computação especializada ao Capability Runtime — hoje, a
    # leitura de bibliografia BibTeX dentro do isolamento WASM/WASI. É uma
    # aresta deliberada e é a mínima: o Runtime executa, e o Core continua a
    # decidir quem pode pedir, o que entra, o que sai e o que isso significa
    # para a instituição.
    #
    # A aresta existe **nesta direcção e em mais nenhuma**. Nem a Experience nem
    # o plano agentic conhecem o Runtime: quem quiser computação isolada pede
    # uma operação de domínio ao Core, e é o Core que escolhe a implementação.
    # Uma linha nova a acrescentar `ocinye-capabilities` a `ocinye-workspace`
    # seria a Experience a executar código por conta própria.
    "ocinye-core": {
        "ocinye-capabilities",
        "ocinye-contracts",
        "ocinye-domain",
        "ocinye-observability",
    },
    "ocinye-core-server": {
        "ocinye-contracts",
        "ocinye-core",
        "ocinye-domain",
        "ocinye-observability",
    },
    "ocinye-worker": {"ocinye-contracts", "ocinye-core", "ocinye-observability"},
    "ocinye-node-agent": {"ocinye-contracts", "ocinye-observability"},
    # A Experience consome contratos tipados. Não conhece `ocinye-core`, nem
    # `ocinye-domain`, nem persistência. Acrescentar aqui qualquer um deles é
    # mover a autoridade institucional para dentro da apresentação.
    "ocinye-workspace": {"ocinye-contracts", "ocinye-observability"},
}

# Dependências de desenvolvimento. Um teste pode levantar o sistema todo; é
# assim que se prova que ele funciona.
DEV = {
    "ocinye-core": {"ocinye-core"},
    # `ocinye-core-server` → `ocinye-core` nos testes, desde 2026-08-30.
    #
    # Os harnesses do servidor passaram a chamar a guarda que recusa escrever
    # fixtures numa base que contém a organização canónica. A guarda vive em
    # `ocinye-core` atrás de `test-fixtures`, e é a mesma que os outros 23
    # harnesses usam — duplicá-la aqui seria ter seis cópias de uma guarda de
    # segurança e esperar que envelhecessem juntas.
    #
    # Esta aresta **não move autoridade**: `ocinye-core-server` já depende de
    # `ocinye-core` em produção, e isto é estritamente mais estreito — só os
    # alvos de teste, e só para uma função que o binário não contém. O portão
    # «Isolamento do fornecedor de teste» confirma-o em cada corrida.
    "ocinye-core-server": {"ocinye-core"},
    "ocinye-workspace": {
        "ocinye-core",
        "ocinye-core-server",
        "ocinye-observability",
    },
}


# ── O que a Experience pode ligar em produção ──────────────────────────────
#
# Não é uma lista de proibidos. Uma lista de proibidos apanha as tecnologias de
# persistência de que alguém se lembrou; esta declara o que existe, e recusa o
# resto.
#
# A propriedade guardada:
#
#     O código de produção da Experience não pode depender de tecnologia de
#     persistência.
#
# `reqwest` está aqui e é a travessia correcta da fronteira: a Experience fala
# com o Core por HTTP, sobre contratos tipados. `sqlx`, um pool de ligações ou
# um ORM não aparecem — e uma linha nova nesta lista obriga a explicar porquê.
EXPERIENCE_RUNTIME = {
    "anyhow",
    "axum",
    "chrono",
    "leptos",
    "ocinye-contracts",
    "ocinye-observability",
    "rand",
    "reqwest",
    "serde",
    "serde_json",
    "tokio",
    "tower-http",
    "tracing",
    "url",
    "uuid",
}

# Tecnologias que, se alguma vez aparecerem do lado da Experience, dizem por si
# o que aconteceu. Existem para dar uma mensagem melhor do que «não declarado».
PERSISTENCIA = {
    "sqlx",
    "diesel",
    "sea-orm",
    "rusqlite",
    "tokio-postgres",
    "postgres",
    "deadpool-postgres",
    "redis",
    "mongodb",
}


def experiencia_runtime():
    """As dependências de produção do Workspace, com nome externo incluído."""
    saida = subprocess.run(
        ["cargo", "metadata", "--no-deps", "--format-version", "1"],
        capture_output=True,
        text=True,
        check=False,
    )
    if saida.returncode != 0:
        raise SystemExit("cargo metadata falhou: " + saida.stderr.strip()[:400])
    for pacote in json.loads(saida.stdout)["packages"]:
        if pacote["name"] == "ocinye-workspace":
            return {
                d["name"]
                for d in pacote["dependencies"]
                if (d.get("kind") or "normal") == "normal"
            }
    raise SystemExit("o `ocinye-workspace` não está no workspace")


def grafo():
    """O grafo interno real, lido do Cargo e não de um ficheiro à parte."""
    saida = subprocess.run(
        ["cargo", "metadata", "--no-deps", "--format-version", "1"],
        capture_output=True,
        text=True,
        check=False,
    )
    if saida.returncode != 0:
        raise SystemExit("cargo metadata falhou: " + saida.stderr.strip()[:400])

    normais, dev, build = {}, {}, {}
    for pacote in json.loads(saida.stdout)["packages"]:
        nome = pacote["name"]
        if not nome.startswith("ocinye"):
            continue
        normais.setdefault(nome, set())
        dev.setdefault(nome, set())
        build.setdefault(nome, set())
        for dependencia in pacote["dependencies"]:
            alvo = dependencia["name"]
            if not alvo.startswith("ocinye"):
                continue
            espécie = dependencia.get("kind")
            if espécie == "dev":
                dev[nome].add(alvo)
            elif espécie == "build":
                build[nome].add(alvo)
            else:
                normais[nome].add(alvo)
    return normais, dev, build


# Porque é que cada aresta proibida é proibida. O guarda ensina a arquitectura
# em vez de dizer apenas que a lista não bate certo.
RAZOES = {
    ("ocinye-workspace", "ocinye-core"): (
        "A Experience alcança a autoridade institucional apenas através dos "
        "contratos suportados e da camada de cliente."
    ),
    ("ocinye-workspace", "ocinye-domain"): (
        "As invariantes de domínio são do Core. A Experience apresenta-as; não "
        "as reimplementa nem as reavalia."
    ),
    ("ocinye-workspace", "ocinye-core-server"): (
        "Ligar o servidor do Core dentro do Workspace é montar duas autoridades "
        "no mesmo processo."
    ),
    ("ocinye-core", "ocinye-workspace"): (
        "O Core tem de continuar utilizável sem o Workspace. Uma dependência "
        "neste sentido torna a apresentação obrigatória para haver autoridade."
    ),
    ("ocinye-domain", "ocinye-workspace"): (
        "O domínio não conhece apresentação. Nem browser, nem CSS, nem ecrãs."
    ),
    ("ocinye-contracts", "ocinye-workspace"): (
        "Contratos institucionais e contratos de apresentação são fronteiras "
        "diferentes. Um `ButtonVariant` não é um contrato institucional."
    ),
}


def _razao(crate, alvo):
    directa = RAZOES.get((crate, alvo))
    if directa:
        return directa
    if crate == "ocinye-workspace":
        return (
            "A Experience consome contratos tipados. Tudo o resto atravessa a "
            "fronteira de confiança."
        )
    if alvo == "ocinye-workspace":
        return "Nada do lado da autoridade pode depender da apresentação."
    return (
        "Uma aresta nova entre crates do workspace é uma decisão de "
        "arquitectura, e não um efeito secundário de um `use`."
    )


def confere(titulo, real, esperado, problemas, producao):
    rotulo = "ARESTA DE PRODUÇÃO NOVA" if producao else "ARESTA DE TESTE NOVA"
    for crate in sorted(set(real) | set(esperado)):
        obtido = real.get(crate, set())
        permitido = esperado.get(crate, set())

        for extra in sorted(obtido - permitido):
            problemas.append(
                "%s:\n"
                "      %s → %s\n\n"
                "      Classificação esperada:\n"
                "      PROIBIDA — %s"
                % (rotulo, crate, extra, _razao(crate, extra))
            )
        for ausente in sorted(permitido - obtido):
            problemas.append(
                "%s: `%s` já não depende de `%s`, mas a tabela ainda o declara.\n"
                "      Uma tabela que descreve o passado deixa de guardar o presente."
                % (titulo, crate, ausente)
            )


# ── Onde o Runtime é conhecido, dentro do Core ──────────────────────────────
#
# A tabela acima é ao nível da crate, e ao nível da crate `ocinye-core →
# ocinye-capabilities` é uma aresta legítima. Mas «o Core pode delegar
# computação isolada» não é a mesma proposição que «qualquer parte do Core pode
# chamar o motor». A primeira é arquitectura; a segunda é o motor a espalhar-se.
#
# A propriedade guardada:
#
#     Dentro de `ocinye-core`, só `capabilities.rs` nomeia o Runtime.
#
# É esse módulo que traduz erros de motor em erros de domínio, que decide que
# componentes existem e que os carrega a partir do manifesto. Um segundo sítio a
# nomear `ocinye_capabilities` é um segundo sítio a decidir o que um erro de
# combustível esgotado significa para a instituição — e serão duas respostas.
#
# O plano agentic é o caso concreto: um handler que chamasse o Runtime
# directamente saltava a autorização, o limite de entrada e a tradução de erros
# que a operação de domínio faz. Compila. Passa a tabela de crates. E é
# exactamente a fronteira que esta secção existe para recusar.
RUNTIME_NO_CORE = "crates/ocinye-core/src/capabilities.rs"
NOMES_DO_MOTOR = ("ocinye_capabilities", "wasmtime")


def contencao_do_runtime(problemas):
    raiz = pathlib.Path("crates/ocinye-core/src")
    ficheiros = sorted(raiz.rglob("*.rs"))
    # Um universo vazio aprova tudo. Se não há ficheiros para ler, ou se o
    # próprio módulo autorizado deixou de nomear o motor, esta secção não está
    # a medir contenção nenhuma — e dizer que está seria pior do que não a ter.
    if len(ficheiros) < 2:
        problemas.append(
            "CONTENÇÃO DO RUNTIME NÃO VERIFICÁVEL:\n"
            "      %d ficheiros lidos em %s.\n\n"
            "      Um universo vazio aprova tudo." % (len(ficheiros), raiz)
        )
        return 0

    autorizado = pathlib.Path(RUNTIME_NO_CORE)
    if not autorizado.exists() or not any(
        nome in autorizado.read_text(encoding="utf-8") for nome in NOMES_DO_MOTOR
    ):
        problemas.append(
            "CONTENÇÃO DO RUNTIME NÃO VERIFICÁVEL:\n"
            "      %s não nomeia o Runtime.\n\n"
            "      O módulo autorizado é o controlo positivo desta secção: se\n"
            "      ele não conhece o motor, não há contenção a medir." % autorizado
        )
        return 0

    for ficheiro in ficheiros:
        if ficheiro == autorizado:
            continue
        texto = ficheiro.read_text(encoding="utf-8")
        for nome in NOMES_DO_MOTOR:
            if not re.search(r"\b%s\b" % re.escape(nome), texto):
                continue
            problemas.append(
                "O RUNTIME ALCANÇADO FORA DO MÓDULO QUE O DETÉM:\n"
                "      %s nomeia `%s`\n\n"
                "      Dentro do Core, só %s conhece o motor.\n"
                "      Quem precisa de computação isolada pede uma operação de\n"
                "      domínio: é aí que a autorização, o limite de entrada e a\n"
                "      tradução de erros acontecem." % (ficheiro, nome, RUNTIME_NO_CORE)
            )
    return len(ficheiros)


def main():
    normais, dev, build = grafo()
    problemas = []
    lidos = contencao_do_runtime(problemas)

    # A promoção silenciosa é verificada primeiro, e em separado, porque é a
    # que o nome da aresta esconde: `ocinye-workspace → ocinye-core` lê-se igual
    # nas duas classificações e significa o oposto.
    promovidas = set()
    for crate, alvos in sorted(DEV.items()):
        for alvo in sorted(alvos):
            # Uma crate pode estar legitimamente nas duas listas — a
            # `ocinye-observability` está, porque os testes a usam directamente.
            # A promoção só é violação quando o destino não é permitido em
            # produção, que é precisamente o caso que o nome da aresta esconde.
            if alvo in NORMAIS.get(crate, set()):
                continue
            if alvo in normais.get(crate, set()):
                promovidas.add((crate, alvo))
                problemas.append(
                    "ARESTA PROMOVIDA DE TESTE PARA PRODUÇÃO:\n"
                    "      %s → %s\n\n"
                    "      Era uma dependência de teste, e passou a ser de produção.\n"
                    "      O nome não mudou; a fronteira mudou.\n\n"
                    "      Classificação esperada:\n"
                    "      PROIBIDA — %s" % (crate, alvo, _razao(crate, alvo))
                )

    # As promovidas já foram explicadas acima, com a razão certa; contá-las
    # outra vez como «aresta nova» diria a mesma coisa pior.
    normais_menos_promovidas = {
        crate: alvos - {a for (c, a) in promovidas if c == crate}
        for crate, alvos in normais.items()
    }
    # A Experience, e o que ela pode ligar.
    runtime = experiencia_runtime()
    for extra in sorted(runtime - EXPERIENCE_RUNTIME):
        if extra in PERSISTENCIA:
            problemas.append(
                "PERSISTÊNCIA NA EXPERIENCE:\n"
                "      ocinye-workspace → %s (produção)\n\n"
                "      Classificação esperada:\n"
                "      PROIBIDA — a Experience apresenta a verdade institucional;\n"
                "      não a lê da base de dados. O caminho é o Core, por\n"
                "      contratos tipados." % extra
            )
        else:
            problemas.append(
                "DEPENDÊNCIA DE PRODUÇÃO NOVA NA EXPERIENCE:\n"
                "      ocinye-workspace → %s\n\n"
                "      Se pertence à camada de apresentação, declare-a em\n"
                "      EXPERIENCE_RUNTIME. Se traz consigo acesso a estado\n"
                "      institucional, não pertence aqui." % extra
            )
    for ausente in sorted(EXPERIENCE_RUNTIME - runtime):
        problemas.append(
            "a Experience já não depende de `%s`, mas a lista ainda o declara."
            % ausente
        )

    confere("dependência", normais_menos_promovidas, NORMAIS, problemas, producao=True)
    confere("dependência de teste", dev, DEV, problemas, producao=False)

    for crate, alvos in sorted(build.items()):
        for alvo in sorted(alvos):
            problemas.append(
                "dependência de build: `%s` → `%s`, que não é esperada em lado nenhum."
                % (crate, alvo)
            )

    if problemas:
        print("Fronteiras arquitecturais violadas:\n", file=sys.stderr)
        for problema in problemas:
            print("  " + problema + "\n", file=sys.stderr)
        print(
            "O Core detém a verdade institucional; a Experience detém a sua\n"
            "apresentação. Uma aresta nova entre estas duas metades move a\n"
            "autoridade, e isso é uma decisão de arquitectura.",
            file=sys.stderr,
        )
        return 1

    print("Fronteiras arquitecturais:")
    for crate in sorted(NORMAIS):
        alvos = sorted(NORMAIS[crate])
        print("  %-22s → %s" % (crate, ", ".join(alvos) if alvos else "—"))
    print()
    print(
        "  A Experience consome contratos tipados. Não alcança o Core, nem o\n"
        "  domínio, nem persistência — e as %d dependências de produção que liga\n"
        "  estão declaradas uma a uma.\n"
        "  Dentro do Core, o Runtime é nomeado num módulo só: os outros %d\n"
        "  ficheiros não conhecem o motor." % (len(EXPERIENCE_RUNTIME), lidos - 1)
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
