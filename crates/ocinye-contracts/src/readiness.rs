//! O que o Ocinye OS diz sobre si próprio antes de saber quem está a perguntar.
//!
//! # Duas projecções, uma verdade
//!
//! ```text
//! estado canónico das capacidades
//!             │
//!             ├── projecção autenticada
//!             │      /system/capabilities
//!             │      o catálogo, para quem já entrou
//!             │
//!             └── projecção pública
//!                    /ready
//!                    só o que uma instalação pode dizer a um desconhecido
//! ```
//!
//! **Seguro para um membro não é o mesmo que seguro antes de autenticar.** Uma
//! razão sem `stack trace` nem `hostname` continua a ser segura para quem já
//! entrou e pode, ainda assim, dizer a um desconhecido quantos nós existem ou
//! que adaptador está configurado.
//!
//! Por isso a projecção pública não é uma serialização do catálogo com campos
//! removidos: é um conjunto **fechado**. [`ReadinessComponent`] só sabe nomear o
//! que está em [`ReadinessComponentId`], e acrescentar algo à lista pública é um
//! acto deliberado — não o efeito de alguém ter acrescentado um campo noutro
//! sítio.

use serde::{Deserialize, Serialize};

use crate::system_capability::SystemCapabilityState;

/// A versão do contrato entre o Core e o Workspace.
///
/// # Porque não o `API_VERSION` nem o SHA do Git
///
/// `API_VERSION` é o `v1` do caminho: muda quando a API inteira muda de versão,
/// que é raro por desenho. O SHA é privado e muda a cada commit, incluindo os
/// que não mexem em contrato nenhum.
///
/// Isto é o que precisa de ser comparado quando o Workspace e o Core são
/// **instalados separadamente** e um deles ficou para trás. Ambos compilam
/// contra esta constante; se os binários forem de gerações diferentes, os
/// números diferem e o arranque diz porquê — em vez de rebentar mais tarde num
/// erro de desserialização que ninguém consegue ler.
///
/// Sobe quando o contrato deixa de ser compatível. Não sobe por acrescentos
/// retrocompatíveis.
pub const CONTRACT_VERSION: u32 = 1;

/// Se o sistema pode ser entregue.
///
/// Três estados, e não uma dúzia. A pergunta que isto responde é operacional e
/// tem exactamente três respostas úteis: segue, segue com menos, não segue.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReadinessOverall {
    /// Tudo o que é preciso está pronto.
    Ready,
    /// O núcleo está pronto; alguma capacidade opcional está limitada.
    Degraded,
    /// Uma dependência crítica impede entregar o sistema.
    Blocked,
}

impl ReadinessOverall {
    /// Se o Workspace pode ser apresentado.
    ///
    /// `Degraded` segue: uma instalação sem correio configurado é uma
    /// instalação, não uma avaria.
    #[must_use]
    pub const fn may_proceed(self) -> bool {
        matches!(self, Self::Ready | Self::Degraded)
    }

    /// Representação estável.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::Degraded => "degraded",
            Self::Blocked => "blocked",
        }
    }

    /// Interpreta a representação estável.
    ///
    /// # Fecha em caso de dúvida
    ///
    /// Um valor que este build não conhece lê-se como [`Self::Blocked`]. Uma
    /// versão futura do Core não pode introduzir um estado que um Workspace
    /// antigo desconheça e ele conclua, por omissão, que está tudo bem.
    #[must_use]
    pub fn parse(value: &str) -> Self {
        match value {
            "ready" => Self::Ready,
            "degraded" => Self::Degraded,
            _ => Self::Blocked,
        }
    }
}

/// Se um componente impede o arranque, ou apenas limita o que se pode fazer.
///
/// # Ortogonal ao estado, e de propósito
///
/// `Unavailable` não diz se é grave: a persistência indisponível bloqueia, o
/// correio indisponível não. São duas perguntas — *em que estado está* e *o que
/// isso implica* — e juntá-las num enum só obrigaria a inventar variantes como
/// `UnavailableButFine`.
///
/// **Quem decide isto é o Core.** O Workspace não pode olhar para uma lista de
/// componentes e concluir «o correio falhou, mas acho que é opcional»: seria uma
/// segunda política de arranque, escrita no browser.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Criticality {
    /// Sem isto, o sistema não é entregue.
    Critical,
    /// Sem isto, o sistema é entregue com menos.
    Optional,
}

impl Criticality {
    /// Representação estável.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Critical => "critical",
            Self::Optional => "optional",
        }
    }

    /// Interpreta a representação estável.
    ///
    /// Um valor desconhecido lê-se como [`Self::Critical`]: presumir que algo
    /// que não se entende é dispensável é a maneira mais silenciosa de entregar
    /// um sistema partido.
    #[must_use]
    pub fn parse(value: &str) -> Self {
        match value {
            "optional" => Self::Optional,
            _ => Self::Critical,
        }
    }
}

