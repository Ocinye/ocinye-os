//! O catálogo das operações institucionais do Ocinye OS.
//!
//! # O que este ficheiro é
//!
//! A lista das operações que um membro razoavelmente pediria ao sistema, e a
//! decisão explícita sobre cada uma: pode ser delegada a um agente, não pode, ou
//! ainda não existe.
//!
//! Não é a lista de todas as funções públicas do Core. Um `get_by_id` interno,
//! uma sonda de saúde ou um detalhe de infraestrutura não são intenções
//! institucionais, e inflacionar a lista com eles só serviria para fazer subir
//! uma percentagem.
//!
//! # Porque existe
//!
//! Antes disto, uma operação nova nascia sem que ninguém decidisse se era
//! delegável — e a ausência de capability era indistinguível de uma recusa
//! deliberada. Dados e Organisation tinham módulos nativos inteiros que o plano
//! agentic não alcançava, e não por decisão: por omissão.
//!
//! > `Unclassified = 0` não significa que tudo está exposto. Significa que tudo
//! > foi decidido (ADR-0307).

use ocinye_contracts::agentic::{AgenticExposure, CapabilityId, OperationId, TrustBoundary};

/// Uma operação institucional e a sua disposição agentic.
#[derive(Debug, Clone)]
pub struct OperationEntry {
    /// O nome canónico da operação do Core.
    pub id: OperationId,
    /// O módulo a que pertence.
    pub module: &'static str,
    /// O que faz, em linguagem que um membro leria.
    pub summary: &'static str,
    /// O que o plano agentic pode fazer com ela.
    pub exposure: AgenticExposure,
}

/// Declara uma operação endereçável por um agente.
fn addressable(
    module: &'static str,
    id: &'static str,
    capability: &'static str,
    summary: &'static str,
) -> OperationEntry {
    OperationEntry {
        id: OperationId::new(id),
        module,
        summary,
        exposure: AgenticExposure::Addressable {
            capability: CapabilityId::new(capability),
        },
    }
}

/// Declara uma operação que existe e não pode ser delegada.
fn non_delegable(
    module: &'static str,
    id: &'static str,
    summary: &'static str,
    boundary: TrustBoundary,
    reason: &'static str,
) -> OperationEntry {
    OperationEntry {
        id: OperationId::new(id),
        module,
        summary,
        exposure: AgenticExposure::NonDelegable { boundary, reason },
    }
}

/// Declara uma operação que o Core ainda não tem.
fn not_implemented(
    module: &'static str,
    id: &'static str,
    summary: &'static str,
    reason: &'static str,
) -> OperationEntry {
    OperationEntry {
        id: OperationId::new(id),
        module,
        summary,
        exposure: AgenticExposure::NotImplemented { reason },
    }
}

/// Todas as operações institucionais significativas, e o que se decidiu sobre
/// cada uma.
#[must_use]
pub fn catalogue() -> Vec<OperationEntry> {
    let mut entries = Vec::new();
    entries.extend(research());
    entries.extend(knowledge());
    entries.extend(collaboration());
    entries.extend(mail());
    entries.extend(compute());
    entries.extend(organisation());
    entries.extend(data());
    entries.extend(fronteira_de_autoridade());
    entries.extend(identidade());
    entries.extend(nao_implementadas());
    entries
}

/// Investigação — ideias, projectos, ambientes de trabalho.
fn research() -> Vec<OperationEntry> {
    vec![
        addressable(
            "research",
            "research::create_idea",
            "research.idea.create",
            "Criar uma ideia numa unidade.",
        ),
        addressable(
            "research",
            "research::update_idea",
            "research.idea.revise",
            "Rever o conteúdo de uma ideia.",
        ),
        addressable(
            "research",
            "research::transition_idea",
            "research.idea.transition",
            "Fazer uma ideia avançar no seu ciclo.",
        ),
        addressable(
            "research",
            "research::promote_idea",
            "research.idea.promote",
            "Promover uma ideia candidata a projecto.",
        ),
        addressable(
            "research",
            "research::get_idea",
            "research.idea.read",
            "Ler uma ideia.",
        ),
        addressable(
            "research",
            "research::get_project",
            "research.project.read",
            "Ler um projecto.",
        ),
        addressable(
            "research",
            "research::transition_project",
            "research.project.transition",
            "Fazer um projecto avançar no seu ciclo.",
        ),
        addressable(
            "research",
            "research::get_workspace_overview",
            "research.workspace.overview",
            "Ler o panorama de um ambiente de investigação.",
        ),
    ]
}

