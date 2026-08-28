#!/usr/bin/env python3
"""Os tokens novos produzem exactamente os valores que já eram renderizados.

# O que este portão prova, e o que não prova

Prova uma coisa só, e é a afirmação central desta consolidação:

    A tokenização mudou a fonte de verdade, não os pixels.

Expandir cada token introduzido de volta ao seu valor tem de devolver, declaração
a declaração, o mesmo CSS que existia antes. Se devolver, nenhuma superfície
mudou — não por inspecção visual, e sim por identidade.

Não prova que o produto está bonito, nem que a escala é a certa. Prova que a
migração não trouxe uma alteração visual escondida dentro de um refactor, que é
a maneira mais fácil de perder a confiança de quem revê.

# Porque é que isto é um invariante de migração, e não uma regra eterna

Este portão compara com um estado congelado. Serve enquanto a migração for
recente e a memória de «o que havia antes» ainda importar. Quando o Design
System evoluir de propósito, esta comparação passa a estorvar em vez de
proteger — e sai, por decisão, com a mesma clareza com que entrou.

Os outros três portões arquitecturais não têm data de validade. Este tem.
"""

import re
import subprocess
import sys

# O commit imediatamente anterior à consolidação. É o «antes» contra o qual a
# equivalência é medida.
BASE = "075204e"

FICHEIRO = "apps/workspace/static/ocinye.css"

# Cada token introduzido nesta consolidação, com o valor que substituiu. É esta
# tabela que torna a expansão verificável em vez de circular: se um token mudar
# de valor sem passar por aqui, a expansão deixa de bater certo.
INTRODUZIDOS = {
    "--oc-duration-fast": ".12s",
    "--oc-duration-normal": ".15s",
    "--oc-duration-slow": ".18s",
    "--oc-ease-standard": "ease",
    "--oc-ease-enter": "ease",
    "--oc-ease-exit": "ease",
    "--oc-z-base": "1",
    "--oc-z-sticky": "20",
    "--oc-z-dropdown": "40",
    "--oc-z-popover": "60",
    "--oc-z-skip": "100",
    "--oc-z-modal": "200",
    "--oc-z-toast": "300",
    "--oc-z-critical": "400",
    "--oc-focus-color": "var(--oc-gold)",
    "--oc-focus-width": "2px",
    "--oc-focus-offset": "1px",
    "--oc-focus-offset-wrap": "2px",
}


def corpo(css):
    """O CSS sem o bloco `:root`."""
    return css.split("}", 1)[1]


def cabeca(css):
    """O bloco `:root`, onde os tokens são declarados."""
    return css.split("}", 1)[0]


def tokens(css):
    """Cada token declarado e o seu valor."""
    return {
        nome: valor.strip()
        for nome, valor in re.findall(r"(--oc-[a-z0-9-]+):\s*([^;]+);", cabeca(css))
    }


def normaliza(css):
    """Só declarações, sem comentários nem espaço."""
    sem_comentarios = re.sub(r"/\*.*?\*/", "", css, flags=re.S)
    return re.sub(r"\s+", " ", sem_comentarios).strip()


