# `apps/workspace` — Ocinye Workspace

A principal interface humana do Ocinye OS.

Não é uma área privada de um website: é o ambiente operacional de investigação e
engenharia através do qual os membros da instituição agem sobre o Ocinye Core.

## O design

Implementa o dossier em [`design/`](../../design/README.md) — 20 ecrãs de alta
fidelidade, sistema visual completo, ícones e protótipo navegável.

O conjunto de ícones cresceu para **42** com o Ocinye Mail: o dossier continua
a ser a fonte visual, e o sprite continua a ser verificado nos dois sentidos
contra a enumeração `Icon`.

Os valores visuais vivem em `static/ocinye.css` como custom properties, copiados
de `design/DESIGN_TOKENS.md`. **Nenhum componente define cor, tamanho ou raio
soltos**, e um teste
([`tests/design_fidelity.rs`](tests/design_fidelity.rs)) lê o dossier e compara
token a token: um `#0B2D4B` em vez de `#0B2D4A` passaria despercebido a olho e
falha ali.

## Arquitectura

### Backend-for-Frontend

Este servidor executa o fluxo OIDC e **guarda os tokens ele próprio**. O browser
recebe um cookie de sessão opaco e nada mais
([ADR-0601](../../docs/adrs/0601-workspace-bff-session.md)).

### Renderização

Leptos em server-side rendering, com uma camada delimitada de progressive
enhancement em `static/app.js` para a command palette, a sidebar colapsável, o
menu `+ Criar`, as tabs locais e a densidade das tabelas
([ADR-0602](../../docs/adrs/0602-workspace-ssr-progressive-enhancement.md)).

Esse ficheiro **nunca** decide autorização, nunca guarda dados institucionais e
nunca obtém dados. Sem JavaScript, a navegação, os formulários e todas as
tabelas continuam a funcionar.

Hidratação é o destino declarado; os componentes já são Leptos, pelo que adoptá-la
é uma mudança de cadeia de build, não uma reescrita.

### O browser nunca é autoridade

As vistas escondem o que um membro não pode usar. Nunca *decidem* que pode. Toda
a operação é autorizada pelo Core, que a recusaria independentemente do que este
servidor tivesse renderizado.

## Estrutura

```
src/
  config.rs      configuração por ambiente
  session.rs     sessões do lado do servidor
  api.rs         cliente do Ocinye Core
  routes.rs      os ecrãs, o correio e a autenticação
  ui/
    mod.rs       documento HTML, escape, iniciais
    icon.rs      os 42 ícones, verificados contra o sprite
    shell.rs     sidebar, topbar, command palette, menu Criar
    components/  badge, botão, cartão, tabela, tabs, campos, progresso, estado vazio
    screens/     um módulo por ecrã ou família de ecrãs
static/
  ocinye.css     tokens, reset e todos os componentes
  app.js         camada de interacção
  icons.svg      sprite dos 42 ícones
```

## Os ecrãs

| Rota | Ecrã |
|---|---|
| `/login` | Início de sessão, estilo workstation |
| `/` | Home / Dashboard |
| `/my-work` | O Meu Trabalho |
| `/units` · `/units/{id}` | Unidades e detalhe (9 tabs) |
| `/ideas` · `/ideas/new` | Ideias e criação |
| `/projects` | Projectos |
| `/workspaces/{id}` | Research Workspace (13 tabs) |
| `/knowledge` | Knowledge Hub |
| `/bibliography` · `/datasets` | Bibliografia e Dados |
| `/ai` · `/ai/agents` · `/ai/agents/new` · `/ai/prompt` | Ocinye AI |
| `/compute` | Computação |
| `/activity` · `/admin` · `/audit` | Institucional |
| `/mail` · `/mail/{mailbox}` | Correio: caixas, pastas, lista, leitura |
| `/mail/message/{id}` | Uma mensagem, higienizada, sem conteúdo remoto |
| `/mail/compose` | Composer, com assistência de escrita |
| `/mail/settings` | Preferências e estado do serviço |