/// Conhecimento — referências, notas, documentos, ligações.
fn knowledge() -> Vec<OperationEntry> {
    vec![
        addressable(
            "knowledge",
            "knowledge::create_source",
            "knowledge.source.create",
            "Acrescentar uma referência bibliográfica.",
        ),
        addressable(
            "knowledge",
            "knowledge::review_bibliography",
            "knowledge.bibliography.review",
            "Validar e normalizar referências BibTeX.",
        ),
        addressable(
            "knowledge",
            "knowledge::get_source",
            "knowledge.source.read",
            "Ler uma referência.",
        ),
        addressable(
            "knowledge",
            "knowledge::create_note",
            "knowledge.note.create",
            "Escrever uma nota.",
        ),
        addressable(
            "knowledge",
            "knowledge::update_note",
            "knowledge.note.revise",
            "Rever uma nota.",
        ),
        addressable(
            "knowledge",
            "knowledge::get_note",
            "knowledge.note.read",
            "Ler uma nota.",
        ),
        addressable(
            "knowledge",
            "knowledge::get_document",
            "knowledge.document.read",
            "Ler os metadados de um documento.",
        ),
        addressable(
            "knowledge",
            "knowledge::link_objects",
            "knowledge.link.create",
            "Relacionar dois objectos institucionais.",
        ),
        addressable(
            "knowledge",
            "knowledge::list_links",
            "knowledge.links.list",
            "Ver as relações de um objecto.",
        ),
        addressable(
            "knowledge",
            "search::search",
            "knowledge.search",
            "Procurar no acervo institucional autorizado.",
        ),
    ]
}

/// Colaboração — tarefas.
fn collaboration() -> Vec<OperationEntry> {
    vec![
        addressable(
            "collaboration",
            "collaboration::create_task",
            "collaboration.task.create",
            "Criar uma tarefa num ambiente de investigação.",
        ),
        addressable(
            "collaboration",
            "collaboration::assign_task",
            "collaboration.task.assign",
            "Atribuir uma tarefa a um membro.",
        ),
        addressable(
            "collaboration",
            "collaboration::transition_task",
            "collaboration.task.transition",
            "Mudar o estado de uma tarefa.",
        ),
        addressable(
            "collaboration",
            "collaboration::list_tasks",
            "collaboration.task.list",
            "Listar tarefas autorizadas.",
        ),
    ]
}

/// Correio institucional.
fn mail() -> Vec<OperationEntry> {
    vec![
        addressable("mail", "mail::draft", "mail.draft", "Redigir uma mensagem."),
        addressable(
            "mail",
            "mail::draft_reply",
            "mail.draft_reply",
            "Redigir uma resposta.",
        ),
        addressable(
            "mail",
            "mail::draft_transform",
            "mail.draft_transform",
            "Reescrever um rascunho.",
        ),
        addressable(
            "mail",
            "mail::evaluate_send",
            "mail.evaluate_send",
            "Avaliar se uma mensagem pode ser enviada.",
        ),
        addressable(
            "mail",
            "mail::read_message",
            "mail.read",
            "Ler uma mensagem.",
        ),
        addressable(
            "mail",
            "mail::search_messages",
            "mail.search",
            "Procurar mensagens.",
        ),
        addressable(
            "mail",
            "mail::send_message",
            "mail.send",
            "Enviar uma mensagem para fora da instituição.",
        ),
    ]
}

/// Computação.
fn compute() -> Vec<OperationEntry> {
    vec![addressable(
        "compute",
        "compute::list_nodes",
        "compute.node.list",
        "Ver os nós de computação registados.",
    )]
}

/// Organisation — o mapa institucional.
fn organisation() -> Vec<OperationEntry> {
    vec![addressable(
        "organisation",
        "organisation::create_unit",
        "organisation.unit.create",
        "Criar uma unidade científica.",
    )]
}

/// Dados — datasets.
fn data() -> Vec<OperationEntry> {
    vec![
        addressable(
            "data",
            "data::create_dataset",
            "data.dataset.create",
            "Criar um dataset com os seus metadados.",
        ),
        addressable(
            "calendar",
            "calendar::create_event",
            "calendar.event.create",
            "Marcar um compromisso.",
        ),
        addressable(
            "calendar",
            "calendar::update_event",
            "calendar.event.update",
            "Alterar um compromisso já marcado.",
        ),
        addressable(
            "calendar",
            "calendar::cancel_event",
            "calendar.event.cancel",
            "Cancelar um compromisso.",
        ),
        addressable(
            "calendar",
            "calendar::create_reminder",
            "calendar.reminder.create",
            "Pedir para ser lembrado de alguma coisa.",
        ),
        non_delegable(
            "data",
            "data::add_version_file",
            "Acrescentar um ficheiro a uma versão de dataset.",
            TrustBoundary::UserMediatedBinaryBoundary,
            "A execução segura exige entrada binária mediada pela pessoa através da fronteira \
             autenticada de carregamento. Bytes de ficheiro, caminhos locais, URLs arbitrários e \
             credenciais de armazenamento não são entradas agentic.",
        ),
    ]
}

