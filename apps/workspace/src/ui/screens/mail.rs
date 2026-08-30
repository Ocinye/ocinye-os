//! Ocinye Mail — a superfície humana do correio institucional.
//!
//! # Três decisões estruturais deste ecrã
//!
//! **O único sítio do Workspace onde entra HTML alheio.** O corpo de uma
//! mensagem é escrito por quem a enviou. Chega aqui já limpo pelo Ocinye Core
//! (`ocinye_core::modules::mail::sanitize`) e é o único `inner_html` da
//! interface. Nenhum outro ecrã injecta markup que não tenha construído.
//!
//! **Gerar não é enviar.** O composer é um formulário com dois botões de
//! submissão e destinos diferentes: `/mail/assist` devolve texto para o campo,
//! `/mail/send` entrega ao serviço de correio. Não é uma convenção — são duas
//! rotas, e a de assistência não tem forma de chamar a outra (briefing §15).
//!
//! **Conteúdo remoto não carrega sozinho.** Uma imagem remota diz a quem
//! enviou a mensagem que ela foi aberta, e a que horas. O Core substitui-as e
//! conta-as; este ecrã mostra a contagem e um botão para as carregar
//! explicitamente (briefing §12).

use leptos::prelude::*;
use serde_json::Value;

use ocinye_contracts::{ComposeAction, RemoteContentPolicy};

use crate::ui::components::{
    badge, button, card, empty_state, field_with_value, section_head, select, select_labelled,
    textarea_with_value, Button, EmptyState, SelectOption, Tone, Variant,
};
use crate::ui::icon::{icon, Icon};
use crate::ui::shell::Viewer;

/// O contexto de correio partilhado por todos os ecrãs desta família.
pub struct MailView {
    /// `GET /api/v1/mail/status`.
    pub status: Value,
    /// O resultado da última actualização pedida nesta visita, se houve.
    ///
    /// Mostrado como confirmação: sem ele, carregar em «Actualizar» e não ver
    /// nada de novo é indistinguível de o botão não funcionar (briefing §100).
    pub sync_notice: Option<String>,
    /// `GET /api/v1/mail/mailboxes`.
    pub mailboxes: Value,
    /// A caixa aberta, quando alguma.
    pub active_mailbox: Option<String>,
    /// A pasta aberta.
    pub folder: String,
    /// O termo pesquisado, se algum.
    pub query: String,
}

impl MailView {
    /// Se o serviço consegue ler correio.
    fn can_read(&self) -> bool {
        self.status
            .get("can_read")
            .and_then(Value::as_bool)
            .unwrap_or(false)
    }

    /// Se o serviço consegue enviar.
    ///
    /// Distinto de ler: IMAP e SMTP são serviços diferentes e falham em
    /// separado (briefing §105).
    fn can_send(&self) -> bool {
        self.status
            .get("can_send")
            .and_then(Value::as_bool)
            .unwrap_or(false)
    }

    /// Se esta pessoa tem uma caixa ligada.
    ///
    /// Distinto de [`Self::can_read`]: uma caixa por ligar e um serviço em
    /// baixo dão ambos «não consegue ler», e pedem coisas diferentes a quem
    /// lê. Uma pede uma acção da própria pessoa; a outra pede que se espere.
    fn mailbox_linked(&self) -> bool {
        self.status
            .get("mailbox_linked")
            .and_then(Value::as_bool)
            .unwrap_or(false)
    }

    /// Se a instalação sabe onde é o serviço de correio.
    ///
    /// Vem do Core como facto. A primeira escrita deduzia-o de a frase do
    /// `detail` conter «ainda não está ligada» — e uma frase não é um guarda:
    /// mudar a redacção mudaria o ecrã, em silêncio e sem nenhum teste a
    /// falhar.
    fn transporte_configurado(&self) -> bool {
        self.status
            .get("transport_configured")
            .and_then(Value::as_bool)
            .unwrap_or(false)
    }

    /// O que o Core disse sobre o estado do serviço.
    fn detail(&self) -> String {
        self.status
            .get("detail")
            .and_then(Value::as_str)
            .unwrap_or(
                "O estado do correio institucional não pôde ser determinado. Nada foi \
                 enviado.",
            )
            .to_owned()
    }

    /// Se a assistência de escrita pode servir um pedido agora.
    fn ai_available(&self) -> bool {
        self.status
            .get("ai_assist_available")
            .and_then(Value::as_bool)
            .unwrap_or(false)
    }

    /// Se este membro pode sequer usar a assistência.
    ///
    /// Separado do anterior de propósito: não poder e não haver são coisas
    /// diferentes, e a interface deve dizer qual é (briefing §61).
    fn may_use_ai(&self) -> bool {
        self.status
            .get("may_use_ai")
            .and_then(Value::as_bool)
            .unwrap_or(false)
    }

    fn boxes(&self) -> &[Value] {
        self.mailboxes.as_array().map_or(&[], Vec::as_slice)
    }

    /// A caixa aberta, ou a primeira a que o membro tem acesso.
    fn current(&self) -> Option<&Value> {
        self.active_mailbox.as_ref().map_or_else(
            || self.boxes().first(),
            |wanted| {
                self.boxes()
                    .iter()
                    .find(|mailbox| mailbox.get("id").and_then(Value::as_str) == Some(wanted))
            },
        )
    }
}

/// Texto de um campo JSON, com alternativa.
fn text<'a>(value: &'a Value, key: &str, fallback: &'a str) -> &'a str {
    value.get(key).and_then(Value::as_str).unwrap_or(fallback)
}

// ── O ecrã principal ────────────────────────────────────────────────────

/// A caixa de correio: identidades, pastas, lista e painel de leitura.
///
/// `open` é a mensagem aberta, quando alguma. Um único ecrã em vez de dois
/// porque o design mantém a lista visível ao ler — sair da lista para ler e
/// voltar a entrar para ler a seguinte é o que torna um cliente de correio
/// cansativo (`CLAUDE.md` §47).
pub fn mail(
    viewer: &Viewer,
    view: &MailView,
    messages: &Value,
    open: Option<&Value>,
    compositor: Option<&ComposeDraft>,
) -> impl IntoView {
    let unavailable = !view.can_read();
    let detail = view.detail();
    let boxes = view.boxes().to_vec();

    // Sem serviço configurado não há caixas, e a lista de pastas com contagens
    // a zero pareceria uma caixa vazia em vez de um serviço ausente. São
    // estados diferentes e o membro tem de os distinguir (briefing §60).
    if unavailable && boxes.is_empty() {
        return unavailable_screen(
            &detail,
            view.mailbox_linked(),
            view.transporte_configurado(),
        )
        .into_any();
    }

    let current = view.current().cloned().unwrap_or(Value::Null);
    let current_id = text(&current, "id", "").to_owned();
    let folder = view.folder.clone();
    let query = view.query.clone();
    // O aviso de serviço aparece quando alguma das pontas não responde, e não
    // apenas quando o envio falha: uma instalação que não lê correio nenhum
    // mostrava a página inteira sem dizer porquê.
    let mostrar_aviso = !view.can_send() || !view.can_read();
    let ja_dito = if mostrar_aviso {
        detail.clone()
    } else {
        String::new()
    };

    let can_compose = view.can_send()
        && current
            .get("may_send")
            .and_then(Value::as_bool)
            .unwrap_or(false);

    view! {
        // A página do Correio não rola; os painéis é que rolam.
        //
        // Uma barra de deslocamento na página inteira leva o cabeçalho e a
        // barra de acções consigo — e depois de descer uma lista longa, deixa
        // de haver «Escrever» no ecrã. Numa aplicação de correio a moldura
        // fica, e o que se percorre é o conteúdo.
        <div class="oc-page oc-page--mail">
            <div class="oc-head">
                <div class="oc-head__text">
                    <h1>"Correio"</h1>
                    <p>
                        "O correio institucional da Ocinye, dentro do Ocinye Workspace."
                    </p>
                </div>
                // A hierarquia que a acção merece.
                //
                // «Actualizar», «Escrever» e «Definições» tinham o mesmo peso,
                // e por isso nenhum tinha peso nenhum. Escrever é o que se vem
                // aqui fazer; actualizar é uma manutenção que se faz de vez em
                // quando; as definições visitam-se uma vez.
                <div class="oc-head__actions oc-mail__accoes">
                    {comandos_de_disposicao()}
                    // As definições continuam a existir, e com o peso que têm:
                    // visitam-se uma vez. Tirá-las da barra ao reorganizá-la
                    // teria deixado a caixa sem caminho para se ligar.
                    <a
                        class="oc-icon-btn"
                        href="/mail/settings"
                        title="Definições de correio"
                    >
                        <span class="oc-sr">"Definições de correio"</span>
                        {icon(Icon::Settings, 15)}
                    </a>
                    {sync_action(&current_id, view.can_read(), &folder)}
                    {compose_action(can_compose, &current_id, view.can_send())}
                </div>
            </div>

            {view.sync_notice.as_ref().map(|notice| view! {
                <div class="oc-callout oc-mail__banner" role="status">
                    {icon(Icon::Restart, 15)}
                    <p>{notice.clone()}</p>
                </div>
            })}

            {mostrar_aviso.then(|| service_notice(&detail, view.mailbox_linked()))}

            // Uma superfície, e não três cartões.
            //
            // As três colunas eram três caixas com borda própria, cada uma com
            // o seu contorno e o seu raio — e uma caixa dentro de uma página
            // dentro de uma casca lê-se como um painel de administração, não
            // como uma aplicação de correio. Agora é uma superfície contínua,
            // dividida por linhas finas e pelos separadores que a pessoa
            // arrasta.
            <div class="oc-mail" data-oc="mail">
                {rail(&boxes, &current_id, &folder, &ja_dito)}
                {separador("pastas", "Ajustar a largura das pastas")}
                {list(&current_id, &folder, &query, messages, open, view.can_read())}
                {separador("lista", "Ajustar a largura da lista")}
                {open.map_or_else(
                    || reading_placeholder().into_any(),
                    |message| reading(viewer, view, message).into_any(),
                )}
            </div>

            // O compositor abre **sobre** o correio, e não noutra página.
            //
            // Escrever é um acto que acontece a olhar para a caixa: para
            // confirmar um nome, reler o que se responde, ver o que já
            // chegou. Uma página à parte tira isso, e obriga a voltar atrás
            // para recuperar aquilo que se tinha à frente.
            {compositor.map(|draft| compositor_flutuante(view, draft))}
        </div>
    }
    .into_any()
}

