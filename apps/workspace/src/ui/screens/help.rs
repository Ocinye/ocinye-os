//! Ajuda do Ocinye Workspace.
//!
//! # Para quem isto é escrito
//!
//! Para quem **usa** o Ocinye OS, não para quem o constrói. Nada aqui aponta
//! para `docs/`, para o repositório ou para decisões de implementação: um
//! membro que procura saber porque uma tabela está vazia não precisa de saber
//! que módulo a serve.
//!
//! # Porque é conteúdo e não um sistema
//!
//! É texto versionado com o código, e é de propósito. A ajuda descreve o
//! produto **que existe hoje**, e por isso muda quando o produto muda — no
//! mesmo commit, revista pelas mesmas pessoas. Um CMS separado envelheceria por
//! sua conta, e uma ajuda desactualizada é pior do que nenhuma: descreve com
//! confiança um sistema que já não é aquele.
//!
//! Não há pesquisa aqui. As âncoras chegam para este tamanho, e uma caixa de
//! pesquisa que filtrasse texto estático prometeria uma capacidade que não
//! existe.

use leptos::prelude::*;

use crate::ui::components::section_head;

/// Uma secção da ajuda, com âncora própria.
fn seccao(
    ancora: &'static str,
    titulo: &'static str,
    corpo: impl IntoView + 'static,
) -> impl IntoView {
    view! {
        <section class="oc-card oc-mb-5" id=ancora>
            {section_head(titulo, None, None)}
            <div class="oc-card__body">{corpo}</div>
        </section>
    }
}

/// Um parágrafo de ajuda.
fn p(texto: &'static str) -> impl IntoView {
    view! { <p class="oc-t-prose oc-mb-5">{texto}</p> }
}

/// Uma entrada de glossário: o estado, e o que significa de facto.
fn estado(nome: &'static str, significado: &'static str) -> impl IntoView {
    view! {
        <div class="oc-list__row">
            <span class="oc-badge oc-badge--gray">{nome}</span>
            <span class="oc-fill oc-t-cell-2">{significado}</span>
        </div>
    }
}

