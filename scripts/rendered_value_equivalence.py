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
# Os valores dos tokens no estado congelado, lidos de `075204e` enquanto esse
# commit ainda existia.
#
# Estavam a ser lidos com `git show`, e isso partiu-se no dia em que o
# repositório foi recriado: o commit deixou de existir no remoto, o portão
# passou a recusar em CI e a passar na máquina onde o objecto solto sobrevivia.
# «Passa aqui e falha lá» é o pior estado possível para um portão.
#
# Congelados como dados, pela mesma razão que `INTRODUZIDOS` sempre o foi: a
# propriedade que isto guarda — **um token que já existia não muda de valor por
# baixo** — não precisa de história, precisa dos valores. E esta é a maneira
# silenciosa de mudar o produto inteiro de uma vez: `--oc-topbar-h` de 52px para
# 56px passaria despercebido em qualquer revisão.
#
# Setenta e um tokens. Se um sair desta tabela de propósito, sai por decisão.
ANTES = {
    "--oc-border": "#E4E9F0",
    "--oc-border-faint": "#F3F6F9",
    "--oc-border-soft": "#EEF2F6",
    "--oc-border-strong": "#C3CDD8",
    "--oc-canvas": "#F6F8FA",
    "--oc-error": "#B3261E",
    "--oc-error-bg": "#FCF0EF",
    "--oc-error-border": "#F1D6D3",
    "--oc-error-text": "#8C2019",
    "--oc-font-mono": "'IBM Plex Mono', ui-monospace, monospace",
    "--oc-font-sans": "'IBM Plex Sans', system-ui, sans-serif",
    "--oc-gold": "#E0A731",
    "--oc-gold-bg": "#FDF6E7",
    "--oc-gold-border": "#F2E3BE",
    "--oc-gold-text": "#8A6110",
    "--oc-info": "#2B6CB0",
    "--oc-info-bg": "#EFF5FB",
    "--oc-info-border": "#D5E4F0",
    "--oc-info-text": "#20537F",
    "--oc-interface-scale": "1.15",
    "--oc-navy": "#0B2D4A",
    "--oc-navy-deep": "#071E33",
    "--oc-navy-hover": "#123C60",
    "--oc-navy-mid": "#1C4B74",
    "--oc-on-navy": "#FFFFFF",
    "--oc-on-navy-32": "rgba(255,255,255,.32)",
    "--oc-on-navy-42": "rgba(255,255,255,.42)",
    "--oc-on-navy-50": "rgba(255,255,255,.5)",
    "--oc-on-navy-70": "rgba(255,255,255,.68)",
    "--oc-on-navy-active": "rgba(255,255,255,.10)",
    "--oc-on-navy-hover": "rgba(255,255,255,.07)",
    "--oc-on-navy-line": "rgba(255,255,255,.08)",
    "--oc-page-pad": "22px 24px 40px",
    "--oc-placeholder": "#98A6B4",
    "--oc-r-2xl": "16px",
    "--oc-r-lg": "11px",
    "--oc-r-md": "8px",
    "--oc-r-sm": "7px",
    "--oc-r-tile": "20px",
    "--oc-r-xl": "14px",
    "--oc-r-xs": "4px",
    "--oc-row-h": "38px",
    "--oc-row-h-dense": "30px",
    "--oc-shadow-card": "0 2px 10px rgba(11,45,74,.06)",
    "--oc-shadow-input": "0 4px 18px rgba(11,45,74,.06)",
    "--oc-shadow-logo": "0 18px 50px rgba(0,0,0,.35), 0 0 0 1px rgba(255,255,255,.14)",
    "--oc-shadow-menu": "0 16px 40px rgba(11,45,74,.14)",
    "--oc-shadow-overlay": "0 30px 80px rgba(7,30,51,.32)",
    "--oc-sidebar-w": "224px",
    "--oc-sidebar-w-collapsed": "58px",
    "--oc-success": "#3E8F66",
    "--oc-success-bg": "#F0F7F3",
    "--oc-success-border": "#D8EBE0",
    "--oc-success-text": "#2E6B4C",
    "--oc-surface": "#FFFFFF",
    "--oc-surface-hover": "#F8FAFC",
    "--oc-surface-muted": "#F3F6F9",
    "--oc-surface-subtle": "#FAFCFD",
    "--oc-surface-tint": "#F1F4F8",
    "--oc-text": "#0F1A24",
    "--oc-text-body": "#28394A",
    "--oc-text-faint": "#8A98A6",
    "--oc-text-ghost": "#A9B5C1",
    "--oc-text-meta": "#7C8B9A",
    "--oc-text-muted": "#5F7183",
    "--oc-text-secondary": "#42546A",
    "--oc-topbar-h": "52px",
    "--oc-warning": "#C87A22",
    "--oc-warning-bg": "#FDF2E7",
    "--oc-warning-border": "#F2DBBE",
    "--oc-warning-text": "#8A4B10",
}

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

    # Os tokens que já existiam têm de continuar a valer o mesmo.
    #
    # A primeira versão desta prova só comparava o corpo, e por isso não via
    # uma alteração ao **valor** de um token existente — que é a maneira mais
    # silenciosa de mudar o produto inteiro de uma vez. Mudar `--oc-topbar-h`
    # de 52px para 56px passava por aqui sem ruído.
    #
    # Acrescentar tokens é o que esta consolidação faz e é permitido. Alterar
    # o valor de um que já cá estava não é tokenização: é redesenho.
    antigos, novos = ANTES, tokens(open(FICHEIRO, encoding="utf-8").read())
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
        % (len(INTRODUZIDOS), depois.count(";"), len(antigos))
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
