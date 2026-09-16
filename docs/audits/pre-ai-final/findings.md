# Findings — Pré-IA (registo vivo)

Severidade: **P0** perda de dados/segurança/outage · **P1** workflow central ou
privilégio partido · **P2** defeito funcional/UX significativo · **P3**
polimento/não-bloqueante.

Estado: **OPEN** · **FIXED** (com PR) · **DEFERRED** (adiado com razão) ·
**WONTFIX** (por desenho).

## Contagem — encontrados vs. abertos

Um defeito **encontrado** e corrigido não é um defeito **aberto**. O portão final
exige `P0 abertos = 0`, `P1 abertos = 0`, `P2 abertos = 0` (salvo deferimento
autorizado). Os P0/P2 desta tabela foram encontrados **durante** a auditoria (ou
imediatamente antes) e corrigidos.

| Sev | Encontrados | **Abertos** |
|---|---|---|
| P0 | 1 (F-02, outage por *stage drift*) | **0** |
| P1 | 0 | **0** |
| P2 | 1 (F-01, descarga institucional) | **0** |
| P3 | 4 (F-03..F-07) | 2 (F-05, F-06 — em avaliação; F-07 é copy, a corrigir) |

| ID | Sev | Domínio | Descrição | Estado |
|---|---|---|---|---|
| F-01 | P2 | Files | Descarga institucional redireccionava para o host interno do armazenamento (`object-store:9000`), inalcançável — descarga partida | **FIXED** (#107, ADR-0608) |
| F-02 | P0 | Deploy | Um stage novo no Dockerfile mudou o alvo *default*; core/worker/workspace saíram do stage errado → workspace em baixo (502) | **FIXED** (#109) + guardado (#111) |
| F-03 | P3 | Ops | Sem rollback rápido: um deploy mau exigia corrigir-para-a-frente (~30 min com produção em baixo) | **FIXED** (#111, `rollback-production.sh` + runbook) |
| F-04 | ~~P2~~ **P3** | IA | `/mail/assist` e `/messaging/assist` devolvem **503** sem provider, enquanto `/ai/prompt` e `/agentic/invoke` devolvem **200 tipado** (ADR-0308). Superfícies distintas (helper inline de texto vs. conversa Prompt); a UI degrada com elegância (mostra «não conseguiu ajudar», sem erro cru); permissão verificada primeiro; sem fallback externo. Não é fuga nem torna o Core não-saudável | **WONTFIX** (por desenho) |
| F-05 | P3 | Observab. | `api.ocinye.com/api/v1/health` → 404; o health vive em `/health` (interno, usado pelo healthcheck do contentor), sem sonda pública equivalente | **OPEN** (a avaliar) |
| F-06 | P3 | Data | `collaboration::add_comment` autoriza `Create` no ambiente mas não verifica que `subject_id` pertence a esse ambiente — nit de integridade, **não** é fuga de acesso (leituras ficam seguras pela visibilidade do ambiente) | **OPEN** (a avaliar) |
| F-07 | P3 | UX | Menu «Criar» global mostra Projecto/Dataset/Referência como «Ainda não disponível» — mas são criáveis a partir da lista/ambiente (criação-em-contexto). Rótulo enganoso, não feature partida | **OPEN** (a verificar intenção) |

## Notas de disposição

- **F-04** foi reavaliado e é **por desenho**: o `assist` de Mail/Messaging é um
  helper inline que devolve texto, uma superfície distinta da conversa do Prompt
  (que usa o envelope tipado do ADR-0308). Sem provider, devolve 503 com mensagem
  institucional, a UI degrada com elegância, a permissão é verificada primeiro, e
  não há fallback externo — honesto e não-bloqueante. Alinhá-lo ao envelope do
  Prompt seria sobre-engenharia (§71) sem benefício para quem usa. O contrato
  tipado central (`/ai/prompt`, `/agentic/invoke`) está correcto.
- **F-05/F-06/F-07** são P3: avaliam-se e corrigem-se se o arranjo for limpo, ou
  documentam-se como por-desenho. Nenhum bloqueia a operação determinista.

## O que a auditoria **não** encontrou (evidência positiva)

- **Sem broken access control:** toda a mutação de estado passa por
  `authorize()`/`can()`/`require()` ou posse validada em SQL (`owner_id`). Duas
  excepções por desenho: `/compute/{enroll,heartbeat}` (credencial de máquina,
  ADR-0500) e `/invitations/accept`,`/auth/login` (o token/credencial é a prova).
- **Sem IDOR:** toda a leitura por-id resolve por um portão de autoridade antes de
  devolver; versões resolvem pelo recurso-pai; recusas são `NotFound` (não revelam
  existência).
- **Sem rota-página inalcançável** e **sem entrada de navegação/Criar para rota
  inexistente**.
- **Sem dados de demonstração fixos** no caminho de render; onde o Core não tem
  endpoint, a UI mostra um estado «ainda não» declarado, não dados falsos.