/// O ícone de uma pasta.
///
/// # Porque ícones e não emoji
///
/// Porque emoji são um segundo sistema de iconografia: desenham-se conforme o
/// sistema operativo de quem olha, trazem cor que a paleta não escolheu, e não
/// respondem aos tokens. O Ocinye OS tem um conjunto próprio, e uma pasta de
/// correio não é razão para abrir uma excepção (`CLAUDE.md` §45).
///
/// # Porque o ícone não substitui o nome
///
/// Porque um envelope e um arquivo distinguem-se mal ao canto do olho, e
/// quem não distinga formas fica sem nada. O ícone acompanha; o nome nomeia.
///
/// Uma pasta que este conjunto não conheça recebe o ícone do correio — é o
/// género certo, e é melhor do que um buraco na coluna.
const fn icone_da_pasta(pasta: &str) -> Icon {
    match pasta.as_bytes() {
        b"starred" => Icon::Star,
        b"drafts" => Icon::Document,
        b"sent" => Icon::Send,
        b"archive" => Icon::Archive,
        b"spam" => Icon::Shield,
        b"trash" => Icon::Trash,
        _ => Icon::Mail,
    }
}

/// Um separador que a pessoa arrasta.
///
/// # Porque não é uma `div` com um `mousedown`
///
/// Porque redimensionar é uma operação, e uma operação que só existe para o
/// rato exclui quem não usa rato. `role="separator"` com `aria-valuenow` é o
/// que diz a uma tecnologia de apoio que isto tem uma posição e que a posição
/// muda; `tabindex` é o que permite lá chegar; as setas são o que a movem.
///
/// A área sensível é maior do que a linha visível: uma linha de um pixel é
/// bonita e é impossível de agarrar. O elemento tem largura de sobra e a linha
/// desenha-se dentro dele.
///
/// **O Core não sabe que isto existe.** Larguras de painel são disposição, e
/// disposição pertence a quem está a olhar — não à instituição.
fn separador(qual: &'static str, rotulo: &'static str) -> impl IntoView {
    view! {
        <div
            class="oc-mail__split"
            data-oc="separador"
            data-oc-separador=qual
            role="separator"
            aria-orientation="vertical"
            aria-label=rotulo
            tabindex="0"
        >
            <span class="oc-mail__split-linha" aria-hidden="true"></span>
        </div>
    }
}

/// Os comandos que mudam a disposição, e nada mais.
///
/// # Porque vivem aqui e não nas Definições
///
/// Porque são gestos de leitura, e um gesto de leitura faz-se enquanto se lê.
/// Mandar alguém a um ecrã de configuração para dar mais espaço a uma
/// mensagem é o mesmo que mandá-lo mudar de sala para acender a luz.
///
/// # Zero controlos decorativos
///
/// Cada um destes faz alguma coisa, e o que faz é reversível. Não há
/// «minimizar» porque não existe minimizado, e não há botão de expandir
/// separado do de repor: é o mesmo botão, e o seu estado diz em qual dos dois
/// mundos se está.
fn comandos_de_disposicao() -> impl IntoView {
    view! {
        <div class="oc-mail__disposicao" data-oc="disposicao">
            <button
                type="button"
                class="oc-icon-btn"
                data-oc="alternar-pastas"
                aria-pressed="false"
                title="Recolher as pastas"
            >
                <span class="oc-sr">"Recolher as pastas"</span>
                {icon(Icon::SidebarCollapse, 15)}
            </button>
            <button
                type="button"
                class="oc-icon-btn"
                data-oc="focar-leitura"
                aria-pressed="false"
                title="Dar o ecrã à leitura"
            >
                <span class="oc-sr">"Dar o ecrã à leitura"</span>
                {icon(Icon::Filter, 15)}
            </button>
        </div>
    }
}

/// Actualizar esta pasta a partir do serviço de correio.
///
/// Explícito, e não automático, porque é isso que o Ocinye OS faz hoje: não
/// existe processo que actualize o correio recebido sozinho. Um botão que
/// existe diz a verdade sobre isso; uma lista que nunca muda não
/// (`CLAUDE.md` §69).
fn sync_action(mailbox_id: &str, service_up: bool, folder: &str) -> impl IntoView {
    if !service_up || mailbox_id.is_empty() {
        return view! {
            <span class="oc-btn oc-btn--secondary oc-unavailable" aria-disabled="true"
                  title="O serviço de correio não está disponível.">
                "Actualizar"
            </span>
        }
        .into_any();
    }

    let action = format!("/mail/{mailbox_id}/sync");
    view! {
        // Formulário e não ligação: actualizar altera estado, e um `GET` que
        // altera estado é actualizado por qualquer pré-carregamento do browser.
        <form method="post" action=action class="oc-mail__sync">
            <input type="hidden" name="folder" value=folder.to_owned() />
            <button type="submit" class="oc-btn oc-btn--secondary">
                {icon(Icon::Restart, 13)}
                "Actualizar"
            </button>
        </form>
    }
    .into_any()
}

/// O botão de escrever, ou a razão pela qual não está disponível.
fn compose_action(can_compose: bool, mailbox_id: &str, service_up: bool) -> impl IntoView {
    if can_compose {
        button(
            Button::new("Escrever", Variant::Primary)
                .href(format!("/mail/compose?mailbox={mailbox_id}")),
        )
        .into_any()
    } else {
        // Visível e declarado, não escondido: quem não vê o botão conclui que a
        // funcionalidade não existe (briefing §53).
        let reason = if service_up {
            "Não possui autorização para enviar a partir desta caixa."
        } else {
            "O serviço de envio não está disponível."
        };
        view! {
            <span class="oc-btn oc-btn--primary oc-unavailable" aria-disabled="true" title=reason>
                "Escrever"
            </span>
        }
        .into_any()
    }
}

/// O aviso de serviço, quando o correio não pode ser enviado.
/// O aviso de serviço, e o caminho quando ele existe.
///
/// O aviso dizia «Ligue-a em Correio → Definições» e não levava lá. Uma
/// instrução para navegar, num sítio onde uma ligação cabia, é trabalho que se
/// pede a quem lê por o produto não o ter feito.
///
/// A acção só aparece quando é da própria pessoa. Um serviço em baixo não se
/// resolve indo às definições, e oferecer lá um botão seria oferecer uma acção
/// que não faz nada — Dead UI com a forma de ajuda.
fn service_notice(detail: &str, ligada: bool) -> impl IntoView {
    let detail = detail.to_owned();
    let por_ligar = !ligada;
    view! {
        <div class="oc-callout oc-callout--warning oc-mail__banner" role="status">
            {icon(Icon::Shield, 15)}
            <p>{detail}</p>
            {por_ligar.then(|| button(
                Button::new("Ligar a minha caixa", Variant::Secondary).href("/mail/settings"),
            ))}
        </div>
    }
}

