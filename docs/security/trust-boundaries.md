# Fronteiras de confiança do Ocinye OS

> Parte 12 da generalização. Complementa o [modelo de ameaças](../threat-model/README.md),
> que diz **o que** pode correr mal; esta página diz **quem confia em quem**, por
> que credencial, e que teste o prova. A generalização não pode alargar confiança
> sem que se veja: cada linha abaixo aponta para a evidência que falha se a
> fronteira ceder.

## Quem confia em quem

| Componente | Confia em | Autenticação | Autorização | Segredos que vê | Rede | Ficheiros |
|---|---|---|---|---|---|---|
| **Browser** | ninguém | — | — | a sessão BFF, num cookie `HttpOnly`·`Secure`·`SameSite` | só o proxy | — |
| **Proxy (nginx)** | — | TLS | nenhuma: encaminha | o certificado | 443/80 públicos; o Workspace na rede interna | a sua config, só leitura |
| **Workspace** | o Core | sessão do membro (BFF) | **nenhuma decisão**: o Core decide, o Workspace só esconde | o token de sessão, no servidor | o Core pela rede interna (`local-container`) | estáticos |
| **Core** | a base, o armazenamento, o Redis | sessão opaca (Argon2id, MFA para privilegiados); credencial de máquina para nós | RBAC + ABAC fail-closed, autoridade reestabelecida no momento do efeito | a raiz de selagem; credenciais da base e do armazenamento | sem porta pública | nenhum do anfitrião |
| **PostgreSQL / Redis / armazenamento** | o Core | palavra-passe/credencial gerada no anfitrião | por conta | — | só a rede interna | os seus volumes |
| **Worker** | a base, o Conversion Runner | as mesmas credenciais do Core | trabalha sobre o que o outbox lhe dá | as do Core | interna | nenhum |
| **Conversion Runner** | o Docker do anfitrião | — | só cria contentores descartáveis endurecidos | — | interna | o spool de conversão |
| **Conversores** | ninguém | — | — | nenhum | **sem rede** | só o ficheiro que converte |
| **Node Agent** | o Core | token de enrolamento, depois credencial de máquina | só o seu próprio nó | a sua credencial | só para o Core | o seu estado |
| **AI Gateway → fornecedor** | o fornecedor recebe o que a política deixa | credencial aberta da Autoridade de Segredos, por pedido | política antes de preferência (ADR-0311) | a credencial, só durante a chamada | https para externos; sem redireccionamentos | — |
| **Fornecedor de IA externo** | ninguém confia nele | — | nunca recebe acima do tecto externo nem do modelo | nunca vê a credencial de outro | — | — |
| **Runtime de modelo local** | idem, como um externo | idem | idem | idem | a rede da Instância | — |
| **Agentes** | nada: actuam por capacidades | a identidade de quem os usa | intersecção actor ∩ agente ∩ recurso (ADR-0302) | **nenhum** | **nenhuma** | **nenhum** |
| **Conectores futuros** | — | por segredo de âmbito `connector` | a definir por conector | só o seu segredo | a definir | — |
| **Autoridade de Segredos** | a raiz de selagem | — | só um serviço do Core, no âmbito do segredo | valores em claro só dentro da chamada que os usa | — | — |

Três invariantes atravessam a tabela:

1. **Menor privilégio.** O único componente com o socket do Docker é o Conversion
   Runner ([ADR-0609](../adrs/0609-disposable-conversion-isolation.md)); o Core não
   tem porta pública; os agentes não têm base, ficheiros, shell, rede nem segredos.
2. **Um nó comprometido não é a Instância.** Um nó autentica-se com credencial de
   máquina própria e só fala do seu estado; não recebe sessões, segredos nem a raiz
   de selagem.
3. **Uma aplicação comprometida não é o Core.** A fronteira Core/aplicações recusa
   a API de uma aplicação inactiva e contém um `panic`
   ([ADR-0015](../adrs/0015-core-and-applications-boundary.md)); a autoridade é
   sempre do Core, nunca da aplicação.

## Os ataques, e o que os prova

| Ataque | Resultado exigido | Evidência |
|---|---|---|
| **Acesso à API sem sessão** | cada uma das operações do Core recusa com `401`; as públicas têm nome e razão | `services/core-server/tests/unauthenticated_sweep_http.rs` — o inventário sai do código das rotas e tem de igualar o `repository-facts.sh`; provado por reversão (uma rota aberta é apanhada) |
| **Contornar uma rota** | uma aplicação inactiva é recusada a qualquer cliente; uma sessão sem segundo factor não alcança a Administração | `application_boundary_http.rs`; viagem `o_desafio_de_mfa_precede_a_autoridade_privilegiada` |
| **Ler um segredo** | não há rota que devolva um valor; um membro não os lista; o valor em claro não está em lado nenhum da base | `secrets_http.rs` (varredura de todas as tabelas, provada por reversão) |
| **Armazenamento de outro membro** | recusado pela posse, no Core | `crates/ocinye-core/tests/resource_storage.rs` — `meus_ficheiros_e_o_espaco_do_dono_e_a_posse_e_a_autoridade`, `mover_para_a_pasta_de_outra_pessoa_e_recusado` |
| **Nó falso** | enrolamento recusado, e o Core continua a responder | `node_fabric_http.rs` |
| **Escalar capacidade** | não existe rota que corra uma capacidade por identificador; um agente nunca alarga quem o usa; uma capacidade fora da definição é recusada; um plano não se alcança por conhecer o seu id; autoridade revogada não executa; instruções injectadas não viram plano | `agentic.rs` — `an_agent_never_widens_the_person_using_it`, `a_capability_outside_the_agent_definition_is_refused`, `injected_instructions_cannot_become_a_plan`; `agentic_lifecycle.rs` — `another_actor_cannot_reach_a_plan_by_knowing_its_identifier`, `a_plan_cannot_run_on_authority_captured_before_it_was_revoked` |
| **Contornar a política de IA externa** | um pedido confidencial nunca chega a um externo; `NONE` fecha a IA externa; externo só por `https` | `ai_routing_http.rs`, `ai_providers_http.rs` — o externo aceitaria tudo, e a sua saúde fica `unknown`; provado por reversão |

## O que ainda não é fronteira

- **Rede entre anfitriões.** Um nó noutro anfitrião fala com o Core pela rede que
  existir; WireGuard é `PLANNED` (§24).
- **Conectores.** O âmbito de segredo `connector` existe; nenhum conector existe.
- **Assinatura dos pacotes de release** (Parte 17). As somas provam integridade,
  não origem.