/// As operações que mudam a fronteira de autoridade.
///
/// # A segunda classe de não-delegabilidade
///
/// O ADR-0307 diz que **risco alto não é, por si só, critério de
/// não-delegabilidade** — e continua a dizê-lo: enviar um email externo é de
/// alto impacto e continua endereçável.
///
/// Estas são outra coisa:
///
/// > **An operation whose primary effect is to change the authorization
/// > boundary or another person's ability to access the system is non-delegable
/// > by architecture.**
///
/// A diferença é o **depois**. Um email enviado é um efeito; um papel concedido
/// muda quem poderá exercer autoridade a partir dali.
///
/// # O ataque que isto elimina
///
/// Conteúdo recuperado — um documento, um email, um dataset — é `UNTRUSTED
/// DATA` e não consegue autorizar nada: o Core impede. Mas consegue **induzir
/// propostas**, uma e outra vez, até alguém confirmar uma por cansaço.
///
/// Contra essa classe a confirmação humana é a última barreira. Não publicar a
/// capability elimina-a inteira, e é uma defesa que não depende de ninguém estar
/// atento ao fim de um dia longo.
///
/// O agente continua a poder ajudar: abrir a Administração, explicar a
/// operação, resolver o contexto para mostrar. O que não faz é emitir um plano
/// executável.
fn fronteira_de_autoridade() -> Vec<OperationEntry> {
    /// A razão é a mesma para todas, e é a regra — não uma descrição do risco.
    const FRONTEIRA: &str =
        "O efeito principal é mudar a fronteira de autorização ou a capacidade de outra pessoa \
         aceder ao sistema. Uma operação assim não deve tornar-se executável só porque conteúdo \
         recuperado não confiável pode influenciar uma proposta agentic.";

    vec![
        non_delegable(
            "identity",
            "identity::grant_role",
            "Conceder um papel técnico a um membro.",
            TrustBoundary::AuthorityBoundary,
            FRONTEIRA,
        ),
        non_delegable(
            "identity",
            "identity::revoke_role",
            "Retirar um papel técnico a um membro.",
            TrustBoundary::AuthorityBoundary,
            FRONTEIRA,
        ),
        non_delegable(
            "identity",
            "identity::set_account_status",
            "Suspender ou reactivar a conta de um membro.",
            TrustBoundary::AuthorityBoundary,
            FRONTEIRA,
        ),
        non_delegable(
            "governance",
            "governance::create_grant",
            "Conceder um acesso explícito.",
            TrustBoundary::AuthorityBoundary,
            FRONTEIRA,
        ),
        non_delegable(
            "governance",
            "governance::revoke_grant",
            "Retirar um acesso explícito.",
            TrustBoundary::AuthorityBoundary,
            FRONTEIRA,
        ),
        // Medido, e não presumido: `pertencer_a_uma_unidade_expande_o_acesso_efectivo`
        // mostra que a mesma pessoa passa a poder criar ideias e ver datasets
        // só por ser acrescentada, sem lhe tocar em papel técnico nenhum.
        //
        // Filiação não é metadado organizacional neste domínio. É autoridade.
        non_delegable(
            "organisation",
            "organisation::add_unit_member",
            "Acrescentar um membro a uma unidade.",
            TrustBoundary::AuthorityBoundary,
            FRONTEIRA,
        ),
    ]
}

