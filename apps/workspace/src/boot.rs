//! O arranque institucional do Ocinye OS.
//!
//! # O que isto é, e o que não é
//!
//! O Splash não é uma animação de carregamento. É a representação visível da
//! prontidão institucional — e a diferença é toda: uma animação enche o tempo
//! enquanto se espera, e isto mostra uma decisão que o Core já tomou.
//!
//!     O Core decide se o sistema está pronto. A Experience apresenta a resposta.
//!
//! Nada neste módulo conclui prontidão. Ele pergunta, lê a resposta tipada e
//! escolhe que superfície mostrar. Contar componentes verdes aqui seria uma
//! segunda política de arranque, escrita no browser, e duas políticas acabam por
//! discordar.
//!
//! # Bloqueado e sem resposta são coisas diferentes
//!
//! `Blocked` é o Core a dizer «não estou suficientemente pronto». `Unreachable`
//! é não termos obtido decisão nenhuma. Ambos impedem o arranque e são
//! diagnósticos opostos: um sabe-se, o outro não. Juntá-los faria a interface
//! afirmar que o Core decidiu quando o Core nem sequer respondeu.
//!
//! Por isso `Unreachable` vive deste lado. Não é um `ReadinessOverall` — não é
//! uma resposta do Core, é a ausência dela.

use ocinye_contracts::readiness::{PublicReadiness, ReadinessOverall, CONTRACT_VERSION};

use crate::WorkspaceState;

/// Nome do marcador de arranque.
///
/// Um cookie de sessão do browser: morre quando a janela fecha, que é a mesma
/// vida que o `sessionStorage` teria. Aqui é preciso do lado do servidor, porque
/// é o servidor que decide se mostra o Splash.
pub const MARKER_COOKIE: &str = "oc_boot";

/// O que o arranque tem para mostrar.
///
/// Cinco superfícies, e não vinte. Cada uma corresponde a uma situação
/// realmente distinta para quem está do outro lado do ecrã.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BootState {
    /// Ainda não se perguntou nada.
    Uninitialized,
    /// Perguntou-se, e a resposta ainda não chegou.
    Checking,
    /// O Core disse que se pode seguir.
    Ready,
    /// O Core disse que se pode seguir com menos.
    Degraded,
    /// O Core disse que não.
    Blocked,
    /// O Core não disse nada.
    ///
    /// Deste lado da fronteira de propósito: é a incapacidade da Experience de
    /// obter uma decisão, e não uma decisão.
    Unreachable,
}

impl BootState {
    /// Se o arranque pode passar à resolução de sessão.
    #[must_use]
    pub const fn may_hand_off(&self) -> bool {
        matches!(self, Self::Ready | Self::Degraded)
    }

    /// A classe CSS que a superfície usa.
    #[must_use]
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::Uninitialized | Self::Checking => "checking",
            Self::Ready => "ready",
            Self::Degraded => "degraded",
            Self::Blocked => "blocked",
            Self::Unreachable => "unreachable",
        }
    }
}

/// O que o arranque apurou.
#[derive(Debug, Clone)]
pub struct BootOutcome {
    /// Que superfície mostrar.
    pub state: BootState,
    /// O que o Core respondeu, quando respondeu.
    ///
    /// `None` em `Unreachable`: não há projecção quando não houve resposta, e
    /// inventar uma vazia faria a interface mostrar uma lista de componentes
    /// que ninguém afirmou.
    pub readiness: Option<PublicReadiness>,
}

