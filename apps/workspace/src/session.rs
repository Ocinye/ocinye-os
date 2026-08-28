//! Server-side sessions.
//!
//! The browser holds an opaque identifier; the Core session token stays here.
//! That is the whole point of the Backend-for-Frontend shape (ADR-0601).
//!
//! Under ADR-0103 the token this holds is a Core session token rather than an
//! OIDC access token. The shape did not change — what the Workspace holds on
//! the member's behalf did.
//!
//! # Storage
//!
//! Sessions live in this process's memory. A restart therefore signs everyone
//! out, which is an accepted limitation of the foundation, not a hidden one:
//! moving the store to Redis is `PLANNED` and is a contained change behind this
//! type's interface.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use rand::rngs::SysRng;
use rand::TryRng;

/// Name of the session cookie.
pub const COOKIE_NAME: &str = "ocinye_session";

/// A signed-in member's session.
#[derive(Debug, Clone)]
pub struct Session {
    /// Core session token. Never leaves this process.
    pub access_token: String,
    /// Display name, for the interface.
    pub display_name: String,
    /// O endereço com que a pessoa entrou.
    ///
    /// Held only so the first-access screen can tell a password manager which
    /// account the new password belongs to. Without it the browser saves the
    /// credential with no name and, at the next sign-in, fills the password
    /// while leaving the account empty.
    ///
    /// It is never sent to the Core and never authorises anything: the session
    /// token is what identifies the member.
    pub email: String,
    /// Whether the member still owes the Core a permanent password.
    ///
    /// Mirrors the Core's session state so the Workspace can send them to the
    /// right screen. It is **not** the enforcement: the Core refuses ordinary
    /// work on such a session regardless of what this says, which is what makes
    /// typing a URL by hand useless (briefing §23).
    pub must_change_password: bool,
    /// When the session expires.
    pub expires_at: Instant,
}

/// Sessions held on members' behalf.
#[derive(Clone)]
pub struct SessionStore {
    inner: Arc<Mutex<Inner>>,
}

#[derive(Default)]
struct Inner {
    sessions: HashMap<String, Session>,
}

impl SessionStore {
    /// Create an empty store.
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(Inner::default())),
        }
    }

    /// O registo, mesmo depois de alguém ter entrado em pânico a segurá-lo.
    ///
    /// # Porque recuperar, e não desistir
    ///
    /// `Mutex::lock` só falha por envenenamento: alguém entrou em pânico com o
    /// registo na mão. As quatro operações abaixo faziam então a mesma coisa —
    /// desistiam em silêncio — e o silêncio saía pelo lado errado:
    ///
    /// - `get` devolvia `None`, que quem chama lê como **«não tem sessão»**, e
    ///   a pessoa era mandada ao Login. Uma falha do registo apresentada como
    ///   uma resposta sobre a identidade dela.
    /// - `create` devolvia um identificador de uma sessão que nunca foi
    ///   guardada: a entrada parecia bem-sucedida e o pedido seguinte mandava a
    ///   pessoa de volta ao Login, sem nada a explicar porquê.
    /// - `remove` deixava a sessão viva depois de sair.
    ///
    /// É a mesma família que o arranque distingue com tanto cuidado: ausência
    /// de resposta lida como resposta negativa. `Blocked` e `Unreachable` são
    /// estados diferentes; «não tem sessão» e «não foi possível perguntar»
    /// também têm de ser.
    ///
    /// # Porque recuperar é seguro aqui
    ///
    /// O que o `Mutex` protege é um `HashMap<String, Session>` e mais nada. Não
    /// há invariante entre chaves, não há índice paralelo a manter coerente, e
    /// nenhuma das operações deixa o mapa a meio de uma transição: um pânico
    /// durante um `insert` ou um `retain` deixa um mapa com mais ou menos
    /// entradas do que se queria, nunca um mapa inválido. Uma entrada a mais é
    /// varrida pela passagem seguinte; uma a menos é uma sessão que acabou.
    ///
    /// Recuperar deliberadamente é, por isso, melhor do que propagar: propagar
    /// transformaria um pânico isolado em toda a gente de fora, para sempre.
    fn registo(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|envenenado| {
            // Deliberado, e não descuido. Ver acima porque é que o mapa
            // sobrevive intacto ao pânico de quem o segurava.
            envenenado.into_inner()
        })
    }

    /// Create a session, returning its opaque identifier.
    pub fn create(&self, session: Session) -> String {
        let id = new_id();
        self.registo().sessions.insert(id.clone(), session);
        id
    }

    /// Look up a live session.
    ///
    /// `None` significa uma coisa só: não há aqui sessão viva com este
    /// identificador. Nunca significa que não foi possível perguntar.
    pub fn get(&self, id: &str) -> Option<Session> {
        let inner = self.registo();
        let session = inner.sessions.get(id)?;
        (session.expires_at > Instant::now()).then(|| session.clone())
    }

    /// End a session.
    pub fn remove(&self, id: &str) {
        self.registo().sessions.remove(id);
    }

    /// Drop everything that has expired.
    pub fn sweep(&self) {
        let now = Instant::now();
        self.registo()
            .sessions
            .retain(|_, session| session.expires_at > now);
    }

    /// Sweep periodically in the background.
    pub fn spawn_sweeper(self) {
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(Duration::from_secs(60));
            loop {
                ticker.tick().await;
                self.sweep();
            }
        });
    }
}

