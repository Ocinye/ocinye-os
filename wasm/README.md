# Capacidades WebAssembly

Componentes que correm no
[Ocinye Capability Runtime](../crates/ocinye-capabilities/README.md).

## Porque estão fora do Cargo workspace

Compilam para `wasm32-wasip1`. Incluí-los no workspace do host forçaria uma build
WASM em cada `cargo build` nativo ([ADR-0005](../docs/adrs/0005-monorepo-cargo-workspace.md)).

## Construir

```bash
./scripts/build-capabilities.sh
```

Produz `target/wasm32-wasip1/release/<nome>.wasm`.

## Escrever uma capacidade

1. Uma crate com um binário que lê **stdin** e escreve **stdout**.
2. Um `manifest.json` a declarar identidade, inputs, outputs, permissões e
   limites.
3. Manter as dependências ao mínimo: uma capacidade é código não confiável dentro
   de um sandbox, e cada dependência é mais código lá dentro.
4. **Reportar o que não foi possível interpretar**, em vez de descartar em
   silêncio.

## O que uma capacidade não tem

Sem rede, sem filesystem do host, sem variáveis de ambiente, sem argumentos, sem
acesso à base de dados, sem segredos. Fuel, memória e tempo limitados e
verificados.

Pedir rede é **recusado**, não silenciosamente concedido.

## Existentes

| Capacidade | O quê |
|---|---|
| [`bibtex-import`](capabilities/bibtex-import) | Converte BibTeX em registos de fonte institucionais. Metadata apenas. |