impl BootOutcome {
    /// Os componentes que limitam o que se pode fazer.
    ///
    /// Só os opcionais que não estão disponíveis: são estes que explicam o
    /// «pronto com limitações». Um crítico em baixo não limita — bloqueia, e
    /// nesse caso a superfície é outra.
    #[must_use]
    pub fn limitations(&self) -> Vec<&ocinye_contracts::readiness::ReadinessComponent> {
        use ocinye_contracts::readiness::Criticality;
        use ocinye_contracts::system_capability::SystemCapabilityState;

        self.readiness
            .as_ref()
            .map(|r| {
                r.components
                    .iter()
                    .filter(|c| c.criticality == Criticality::Optional)
                    .filter(|c| !matches!(c.state, SystemCapabilityState::Available))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Os componentes críticos que impedem o arranque.
    #[must_use]
    pub fn blockers(&self) -> Vec<&ocinye_contracts::readiness::ReadinessComponent> {
        use ocinye_contracts::readiness::Criticality;
        use ocinye_contracts::system_capability::SystemCapabilityState;

        self.readiness
            .as_ref()
            .map(|r| {
                r.components
                    .iter()
                    .filter(|c| c.criticality == Criticality::Critical)
                    .filter(|c| !matches!(c.state, SystemCapabilityState::Available))
                    .collect()
            })
            .unwrap_or_default()
    }
}

/// Pergunta ao Core se o sistema está pronto.
///
/// # Porque é que o contrato vai no pedido
///
/// Um Workspace e um Core instalados em separado podem ficar de gerações
/// diferentes. Dizer qual o contrato que este binário fala permite ao Core
/// responder «não falamos a mesma língua» no arranque, em vez de a incompatibilidade
/// aparecer mais tarde num erro de desserialização que ninguém consegue ler.
///
/// # Porque é que nada aqui interpreta a lista
///
/// O `overall` vem do Core. Este código não o recalcula a partir dos
/// componentes, nem decide que um componente em baixo «provavelmente não é
/// grave». Essa é a política de arranque, e ela vive de um lado só.
pub async fn probe(state: &WorkspaceState) -> BootOutcome {
    let resposta = state
        .http
        .get(format!(
            "{}/ready?contract={CONTRACT_VERSION}",
            state.config.core_url
        ))
        // O arranque não pode ficar pendurado. Uma capacidade opcional lenta não
        // é razão para uma pessoa esperar indefinidamente à porta.
        .timeout(std::time::Duration::from_secs(3))
        .send()
        .await;

    let Ok(resposta) = resposta else {
        return BootOutcome {
            state: BootState::Unreachable,
            readiness: None,
        };
    };

    // O corpo é que decide, e não o estado HTTP. Um `/ready` que responde 503
    // continua a trazer a projecção, e é ela que diz porquê.
    let Ok(prontidao) = resposta.json::<PublicReadiness>().await else {
        // Respondeu algo que não é a projecção que este binário conhece. Isso não
        // é uma decisão do Core sobre prontidão; é não termos obtido decisão.
        return BootOutcome {
            state: BootState::Unreachable,
            readiness: None,
        };
    };

    let state = match prontidao.overall {
        ReadinessOverall::Ready => BootState::Ready,
        ReadinessOverall::Degraded => BootState::Degraded,
        ReadinessOverall::Blocked => BootState::Blocked,
    };

    BootOutcome {
        state,
        readiness: Some(prontidao),
    }
}

/// O cabeçalho que grava o marcador de arranque.
///
/// Sem `Max-Age`: morre com a janela. O Splash é uma cortesia de entrada, não
/// uma preferência a guardar.
#[must_use]
pub fn marker_cookie(secure: bool) -> String {
    let mut cookie = format!("{MARKER_COOKIE}=1; Path=/; HttpOnly; SameSite=Lax");
    if secure {
        cookie.push_str("; Secure");
    }
    cookie
}

/// Se o browser já viu o arranque nesta janela.
///
/// # O que este valor pode, e o que não pode
///
/// Pode dispensar o Splash. Não pode mais nada.
///
/// Forjá-lo salta a apresentação — e é tudo. A sonda de prontidão corre na
/// mesma, portanto um marcador inventado não faz um Core bloqueado parecer
/// pronto; e a sessão é resolvida na mesma, portanto não autentica ninguém.
///
///     O arranque-concluído pode ser guardado como estado de apresentação.
///     A autoridade sobre prontidão não pode.
#[must_use]
pub fn has_marker(cookies: Option<&str>) -> bool {
    cookies.is_some_and(|cabecalho| {
        cabecalho
            .split(';')
            .filter_map(|parte| parte.split_once('='))
            .any(|(nome, valor)| nome.trim() == MARKER_COOKIE && !valor.trim().is_empty())
    })
}

/// Se um destino de regresso é uma rota deste Workspace.
///
/// # Porque é que o catálogo é a autoridade
///
/// O Workspace já tem a lista das suas rotas, e ela é usada para navegação e
/// para os testes estruturais. Uma segunda lista escrita à mão aqui divergiria
/// da primeira ao fim de duas funcionalidades, e a que decide segurança seria
/// justamente a desactualizada.
///
/// # O que é recusado, e porquê
///
/// Tudo o que não seja um caminho interno. Um destino de regresso é a única
/// entrada de terceiros que atravessa o arranque, e é exactamente a forma de um
/// redireccionamento aberto: `https://exemplo.mau`, `//exemplo.mau`,
/// `javascript:`, variantes codificadas, barras invertidas que alguns browsers
/// normalizam para barras.
///
/// A verificação é por **lista de permitidos**, e não por lista de proibidos.
/// Uma lista de proibidos apanha os truques de que alguém se lembrou.
#[must_use]
pub fn safe_return_target(destino: &str, catalogo: &[&str]) -> Option<String> {
    // Tem de começar por uma barra só. `//host` é relativo ao protocolo e sai
    // do sítio; `\\host` é normalizado por alguns browsers para o mesmo.
    if !destino.starts_with('/') || destino.starts_with("//") || destino.starts_with("/\\") {
        return None;
    }
    if destino.contains('\\') {
        return None;
    }
    // Nada de esquema, nem de autoridade, nem de caracteres de controlo.
    if destino.contains(':') || destino.contains('@') {
        return None;
    }
    if destino.chars().any(|c| c.is_control()) {
        return None;
    }

    // O caminho, sem a consulta. É o caminho que o catálogo conhece.
    let caminho = destino.split(['?', '#']).next().unwrap_or(destino);

    let conhecido = catalogo.iter().any(|rota| corresponde(rota, caminho));
    conhecido.then(|| destino.to_owned())
}

/// Se um caminho concreto corresponde a um padrão do catálogo.
///
/// Os padrões trazem segmentos entre chavetas — `/calendar/events/{event_id}` —
/// e um segmento desses aceita qualquer valor que não seja vazio nem contenha
/// barras.
fn corresponde(padrao: &str, caminho: &str) -> bool {
    let padrao: Vec<&str> = padrao.trim_matches('/').split('/').collect();
    let caminho: Vec<&str> = caminho.trim_matches('/').split('/').collect();
    if padrao.len() != caminho.len() {
        return false;
    }
    padrao.iter().zip(caminho.iter()).all(|(p, c)| {
        if p.starts_with('{') && p.ends_with('}') {
            !c.is_empty()
        } else {
            p == c
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const CATALOGO: &[&str] = &[
        "/",
        "/calendar",
        "/calendar/events/{event_id}",
        "/notifications",
        "/login",
    ];

    /// Um destino interno conhecido é preservado.
    #[test]
    fn um_destino_interno_conhecido_e_preservado() {
        assert_eq!(
            safe_return_target("/calendar", CATALOGO).as_deref(),
            Some("/calendar")
        );
        assert_eq!(
            safe_return_target("/calendar/events/abc", CATALOGO).as_deref(),
            Some("/calendar/events/abc")
        );
        assert_eq!(
            safe_return_target("/calendar?view=week", CATALOGO).as_deref(),
            Some("/calendar?view=week"),
            "a consulta faz parte do destino e tem de sobreviver"
        );
    }

    /// Nada sai do Ocinye OS.
    ///
    /// Um destino de regresso é a única entrada de terceiros que atravessa o
    /// arranque. Cada linha aqui é uma forma conhecida de redireccionamento
    /// aberto, e nenhuma pode passar.
    #[test]
    fn nenhum_destino_sai_do_ocinye() {
        for hostil in [
            "https://exemplo.mau",
            "http://exemplo.mau/calendar",
            "//exemplo.mau",
            "//exemplo.mau/calendar",
            "javascript:alert(1)",
            "JavaScript:alert(1)",
            "/\\exemplo.mau",
            "/calendar\\@exemplo.mau",
            "https://utilizador@exemplo.mau",
            "/calendar\n/x",
            "data:text/html,<script>",
            "",
            "calendar",
        ] {
            assert_eq!(
                safe_return_target(hostil, CATALOGO),
                None,
                "«{hostil}» foi aceite como destino de regresso"
            );
        }
    }

    /// Uma rota que este Workspace não tem não é destino.
    ///
    /// Não é só segurança: é evitar entregar alguém numa página que não existe
    /// depois de ter autenticado.
    #[test]
    fn uma_rota_desconhecida_nao_e_destino() {
        assert_eq!(safe_return_target("/inexistente", CATALOGO), None);
        assert_eq!(safe_return_target("/calendar/events", CATALOGO), None);
        assert_eq!(safe_return_target("/calendar/events/a/b", CATALOGO), None);
    }

    #[test]
    fn so_ready_e_degraded_entregam_a_sessao() {
        assert!(BootState::Ready.may_hand_off());
        assert!(BootState::Degraded.may_hand_off());
        assert!(!BootState::Blocked.may_hand_off());
        assert!(!BootState::Unreachable.may_hand_off());
        assert!(!BootState::Checking.may_hand_off());
        assert!(!BootState::Uninitialized.may_hand_off());
    }

    /// Bloqueado e sem resposta não são a mesma superfície.
    ///
    /// São os dois casos que uma interface descuidada junta num «erro», e são
    /// diagnósticos opostos: um é uma decisão do Core, o outro é a ausência
    /// dela.
    #[test]
    fn bloqueado_e_sem_resposta_sao_distintos() {
        assert_ne!(BootState::Blocked, BootState::Unreachable);
        assert_ne!(BootState::Blocked.kind(), BootState::Unreachable.kind());
    }

    #[test]
    fn o_marcador_e_lido_de_entre_outros_cookies() {
        assert!(has_marker(Some("oc_boot=1")));
        assert!(has_marker(Some("oc_session=abc; oc_boot=1; outro=x")));
        assert!(!has_marker(Some("oc_session=abc")));
        assert!(!has_marker(Some("oc_boot=")));
        assert!(!has_marker(None));
    }

    /// O marcador não é legível por scripts, nem viaja para fora.
    ///
    /// Não porque valha alguma coisa — não vale: forjá-lo salta o Splash e mais
    /// nada. É porque um cookie de apresentação legível por scripts convida a
    /// que alguém, um dia, lhe encoste uma decisão. As bandeiras dizem, no
    /// próprio cabeçalho, que isto não é para ser lido nem enviado por terceiros.
    #[test]
    fn o_marcador_nao_e_legivel_por_scripts_nem_viaja_para_fora() {
        let cookie = marker_cookie(false);
        assert!(cookie.contains("HttpOnly"), "{cookie}");
        assert!(cookie.contains("SameSite=Lax"), "{cookie}");
        assert!(cookie.contains("Path=/"), "{cookie}");

        // E em transporte seguro, só em transporte seguro.
        assert!(marker_cookie(true).contains("; Secure"));
        assert!(!cookie.contains("Secure"), "{cookie}");
    }

    /// O valor do marcador não transporta nada.
    ///
    /// Um marcador que levasse consigo o destino, o estado da prontidão ou o
    /// instante do arranque seria uma superfície onde alguém podia escrever. É
    /// um `1`, e a única pergunta que responde é «já viu isto nesta janela».
    #[test]
    fn o_marcador_nao_transporta_informacao() {
        let cookie = marker_cookie(false);
        let valor = cookie
            .split(';')
            .next()
            .and_then(|p| p.split_once('='))
            .map(|(_, v)| v)
            .expect("valor");
        assert_eq!(
            valor, "1",
            "o marcador passou a dizer alguma coisa: {cookie}"
        );
    }

    #[test]
    fn o_marcador_morre_com_a_janela() {
        let cookie = marker_cookie(false);
        assert!(
            !cookie.contains("Max-Age") && !cookie.contains("Expires"),
            "o marcador tem de morrer com a janela, e não sobreviver-lhe: {cookie}"
        );
        assert!(cookie.contains("HttpOnly"));
    }
}
