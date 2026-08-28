# Desenvolvimento

Do zero a uma stack a correr.

## Requisitos

- **Docker** com Compose.
- **rustup**. O toolchain (1.98.0) e o alvo `wasm32-wasip1` são instalados
  automaticamente a partir de [`rust-toolchain.toml`](../../rust-toolchain.toml).
- **`sqlx-cli`**, para as migrations:
  `cargo install sqlx-cli --no-default-features --features postgres,rustls`

## Passo a passo

### 1. Configuração

```bash
cp .env.example .env
```

Nada em `.env.example` é secreto. Os valores marcados `CHANGE_ME` devem ser
alterados mesmo em desenvolvimento, para que nunca se tornem hábito.

### 2. Infraestrutura

```bash
docker compose -f infra/compose/docker-compose.yml up -d
```

Levanta PostgreSQL 17 com pgvector (porta 5442), Redis (6380), MinIO (9000, consola
9001). O `minio-init` cria o bucket e deixa-o **privado**.

Portas fora do habitual de propósito: colidir com outro PostgreSQL local é a
primeira coisa que acontece a quem já desenvolve noutro projecto.

**Ligadas a `127.0.0.1`**, não a todas as interfaces. Estes serviços correm com
as credenciais que estão no `.env.example`; publicá-los em `0.0.0.0` numa rede
de que não se é dono é oferecer uma base de dados com password conhecida a quem
a queira. `POSTGRES_BIND`, `REDIS_BIND` e `MINIO_BIND` mudam isto, e mudá-las
precisa de uma razão (`CLAUDE.md` §56).

### 3. Migrations

```bash
export DATABASE_URL="postgres://ocinye:ocinye_dev_only@localhost:5442/ocinye"
sqlx migrate run --source migrations
```

O Core também as aplica ao arrancar. Uma migration falhada **impede o arranque**:
o serviço recusa correr contra um schema que não compreende.

### 4. Serviços

Em três terminais:

```bash
set -a && source .env && set +a

cargo run --bin ocinye-core-server    # http://localhost:8080
cargo run --bin ocinye-worker
cargo run --bin ocinye-workspace      # http://localhost:8090
```

### 5. Verificar

```bash
curl -s http://localhost:8080/health
curl -s http://localhost:8080/ready | python3 -m json.tool
```

`/ready` sonda a base de dados com uma query real e reporta o storage, o IdP e o
Intelligence Plane. Com a configuração de exemplo, a IA aparece como
indisponível — **isso é o estado correcto**, não uma avaria.

### 6. Primeiro membro

O Ocinye Core não cria membros: a filiação vem de um convite, e o acesso começa
no primeiro início de sessão verificado.

A sequência é: um `platform_admin` cria unidades e convida uma pessoa; a pessoa
aceita o convite; a pessoa autentica-se no Core com a credencial temporária
verificado à pessoa e a filiação passa a activa.

Para arrancar a primeira vez é preciso conceder `platform_admin` a alguém
directamente na base de dados — não há outra forma de criar o primeiro
administrador, e criar um por omissão seria uma porta aberta:

```sql
-- Só na primeira instalação, e apenas depois de a pessoa existir.
INSERT INTO person_roles (person_id, role, granted_reason)
VALUES ('<person-uuid>', 'platform_admin', 'bootstrap da instalação');
```

Isto está deliberadamente fora da API.

## Capacidades WebAssembly

Algumas operações do Core correm dentro do Capability Runtime, e precisam do
componente construído:

```bash
./scripts/build-capabilities.sh
```

`./scripts/verify.sh` já o faz antes dos testes. Quem correr uma suite à mão —
`ocinye-core --test bibliography`, `ocinye-core-server --test bibliography_http`
ou as viagens de browser — corre isto primeiro; as três dizem-no ao arrancar se
faltar.

## Comandos

```bash
cargo fmt --all                       # formatar
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace                # testes que não precisam de infraestrutura

./scripts/build-capabilities.sh       # compila as capacidades para wasm32-wasip1
cargo test -p ocinye-capabilities     # sandbox WASM contra componente real

# Autorização contra base de dados real
psql -h localhost -p 5442 -U ocinye -d postgres -c 'CREATE DATABASE ocinye_test'
OCINYE_TEST_DATABASE_URL="postgres://ocinye:ocinye_dev_only@localhost:5442/ocinye_test" \
  cargo test -p ocinye-core --test authorization
```

### Sem Docker

A suite de autorização só precisa de um PostgreSQL com `pgvector`; não precisa
do Compose. Contra uma instância local (Postgres.app 18.1, pgvector 0.8.1):

