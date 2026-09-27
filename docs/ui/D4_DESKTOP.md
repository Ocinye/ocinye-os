# D4 · Desktop e widgets

CSS: `static/ods-d4-desktop.css`. Rota: `GET /` (`screens/home.rs`). Os marcadores `data-oc-content="1"` actuais mantêm-se nas linhas de conteúdo de membros.

## Dependência: **G-02** (Desktop persistente) e **G-03** (resumo de indicadores)
Não existe `Desktop` nem `Widget Registry`. Até haver contrato:
- a Home mostra **uma disposição fixa servida pelo servidor** (a predefinição do perfil), com os widgets cujos dados já existem;
- o botão Personalizar (`data-oc="desktop-edit"`) aparece com `aria-disabled="true"` e tooltip `ods.state.pending_contract`;
- minimizar, mover, redimensionar, fundo e densidade ficam **desligados** — nada se guarda em `localStorage`.

## Estrutura
```html
<section class="ods-desktop" data-oc="desktop" data-profile="research">
  <div class="ods-desktop__scroll" data-ods-scroll>
    <div class="ods-desktop__grid" data-oc="widget-grid">
      <article class="ods-widget" data-kind="kpis" data-w="full" data-oc="widget" data-widget="kpis">
        <div class="ods-widget__body ods-kpis">
          <a class="ods-kpi ods-widget-surface" href="/units" data-oc="kpi" data-kpi="units">
            <span class="ods-kpi__head"><span class="ods-widget__icon"><svg class="ods-icon ods-icon--sm"><use href="/static/ods-icons.svg#ods-units"/></svg></span>Unidades<svg class="ods-icon ods-icon--sm"><use href="/static/ods-icons.svg#ods-arrow-r"/></svg></span>
            <span><span class="ods-kpi__value">4</span> <span class="ods-kpi__label">activas</span></span>
          </a>
          …ideas (/ideas) · projects (/projects) · datasets (/datasets)
        </div>
      </article>
      <article class="ods-widget ods-widget-surface" data-kind="calendar" data-w="1" data-h="2" data-oc="widget" data-widget="calendar">
        <header class="ods-widget__head">
          <span class="ods-widget__icon">…</span>
          <span class="ods-widget__titles"><span class="ods-widget__title">Calendário</span><span class="ods-widget__sub">UENR-001</span></span>
          <button class="ods-iconbtn" data-oc="widget-minimize" aria-pressed="false" aria-label="Minimizar" aria-disabled="true">…</button>
          <a class="ods-iconbtn" href="/calendar" aria-label="Abrir">…</a>
        </header>
        <div class="ods-widget__body">…</div>
      </article>
    </div>
  </div>
  <button class="ods-float-apps" data-oc="launcher-open" data-ods-float aria-label="Mostrar barra de aplicações">…apps-brand-dark…</button>
  <!-- Nye flutuante: D7 -->
</section>
```

## Widgets da predefinição Research e origem dos dados
| Widget | `data-widget` | Tamanho | Dados | Estado sem dados |
|---|---|---|---|---|
| Indicadores | `kpis` | full × 4 linhas | **G-03**; até lá contagens das listagens `/units`, `/ideas`, `/projects`, `/datasets` feitas no servidor | se uma listagem falhar: `—` + `ods.state.error` no cartão, **nunca 0** |
| Calendário | `calendar` | 1×2 | eventos de `/calendar` | vazio: «Sem eventos esta semana» |
| Avisos institucionais | `notice` | 2×1, obrigatório (sem minimizar/remover) | notificações institucionais | vazio: «Sem avisos» |
| Continuar trabalho | `continue` | 2×1 | actividade do membro (`/activity` com owner) | vazio |
| Tarefas | `tasks` | 1×2 | `/my-work` | vazio: «Sem tarefas atribuídas» |
| Projectos | `projects` | 2×1 | `/projects` | recusado: `ods-state--denied` |
| Ideias, Armazenamento, Actividade | `ideas` `storage` `activity` | 1×1 / 1×2 | listagens / quota de `/resources` | idem |

Cada corpo de widget tem os 5 estados de D1 (`ods-skeleton` a carregar; `ods-empty`; `ods-state--error`; `--denied`; `--unavailable`).

## Predefinição do Desktop (G-04)
Banner «Está disponível uma nova predefinição», diálogo de reposição com pré-visualização e anular, e o editor de administração **não entram** até G-04. Especificação visual nos protótipos (`design-reference/Ocinye OS.dc.html`, «reset» e «Administração → Desktop Default»).