/// O que a projecção pública pode nomear.
///
/// # Isto é a lista de autorização
///
/// Não há aqui um campo livre. Um componente que não esteja nesta lista não
/// consegue ser serializado para o mundo, e acrescentar um é uma decisão que
/// alguém tem de tomar neste ficheiro — não o efeito colateral de ter
/// acrescentado uma capacidade noutro sítio.
///
/// O que deliberadamente **não** entra: identificadores de capabilities,
/// operações, âmbitos, papéis, permissões, contagens de recursos, nomes de
/// fornecedores ou de nós, adaptadores, topologia. Nada disso é da conta de quem
/// ainda não entrou.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReadinessComponentId {
    /// O núcleo determinístico responde.
    Core,
    /// A persistência institucional está utilizável.
    Persistence,
    /// É possível autenticar e resolver sessões.
    Identity,
    /// O Workspace e o Core falam o mesmo contrato.
    Compatibility,
    /// Armazenamento de objectos.
    Storage,
    /// Correio institucional.
    Mail,
    /// Inferência.
    Intelligence,
    /// Computação.
    Compute,
    /// Calendário.
    Calendar,
}

impl ReadinessComponentId {
    /// Representação estável.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Core => "core",
            Self::Persistence => "persistence",
            Self::Identity => "identity",
            Self::Compatibility => "compatibility",
            Self::Storage => "storage",
            Self::Mail => "mail",
            Self::Intelligence => "intelligence",
            Self::Compute => "compute",
            Self::Calendar => "calendar",
        }
    }

    /// Como se diz a uma pessoa.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Core => "Núcleo institucional",
            Self::Persistence => "Persistência",
            Self::Identity => "Identidade",
            Self::Compatibility => "Compatibilidade",
            Self::Storage => "Armazenamento",
            Self::Mail => "Correio",
            Self::Intelligence => "Inteligência",
            Self::Compute => "Computação",
            Self::Calendar => "Calendário",
        }
    }

    /// Tudo o que a projecção pública sabe nomear.
    #[must_use]
    pub const fn all() -> [Self; 9] {
        [
            Self::Core,
            Self::Persistence,
            Self::Identity,
            Self::Compatibility,
            Self::Storage,
            Self::Mail,
            Self::Intelligence,
            Self::Compute,
            Self::Calendar,
        ]
    }
}

/// Um componente, como o mundo o vê.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReadinessComponent {
    /// Qual.
    pub component: ReadinessComponentId,
    /// Em que estado está.
    pub state: SystemCapabilityState,
    /// Se impede o arranque.
    pub criticality: Criticality,
    /// Porquê, em linguagem institucional.
    ///
    /// `String` porque o Workspace tem de o desserializar; **escrito** sempre a
    /// partir de um conjunto fixo de frases conhecidas, e nunca a partir de uma
    /// mensagem de erro reencaminhada — um erro de base de dados traria o nome
    /// do servidor com ele.
    ///
    /// A garantia vive onde se pode provar: quem constrói escolhe de
    /// [`reasons`], e um teste confirma que nada fora desse conjunto sai daqui.
    pub reason: String,
}

/// O que o Ocinye OS responde a quem ainda não entrou.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicReadiness {
    /// Se o sistema pode ser entregue. **Decidido pelo Core.**
    ///
    /// O Workspace não recalcula isto a partir da lista. Contar componentes
    /// verdes no browser seria uma segunda política de arranque, e duas
    /// políticas acabam por discordar.
    pub overall: ReadinessOverall,
    /// A versão do contrato que este Core fala.
    pub contract_version: u32,
    /// Os componentes, na ordem em que fazem sentido para quem lê.
    pub components: Vec<ReadinessComponent>,
}

impl PublicReadiness {
    /// Os componentes críticos.
    pub fn critical(&self) -> impl Iterator<Item = &ReadinessComponent> {
        self.components
            .iter()
            .filter(|c| c.criticality == Criticality::Critical)
    }

    /// Os componentes opcionais que não estão plenamente disponíveis.
    ///
    /// É o que o arranque degradado mostra: só o que está limitado, e não a
    /// lista inteira.
    pub fn limitations(&self) -> impl Iterator<Item = &ReadinessComponent> {
        self.components.iter().filter(|c| {
            c.criticality == Criticality::Optional && c.state != SystemCapabilityState::Available
        })
    }
}

