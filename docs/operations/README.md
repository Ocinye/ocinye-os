# Operação

**Nada está deployado.** Este documento descreve o que existe para operar e o
que ainda falta, não um sistema em produção.

## Saúde

| Endpoint | O que responde |
|---|---|
| `GET /health` | O processo está vivo. |
| `GET /ready` | Consegue servir. Sonda a base de dados com uma query real. |

`/ready` reporta separadamente: base de dados, object storage, Identity Provider
e Intelligence Plane.

**A prontidão depende apenas da base de dados.** Storage e IA podem legitimamente
estar ausentes, e um deployment sem eles ainda serve a instituição.

Um health check nunca reporta saudável algo que não verificou.

## Logs

Estruturados, JSON fora de desenvolvimento. Cada linha carrega `request_id` e
`correlation_id`, propagados do Workspace ao Core e ao Worker.

Um membro que reporte um problema pode citar o `x-request-id` devolvido na
resposta, e isso localiza as linhas correspondentes.

**Nunca aparecem em logs:** passwords, tokens, cookies, documentos, conteúdos de
datasets, prompts. As query strings são omitidas dos logs de acesso porque podem
transportar termos de pesquisa sobre material classificado.

## Worker

Drena o outbox a cada 2 segundos quando está vazio, imediatamente quando não
está. De 30 em 30 segundos marca como indisponíveis os modelos de nós silenciosos.

Um evento que falhe 10 vezes **deixa de ser tentado mas não é descartado**, com o
erro na linha. Um evento encravado é um sinal:

```sql
SELECT name, aggregate_type, attempts, last_error, occurred_at
  FROM outbox_events
 WHERE published_at IS NULL AND attempts >= 10
 ORDER BY occurred_at;
```

## Auditoria

```sql
-- Quem acedeu a material RESTRICTED
SELECT occurred_at, actor_subject, action, resource_type, resource_id
  FROM audit_events
 WHERE classification = 'RESTRICTED'
 ORDER BY occurred_at DESC;

-- Recusas de segurança
SELECT occurred_at, actor_subject, resource_type, metadata->>'reason'
  FROM audit_events
 WHERE action = 'security_denial'
 ORDER BY occurred_at DESC;
```

A tabela é append-only por trigger. Trabalho de retenção exige uma migration
privilegiada que remova e reponha o trigger — deliberadamente difícil.

## Ambientes

Development, Staging, Production. Configuração própria por ambiente; credenciais
nunca partilhadas.

Em produção o Core recusa arrancar sem issuer OIDC HTTPS ou com origem CORS
wildcard; o Workspace recusa arrancar sem HTTPS, sem cookies seguros ou sem
client secret.

## O que falta para operar a sério

| Falta | Estado |
|---|---|
| Backups | **Não configurados.** Ver [backups](../backups/README.md). |
| Métricas e alertas | Não implementados. |
| Rate limiting | Não implementado. |
| Runbook de incidente | Não escrito. |
| Rotação de credenciais | Sem procedimento. |
| Sessões do Workspace duráveis | Em memória; um reinício termina-as. |
