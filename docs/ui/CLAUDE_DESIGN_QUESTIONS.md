# Perguntas ao Claude Design

> O que a integração encontrou onde a especificação é omissa, ou choca com uma
> regra do produto. Nenhuma foi decidida por quem integra: em cada uma diz-se o
> que ficou entretanto, e porquê. A resposta volta num pacote, ou aqui.

| # | Pacote | Pergunta | Entretanto |
|---|---|---|---|
| Q-01 | D2 | O «+ Criar» do D2 lista nota, documento, tarefa, evento, mensagem, carregar e agente, e pede filtragem por permissão. O produto tem hoje ideia, projecto, nota, referência, dataset, tarefa e agente, **sem** filtragem por permissão (a autoridade é do Core no fluxo de cada acção). Qual é o conjunto? | Ficam os sete actuais, com a apresentação do D2: nenhuma criação existente se perde. |
| Q-02 | D2 | O botão de contexto «UENR-001» escolhe uma unidade activa. O Ocinye OS não tem unidade activa global (`CLAUDE.md` §34.3): o âmbito é de cada consulta. | Mostra o nome da Instância, como texto e não botão — um botão sem escolha seria interface morta. |
| Q-03 | D4/D6 | O D4 põe `data-oc="launcher-open"` no botão flutuante; o D6 diz que ele abre a barra vertical. | O flutuante abre a barra (`data-oc="shelf-toggle"`); o lançador abre pelo «Aplicações» da barra e por ⌘J. |
| Q-04 | D1 | O sprite tem 40 símbolos. Faltam: mensagens, conhecimento, bibliografia, Ocinye AI, computação, administração, `plus`, `chev-d`. | Essas aplicações mostram `apps-brand`; o «+ Criar» leva «+» em texto; as notificações de mensagens usam o sino. |
| Q-05 | D2 | A `.ods-shell` tem altura fixa e `overflow: hidden`, e nenhuma regra dá ao conteúdo de uma página o seu contentor de scroll. | `static/ods-integration.css`: `.ods-shell__main { flex: 1; min-height: 0; overflow-y: auto; }` — cola técnica, para adoptar ou substituir. |
| Q-06 | D2 | `.ods-cal__week[data-current]` numa grelha de 7 colunas precisa de um contentor por semana que o CSS não descreve. | Os dias da semana corrente levam `data-semana`, sem estilo; hoje leva `aria-current="date"`. |
| Q-07 | D2 | A faixa de sessão privilegiada (obrigatória, ADR-0107) não está desenhada. | `ods-notice ods-notice--error` com o escudo, o rótulo e quem conduz a sessão. |
| Q-08 | D2 | A command palette (⌘K) não está desenhada. | Diálogo com as primitivas do D1: `ods-modal`, `ods-search`, `ods-menu`. |
| Q-09 | D2 | O menu da conta perdeu a linha «Sessão actual · expira em…». Confirmar. | Retirada, como no D2. |
| Q-10 | D6 | «O Meu Trabalho» deixa de estar fixo na barra (era navegação essencial com a Home). Confirmar. | Como no D6: Desktop, janelas, Aplicações, fixadas, Lixo. |
| Q-11 | D2 | «Estado detalhado» no cartão de estado não tem destino no Workspace. | Omitido; fica «Ver opções de IA» → `/ai`. |
| Q-12 | D0 | Os protótipos declaram `*{box-sizing:border-box}` e `html,body{margin:0}`; o `ocinye-ds.css` não. Sem elas o arranque transborda a 390 px. | Copiadas dos protótipos para `static/ods-integration.css`. |
| Q-13 | D3 | O login do D3 mostra «{nome da instância}». À porta não há sessão: o nome vem de `GET /instance/branding`, público. Sem resposta, mostra «OCINYE OS». | Como descrito. |