`/ideas/{id}` e `/projects/{id}` encaminham para `/workspaces/{id}`. **Isto é um
desvio deliberado ao dossier**, e vale a pena explicá-lo: no domínio do Ocinye
Core, promover uma ideia mantém o *mesmo* Research Workspace, que passa a
hospedar o projecto. Um único URL canónico é mais verdadeiro do que dois URLs
para o mesmo objecto.

## Princípios do design que o código impõe

| Regra do dossier | Como é imposta |
|---|---|
| Estado nunca só por cor | Todos os badges levam ponto e texto; teste em `components/badge.rs` |
| Classificação visível | `classification_badge` em Idea, Project, Dataset, Document e no contexto de IA |
| Ideia ≠ Projecto | Estados e tabs distintos; teste em `screens/workspaces.rs` |
| Audit Log não é feed | Notação técnica `recurso.acção`, resultado `OK`/`NEGADO`/`AVISO` |
| Infraestrutura não existe | Estados vazios reais; testes que falham se `CAM-01` ou um modelo forem inventados |
| Ícones de um só conjunto | `Icon` é uma enumeração fechada, verificada contra o sprite nos dois sentidos |
| Conteúdo é exemplo | Nenhum dado de demonstração no código: tudo vem do Core |
| HTML alheio é contido | Um único `inner_html`, no correio, com o conteúdo já higienizado pelo Core; o CSS neutraliza `position` e `float` |
| Gerar não é enviar | Dois `formaction` distintos no composer; `/mail/assist` devolve texto, `/mail/send` é a única rota que envia |

## Acções ainda sem ecrã

O dossier especifica acções cujo ecrã de destino não está entre os 20 que
especifica: `Novo Projecto`, `Nova Nota`, `Nova Referência`, `Novo Dataset`,
`Nova Tarefa`, `Convidar Membro`, `Exportar`, `Adicionar Nó`, `Promover a
Projecto`, `Partilhar`, `Definições` e `Ajuda`.

No correio, a mesma regra vale para **Descarregar** um anexo: depende de object
storage, que não está configurado, e por isso aparece declarado indisponível em
vez de falhar ao ser carregado.

Ficam **visíveis e declaradas como indisponíveis**, em vez de levarem a um 404 ou
de serem escondidas. Um teste
(`nenhuma_ligacao_aponta_para_um_ecra_inexistente`) falha se alguma delas for
ligada a um ecrã que não existe.

## Configuração

Ver [`.env.example`](../../.env.example). Em produção recusa arrancar sem HTTPS,
sem cookies seguros ou sem client secret.

## Execução e testes

```bash
set -a && source .env && set +a
cargo run --bin ocinye-workspace     # http://localhost:8090

cargo test -p ocinye-workspace       # 66 testes
```

## Segurança relevante

- Cookie `HttpOnly`, `SameSite=Lax`, `Secure`; identificador de 256 bits.
- PKCE `S256` e `state` anti-CSRF; um callback repetido não encontra nada.
- CSP `default-src 'none'` com o próprio script e stylesheet e as fontes do
  Google; sem inline, sem outras origens, sem frames.
- `Cache-Control: no-store`: as páginas são por membro.
- Terminar sessão é `POST` e encerra também a sessão no Identity Provider.
- As sondas ao IdP e ao Core têm limite curto, para que um serviço em baixo
  produza uma mensagem em vez de um ecrã pendurado.

## Acessibilidade

Contraste AA, focus ring dourado global, navegação por teclado em navegação,
tabs, tabelas e palette, rótulo em todos os campos, `lang="pt"`, skip link,
`prefers-reduced-motion` respeitado, e estado nunca comunicado só por cor.

## Limitações declaradas

- **Sessões em memória**: um reinício termina-as ([ADR-0601](../../docs/adrs/0601-workspace-bff-session.md)).
- **Sem hidratação**: cada navegação é um round-trip.
- **Paginação, ordenação e filtros são server-side por desenho**, mas os
  controlos de filtro e paginação ainda não submetem — os endpoints do Core
  aceitam os parâmetros; a ligação da UI falta.
- **Sem comparação visual lado a lado com o protótipo**: os tokens estão
  verificados automaticamente, o alinhamento visual não foi conferido num
  browser.
