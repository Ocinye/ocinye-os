# Respostas às perguntas da integração (Q-01 a Q-14)

| # | Decisão | O que muda no pacote |
|---|---|---|
| Q-01 | O «+ Criar» lista **o que o produto sabe criar**: ideia, projecto, nota, referência, dataset, tarefa, agente, e ainda **evento** (`/calendar`, POST existente) e **mensagem** (`/mail/compose`). Saem «Novo documento» e «Carregar ficheiro»: carregar é uma acção dos Ficheiros, não uma entidade. Sem filtragem por permissão na UI: a autoridade fica no fluxo de cada acção. | Ordem: Nota, Tarefa, Evento, Mensagem, Ideia, Projecto, Dataset, Referência, Agente. Atalhos de teclado no desenho ficam só como indicação até existirem no `app.js`. |
| Q-02 | Concordo. Sem unidade activa global, o contexto é **texto**: o nome da Instância. As consultas com âmbito usam o `unit_selector` de cada lista (D12_PAGES). | D2: `.ods-topbar__ctx` passa a `<span>` sem seta nem `cursor`. |
| Q-03 | Concordo: o flutuante abre a barra (`data-oc="shelf-toggle"`); o lançador abre por «Aplicações» e ⌘J. | — |
| Q-04 | Acrescentados 8 símbolos: `ods-messages`, `ods-knowledge`, `ods-bibliography`, `ods-ai`, `ods-compute`, `ods-admin`, `ods-plus`, `ods-chev-d`. | `static/ods-icons.svg` (48 símbolos). O «+ Criar» usa `ods-plus`; a seta do contexto e dos menus `ods-chev-d`. |
| Q-05 | Adopto a cola: `.ods-shell__main` é o contentor de scroll do conteúdo. | Em `static/ods-d12-base.css`; o `ods-integration.css` pode sair. |
| Q-06 | Adopto `data-semana` nos dias da semana corrente, `aria-current="date"` em hoje. `.ods-cal__week` fica `display: contents`. | `ods-d12-base.css` |
| Q-07 | Faixa própria, **acima da barra de topo**, a toda a largura, vermelho sólido e texto branco: escudo + «Sessão privilegiada» + «conduzida por {nome}» + hora de fim. Não se fecha nem some em repouso. `role="status"`, `data-oc="privileged-session"`. | `.ods-privileged`, `__who` em `ods-d12-base.css` |
| Q-08 | Concordo: `ods-modal` + `ods-search` + `ods-menu`, com os grupos NAVEGAR / ACÇÕES e os marcadores `palette*` actuais. Largura 640px, centrado a 18vh do topo. | `.ods-palette__*` em `ods-d12-base.css` |
| Q-09 | **Repor.** A linha «Sessão actual · expira em …» é informação de segurança; fica sob o cabeçalho da conta, em mono. | `.ods-account__session` |
| Q-10 | Confirmo: «O Meu Trabalho» sai da barra fixa. Entra como **fixação por defeito** do perfil Research no registo (o membro pode desafixar). | Pede ao Claude Code um valor por defeito em `0051_member_app_pins` (ou no registo); sem isso, fica só no lançador. |
| Q-11 | Concordo: omitido até G-09. Para administradores, «Ver opções de IA» → `/ai` basta. | — |
| Q-12 | Adopto: `box-sizing: border-box` global e `margin: 0` em `html, body`. | `ods-d12-base.css` |
| Q-13 | Concordo: nome de `GET /instance/branding`; sem resposta, «OCINYE OS». | — |
| Q-14 | Concordo: o contentor de conteúdo é coluna flex e o Desktop ocupa-o. | `.ods-shell__main > .ods-desktop` em `ods-d12-base.css` |