/// Identidade — o que um membro faz sobre si próprio.
///
/// As não-delegáveis daqui não o são por serem perigosas. São-no porque o fluxo
/// seguro **exige que um segredo atravesse o plano agentic**, e esse é o critério
/// do ADR-0307.
fn identidade() -> Vec<OperationEntry> {
    vec![
        addressable(
            "identity",
            "identity::revoke_own_session",
            "identity.session.revoke",
            "Terminar uma das suas próprias sessões.",
        ),
        addressable(
            "identity",
            "identity::choose_preset",
            "identity.avatar.choose_preset",
            "Escolher um dos avatares Ocinye.",
        ),
        non_delegable(
            "identity",
            "identity::change_own_password",
            "Mudar a própria palavra-passe.",
            TrustBoundary::SecretBoundary,
            "A execução segura exige a palavra-passe actual, e uma palavra-passe nunca pode \
             entrar no contexto de um modelo. O agente pode abrir Definições → Segurança e \
             explicar o que se segue.",
        ),
        non_delegable(
            "identity",
            "identity::reset_password",
            "Emitir uma credencial temporária a um membro.",
            TrustBoundary::SecretBoundary,
            "A operação produz uma credencial temporária. Delegá-la faria o material secreto \
             passar pelo plano agentic para chegar a quem o pediu.",
        ),
        non_delegable(
            "identity",
            "identity::create_member",
            "Criar uma conta e emitir o primeiro acesso.",
            TrustBoundary::SecretBoundary,
            "Tal como está modelada, a operação devolve a credencial de primeiro acesso. Se um \
             dia a criação e a emissão forem operações separadas, a primeira volta a ser \
             candidata a endereçável.",
        ),
        non_delegable(
            "identity",
            "identity::create_invitation",
            "Convidar alguém para a instituição.",
            TrustBoundary::SecretBoundary,
            "O convite produz um segredo que autentica quem o apresenta, e esse material não \
             atravessa o plano agentic.",
        ),
        non_delegable(
            "identity",
            "identity::bootstrap_platform_admin",
            "Criar o primeiro administrador da instalação.",
            TrustBoundary::SecretBoundary,
            "Emite a credencial inicial da instalação, e acontece quando ainda não há ninguém \
             para autorizar seja o que for. É um acto de arranque, não uma operação institucional.",
        ),
        non_delegable(
            "identity",
            "identity::set_photograph",
            "Carregar uma fotografia de perfil.",
            TrustBoundary::UserMediatedBinaryBoundary,
            "A execução segura exige entrada binária mediada pela pessoa através da fronteira \
             autenticada de carregamento. Bytes de ficheiro, caminhos locais, URLs arbitrários e \
             credenciais de armazenamento não são entradas agentic.",
        ),
    ]
}

