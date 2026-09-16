# syntax=docker/dockerfile:1.7
#
# As três imagens do Ocinye OS, de uma única construção.
#
# # Porque um Dockerfile e não três
#
# Porque os três binários saem da mesma workspace e partilham quase todas as
# dependências. Três ficheiros compilariam `sqlx`, `axum` e `leptos` três vezes,
# e produziriam três árvores de dependências que podiam divergir sem que nada o
# dissesse — o que é exactamente o contrário de um deployment reproduzível.
#
# O binário escolhe-se com `--build-arg BIN=`, e a etapa de construção é a mesma
# para todos: o que sai são três imagens de um só `cargo build`.
#
# # Porque a versão do compilador não está escrita aqui
#
# Porque está no `rust-toolchain.toml`, e é lá que pertence. Escrevê-la também
# aqui criava dois sítios para a mesma verdade, e o dia em que discordassem a
# imagem seria construída por um compilador que a árvore não conhece.

FROM rust:1.98-bookworm AS builder

WORKDIR /build

# As dependências de sistema que a árvore precisa para compilar.
RUN apt-get update \
 && apt-get install -y --no-install-recommends pkg-config libssl-dev \
 && rm -rf /var/lib/apt/lists/*

COPY . .

ARG BIN
# `sharing=locked`, e não o `shared` por omissão.
#
# As três imagens constroem em paralelo e partilhavam este cache. Duas
# desempacotavam o mesmo crate ao mesmo tempo e a segunda encontrava o
# `.cargo-ok` da primeira meio escrito:
#
#     failed to unpack package `async-channel`
#     File exists (os error 17)
#
# Um cache partilhado sem tranca não é um cache: é uma corrida que só se
# manifesta quando há paralelismo — e por isso nunca aparece em máquina de
# quem constrói uma imagem de cada vez.
#
# O `target` leva `id` por binário: são três compilações diferentes, e
# partilhá-lo faria uma invalidar os artefactos da outra a cada construção.
RUN --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,target=/build/target,id=ocinye-target-${BIN},sharing=locked \
    cargo build --release --locked --bin "${BIN}" \
 && cp "target/release/${BIN}" /usr/local/bin/ocinye-binario

# ── A imagem que corre ───────────────────────────────────────────────────
#
# `debian:bookworm-slim` e não `scratch`: os binários ligam a `libssl` e a
# `libgcc`, e uma imagem sem elas não arranca. Distroless seria mais pequena,
# mas o que se ganha em megabytes perde-se no dia em que é preciso entrar lá
# dentro para perceber porque é que o Core não fala com o PostgreSQL.
FROM debian:bookworm-slim AS runtime

# `poppler-utils` traz o `pdftoppm`, que o worker usa para rasterizar a primeira
# página de um PDF numa imagem — sem executar o JavaScript embutido no documento
# (o poppler é um renderizador, não um motor de scripts; spec §76). Vai em todas
# as imagens porque a construção é uma só; só o worker o invoca.
RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates libssl3 curl poppler-utils \
 && rm -rf /var/lib/apt/lists/* \
 && useradd --system --create-home --uid 10001 ocinye

COPY --from=builder /usr/local/bin/ocinye-binario /usr/local/bin/ocinye

# Os estáticos do Workspace. Vão em todas as imagens porque custam kilobytes e
# porque uma imagem que os não tivesse falharia só quando alguém abrisse uma
# página — que é o pior momento para descobrir que faltam.
COPY --from=builder /build/apps/workspace/static /srv/ocinye/static

USER ocinye
WORKDIR /srv/ocinye

ENTRYPOINT ["/usr/local/bin/ocinye"]

# ── A imagem do conversor descartável ─────────────────────────────────────
#
# O que corre **dentro** de cada contentor de conversão, e nada mais. Quase
# vazio de propósito: só o `ocinye-convert` e as ferramentas de conversão. Sem
# rede, sem segredos, sem o resto do sistema — é a caixa que uma vulnerabilidade
# de parsing de conteúdo hostil encontra, em vez do worker (ADR-0609). O runner
# corre-a com `--user 65534`, `--read-only`, `--network=none`, `--cap-drop=ALL` e
# os restantes tectos; a imagem não precisa de mais do que a ferramenta.
#
# Constrói-se com `--build-arg BIN=ocinye-convert`.
FROM debian:bookworm-slim AS converter
RUN apt-get update \
 && apt-get install -y --no-install-recommends poppler-utils \
 && rm -rf /var/lib/apt/lists/*
COPY --from=builder /usr/local/bin/ocinye-binario /usr/local/bin/ocinye-convert
ENTRYPOINT ["/usr/local/bin/ocinye-convert"]

# ── A imagem do Conversion Runner ─────────────────────────────────────────
#
# O sidecar que detém a autoridade de criar contentores. Traz **só o cliente**
# do Docker — copiado da imagem oficial, um binário estático — para falar com o
# socket do host montado; não corre daemon nenhum. Corre como root porque é o
# dono do socket, e a fronteira de confiança é o socket, não o uid — é a única
# imagem onde o socket é montado (ADR-0609).
#
# Constrói-se com `--build-arg BIN=ocinye-conversion-runner`.
FROM debian:bookworm-slim AS conversion-runner
RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates \
 && rm -rf /var/lib/apt/lists/*
COPY --from=docker:27-cli /usr/local/bin/docker /usr/local/bin/docker
COPY --from=builder /usr/local/bin/ocinye-binario /usr/local/bin/ocinye-conversion-runner
WORKDIR /srv/ocinye
ENTRYPOINT ["/usr/local/bin/ocinye-conversion-runner"]
