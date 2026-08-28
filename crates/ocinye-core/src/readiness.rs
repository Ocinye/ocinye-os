//! Se o Ocinye OS pode ser entregue, e o que dizer a quem ainda não entrou.
//!
//! # Porque isto vive no Core
//!
//! Porque a pergunta «isto está pronto?» tem de ter uma resposta só. Se o
//! Workspace contasse componentes verdes no browser para decidir, teríamos duas
//! políticas de arranque — e duas políticas acabam por discordar num sítio onde
//! ninguém está a olhar (ADR-0603).

use ocinye_contracts::readiness::{
    reasons, Criticality, PublicReadiness, ReadinessComponent, ReadinessComponentId,
    ReadinessOverall, CONTRACT_VERSION,
};
use ocinye_contracts::system_capability::SystemCapabilityState;
use ocinye_contracts::{SystemCapabilities, SystemCapability};
use sqlx::PgPool;

use crate::db;

/// Quanto tempo se espera por uma dependência antes de dizer que não respondeu.
///
/// Curto de propósito: isto corre no arranque, à frente de uma pessoa que está a
/// olhar para um ecrã. Uma dependência que precise de mais do que isto para
/// dizer «estou cá» está, para efeitos de arranque, em baixo.
const PROBE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(2);

/// A frase pública que corresponde a um estado.
///
/// # Porque isto é uma tradução e não um reencaminhamento
///
/// A razão canónica é segura para um **membro**. Pode, ainda assim, dizer a um
/// desconhecido quantos nós existem ou que adaptador está configurado. Aqui
/// escolhe-se de um conjunto fechado de treze frases, e é isso que torna
/// impossível alguém passar para o mundo o `Display` de um erro «só desta vez».
const fn public_reason(state: SystemCapabilityState) -> &'static str {
    match state {
        SystemCapabilityState::Available => reasons::AVAILABLE,
        SystemCapabilityState::NotConfigured => reasons::NOT_CONFIGURED,
        SystemCapabilityState::NoResource => reasons::NO_RESOURCE,
        SystemCapabilityState::Planned => reasons::NOT_IMPLEMENTED,
        SystemCapabilityState::Degraded | SystemCapabilityState::Unavailable => {
            reasons::UNAVAILABLE
        }
    }
}

/// O pior estado entre um conjunto de capacidades canónicas.
///
/// Um módulo público agrega várias capacidades — o correio são três. Se alguma
/// está pior, o módulo está pior: dizer «disponível» porque duas das três
/// respondem seria dizer que o correio funciona quando não se consegue enviar.
fn worst(capabilities: &SystemCapabilities, wanted: &[SystemCapability]) -> SystemCapabilityState {
    let ordem = |state: SystemCapabilityState| match state {
        SystemCapabilityState::Available => 0,
        SystemCapabilityState::Degraded => 1,
        SystemCapabilityState::Planned => 2,
        SystemCapabilityState::NoResource => 3,
        SystemCapabilityState::NotConfigured => 4,
        SystemCapabilityState::Unavailable => 5,
    };

    capabilities
        .capabilities
        .iter()
        .filter(|report| wanted.contains(&report.capability))
        .map(|report| report.state)
        .max_by_key(|state| ordem(*state))
        // Uma capacidade que o catálogo não conhece não é «disponível».
        .unwrap_or(SystemCapabilityState::Unavailable)
}

fn component(
    id: ReadinessComponentId,
    state: SystemCapabilityState,
    criticality: Criticality,
    reason: &str,
) -> ReadinessComponent {
    ReadinessComponent {
        component: id,
        state,
        criticality,
        reason: reason.to_owned(),
    }
}

