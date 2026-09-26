# ADR-0016 — O manifesto de aplicação

- **Estado:** Accepted
- **Domínio:** Foundation
- **Impacto:** HIGH
- **Depende de:** [ADR-0014](0014-instance-profiles-and-application-activation.md) · [ADR-0015](0015-core-and-applications-boundary.md)
- **Data:** 2026-09-26

## Context

O registo de aplicações (§45-A) nasceu no Workspace, em dados Rust estáticos.
Desde a [ADR-0014](0014-instance-profiles-and-application-activation.md) o Core
também precisa de saber que aplicações existem, para validar a activação; desde a
[ADR-0015](0015-core-and-applications-boundary.md), que rotas da API são de cada
uma, para recusar as de uma aplicação inactiva — e esse mapa era mantido à mão
dentro do Core, ao lado de um registo no Workspace que dizia o resto. Duas fontes
para a mesma aplicação.

O programa de generalização pede um contrato permanente de aplicação: identidade,
versão, nome e descrição por chave i18n, ícone, categoria, rota, capacidades
pedidas, recursos, armazenamento, rede, capacidades de IA, disponibilidade, ciclo
de vida, saúde e política de fixação — sem construir ainda instalação de terceiros
nem loja, e sem assumir que as aplicações futuras são compiladas para sempre.

## Decision

**1. O manifesto vive nos contratos** (`ocinye_contracts::application`,
`ApplicationManifest`, `MANIFESTS`), partilhado pelo Core e pelo Workspace, sem
I/O. Declara, por aplicação:

| Campo | Para quê |
|---|---|
| `id`, classe (essencial/opcional) | identidade e activação |
| `category`, `route`, `name_key`, `description_key` | apresentação e lançamento |
| `api_prefixes` | as rotas da API que são **só** desta aplicação |
| `storage` | que armazenamento usa — sempre pelas fronteiras do Core |
| `network` | que rede externa precisa (hoje: só o Correio) |
| `ai_capabilities` | que capacidades de IA pede — capacidades, nunca modelos |
| `requested_resources` | que recursos governados consome |
| `health` | de onde vem a sua disponibilidade |
| `can_pin`, `default_pin` | política de fixação |
| versão do contrato, ciclo de vida | `1`; `Native` |

**2. Pedir não é receber.** Um manifesto declara o que a aplicação precisa; o
Core governa o que lhe dá. Nenhum campo concede armazenamento, rede, IA ou
segredos — e as aplicações nativas continuam a alcançar o Core só pelos serviços
de domínio, como sempre.

**3. Uma fonte.** O mapa de rotas da fronteira sai dos `api_prefixes`; a
categoria, a descrição e a política de fixação que o lançador mostra saem do
manifesto. O registo do Workspace fica com o que é só apresentação: o ecrã tipado
e as palavras de pesquisa. Um teste exige que o manifesto e o ecrã concordem na
rota e no nome, outro que cada prefixo declarado leve a uma rota real do Core.

**4. Hoje, só aplicações nativas.** `Lifecycle::Native` é o único ciclo de vida:
compilada no Ocinye OS, confiável. Aplicações da organização, conectores e
pacotes externos são classes futuras; o manifesto já as não impede, porque
nenhuma parte do Core decide sobre uma aplicação por outra coisa que não o seu
manifesto e a sua activação.

## Alternatives

**Manifestos em ficheiros (TOML, JSON) lidos no arranque.** Seria o caminho para
aplicações que não são compiladas; hoje trocaria a verificação do compilador por
um parser, sem nenhuma aplicação que o precise.

**Deixar o registo no Workspace e copiar para o Core.** Foi o que existiu durante
uma parte, e é exactamente a duplicação que isto fecha.

## Consequences

- Acrescentar uma aplicação é acrescentar um manifesto; o compilador e os testes
  apontam o que falta no Workspace e no Core.
- `GET /api/v1/instance/applications` devolve, com o estado de cada aplicação, o
  seu manifesto.
- Não há loja, instalação de terceiros nem execução de código não assinado. É
  arquitectura pronta, e não uma plataforma aberta.