/// O ecrã inteiro, quando o correio não está configurado nesta instalação.
///
/// Uma caixa de entrada vazia seria uma mentira: sugeriria que não há
/// mensagens, quando o que não há é serviço (`CLAUDE.md` §69).
/// Correio sem nada para mostrar, e a razão certa.
///
/// # Três ausências, e não uma
///
/// «Não consegue ler» tinha uma só página, e por baixo dela estavam três
/// factos diferentes, com três acções diferentes:
///
/// | | quem resolve |
/// |---|---|
/// | a instalação não tem serviço de correio | quem administra |
/// | o serviço não responde | quem administra, e depois passa |
/// | esta pessoa ainda não ligou a sua caixa | **ela própria** |
///
/// A terceira era a que ficava pior servida: mandava alguém pedir a quem
/// administra o que só ele podia fazer, e num ecrã cuja acção era «ir à
/// Administração».
fn unavailable_screen(detail: &str, ligada: bool, configurado: bool) -> impl IntoView {
    let detail = detail.to_owned();

    let (titulo, corpo, accao) = if !configurado {
        (
            "O correio institucional não está configurado",
            format!(
                "{detail} É uma questão de configuração da instalação, e não do seu \
                 acesso. Quem administra o Ocinye OS pode activá-lo."
            ),
            Button::new("Administração", Variant::Secondary).href("/admin"),
        )
    } else if ligada {
        (
            "O serviço de correio não está a responder",
            format!(
                "{detail} A sua caixa continua ligada, e nada se perdeu — as mensagens \
                 estão no servidor e aparecem quando ele voltar."
            ),
            Button::new("Definições de correio", Variant::Secondary).href("/mail/settings"),
        )
    } else {
        (
            "A sua caixa ainda não está ligada",
            format!(
                "{detail} O Ocinye OS sabe onde é o servidor; falta a sua credencial. \
                 Ela é experimentada antes de ser guardada, e fica cifrada."
            ),
            Button::new("Ligar a minha caixa", Variant::Primary).href("/mail/settings"),
        )
    };

    view! {
        <div class="oc-page">
            <div class="oc-head">
                <div class="oc-head__text">
                    <h1>"Correio"</h1>
                    <p>"Correio institucional da Ocinye."</p>
                </div>
            </div>

            {empty_state(EmptyState {
                icon: Icon::Mail,
                title: titulo.to_owned(),
                body: corpo,
                actions: vec![accao],
                small: false,
            })}
        </div>
    }
}

// ── Coluna 1: identidades e pastas ──────────────────────────────────────

/// A coluna das caixas.
///
/// `ja_dito` é a razão que a página já mostrou em cima. Uma caixa cujo último
/// erro de sincronização diga exactamente o mesmo não o repete: são duas
/// origens diferentes — o estado do serviço e o estado desta caixa — mas
/// quando o texto coincide, quem lê vê duas caixas cor de laranja com a mesma
/// frase e conclui que são dois problemas.
///
/// O erro da caixa continua a aparecer quando **difere**, que é o caso que
/// importa: o serviço responde e é a credencial desta caixa que não entra
/// (ADR-0409).
fn rail(boxes: &[Value], current_id: &str, folder: &str, ja_dito: &str) -> impl IntoView {
    let boxes = boxes.to_vec();
    let current_id = current_id.to_owned();
    let folder = folder.to_owned();
    let ja_dito = ja_dito.trim().to_owned();

    view! {
        <nav class="oc-mail__rail" aria-label="Caixas de correio">
            {boxes
                .into_iter()
                .map(|mailbox| {
                    let id = text(&mailbox, "id", "").to_owned();
                    let active = id == current_id;
                    let shared = text(&mailbox, "kind", "personal") == "shared";
                    let address = text(&mailbox, "address", "").to_owned();
                    let name = mailbox
                        .get("display_name")
                        .and_then(Value::as_str)
                        .unwrap_or(&address)
                        .to_owned();
                    let sync_error = mailbox
                        .get("last_sync_error")
                        .and_then(Value::as_str)
                        .map(str::trim)
                        .filter(|reason| !reason.is_empty() && *reason != ja_dito)
                        .map(str::to_owned);
                    let folders = mailbox
                        .get("unread")
                        .and_then(Value::as_array)
                        .cloned()
                        .unwrap_or_default();
                    let folder = folder.clone();

                    view! {
                        <div class="oc-mail__box" data-active=active.to_string()>
                            <div class="oc-mail__box-head">
                                <span class="oc-mail__box-name">{name}</span>
                                {shared.then(|| badge("Partilhada", Tone::Gray))}
                            </div>
                            <span class="oc-mail__box-address oc-mono">{address}</span>

                            // Uma falha de sincronização aparece onde a caixa
                            // aparece. Escondê-la faria a lista parecer
                            // actualizada quando não está (briefing §100).
                            {sync_error.map(|reason| view! {
                                <p class="oc-mail__box-error" role="status">{reason}</p>
                            })}

                            <ul class="oc-mail__folders">
                                {folders
                                    .into_iter()
                                    .map(|entry| {
                                        let key = text(&entry, "folder", "inbox").to_owned();
                                        let label = text(&entry, "label", "").to_owned();
                                        let unread = entry
                                            .get("unread")
                                            .and_then(Value::as_i64)
                                            .unwrap_or(0);
                                        let selected = active && key == folder;
                                        let href = format!("/mail/{id}?folder={key}");

                                        view! {
                                            <li>
                                                <a
                                                    class="oc-mail__folder"
                                                    href=href
                                                    aria-current=selected.then_some("page")
                                                >
                                                    {icon(icone_da_pasta(&key), 14)}
                                                    <span>{label}</span>
                                                    {(unread > 0).then(|| view! {
                                                        <span class="oc-mail__count">{unread}</span>
                                                    })}
                                                </a>
                                            </li>
                                        }
                                    })
                                    .collect_view()}
                            </ul>
                        </div>
                    }
                })
                .collect_view()}
        </nav>
    }
}

// ── Coluna 2: a lista de mensagens ──────────────────────────────────────