/// Compõe a prontidão pública desta instalação.
///
/// # O que isto não faz
///
/// Não envia correio, não corre inferência, não submete trabalho, não escreve
/// ficheiros, não altera sessões. Prontidão lê estado registado e faz sondas que
/// não mudam nada — um arranque que provoque efeitos é um arranque que altera a
/// instituição sempre que alguém abre o separador.
///
/// # Erros
///
/// Nunca. Uma falha ao consultar traduz-se em `Blocked` com razão pública: não
/// conseguir determinar se o núcleo está pronto **não** autoriza assumir que
/// está.
pub async fn public_snapshot(
    pool: &PgPool,
    capabilities: Option<&SystemCapabilities>,
    realtime: Option<&crate::realtime::Realtime>,
) -> PublicReadiness {
    let mut components = Vec::with_capacity(ReadinessComponentId::all().len());

    // ── Persistência ────────────────────────────────────────────────────
    //
    // Com limite: uma base que aceita a ligação e emudece pendurava isto para
    // sempre, e o arranque com ela.
    let persistence = match tokio::time::timeout(PROBE_TIMEOUT, db::health(pool)).await {
        Ok(health) if health.reachable => SystemCapabilityState::Available,
        _ => SystemCapabilityState::Unavailable,
    };
    let persistente = persistence == SystemCapabilityState::Available;

    components.push(component(
        ReadinessComponentId::Core,
        SystemCapabilityState::Available,
        Criticality::Critical,
        reasons::CORE_UP,
    ));
    components.push(component(
        ReadinessComponentId::Persistence,
        persistence,
        Criticality::Critical,
        if persistente {
            reasons::PERSISTENCE_UP
        } else {
            reasons::PERSISTENCE_DOWN
        },
    ));

    // ── Identidade ──────────────────────────────────────────────────────
    //
    // **Derivada, hoje.** Autenticar exige ler pessoas e escrever sessões, e as
    // duas coisas exigem a persistência: neste desenho não existe uma
    // dependência de runtime independente que justifique outra sonda, e inventar
    // uma daria a impressão de uma garantia a mais.
    //
    // Isto não é uma lei — é uma leitura do sistema como ele está. No dia em que
    // a identidade tiver uma dependência própria (um fornecedor externo, um
    // serviço à parte), o conjunto fechado obriga a decidir isto outra vez neste
    // ficheiro, em vez de a derivação se manter por inércia.
    components.push(component(
        ReadinessComponentId::Identity,
        persistence,
        Criticality::Critical,
        if persistente {
            reasons::IDENTITY_UP
        } else {
            reasons::IDENTITY_DOWN
        },
    ));

    components.push(component(
        ReadinessComponentId::Compatibility,
        SystemCapabilityState::Available,
        Criticality::Critical,
        reasons::COMPATIBLE,
    ));

    // ── Módulos opcionais ───────────────────────────────────────────────
    //
    // Vêm do catálogo canónico, e não de sondas próprias: perguntar outra vez
    // seria uma segunda medição da mesma coisa, com hipótese de discordar.
    //
    // Sem catálogo — porque a persistência não responde — ficam indisponíveis,
    // e não «disponíveis por omissão».
    let opcional = |id, wanted: &[SystemCapability]| {
        let state = capabilities.map_or(SystemCapabilityState::Unavailable, |c| worst(c, wanted));
        component(id, state, Criticality::Optional, public_reason(state))
    };

    components.push(opcional(
        ReadinessComponentId::Storage,
        &[SystemCapability::ObjectStorage],
    ));
    components.push(opcional(
        ReadinessComponentId::Mail,
        &[
            SystemCapability::Mail,
            SystemCapability::MailSend,
            SystemCapability::MailSync,
        ],
    ));
    components.push(opcional(
        ReadinessComponentId::Intelligence,
        &[
            SystemCapability::AiGeneral,
            SystemCapability::AiCoding,
            SystemCapability::AiReasoning,
            SystemCapability::AiEmbedding,
        ],
    ));
    components.push(opcional(
        ReadinessComponentId::Compute,
        &[SystemCapability::Compute],
    ));

    // O Calendário está construído e disponível.
    //
    // Este componente esteve `Planned`, e o comentário que o acompanhava dizia
    // «tem domínio e não tem interface». Era verdade quando foi escrito. Deixou
    // de ser — o Calendário fechou com catorze operações, entrega de lembretes
    // pelo worker, quatro vistas no Workspace e paridade com a entrada agentic
    // (ADR-0410, `Accepted`).
    //
    // Uma afirmação obsoleta a decidir estado é pior do que nenhuma: dizia a
    // quem arranca que uma capacidade fechada ainda não existe, e arrastava o
    // sistema inteiro para `Degraded` por causa disso.
    //
    // Não depende de configuração nem de recursos: o calendário é domínio do
    // Core e persistência, que são críticos e já foram avaliados acima. Se a
    // persistência cair, o arranque bloqueia antes de esta linha importar.
    components.push(component(
        ReadinessComponentId::Calendar,
        SystemCapabilityState::Available,
        Criticality::Optional,
        reasons::AVAILABLE,
    ));

    // O tempo real, tal como está agora.
    //
    // `Optional`, e é deliberado: sem ele o histórico lê-se, as operações
    // duráveis acontecem, e o que se perde é a chegada instantânea. Torná-lo
    // crítico deitaria a instituição abaixo por causa de um serviço de
    // coordenação efémera (ADR-0012 §9).
    //
    // Três estados e não dois: «não configurado» é uma escolha desta
    // instalação, e «registado e sem resposta» é uma avaria. Dizer o mesmo aos
    // dois faria quem administra procurar uma avaria que não existe.
    let (tempo_real, razao_do_tempo_real) = match realtime {
        None => (
            SystemCapabilityState::NotConfigured,
            reasons::NOT_CONFIGURED,
        ),
        Some(plano) if plano.saudavel() => (SystemCapabilityState::Available, reasons::AVAILABLE),
        Some(plano) if plano.configurado() => {
            (SystemCapabilityState::Unavailable, reasons::UNAVAILABLE)
        }
        Some(_) => (
            SystemCapabilityState::NotConfigured,
            reasons::NOT_CONFIGURED,
        ),
    };
    components.push(component(
        ReadinessComponentId::Realtime,
        tempo_real,
        Criticality::Optional,
        razao_do_tempo_real,
    ));

    let overall = decide(&components);

    PublicReadiness {
        overall,
        contract_version: CONTRACT_VERSION,
        components,
    }
}