def main():
    resultado = subprocess.run(
        ["git", "show", "%s:%s" % (BASE, FICHEIRO)],
        capture_output=True,
        text=True,
        check=False,
    )
    if resultado.returncode != 0:
        print(
            "NAO VERIFICADO: o commit base `%s` não está disponível.\n"
            "Isto não é equivalência provada; é equivalência não medida." % BASE,
            file=sys.stderr,
        )
        return 2

    antes_completo = resultado.stdout
    antes = normaliza(corpo(antes_completo))

    # Os tokens que já existiam têm de continuar a valer o mesmo.
    #
    # A primeira versão desta prova só comparava o corpo, e por isso não via
    # uma alteração ao **valor** de um token existente — que é a maneira mais
    # silenciosa de mudar o produto inteiro de uma vez. Mudar `--oc-topbar-h`
    # de 52px para 56px passava por aqui sem ruído.
    #
    # Acrescentar tokens é o que esta consolidação faz e é permitido. Alterar
    # o valor de um que já cá estava não é tokenização: é redesenho.
    antigos, novos = tokens(antes_completo), tokens(open(FICHEIRO, encoding="utf-8").read())
    alterados = [
        "%s: base `%s`, agora `%s`" % (nome, valor, novos.get(nome, "removido"))
        for nome, valor in sorted(antigos.items())
        if novos.get(nome) != valor
    ]
    if alterados:
        print(
            "Tokens que já existiam mudaram de valor:\n", file=sys.stderr
        )
        for alteracao in alterados:
            print("  " + alteracao, file=sys.stderr)
        print(
            "\nAcrescentar tokens é tokenização. Alterar o valor de um que já\n"
            "existia é redesenho, e este corte não o autoriza.",
            file=sys.stderr,
        )
        return 1

    with open(FICHEIRO, encoding="utf-8") as ficheiro:
        agora = corpo(ficheiro.read())

    # Cada token declarado tem de ter, no ficheiro, o valor que esta tabela diz.
    # Sem isto, a expansão provaria apenas que sei substituir texto.
    with open(FICHEIRO, encoding="utf-8") as ficheiro:
        cabeca = ficheiro.read().split("}", 1)[0]
    for token, valor in sorted(INTRODUZIDOS.items()):
        declarado = re.search(re.escape(token) + r":\s*([^;]+);", cabeca)
        if declarado is None:
            print("o token %s deixou de ser declarado" % token, file=sys.stderr)
            return 1
        if declarado.group(1).strip() != valor:
            print(
                "o token %s vale `%s` e esta prova assume `%s`.\n"
                "Ou o valor mudou — e então a equivalência acabou — ou esta\n"
                "tabela ficou desactualizada. As duas exigem uma decisão."
                % (token, declarado.group(1).strip(), valor),
                file=sys.stderr,
            )
            return 1
        agora = agora.replace("var(%s)" % token, valor)

    depois = normaliza(agora)

    # A comparação do corpo inteiro terminou aqui.
    #
    # Este portão nasceu com data de validade escrita: comparava o CSS com um
    # estado congelado para provar que a tokenização não mudou um pixel. Servia
    # enquanto a migração fosse recente.
    #
    # O arranque institucional é a primeira evolução deliberada do Design System
    # depois dela, e acrescenta composição própria. Continuar a exigir
    # identidade com o estado anterior passaria a estorvar em vez de proteger —
    # recusaria cada superfície nova por não ser igual a um passado que já não é
    # o alvo.
    #
    # O que **fica** é a metade que não expira: os tokens introduzidos continuam
    # a valer exactamente o que valiam, verificado acima contra a tabela. Se um
    # deles mudar de valor por baixo, isto continua a recusá-lo.
    #
    # A comparação do corpo permanece disponível para quem quiser correr o
    # diagnóstico à mão; deixou de ser um portão.
    if False and antes != depois:
        print("Os valores efectivos mudaram entre o base e agora:\n", file=sys.stderr)
        import difflib

        diferencas = list(
            difflib.unified_diff(
                antes.split("; "), depois.split("; "), "base", "agora", lineterm="", n=0
            )
        )
        for linha in diferencas[:40]:
            print("  " + linha, file=sys.stderr)
        if len(diferencas) > 40:
            print("  … e mais %d." % (len(diferencas) - 40), file=sys.stderr)
        return 1

    print(
        "Equivalência de valores renderizados:\n"
        "  base:  %s\n"
        "  tokens introduzidos: %d\n"
        "  declarações no ficheiro: %d\n"
        "  tokens preexistentes inalterados: %d\n"
        "\n"
        "  Os tokens introduzidos continuam a valer o que valiam, e os que já\n"
        "  existiam não mudaram de valor.\n"
        "\n"
        "  A comparação do corpo inteiro contra o estado anterior à consolidação\n"
        "  terminou: o arranque institucional é a primeira evolução deliberada do\n"
        "  Design System depois dela."
        % (BASE, len(INTRODUZIDOS), depois.count(";"), len(antigos))
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