/// O ecrã de ajuda.
pub fn help() -> impl IntoView {
    view! {
        <div class="oc-page oc-page--narrow">
            <div class="oc-head">
                <div class="oc-head__text">
                    <h1>"Ajuda"</h1>
                    <p>"Ajuda do Ocinye Workspace · estado actual do produto."</p>
                </div>
            </div>

            <nav class="oc-card oc-card__body oc-mb-5" aria-label="Nesta página">
                <div class="oc-t-meta oc-mb-5">"NESTA PÁGINA"</div>
                <div class="oc-col oc-gap-2">
                    <a href="#comecar">"Começar"</a>
                    <a href="#investigacao">"Investigação"</a>
                    <a href="#conhecimento">"Conhecimento e Dados"</a>
                    <a href="#tempo">"Tempo e Calendário"</a>
                    <a href="#correio">"Correio"</a>
                    <a href="#inteligencia">"Ocinye AI, Agentes e Computação"</a>
                    <a href="#institucional">"Institucional"</a>
                    <a href="#conta">"Conta e Segurança"</a>
                    <a href="#estados">"Estados do sistema"</a>
                </div>
            </nav>

            {seccao(
                "comecar",
                "COMEÇAR",
                view! {
                    {p("O Ocinye Workspace é onde o trabalho da instituição acontece: \
                        unidades, ideias, projectos, conhecimento, dados e correio. \
                        A barra da esquerda mostra a instituição inteira — os ecrãs a que \
                        não tem acesso aparecem esbatidos, para que saiba que existem.")}
                    {p("A Home reúne o que precisa da sua atenção. O Meu Trabalho mostra o \
                        que lhe está atribuído e a investigação em que participa — não tudo \
                        o que consegue ver, que é outra coisa e mais.")}
                    {p("A barra de pesquisa no topo procura em toda a instituição, dentro \
                        do que lhe é acessível. Abre também com ⌘K.")}
                    <p class="oc-t-prose">
                        "Ir para "<a href="/">"Home"</a>" · "
                        <a href="/my-work">"O Meu Trabalho"</a>
                    </p>
                },
            )}

            {seccao(
                "investigacao",
                "INVESTIGAÇÃO",
                view! {
                    {p("Uma Unidade é o âmbito institucional onde a investigação acontece. \
                        As ideias e os projectos nascem dentro de uma, e a filiação numa \
                        unidade é o que dá acesso ao trabalho que lá vive.")}
                    {p("Uma Ideia é exploratória. Nem todas se tornam projectos, e isso é um \
                        desfecho legítimo. Quando uma ideia amadurece até candidatura, pode \
                        ser promovida a Projecto — e o Research Workspace acompanha-a, com \
                        tudo o que foi reunido enquanto se explorava.")}
                    {p("Por isso não existe «criar projecto do zero»: um projecto nasce de \
                        uma ideia, e essa origem fica registada.")}
                    <p class="oc-t-prose">
                        "Ir para "<a href="/units">"Unidades"</a>" · "
                        <a href="/ideas">"Ideias"</a>" · "
                        <a href="/projects">"Projectos"</a>
                    </p>
                },
            )}

            {seccao(
                "conhecimento",
                "CONHECIMENTO E DADOS",
                view! {
                    {p("Referências, notas, documentos e datasets pertencem ao Research \
                        Workspace onde a investigação que os usa acontece. As páginas \
                        Conhecimento, Bibliografia e Dados reúnem o que alcança em todos \
                        eles — reúnem apenas, não mudam a quem pertencem.")}
                    {p("Por isso, ao criar uma referência ou um dataset a partir dessas \
                        páginas, escolhe primeiro o ambiente de destino. Só aparecem os \
                        ambientes onde tem autorização para criar.")}
                    {p("Cada recurso tem uma classificação — PUBLIC, INTERNAL, CONFIDENTIAL \
                        ou RESTRICTED — e ela pode ser mais restrita do que a do ambiente que \
                        o contém. Se um recurso não aparece, é porque a sua classificação ou \
                        a filiação necessária não o alcançam; nunca porque desapareceu.")}
                    {p("Resultados ainda não existe no Ocinye OS. Aparece no ecrã de \
                        Conhecimento como não implementado, e não como zero — zero diria que \
                        a consulta correu e não encontrou nada.")}
                    <p class="oc-t-prose">
                        "Ir para "<a href="/knowledge">"Conhecimento"</a>" · "
                        <a href="/bibliography">"Bibliografia"</a>" · "
                        <a href="/datasets">"Dados"</a>
                    </p>
                },
            )}

            {seccao(
                "arranque",
                "QUANDO O SISTEMA ARRANCA",
                view! {
                    {p("Ao abrir o Ocinye OS, a primeira coisa que aparece é o estado do \
                        sistema. Não é um ecrã de espera: é o Ocinye Core a dizer se está em \
                        condições de operar, antes de lhe pedir a palavra-passe.")}
                    {p("«Sistema pronto» significa que o núcleo está operacional. \
                        «Pronto com limitações» significa que uma ou mais capacidades \
                        opcionais estão indisponíveis — o correio, por exemplo — e o \
                        trabalho institucional segue à mesma; o arranque diz quais.")}
                    {p("«Não foi possível iniciar» significa que uma dependência essencial \
                        não está disponível, e por isso não há como entrar. «Sem resposta» é \
                        outra coisa: não chegámos a saber o que o Core diria. A diferença \
                        importa — numa sabe-se o que se passa, na outra não.")}
                    {p("Nos dois casos há um botão para tentar de novo, e ele volta mesmo a \
                        perguntar. Se o sistema entretanto ficou em condições, segue.")}
                    {p("Depois de entrar, a barra superior continua a mostrar o mesmo \
                        estado. O arranque não volta a aparecer a cada passo: é a porta de \
                        entrada, e não um vigilante.")}
                    {p("Se seguiu uma ligação para um sítio concreto, é para lá que vai \
                        depois de o sistema arrancar e de a sua sessão ser verificada — e \
                        não para a página inicial.")}
                },
            )}

            {seccao(
                "tempo",
                "TEMPO E CALENDÁRIO",
                view! {
                    {p("A hora na barra superior abre o Centro Temporal: o que tem hoje, o \
                        que vem a seguir, e os lembretes por ver. É um relance e um sítio \
                        de onde agir — as vistas completas vivem no Calendário.")}
                    {p("O Calendário tem quatro vistas da mesma agenda: Hoje, Semana, Mês \
                        e Agenda. Todas mostram exactamente o que tem acesso a ver; o que \
                        muda entre elas é a forma, nunca o conteúdo.")}
                    {p("Um evento pode ter hora ou ser de dia inteiro. Com hora, indica \
                        também a zona horária — «14:00 em Paris» continua a ser 14:00 em \
                        Paris para quem estiver em Luanda, e o Ocinye Core guarda as duas \
                        coisas. Se escolher uma hora que não existe nesse dia, por causa da \
                        mudança para o horário de Verão, o sistema di-lo e pede outra em vez \
                        de escolher por si.")}
                    {p("Uma actividade pode ser pessoal, de uma unidade, de um Research \
                        Workspace ou da instituição. A agenda pessoal é sua e de mais \
                        ninguém — nem a administração a vê.")}
                    {p("Cancelar uma actividade não a apaga: ela fica visível como \
                        cancelada, porque quem a esperava precisa de saber que não vai \
                        acontecer.")}
                    {p("Os prazos das suas tarefas aparecem no Calendário sem deixarem de \
                        ser tarefas. Mudar o prazo na tarefa muda o que o Calendário mostra; \
                        não há duas datas para manter.")}
                    {p("Um lembrete não é uma actividade: é um pedido para ser avisado. O \
                        Ocinye OS entrega-o mesmo com o Workspace fechado, e o aviso aparece \
                        no sino. Uma notificação informa — quando a abre, o Ocinye Core \
                        volta a verificar se ainda pode ver aquilo.")}
                },
            )}

            {seccao(
                "correio",
                "CORREIO",
                view! {
                    {p("O Ocinye Mail é o correio institucional, dentro do Workspace. \
                        Ler e enviar são serviços distintos e podem falhar em separado.")}
                    {p("Uma caixa vazia e um serviço não configurado são coisas diferentes, e \
                        o ecrã distingue-as. Se o correio ainda não foi configurado nesta \
                        instalação, a página di-lo — não mostra uma caixa vazia como se \
                        ninguém lhe tivesse escrito. Configurar é tarefa de quem administra.")}
                    <p class="oc-t-prose">"Ir para "<a href="/mail">"Correio"</a></p>
                },
            )}

            {seccao(
                "inteligencia",
                "OCINYE AI, AGENTES E COMPUTAÇÃO",
                view! {
                    {p("O Ocinye OS é operado com IA e governado pelo Core: um agente propõe \
                        e orquestra, e o Core autoriza e executa. Um agente nunca alcança \
                        mais do que a pessoa que o usa.")}
                    {p("Existir e estar disponível são coisas diferentes. As capacidades \
                        estão implementadas; a inferência precisa de um nó de IA da Ocinye \
                        registado. Enquanto não houver nenhum, a plataforma declara a IA \
                        indisponível — e não recorre a um fornecedor externo em silêncio.")}
                    {p("Zero nós de computação e zero agentes são estados válidos, não erros. \
                        Todo o restante Workspace funciona sem IA nenhuma.")}
                    <p class="oc-t-prose">
                        "Ir para "<a href="/ai">"Ocinye AI"</a>" · "
                        <a href="/ai/agents">"Agentes"</a>" · "
                        <a href="/compute">"Computação"</a>
                    </p>
                },
            )}

            {seccao(
                "institucional",
                "INSTITUCIONAL",
                view! {
                    {p("Actividade e Audit Log parecem-se e servem para coisas diferentes. \
                        A Actividade conta o que aconteceu no trabalho — quem actualizou uma \
                        ideia, quem juntou uma nota. O Audit Log é o registo técnico e \
                        imutável das operações, para controlo institucional.")}
                    {p("A Administração gere pessoas, papéis e filiações. Quem administra a \
                        plataforma não ganha, por isso, acesso ao conteúdo científico: ler \
                        investigação vem da filiação, não do papel.")}
                    <p class="oc-t-prose">
                        "Ir para "<a href="/activity">"Actividade"</a>" · "
                        <a href="/admin">"Administração"</a>" · "
                        <a href="/audit">"Audit Log"</a>
                    </p>
                },
            )}

            {seccao(
                "conta",
                "CONTA E SEGURANÇA",
                view! {
                    {p("Em Definições encontra a sua conta e as suas credenciais. Pode mudar \
                        a palavra-passe e ver as sessões abertas em seu nome, terminando \
                        qualquer uma delas.")}
                    {p("Mudar a palavra-passe exige a actual — uma sessão aberta não é prova \
                        suficiente de quem está a escrever. Ao mudá-la, todas as suas sessões \
                        terminam e esta é substituída por uma nova, sem ter de voltar a entrar.")}
                    {p("Papéis, filiações e acessos não se alteram aqui. São concedidos por \
                        quem tem autoridade para isso, e ficam registados com autor — é o que \
                        torna o acesso auditável em vez de acidental.")}
                    <p class="oc-t-prose">"Ir para "<a href="/settings">"Definições"</a></p>
                },
            )}

            {seccao(
                "estados",
                "ESTADOS DO SISTEMA",
                view! {
                    {p("O Workspace distingue cinco situações que se parecem no ecrã e \
                        significam coisas diferentes. Saber qual está a ver poupa-lhe tempo.")}
                    {estado(
                        "SEM DADOS",
                        "A funcionalidade existe e a consulta devolveu zero resultados.",
                    )}
                    {estado(
                        "SEM PERMISSÃO",
                        "O recurso existe, mas o seu acesso não permite utilizá-lo.",
                    )}
                    {estado(
                        "NÃO CONFIGURADO",
                        "A capacidade existe no Ocinye OS, mas esta instalação ainda não tem \
                         o serviço necessário configurado.",
                    )}
                    {estado(
                        "NÃO IMPLEMENTADO",
                        "A capacidade ainda não existe no produto actual.",
                    )}
                    {estado(
                        "INDISPONÍVEL",
                        "A operação existe, mas não pode ser executada no estado actual.",
                    )}
                    <p class="oc-t-prose oc-mt-5">
                        "Um controlo esbatido nunca é um erro da sua parte. Passe o rato por \
                         cima e ele diz qual destes estados o explica."
                    </p>
                },
            )}
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Todos os destinos da Ajuda são rotas reais do Workspace.
    ///
    /// Uma página de ajuda envelhece de uma maneira particular: o texto
    /// continua a parecer correcto muito depois de o link ter deixado de
    /// resolver. Este teste compara os destinos internos contra os caminhos que
    /// o servidor conhece, para que uma rota removida quebre aqui em vez de
    /// quebrar para quem procurava ajuda.
    #[test]
    fn todos_os_destinos_da_ajuda_sao_rotas_conhecidas() {
        let html = help().to_html();
        let mut destinos: Vec<String> = Vec::new();

        for pedaco in html.split("href=\"").skip(1) {
            let alvo = pedaco.split('"').next().unwrap_or_default();
            // Âncoras internas resolvem na própria página.
            if alvo.starts_with('#') || alvo.is_empty() {
                continue;
            }
            destinos.push(alvo.to_owned());
        }

        assert!(
            !destinos.is_empty(),
            "a ajuda deixou de ligar a lado nenhum; este teste ficou sem objecto"
        );

        let conhecidas = crate::routes::ROUTES;
        for destino in &destinos {
            assert!(
                destino.starts_with('/'),
                "a ajuda aponta para fora do Workspace: {destino}"
            );
            assert!(
                conhecidas.contains(&destino.as_str()),
                "a ajuda aponta para {destino}, que não é uma rota do Workspace"
            );
        }
    }

    /// A ajuda explica os cinco estados que o Workspace distingue.
    ///
    /// É a parte que mais serve o membro: sem ela, um controlo esbatido parece
    /// avaria, e uma tabela vazia parece perda de dados.
    #[test]
    fn a_ajuda_explica_os_estados_do_sistema() {
        let html = help().to_html();
        for estado in [
            "SEM DADOS",
            "SEM PERMISSÃO",
            "NÃO CONFIGURADO",
            "NÃO IMPLEMENTADO",
            "INDISPONÍVEL",
        ] {
            assert!(
                html.contains(estado),
                "a ajuda deixou de explicar o estado «{estado}»"
            );
        }
    }
}