/// As frases que a projecção pública pode dizer.
///
/// # Porque é um conjunto fechado
///
/// Porque a alternativa é alguém, um dia, passar para aqui o `Display` de um
/// erro «só desta vez». O erro traz consigo o que o produziu: o endereço, a
/// porta, o nome do servidor. Um conjunto fechado torna esse gesto impossível
/// sem passar por este ficheiro.
pub mod reasons {
    /// O núcleo responde.
    pub const CORE_UP: &str = "O núcleo institucional está a responder.";
    /// O núcleo não responde.
    // Não há razão para «o núcleo não está disponível», e não é esquecimento.
    //
    // Esta projecção é escrita pelo Core. Um Core que não esteja disponível não
    // a escreve — não responde de todo, e a Experience lê isso como
    // `Unreachable`, que é a ausência de uma decisão e não uma decisão de
    // indisponibilidade. Guardar aqui uma frase que nenhum caminho pode emitir
    // sugeria uma simetria que não existe: o núcleo é o único componente que
    // não se pode reportar em baixo a si próprio.
    /// A persistência responde.
    pub const PERSISTENCE_UP: &str = "A persistência institucional está disponível.";
    /// A persistência não responde.
    pub const PERSISTENCE_DOWN: &str = "A persistência institucional não está disponível.";
    /// A identidade responde.
    pub const IDENTITY_UP: &str = "É possível iniciar sessão.";
    /// A identidade não responde.
    pub const IDENTITY_DOWN: &str = "O serviço de identidade não está disponível.";
    /// As versões coincidem.
    pub const COMPATIBLE: &str = "Esta versão do Ocinye OS é compatível com o núcleo.";
    /// As versões não coincidem.
    pub const INCOMPATIBLE: &str =
        "Esta versão do Ocinye OS não é compatível com o núcleo actualmente disponível. \
         Actualize a aplicação e tente novamente.";
    /// Um componente opcional está pronto.
    pub const AVAILABLE: &str = "Disponível.";
    /// Um componente opcional não foi configurado nesta instalação.
    pub const NOT_CONFIGURED: &str = "Não configurado nesta instalação.";
    /// Um componente opcional não tem recursos registados.
    pub const NO_RESOURCE: &str = "Nenhum recurso registado.";
    /// Um componente opcional está registado e não responde.
    pub const UNAVAILABLE: &str = "Registado, mas sem resposta.";
    /// Um componente ainda não existe.
    pub const NOT_IMPLEMENTED: &str = "Ainda não implementado.";

    /// Todas as frases que podem sair daqui.
    #[must_use]
    pub const fn all() -> [&'static str; 12] {
        [
            CORE_UP,
            PERSISTENCE_UP,
            PERSISTENCE_DOWN,
            IDENTITY_UP,
            IDENTITY_DOWN,
            COMPATIBLE,
            INCOMPATIBLE,
            AVAILABLE,
            NOT_CONFIGURED,
            NO_RESOURCE,
            UNAVAILABLE,
            NOT_IMPLEMENTED,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Um estado que este build não conhece nunca autoriza o arranque.
    ///
    /// Uma versão futura do Core pode responder algo que este Workspace não
    /// entende. A leitura por omissão tem de ser a que não entrega o sistema —
    /// senão um cliente antigo conclui, por não perceber, que está tudo bem.
    #[test]
    fn um_estado_desconhecido_nunca_e_ready() {
        assert_eq!(ReadinessOverall::parse("ready"), ReadinessOverall::Ready);
        assert_eq!(
            ReadinessOverall::parse("degraded"),
            ReadinessOverall::Degraded
        );
        for desconhecido in ["", "maybe", "READY", "ok", "partially_ready", "starting"] {
            assert_eq!(
                ReadinessOverall::parse(desconhecido),
                ReadinessOverall::Blocked,
                "«{desconhecido}» foi interpretado como algo que deixa arrancar"
            );
        }
    }

    /// Uma criticalidade desconhecida é crítica.
    #[test]
    fn uma_criticalidade_desconhecida_e_critica() {
        assert_eq!(Criticality::parse("optional"), Criticality::Optional);
        for desconhecido in ["", "nice_to_have", "OPTIONAL", "advisory"] {
            assert_eq!(
                Criticality::parse(desconhecido),
                Criticality::Critical,
                "«{desconhecido}» foi tratado como dispensável"
            );
        }
    }

    #[test]
    fn so_ready_e_degraded_deixam_seguir() {
        assert!(ReadinessOverall::Ready.may_proceed());
        assert!(ReadinessOverall::Degraded.may_proceed());
        assert!(!ReadinessOverall::Blocked.may_proceed());
    }

    #[test]
    fn os_identificadores_sobrevivem_a_ida_e_volta() {
        let mut vistos = std::collections::BTreeSet::new();
        for id in ReadinessComponentId::all() {
            assert!(vistos.insert(id.as_str()), "{id:?} repete um identificador");
        }
        assert_eq!(vistos.len(), ReadinessComponentId::all().len());
    }
}