impl Default for SessionStore {
    fn default() -> Self {
        Self::new()
    }
}

/// A 256-bit identifier from OS entropy.
///
/// Session identifiers are bearer credentials; a predictable one is a
/// session-fixation vulnerability.
fn new_id() -> String {
    let mut bytes = [0_u8; 32];
    SysRng
        .try_fill_bytes(&mut bytes)
        .expect("o sistema não deu entropia para um identificador de sessão");
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// Build the `Set-Cookie` value for a session.
///
/// `HttpOnly` keeps scripts away from it, `SameSite=Lax` blunts CSRF while
/// still allowing the OIDC redirect back, and `Secure` is on everywhere except
/// an explicit local opt-out.
#[must_use]
pub fn cookie_header(id: &str, secure: bool, max_age: Duration) -> String {
    let mut cookie = format!(
        "{COOKIE_NAME}={id}; Path=/; HttpOnly; SameSite=Lax; Max-Age={}",
        max_age.as_secs()
    );
    if secure {
        cookie.push_str("; Secure");
    }
    cookie
}

/// Build the `Set-Cookie` value that clears a session.
#[must_use]
pub fn clear_cookie_header(secure: bool) -> String {
    let mut cookie = format!("{COOKIE_NAME}=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0");
    if secure {
        cookie.push_str("; Secure");
    }
    cookie
}

/// Read the session identifier from a `Cookie` header.
#[must_use]
pub fn session_id_from_cookies(header: Option<&str>) -> Option<String> {
    header?
        .split(';')
        .filter_map(|pair| pair.trim().split_once('='))
        .find(|(name, _)| *name == COOKIE_NAME)
        .map(|(_, value)| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

/// O nome do cookie onde o browser declara a sua zona horária.
///
/// Escrito por JavaScript, e por isso sem `HttpOnly`: o que ele leva é o nome de
/// um fuso — `Europe/Lisbon` —, que não é segredo nem identifica ninguém.
pub const ZONE_COOKIE: &str = "oc_tz";

/// A zona que o browser declarou, se declarou.
///
/// # Porque o browser, e não uma preferência guardada
///
/// Porque o Ocinye não tem preferência de fuso por pessoa, e inventar uma
/// enquanto se corrige um agrupamento seria decidir uma coisa que ninguém pediu.
/// O browser já é a fonte de onde o formulário de marcação tira a zona de quem
/// marca; é a mesma fonte, para a mesma pergunta.
#[must_use]
pub fn zone_from_cookies(header: Option<&str>) -> Option<String> {
    let bruto = header?
        .split(';')
        .filter_map(|pair| pair.trim().split_once('='))
        .find(|(name, _)| *name == ZONE_COOKIE)
        .map(|(_, value)| value.trim().to_owned())
        .filter(|value| !value.is_empty())?;

    Some(descodificar(&bruto)).filter(|valor| !valor.is_empty())
}

/// Descodifica os `%XX` que o browser escreve.
///
/// # Porque isto é preciso
///
/// `encodeURIComponent('Europe/Lisbon')` dá `Europe%2FLisbon`, e um nome de fuso
/// com `%2F` não existe em base de dados de fusos nenhuma. Sem esta conversão a
/// zona era sempre inválida, caía em UTC, e o Calendário continuava a agrupar em
/// Greenwich — com um cookie a dizer o contrário, que é a pior maneira de um
/// defeito se esconder.
///
/// Só descodifica; não interpreta. O que sair daqui continua a ter de ser um
/// fuso conhecido para ser aceite.
fn descodificar(valor: &str) -> String {
    let bytes = valor.as_bytes();
    let mut saida = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or("");
            if let Ok(byte) = u8::from_str_radix(hex, 16) {
                saida.push(byte);
                i += 3;
                continue;
            }
        }
        saida.push(bytes[i]);
        i += 1;
    }
    String::from_utf8(saida).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_zona_do_browser_chega_descodificada() {
        // `encodeURIComponent('Europe/Lisbon')` dá `Europe%2FLisbon`, e sem
        // descodificar isto o fuso era sempre inválido — e o Calendário
        // agrupava em UTC com um cookie a dizer que não.
        assert_eq!(
            zone_from_cookies(Some("oc_tz=Europe%2FLisbon")).as_deref(),
            Some("Europe/Lisbon")
        );
        assert_eq!(
            zone_from_cookies(Some("a=b; oc_tz=America%2FSao_Paulo; c=d")).as_deref(),
            Some("America/Sao_Paulo")
        );
        // Já descodificado passa na mesma.
        assert_eq!(zone_from_cookies(Some("oc_tz=UTC")).as_deref(), Some("UTC"));
        assert_eq!(zone_from_cookies(Some("outro=x")), None);
        assert_eq!(zone_from_cookies(None), None);
        // Um `%` solto não faz rebentar nada.
        assert_eq!(zone_from_cookies(Some("oc_tz=%")).as_deref(), Some("%"));
    }

    fn session() -> Session {
        Session {
            access_token: "token".into(),
            display_name: "Member".into(),
            email: "member@ocinye.com".into(),
            must_change_password: false,
            expires_at: Instant::now() + Duration::from_secs(60),
        }
    }

    #[test]
    fn identifiers_are_unpredictable() {
        assert_ne!(new_id(), new_id());
        assert_eq!(new_id().len(), 64);
    }

    #[test]
    fn expired_sessions_are_not_returned() {
        let store = SessionStore::new();
        let id = store.create(Session {
            expires_at: Instant::now() - Duration::from_secs(1),
            ..session()
        });
        assert!(store.get(&id).is_none());
    }

    #[test]
    fn a_removed_session_is_gone() {
        // Rotation after a password change works by removing and recreating:
        // the old identifier must not resolve afterwards (briefing §30).
        let store = SessionStore::new();
        let id = store.create(session());
        assert!(store.get(&id).is_some());
        store.remove(&id);
        assert!(store.get(&id).is_none());
    }

    #[test]
    fn a_restricted_session_is_marked_as_such() {
        let store = SessionStore::new();
        let id = store.create(Session {
            must_change_password: true,
            ..session()
        });
        assert!(store.get(&id).unwrap().must_change_password);
    }

    #[test]
    fn cookies_are_httponly_samesite_and_secure_by_default() {
        let cookie = cookie_header("abc", true, Duration::from_secs(3600));
        assert!(cookie.contains("HttpOnly"));
        assert!(cookie.contains("SameSite=Lax"));
        assert!(cookie.contains("Secure"));
        assert!(!clear_cookie_header(true).contains("abc"));
    }

    #[test]
    fn the_session_identifier_is_read_from_a_cookie_header() {
        assert_eq!(
            session_id_from_cookies(Some("other=1; ocinye_session=abc123; x=2")).as_deref(),
            Some("abc123")
        );
        assert!(session_id_from_cookies(Some("other=1")).is_none());
        assert!(session_id_from_cookies(None).is_none());
    }

    /// Um registo envenenado continua a responder, e não desautentica ninguém.
    ///
    /// # A pergunta
    ///
    /// «Uma falha do registo de sessões pode ser confundida com não estar
    /// autenticado?» A resposta tem de ser não, e tem de ser demonstrável.
    ///
    /// # Como se envenena um `Mutex` de propósito
    ///
    /// Uma thread entra em pânico com o registo na mão. A partir daí, todo o
    /// `lock()` devolve erro — que é exactamente o estado que fazia `get`
    /// devolver `None` e a pessoa acabar no Login sem ter sido desautenticada
    /// por ninguém.
    #[test]
    fn um_registo_envenenado_nao_desautentica_ninguem() {
        let store = SessionStore::new();
        let id = store.create(sessao());
        assert!(store.get(&id).is_some(), "controlo positivo");

        let interior = store.inner.clone();
        let panico = std::thread::spawn(move || {
            let _guarda = interior.lock().expect("registo");
            panic!("alguém entrou em pânico com o registo na mão");
        })
        .join();
        assert!(panico.is_err(), "a thread tinha de entrar em pânico");
        assert!(
            store.inner.lock().is_err(),
            "o registo tinha de ficar envenenado, senão este teste não prova nada"
        );

        // A pergunta.
        assert!(
            store.get(&id).is_some(),
            "uma falha do registo apresentou-se como «esta pessoa não tem sessão»"
        );

        // E as outras três operações continuam a acontecer de verdade.
        let outra = store.create(sessao());
        assert!(
            store.get(&outra).is_some(),
            "`create` devolveu um identificador de uma sessão que nunca guardou"
        );
        store.remove(&outra);
        assert!(
            store.get(&outra).is_none(),
            "sair não terminou a sessão: ela continua viva depois do `remove`"
        );
        store.sweep();
        assert!(
            store.get(&id).is_some(),
            "a varredura levou uma sessão viva"
        );
    }

    fn sessao() -> Session {
        Session {
            access_token: "t".to_owned(),
            display_name: "Alguém".to_owned(),
            email: "alguem@ocinye.com".to_owned(),
            must_change_password: false,
            expires_at: Instant::now() + Duration::from_secs(600),
        }
    }
}
