#!/usr/bin/env bash
# Os números da Secção 1 do `CLAUDE.md`, derivados da árvore.
#
# Existe porque a Secção 1 é a única parte daquele ficheiro que descreve o
# estado real, e um número escrito à mão envelhece sem que nada falhe. Já houve
# neste repositório três contagens em circulação para a mesma coisa.
#
# Só lê. Não escreve em nada versionado — é a regra do `CLAUDE.md` §59, e um
# verificador que toca no que observa deixa de poder ser confiado.
set -euo pipefail
cd "$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

python3 - <<'PY'
import pathlib, re

def ler(padrao):
    return [p.read_text() for p in pathlib.Path().glob(padrao)]

# ── O Core: caminhos e operações HTTP ────────────────────────────────────
#
# Um `.route(...)` pode atravessar várias linhas, e dentro dele vivem um
# caminho e um ou mais métodos — `get(listar).post(criar)` são duas operações
# no mesmo caminho. Contar linhas com `.route(` dava caminhos e chamava-lhes
# operações; contar `get(` dava também os `payload.get(` do resto do ficheiro.
rotas = "\n".join(ler('services/core-server/src/routes/*.rs'))

def argumentos_de_route(fonte):
    """O interior de cada `.route(...)`, fechado a contar parênteses.

    Um lookahead por `.route`/`.merge` perdia a última rota de cada router —
    três, quando isto foi escrito — e ninguém dava por isso porque um número
    que encolhe continua a parecer um número. Contar parênteses não tem esse
    fim de linha.
    """
    marca = '.route('
    inicio = 0
    while (i := fonte.find(marca, inicio)) != -1:
        j, nivel, dentro_de_texto, escapado = i + len(marca), 1, False, False
        while j < len(fonte) and nivel:
            c = fonte[j]
            if escapado:
                escapado = False
            elif c == '\\':
                escapado = True
            elif c == '"':
                dentro_de_texto = not dentro_de_texto
            elif not dentro_de_texto:
                nivel += (c == '(') - (c == ')')
            j += 1
        yield fonte[i + len(marca):j - 1]
        inicio = j

caminhos, operacoes = set(), 0
for argumento in argumentos_de_route(rotas):
    caminho = re.match(r'\s*"([^"]+)"\s*,(.*)', argumento, re.S)
    if not caminho:
        continue
    caminhos.add(caminho.group(1))
    operacoes += len(re.findall(r'(?<![A-Za-z0-9_])(get|post|put|patch|delete)\(', caminho.group(2)))

print(f"caminhos-core        {len(caminhos)}")
print(f"operacoes-core       {operacoes}")

# ── O Workspace: ecrãs ───────────────────────────────────────────────────
#
# Um ecrã é um caminho que um membro abre. `KNOWN_PATHS` inclui destinos de
# `POST` que nunca se abrem, por isso a contagem sai do router e conta os
# caminhos que respondem a `GET`.
ws = pathlib.Path('apps/workspace/src/routes.rs').read_text()
ecras = {m.group(1) for m in re.finditer(r'\.route\(\s*"([^"]+)"\s*,\s*get\(', ws, re.S)}
print(f"ecras-workspace      {len(ecras)}")

# ── Persistência ─────────────────────────────────────────────────────────
sql = "\n".join(ler('migrations/*.sql'))
print(f"migrations           {len(list(pathlib.Path('migrations').glob('*.sql')))}")
print(f"tabelas              {len(re.findall(r'^CREATE TABLE', sql, re.M))}")

# ── Governação ───────────────────────────────────────────────────────────
acesso = pathlib.Path('crates/ocinye-contracts/src/access.rs').read_text()
permissoes = re.search(r'pub const fn all\(\) -> \[Self; (\d+)\]\s*\{\s*\[\s*Self::OrganisationView',
                       acesso)
print(f"permissoes           {permissoes.group(1) if permissoes else '?'}")

# ── Testes que exigem PostgreSQL ─────────────────────────────────────────
#
# Não é o total da suite — esse é o resultado de uma corrida, e sai do
# `verify.sh`. É quantos testes não se conseguem exercer sem base de dados, que
# é um facto da árvore: os ficheiros que leem `OCINYE_TEST_DATABASE_URL`.
com_postgres = 0
for ficheiro in list(pathlib.Path().rglob('tests/*.rs')):
    if 'target' in ficheiro.parts:
        continue
    fonte = ficheiro.read_text()
    if 'OCINYE_TEST_DATABASE_URL' in fonte:
        com_postgres += len(re.findall(r'#\[(?:tokio::)?test\]', fonte))
print(f"testes-com-postgres  {com_postgres}")

# ── Funções de teste escritas na árvore ──────────────────────────────────
#
# **Não** é o número de testes que correram: esse é o resultado de uma corrida,
# e conta cada alvo em que um teste é compilado. Este é um facto da árvore —
# quantas funções de teste existem escritas — e existe porque a Secção 1
# carregava um total de corrida mantido à mão, que derivou três vezes numa
# única sessão sem que nada falhasse. É exactamente o defeito que a própria
# Secção 1 avisa.
funcoes = 0
for ficheiro in pathlib.Path().rglob('*.rs'):
    if 'target' in ficheiro.parts:
        continue
    funcoes += len(re.findall(r'#\[(?:tokio::)?test\]', ficheiro.read_text(errors='replace')))
print(f"funcoes-de-teste     {funcoes}")

print(f"adrs                 {len(list(pathlib.Path('docs/adrs').glob('[0-9]*.md')))}")
print(f"runbooks             {len([p for p in pathlib.Path('docs/runbooks').glob('*.md') if p.name != 'README.md'])}")
print(f"readmes              {len([p for p in pathlib.Path().rglob('README.md') if 'target' not in p.parts and '.git' not in p.parts])}")
PY
