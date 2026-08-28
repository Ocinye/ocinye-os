# `ocinye-capabilities`

**Ocinye Capability Runtime** — capacidades institucionais isoladas em
WebAssembly/WASI.

## Finalidade

Executar transformações sobre artefactos institucionais — importar BibTeX,
extrair metadados, validar um dataset, processar um resultado — sob permissões
declaradas e limites de recursos.

## Porque WASM aqui, e não em todo o lado

Executar estas transformações no processo do Core dar-lhes-ia, por omissão, a
base de dados, o filesystem, a rede e os segredos. É esse o problema que este
crate resolve.

O WASM ganha o seu lugar aqui porque dá isolamento, portabilidade, permissões
explícitas e limites de recursos ao mesmo tempo — **não** porque o WASM está
disponível ([ADR-0501](../../docs/adrs/0501-capability-runtime-wasm.md), briefing §64).

É também por isso que o WASM **não** entra no Workspace nesta fase
([ADR-0600](../../docs/adrs/0600-leptos-workspace-runtime.md)).

## Deny by default

Uma capacidade recebe apenas o que o seu manifesto pede **e** a política
institucional aprova. Sem declaração recebe:

- **sem rede** — e pedir rede é *recusado*, não silenciosamente concedido, porque
  este host ainda não a sabe policiar;
- **sem filesystem do host** — apenas os inputs que o host injecta;
- **sem variáveis de ambiente, sem argumentos, sem directório preaberto**;
- fuel, memória e tempo de parede limitados.

## O contrato de invocação

Input em stdin, output em stdout, diagnóstico em stderr. Deliberadamente o
contrato mais simples que funciona entre linguagens: uma capacidade pode ser
escrita em Rust hoje e noutra coisa amanhã sem o host mudar.

## WASM não é segurança mágica

O sandbox é uma camada, não a política. Continuam a aplicar-se: validação de
input, autorização antes da invocação, limites de recursos e proveniência do
resultado.

## Âmbito nesta fase

Manifesto, modelo de permissões, host Wasmtime com limites, e **uma** capacidade
de exemplo. Não é um marketplace de plugins.

**Não implementado, e declarado como tal:** verificação de assinatura e de
checksum do componente. O campo existe no manifesto; o host ainda não o verifica.

## Execução e testes

```bash
./scripts/build-capabilities.sh     # produz o componente wasm32-wasip1
cargo test -p ocinye-capabilities
```

Os testes de integração correm contra o componente real e provam que:

- uma capacidade real corre e devolve output estruturado;
- uma capacidade sem fuel é **efectivamente parada**, não apenas configurada;
- uma capacidade não vê variáveis de ambiente do host;
- input malformado é reportado pela capacidade, não por um crash.

Sem o artefacto, os testes **falham** com o comando a executar. Uma versão
anterior saltava-os, e como o cargo esconde o output dos testes que passam, um
artefacto no directório errado fez os quatro saltarem e reportarem sucesso — com
dois defeitos reais a sobreviverem atrás desse verde.