```bash
psql -h 127.0.0.1 -p 5432 -d postgres \
  -c 'DROP DATABASE IF EXISTS ocinye_test' -c 'CREATE DATABASE ocinye_test'
OCINYE_TEST_DATABASE_URL="postgres://$USER@127.0.0.1:5432/ocinye_test" \
  cargo test -p ocinye-core --test authorization
```

**Deixa a base vazia.** O harness aplica as migrations ele próprio e regista-as
em `_sqlx_migrations`; aplicá-las antes por `psql` faz o `sqlx` tentar recriar as
tabelas e a suite falha inteira com `relation "organisations" already exists`.

## Onde mexer

| Quero… | Vou a… |
|---|---|
| Acrescentar um conceito institucional | `crates/ocinye-contracts` |
| Mudar quem pode fazer o quê | `crates/ocinye-domain/src/policy` — **e o teste de equivalência** |
| Mudar um ciclo de vida | `crates/ocinye-domain/src/workflow` |
| Acrescentar comportamento a um domínio | `crates/ocinye-core/src/modules/<domínio>/service.rs` |
| Acrescentar um endpoint | `services/core-server/src/routes/` |
| Mudar o schema | Nova migration em `migrations/`. Nunca alterar uma existente. |
| Mudar a interface | `apps/workspace/src/views/` |

## Regras que o repositório impõe

- **Toda a mudança de estado passa por um serviço.** Um handler HTTP nunca chama
  um repositório.
- **Toda a alteração de schema é uma migration nova.** Migrations aplicadas não
  se editam.
- **A política e o filtro SQL mudam juntos.** O teste de equivalência falha se
  não mudarem.
- **`CURRENT` e `PLANNED` são distintos.** Documentação que descreva roadmap como
  realidade é um defeito.
- **A escrita institucional acontece no Ocinye Core.** Nenhum outro crate escreve
  no estado institucional; as duas excepções de infraestrutura estão declaradas
  em `scripts/mutation_authority.py`, com a razão.
- **Toda a dependência de produção tem um consumidor.** Uma dependência que o
  código não menciona é lixo, uma dependência de teste declarada como de
  produção, ou uma intenção por cumprir.
- **Toda a configuração suportada tem leitor e documentação.** Uma variável
  documentada que ninguém lê promete um controlo que não existe.

## Disciplina de evidência

Estas regras governam o que conta como prova neste repositório. São a razão pela
qual `./scripts/verify.sh` recusa candidatos que, à primeira vista, parecem
verdes.

**Uma afirmação sobre o sistema vale o que vale a evidência que a acompanha.**
Números sobre o sistema — contagens de testes, de operações, de dependências —
derivam-se da fonte, e não se escrevem de memória.

**Um teste que não correu não é evidência.** Um teste que retorna cedo, por falta
de pré-condição ou por um erro silenciado no arranque, é contado como passado e
nunca aparece como salto. Por isso as suites críticas têm um contrato de
enumeração: `scripts/test-enumeration.sh` exige que os testes esperados tenham
sido descobertos, tenham corrido e tenham passado, e as viagens de browser
emitem uma marca positiva no ponto em que já não é possível sair sem correr.

**Um verificador que não observou nada não é evidência alguma.** Quatro estados,
e só um é verde:

| | |
|---|---|
| `PASS` | a propriedade foi observada e está satisfeita |
| `FAIL` | a propriedade foi observada e está violada |
| `INVALID` | o verificador, a compilação ou a fixture falhou |
| `NOT_RUN` | a verificação não chegou a correr |

`INVALID` nunca se converte em `PASS` por ter aparecido a palavra certa no
stdout. O gate `Verification Harness Integrity` verifica esta propriedade nos
próprios verificadores, e corre antes de todos os outros.

**O resultado canónico de uma execução é o seu código de saída**, e não a última
linha impressa. Um comando que imprime uma falha seguido de um `echo` que declara
sucesso produz um relatório falso; capture `$?` e decida por ele.

**Uma defesa nova prova-se partindo-a de propósito.** Uma reversão só é evidência
quando cumpre a sequência inteira:

```text
controlo positivo verde
→ injectar exactamente uma violação
→ o alvo pretendido foi alterado
→ o guarda esperado executou
→ o diagnóstico esperado apareceu
→ não existe outra falha que explique a recusa
→ o controlo positivo continua válido
```

Duas consequências que custaram caro a aprender: a reversão tem de usar a mesma
forma sintáctica que a implementação real usa — provar que um guarda de cores
recusa `#RRGGBB` não prova nada se o código escrever `rgb(...)` — e uma reversão
que bloqueia é uma observação `INVALID`, não uma propriedade que falhou. Toda a
verificação sujeita a bloqueio precisa de um limite de tempo.

**A verificação é de leitura.** Se `verify.sh` alterar um ficheiro versionado,
falha — mesmo que o restaure a seguir.
