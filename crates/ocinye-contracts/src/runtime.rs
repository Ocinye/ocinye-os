//! Runtimes: por onde uma pessoa chega a uma Instância (ADR-0018, ADR-0611).
//!
//! # Três runtimes, e não perfis
//!
//! `Web`, `Desktop` e `Dedicated` dizem **por onde** se trabalha; os perfis de
//! Instância (`InstanceProfile`) dizem **o que** a organização usa. Os dois
//! eixos são ortogonais, e nada aqui decide um a partir do outro.
//!
//! # Uma declaração, e uma só
//!
//! O que muda entre runtimes declara-se neste tipo, com ids estáveis. O
//! Workspace lê-o por um único módulo de cliente (`static/runtime.js`); um teste
//! do Workspace obriga esse módulo a nomear exactamente as capacidades daqui.
//!
//! # O runtime não autoriza
//!
//! Uma capacidade de runtime diz se o **cliente consegue** — há diálogo nativo
//! de guardar? — e nunca se a **pessoa pode**. Isso continua a ser o Core a
//! decidir, igual nos três runtimes.

use serde::{Deserialize, Serialize};

/// A versão desta declaração. Muda quando uma capacidade entra, sai ou muda de
/// significado — e só então —, para a casca e a Instância poderem dizer se se
/// entendem (ADR-0704).
pub const CAPABILITY_VERSION: u32 = 1;

/// O runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeMode {
    /// Um navegador moderno — também instalado como PWA.
    Web,
    /// A casca nativa Ocinye Desktop.
    Desktop,
    /// A casca num posto dedicado, com política de arranque e de modo.
    Dedicated,
}

impl RuntimeMode {
    /// Os três, por esta ordem.
    pub const ALL: [RuntimeMode; 3] = [Self::Web, Self::Desktop, Self::Dedicated];

    /// O id estável.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Web => "web",
            Self::Desktop => "desktop",
            Self::Dedicated => "dedicated",
        }
    }
}

/// Se uma capacidade existe neste runtime.
///
/// `Limited` é um valor de primeira classe, e não um `Yes` envergonhado: a Web
/// tem notificações **se** a pessoa as permitir, e o Browser integrado só
/// mostra dentro os sites que aceitam ser incorporados.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Availability {
    /// Existe.
    Yes,
    /// Existe com limites que a interface tem de dizer.
    Limited,
    /// Não existe; a interface degrada e oferece alternativa.
    No,
}

impl Availability {
    /// O id estável.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Yes => "yes",
            Self::Limited => "limited",
            Self::No => "no",
        }
    }
}

/// Uma capacidade de runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeCapability {
    /// Ler ou escrever ficheiros do computador sem diálogo (nunca na Web).
    NativeFilesystem,
    /// Escolher ficheiros do computador para abrir.
    NativeOpenDialog,
    /// Escolher onde guardar no computador.
    NativeSaveDialog,
    /// Mostrar sites externos dentro do Ocinye (Browser integrado).
    ExternalWebview,
    /// Notificações do sistema anfitrião.
    NativeNotifications,
    /// Receber ligações `ocinye://` do sistema anfitrião.
    ProtocolHandler,
    /// Escrever na área de transferência por gesto da pessoa.
    NativeClipboard,
    /// Continuar a trabalhar com a janela fechada.
    BackgroundExecution,
    /// Actualizar o próprio cliente.
    NativeUpdates,
    /// Ocupar o ecrã inteiro como ambiente de trabalho.
    FullscreenWorkspace,
}

impl RuntimeCapability {
    /// Todas, na ordem em que se apresentam.
    pub const ALL: [RuntimeCapability; 10] = [
        Self::NativeFilesystem,
        Self::NativeOpenDialog,
        Self::NativeSaveDialog,
        Self::ExternalWebview,
        Self::NativeNotifications,
        Self::ProtocolHandler,
        Self::NativeClipboard,
        Self::BackgroundExecution,
        Self::NativeUpdates,
        Self::FullscreenWorkspace,
    ];