/// A decisão, a partir dos componentes.
///
/// Crítico que não esteja disponível bloqueia. Opcional que não esteja
/// plenamente disponível degrada. Nada mais entra nesta conta — e é o Core que a
/// faz, uma vez.
fn decide(components: &[ReadinessComponent]) -> ReadinessOverall {
    let critico_em_falta = components.iter().any(|c| {
        c.criticality == Criticality::Critical && c.state != SystemCapabilityState::Available
    });
    if critico_em_falta {
        return ReadinessOverall::Blocked;
    }

    let opcional_limitado = components.iter().any(|c| {
        c.criticality == Criticality::Optional && c.state != SystemCapabilityState::Available
    });
    if opcional_limitado {
        ReadinessOverall::Degraded
    } else {
        ReadinessOverall::Ready
    }
}

/// A resposta quando o contrato não coincide.
///
/// Um Core que responde e um Workspace que não o entende não é um sistema
/// pronto. Dizê-lo aqui evita que rebente mais tarde num erro de desserialização
/// que ninguém consegue ler.
#[must_use]
pub fn incompatible(core_version: u32) -> PublicReadiness {
    PublicReadiness {
        overall: ReadinessOverall::Blocked,
        contract_version: core_version,
        components: vec![component(
            ReadinessComponentId::Compatibility,
            SystemCapabilityState::Unavailable,
            Criticality::Critical,
            reasons::INCOMPATIBLE,
        )],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(
        id: ReadinessComponentId,
        state: SystemCapabilityState,
        crit: Criticality,
    ) -> ReadinessComponent {
        component(id, state, crit, reasons::AVAILABLE)
    }

    #[test]
    fn tudo_disponivel_e_ready() {
        let componentes = vec![
            c(
                ReadinessComponentId::Core,
                SystemCapabilityState::Available,
                Criticality::Critical,
            ),
            c(
                ReadinessComponentId::Mail,
                SystemCapabilityState::Available,
                Criticality::Optional,
            ),
        ];
        assert_eq!(decide(&componentes), ReadinessOverall::Ready);
    }

    /// Um opcional em falta degrada. Não bloqueia.
    #[test]
    fn um_opcional_em_falta_nao_bloqueia() {
        for estado in [
            SystemCapabilityState::NotConfigured,
            SystemCapabilityState::NoResource,
            SystemCapabilityState::Planned,
            SystemCapabilityState::Unavailable,
            SystemCapabilityState::Degraded,
        ] {
            let componentes = vec![
                c(
                    ReadinessComponentId::Core,
                    SystemCapabilityState::Available,
                    Criticality::Critical,
                ),
                c(ReadinessComponentId::Mail, estado, Criticality::Optional),
            ];
            assert_eq!(
                decide(&componentes),
                ReadinessOverall::Degraded,
                "um correio em «{}» impediu o arranque",
                estado.as_str()
            );
        }
    }

    /// Um crítico em falta bloqueia, mesmo que tudo o resto esteja bem.
    #[test]
    fn um_critico_em_falta_bloqueia() {
        for estado in [
            SystemCapabilityState::Unavailable,
            SystemCapabilityState::Degraded,
            SystemCapabilityState::NotConfigured,
            SystemCapabilityState::NoResource,
            SystemCapabilityState::Planned,
        ] {
            let componentes = vec![
                c(
                    ReadinessComponentId::Persistence,
                    estado,
                    Criticality::Critical,
                ),
                c(
                    ReadinessComponentId::Mail,
                    SystemCapabilityState::Available,
                    Criticality::Optional,
                ),
            ];
            assert_eq!(
                decide(&componentes),
                ReadinessOverall::Blocked,
                "uma persistência em «{}» deixou o sistema arrancar",
                estado.as_str()
            );
        }
    }

    /// Uma capacidade que o catálogo não conhece não é «disponível».
    #[test]
    fn uma_capacidade_ausente_do_catalogo_nao_e_disponivel() {
        let vazio = SystemCapabilities {
            capabilities: Vec::new(),
        };
        assert_eq!(
            worst(&vazio, &[SystemCapability::Mail]),
            SystemCapabilityState::Unavailable
        );
    }

    /// O pior estado governa um módulo composto.
    #[test]
    fn o_pior_estado_governa_o_modulo() {
        use ocinye_contracts::SystemCapabilityReport;
        let catalogo = SystemCapabilities {
            capabilities: vec![
                SystemCapabilityReport::new(
                    SystemCapability::Mail,
                    SystemCapabilityState::Available,
                    "",
                ),
                SystemCapabilityReport::new(
                    SystemCapability::MailSend,
                    SystemCapabilityState::NotConfigured,
                    "",
                ),
            ],
        };
        assert_eq!(
            worst(
                &catalogo,
                &[SystemCapability::Mail, SystemCapability::MailSend]
            ),
            SystemCapabilityState::NotConfigured,
            "o correio disse-se disponível com o envio por configurar"
        );
    }

    /// Nenhuma razão pública sai fora do conjunto fechado.
    #[test]
    fn as_razoes_publicas_sao_todas_do_conjunto_fechado() {
        for estado in [
            SystemCapabilityState::Available,
            SystemCapabilityState::NotConfigured,
            SystemCapabilityState::NoResource,
            SystemCapabilityState::Planned,
            SystemCapabilityState::Degraded,
            SystemCapabilityState::Unavailable,
        ] {
            let razao = public_reason(estado);
            assert!(
                reasons::all().contains(&razao),
                "«{razao}» não pertence ao conjunto público"
            );
        }
    }
}
