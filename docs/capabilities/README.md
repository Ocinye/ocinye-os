# Capability Plane

Ver [ADR-0501](../adrs/0501-capability-runtime-wasm.md) para a decisão.
Implementação: [`crates/ocinye-capabilities`](../../crates/ocinye-capabilities/README.md).

## O problema que resolve

Capacidades institucionais — importar BibTeX, extrair metadados, validar um
dataset, processar um resultado — vão crescer, virão de vários autores e correrão
sobre dados classificados.

Executá-las no processo do Core dar-lhes-ia, por omissão, a base de dados, o
filesystem, a rede e os segredos.

## Manifesto

```json
{
  "identifier": "ocinye.bibtex-import",
  "name": "BibTeX import",
  "version": "0.1.0",
  "description": "Parses a BibTeX file into institutional source records.",
  "inputs": ["text/x-bibtex"],
  "outputs": ["application/json"],
  "network": { "mode": "none" },
  "filesystem": { "mode": "none" },
  "limits": { "fuel": 200000000, "memory_bytes": 33554432, "wall_time_ms": 5000 },
  "runtime": "wasm32-wasip1"
}
```

O host concede a intersecção do que é declarado com o que a política aprova.

## Capability-based security

Uma capacidade **pode**: ler o input que o host lhe injecta; escrever output;
usar o fuel, a memória e o tempo concedidos.

Uma capacidade **não pode**: consultar o PostgreSQL; ler outros workspaces;
aceder à rede; ler segredos; ler o filesystem do host; ver variáveis de ambiente.

Pedir rede é **recusado**, não silenciosamente concedido: uma capacidade nunca
deve acreditar que tem acesso que o host não sabe policiar.

## Limites, verificados

| Limite | Como é imposto |
|---|---|
| Fuel | `Config::consume_fuel` — bounda computação independentemente da carga da máquina |
| Tempo de parede | `epoch_interruption` com watchdog — para uma capacidade que nunca ceda |
| Memória | `StoreLimits` |
| Instâncias e tabelas | `StoreLimits` |

Um teste de integração prova que uma capacidade com **1 unidade de fuel é
efectivamente parada** — o limite é imposto, não apenas configurado. Outro prova
que uma variável de ambiente do host não é visível dentro do sandbox.

## Contrato de invocação

Input em stdin, output em stdout, diagnóstico em stderr. Deliberadamente o
contrato mais simples que funciona entre linguagens: uma capacidade pode ser
escrita em Rust hoje e noutra coisa amanhã sem o host mudar.

## WASM não é segurança mágica

O sandbox é uma camada. Continuam a aplicar-se: validação de input, autorização
antes da invocação, limites de recursos, proveniência do resultado e trust
boundaries explícitas.

## Exemplo

[`wasm/capabilities/bibtex-import`](../../wasm/capabilities/bibtex-import) —
75 KB compilados, deliberadamente sem dependências pesadas: uma capacidade é
código não confiável dentro de um sandbox, e cada dependência é mais código lá
dentro.

Reporta as entradas que não conseguiu interpretar em vez de as descartar.

## Não implementado

- Verificação de assinatura e de checksum. O campo existe no manifesto; o host
  não o verifica.
- Política de rede.
- Distribuição de capacidades entre nós.
- Um SDK. A visão está documentada; o SDK não existe.
- Marketplace. Não é objectivo.