    /// O id estável — o nome que o `runtime.js` usa.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NativeFilesystem => "native_filesystem",
            Self::NativeOpenDialog => "native_open_dialog",
            Self::NativeSaveDialog => "native_save_dialog",
            Self::ExternalWebview => "external_webview",
            Self::NativeNotifications => "native_notifications",
            Self::ProtocolHandler => "protocol_handler",
            Self::NativeClipboard => "native_clipboard",
            Self::BackgroundExecution => "background_execution",
            Self::NativeUpdates => "native_updates",
            Self::FullscreenWorkspace => "fullscreen_workspace",
        }
    }

    /// O que a arquitectura permite a cada runtime — o **alvo** da matriz
    /// (`docs/runtime/CAPABILITY_MATRIX.md`), e não o que já está provado.
    ///
    /// A Web nunca tem `Yes` onde o navegador impõe limites: prometer menos do
    /// que se entrega é aceitável; o contrário é mentir.
    #[must_use]
    pub const fn target(self, mode: RuntimeMode) -> Availability {
        use Availability::{Limited, No, Yes};
        match (self, mode) {
            (Self::NativeFilesystem, RuntimeMode::Web) => No,
            (Self::NativeOpenDialog, _) => Yes,
            (Self::NativeSaveDialog, RuntimeMode::Web) => Limited,
            (Self::ExternalWebview, RuntimeMode::Web) => Limited,
            (Self::NativeNotifications, RuntimeMode::Web) => Limited,
            (Self::ProtocolHandler, RuntimeMode::Web) => Limited,
            (Self::NativeClipboard, RuntimeMode::Web) => Limited,
            (Self::BackgroundExecution, RuntimeMode::Web) => No,
            (Self::NativeUpdates, RuntimeMode::Web) => No,
            (Self::FullscreenWorkspace, RuntimeMode::Web) => Limited,
            // A casca não lê nem escreve caminhos arbitrários: tudo o que toca
            // no anfitrião passa por um diálogo (ADR-0703 §2).
            (Self::NativeFilesystem, _) => No,
            (_, RuntimeMode::Desktop | RuntimeMode::Dedicated) => Yes,
        }
    }
}

/// A declaração de um cliente: o runtime e o que ele consegue.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeCapabilities {
    /// O runtime.
    pub mode: RuntimeMode,
    /// A versão da casca, quando há casca (`Desktop`/`Dedicated`).
    #[serde(default)]
    pub shell_version: Option<String>,
    /// A versão desta declaração ([`CAPABILITY_VERSION`]).
    pub capability_version: u32,
    /// Cada capacidade e a sua disponibilidade, pela ordem de
    /// [`RuntimeCapability::ALL`].
    pub capabilities: Vec<(RuntimeCapability, Availability)>,
}

impl RuntimeCapabilities {
    /// A declaração-alvo de um runtime.
    #[must_use]
    pub fn target(mode: RuntimeMode) -> Self {
        Self {
            mode,
            shell_version: None,
            capability_version: CAPABILITY_VERSION,
            capabilities: RuntimeCapability::ALL
                .iter()
                .map(|c| (*c, c.target(mode)))
                .collect(),
        }
    }

    /// A disponibilidade de uma capacidade (`No` se a declaração a omitir).
    #[must_use]
    pub fn get(&self, capability: RuntimeCapability) -> Availability {
        self.capabilities
            .iter()
            .find(|(c, _)| *c == capability)
            .map_or(Availability::No, |(_, a)| *a)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn os_ids_sao_estaveis_e_unicos() {
        let ids: std::collections::BTreeSet<_> =
            RuntimeCapability::ALL.iter().map(|c| c.as_str()).collect();
        assert_eq!(ids.len(), RuntimeCapability::ALL.len());
        for c in RuntimeCapability::ALL {
            let json = serde_json::to_string(&c).expect("serializa");
            assert_eq!(
                json,
                format!("\"{}\"", c.as_str()),
                "o id serde e o id estável divergem"
            );
        }
        for m in RuntimeMode::ALL {
            assert_eq!(
                serde_json::to_string(&m).unwrap(),
                format!("\"{}\"", m.as_str())
            );
        }
    }

    /// A Web é a linha de base, mas não se lhe atribui o que o navegador não dá.
    #[test]
    fn a_web_nunca_promete_o_que_o_navegador_impede() {
        use RuntimeCapability as C;
        let web = RuntimeCapabilities::target(RuntimeMode::Web);
        for c in [
            C::NativeFilesystem,
            C::BackgroundExecution,
            C::NativeUpdates,
        ] {
            assert_eq!(web.get(c), Availability::No, "{c:?} na Web");
        }
        for c in [
            C::ExternalWebview,
            C::NativeNotifications,
            C::ProtocolHandler,
        ] {
            assert_eq!(web.get(c), Availability::Limited, "{c:?} na Web");
        }
        assert_eq!(web.get(C::NativeOpenDialog), Availability::Yes);
    }

    /// Nenhum runtime ganha acesso livre ao sistema de ficheiros do anfitrião.
    #[test]
    fn nenhum_runtime_le_caminhos_arbitrarios() {
        for m in RuntimeMode::ALL {
            assert_eq!(
                RuntimeCapability::NativeFilesystem.target(m),
                Availability::No,
                "{m:?}"
            );
        }
    }

    #[test]
    fn a_declaracao_leva_todas_as_capacidades_pela_ordem() {
        let d = RuntimeCapabilities::target(RuntimeMode::Desktop);
        let ordem: Vec<_> = d.capabilities.iter().map(|(c, _)| *c).collect();
        assert_eq!(ordem, RuntimeCapability::ALL.to_vec());
        assert_eq!(d.capability_version, CAPABILITY_VERSION);
    }
}