/// A coluna das mensagens.
///
/// `pode_ler` decide o que dizer quando não há linhas. Sem ele, a lista
/// afirmava «Nenhuma mensagem nesta pasta» a quem ainda não tinha ligado a
/// caixa — por baixo de um aviso que dizia exactamente o contrário. Duas
/// leituras contraditórias no mesmo ecrã, e a mais tranquilizadora era a
/// falsa: quem a lesse concluía que não tinha recebido nada.
fn list(
    mailbox_id: &str,
    folder: &str,
    query: &str,
    messages: &Value,
    open: Option<&Value>,
    pode_ler: bool,
) -> impl IntoView {
    let items = messages
        .get("items")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    let mailbox_id = mailbox_id.to_owned();
    let folder = folder.to_owned();
    let action = format!("/mail/{mailbox_id}");
    let open_id = open
        .and_then(|message| message.get("message"))
        .and_then(|message| message.get("id"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    let searching = !query.is_empty();
    let count = items.len();

    view! {
        <div class="oc-mail__list">
            // Pesquisa por GET, dentro da caixa aberta. O Core filtra por
            // pertença antes de devolver — pesquisar não é forma de alcançar
            // correio alheio (`CLAUDE.md` §28).
            <form class="oc-mail__search" method="get" action=action role="search">
                <input type="hidden" name="folder" value=folder.clone() />
                <label class="oc-sr" for="mail-q">"Pesquisar nesta caixa"</label>
                <span class="oc-mail__search-icon">{icon(Icon::Search, 14)}</span>
                <input
                    class="oc-input"
                    id="mail-q"
                    name="q"
                    type="search"
                    value=query.to_owned()
                    placeholder="Pesquisar nesta caixa…"
                />
                <button type="submit" class="oc-btn oc-btn--secondary">"Pesquisar"</button>
            </form>

            {searching.then(|| view! {
                <p class="oc-mail__result-count" role="status">
                    {format!("{count} resultado(s) para a pesquisa.")}
                </p>
            })}

            {if items.is_empty() {
                empty_state(EmptyState {
                    icon: Icon::Mail,
                    title: if !pode_ler {
                        "Ainda não há correio para mostrar".to_owned()
                    } else if searching {
                        "Nenhuma mensagem corresponde".to_owned()
                    } else {
                        "Nenhuma mensagem nesta pasta".to_owned()
                    },
                    body: if !pode_ler {
                        "Esta caixa não está a ser lida — a razão está indicada em cima. \
                         Uma pasta vazia aqui não quer dizer que não tenha recebido nada."
                            .to_owned()
                    } else if searching {
                        "Nenhuma mensagem desta caixa corresponde ao termo pesquisado."
                            .to_owned()
                    } else {
                        "Esta pasta não tem mensagens indexadas no Ocinye OS.".to_owned()
                    },
                    actions: Vec::new(),
                    small: true,
                })
                .into_any()
            } else {
                view! {
                    <ul class="oc-mail__items">
                        {items
                            .into_iter()
                            .map(|message| row(&message, &open_id))
                            .collect_view()}
                    </ul>
                }
                .into_any()
            }}
        </div>
    }
}

fn row(message: &Value, open_id: &str) -> impl IntoView {
    let id = text(message, "id", "").to_owned();
    let selected = id == open_id;
    let unread = !message
        .get("is_read")
        .and_then(Value::as_bool)
        .unwrap_or(true);
    let starred = message
        .get("is_starred")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    // O nome escolhido por quem envia aparece ao lado do endereço, nunca em
    // vez dele: um remetente que se apresenta como «Ocinye Suporte» a partir
    // de um domínio qualquer não deve poder esconder o domínio (briefing §14).
    let from = text(message, "from_display_name", "").to_owned();
    let address = text(message, "from_address", "(remetente desconhecido)").to_owned();
    let from = if from.is_empty() {
        address.clone()
    } else {
        from
    };
    let subject = text(message, "subject", "(sem assunto)").to_owned();
    let preview = text(message, "snippet", "").to_owned();
    let when = short_date(text(message, "sent_at", ""));
    let has_attachments = message
        .get("has_attachments")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let href = format!("/mail/message/{id}");

    view! {
        <li>
            <a
                class="oc-mail__item"
                href=href
                data-unread=unread.to_string()
                aria-current=selected.then_some("page")
            >
                <div class="oc-mail__item-top">
                    <span class="oc-mail__from">{from}</span>
                    <span class="oc-mail__address oc-mono">{address}</span>
                    <span class="oc-mail__when oc-mono">{when}</span>
                </div>
                <div class="oc-mail__item-mid">
                    // Não-lida marcada por peso e por marca, nunca só por cor
                    // (`CLAUDE.md` §51).
                    {unread.then(|| view! { <span class="oc-mail__dot" aria-label="Não lida"></span> })}
                    <span class="oc-mail__subject">{subject}</span>
                    {starred.then(|| view! {
                        <span class="oc-mail__star" aria-label="Assinalada">
                            {icon(Icon::Star, 12)}
                        </span>
                    })}
                    {has_attachments.then(|| view! {
                        <span class="oc-mail__clip" aria-label="Tem anexos">
                            {icon(Icon::Attach, 12)}
                        </span>
                    })}
                </div>
                <p class="oc-mail__preview">{preview}</p>
            </a>
        </li>
    }
}

/// A coluna de leitura antes de se abrir alguma coisa.
fn reading_placeholder() -> impl IntoView {
    view! {
        <div class="oc-mail__pane oc-mail__pane--empty">
            {empty_state(EmptyState {
                icon: Icon::Mail,
                title: "Seleccione uma mensagem".to_owned(),
                body: "O conteúdo aparece aqui. Imagens e conteúdo remoto não são \
                       carregados automaticamente."
                    .to_owned(),
                actions: Vec::new(),
                small: true,
            })}
        </div>
    }
}

// ── Coluna 3: a leitura ─────────────────────────────────────────────────

fn reading(viewer: &Viewer, view: &MailView, payload: &Value) -> impl IntoView {
    let message = payload.get("message").cloned().unwrap_or(Value::Null);
    let id = text(&message, "id", "").to_owned();
    let subject = text(&message, "subject", "(sem assunto)").to_owned();
    let from_display = text(&message, "from_display_name", "").to_owned();
    let from_address = text(&message, "from_address", "").to_owned();
    let when = short_date(text(&message, "sent_at", ""));
    let starred = message
        .get("is_starred")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    let body_html = payload
        .get("body_html")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_owned();
    let blocked = payload
        .get("blocked_remote_count")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let domains: Vec<String> = payload
        .get("linked_domains")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default();
    let attachments = payload
        .get("attachments")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let recipients = addresses(payload, "to");
    let copies = addresses(payload, "cc");

    let can_reply = view.can_send();
    let reply_href = format!("/mail/compose?reply={id}");
    let _ = viewer;

    view! {
        <article class="oc-mail__pane">
            <header class="oc-mail__pane-head">
                <h2>{subject}</h2>
                <div class="oc-mail__meta">
                    <span class="oc-mail__from">
                        {if from_display.is_empty() { from_address.clone() } else { from_display }}
                    </span>
                    <span class="oc-mono oc-mail__address">{from_address}</span>
                    <span class="oc-mono oc-mail__when">{when}</span>
                </div>
                {(!recipients.is_empty()).then(|| view! {
                    <p class="oc-mail__recipients">"Para: "<span class="oc-mono">{recipients}</span></p>
                })}
                {(!copies.is_empty()).then(|| view! {
                    <p class="oc-mail__recipients">"Cc: "<span class="oc-mono">{copies}</span></p>
                })}

                <div class="oc-mail__actions">
                    {if can_reply {
                        button(Button::new("Responder", Variant::Primary).href(reply_href)).into_any()
                    } else {
                        view! {
                            <span
                                class="oc-btn oc-btn--primary oc-unavailable"
                                aria-disabled="true"
                                title="O serviço de envio não está disponível."
                            >"Responder"</span>
                        }
                        .into_any()
                    }}
                    {flag_form(&id, "starred", !starred, if starred {
                        "Retirar destaque"
                    } else {
                        "Assinalar"
                    }, Icon::Star)}
                    {flag_form(&id, "read", false, "Marcar como não lida", Icon::Mail)}
                </div>
            </header>

            {(blocked > 0).then(|| remote_banner(&id, blocked))}

            // O único `inner_html` do Ocinye Workspace. O conteúdo vem
            // higienizado do Ocinye Core, por lista de permissões: sem
            // `<script>`, sem `on*`, sem `javascript:`, sem `<iframe>`,
            // sem `<form>`. Ver `ocinye_core::modules::mail::sanitize`.
            <div class="oc-mail__body" inner_html=body_html></div>

            {(!attachments.is_empty()).then(|| attachment_list(&attachments))}

            {(!domains.is_empty()).then(|| view! {
                <p class="oc-mail__domains">
                    "Esta mensagem liga para: "
                    <span class="oc-mono">{domains.join(", ")}</span>
                </p>
            })}
        </article>
    }
}

/// Uma lista de endereços num campo do payload.
fn addresses(payload: &Value, key: &str) -> String {
    payload
        .get(key)
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_default()
}

/// Um botão que altera um estado da mensagem.
///
/// Formulário e não script: funciona sem JavaScript, e o `POST` deixa rasto no
/// Core em vez de mudar apenas o que se vê.
fn flag_form(
    id: &str,
    field: &'static str,
    value: bool,
    label: &'static str,
    kind: Icon,
) -> impl IntoView {
    let action = format!("/mail/message/{id}/flags");
    view! {
        <form method="post" action=action class="oc-mail__flag">
            <input type="hidden" name="field" value=field />
            <input type="hidden" name="value" value=value.to_string() />
            <button type="submit" class="oc-btn oc-btn--secondary" title=label>
                {icon(kind, 13)}
                {label}
            </button>
        </form>
    }
}

/// O aviso de conteúdo remoto bloqueado.
///
/// # Porque não há botão para o carregar
///
/// Havia, e não fazia nada. O botão ia a `?remote=1`, o Ocinye Core devolvia o
/// corpo com os `src` originais — e a Content Security Policy do Workspace, que
/// declara `img-src 'self' data:`, recusava cada um deles. A página recarregava,
/// o aviso desaparecia porque já nada estava por carregar, e as imagens
/// continuavam a não aparecer. O membro ficava com a impressão de que o pedido
/// tinha sido atendido.
///
/// Das duas saídas possíveis, alargar a CSP a origens de terceiros seria
/// desmontar a última barreira contra o rastreio por email para repor um botão.
/// A outra — servir o conteúdo remoto através do Ocinye, sem contactar o
/// remetente a partir do browser do membro — é uma funcionalidade por
/// construir, não uma correcção.
///
/// Fica o estado, dito por inteiro. Um botão que finge é pior do que uma
/// ausência declarada (`CLAUDE.md` §69, briefing §66).
fn remote_banner(_id: &str, blocked: u64) -> impl IntoView {
    view! {
        <div class="oc-callout oc-mail__banner" role="status">
            {icon(Icon::Shield, 15)}
            <p>
                {format!(
                    "{blocked} elemento(s) remoto(s) não foram carregados. O Ocinye \
                     Workspace não vai buscar conteúdo a servidores de terceiros: \
                     fazê-lo informaria quem enviou esta mensagem de que ela foi aberta."
                )}
            </p>
        </div>
    }
}

/// Os anexos, descritos.
///
/// Sem descarregamento nesta fase: o Ocinye OS não tem armazenamento de
/// objectos configurado, e um botão que falhasse ao ser carregado seria pior do
/// que um estado declarado (briefing §60, `CLAUDE.md` §69).
fn attachment_list(attachments: &[Value]) -> impl IntoView {
    let attachments = attachments.to_vec();
    view! {
        <section class="oc-mail__attachments">
            <h3>"Anexos"</h3>
            <ul>
                {attachments
                    .into_iter()
                    .map(|attachment| {
                        let name = text(&attachment, "filename", "anexo").to_owned();
                        let kind = text(&attachment, "content_type", "").to_owned();
                        let size = attachment
                            .get("size_bytes")
                            .and_then(Value::as_i64)
                            .unwrap_or(0);
                        view! {
                            <li class="oc-mail__attachment">
                                {icon(Icon::Attach, 13)}
                                <span class="oc-mail__attachment-name">{name}</span>
                                <span class="oc-mono oc-mail__attachment-meta">
                                    {format!("{kind} · {}", human_size(size))}
                                </span>
                                <span class="oc-unavailable" aria-disabled="true"
                                      title="A descarga de anexos ainda não está disponível.">
                                    "Descarregar"
                                </span>
                            </li>
                        }
                    })
                    .collect_view()}
            </ul>
        </section>
    }
}

/// Uma data ISO-8601 reduzida ao que cabe numa linha de lista.
///
/// Sem biblioteca de formatação: o Core devolve UTC em formato fixo, e cortar
/// no `T` é suficiente para o que a lista mostra. Uma dependência inteira para
/// isto seria desproporcionada (`CLAUDE.md` §54).
fn short_date(raw: &str) -> String {
    let (date, rest) = raw.split_once('T').unwrap_or((raw, ""));
    let time: String = rest.chars().take(5).collect();

    if time.is_empty() {
        date.to_owned()
    } else {
        format!("{date} {time}")
    }
}

/// Um tamanho em bytes, legível.
fn human_size(bytes: i64) -> String {
    let bytes = bytes.max(0);
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.0} kB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}

// ── O composer ──────────────────────────────────────────────────────────

/// O que o composer tem neste momento.
///
/// Existe porque o composer é re-renderizado com o que já lá estava: com uma
/// sugestão gerada, com um pedido de confirmação, ou com uma recusa. Devolver
/// os campos vazios perderia o trabalho de quem escreveu.
#[derive(Default)]
pub struct ComposeDraft {
    /// A caixa a partir da qual se envia.
    pub mailbox_id: String,
    /// Destinatários, separados por vírgula.
    pub to: String,
    /// Em cópia.
    pub cc: String,
    /// Assunto.
    pub subject: String,
    /// Corpo.
    pub body: String,
    /// A mensagem a que isto responde, quando é uma resposta.
    pub reply_to: Option<String>,
    /// Instrução dada à assistência, para não se perder ao regenerar.
    pub instruction: String,
    /// Confirmação de envio para fora da instituição, quando pedida.
    pub confirmation: Option<String>,
    /// Recusa ou erro a mostrar.
    pub error: Option<String>,
    /// Aviso de que o texto abaixo foi gerado e ainda não foi enviado.
    pub generated: bool,
}

/// O compositor, como janela sobre o correio.
///
/// # Porque é um painel e não uma página
///
/// Porque escrever é um acto que se faz a olhar para a caixa: para confirmar
/// um nome, reler o que se responde, ver o que entretanto chegou. Uma página à
/// parte tira isso, e obriga a voltar atrás para recuperar o que se tinha à
/// frente. Era uma página, e parecia um formulário de administração.
///
/// # Porque continua a ser um `<form>` que submete
///
/// Porque sem JavaScript continua a escrever-se e a enviar-se. As fichas de
/// destinatário, o redimensionar e o expandir são **melhorias**: se nenhuma
/// carregar, ficam os campos de texto e o botão, e a mensagem sai na mesma.
fn compositor_flutuante(view: &MailView, draft: &ComposeDraft) -> impl IntoView {
    let identidades: Vec<String> = view
        .boxes()
        .iter()
        .filter(|caixa| {
            caixa
                .get("may_send")
                .and_then(Value::as_bool)
                .unwrap_or(false)
        })
        .map(|caixa| text(caixa, "address", "").to_owned())
        .collect();

    let de = identidades.first().cloned().unwrap_or_default();
    let sem_identidade = de.is_empty();
    let responde = draft.reply_to.is_some();
    let titulo = if responde {
        "Responder"
    } else {
        "Nova mensagem"
    };

    let mailbox_id = draft.mailbox_id.clone();
    let reply_to = draft.reply_to.clone();
    let to = draft.to.clone();
    let cc = draft.cc.clone();
    let subject = draft.subject.clone();
    let corpo = draft.body.clone();
    let instrucao = draft.instruction.clone();
    let erro = draft.error.clone();
    let gerado = draft.generated;
    let confirmacao = draft.confirmation.clone();
    let cc_aberto = !cc.is_empty();

    view! {
        <div class="oc-comp" data-oc="compositor" role="dialog" aria-label=titulo>
            // A pega é o cabeçalho inteiro: agarrar por uma barra fina é
            // preciso de mais para uma janela que se quer mover à pressa.
            <header class="oc-comp__topo" data-oc="compositor-pega">
                <h2 class="oc-comp__titulo">{titulo}</h2>
                <div class="oc-comp__janela">
                    <button
                        type="button"
                        class="oc-icon-btn"
                        data-oc="compositor-expandir"
                        aria-pressed="false"
                        title="Expandir"
                    >
                        <span class="oc-sr">"Expandir o compositor"</span>
                        {icon(Icon::Filter, 14)}
                    </button>
                    // Fechar é uma ligação, e não um botão de JavaScript:
                    // fechar sem script tem de funcionar, e voltar ao correio
                    // é exactamente o que fechar significa.
                    <a class="oc-icon-btn" href="/mail" title="Fechar">
                        <span class="oc-sr">"Fechar o compositor"</span>
                        {icon(Icon::Close, 14)}
                    </a>
                </div>
            </header>

            {erro.map(|razao| view! {
                <p class="oc-comp__erro" role="alert">{razao}</p>
            })}

            {gerado.then(|| view! {
                <p class="oc-comp__gerado" role="status">
                    "O texto abaixo foi preparado pela assistência. Nada foi enviado:
                     leia, altere o que for preciso, e envie quando quiser."
                </p>
            })}

            <form class="oc-comp__forma" method="post" action="/mail/send">
                <input type="hidden" name="mailbox_id" value=mailbox_id />
                {reply_to.map(|id| view! {
                    <input type="hidden" name="reply_to" value=id />
                })}

                // O remetente é um facto, e não uma escolha: o Core resolve-o
                // a partir de quem está autenticado e recusa qualquer outro.
                <div class="oc-comp__linha oc-comp__linha--de">
                    <span class="oc-comp__rotulo">"De"</span>
                    {if sem_identidade {
                        view! {
                            <span class="oc-comp__sem-identidade">
                                "Não tem nenhuma caixa a partir da qual possa enviar."
                            </span>
                        }
                        .into_any()
                    } else {
                        view! {
                            <span class="oc-comp__de oc-mono">{de.clone()}</span>
                            <input type="hidden" name="from" value=de.clone() />
                        }
                        .into_any()
                    }}
                </div>

                {campo_de_destinatarios("to", "Para", &to, true)}

                <div class="oc-comp__linha oc-comp__linha--cc" data-oc="linha-cc" hidden=!cc_aberto>
                    {campo_de_destinatarios("cc", "Cc", &cc, false)}
                </div>

                <input
                    class="oc-comp__assunto"
                    type="text"
                    name="subject"
                    value=subject
                    placeholder="Assunto"
                    aria-label="Assunto"
                />

                <textarea
                    class="oc-comp__corpo"
                    name="body"
                    data-oc="compositor-corpo"
                    placeholder="Escreva a mensagem…"
                    aria-label="Mensagem"
                >{corpo}</textarea>

                // A instrução da assistência viaja com o formulário para não
                // se perder quando o texto é regenerado.
                <input type="hidden" name="instruction" value=instrucao />

                {confirmacao.map(|_| view! {
                    <label class="oc-comp__confirmar" for="mail-confirm">
                        <input type="checkbox" id="mail-confirm" name="confirmed" value="true" />
                        <span>
                            "Confirmo que pretendo enviar esta mensagem para fora da
                             instituição."
                        </span>
                    </label>
                })}

                <footer class="oc-comp__barra">
                    {assistencia_na_barra(view)}
                    <div class="oc-comp__enviar">
                        <button
                            type="submit"
                            class="oc-btn oc-btn--primary"
                            data-oc="compositor-enviar"
                            disabled=sem_identidade
                        >
                            {icon(Icon::Send, 14)}
                            "Enviar"
                        </button>
                    </div>
                </footer>
            </form>

            <span
                class="oc-comp__puxador"
                data-oc="compositor-puxador"
                aria-hidden="true"
            ></span>
        </div>
    }
}

/// Uma linha de destinatários, com fichas quando o browser as souber fazer.
///
/// # Porque o campo de texto continua a existir
///
/// Porque é ele que submete. As fichas são uma camada por cima: o JavaScript
/// lê este campo, desenha as fichas, e volta a escrevê-lo a cada alteração. Se
/// o script não correr, fica um campo de texto com endereços separados por
/// vírgula — que é feio e funciona.
fn campo_de_destinatarios(
    nome: &'static str,
    rotulo: &'static str,
    valor: &str,
    principal: bool,
) -> impl IntoView {
    let valor = valor.to_owned();
    let id = format!("mail-{nome}");
    view! {
        <div class="oc-comp__linha" data-oc="destinatarios" data-oc-campo=nome>
            <label class="oc-comp__rotulo" for=id.clone()>{rotulo}</label>
            <div class="oc-comp__campo">
                <div class="oc-chips" data-oc="fichas" hidden></div>
                <input
                    class="oc-comp__entrada"
                    type="text"
                    id=id
                    name=nome
                    value=valor
                    placeholder="Nome ou endereço"
                    autocomplete="off"
                    data-oc="destino-entrada"
                />
                <ul class="oc-sugestoes" data-oc="sugestoes" hidden></ul>
            </div>
            {principal.then(|| view! {
                // `Cc` é uma acção discreta, e não um campo vazio permanente:
                // a maioria das mensagens não leva cópia, e um campo que quase
                // nunca se usa a ocupar uma linha é ruído em todas as outras.
                <button type="button" class="oc-comp__cc" data-oc="mostrar-cc">"Cc"</button>
            })}
        </div>
    }
}

/// A assistência, na barra e não num cartão.
///
/// # As três situações, ditas onde acontecem
///
/// Pode e há; pode e não há; não pode. Eram um cartão inteiro com um título e
/// um parágrafo — do tamanho de um erro, para dizer que uma conveniência está
/// em falta. Aqui são a barra: com IA, os verbos; sem ela, uma linha que
/// explica e não estorva; sem autorização, nada, porque um controlo que nunca
/// vai funcionar não deve ocupar espaço.
fn assistencia_na_barra(view: &MailView) -> impl IntoView {
    if !view.may_use_ai() {
        return view! { <div class="oc-comp__ia"></div> }.into_any();
    }

    if !view.ai_available() {
        return view! {
            <div class="oc-comp__ia">
                <span class="oc-comp__ia-nota">
                    "A assistência de escrita não está disponível nesta instalação."
                </span>
                <a class="oc-comp__ia-link" href="/ai">"Porquê"</a>
            </div>
        }
        .into_any();
    }

    view! {
        <div class="oc-comp__ia" data-oc="assistencia">
            {[
                (ComposeAction::Proofread, "Corrigir"),
                (ComposeAction::Clarify, "Mais claro"),
                (ComposeAction::MoreFormal, "Mais formal"),
                (ComposeAction::Shorter, "Mais curto"),
                (ComposeAction::Translate, "Traduzir"),
            ]
            .into_iter()
            .map(|(accao, rotulo)| view! {
                // Submete o mesmo formulário para outra rota. É o mecanismo do
                // próprio HTML: nenhum script decide para onde isto vai, e o
                // texto que a pessoa escreveu segue com ele.
                <button
                    type="submit"
                    class="oc-comp__ia-accao"
                    formaction="/mail/assist"
                    name="action"
                    value=accao.as_str()
                >
                    {rotulo}
                </button>
            })
            .collect_view()}
        </div>
    }
    .into_any()
}

/// O ecrã de composição.
pub fn compose(view: &MailView, draft: &ComposeDraft) -> impl IntoView {
    let boxes = view.boxes().to_vec();
    let can_send = view.can_send();

    let identities: Vec<(String, bool)> = boxes
        .iter()
        .filter(|mailbox| {
            mailbox
                .get("may_send")
                .and_then(Value::as_bool)
                .unwrap_or(false)
        })
        .map(|mailbox| (text(mailbox, "address", "").to_owned(), true))
        .collect();

    let no_identity = identities.is_empty();

    view! {
        <div class="oc-page">
            <div class="oc-head">
                <div class="oc-head__text">
                    <h1>{if draft.reply_to.is_some() { "Responder" } else { "Nova mensagem" }}</h1>
                    <p>
                        "Nada é enviado antes de carregar em «Enviar». A assistência de escrita
                         devolve texto para este formulário e nunca envia."
                    </p>
                </div>
                <div class="oc-head__actions">
                    {button(Button::new("Voltar ao correio", Variant::Secondary).href("/mail"))}
                </div>
            </div>

            {draft.error.as_ref().map(|reason| view! {
                <div class="oc-callout oc-callout--error oc-mail__banner" role="alert">
                    {icon(Icon::Shield, 15)}
                    <p>{reason.clone()}</p>
                </div>
            })}

            {draft.confirmation.as_ref().map(|reason| view! {
                <div class="oc-callout oc-callout--warning oc-mail__banner" role="alert">
                    {icon(Icon::Shield, 15)}
                    <p>{reason.clone()}</p>
                </div>
            })}

            {draft.generated.then(|| view! {
                // A distinção mais importante deste ecrã, dita por escrito
                // (briefing §15).
                <div class="oc-callout oc-mail__banner oc-mail__banner--ai" role="status">
                    <span class="oc-btn__dot"></span>
                    <p>
                        "Texto sugerido pela assistência do Ocinye OS. "
                        <strong>"Ainda não foi enviado."</strong>
                        " Reveja-o e edite-o antes de enviar."
                    </p>
                </div>
            })}

            // Um só formulário, dois destinos. O botão de assistência submete para
            // `/mail/assist`, que devolve texto; o de envio submete para
            // `/mail/send`, que é a única rota que fala com o serviço de correio.
            <form class="oc-mail__compose" method="post" action="/mail/send">
                <input type="hidden" name="mailbox_id" value=draft.mailbox_id.clone() />
                {draft.reply_to.as_ref().map(|id| view! {
                    <input type="hidden" name="reply_to" value=id.clone() />
                })}

                {if no_identity {
                    view! {
                        <p class="oc-mail__no-identity" role="status">
                            "Não possui nenhuma identidade de correio a partir da qual possa enviar."
                        </p>
                    }
                    .into_any()
                } else if identities.len() == 1 {
                    // Uma identidade não é uma escolha.
                    //
                    // Um selector com uma opção pede uma decisão que não
                    // existe, e sugere que o remetente é escolhível — quando o
                    // Core o determina a partir de quem está a enviar e recusa
                    // qualquer outro. Mostra-se o endereço, e envia-se num
                    // campo que ninguém edita.
                    let unica = identities
                        .first()
                        .map(|(endereco, _)| endereco.clone())
                        .unwrap_or_default();
                    let rotulo = unica.clone();
                    view! {
                        <div class="oc-field">
                            <span class="oc-field__label">"De"</span>
                            <p class="oc-mail__de oc-mono">{rotulo}</p>
                            <input type="hidden" name="from" value=unica />
                        </div>
                    }
                    .into_any()
                } else {
                    select("mail-from", "De", "from", identities).into_any()
                }}

                {field_with_value("mail-to", "Para", "to",
                    "endereços separados por vírgula", "text", draft.to.clone())}
                {field_with_value("mail-cc", "Cc", "cc",
                    "opcional", "text", draft.cc.clone())}
                {field_with_value("mail-subject", "Assunto", "subject",
                    "assunto da mensagem", "text", draft.subject.clone())}
                {textarea_with_value("mail-body", "Mensagem", "body",
                    "Escreva a mensagem…", 260, draft.body.clone())}

                {assistance_panel(view, draft)}

                {draft.confirmation.is_some().then(|| view! {
                    <label class="oc-check" for="mail-confirm">
                        <input type="checkbox" id="mail-confirm" name="confirmed" value="true" />
                        <span>
                            "Confirmo que pretendo enviar esta mensagem para fora da instituição."
                        </span>
                    </label>
                })}

                <div class="oc-mail__compose-actions">
                    {if can_send && !no_identity {
                        view! {
                            <button type="submit" class="oc-btn oc-btn--primary">
                                {icon(Icon::Send, 13)}
                                "Enviar"
                            </button>
                        }
                        .into_any()
                    } else {
                        view! {
                            <span class="oc-btn oc-btn--primary oc-unavailable" aria-disabled="true"
                                  title="O serviço de envio não está disponível.">
                                "Enviar"
                            </span>
                        }
                        .into_any()
                    }}
                    <a class="oc-btn oc-btn--secondary" href="/mail">"Descartar"</a>
                </div>
            </form>
        </div>
    }
}

/// O painel de assistência de escrita.
///
/// Vive dentro do formulário para poder ler o que já foi escrito, mas o seu
/// botão submete para outra rota. As três situações — pode e há, pode e não há,
/// não pode — dizem-se por extenso: um painel apagado sem explicação é tão
/// opaco como um controlo que não faz nada (briefing §61).
fn assistance_panel(view: &MailView, draft: &ComposeDraft) -> impl IntoView {
    let may = view.may_use_ai();
    let available = view.ai_available();
    let instruction = draft.instruction.clone();

    let head = section_head("Assistência de escrita", None, None);

    if !may {
        return card(
            head,
            view! {
                <p class="oc-mail__assist-note">
                    "Não possui autorização para usar a assistência de escrita no correio.
                     Escrever, responder e enviar continuam disponíveis."
                </p>
            },
        )
        .into_any();
    }

    if !available {
        return card(
            head,
            view! {
                <p class="oc-mail__assist-note">
                    "A assistência de escrita depende de uma capacidade de IA do Ocinye OS,
                     que não está actualmente disponível. Escrever, responder e enviar não
                     dependem dela e continuam a funcionar normalmente."
                </p>
                <a class="oc-mail__assist-link" href="/ai">"Ver o estado da inteligência"</a>
            },
        )
        .into_any();
    }

    card(
        head,
        view! {
            <p class="oc-mail__assist-note">
                "A assistência devolve texto para o campo «Mensagem». Nunca envia."
            </p>

            // As acções vêm do contrato, não de uma lista escrita aqui: uma
            // acção acrescentada ao Core aparece sozinha, e uma removida
            // desaparece em vez de ficar a produzir recusas.
            {select_labelled(
                "mail-assist-action",
                "O que pretende",
                "action",
                ComposeAction::all()
                    .into_iter()
                    .map(|action| {
                        SelectOption::new(action.as_str(), action.label())
                            .selected(action == ComposeAction::Generate)
                    })
                    .collect(),
            )}

            {textarea_with_value(
                "mail-instruction",
                "Instrução",
                "instruction",
                "Descreva o que pretende. O conteúdo do email é tratado como dados, nunca como instruções.",
                92,
                instruction,
            )}

            // Submete para `/mail/assist`. O `formaction` é o mecanismo do
            // próprio HTML: nenhum script, nenhuma ambiguidade sobre qual das
            // duas rotas é chamada.
            <button
                type="submit"
                formaction="/mail/assist"
                class="oc-btn oc-btn--secondary"
            >
                <span class="oc-btn__dot"></span>
                "Gerar sugestão"
            </button>
        },
    )
    .into_any()
}

// ── Definições ──────────────────────────────────────────────────────────

/// As preferências de correio do membro, e o estado do serviço.
/// A ligação de uma caixa, dentro das definições de correio.
///
/// # Porque a senha se escreve aqui e não no `.env`
///
/// Porque é a senha **desta pessoa**, e não da instalação. Quem a tem é ela, e
/// pedir que a entregue a quem administra para ser escrita num ficheiro é pedir
/// que a partilhe — que é o contrário do que uma credencial pessoal significa
/// (ADR-0409).
///
/// # O que este formulário nunca faz
///
/// Não devolve a senha. Uma vez guardada, é cifrada e só sai no momento de abrir
/// uma sessão de IMAP. O campo abre sempre vazio: preenchê-lo com a senha
/// existente seria pô-la a atravessar o browser outra vez, e a cada visita.
fn ligacao_da_caixa(caixa: &Value) -> impl IntoView {
    let id = text(caixa, "id", "").to_owned();
    let endereco = text(caixa, "address", "").to_owned();
    let ligada = caixa
        .get("connected")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let sugestao = endereco.clone();

    view! {
        <div class="oc-mail__ligacao" data-oc="ligacao-caixa">
            <p class="oc-mail__ligacao-caixa">
                <strong>{endereco}</strong>
                " "
                {if ligada {
                    view! { <span class="oc-badge oc-badge--ok">"Ligada"</span> }.into_any()
                } else {
                    view! { <span class="oc-badge">"Por ligar"</span> }.into_any()
                }}
            </p>

            {if ligada {
                view! {
                    <form
                        data-oc="desligar-caixa"
                        method="post"
                        action=format!("/mail/{id}/disconnect")
                    >
                        <button type="submit" class="oc-btn oc-btn--secondary">
                            "Desligar e esquecer a senha"
                        </button>
                    </form>
                }
                .into_any()
            } else {
                view! {
                    <form
                        class="oc-mail__ligacao-form"
                        data-oc="ligar-caixa"
                        method="post"
                        action=format!("/mail/{id}/connect")
                    >
                        // O endereço **não** é editável, e não é enviado.
                        //
                        // Era um campo de texto pré-preenchido com o endereço
                        // da caixa. Um campo editável convida a editar, e o
                        // que se editava era a conta com que o Ocinye se
                        // autentica no servidor de correio — deixando o
                        // browser escolher em nome de quem a sessão abria.
                        //
                        // O Core resolve-o sozinho, e é a única resolução que
                        // pode valer:
                        //
                        //   principal → MemberId → endereço institucional
                        //
                        // Fica visível, em texto, porque a pessoa precisa de
                        // saber que caixa está a ligar. Não fica como entrada.
                        <input
                            type="text"
                            name="_username"
                            value=sugestao
                            autocomplete="username"
                            aria-hidden="true"
                            tabindex="-1"
                            readonly
                            class="oc-sr"
                        />
                        <label class="oc-campo">
                            <span class="oc-campo__rotulo">"Senha da caixa"</span>
                            // A senha do correio, e não a do Ocinye. São coisas
                            // distintas, e nenhuma serve para obter a outra.
                            <input
                                class="oc-entrada"
                                type="password"
                                name="password"
                                autocomplete="new-password"
                                required=true
                            />
                        </label>
                        <button type="submit" class="oc-btn oc-btn--primary">"Ligar caixa"</button>
                    </form>
                }
                .into_any()
            }}
        </div>
    }
}

pub fn settings(view: &MailView, preferences: &Value) -> impl IntoView {
    let signature = preferences
        .get("signature")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_owned();
    // Qualquer valor irreconhecível bloqueia: uma preferência corrompida não
    // pode voltar a ligar o rastreio (ver `RemoteContentPolicy::parse`).
    let remote_policy =
        RemoteContentPolicy::parse(text(preferences, "remote_content_policy", "block"));

    let endpoints: Vec<String> = view
        .status
        .get("endpoints")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default();

    let adapter = text(&view.status, "adapter", "desconhecido").to_owned();
    let can_read = view.can_read();
    let can_send = view.can_send();
    let ai = view.ai_available();
    let detail = view.detail();

    view! {
        <div class="oc-page">
            <div class="oc-head">
                <div class="oc-head__text">
                    <h1>"Definições de correio"</h1>
                    <p>"As suas preferências e o estado do serviço."</p>
                </div>
                <div class="oc-head__actions">
                    {button(Button::new("Voltar ao correio", Variant::Secondary).href("/mail"))}
                </div>
            </div>

            {card(
                section_head("As suas caixas", None, None),
                view! {
                    <p class="oc-muted oc-mail__ligacao-nota">
                        "A senha de cada caixa é sua, fica cifrada, e nunca volta a ser mostrada."
                    </p>
                    {view.boxes().iter().map(ligacao_da_caixa).collect_view()}
                    {view.boxes().is_empty().then(|| view! {
                        <p class="oc-muted">
                            "Ainda não há nenhuma caixa institucional associada a si."
                        </p>
                    })}
                },
            )}

            {card(
                section_head("Disposição do Correio", None, None),
                view! {
                    <p class="oc-muted">
                        "As larguras dos painéis e as pastas recolhidas ficam guardadas neste
                         browser, e só aqui: são a sua maneira de ler, e não um dado da
                         instituição."
                    </p>
                    <button
                        type="button"
                        class="oc-btn oc-btn--secondary"
                        data-oc="repor-disposicao"
                    >
                        "Repor disposição"
                    </button>
                    <p class="oc-muted oc-mail__reposto" data-oc="disposicao-reposta" hidden>
                        "Reposta. Volte ao Correio para a ver."
                    </p>
                },
            )}

            <div class="oc-grid oc-grid--2">
                {card(
                    section_head("Preferências", None, None),
                    view! {
                        <form method="post" action="/mail/settings">
                            {textarea_with_value(
                                "mail-signature",
                                "Assinatura",
                                "signature",
                                "Acrescentada ao fim das mensagens que escrever.",
                                120,
                                signature,
                            )}

                            {select_labelled(
                                "mail-remote",
                                "Conteúdo remoto",
                                "remote_content_policy",
                                vec![
                                    SelectOption::new(
                                        RemoteContentPolicy::Block.as_str(),
                                        "Nunca carregar",
                                    )
                                    .selected(remote_policy == RemoteContentPolicy::Block),
                                    SelectOption::new(
                                        RemoteContentPolicy::AllowOnce.as_str(),
                                        "Perguntar em cada mensagem",
                                    )
                                    .selected(remote_policy == RemoteContentPolicy::AllowOnce),
                                    SelectOption::new(
                                        RemoteContentPolicy::AllowKnownSenders.as_str(),
                                        "Carregar de remetentes que eu permitir",
                                    )
                                    .selected(
                                        remote_policy == RemoteContentPolicy::AllowKnownSenders,
                                    ),
                                ],
                            )}

                            <p class="oc-mail__assist-note">
                                "Carregar conteúdo remoto informa quem enviou a mensagem de que
                                 ela foi aberta. O Ocinye OS não o carrega por omissão."
                            </p>

                            <button type="submit" class="oc-btn oc-btn--primary">"Guardar"</button>
                        </form>
                    },
                )}

                {card(
                    section_head("Estado do serviço", None, None),
                    view! {
                        <ul class="oc-mail__status">
                            <li>
                                <span>"Leitura"</span>
                                {state_badge(can_read)}
                            </li>
                            <li>
                                <span>"Envio"</span>
                                {state_badge(can_send)}
                            </li>
                            <li>
                                <span>"Assistência de escrita"</span>
                                {state_badge(ai)}
                            </li>
                            <li>
                                <span>"Adaptador"</span>
                                <span class="oc-mono">{adapter}</span>
                            </li>
                        </ul>

                        <p class="oc-mail__assist-note">{detail}</p>

                        // Anfitriões e portos. Nenhuma credencial aparece aqui, e
                        // nenhuma pode: o Core não as devolve (briefing §59).
                        {(!endpoints.is_empty()).then(|| view! {
                            <p class="oc-mono oc-mail__endpoints">{endpoints.join(" · ")}</p>
                        })}
                    },
                )}
            </div>
        </div>
    }
}

fn state_badge(ok: bool) -> impl IntoView {
    if ok {
        badge("Disponível", Tone::Ok).into_any()
    } else {
        badge("Indisponível", Tone::Gray).into_any()
    }
}

#[cfg(test)]
mod integridade {
    use super::*;
    use serde_json::json;

    fn sem_servico() -> MailView {
        MailView {
            status: json!({
                "can_read": false,
                "can_send": false,
                "detail": "O correio institucional ainda não foi configurado.",
            }),
            sync_notice: None,
            mailboxes: json!({"items": []}),
            active_mailbox: None,
            folder: "inbox".to_owned(),
            query: String::new(),
        }
    }

    pub(super) fn viewer() -> Viewer {
        Viewer {
            zona: "UTC".to_owned().try_into().expect("fuso conhecido"),
            avatar: ocinye_contracts::AvatarChoice::Initials,
            email: Some("jmanuel@ocinye.com".to_owned()),
            session_expires_in: Some(std::time::Duration::from_secs(8 * 3600)),
            name: "Teste".to_owned(),
            organisation: "Ocinye".to_owned(),
            core_status: crate::ui::shell::CoreStatus::Ok,
            temporal: Vec::new(),
            temporal_failure: None,
            unread: 0,
            modules: Vec::new(),
            capabilities: ocinye_contracts::Permission::all()
                .into_iter()
                .map(|p| p.as_str().to_owned())
                .collect(),
        }
    }

    /// Serviço ausente não é caixa vazia.
    ///
    /// São estados diferentes e parecem-se: uma lista de pastas com contagens a
    /// zero diria «não tem mensagens» quando o que não há é serviço. O membro
    /// concluiria que ninguém lhe escreveu — e agiria em cima disso.
    #[test]
    fn sem_configuracao_o_ecra_diz_o_e_nao_finge_uma_caixa() {
        let html = mail(&viewer(), &sem_servico(), &json!({"items": []}), None, None).to_html();

        assert!(
            html.contains("não está configurado"),
            "o ecrã não declarou a ausência de serviço"
        );
        assert!(
            !html.contains("oc-mail__rail"),
            "apareceu uma lista de caixas sem haver serviço"
        );
        assert!(
            !html.contains("Não tem mensagens"),
            "a ausência de serviço foi apresentada como caixa vazia"
        );
    }
}

#[cfg(test)]
mod uma_explicacao_so {
    use super::*;
    use serde_json::json;

    /// Constrói uma vista com serviço em baixo e uma caixa cujo último erro de
    /// sincronização é a razão indicada.
    fn com_erro_de_caixa(razao_do_servico: &str, erro_da_caixa: &str) -> MailView {
        MailView {
            status: json!({
                "can_read": false,
                "can_send": false,
                "detail": razao_do_servico,
            }),
            sync_notice: None,
            mailboxes: json!([{
                "id": "11111111-1111-4111-8111-111111111111",
                "address": "fidel.monteiro@ocinye.com",
                "kind": "personal",
                "may_send": false,
                "last_sync_error": erro_da_caixa,
                "unread": [],
            }]),
            active_mailbox: None,
            folder: "inbox".to_owned(),
            query: String::new(),
        }
    }

    fn html_de(view: &MailView) -> String {
        mail(&super::integridade::viewer(), view, &json!([]), None, None).to_html()
    }

    /// A mesma razão não aparece duas vezes.
    ///
    /// # O defeito que isto guarda
    ///
    /// A página mostrava o estado do serviço em cima e, dentro da coluna das
    /// caixas, o último erro de sincronização da caixa. Com o correio por
    /// configurar as duas frases são a mesma, e o ecrã ficava com duas caixas
    /// cor de laranja a dizer o mesmo — que se lê como dois problemas.
    #[test]
    fn a_mesma_razao_nao_aparece_duas_vezes() {
        let razao = "O correio institucional ainda não foi configurado nesta \
                     instalação do Ocinye OS.";
        let html = html_de(&com_erro_de_caixa(razao, razao));

        assert_eq!(
            html.matches(razao).count(),
            1,
            "a mesma razão apareceu mais do que uma vez:\n{html}"
        );
    }

    /// Uma razão diferente continua a aparecer.
    ///
    /// É o caso que importa: o serviço responde, e é a credencial **desta**
    /// caixa que não entra (ADR-0409). Suprimir isto esconderia o problema de
    /// uma pessoa atrás do estado geral.
    #[test]
    fn uma_razao_diferente_continua_a_aparecer() {
        let html = html_de(&com_erro_de_caixa(
            "O serviço de correio não está a responder nesta instalação.",
            "O serviço de correio recusou as credenciais desta caixa.",
        ));

        assert!(
            html.contains("não está a responder nesta instalação"),
            "o estado do serviço desapareceu"
        );
        assert!(
            html.contains("recusou as credenciais desta caixa"),
            "o erro próprio da caixa foi suprimido, e não devia"
        );
    }
}

#[cfg(test)]
mod tres_ausencias {
    use super::*;
    use serde_json::json;

    fn vista(transporte: bool, ligada: bool, detalhe: &str) -> MailView {
        MailView {
            status: json!({
                "can_read": false,
                "can_send": false,
                "transport_configured": transporte,
                "mailbox_linked": ligada,
                "detail": detalhe,
            }),
            sync_notice: None,
            mailboxes: json!([]),
            active_mailbox: None,
            folder: "inbox".to_owned(),
            query: String::new(),
        }
    }

    fn html_de(view: &MailView) -> String {
        mail(&super::integridade::viewer(), view, &json!([]), None, None).to_html()
    }

    /// Uma caixa por ligar não é um serviço em baixo.
    ///
    /// # O defeito que isto guarda
    ///
    /// «Não consegue ler» tinha uma página só, e mandava sempre à
    /// Administração. Quem ainda não tinha ligado a sua caixa era mandado
    /// pedir a outra pessoa o que só ele podia fazer.
    #[test]
    fn quem_nao_ligou_a_caixa_tem_o_caminho_para_a_ligar() {
        let html = html_de(&vista(
            true,
            false,
            "A sua caixa de correio ainda não está ligada.",
        ));

        assert!(
            html.contains("ainda não está ligada"),
            "o ecrã não diz que a caixa é que falta:\n{html}"
        );
        assert!(
            html.contains("/mail/settings"),
            "a acção não leva a onde se liga a caixa"
        );
        assert!(
            !html.contains("não está configurado"),
            "chamou configuração a um serviço que está configurado"
        );
    }

    /// Um serviço em baixo não pede uma acção a quem lê.
    #[test]
    fn servico_em_baixo_diz_que_nada_se_perdeu() {
        let html = html_de(&vista(
            true,
            true,
            "O serviço de correio não está a responder nesta instalação.",
        ));

        assert!(html.contains("não está a responder"));
        assert!(
            html.contains("nada se perdeu"),
            "não disse a quem espera correio que as mensagens continuam lá"
        );
        assert!(
            !html.contains("ainda não está ligada"),
            "disse que a caixa não está ligada, e está"
        );
    }

    /// Sem transporte, continua a ser uma questão de configuração.
    #[test]
    fn sem_transporte_e_configuracao() {
        let html = html_de(&vista(
            false,
            false,
            "O correio institucional não está configurado nesta instalação.",
        ));

        assert!(html.contains("não está configurado"));
        assert!(
            html.contains("/admin"),
            "quem administra é quem resolve, e o ecrã não o leva lá"
        );
        assert!(
            !html.contains("Ligar a minha caixa"),
            "ofereceu ligar uma caixa a um servidor que não existe"
        );
    }
}

#[cfg(test)]
mod uma_pagina_coerente {
    use super::*;
    use serde_json::json;

    fn vista(ligada: bool, pode_ler: bool) -> MailView {
        MailView {
            status: json!({
                "can_read": pode_ler,
                "can_send": pode_ler,
                "transport_configured": true,
                "mailbox_linked": ligada,
                "detail": if ligada {
                    "O serviço de correio não está a responder nesta instalação."
                } else {
                    "A sua caixa de correio ainda não está ligada."
                },
            }),
            sync_notice: None,
            mailboxes: json!([{
                "id": "11111111-1111-4111-8111-111111111111",
                "address": "fidel.monteiro@ocinye.com",
                "kind": "personal",
                "may_send": false,
                "unread": [],
            }]),
            active_mailbox: None,
            folder: "inbox".to_owned(),
            query: String::new(),
        }
    }

    fn html_de(view: &MailView) -> String {
        mail(
            &super::integridade::viewer(),
            view,
            &json!({"items": []}),
            None,
            None,
        )
        .to_html()
    }

    /// Uma pasta que não está a ser lida não se declara vazia.
    ///
    /// # O defeito que isto guarda
    ///
    /// O aviso em cima dizia «a sua caixa ainda não está ligada» e a coluna
    /// por baixo dizia «Nenhuma mensagem nesta pasta». Duas leituras
    /// contraditórias no mesmo ecrã — e a mais tranquilizadora era a falsa:
    /// quem a lesse concluía que não tinha recebido nada.
    #[test]
    fn uma_pasta_que_nao_se_le_nao_se_declara_vazia() {
        let html = html_de(&vista(false, false));

        assert!(
            !html.contains("Nenhuma mensagem nesta pasta"),
            "a coluna declarou a pasta vazia enquanto o aviso dizia outra coisa:\n{html}"
        );
        assert!(
            html.contains("não quer dizer que não tenha recebido nada"),
            "a coluna não diz que uma pasta vazia aqui não significa caixa vazia"
        );
    }

    /// O caminho aparece quando é da própria pessoa, e só então.
    ///
    /// Um serviço em baixo não se resolve indo às definições. Oferecer lá um
    /// botão seria oferecer uma acção que não faz nada — Dead UI com a forma
    /// de ajuda.
    #[test]
    fn a_accao_aparece_so_quando_e_de_quem_le() {
        let por_ligar = html_de(&vista(false, false));
        assert!(
            por_ligar.contains("Ligar a minha caixa"),
            "quem tem de ligar a caixa não tem o caminho para o fazer"
        );

        let servico_em_baixo = html_de(&vista(true, false));
        assert!(
            !servico_em_baixo.contains("Ligar a minha caixa"),
            "ofereceu ligar uma caixa que já está ligada, para um problema que \
             não é dela"
        );
    }
}