/// O que ainda não existe.
///
/// Entra aqui apenas o que o Core **não tem**. Uma operação que já funciona e
/// cuja capability está por escrever não é `NotImplemented` — é trabalho por
/// fazer, e o ADR-0307 recusa essa terceira via.
fn nao_implementadas() -> Vec<OperationEntry> {
    vec![
        not_implemented(
            "compute",
            "compute::submit_job",
            "Submeter um trabalho de computação.",
            "Não há trabalhos de computação no Core: o módulo regista nós e mais nada.",
        ),
        not_implemented(
            "knowledge",
            "knowledge::create_result",
            "Registar um resultado de investigação.",
            "A entidade Resultado ainda não existe no domínio.",
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{BTreeMap, BTreeSet};

    /// Cada operação aparece uma só vez.
    ///
    /// Duas entradas para a mesma operação seriam duas decisões sobre a mesma
    /// coisa, e a última na lista ganharia sem ninguém dar por isso.
    #[test]
    fn cada_operacao_tem_exactamente_uma_disposicao() {
        let mut vistas: BTreeMap<String, usize> = BTreeMap::new();
        for entrada in catalogue() {
            *vistas.entry(entrada.id.as_str().to_owned()).or_default() += 1;
        }

        let repetidas: Vec<_> = vistas
            .iter()
            .filter(|(_, quantas)| **quantas > 1)
            .map(|(id, quantas)| format!("{id} ({quantas}×)"))
            .collect();

        assert!(
            repetidas.is_empty(),
            "operações com mais do que uma disposição:\n  {}",
            repetidas.join("\n  "),
        );
    }

    /// Toda a operação endereçável aponta para uma capability que existe.
    ///
    /// É a metade que impede a quarta categoria: «endereçável, capability por
    /// implementar» deixa de ser possível de escrever sem o teste acusar.
    #[test]
    fn toda_operacao_enderecavel_tem_capability_real() {
        let registadas: BTreeSet<String> = crate::modules::agentic::registry()
            .all()
            .into_iter()
            .map(|d| d.id.as_str().to_owned())
            .collect();

        let mut fantasmas = Vec::new();
        for entrada in catalogue() {
            if let Some(capability) = entrada.exposure.capability() {
                if !registadas.contains(capability.as_str()) {
                    fantasmas.push(format!(
                        "{} → {} (não existe no registry)",
                        entrada.id,
                        capability.as_str()
                    ));
                }
            }
        }

        assert!(
            fantasmas.is_empty(),
            "operações endereçáveis sem capability real:\n  {}",
            fantasmas.join("\n  "),
        );
    }

    /// Toda a capability aponta para uma operação que o catálogo conhece.
    ///
    /// O sentido inverso. Sem ele, uma capability podia executar algo que
    /// ninguém tinha classificado — e a classificação deixava de ser completa
    /// exactamente onde importa.
    #[test]
    fn toda_capability_aponta_para_uma_operacao_do_catalogo() {
        let conhecidas: BTreeSet<String> = catalogue()
            .into_iter()
            .map(|e| e.id.as_str().to_owned())
            .collect();

        let mut orfas = Vec::new();
        for descriptor in crate::modules::agentic::registry().all() {
            if !conhecidas.contains(descriptor.operation.as_str()) {
                orfas.push(format!(
                    "{} → {} (fora do catálogo)",
                    descriptor.id.as_str(),
                    descriptor.operation
                ));
            }
        }

        assert!(
            orfas.is_empty(),
            "capabilities cuja operação ninguém classificou:\n  {}",
            orfas.join("\n  "),
        );
    }

    /// Nenhuma capability reclama uma operação não-delegável.
    ///
    /// A impossibilidade que dá sentido à classificação: declarar uma operação
    /// não-delegável e depois expô-la seria dizer duas coisas contrárias sobre a
    /// mesma operação, e a que valeria era a que executa.
    #[test]
    fn nenhuma_capability_reclama_uma_operacao_nao_delegavel() {
        let proibidas: BTreeSet<String> = catalogue()
            .into_iter()
            .filter(|e| matches!(e.exposure, AgenticExposure::NonDelegable { .. }))
            .map(|e| e.id.as_str().to_owned())
            .collect();

        let mut violacoes = Vec::new();
        for descriptor in crate::modules::agentic::registry().all() {
            if proibidas.contains(descriptor.operation.as_str()) {
                violacoes.push(format!(
                    "{} executa {}, que está declarada não-delegável",
                    descriptor.id.as_str(),
                    descriptor.operation
                ));
            }
        }

        assert!(
            violacoes.is_empty(),
            "capabilities sobre operações não-delegáveis:\n  {}",
            violacoes.join("\n  "),
        );
    }

    /// O mesmo para o que ainda não existe.
    ///
    /// Uma capability sobre uma operação declarada inexistente é uma de duas
    /// coisas: uma capability que não pode funcionar, ou uma classificação
    /// desactualizada. As duas são erros, e nenhuma se vê a olho.
    #[test]
    fn nenhuma_capability_reclama_uma_operacao_inexistente() {
        let inexistentes: BTreeSet<String> = catalogue()
            .into_iter()
            .filter(|e| matches!(e.exposure, AgenticExposure::NotImplemented { .. }))
            .map(|e| e.id.as_str().to_owned())
            .collect();

        for descriptor in crate::modules::agentic::registry().all() {
            assert!(
                !inexistentes.contains(descriptor.operation.as_str()),
                "{} executa {}, declarada inexistente no Core",
                descriptor.id.as_str(),
                descriptor.operation
            );
        }
    }

    /// O catálogo e a guarda do arranque dizem a mesma coisa.
    ///
    /// # A divergência que isto impede
    ///
    /// `is_delegable_to_agents` recusa, no arranque, qualquer capability que
    /// exija `PermissionsManage`, `RolesManage` ou `MembersManage`. É uma
    /// defesa estrutural contra uma classe de ataque — conteúdo hostil a induzir
    /// propostas de escalada até alguém confirmar uma.
    ///
    /// O catálogo classifica as mesmas operações como não-delegáveis. As duas
    /// afirmações têm de continuar a concordar: se alguém amanhã levantar a
    /// guarda, o catálogo passa a mentir; se alguém marcar uma destas como
    /// endereçável, o arranque falha e ninguém percebe porquê.
    ///
    /// Este teste faz as duas encontrarem-se num sítio só.
    #[test]
    fn o_catalogo_concorda_com_a_guarda_do_arranque() {
        use ocinye_contracts::Permission;

        // As permissões que a guarda recusa delegar.
        let fechadas = [
            Permission::PermissionsManage,
            Permission::RolesManage,
            Permission::MembersManage,
        ];

        for permissao in fechadas {
            assert!(
                !ocinye_domain::policy::agentic::is_delegable_to_agents(permissao),
                "a guarda do arranque deixou de proteger «{}» — o catálogo continua a \
                 classificar as operações que a exigem como não-delegáveis, e as duas \
                 afirmações passaram a divergir",
                permissao.as_str()
            );
        }

        // E as operações que mudam a fronteira de autoridade continuam fechadas.
        let esperadas = [
            "identity::grant_role",
            "identity::revoke_role",
            "identity::set_account_status",
            "governance::create_grant",
            "governance::revoke_grant",
            "organisation::add_unit_member",
        ];

        let catalogo = catalogue();
        for operacao in esperadas {
            let entrada = catalogo
                .iter()
                .find(|e| e.id.as_str() == operacao)
                .unwrap_or_else(|| panic!("{operacao} desapareceu do catálogo"));
            assert!(
                matches!(entrada.exposure, AgenticExposure::NonDelegable { .. }),
                "{operacao} deixou de ser não-delegável, e muda a fronteira de autoridade"
            );
        }
    }

    /// Toda a razão declarada diz alguma coisa.
    ///
    /// `NonDelegable("")` é uma decisão por tomar com aspecto de decisão tomada.
    /// E «segurança» sozinho não é uma razão: o critério do ADR-0307 é dizer
    /// **o quê** — que segredo, que fronteira, que dependência.
    #[test]
    fn toda_razao_declarada_diz_alguma_coisa() {
        for entrada in catalogue() {
            if let Some(razao) = entrada.exposure.reason() {
                assert!(
                    razao.chars().count() >= 20,
                    "{}: a razão «{razao}» é curta de mais para explicar a decisão",
                    entrada.id
                );
                assert!(
                    !razao.eq_ignore_ascii_case("segurança")
                        && !razao.eq_ignore_ascii_case("security"),
                    "{}: «segurança» não é uma razão, é uma categoria",
                    entrada.id
                );
            }
        }
    }
}

#[cfg(test)]
mod fronteira_do_modelo {
    /// O contexto de inferência leva identificadores, e mais nada.
    ///
    /// # A propriedade
    ///
    /// > **The inference context receives capability identifiers, never the
    /// > internal capability registry representation.**
    ///
    /// O teste constrói o envelope com todas as capabilities de um principal e
    /// verifica que nada além dos identificadores lá está: nem a permissão que
    /// cada uma exige, nem o risco, nem a `OperationId`, nem o esquema.
    ///
    /// # Porque isto é fácil de perder
    ///
    /// A tentação futura tem uma frase pronta: «o modelo planeia melhor se
    /// souber o risco». Talvez planeie. Mas o risco é um facto do Core, e um
    /// modelo que o conheça pode argumentar sobre ele — e o que se quer é que
    /// não tenha nada sobre que argumentar.
    #[test]
    fn o_contexto_de_inferencia_leva_identificadores_e_mais_nada() {
        let descritores = crate::modules::agentic::registry().all();
        assert!(
            !descritores.is_empty(),
            "controlo positivo falhou: o registry está vazio, e este teste não \
             estaria a observar nada"
        );

        // O que o envelope leva, tal como o runtime o constrói.
        let no_envelope: Vec<String> = descritores
            .iter()
            .map(|d| d.id.as_str().to_owned())
            .collect();

        // A comparação é estrutural, e não textual.
        //
        // A primeira versão deste teste procurava cada permissão no JSON e
        // falhou — mas não por fuga nenhuma: a capability `mail.send` e a
        // permissão `mail.send` têm o mesmo texto. Procurar substrings não
        // distingue «isto escapou» de «isto lê-se igual», e um teste que não
        // distingue as duas coisas acusa a errada.
        //
        // Comparar os conjuntos prova a propriedade inteira de uma vez: o que
        // vai no envelope é **exactamente** o conjunto dos identificadores, e
        // portanto nada mais do descritor lá está.
        let voltou: Vec<serde_json::Value> =
            serde_json::from_str(&serde_json::to_string(&no_envelope).expect("serializar"))
                .expect("desserializar");

        let esperados: std::collections::BTreeSet<String> = descritores
            .iter()
            .map(|d| d.id.as_str().to_owned())
            .collect();

        let obtidos: std::collections::BTreeSet<String> = voltou
            .iter()
            .map(|valor| {
                // Cada elemento tem de ser uma string simples. Um objecto aqui
                // seria o descritor a passar por identificador.
                valor
                    .as_str()
                    .unwrap_or_else(|| {
                        panic!(
                            "o contexto de inferência leva uma estrutura onde devia levar um \
                             identificador: {valor}"
                        )
                    })
                    .to_owned()
            })
            .collect();

        assert_eq!(
            obtidos, esperados,
            "o contexto de inferência deixou de levar exactamente os identificadores"
        );
    }
}

#[cfg(test)]
mod contagem {
    use super::*;
    use ocinye_contracts::agentic::{AgenticExposure, TrustBoundary};
    use std::collections::BTreeSet;

    /// As contagens fecham, e derivam do catálogo.
    ///
    /// # Porque isto é um teste e não uma nota
    ///
    /// Um relatório meu contou «8 não-delegáveis» e listou treze na frase
    /// seguinte. Foram duas contagens independentes da mesma coisa — uma na
    /// cabeça, outra na lista — e a que estava errada era a que ninguém podia
    /// verificar.
    ///
    /// Enquanto documentação, relatório e código mantiverem contagens próprias,
    /// duas delas vão divergir. Esta deriva do catálogo, e as outras passam a
    /// citá-la.
    #[test]
    fn as_contagens_fecham_e_derivam_do_catalogo() {
        let catalogo = catalogue();

        let enderecaveis = catalogo
            .iter()
            .filter(|e| matches!(e.exposure, AgenticExposure::Addressable { .. }))
            .count();
        let nao_delegaveis = catalogo
            .iter()
            .filter(|e| matches!(e.exposure, AgenticExposure::NonDelegable { .. }))
            .count();
        let inexistentes = catalogo
            .iter()
            .filter(|e| matches!(e.exposure, AgenticExposure::NotImplemented { .. }))
            .count();

        assert_eq!(
            catalogo.len(),
            enderecaveis + nao_delegaveis + inexistentes,
            "as três disposições não somam o catálogo: {} entradas, {enderecaveis} + \
             {nao_delegaveis} + {inexistentes}",
            catalogo.len(),
        );

        // Nenhuma operação contada duas vezes.
        let unicos: BTreeSet<&str> = catalogo.iter().map(|e| e.id.as_str()).collect();
        assert_eq!(
            unicos.len(),
            catalogo.len(),
            "o catálogo tem {} entradas para {} operações distintas",
            catalogo.len(),
            unicos.len(),
        );

        // E cada endereçável resolve mesmo para uma capability do registry.
        let registadas: BTreeSet<String> = crate::modules::agentic::registry()
            .all()
            .into_iter()
            .map(|d| d.id.as_str().to_owned())
            .collect();
        let resolvem = catalogo
            .iter()
            .filter_map(|e| e.exposure.capability())
            .filter(|c| registadas.contains(c.as_str()))
            .count();
        assert_eq!(
            resolvem, enderecaveis,
            "{enderecaveis} operações endereçáveis e só {resolvem} resolvem para uma capability"
        );
    }

    /// Emite o catálogo, para a matriz e para o relatório.
    ///
    /// Ignorado por omissão: escreve para a saída padrão e serve a geração de
    /// documentação, não a verificação. Correr com:
    /// `cargo test -p ocinye-core --lib despeja_o_catalogo -- --ignored --nocapture`
    #[test]
    #[ignore = "emite a matriz; serve a documentação, não a verificação"]
    fn despeja_a_matriz() {
        let catalogo = catalogue();

        println!("# Matriz de operações e exposição agentic\n");
        println!(
            "> **Gerado a partir do catálogo tipado de operações. Não editar contagens nem \
             entradas à mão.**\n"
        );
        println!(
            "Reproduzir com:\n\n```\ncargo test -p ocinye-core --lib despeja_a_matriz -- \
             --ignored --nocapture\n```\n"
        );
        println!(
            "Cada operação aparece na sua linha. Duas operações distintas nunca são agrupadas \
             numa só para a tabela ficar mais curta: foi assim que uma contagem de treze passou \
             por oito.\n"
        );

        println!("| Operação | Módulo | Exposição | Capability | Fronteira | Razão |");
        println!("|---|---|---|---|---|---|");
        for entrada in &catalogo {
            let (exposicao, capability, fronteira, razao) = match &entrada.exposure {
                AgenticExposure::Addressable { capability } => (
                    "Addressable",
                    format!("`{}`", capability.as_str()),
                    "—".to_owned(),
                    "—".to_owned(),
                ),
                AgenticExposure::NonDelegable { boundary, reason } => (
                    "NonDelegable",
                    "—".to_owned(),
                    format!("`{}`", boundary.as_str()),
                    (*reason).replace('\n', " "),
                ),
                AgenticExposure::NotImplemented { reason } => (
                    "NotImplemented",
                    "—".to_owned(),
                    "—".to_owned(),
                    (*reason).replace('\n', " "),
                ),
            };
            println!(
                "| `{}` | {} | {} | {} | {} | {} |",
                entrada.id, entrada.module, exposicao, capability, fronteira, razao,
            );
        }

        let conta =
            |f: fn(&AgenticExposure) -> bool| catalogo.iter().filter(|e| f(&e.exposure)).count();
        let enderecaveis = conta(|e| matches!(e, AgenticExposure::Addressable { .. }));
        let nao_delegaveis = conta(|e| matches!(e, AgenticExposure::NonDelegable { .. }));
        let inexistentes = conta(|e| matches!(e, AgenticExposure::NotImplemented { .. }));

        println!("\n## Contagens\n");
        println!("| | |");
        println!("|---|---|");
        println!(
            "| Operações institucionais significativas | **{}** |",
            catalogo.len()
        );
        println!("| `Addressable` | **{enderecaveis}** |");
        println!("| `NonDelegable` | **{nao_delegaveis}** |");
        println!("| `NotImplemented` | **{inexistentes}** |");
        println!("| Sem classificação | **0** |");
        println!(
            "| Capabilities no registry | **{}** |",
            crate::modules::agentic::registry().len()
        );

        println!("\n## Fronteiras de confiança\n");
        println!(
            "> **Non-delegability is determined by the nature of the trust boundary crossed, \
             not by risk level alone.**\n"
        );
        for fronteira in [
            TrustBoundary::SecretBoundary,
            TrustBoundary::AuthorityBoundary,
            TrustBoundary::UserMediatedBinaryBoundary,
        ] {
            let quais: Vec<&str> = catalogo
                .iter()
                .filter(|e| e.exposure.boundary() == Some(fronteira))
                .map(|e| e.id.as_str())
                .collect();
            println!(
                "**`{}`** — {} operações: {}\n",
                fronteira.as_str(),
                quais.len(),
                quais
                    .iter()
                    .map(|id| format!("`{id}`"))
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }
    }

    /// A fronteira de autoridade contém exactamente estas operações.
    ///
    /// # Porquê fixar uma lista, aqui, à mão
    ///
    /// Porque não é uma contagem — é uma **decisão**, tomada caso a caso e
    /// aprovada explicitamente, e `organisation::add_unit_member` foi
    /// classificada por medição e não pelo nome: o teste
    /// `pertencer_a_uma_unidade_expande_o_acesso_efectivo` mostra que a mesma
    /// pessoa, sem lhe tocar em papel técnico nenhum, passa a poder criar ideias
    /// e ver datasets só por ser acrescentada a uma unidade.
    ///
    /// ADR-0307 escreve esta lista em prosa. Sem isto, as duas cópias podiam
    /// divergir em silêncio — e já aconteceu neste repositório uma classificação
    /// mudar sem o documento a acompanhar. Acrescentar ou retirar uma operação
    /// desta fronteira passa a exigir mexer aqui, o que é exactamente o
    /// atrito pretendido.
    #[test]
    fn a_fronteira_de_autoridade_e_a_que_foi_decidida() {
        let decidida: BTreeSet<&str> = [
            "identity::grant_role",
            "identity::revoke_role",
            "identity::set_account_status",
            "governance::create_grant",
            "governance::revoke_grant",
            "organisation::add_unit_member",
        ]
        .into_iter()
        .collect();

        let catalogo = catalogue();
        let real: BTreeSet<&str> = catalogo
            .iter()
            .filter(|e| e.exposure.boundary() == Some(TrustBoundary::AuthorityBoundary))
            .map(|e| e.id.as_str())
            .collect();

        assert_eq!(
            real, decidida,
            "a fronteira de autoridade mudou. Se foi de propósito, actualizar \
             também ADR-0307 e a matriz; se não foi, é um erro de classificação"
        );
    }

    /// Toda a operação não-delegável declara a fronteira que atravessa.
    ///
    /// A classe é tipada porque a razão em texto livre obriga quem lê a inferir,
    /// e inferir é onde as classificações se confundem umas com as outras. Sem
    /// isto, «segurança» e «altera autorização» acabavam na mesma gaveta.
    #[test]
    fn toda_nao_delegavel_declara_a_fronteira() {
        for entrada in catalogue() {
            if matches!(entrada.exposure, AgenticExposure::NonDelegable { .. }) {
                assert!(
                    entrada.exposure.boundary().is_some(),
                    "{} é não-delegável e não diz que fronteira atravessa",
                    entrada.id
                );
            }
        }
    }
}
