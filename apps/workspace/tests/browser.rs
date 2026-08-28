//! O Ocinye OS conduzido por um browser a sério.
//!
//! # O que este harness atravessa
//!
//! ```text
//! Chrome → rota do Workspace → HTTP → Ocinye Core → PostgreSQL
//! ```
//!
//! Tudo verdadeiro. O Workspace é o router real, o Core é o router real, a base
//! é a base. Nada aqui é simulado — um harness que trocasse o Core por um duplo
//! provaria o frontend isolado, e a pergunta que interessa é se **uma pessoa
//! consegue usar o sistema**.
//!
//! # O browser é o que está instalado
//!
//! Não se descarrega nenhum. `chromiumoxide` fala Chrome DevTools Protocol com o
//! Chrome da máquina; a CI aponta-lhe o seu. Descarregar centenas de megabytes
//! por corrida para repetir o que já existe seria custo sem proveito, e este
//! projecto já esteve duas vezes com o disco cheio.
//!
//! Salta quando `OCINYE_TEST_DATABASE_URL` não está definida, ou quando não há
//! Chrome — e **falha** quando estão as duas e alguma coisa não funciona.

use std::sync::Arc;
use std::time::Duration;

use chromiumoxide::browser::{Browser, BrowserConfig};
use chromiumoxide::Page;
use futures::StreamExt;
use ocinye_contracts::{CredentialKind, TechnicalRole, UnitRole};
use ocinye_core::config::CoreConfig;
use ocinye_core::modules::identity::{Authenticator, Throttle};
use ocinye_core::modules::mail::provider::UnconfiguredProvider;
use ocinye_core::password::Secret;
use ocinye_core::password::{Hasher, HashingParams};
use ocinye_core_server::state::AppState;
use ocinye_workspace::config::WorkspaceConfig;
use ocinye_workspace::session::SessionStore;
use ocinye_workspace::{routes as workspace_routes, WorkspaceState};
use sqlx::PgPool;
use uuid::Uuid;

// ── Onde encontrar o Chrome ─────────────────────────────────────────────

/// O executável do Chrome, se existir nesta máquina.
///
/// `OCINYE_TEST_CHROME` primeiro, para a CI poder apontar o seu sem depender de
/// onde o sistema o instalou.
fn chrome_path() -> Option<String> {
    if let Ok(caminho) = std::env::var("OCINYE_TEST_CHROME") {
        return std::path::Path::new(&caminho).exists().then_some(caminho);
    }
    [
        "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
        "/Applications/Chromium.app/Contents/MacOS/Chromium",
        "/usr/bin/google-chrome",
        "/usr/bin/chromium",
        "/usr/bin/chromium-browser",
    ]
    .into_iter()
    .find(|caminho| std::path::Path::new(caminho).exists())
    .map(ToOwned::to_owned)
}

/// Espera que o arranque entregue ao Login.
///
/// O portão encaminha qualquer abertura a frio para `/boot`, e o arranque
/// entrega a seguir. Esperar por estado observável — a página ter o formulário —
/// e nunca por tempo adivinhado.
async fn esperar_pelo_login(page: &Page) {
    let inicio = std::time::Instant::now();
    loop {
        let html = page.content().await.unwrap_or_default();
        if html.contains("oc-login__submit") {
            return;
        }
        assert!(
            inicio.elapsed() < std::time::Duration::from_secs(45),
            "o arranque não entregou ao Login: {}",
            &html[..html.len().min(300)]
        );
        tokio::time::sleep(std::time::Duration::from_millis(120)).await;
    }
}

macro_rules! harness {
    () => {{
        let Ok(url) = std::env::var("OCINYE_TEST_DATABASE_URL") else {
            eprintln!("skipping: OCINYE_TEST_DATABASE_URL is not set");
            return;
        };
        let Some(chrome) = chrome_path() else {
            // Saltar é uma conveniência para quem não tem Chrome instalado. Na
            // CI é um defeito: lá o Chrome é localizado num passo próprio, e uma
            // suite de browser que se salta a si mesma é verde a dizer nada.
            assert!(
                std::env::var("CI").is_err(),
                "não há Chrome, e isto é a CI: as viagens de browser não podem \
                 ser saltadas aqui"
            );
            eprintln!("skipping: no Chrome found; set OCINYE_TEST_CHROME");
            return;
        };
        match Harness::start(&url, &chrome).await {
            Some(harness) => harness,
            None => return,
        }
    }};
}

// ── O sistema, levantado de verdade ─────────────────────────────────────

/// O que é preciso para voltar a entrar como alguém.
struct Credenciais {
    username: String,
    password: String,
}

struct Harness {
    /// O lugar ocupado no limite de browsers simultâneos.
    ///
    /// Guardado aqui porque é o `Drop` do harness que o tem de libertar: é
    /// quando o Chrome fecha que há espaço para o próximo.
    _lugar: tokio::sync::OwnedSemaphorePermit,
    pool: PgPool,
    /// A organização deste harness.
    ///
    /// Cada teste tem a sua, mas todos partilham a base de dados e correm em
    /// paralelo. Uma contagem que não filtre por aqui está a contar o trabalho
    /// dos outros.
    organisation_id: Uuid,
    /// O directório de perfil deste browser, apagado quando o harness cai.
    perfil: std::path::PathBuf,
    workspace_url: String,
    core_url: String,
    browser: Browser,
    _handler: tokio::task::JoinHandle<()>,
    core_task: tokio::task::JoinHandle<()>,
    _workspace: tokio::task::JoinHandle<()>,
}

impl Drop for Harness {
    fn drop(&mut self) {
        // Primeiro o processo, depois o directório. Por esta ordem: apagar o
        // perfil por baixo de um Chrome que ainda escreve nele é deixá-lo a
        // escrever num sítio que já não existe.
        self.fechar_o_browser();

        // O perfil é de uma execução e de mais nenhuma. Sem isto, cada teste
        // deixaria para trás um directório de Chrome no disco temporário — e
        // esta suite corre trinta e sete vezes por invocação.
        let _ = std::fs::remove_dir_all(&self.perfil);
    }
}

impl Harness {
    /// Termina o Chrome desta execução, e só o desta execução.
    ///
    /// # Porque não basta deixar cair o `Browser`
    ///
    /// O `chromiumoxide` conta com `kill_on_drop` e diz, no seu próprio código,
    /// que isso não dá garantias de quando o processo é colhido — e recomenda
    /// que seja quem usa a biblioteca a fechá-lo. Medi-o: um minuto depois de a
    /// suite terminar ainda havia dois Chromes vivos; dois minutos depois já
    /// não havia nenhum. Não é uma fuga, é uma demora sem limite — e um harness
    /// que corre muitas vezes seguidas acumula o que não fechou.
    ///
    /// `Browser::kill` é assíncrona e o `Drop` não pode esperar por ela.
    ///
    /// # Porque isto não é um `pkill` largo
    ///
    /// O padrão não é «chrome»: é o directório de perfil desta execução, que
    /// leva um UUID gerado no arranque deste harness. Nenhum outro processo na
    /// máquina o carrega — nem outro teste a correr em paralelo, nem o browser
    /// que a pessoa tem aberto. Localiza-se o identificador, e mata-se por
    /// identificador.
    ///
    /// A auditoria pós-Boot pediu exactamente esta distinção, depois de um
    /// `pkill -f` demasiado largo ter apanhado um serviço que não era o alvo.
    fn fechar_o_browser(&self) {
        let Some(marca) = self.perfil.file_name().and_then(|n| n.to_str()) else {
            return;
        };
        for pid in Self::processos_com(marca) {
            // Silencioso de propósito: entre o `pgrep` e o `kill` o processo
            // pode ter saído sozinho, e um «No such process» por cada um enche
            // a saída dos testes de ruído que não é notícia nenhuma.
            let _ = std::process::Command::new("kill")
                .arg(pid)
                .stderr(std::process::Stdio::null())
                .status();
        }

        // E esperar que saiam mesmo.
        //
        // `kill` devolve quando o sinal foi entregue, não quando o processo
        // acabou. O Chrome trata o `SIGTERM` e leva o seu tempo a fechar,
        // escrevendo cache pelo caminho — e foi isso que encheu o disco de
        // esqueletos de perfil: o directório era apagado e o Chrome, ainda a
        // sair, recriava-o. Dois mil e noventa e três directórios, quatrocentos
        // e quarenta e dois megabytes.
        //
        // Dois segundos chegam com folga; se não chegarem, apaga-se na mesma,
        // porque um perfil a mais é melhor do que um teste que não termina.
        let limite = std::time::Instant::now() + Duration::from_secs(2);
        while std::time::Instant::now() < limite {
            if Self::processos_com(marca).is_empty() {
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    /// Os identificadores de processo que carregam esta marca.
    fn processos_com(marca: &str) -> Vec<String> {
        std::process::Command::new("pgrep")
            .arg("-f")
            .arg(marca)
            .output()
            .map(|saida| {
                String::from_utf8_lossy(&saida.stdout)
                    .lines()
                    .filter_map(|linha| {
                        let pid = linha.trim();
                        pid.parse::<i32>().ok().map(|_| pid.to_owned())
                    })
                    .collect()
            })
            .unwrap_or_default()
    }
}

impl Harness {
    async fn start(database_url: &str, chrome: &str) -> Option<Self> {
        Self::start_com_estaticos(
            database_url,
            chrome,
            &format!("{}/static", env!("CARGO_MANIFEST_DIR")),
        )
        .await
    }

    /// O mesmo harness, servindo os estáticos de outro sítio.
    ///
    /// Existe para uma coisa só: comparar o produto renderizado com o de antes
    /// da consolidação, sem tocar no ficheiro que os outros catorze testes
    /// estão a ler ao mesmo tempo. Trocar a folha de estilos em disco era a
    /// solução óbvia e estava errada — estado global mutável entre testes
    /// concorrentes, que é a família de defeito que este repositório já
    /// aprendeu a desconfiar.
    async fn start_com_estaticos(
        database_url: &str,
        chrome: &str,
        estaticos: &str,
    ) -> Option<Self> {
        let pool = PgPool::connect(database_url)
            .await
            .expect("OCINYE_TEST_DATABASE_URL is set but the database is unreachable");
        sqlx::migrate!("../../migrations")
            .run(&pool)
            .await
            .expect("migrations must apply");

        // ── O Core, no seu próprio porto ────────────────────────────────
        let core_listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("porto para o Core");
        let core_port = core_listener.local_addr().expect("endereço").port();

        let organisation_id: Uuid = sqlx::query_scalar(
            "INSERT INTO organisations (slug, name) VALUES ($1, $2) RETURNING id",
        )
        .bind(format!("e2e-{}", Uuid::new_v4().simple()))
        .bind("Instituição do harness")
        .fetch_one(&pool)
        .await
        .expect("organização");

        let core_state = core_state(pool.clone(), organisation_id, database_url);
        let core = tokio::spawn(async move {
            let app = ocinye_core_server::routes::router(core_state);
            let _ = axum::serve(core_listener, app).await;
        });

        // ── O Workspace, apontado ao Core ───────────────────────────────
        let ws_listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("porto para o Workspace");
        let ws_port = ws_listener.local_addr().expect("endereço").port();
        let workspace_url = format!("http://127.0.0.1:{ws_port}");

        let core_url = format!("http://127.0.0.1:{core_port}");
        let ws_state = workspace_state(&core_url, &workspace_url, estaticos);
        let workspace = tokio::spawn(async move {
            let app = workspace_routes::router(ws_state);
            let _ = axum::serve(ws_listener, app).await;
        });

        // ── O browser ───────────────────────────────────────────────────
        //
        // Um Chrome que não arranca é uma **falha**, nunca um salto.
        //
        // Isto já esteve ao contrário, e custou caro: um `.ok()?` aqui fazia o
        // arranque devolver `None`, o teste sair pela porta do lado, e a suite
        // reportar `ok`. Catorze viagens de browser podiam dizer-se verdes sem
        // ter aberto um único browser — e foi assim que um defeito real de SQL
        // atravessou a CI inteira sem que ninguém o visse.
        //
        // A ausência do Chrome é outra coisa, e essa continua a saltar: é uma
        // condição decidida antes de qualquer trabalho, no macro `harness!`.
        // Aqui o Chrome existe; se não arranca, há alguma coisa errada.
        // Um perfil por harness, e não o partilhado por omissão.
        //
        // Sem isto, o segundo Chrome a arrancar encontra o `SingletonLock` do
        // primeiro e aborta — o Chrome recusa-se, com razão, a partilhar um
        // directório de perfil entre processos. Como os testes correm em
        // paralelo, isso significava que só um deles alguma vez abria um
        // browser. Os outros saíam pela porta do lado e diziam `ok`.
        // Quantos browsers ao mesmo tempo.
        //
        // `cargo test` usa por omissão tantas threads quantos os núcleos, e
        // cada uma destas viagens conduz um Chrome inteiro. Numa máquina
        // ocupada, trinta e oito Chromes em paralelo deixam de ser paralelismo e
        // passam a ser contenção: o processo de teste abortou com `SIGABRT`,
        // sem uma linha a explicar, e cada aborto deixou para trás os Chromes
        // que o `Drop` já não chegou a fechar — duzentos e sessenta e cinco, ao
        // fim de algumas tentativas.
        //
        // Com quatro de cada vez, a mesma suite corre em quarenta e cinco
        // segundos e não deixa nada. O limite vive aqui, e não numa opção que
        // quem invoca tem de se lembrar de passar.
        // O lugar dura o que o harness durar, e não o que o construtor durar.
        //
        // Escrevi-o primeiro como `acquire()` numa variável local: o `Drop` do
        // guarda corria no fim desta função, e o Chrome ficava vivo sem lugar
        // nenhum ocupado. O semáforo limitava a construção — que é rápida — e
        // não os browsers, que é o que custa. O processo continuou a abortar, e
        // a contagem de órfãos a subir.
        let lugar = Arc::clone(concorrencia())
            .acquire_owned()
            .await
            .expect("lugar no harness");

        let perfil = std::env::temp_dir().join(format!("ocinye-e2e-{}", Uuid::new_v4().simple()));

        let config = BrowserConfig::builder()
            .chrome_executable(chrome)
            .user_data_dir(&perfil)
            .no_sandbox()
            .build()
            .unwrap_or_else(|erro| panic!("configuração do browser: {erro}"));
        let (browser, mut handler) = Browser::launch(config)
            .await
            .unwrap_or_else(|erro| panic!("o Chrome não arrancou: {erro}"));
        let handle = tokio::spawn(async move { while handler.next().await.is_some() {} });

        // A marca de que esta viagem chegou mesmo a levantar voo.
        //
        // A ausência de um salto não é prova de execução: um teste que retorna
        // cedo imprime `... ok` e não imprime mais nada. Só uma marca positiva
        // distingue «correu» de «não correu», e é esta linha que o contrato de
        // enumeração conta em `scripts/test-enumeration.sh`.
        println!("VIAGEM LEVANTADA");

        Some(Self {
            _lugar: lugar,
            pool,
            organisation_id,
            perfil,
            workspace_url,
            core_url,
            browser,
            _handler: handle,
            core_task: core,
            _workspace: workspace,
        })
    }

    /// Uma conta com palavra-passe, e o browser com sessão iniciada.
    ///
    /// # A autenticação é a verdadeira
    ///
    /// A conta e a credencial preparam-se fora do browser — é montagem, não é o
    /// que se está a provar. Mas a **entrada** acontece pelo formulário real:
    /// escreve-se o nome e a palavra-passe, submete-se, e o Workspace fala com o
    /// Core. Injectar um cookie provaria que a página abre com um cookie, e não
    /// que uma pessoa consegue entrar.
    async fn sign_in(&self, roles: &[TechnicalRole]) -> (Uuid, Credenciais) {
        let organisation_id: Uuid =
            sqlx::query_scalar("SELECT id FROM organisations ORDER BY created_at DESC LIMIT 1")
                .fetch_one(&self.pool)
                .await
                .expect("organização");

        let handle = format!("e{}", Uuid::new_v4().simple());
        let person_id: Uuid = sqlx::query_scalar(
            "INSERT INTO people (organisation_id, full_name, email, username, status)
                 VALUES ($1, $2, $3, $2, 'active') RETURNING id",
        )
        .bind(organisation_id)
        .bind(&handle)
        .bind(format!("{handle}@ocinye.com"))
        .fetch_one(&self.pool)
        .await
        .expect("pessoa");

        for role in roles {
            sqlx::query("INSERT INTO person_roles (person_id, role) VALUES ($1, $2)")
                .bind(person_id)
                .bind(role.as_str())
                .execute(&self.pool)
                .await
                .expect("papel");
        }

        // A palavra-passe é criada com o mesmo `Hasher` que o Core usa para a
        // verificar. Um verificador escrito de outra maneira faria o teste
        // provar a sua própria aritmética.
        let password = format!("Ocinye-{}-2026!", Uuid::new_v4().simple());
        let hasher = Hasher::new(HashingParams {
            memory_kib: 19 * 1024,
            iterations: 2,
            parallelism: 1,
        });
        let verifier = hasher
            .hash(&Secret::new(password.clone()))
            .expect("verificador");
        // Escrita directa: `credentials::insert` não é reexportado, e alargar a
        // API pública do Core para uma fixture seria pagar em superfície o que
        // se poupa em linhas. O verificador é que tem de ser o verdadeiro, e é.
        sqlx::query(
            "INSERT INTO credentials (person_id, kind, state, verifier, issued_reason)
                 VALUES ($1, $2, 'active', $3, 'harness de browser')",
        )
        .bind(person_id)
        .bind(CredentialKind::Permanent.as_str())
        .bind(&verifier)
        .execute(&self.pool)
        .await
        .expect("credencial");

        // Antes do browser: o Core aceita esta credencial?
        //
        // Isola a pergunta. Se o Core recusar, o problema é a fixture; se
        // aceitar e o browser falhar, o problema está no caminho do Workspace.
        let directo = reqwest::Client::new()
            .post(format!("{}/api/v1/auth/login", self.core_url))
            .json(&serde_json::json!({ "username": handle, "password": password }))
            .send()
            .await
            .expect("pedido ao Core");
        let estado = directo.status();
        let corpo = directo.text().await.unwrap_or_default();
        assert!(
            estado.is_success(),
            "o Core recusou a credencial da fixture: {estado} · {corpo}"
        );

        // Pelo caminho de uma pessoa: abrir, passar pelo arranque, chegar ao
        // Login. O harness modela o produto, e o produto passa pelo arranque.
        let page = self.open("/login").await;
        esperar_pelo_login(&page).await;

        // Diagnóstico antes de tentar: um botão desactivado não submete, e o
        // sintoma seria idêntico ao de credenciais erradas.
        let botao = elemento(&page, "button[type=submit]").await;
        if botao
            .attribute("disabled")
            .await
            .expect("atributo")
            .is_some()
        {
            let ready = reqwest::get(format!("{}/ready", self.core_url))
                .await
                .map(|r| r.status().to_string())
                .unwrap_or_else(|e| format!("sem resposta: {e}"));
            panic!(
                "o botão de entrar está desactivado: o Workspace não considera o Core \
                 pronto. `/ready` do Core respondeu: {ready}"
            );
        }

        elemento(&page, "input[name=username]")
            .await
            .click()
            .await
            .expect("foco")
            .type_str(&handle)
            .await
            .expect("nome");
        elemento(&page, "input[name=password]")
            .await
            .click()
            .await
            .expect("foco")
            .type_str(&password)
            .await
            .expect("palavra-passe");
        elemento(&page, "button[type=submit]")
            .await
            .click()
            .await
            .expect("submeter");
        // Espera-se que o endereço deixe de ser `/login`, e não por um evento de
        // navegação: o POST navega uma vez e a resposta redirecciona outra, e
        // `wait_for_navigation` devolve na primeira — antes de a segunda ter
        // acontecido. Ler o endereço nesse instante dá sempre `/login`.
        let mut destino = String::new();
        for _ in 0..60 {
            destino = page.url().await.expect("endereço").unwrap_or_default();
            if !destino.ends_with("/login") {
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        if destino.ends_with("/login") {
            let diagnostico = self
                .open("/login")
                .await
                .content()
                .await
                .unwrap_or_default();
            let credenciais: (String, String) =
                sqlx::query_as("SELECT kind, state FROM credentials WHERE person_id = $1")
                    .bind(person_id)
                    .fetch_one(&self.pool)
                    .await
                    .unwrap_or_else(|e| ("(nenhuma)".to_owned(), e.to_string()));

            let visivel: String = diagnostico
                .split("oc-login")
                .nth(1)
                .unwrap_or(&diagnostico)
                .chars()
                .take(700)
                .collect();

            panic!(
                "a entrada não passou.\n  utilizador: {handle}\n  credencial: {credenciais:?}\n  \
                 página: {visivel}"
            );
        }

        (
            person_id,
            Credenciais {
                username: handle,
                password,
            },
        )
    }

    /// Entra com uma credencial temporária, como quem recebe um primeiro acesso.
    ///
    /// A credencial é criada como `Temporary`, que é o que faz o Core devolver
    /// uma sessão em estado de mudança de palavra-passe obrigatória. A entrada
    /// é pelo formulário verdadeiro, tal como na entrada ordinária.
    async fn entrar_com_credencial_temporaria(&self) -> Credenciais {
        let handle = format!("t{}", Uuid::new_v4().simple());
        let person_id: Uuid = sqlx::query_scalar(
            "INSERT INTO people (organisation_id, full_name, email, username, status)
                 VALUES ($1, $2, $3, $2, 'active') RETURNING id",
        )
        .bind(self.organisation_id)
        .bind(&handle)
        .bind(format!("{handle}@ocinye.com"))
        .fetch_one(&self.pool)
        .await
        .expect("pessoa");

        let password = format!("Ocinye-{}-2026!", Uuid::new_v4().simple());
        let hasher = Hasher::new(HashingParams {
            memory_kib: 19 * 1024,
            iterations: 2,
            parallelism: 1,
        });
        let verifier = hasher
            .hash(&Secret::new(password.clone()))
            .expect("verificador");
        sqlx::query(
            // A expiração não é decoração: a base recusa uma credencial
            // temporária sem ela, porque uma temporária que não expira é uma
            // palavra-passe permanente com a etiqueta errada.
            "INSERT INTO credentials (person_id, kind, state, verifier, issued_reason, expires_at)
                 VALUES ($1, $2, 'active', $3, 'primeiro acesso do harness', now() + interval '1 hour')",
        )
        .bind(person_id)
        .bind(CredentialKind::Temporary.as_str())
        .bind(&verifier)
        .execute(&self.pool)
        .await
        .expect("credencial temporária");

        let credenciais = Credenciais {
            username: handle,
            password,
        };
        self.login_as(&credenciais).await;
        credenciais
    }

    /// Volta a entrar com credenciais já criadas.
    ///
    /// # Porque isto é preciso
    ///
    /// Os cookies são do browser, não da página: entrar como outra pessoa
    /// substitui a sessão. Um teste que crie um segundo actor e depois volte a
    /// observar como o primeiro **tem** de voltar a entrar — senão está a medir
    /// a agenda da pessoa errada, e passa ou falha por acaso.
    ///
    /// Descobri-o pela CI: localmente passava, e no runner não.
    async fn login_as(&self, credenciais: &Credenciais) {
        let page = self.open("/login").await;
        set_field(&page, "input[name=username]", &credenciais.username).await;
        set_field(&page, "input[name=password]", &credenciais.password).await;
        submit(&page, "form").await;

        let destino = wait_until_left(&page, "/login").await;
        assert!(
            !destino.ends_with("/login"),
            "não foi possível voltar a entrar como «{}»",
            credenciais.username
        );
    }

    /// Uma unidade que esta pessoa gere, para poder marcar fora do pessoal.
    /// Um Research Workspace onde esta pessoa pode acrescentar referências.
    ///
    /// Pertença ao **ambiente**, e não só à unidade: criar num ambiente de
    /// investigação depende dela, e a unidade só dá para ler o que é interno.
    async fn owns_a_workspace(&self, person_id: Uuid) -> Uuid {
        let unit_id = self.manages_a_unit(person_id).await;
        let organisation_id: Uuid =
            sqlx::query_scalar("SELECT organisation_id FROM people WHERE id = $1")
                .bind(person_id)
                .fetch_one(&self.pool)
                .await
                .expect("organização");

        let workspace_id: Uuid = sqlx::query_scalar(
            "INSERT INTO research_workspaces
                 (organisation_id, unit_id, code, title, kind, classification)
             VALUES ($1, $2, $3, 'Ambiente do harness', 'idea', 'INTERNAL') RETURNING id",
        )
        .bind(organisation_id)
        .bind(unit_id)
        .bind(format!("WSB{}", &Uuid::new_v4().simple().to_string()[..6]).to_uppercase())
        .fetch_one(&self.pool)
        .await
        .expect("ambiente");

        sqlx::query(
            "INSERT INTO workspace_memberships (workspace_id, person_id, role)
                 VALUES ($1, $2, 'lead')",
        )
        .bind(workspace_id)
        .bind(person_id)
        .execute(&self.pool)
        .await
        .expect("pertença ao ambiente");

        workspace_id
    }

    async fn manages_a_unit(&self, person_id: Uuid) -> Uuid {
        let organisation_id: Uuid =
            sqlx::query_scalar("SELECT organisation_id FROM people WHERE id = $1")
                .bind(person_id)
                .fetch_one(&self.pool)
                .await
                .expect("organização");

        let unit_id: Uuid = sqlx::query_scalar(
            "INSERT INTO units (organisation_id, code, name, created_by_id)
                 VALUES ($1, $2, $3, $4) RETURNING id",
        )
        .bind(organisation_id)
        .bind(format!("E{}", &Uuid::new_v4().simple().to_string()[..5]))
        .bind("Unidade do harness")
        .bind(person_id)
        .fetch_one(&self.pool)
        .await
        .expect("unidade");

        sqlx::query("INSERT INTO unit_memberships (unit_id, person_id, role) VALUES ($1, $2, $3)")
            .bind(unit_id)
            .bind(person_id)
            .bind(UnitRole::Manager.as_str())
            .execute(&self.pool)
            .await
            .expect("pertença");

        unit_id
    }

    /// Marca um evento pela interface, e devolve o título e o identificador.
    ///
    /// É o caminho que uma pessoa percorre, e não uma chamada à API: as viagens
    /// seguintes precisam de eventos que existam **como a interface os cria**,
    /// senão estariam a medir dados que só o teste sabe produzir.
    async fn create_event_via_ui(&self, titulo: &str, dia: chrono::NaiveDate, hora: u32) -> String {
        let formulario = self.open("/calendar/events/new").await;
        set_field(&formulario, "input[name=title]", titulo).await;
        set_field(
            &formulario,
            "input[name=starts_at]",
            &format!("{dia}T{hora:02}:00"),
        )
        .await;
        set_field(
            &formulario,
            "input[name=ends_at]",
            &format!("{dia}T{:02}:00", hora + 1),
        )
        .await;
        set_field(&formulario, "input[name=timezone]", "Europe/Lisbon").await;
        submit(&formulario, "form.oc-editor__form").await;

        let destino = wait_until_left(&formulario, "/calendar/events/new").await;
        assert!(
            destino.contains("/calendar/events/"),
            "a marcação não levou ao detalhe: {destino}"
        );
        destino
            .rsplit('/')
            .next()
            .expect("identificador")
            .to_owned()
    }

    /// Marca um evento de dia inteiro pela interface.
    async fn create_all_day_via_ui(&self, titulo: &str, dia: chrono::NaiveDate) -> String {
        let formulario = self.open("/calendar/events/new").await;
        set_field(&formulario, "input[name=title]", titulo).await;

        // A caixa muda o que o formulário envia. Marca-se como uma pessoa a
        // marcaria, e não escrevendo directamente nos campos escondidos.
        elemento(&formulario, "input[name=all_day]")
            .await
            .click()
            .await
            .expect("marcar");

        // O último dia é **inclusivo** para quem escreve: um evento de um dia é
        // 24 → 24. A base guarda 24 → 25, e essa conversão não é da pessoa.
        set_field(&formulario, "input[name=starts_on]", &dia.to_string()).await;
        set_field(&formulario, "input[name=ends_on]", &dia.to_string()).await;
        submit(&formulario, "form.oc-editor__form").await;

        let destino = wait_until_left(&formulario, "/calendar/events/new").await;
        assert!(
            destino.contains("/calendar/events/"),
            "a marcação de dia inteiro não levou ao detalhe: {destino}"
        );
        destino
            .rsplit('/')
            .next()
            .expect("identificador")
            .to_owned()
    }

    /// Outra pessoa, com um evento pessoal seu.
    ///
    /// Entra, marca, e sai — deixando o browser com a sessão da segunda pessoa.
    /// Quem chamar isto deve reentrar depois, se precisar da primeira.
    async fn sign_in_other(&self, titulo: &str, dia: chrono::NaiveDate) -> (Uuid, String) {
        let (outra, _) = self.sign_in(&[TechnicalRole::ResearchMember]).await;
        let id = self.create_event_via_ui(titulo, dia, 16).await;
        (outra, id)
    }

    /// O mesmo, mas devolvendo o browser à sessão de quem estava antes.
    async fn other_person_event(
        &self,
        titulo: &str,
        dia: chrono::NaiveDate,
        voltar_a: &Credenciais,
    ) -> Uuid {
        let (outra, _) = self.sign_in_other(titulo, dia).await;
        self.login_as(voltar_a).await;
        outra
    }

    /// Um Research Workspace desta pessoa, para as tarefas terem onde viver.
    async fn a_workspace(&self, person_id: Uuid, unit_id: Uuid) -> Uuid {
        // `code` é obrigatório e único por organização. Ficou de fora deste
        // INSERT desde o início e nunca ninguém reparou, porque o teste que o
        // chama saltava-se em silêncio quando o Chrome não arrancava.
        let workspace_id: Uuid = sqlx::query_scalar(
            "INSERT INTO research_workspaces
                 (organisation_id, unit_id, code, kind, title, classification, created_by_id)
                 SELECT organisation_id, $1, $3, 'idea', 'Ambiente do harness', 'INTERNAL', $2
                   FROM people WHERE id = $2
             RETURNING id",
        )
        .bind(unit_id)
        .bind(person_id)
        .bind(format!("HARNESS-{}", Uuid::new_v4().simple()))
        .fetch_one(&self.pool)
        .await
        .expect("workspace");

        sqlx::query(
            "INSERT INTO workspace_memberships (workspace_id, person_id, role)
                 VALUES ($1, $2, 'lead')",
        )
        .bind(workspace_id)
        .bind(person_id)
        .execute(&self.pool)
        .await
        .expect("pertença ao workspace");

        workspace_id
    }

    /// Derruba o Core, deixando o Workspace de pé.
    ///
    /// # Porque isto é a forma certa de provar «erro ≠ vazio»
    ///
    /// Porque a sessão do Workspace é sua e vive na memória dele: a página
    /// continua a renderizar-se, e o que falha é a consulta ao Core. É
    /// exactamente o que acontece quando o núcleo cai a meio de uma tarde.
    fn stop_core(&self) {
        self.core_task.abort();
    }

    /// Abre uma página como uma pessoa a abre — passando pelo arranque.
    ///
    /// # Porque é que isto espera por estado e não por navegação
    ///
    /// `wait_for_navigation` espera pela navegação **seguinte**, e a seguinte é
    /// a entrega do arranque. Quem a chamasse e lesse o conteúdo a seguir
    /// apanharia o contexto a ser destruído a meio — «Cannot find context with
    /// specified id», que se lê como avaria do browser e é uma corrida.
    ///
    /// O que aqui se espera é o estado observável: já não estar no arranque.
    async fn open(&self, path: &str) -> Page {
        let page = self
            .browser
            .new_page(format!("{}{path}", self.workspace_url))
            .await
            .expect("página");

        let inicio = std::time::Instant::now();
        loop {
            let url = page.url().await.ok().flatten().unwrap_or_default();
            if !url.contains("/boot") && !url.is_empty() && url != "about:blank" {
                return page;
            }
            assert!(
                inicio.elapsed() < std::time::Duration::from_secs(45),
                "o arranque não entregou em vinte e cinco segundos; ficou em «{url}»"
            );
            tokio::time::sleep(std::time::Duration::from_millis(120)).await;
        }
    }
}

/// Espera que um texto apareça na página.
async fn esperar_por(page: &Page, agulha: &str) {
    let inicio = std::time::Instant::now();
    loop {
        let html = conteudo_estavel(page).await;
        if html.contains(agulha) {
            return;
        }
        assert!(
            inicio.elapsed() < DEADLINE,
            "«{agulha}» não apareceu em {DEADLINE:?}: {}",
            &html[..html.len().min(400)]
        );
        tokio::time::sleep(Duration::from_millis(120)).await;
    }
}

/// Escolhe um valor num `select`, como uma pessoa faria.
async fn escolher(page: &Page, seletor: &str, valor: &str) {
    let script = format!(
        "(() => {{ const campo = document.querySelector('{seletor}'); \
          if (!campo) return null; \
          campo.value = '{valor}'; \
          campo.dispatchEvent(new Event('change', {{ bubbles: true }})); \
          return campo.value; }})()"
    );
    let escolhido: Option<String> = page
        .evaluate(script)
        .await
        .expect("escolher")
        .into_value()
        .ok();
    assert_eq!(
        escolhido.as_deref(),
        Some(valor),
        "«{seletor}» não aceitou «{valor}»: o formulário mudou de forma"
    );
}

/// O que está entre duas marcas, para inspeccionar um campo do documento.
fn entre<'a>(html: &'a str, depois: &str, antes: &str) -> &'a str {
    let Some(inicio) = html.find(depois) else {
        return "";
    };
    let resto = &html[inicio + depois.len()..];
    let Some(abre) = resto.find('>') else {
        return "";
    };
    let resto = &resto[abre + 1..];
    resto.find(antes).map_or(resto, |fim| &resto[..fim])
}

/// Quantas viagens conduzem um browser ao mesmo tempo.
fn concorrencia() -> &'static Arc<tokio::sync::Semaphore> {
    use std::sync::OnceLock;
    static LUGARES: OnceLock<Arc<tokio::sync::Semaphore>> = OnceLock::new();
    LUGARES.get_or_init(|| {
        // Dois, e não quantos os núcleos.
        //
        // Cada viagem conduz um Chrome inteiro, e um Chrome inteiro custa
        // memória, não CPU. Medi-o: com quatro em paralelo esta máquina fica
        // com 1,3 GB livres e o processo de teste aborta — `SIGABRT`, sem uma
        // linha de pânico, porque quem morre é o browser e não o teste.
        //
        // Dois é o que cabe na máquina mais apertada onde isto corre. Numa
        // máquina folgada custa alguns segundos; numa apertada é a diferença
        // entre uma suite que prova alguma coisa e uma que aborta a meio e
        // deixa Chromes para trás.
        Arc::new(tokio::sync::Semaphore::new(2))
    })
}

/// O Capability Runtime desta execução, partilhado por todas as viagens.
fn capacidades() -> &'static std::sync::Arc<ocinye_core::capabilities::Capabilities> {
    use std::sync::OnceLock;
    static UM: OnceLock<std::sync::Arc<ocinye_core::capabilities::Capabilities>> = OnceLock::new();
    UM.get_or_init(|| {
        let directorio = format!(
            "{}/../../target/wasm32-wasip1/release",
            env!("CARGO_MANIFEST_DIR")
        );
        let carregadas = ocinye_core::capabilities::Capabilities::load(&directorio)
            .expect("motor de capacidades");

        // Falha aqui, e não vinte e cinco segundos depois.
        //
        // Sem esta linha, um componente por construir manifesta-se como uma
        // viagem que espera por um resultado que nunca chega, e a mensagem fala
        // de um texto que não apareceu na página. A causa é outra, e dizê-la
        // aqui poupa a quem vier a seguir o caminho que eu fiz.
        assert!(
            carregadas.has(ocinye_core::capabilities::Component::BibtexImport),
            "o componente WebAssembly não está construído em «{directorio}».\n\
             Corra: ./scripts/build-capabilities.sh"
        );
        std::sync::Arc::new(carregadas)
    })
}

fn core_state(pool: PgPool, organisation_id: Uuid, database_url: &str) -> AppState {
    use std::sync::Once;
    static ONCE: Once = Once::new();
    ONCE.call_once(|| std::env::set_var("OCINYE_DATABASE_URL", database_url));

    let config = CoreConfig::from_env().expect("configuração do Core");
    let verifier =
        ocinye_core::authn::TokenVerifier::new(config.oidc.clone()).expect("verificador");
    let authenticator = Arc::new(Authenticator::new(
        Hasher::new(HashingParams {
            memory_kib: config.auth.argon2_memory_kib,
            iterations: config.auth.argon2_iterations,
            parallelism: config.auth.argon2_parallelism,
        }),
        Throttle {
            per_ip: config.auth.throttle_per_ip,
            per_username: config.auth.throttle_per_username,
            window_minutes: config.auth.throttle_window_minutes,
        },
        config.auth.temporary_credential_hours,
    ));

    AppState {
        pool,
        config: Arc::new(config),
        verifier,
        authenticator,
        store: None,
        inference: Arc::new(ocinye_core::modules::intelligence::NoProvider),
        mail_provider: Arc::new(UnconfiguredProvider),
        // O Capability Runtime com o componente a sério, e um só por processo.
        //
        // Um harness que carregasse um conjunto vazio provaria que a interface
        // mostra um erro, e não que a bibliografia atravessa o isolamento.
        //
        // E um por harness não servia: cada motor `wasmtime` traz consigo uma
        // thread que faz avançar a época, e trinta e oito motores ao lado de
        // trinta e oito Chromes abortaram o processo de teste — duas vezes em
        // três, com `SIGABRT` e sem uma linha a explicar porquê. O Core tem um
        // no seu estado; aqui é a mesma coisa.
        capabilities: std::sync::Arc::clone(capacidades()),
        organisation_id,
    }
}

/// A configuração do Workspace, construída à mão.
///
/// # Porque não `from_env`
///
/// Porque as variáveis de ambiente são globais ao processo, e os testes correm
/// em paralelo. Dois harnesses a escreverem `OCINYE_CORE_URL` faziam um deles
/// apontar ao Core do outro — e o sintoma era o botão de entrar desactivado,
/// porque a sonda de prontidão batia num porto que já não era daquele Core.
///
/// Descobri isto assim: o formulário não submetia, e não havia mensagem de erro
/// nenhuma para explicar porquê.
fn workspace_state(core_url: &str, public_url: &str, estaticos: &str) -> WorkspaceState {
    let config = WorkspaceConfig {
        bind_address: "127.0.0.1:0".to_owned(),
        public_url: public_url.to_owned(),
        core_url: core_url.to_owned(),
        session_ttl: Duration::from_secs(3600),
        cookie_secure: false,
        log_level: "warn".to_owned(),
        log_format: "pretty".to_owned(),
        is_production: false,
        // Normalmente `…/static` do crate; a comparação com o estado anterior
        // passa outro directório, para não mexer no que os testes concorrentes
        // estão a ler.
        static_dir: estaticos.to_owned(),
    };
    WorkspaceState {
        config: Arc::new(config),
        sessions: SessionStore::new(),
        http: reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .expect("cliente"),
    }
}

// ── Ajudantes ───────────────────────────────────────────────────────────
//
// Cada um destes existe porque um teste tropeçou no problema que ele resolve.
// Ficam num sítio só para que o tropeção não se repita por cópia.

/// Quanto tempo se espera por um estado antes de desistir.
const DEADLINE: Duration = Duration::from_secs(6);

/// Espera que o endereço deixe de ser o que era.
///
/// # Porque não `wait_for_navigation`
///
/// Porque um POST navega uma vez e a resposta redirecciona outra.
/// `wait_for_navigation` devolve na primeira, e ler o endereço nesse instante dá
/// sempre a página de onde se veio.
async fn wait_until_left(page: &Page, path: &str) -> String {
    let inicio = std::time::Instant::now();
    loop {
        let actual = page.url().await.ok().flatten().unwrap_or_default();
        if !actual.ends_with(path) {
            return actual;
        }
        assert!(
            inicio.elapsed() < DEADLINE,
            "continuámos em «{path}» ao fim de {DEADLINE:?}"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

/// Espera que um elemento exista e esteja visível.
///
/// O relógio, por exemplo, nasce `hidden` até o JS lhe escrever a hora — mostrar
/// um relógio vazio seria mostrar uma hora que não sabemos.
async fn wait_visible(page: &Page, seletor: &str) -> chromiumoxide::Element {
    let inicio = std::time::Instant::now();
    loop {
        if let Ok(elemento) = page.find_element(seletor).await {
            if elemento.attribute("hidden").await.ok().flatten().is_none() {
                return elemento;
            }
        }
        assert!(
            inicio.elapsed() < DEADLINE,
            "«{seletor}» nunca ficou visível em {DEADLINE:?}"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

/// Escreve num campo pelo mecanismo que funciona para ele.
///
/// # Porque nem tudo se escreve por teclas
///
/// O CDP escreve carácter a carácter, e há duas coisas que isso não faz:
/// acentos, porque não há tecla «ã»; e `datetime-local`, que é um widget
/// composto e não um campo de texto. Para esses põe-se o valor e dispara-se o
/// evento, que é o que um selector de data faria.
/// Encontra um elemento, tolerando que o documento acabe de ser substituído.
///
/// # A corrida
///
/// `find_element` pede o documento ao Chrome e depois pergunta-lhe pelo
/// selector. São dois pedidos, e entre eles pode entrar a confirmação de uma
/// navegação — a entrega do arranque, por exemplo. Quando isso acontece, o
/// identificador de nó que a primeira metade obteve já não pertence a nada, e o
/// Chrome responde «Could not find node with given id».
///
/// Isso não é o produto a falhar: é quem observa a chegar meio instante cedo
/// de mais. Apanhou-me na CI, onde a máquina é mais lenta e a janela é maior —
/// duas viagens que localmente passavam sempre.
///
/// Continua a falhar se o elemento **não existir** — verificado por reversão,
/// com um selector inventado: falha em vinte segundos e diz o que procurava.
///
/// O Chrome usa a mesma mensagem para as duas situações, e por isso esta espera
/// não distingue «o documento mudou» de «não há nada assim». Não precisa: ao fim
/// do limite, as duas são falhas.
async fn elemento(page: &Page, seletor: &str) -> chromiumoxide::element::Element {
    let inicio = std::time::Instant::now();
    let mut ultimo = None;
    while inicio.elapsed() < Duration::from_secs(20) {
        match page.find_element(seletor).await {
            Ok(elemento) => return elemento,
            Err(erro) => ultimo = Some(erro),
        }
        tokio::time::sleep(Duration::from_millis(120)).await;
    }
    panic!("«{seletor}» não apareceu: {ultimo:?}");
}

async fn set_field(page: &Page, seletor: &str, valor: &str) {
    // Um valor vai para dentro de um literal de JavaScript, e portanto tudo o
    // que fecha um literal tem de ser escapado — não só a plica.
    //
    // A barra primeiro, senão escapa-se o que se acabou de escapar. E a nova
    // linha porque um literal de JavaScript não a atravessa: o BibTeX que estas
    // viagens colam tem várias, e o Chrome respondia «SyntaxError: invalid or
    // unexpected token» sem dizer que a culpa era do teste.
    let escapado = valor
        .replace('\\', "\\\\")
        .replace('\'', "\\'")
        .replace('\n', "\\n")
        .replace('\r', "\\r");
    let script = format!(
        "(() => {{ const campo = document.querySelector('{seletor}'); \
          if (!campo) return null; \
          campo.value = '{escapado}'; \
          campo.dispatchEvent(new Event('input', {{ bubbles: true }})); \
          campo.dispatchEvent(new Event('change', {{ bubbles: true }})); \
          return campo.value; }})()"
    );
    let escrito: Option<String> = page
        .evaluate(script)
        .await
        .expect("preencher")
        .into_value()
        .ok();
    assert_eq!(
        escrito.as_deref(),
        Some(valor),
        "«{seletor}» não aceitou o valor: o formulário mudou de forma"
    );
}

/// Submete o formulário da página.
///
/// O selector é o do formulário, e não `button[type=submit]` solto: a shell tem
/// outros botões de submissão, e o primeiro da página pode estar escondido.
/// Carrega num elemento, como uma pessoa faria.
async fn clicar(page: &Page, seletor: &str) {
    elemento(page, seletor).await.click().await.expect("clicar");
}

async fn submit(page: &Page, formulario: &str) {
    elemento(page, &format!("{formulario} button[type=submit]"))
        .await
        .click()
        .await
        .expect("submeter");
}

/// Abre o Centro Temporal pelo relógio, como uma pessoa faria.
async fn open_temporal_centre(page: &Page) {
    let relogio = wait_visible(page, "[data-oc=\"clock\"]").await;
    assert_eq!(
        relogio
            .attribute("aria-expanded")
            .await
            .ok()
            .flatten()
            .as_deref(),
        Some("false"),
        "o relógio não declara que está fechado"
    );
    relogio.click().await.expect("clicar no relógio");

    let inicio = std::time::Instant::now();
    loop {
        let estado = elemento(page, "[data-oc=\"clock\"]")
            .await
            .attribute("aria-expanded")
            .await
            .ok()
            .flatten();
        if estado.as_deref() == Some("true") {
            return;
        }
        assert!(
            inicio.elapsed() < DEADLINE,
            "clicar no relógio não abriu o Centro Temporal"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

/// Um título que não pode colidir com o de outra corrida.
fn unique_title(prefixo: &str) -> String {
    format!("{prefixo} {}", Uuid::new_v4().simple())
}

// ── A primeira prova ────────────────────────────────────────────────────

/// O Workspace levanta-se e serve a página de entrada a um browser real.
///
/// # Porque este teste existe antes dos outros
///
/// Porque prova o harness. Se ele falhar, tudo o que vier a seguir estaria a
/// medir o harness em vez de medir o produto — e não haveria maneira de
/// distinguir as duas coisas a partir do resultado.
#[tokio::test]
async fn o_workspace_serve_um_browser_a_serio() {
    let harness = harness!();

    let page = harness.open("/login").await;
    let html = page.content().await.expect("conteúdo");

    assert!(
        html.contains("Sessão institucional"),
        "a página de entrada não chegou ao browser"
    );
    assert!(
        html.contains("OCINYE OS"),
        "o browser recebeu HTML que não é o do Workspace"
    );
}

/// Uma pessoa marca um compromisso, do relógio até ao PostgreSQL.
///
/// # A viagem
///
/// ```text
/// entrar → clicar no relógio → Centro Temporal → Abrir Calendário
///        → Nova actividade → preencher → submeter
///        → Core → PostgreSQL → recarregar → o evento continua lá
/// ```
///
/// # Duas provas da mesma coisa
///
/// A interface mostrar o evento não chega: um render optimista mostra o mesmo.
/// Por isso o teste pergunta também ao PostgreSQL, e exige **exactamente uma**
/// linha com aquele título. O título é único por corrida, para nunca passarmos
/// por causa de dados que ficaram de outra.
#[tokio::test]
async fn uma_pessoa_marca_um_compromisso_de_ponta_a_ponta() {
    let harness = harness!();
    let inicio = std::time::Instant::now();

    let (person_id, _) = harness.sign_in(&[TechnicalRole::ResearchMember]).await;
    harness.manages_a_unit(person_id).await;
    let entrada = inicio.elapsed();

    // ── O relógio é um controlo ─────────────────────────────────────────
    let page = harness.open("/").await;
    let antes_do_clique = std::time::Instant::now();
    open_temporal_centre(&page).await;
    let abertura = antes_do_clique.elapsed();

    let html = page.content().await.expect("conteúdo");
    assert!(
        html.contains("Abrir Calendário"),
        "o Centro Temporal abriu sem a acção que leva ao Calendário"
    );

    // ── O Calendário ────────────────────────────────────────────────────
    let calendario = harness.open("/calendar").await;
    let html = calendario.content().await.expect("conteúdo");
    assert!(html.contains("Calendário"), "o Calendário não respondeu");
    assert!(
        !html.contains("Não foi possível ler a agenda"),
        "a agenda falhou a carregar"
    );

    // ── Marcar ──────────────────────────────────────────────────────────
    //
    // Sem acentos no título: o CDP escreve por eventos de tecla. O que este
    // percurso mede é o caminho vertical, não a codificação de caracteres.
    let titulo = unique_title("Reuniao");
    let dia = (chrono::Utc::now() + chrono::Duration::days(2)).date_naive();

    let antes_de_submeter = std::time::Instant::now();
    let event_id = harness.create_event_via_ui(&titulo, dia, 10).await;
    let submissao = antes_de_submeter.elapsed();

    // ── A segunda prova: o PostgreSQL ───────────────────────────────────
    let (quantos, dono, zona): (i64, Option<Uuid>, Option<String>) = sqlx::query_as(
        "SELECT COUNT(*) OVER (), owner_id, timezone FROM calendar_events WHERE title = $1",
    )
    .bind(&titulo)
    .fetch_one(&harness.pool)
    .await
    .expect("o evento não chegou ao PostgreSQL");

    assert_eq!(quantos, 1, "o evento foi escrito mais do que uma vez");
    assert_eq!(dono, Some(person_id), "o dono não é quem marcou");
    assert_eq!(
        zona.as_deref(),
        Some("Europe/Lisbon"),
        "a zona da intenção não ficou guardada"
    );

    // ── E continua lá depois de recarregar ──────────────────────────────
    //
    // Isto separa estado local de estado institucional: uma interface que
    // guardasse o evento em memória mostrava-o na mesma até alguém carregar F5.
    let antes_do_reload = std::time::Instant::now();
    let outra_vez = harness.open(&format!("/calendar/events/{event_id}")).await;
    let html = outra_vez.content().await.expect("conteúdo");
    let reaparecer = antes_do_reload.elapsed();

    assert!(
        html.contains(&titulo),
        "o evento desapareceu depois de recarregar: era estado local"
    );

    eprintln!(
        "tempos · entrada {entrada:?} · relogio {abertura:?} · \
         submeter {submissao:?} · reabrir {reaparecer:?}"
    );
}

// ── As cinco superfícies ────────────────────────────────────────────────

/// O mesmo evento aparece nas cinco superfícies, e o alheio em nenhuma.
///
/// # A propriedade
///
/// > **A apresentação de um item temporal pode diferir entre o Centro Temporal,
/// > Hoje, Semana, Mês e Agenda; a sua autorização não.**
///
/// O universo é heterogéneo de propósito: um evento dentro do intervalo, um
/// fora, e um de outra pessoa. Se alguma superfície escrevesse a sua própria
/// consulta, discordaria numa destas três — e o defeito apareceria como um
/// evento que está numa vista e não está na outra.
#[tokio::test]
async fn as_vistas_partilham_o_universo_autorizado() {
    let harness = harness!();
    let (person_id, credenciais) = harness.sign_in(&[TechnicalRole::ResearchMember]).await;
    harness.manages_a_unit(person_id).await;

    // O dia e a hora vêm do relógio de Lisboa, que é o fuso que o formulário
    // escreve — e não de UTC.
    //
    // Duas vezes este teste falhou por causa disto, e as duas foram
    // determinísticas com a hora do dia como entrada escondida. Primeiro com
    // uma hora fixa: um evento às 10:00 sai da janela do Centro Temporal
    // (`agora - 12h .. agora + 14 dias`) assim que passam as 22:00. Depois com
    // a hora de UTC: à meia-noite UTC, `hora = 0` escrito em Lisboa é 23:00 do
    // dia anterior, e o evento acabava de terminar quando o teste o procurava.
    //
    // Com o relógio do fuso que o formulário usa, o evento começa na hora
    // corrente **daquele** relógio e ainda está a decorrer quando se observa —
    // seja qual for a hora a que a suite corra.
    use chrono::Timelike;
    let lisboa: chrono::NaiveDateTime = ocinye_contracts::temporal::in_zone(
        chrono::Utc::now(),
        "Europe/Lisbon"
            .to_owned()
            .try_into()
            .expect("fuso conhecido"),
    );
    let hoje = lisboa.date();
    // O fim é uma hora depois, e não existe hora 24.
    let hora = lisboa.hour().min(22);

    let visivel = unique_title("Visivel");
    let distante = unique_title("Distante");

    harness.create_event_via_ui(&visivel, hoje, hora).await;
    // Fora de qualquer intervalo natural: nem hoje, nem esta semana, nem este
    // mês, nem os próximos noventa dias.
    harness
        .create_event_via_ui(&distante, hoje + chrono::Duration::days(200), hora)
        .await;

    // De outra pessoa, e pessoal: não deve aparecer em superfície nenhuma.
    //
    // A segunda entrada substitui a sessão no browser, portanto volta-se à
    // primeira antes de observar — senão estaríamos a ver a agenda da pessoa
    // errada, e o teste passaria ou falharia por acaso.
    let alheio = unique_title("Alheio");
    harness
        .other_person_event(&alheio, hoje, &credenciais)
        .await;

    // ── As vistas do Calendário ─────────────────────────────────────────
    //
    // # Porque o calendário da barra deixou de estar nesta lista
    //
    // Estava, e mostrava os compromissos de hoje. Passou a ser apresentação
    // pura — data, mês corrente, dia de hoje, e uma porta para o Calendário —
    // por decisão institucional: um painel da barra que lê a agenda passa a ter
    // uma segunda opinião sobre o que a pessoa tem marcado, e duas superfícies
    // a responder à mesma pergunta acabam por discordar.
    //
    // A propriedade que este teste guarda — todas as superfícies mostram o
    // mesmo universo autorizado, e nenhuma mostra o que é de outra pessoa —
    // continua inteira. O que mudou foi quantas superfícies respondem à
    // pergunta, não o que respondem.
    //
    // O calendário da barra tem o seu próprio teste, e o que ele prova é o
    // contrário: que não mostra actividade nenhuma.
    let mut superficies: Vec<(&str, String)> = Vec::new();
    for vista in ["day", "week", "month", "agenda"] {
        let page = harness
            .open(&format!("/calendar?view={vista}&on={hoje}"))
            .await;
        superficies.push((vista, page.content().await.expect("conteúdo")));
    }

    for (nome, html) in &superficies {
        assert!(
            !html.contains("Não foi possível ler a agenda"),
            "«{nome}» falhou a carregar"
        );
        assert!(
            html.contains(&visivel),
            "«{nome}» não mostra um evento de hoje que o actor pode ver"
        );
        assert!(
            !html.contains(&alheio),
            "«{nome}» mostra o evento pessoal de outra pessoa"
        );
    }

    // O evento distante fica fora dos intervalos naturais de todas elas. O
    // Centro Temporal olha catorze dias; a agenda, noventa.
    for (nome, html) in &superficies {
        assert!(
            !html.contains(&distante),
            "«{nome}» mostra um evento a duzentos dias de distância: o intervalo \
             não está a ser respeitado"
        );
    }
}

/// Nenhuma vista escreve a sua própria consulta.
///
/// # Porque isto é um teste de código e não de comportamento
///
/// Porque o comportamento pode coincidir por acaso durante meses. O que este
/// teste protege é a **arquitectura de consumo**: as quatro vistas recebem uma
/// lista de itens que já vem decidida, e não têm forma de perguntar mais nada.
///
/// Se um dia alguém acrescentar uma consulta dentro de uma vista, isto falha —
/// que é muito antes de a divergência aparecer a alguém.
/// Nenhuma observação do browser procura um elemento sem tolerar a transição.
///
/// # A classe que isto fecha
///
/// > **Browser observations that need to be atomic must not be implemented as
/// > independent observations separated by an uncontrolled UI transition.**
///
/// `find_element` são dois pedidos ao Chrome: pede o documento, e depois
/// pergunta-lhe por um selector. Entre os dois pode entrar a confirmação de uma
/// navegação — a entrega do arranque, por exemplo — e o identificador de nó que
/// a primeira metade obteve deixa de pertencer a alguma coisa.
///
/// Apanhou duas viagens na CI e nenhuma aqui, porque a janela é maior numa
/// máquina mais lenta. `elemento()` fecha-a esperando por estado observável.
///
/// Este teste impede que a chamada crua volte por distracção. Os sítios que
/// **toleram** a ausência — dentro de ciclos de sondagem, com `if let Ok` ou
/// `match` — continuam a poder usá-la: aí a falha é o resultado que se procura,
/// e não uma surpresa.
#[test]
fn nenhuma_observacao_procura_um_elemento_sem_tolerar_a_transicao() {
    let fonte = include_str!("browser.rs");

    let mut cruas = Vec::new();
    for (numero, linha) in fonte.lines().enumerate() {
        let sem_comentario = linha.split("//").next().unwrap_or(linha);
        // Sem o que está entre aspas: senão este teste acusa-se a si próprio,
        // que foi o que fez à primeira.
        let codigo: String = sem_comentario
            .split('"')
            .step_by(2)
            .collect::<Vec<_>>()
            .join(" ");
        if !codigo.contains(".find_element(") {
            continue;
        }
        let sem_comentario = codigo.as_str();
        // Tolerado: a chamada cujo `Result` é examinado no mesmo sítio.
        if sem_comentario.contains("if let Ok") || sem_comentario.contains("match ") {
            continue;
        }
        cruas.push(format!("linha {}: {}", numero + 1, sem_comentario.trim()));
    }

    assert!(
        cruas.is_empty(),
        "uma observação chama `find_element` directamente sem tolerar a falha. \
         Use `elemento(&page, seletor)`, que espera por estado observável em vez \
         de competir com a transição:\n{cruas:#?}"
    );
}

#[test]
fn nenhuma_vista_do_calendario_consulta_por_si() {
    let ecra = include_str!("../src/ui/screens/calendar.rs");

    for proibido in [
        "api::get",
        "api::post",
        "sqlx",
        "reqwest",
        "VisibilityFilter",
        "/api/v1/",
    ] {
        assert!(
            !ecra.contains(proibido),
            "o ecrã do calendário contém «{proibido}»: uma vista passou a \
             consultar por si, em vez de receber o universo já autorizado"
        );
    }

    // E o predicado de visibilidade continua a existir uma só vez.
    let repositorio =
        include_str!("../../../crates/ocinye-core/src/modules/calendar/repository.rs");
    assert_eq!(
        repositorio.matches("fn visible(").count(),
        1,
        "há mais do que uma definição de visibilidade no calendário"
    );
    assert_eq!(
        repositorio.matches("fn intersects(").count(),
        1,
        "há mais do que uma definição de sobreposição temporal"
    );
}

// ── Alterar e cancelar ──────────────────────────────────────────────────

/// Alterar persiste, e não mexe na autoridade.
#[tokio::test]
async fn alterar_um_evento_pelo_browser_persiste_sem_mexer_na_autoridade() {
    let harness = harness!();
    let (person_id, _) = harness.sign_in(&[TechnicalRole::ResearchMember]).await;
    harness.manages_a_unit(person_id).await;

    let titulo = unique_title("Antes");
    let dia = (chrono::Utc::now() + chrono::Duration::days(3)).date_naive();
    let event_id = harness.create_event_via_ui(&titulo, dia, 9).await;

    let antes: (String, Option<Uuid>, Option<Uuid>, Option<Uuid>, String) = sqlx::query_as(
        "SELECT scope, owner_id, unit_id, workspace_id, classification
           FROM calendar_events WHERE title = $1",
    )
    .bind(&titulo)
    .fetch_one(&harness.pool)
    .await
    .expect("antes");

    let novo_titulo = unique_title("Depois");
    let edicao = harness
        .open(&format!("/calendar/events/{event_id}/edit"))
        .await;
    set_field(&edicao, "input[name=title]", &novo_titulo).await;
    set_field(&edicao, "input[name=starts_at]", &format!("{dia}T15:00")).await;
    set_field(&edicao, "input[name=ends_at]", &format!("{dia}T16:00")).await;
    set_field(&edicao, "input[name=timezone]", "Europe/Lisbon").await;
    submit(&edicao, "form.oc-editor__form").await;
    wait_until_left(&edicao, "/edit").await;

    // O PostgreSQL primeiro.
    let linha: (String, Option<Uuid>, Option<Uuid>, Option<Uuid>, String) = sqlx::query_as(
        "SELECT scope, owner_id, unit_id, workspace_id, classification
           FROM calendar_events WHERE id = $1",
    )
    .bind(Uuid::parse_str(&event_id).expect("identificador"))
    .fetch_one(&harness.pool)
    .await
    .expect("depois");
    let depois = linha;

    let hora: Option<chrono::DateTime<chrono::Utc>> =
        sqlx::query_scalar("SELECT starts_at FROM calendar_events WHERE id = $1")
            .bind(Uuid::parse_str(&event_id).expect("identificador"))
            .fetch_one(&harness.pool)
            .await
            .expect("hora");

    assert_eq!(
        antes, depois,
        "uma alteração pela interface mexeu em âmbito, dono, contentor ou classificação"
    );
    assert!(
        hora.is_some_and(|i| i.format("%H:%M").to_string() != "09:00"),
        "a hora não mudou: a alteração não fez nada"
    );

    // E a interface mostra-o, depois de recarregar.
    let detalhe = harness
        .open(&format!("/calendar/events/{event_id}"))
        .await
        .content()
        .await
        .expect("conteúdo");
    assert!(
        detalhe.contains(&novo_titulo),
        "o título novo não persistiu"
    );
    assert!(
        !detalhe.contains(&titulo),
        "o título antigo continua a aparecer"
    );

    // Só uma entidade, e é a mesma.
    let quantos: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM calendar_events WHERE id = $1")
        .bind(Uuid::parse_str(&event_id).expect("identificador"))
        .fetch_one(&harness.pool)
        .await
        .expect("contagem");
    assert_eq!(quantos, 1);
}

/// Cancelar transita, não apaga, e repetir não dói.
#[tokio::test]
async fn cancelar_pelo_browser_transita_sem_apagar() {
    let harness = harness!();
    let (person_id, _) = harness.sign_in(&[TechnicalRole::ResearchMember]).await;
    harness.manages_a_unit(person_id).await;

    let titulo = unique_title("A cancelar");
    let dia = (chrono::Utc::now() + chrono::Duration::days(4)).date_naive();
    let event_id = harness.create_event_via_ui(&titulo, dia, 11).await;
    let uuid = Uuid::parse_str(&event_id).expect("identificador");

    for tentativa in 1..=2 {
        let detalhe = harness.open(&format!("/calendar/events/{event_id}")).await;
        if tentativa == 1 {
            submit(&detalhe, "form[action$=\"/cancel\"]").await;

            // Espera-se pelo **estado**, e não pelo endereço: a página já estava
            // em `/calendar/events/{id}` e continua lá depois do cancelamento,
            // portanto esperar que o endereço mude devolveria de imediato — e o
            // teste seguia antes de a escrita ter acontecido.
            let inicio = std::time::Instant::now();
            loop {
                let estado: Option<String> =
                    sqlx::query_scalar("SELECT state FROM calendar_events WHERE id = $1")
                        .bind(uuid)
                        .fetch_optional(&harness.pool)
                        .await
                        .expect("estado");
                if estado.as_deref() == Some("cancelled") {
                    break;
                }
                assert!(
                    inicio.elapsed() < DEADLINE,
                    "o cancelamento não chegou ao PostgreSQL"
                );
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        } else {
            // À segunda o botão já não aparece — o evento está cancelado. A
            // operação continua idempotente pela API; a interface é que deixa
            // de a oferecer, que é o comportamento certo.
            let html = detalhe.content().await.expect("conteúdo");
            assert!(
                html.contains("foi cancelada"),
                "o detalhe não diz que a actividade está cancelada"
            );
            assert!(
                !html.contains("form[action$=\"/cancel\"]"),
                "o botão de cancelar continua a aparecer num evento cancelado"
            );
        }
    }

    let (quantos, estado): (i64, String) =
        sqlx::query_as("SELECT COUNT(*) OVER (), state FROM calendar_events WHERE id = $1")
            .bind(uuid)
            .fetch_one(&harness.pool)
            .await
            .expect("estado");

    assert_eq!(quantos, 1, "cancelar apagou a linha em vez de a transitar");
    assert_eq!(estado, "cancelled");

    // Continua visível na agenda: quem o esperava precisa de saber.
    let agenda = harness
        .open(&format!(
            "/calendar?view=agenda&on={}",
            chrono::Utc::now().date_naive()
        ))
        .await
        .content()
        .await
        .expect("conteúdo");
    assert!(
        agenda.contains(&titulo),
        "o evento cancelado desapareceu da agenda em vez de aparecer cancelado"
    );
}

// ── Dia inteiro ─────────────────────────────────────────────────────────

/// A pessoa escreve o último dia; a base guarda o dia seguinte.
///
/// # O que isto protege
///
/// O intervalo meio-aberto é a forma de guardar datas civis sem erros de um dia.
/// Não é forma de as contar a alguém: quem marca um evento a 24 de Agosto
/// escreve 24, e lê «Dia inteiro» — nunca «24 → 25».
#[tokio::test]
async fn um_evento_de_dia_inteiro_esconde_o_intervalo_meio_aberto() {
    let harness = harness!();
    let (person_id, _) = harness.sign_in(&[TechnicalRole::ResearchMember]).await;
    harness.manages_a_unit(person_id).await;

    let titulo = unique_title("Prazo");
    let dia = (chrono::Utc::now() + chrono::Duration::days(5)).date_naive();
    let event_id = harness.create_all_day_via_ui(&titulo, dia).await;

    let (all_day, inicio, fim): (bool, Option<chrono::NaiveDate>, Option<chrono::NaiveDate>) =
        sqlx::query_as("SELECT all_day, starts_on, ends_before FROM calendar_events WHERE id = $1")
            .bind(Uuid::parse_str(&event_id).expect("identificador"))
            .fetch_one(&harness.pool)
            .await
            .expect("evento");

    assert!(all_day, "o evento não ficou marcado como de dia inteiro");
    assert_eq!(inicio, Some(dia), "o primeiro dia não é o que foi escrito");
    assert_eq!(
        fim,
        dia.succ_opt(),
        "a base não guardou o dia seguinte: o intervalo deixou de ser meio-aberto"
    );

    // E a pessoa nunca vê o dia seguinte.
    let detalhe = harness
        .open(&format!("/calendar/events/{event_id}"))
        .await
        .content()
        .await
        .expect("conteúdo");
    assert!(
        detalhe.contains("Dia inteiro"),
        "o detalhe não diz que é de dia inteiro"
    );
    let dia_seguinte = dia.succ_opt().expect("dia seguinte").to_string();
    assert!(
        !detalhe.contains(&dia_seguinte),
        "o detalhe mostra o dia seguinte: a semântica de armazenamento vazou para \
         a interface"
    );
}

// ── Horário de Verão ────────────────────────────────────────────────────

/// Uma hora que não existe é explicada, e não rebenta.
#[tokio::test]
async fn uma_hora_que_nao_existe_e_explicada_a_pessoa() {
    let harness = harness!();
    let (person_id, _) = harness.sign_in(&[TechnicalRole::ResearchMember]).await;
    harness.manages_a_unit(person_id).await;

    let titulo = unique_title("Hora impossivel");
    let formulario = harness.open("/calendar/events/new").await;
    set_field(&formulario, "input[name=title]", &titulo).await;
    // 2026-03-29, 02:30 em Paris: o relógio salta essa hora.
    set_field(&formulario, "input[name=starts_at]", "2026-03-29T02:30").await;
    set_field(&formulario, "input[name=ends_at]", "2026-03-29T03:30").await;
    set_field(&formulario, "input[name=timezone]", "Europe/Paris").await;
    submit(&formulario, "form.oc-editor__form").await;

    // Fica no formulário, com a explicação.
    let inicio = std::time::Instant::now();
    let mut html = String::new();
    while inicio.elapsed() < DEADLINE {
        html = formulario.content().await.unwrap_or_default();
        if html.contains("oc-alert--error") {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    assert!(
        html.contains("oc-alert--error"),
        "uma hora inexistente não produziu erro visível"
    );
    assert!(
        html.contains("não existe"),
        "a mensagem não explica o que se passou"
    );
    // E continuamos no formulário, com os dados: uma recusa de validação
    // devolve a pessoa ao sítio onde pode corrigir. `500` como texto não serve
    // de sinal — aparece em qualquer folha de estilos como peso de tipo.
    assert!(
        html.contains("form") && html.contains("name=\"title\""),
        "a recusa levou a pessoa para fora do formulário, em vez de a deixar corrigir"
    );

    // E nada foi escrito. Um erro que criasse metade do evento seria pior do que
    // o erro.
    let quantos: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM calendar_events WHERE title = $1")
        .bind(&titulo)
        .fetch_one(&harness.pool)
        .await
        .expect("contagem");
    assert_eq!(quantos, 0, "uma marcação recusada deixou um evento escrito");
}

// ── Controlo visual da consolidação ─────────────────────────────────────
//
// Esta consolidação afirma uma coisa forte: **nada do que a pessoa vê mudou**.
//
// A prova mecânica está em `scripts/rendered_value_equivalence.py` — expandir
// os tokens novos devolve o mesmo CSS. Mas identidade de texto não é identidade
// de composição: um token pode valer o mesmo e estar no selector errado, e a
// cascata dava outro resultado sem que uma diferença de valores aparecesse.
//
// O que estes testes fazem é comparar o produto **renderizado** com o de antes,
// através do browser a sério. A única variável é a folha de estilos: a
// consolidação não tocou num único ficheiro de `apps/workspace/src/`, e o CSS é
// servido do disco a cada pedido. Trocar o ficheiro reproduz exactamente o
// estado anterior sem compilar uma segunda árvore.

/// A folha de estilos como estava antes da consolidação.
const CSS_BASE: &str = "075204e";

/// Um directório de estáticos com a folha de estilos de um commit.
///
/// Uma cópia dos estáticos actuais, com o `ocinye.css` substituído pelo de
/// então. Um directório próprio, e não o ficheiro do repositório: os outros
/// catorze testes estão a lê-lo ao mesmo tempo, e trocá-lo debaixo deles foi a
/// primeira tentativa — que os fez falhar por uma razão que nada tinha que ver
/// com o que eles provam.
fn estaticos_de(commit: &str) -> std::path::PathBuf {
    let origem = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("static");
    let destino = std::env::temp_dir().join(format!("ocinye-base-{}", Uuid::new_v4().simple()));
    std::fs::create_dir_all(&destino).expect("directório base");

    for entrada in std::fs::read_dir(&origem).expect("estáticos") {
        let entrada = entrada.expect("entrada");
        if entrada.path().is_file() {
            std::fs::copy(entrada.path(), destino.join(entrada.file_name())).expect("copiar");
        }
    }

    let anterior = std::process::Command::new("git")
        .args([
            "show",
            &format!("{commit}:apps/workspace/static/ocinye.css"),
        ])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("git show");
    assert!(
        anterior.status.success(),
        "o commit base {commit} não está disponível; isto seria uma comparação \
         não medida, e não uma comparação igual"
    );
    std::fs::write(destino.join("ocinye.css"), &anterior.stdout).expect("CSS base");
    destino
}

/// O estilo computado das propriedades que a consolidação migrou.
async fn estilo_computado(page: &Page, alvos: &[(&str, &str)]) -> String {
    let pedido = alvos
        .iter()
        .map(|(selector, propriedades)| format!("[{selector:?},{propriedades:?}]"))
        .collect::<Vec<_>>()
        .join(",");
    let guiao = format!(
        r#"(() => {{
             const alvos = [{pedido}];
             const linhas = [];
             for (const [selector, props] of alvos) {{
               const el = document.querySelector(selector);
               if (!el) {{ linhas.push(selector + ' AUSENTE'); continue; }}
               const s = getComputedStyle(el);
               const valores = props.split(',').map(p => p + '=' + s.getPropertyValue(p));
               linhas.push(selector + ' ' + valores.join(' '));
             }}
             return linhas.join('\n');
           }})()"#
    );
    page.evaluate(guiao.as_str())
        .await
        .expect("estilo computado")
        .into_value::<String>()
        .expect("texto")
}

/// Nada do que a pessoa vê mudou com a consolidação.
///
/// Um teste só, e não dois, por uma razão que custou uma execução vermelha a
/// perceber: a comparação troca o ficheiro de estilos em disco, e dois testes a
/// fazê-lo em paralelo lêem-se um ao outro pelo meio. Estado global mutável
/// partilhado entre testes concorrentes — que é exactamente o que este
/// repositório aprendeu a desconfiar.
///
/// Compara duas coisas que se completam:
///
/// **Estilo computado** das propriedades migradas — espaçamento, tipografia,
/// movimento, raio — em elementos concretos de várias superfícies.
///
/// **Camadas migradas**, pelo `z-index` computado de cada elemento. É aqui que
/// estava o risco: um token pode valer o mesmo e estar no selector errado, e só
/// o valor computado **no elemento certo** o revela.
///
/// # O que não prova, e porquê
///
/// Não prova ordem de empilhamento observada. Tentei, e a tentativa ensinou
/// mais do que teria ensinado o sucesso: com o menu de conta aberto, pôr
/// `--oc-z-dropdown` a zero não muda quem está por cima, porque o menu é
/// **descendente** da barra lateral. Entre ascendente e descendente o `z-index`
/// não decide nada; um filho pinta sobre o fundo do pai por ordem de documento.
///
/// Nesta configuração não há dois sobrepostos irmãos abertos ao mesmo tempo,
/// portanto não há competição real para observar. Fabricar uma — abrir à força
/// dois painéis que o produto nunca mostra juntos — provaria o cenário
/// inventado, e não o produto. Quando o Boot trouxer um sobreposto de ecrã
/// inteiro, passa a haver competição a sério, e é aí que este teste deve
/// crescer.
#[tokio::test]
async fn a_consolidacao_nao_mudou_o_que_a_pessoa_ve() {
    let harness = harness!();
    harness.sign_in(&[TechnicalRole::ResearchMember]).await;

    // Shell, navegação, conteúdo denso, botão e cartão. Cada linha cobre uma
    // das categorias migradas.
    const ALVOS: [(&str, &str); 7] = [
        (".oc-top", "height,background-color,z-index"),
        (".oc-side", "width,background-color,padding"),
        (".oc-side__nav", "padding,gap,font-size"),
        (".oc-main", "padding,font-family,font-size"),
        (
            "body",
            "font-family,font-size,line-height,color,background-color",
        ),
        (
            ".oc-btn",
            "padding,font-size,font-weight,border-radius,transition",
        ),
        (
            ".oc-card",
            "padding,border-radius,box-shadow,background-color",
        ),
    ];

    const PAGINAS: [&str; 3] = ["/", "/calendar", "/notifications"];

    async fn observa(harness: &Harness, alvos: &[(&str, &str)]) -> Vec<String> {
        let mut leituras = Vec::new();
        for caminho in PAGINAS {
            let page = harness.open(caminho).await;
            leituras.push(estilo_computado(&page, alvos).await);
            page.close().await.ok();
        }

        // As camadas, com o menu de conta aberto para existir no DOM.
        let page = harness.open("/").await;
        if let Ok(botao) = page.find_element("[data-oc=\"account-toggle\"]").await {
            let _ = botao.click().await;
        }
        leituras.push(
            page.evaluate(
                r#"(() => {
                     const migrados = ['.oc-skip', '.oc-account__menu',
                                       '.oc-create__menu', '.oc-palette',
                                       '.oc-login__bar'];
                     return migrados.map((s) => {
                       const e = document.querySelector(s);
                       if (!e) return s + ' ausente';
                       const c = getComputedStyle(e);
                       return s + ' z-index=' + c.zIndex + ' position=' + c.position;
                     }).join('\n');
                   })()"#,
            )
            .await
            .expect("camadas")
            .into_value::<String>()
            .expect("texto"),
        );
        page.close().await.ok();
        leituras
    }

    // Antes de comparar seja o que for: cada lado carregou mesmo a sua folha de
    // estilos?
    //
    // Servidor certo com asset errado daria uma comparação perfeita entre dois
    // nadas iguais. A impressão digital é o tamanho do ficheiro que o browser
    // recebeu de facto, pedido pelo próprio browser à sua origem.
    async fn impressao_do_css(harness: &Harness) -> usize {
        let page = harness.open("/").await;
        let tamanho = page
            .evaluate(
                r#"(async () => {
                     const r = await fetch('/static/ocinye.css', {cache: 'no-store'});
                     const t = await r.text();
                     return t.length;
                   })()"#,
            )
            .await
            .expect("folha de estilos servida")
            .into_value::<usize>()
            .expect("tamanho");
        page.close().await.ok();
        tamanho
    }

    let impressao_head = impressao_do_css(&harness).await;
    let depois = observa(&harness, &ALVOS).await;

    // O estado anterior corre no seu próprio Workspace, servindo a folha de
    // estilos de então a partir de um directório próprio. Nada no repositório é
    // tocado, e os outros testes continuam a ler o que sempre leram.
    let base = estaticos_de(CSS_BASE);
    let Ok(url) = std::env::var("OCINYE_TEST_DATABASE_URL") else {
        return;
    };
    let Some(chrome) = chrome_path() else { return };
    let Some(anterior) = Harness::start_com_estaticos(&url, &chrome, &base.to_string_lossy()).await
    else {
        panic!("o harness do estado anterior não levantou");
    };
    anterior.sign_in(&[TechnicalRole::ResearchMember]).await;
    let impressao_base = impressao_do_css(&anterior).await;
    let antes = observa(&anterior, &ALVOS).await;
    drop(anterior);
    let _ = std::fs::remove_dir_all(&base);

    // Os dois lados serviram folhas diferentes. Se fossem iguais, esta
    // comparação estaria a medir o mesmo ficheiro duas vezes — e passaria
    // sempre, sem observar nada.
    assert_ne!(
        impressao_base, impressao_head,
        "os dois servidores serviram a mesma folha de estilos ({impressao_base} \
         bytes); a comparação seria entre dois lados idênticos e não provaria \
         equivalência nenhuma"
    );
    assert!(
        impressao_base > 10_000 && impressao_head > 10_000,
        "uma das folhas de estilos veio vazia ou truncada: base={impressao_base}, \
         head={impressao_head}"
    );

    for (indice, leitura) in antes.iter().enumerate() {
        let onde = PAGINAS.get(indice).copied().unwrap_or("camadas migradas");
        assert_eq!(
            leitura, &depois[indice],
            "o que é renderizado mudou em {onde}"
        );
    }

    // Comparar ausências com ausências passaria sem provar nada.
    let ausentes =
        antes.join("\n").matches("AUSENTE").count() + antes.join("\n").matches(" ausente").count();
    assert!(
        ausentes <= 4,
        "{ausentes} alvos desapareceram; a comparação seria sobretudo entre \
         nadas: {antes:#?}"
    );
    let camadas_resolvidas = antes
        .last()
        .expect("as camadas")
        .lines()
        .filter(|l| l.contains("z-index=") && !l.contains("z-index=auto"))
        .count();
    assert!(
        camadas_resolvidas >= 3,
        "só {camadas_resolvidas} camadas resolveram para um valor: {:?}",
        antes.last()
    );
}

// ── Prazos de tarefas ───────────────────────────────────────────────────

/// Um prazo aparece no calendário sem virar evento.
#[tokio::test]
async fn um_prazo_de_tarefa_aparece_no_calendario_sem_duplicar() {
    let harness = harness!();
    let (person_id, _) = harness.sign_in(&[TechnicalRole::ResearchMember]).await;
    let unit_id = harness.manages_a_unit(person_id).await;
    let workspace_id = harness.a_workspace(person_id, unit_id).await;

    // Contado dentro desta organização, e não em toda a base.
    //
    // A versão anterior contava `calendar_events` inteiro. Cada teste desta
    // suite tem a sua organização, mas partilham a base de dados e correm em
    // paralelo — portanto a contagem global subia por causa do trabalho de
    // outro teste, e a asserção acusava esta tarefa de ter criado um evento que
    // nunca criou.
    //
    // Passou despercebido enquanto a suite era pequena. Cresceu, e a corrida
    // apareceu — o que é a forma habitual destas coisas se revelarem.
    let eventos_antes: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM calendar_events WHERE organisation_id = $1")
            .bind(harness.organisation_id)
            .fetch_one(&harness.pool)
            .await
            .expect("contagem");

    let titulo = unique_title("Entregar");
    let amanha = (chrono::Utc::now() + chrono::Duration::days(1)).date_naive();
    let task_id: Uuid = sqlx::query_scalar(
        "INSERT INTO tasks (organisation_id, unit_id, workspace_id, title, due_on,
                            classification, created_by_id)
             SELECT organisation_id, $1, $2, $3, $4, 'INTERNAL', $5
               FROM people WHERE id = $5
         RETURNING id",
    )
    .bind(unit_id)
    .bind(workspace_id)
    .bind(&titulo)
    .bind(amanha)
    .bind(person_id)
    .fetch_one(&harness.pool)
    .await
    .expect("tarefa");

    let agenda = harness
        .open(&format!(
            "/calendar?view=agenda&on={}",
            chrono::Utc::now().date_naive()
        ))
        .await
        .content()
        .await
        .expect("conteúdo");

    assert!(
        agenda.contains(&titulo),
        "o prazo da tarefa não aparece no calendário"
    );
    // Visivelmente um prazo, e não um evento: a interface marca a origem.
    assert!(
        agenda.contains("data-kind=\"task_due\"") || agenda.contains("Prazo"),
        "o prazo aparece sem se distinguir de um evento"
    );

    // Um evento de outra instituição, criado de propósito entre as duas
    // leituras.
    //
    // A corrida que isto substitui era real e intermitente: a contagem global
    // subia por causa de outro teste em paralelo, e a asserção acusava esta
    // tarefa de um evento que nunca criou. Vi-a falhar uma vez — 1174 contra
    // 1175 — e não a consegui provocar em três tentativas seguintes.
    //
    // Um defeito que só aparece por acaso não se prova por reversão. Este
    // evento forasteiro torna-o determinístico: com a contagem por organização
    // é invisível, e com a contagem global faz falhar sempre.
    let outra_instituicao: Uuid =
        sqlx::query_scalar("INSERT INTO organisations (slug, name) VALUES ($1, $2) RETURNING id")
            .bind(format!("alheia-{}", Uuid::new_v4().simple()))
            .bind("Instituição alheia")
            .fetch_one(&harness.pool)
            .await
            .expect("outra instituição");

    sqlx::query(
        "INSERT INTO calendar_events
             (organisation_id, scope, title, classification, all_day, state,
              starts_at, ends_at, timezone, created_by_id)
         VALUES ($1, 'institution', 'Evento alheio', 'INTERNAL', false,
                 'scheduled', now(), now() + interval '1 hour', 'UTC', $2)",
    )
    .bind(outra_instituicao)
    .bind(person_id)
    .execute(&harness.pool)
    .await
    .expect("evento alheio");

    let eventos_depois: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM calendar_events WHERE organisation_id = $1")
            .bind(harness.organisation_id)
            .fetch_one(&harness.pool)
            .await
            .expect("contagem");
    assert_eq!(
        eventos_antes, eventos_depois,
        "a tarefa com prazo criou um evento de calendário"
    );

    // Tirar o prazo tira a projecção. Não há segunda cópia a actualizar.
    sqlx::query("UPDATE tasks SET due_on = NULL WHERE id = $1")
        .bind(task_id)
        .execute(&harness.pool)
        .await
        .expect("tirar o prazo");

    let agenda = harness
        .open(&format!(
            "/calendar?view=agenda&on={}",
            chrono::Utc::now().date_naive()
        ))
        .await
        .content()
        .await
        .expect("conteúdo");
    assert!(
        !agenda.contains(&titulo),
        "uma tarefa sem prazo continua a aparecer no calendário"
    );
}

// ── Lembrete até ao sino ────────────────────────────────────────────────

/// A cadeia inteira, sem o browser estar aberto para a entrega.
///
/// ```text
/// browser cria → PostgreSQL → worker → posse atómica
///              → entrega → notificação → sino → pessoa
/// ```
///
/// # Sem esperar por minutos
///
/// O lembrete nasce com hora já passada e a passagem do worker é chamada
/// directamente. Dormir sessenta segundos provaria o mesmo e tornaria a suite
/// inutilizável.
#[tokio::test]
async fn um_lembrete_percorre_o_worker_ate_ao_sino() {
    let harness = harness!();
    let (person_id, _) = harness.sign_in(&[TechnicalRole::ResearchMember]).await;
    harness.manages_a_unit(person_id).await;

    // O sino começa sem nada. Zero é zero, e não «desconhecido».
    let inicio = harness
        .open("/notifications")
        .await
        .content()
        .await
        .expect("conteúdo");
    assert!(
        inicio.contains("Nada por ler") || inicio.contains("Ainda não há notificações"),
        "o centro de notificações não começa vazio"
    );

    let nota = unique_title("Rever");
    sqlx::query(
        "INSERT INTO reminders (organisation_id, owner_id, note, trigger_at)
             SELECT organisation_id, id, $2, now() - interval '1 minute'
               FROM people WHERE id = $1",
    )
    .bind(person_id)
    .bind(&nota)
    .execute(&harness.pool)
    .await
    .expect("lembrete");

    // O worker corre. O browser podia estar fechado.
    let entregues = ocinye_core::modules::calendar::delivery::deliver_due(&harness.pool)
        .await
        .expect("passagem do worker");
    assert!(entregues >= 1, "o worker não entregou o lembrete vencido");

    // O sino conta.
    let painel = harness.open("/notifications").await;
    let html = painel.content().await.expect("conteúdo");
    assert!(
        html.contains("1 por ler"),
        "o centro de notificações não conta a entrega"
    );
    assert!(
        html.contains(&nota),
        "a notificação não diz de que lembrete se trata"
    );

    // Marcar como lida baixa o contador.
    submit(&painel, "form[action$=\"/read\"]").await;

    // Pelo estado, e não pelo endereço: a página está em `/notifications` e
    // continua lá, portanto esperar que o endereço mude devolvia de imediato.
    let inicio = std::time::Instant::now();
    loop {
        let por_ler: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM notifications WHERE recipient_id = $1 AND read_at IS NULL",
        )
        .bind(person_id)
        .fetch_one(&harness.pool)
        .await
        .expect("contagem");
        if por_ler == 0 {
            break;
        }
        assert!(
            inicio.elapsed() < DEADLINE,
            "marcar como lida não chegou ao PostgreSQL"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    let depois = harness
        .open("/notifications")
        .await
        .content()
        .await
        .expect("conteúdo");
    assert!(
        depois.contains("Nada por ler"),
        "marcar como lida não baixou o contador"
    );

    // Uma segunda passagem não entrega outra vez.
    let _ = ocinye_core::modules::calendar::delivery::deliver_due(&harness.pool).await;
    let notificacoes: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM notifications WHERE recipient_id = $1")
            .bind(person_id)
            .fetch_one(&harness.pool)
            .await
            .expect("contagem");
    assert_eq!(
        notificacoes, 1,
        "uma segunda passagem do worker duplicou a notificação"
    );
}

/// Uma notificação não abre o que a pessoa deixou de poder ver.
#[tokio::test]
async fn uma_notificacao_antiga_nao_contorna_a_autorizacao_actual() {
    let harness = harness!();
    let (person_id, _) = harness.sign_in(&[TechnicalRole::ResearchMember]).await;
    let unit_id = harness.manages_a_unit(person_id).await;

    // Um evento da unidade, restrito: alcançável enquanto gerir a unidade.
    let event_id: Uuid = sqlx::query_scalar(
        "INSERT INTO calendar_events
             (organisation_id, scope, unit_id, title, all_day, starts_at, ends_at,
              timezone, classification, created_by_id)
             SELECT organisation_id, 'unit', $1, 'Reuniao restrita', FALSE,
                    now() + interval '1 day', now() + interval '25 hours',
                    'Europe/Lisbon', 'RESTRICTED', $2
               FROM people WHERE id = $2
         RETURNING id",
    )
    .bind(unit_id)
    .bind(person_id)
    .fetch_one(&harness.pool)
    .await
    .expect("evento");

    // Um lembrete sobre ele, entregue enquanto o acesso existe.
    sqlx::query(
        "INSERT INTO reminders (organisation_id, owner_id, event_id, trigger_at)
             SELECT organisation_id, id, $2, now() - interval '1 minute'
               FROM people WHERE id = $1",
    )
    .bind(person_id)
    .bind(event_id)
    .execute(&harness.pool)
    .await
    .expect("lembrete");
    ocinye_core::modules::calendar::delivery::deliver_due(&harness.pool)
        .await
        .expect("entrega");

    // Controlo positivo: com acesso, a notificação leva ao recurso.
    let antes = harness
        .open(&format!("/calendar/events/{event_id}"))
        .await
        .content()
        .await
        .expect("conteúdo");
    assert!(
        antes.contains("Reuniao restrita"),
        "o evento não é alcançável antes da revogação"
    );

    // A pertença é revogada.
    sqlx::query("UPDATE unit_memberships SET revoked_at = now() WHERE person_id = $1")
        .bind(person_id)
        .execute(&harness.pool)
        .await
        .expect("revogar");

    // A notificação continua na lista — ela informa, e informar não é autorizar.
    let painel = harness
        .open("/notifications")
        .await
        .content()
        .await
        .expect("conteúdo");
    assert!(
        !painel.contains("Reuniao restrita"),
        "a notificação copiou o título do evento: passou a ser uma cópia que \
         ninguém reautoriza"
    );

    // E abrir o recurso pela ligação da notificação não o revela.
    let depois = harness
        .open(&format!("/calendar/events/{event_id}"))
        .await
        .content()
        .await
        .expect("conteúdo");
    assert!(
        !depois.contains("Reuniao restrita"),
        "o recurso continua legível depois de a pertença ser revogada: a \
         notificação funcionou como desvio à autorização"
    );
}

// ── Segurança ───────────────────────────────────────────────────────────

/// Perder o acesso faz o item desaparecer, e o identificador não o traz de volta.
#[tokio::test]
async fn perder_o_acesso_esconde_o_evento_tambem_pelo_identificador() {
    let harness = harness!();
    let (person_id, _) = harness.sign_in(&[TechnicalRole::ResearchMember]).await;
    let unit_id = harness.manages_a_unit(person_id).await;

    let titulo = unique_title("Direccao");
    let event_id: Uuid = sqlx::query_scalar(
        "INSERT INTO calendar_events
             (organisation_id, scope, unit_id, title, all_day, starts_at, ends_at,
              timezone, classification, created_by_id)
             SELECT organisation_id, 'unit', $1, $3, FALSE,
                    now() + interval '2 hours', now() + interval '3 hours',
                    'Europe/Lisbon', 'RESTRICTED', $2
               FROM people WHERE id = $2
         RETURNING id",
    )
    .bind(unit_id)
    .bind(person_id)
    .bind(&titulo)
    .fetch_one(&harness.pool)
    .await
    .expect("evento");

    // Controlo positivo primeiro: sem isto, um mundo onde nada se vê passaria.
    let agenda = harness
        .open(&format!(
            "/calendar?view=today&on={}",
            chrono::Utc::now().date_naive()
        ))
        .await
        .content()
        .await
        .expect("conteúdo");
    assert!(
        agenda.contains(&titulo),
        "o gestor não vê o evento da sua unidade"
    );

    sqlx::query("UPDATE unit_memberships SET revoked_at = now() WHERE person_id = $1")
        .bind(person_id)
        .execute(&harness.pool)
        .await
        .expect("revogar");

    let agenda = harness
        .open(&format!(
            "/calendar?view=today&on={}",
            chrono::Utc::now().date_naive()
        ))
        .await
        .content()
        .await
        .expect("conteúdo");
    assert!(
        !agenda.contains(&titulo),
        "o evento continua na agenda depois de a pertença ser revogada"
    );

    let directo = harness
        .open(&format!("/calendar/events/{event_id}"))
        .await
        .content()
        .await
        .expect("conteúdo");
    assert!(
        !directo.contains(&titulo),
        "o identificador exacto trouxe de volta um evento já inalcançável"
    );
}

/// Privilégio técnico não abre a agenda pessoal de ninguém.
#[tokio::test]
async fn nem_o_administrador_ve_a_agenda_pessoal_alheia() {
    let harness = harness!();

    // A dona marca um evento pessoal.
    let (dona, _) = harness.sign_in(&[TechnicalRole::ResearchMember]).await;
    harness.manages_a_unit(dona).await;
    let titulo = unique_title("Consulta");
    let hoje = chrono::Utc::now().date_naive();
    harness.create_event_via_ui(&titulo, hoje, 15).await;

    // Controlo positivo: a dona vê-o.
    let dela = harness
        .open(&format!("/calendar?view=today&on={hoje}"))
        .await
        .content()
        .await
        .expect("conteúdo");
    assert!(dela.contains(&titulo), "a dona não vê o seu próprio evento");

    // O administrador entra, e não vê.
    let (_, _) = harness.sign_in(&[TechnicalRole::PlatformAdmin]).await;
    let dele = harness
        .open(&format!("/calendar?view=today&on={hoje}"))
        .await
        .content()
        .await
        .expect("conteúdo");
    assert!(
        !dele.contains(&titulo),
        "um administrador de plataforma alcança a agenda pessoal de outra pessoa"
    );
}

// ── Erro não é vazio ────────────────────────────────────────────────────

/// Uma agenda que não carrega diz que não carregou.
///
/// # Erro e vazio nunca se dizem da mesma maneira
///
/// «Nenhuma actividade» é uma afirmação sobre a agenda. «Não consegui ler» é uma
/// afirmação sobre o sistema. Trocá-las faz alguém faltar a uma reunião por
/// acreditar no ecrã.
#[tokio::test]
async fn uma_agenda_que_falha_nao_diz_que_esta_vazia() {
    let harness = harness!();
    let (person_id, _) = harness.sign_in(&[TechnicalRole::ResearchMember]).await;
    harness.manages_a_unit(person_id).await;

    // Controlo positivo: com o Core de pé, uma agenda vazia **diz** que está
    // vazia. Sem isto, o teste passaria num mundo onde nunca se diz nada.
    let vazia = harness
        .open("/calendar?view=agenda&on=2030-01-01")
        .await
        .content()
        .await
        .expect("conteúdo");
    assert!(
        vazia.contains("Nenhuma actividade para este período"),
        "uma agenda genuinamente vazia não o diz"
    );

    // E as vistas com estrutura dizem-no de outra maneira: desenhando-se.
    //
    // Um mês sem nada marcado não escreve «nenhuma actividade» — mostra a
    // grelha vazia, que é o que um calendário limpo é. A distinção entre vazio
    // e falha continua a existir nas duas, e é isso que este teste guarda: só
    // muda a forma como cada uma a diz.
    let mes_vazio = harness
        .open("/calendar?view=month&on=2030-01-01")
        .await
        .content()
        .await
        .expect("conteúdo");
    assert!(
        mes_vazio.contains("oc-cal-month__cell"),
        "um mês genuinamente vazio deixou de desenhar a grelha"
    );
    assert!(
        !mes_vazio.contains("Não foi possível ler a agenda"),
        "um mês vazio disse que a leitura falhou"
    );

    // O núcleo cai.
    harness.stop_core();
    tokio::time::sleep(Duration::from_millis(200)).await;

    for vista in ["day", "week", "month", "agenda"] {
        let html = harness
            .open(&format!("/calendar?view={vista}"))
            .await
            .content()
            .await
            .expect("conteúdo");

        assert!(
            !html.contains("Nenhuma actividade para este período"),
            "«{vista}» disse que não há nada quando a verdade é que não conseguiu ler"
        );
        assert!(
            html.contains("Não foi possível ler a agenda"),
            "«{vista}» não diz que a leitura falhou"
        );
    }
}

// ── Arranque institucional ──────────────────────────────────────────────
//
// O que estas viagens provam não é que existe um ecrã bonito. É que uma pessoa
// que abre o Ocinye OS encontra primeiro a prontidão institucional, e só depois
// aquilo a que tem direito.
//
// Login antes do arranque significaria convidar alguém a autenticar-se num
// sistema que ainda não se sabe se serve. Workspace antes do arranque
// significaria mostrar conteúdo protegido e escondê-lo a seguir — e um flash de
// conteúdo protegido é conteúdo protegido mostrado.

/// Um Core de mentira que responde exactamente a prontidão que se lhe pedir.
///
/// # Porque é preciso
///
/// Provar `Blocked`, `Degraded` e a ausência de resposta exige um Core nesses
/// estados, e derrubar o Core verdadeiro deixaria o resto do teste sem base de
/// dados. Um `?contract=` na consulta do arranque não serve: seria uma porta de
/// serviço no produto, aberta para os testes entrarem.
///
/// Isto serve a projecção pública tal como o Core a serviria, e o Workspace que
/// fala com ele não sabe a diferença — que é precisamente o ponto.
async fn core_de_mentira(
    corpo: serde_json::Value,
    estado: u16,
) -> (String, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind(std::net::SocketAddr::from((
        std::net::Ipv4Addr::LOCALHOST,
        0,
    )))
    .await
    .expect("ouvinte");
    let url = format!("http://{}", listener.local_addr().expect("endereço"));

    let router = axum::Router::new().route(
        "/ready",
        axum::routing::get(move || {
            let corpo = corpo.clone();
            async move {
                (
                    axum::http::StatusCode::from_u16(estado).expect("estado"),
                    axum::Json(corpo),
                )
            }
        }),
    );

    let tarefa = tokio::spawn(async move {
        let _ = axum::serve(listener, router).await;
    });
    (url, tarefa)
}

/// Uma projecção pública com o estado que se pedir.
fn projeccao(overall: &str, componentes: serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "overall": overall,
        "contract_version": ocinye_contracts::readiness::CONTRACT_VERSION,
        "components": componentes,
    })
}

/// Um Workspace que fala com o Core indicado.
///
/// Reutiliza tudo o resto do harness — os estáticos, as sessões, o router real.
/// A única coisa que muda é com quem fala.
async fn workspace_contra(harness: &Harness, core_url: &str) -> String {
    let listener = tokio::net::TcpListener::bind(std::net::SocketAddr::from((
        std::net::Ipv4Addr::LOCALHOST,
        0,
    )))
    .await
    .expect("ouvinte");
    let url = format!("http://{}", listener.local_addr().expect("endereço"));
    let estado = workspace_state(
        core_url,
        &url,
        &format!("{}/static", env!("CARGO_MANIFEST_DIR")),
    );
    let router = workspace_routes::router(estado);
    tokio::spawn(async move {
        let _ = axum::serve(listener, router).await;
    });
    let _ = harness;
    url
}

/// Se o browser tem o marcador de arranque.
///
/// # Porque não o histórico, e porque não o conteúdo
///
/// O conteúdo é uma corrida: o arranque entrega depressa de propósito, e
/// observá-lo pelo HTML significa competir com ele — um teste que falha por o
/// produto ser rápido.
///
/// O histórico também não serve: o encaminhamento do portão colapsa numa
/// entrada só, e a actualização de meta substitui-a. Tentei, e o histórico
/// mostrava apenas `about:blank` e o destino final.
///
/// O marcador é durável e só o `/boot` o emite, e só quando decidiu que havia
/// por onde seguir. A sua presença é prova de que o arranque correu.
async fn tem_marcador(page: &Page) -> bool {
    use chromiumoxide::cdp::browser_protocol::network::GetCookiesParams;
    page.execute(GetCookiesParams::default())
        .await
        .map(|r| r.result.cookies.iter().any(|c| c.name == "oc_boot"))
        .unwrap_or(false)
}

/// O portão encaminha, ao nível do HTTP, quem chega sem marcador.
///
/// Verificado fora do browser de propósito: sem cookies, sem seguir
/// encaminhamentos, e portanto sem corrida nenhuma. O que se vê é o que o
/// servidor responde a quem bate à porta pela primeira vez.
async fn o_portao_encaminha(workspace_url: &str, caminho: &str) -> (u16, String) {
    let cliente = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .expect("cliente");
    let resposta = cliente
        .get(format!("{workspace_url}{caminho}"))
        .header("Accept", "text/html")
        .send()
        .await
        .expect("pedido");
    let estado = resposta.status().as_u16();
    let destino = resposta
        .headers()
        .get("location")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_owned();
    (estado, destino)
}

/// Abre sem sessão e sem marcador, como quem chega pela primeira vez.
async fn abrir_a_frio(harness: &Harness, caminho: &str) -> Page {
    let page = harness
        .browser
        .new_page("about:blank")
        .await
        .expect("página em branco");
    // Sem cookies: nem sessão, nem marcador de arranque.
    page.execute(
        chromiumoxide::cdp::browser_protocol::network::ClearBrowserCookiesParams::default(),
    )
    .await
    .expect("limpar cookies");
    page.goto(format!("{}{caminho}", harness.workspace_url))
        .await
        .expect("abrir");
    // Sem `wait_for_navigation`: a navegação seguinte é a entrega do arranque, e
    // esperá-la destruiria o contexto antes de podermos ver o que o arranque
    // mostrou. Quem chama lê já.
    page
}

/// Lê o conteúdo tolerando que a página esteja a navegar.
///
/// A entrega do arranque acontece por actualização de meta, e uma leitura que
/// caia exactamente nesse instante encontra o contexto a ser substituído. Isso
/// não é uma falha do produto; é uma corrida de quem observa.
async fn conteudo_estavel(page: &Page) -> String {
    for _ in 0..40 {
        if let Ok(html) = page.content().await {
            if !html.is_empty() {
                return html;
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(80)).await;
    }
    panic!("a página nunca deu conteúdo legível");
}

/// Uma abertura a frio encontra o arranque, e não o Login.
///
/// O primeiro controlo positivo de todo o ciclo. Se esta viagem falhar, tudo o
/// resto que depender de «a pessoa vê primeiro o arranque» está a medir outra
/// coisa.
#[tokio::test]
async fn uma_abertura_a_frio_encontra_o_arranque_primeiro() {
    let harness = harness!();

    let page = abrir_a_frio(&harness, "/").await;

    // Esperar por estado observável: já não estar no arranque.
    let inicio = std::time::Instant::now();
    while page
        .url()
        .await
        .ok()
        .flatten()
        .unwrap_or_default()
        .contains("/boot")
        || page
            .url()
            .await
            .ok()
            .flatten()
            .unwrap_or_default()
            .is_empty()
    {
        assert!(
            inicio.elapsed() < std::time::Duration::from_secs(45),
            "o arranque não entregou"
        );
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }

    // O arranque correu: o marcador só é emitido por ele.
    assert!(
        tem_marcador(&page).await,
        "uma abertura a frio não passou pelo arranque"
    );

    // E o portão encaminha mesmo quem chega sem marcador — verificado ao nível
    // do HTTP, sem cookies e sem seguir encaminhamentos.
    let (estado, destino) = o_portao_encaminha(&harness.workspace_url, "/").await;
    assert!(
        (300..400).contains(&estado),
        "uma abertura a frio devia ser encaminhada, e respondeu {estado}"
    );
    assert!(
        destino.contains("/boot"),
        "o portão encaminhou para «{destino}», e não para o arranque"
    );
}

/// Com o Core pronto, o arranque entrega a sessão — e sem sessão isso é o Login.
#[tokio::test]
async fn sem_sessao_o_arranque_entrega_ao_login() {
    let harness = harness!();

    let page = abrir_a_frio(&harness, "/").await;

    // A entrega é uma actualização de meta. Esperar por estado observável, e
    // nunca por tempo adivinhado.
    let inicio = std::time::Instant::now();
    loop {
        let url = page.url().await.expect("url").unwrap_or_default();
        if url.contains("/login") {
            break;
        }
        assert!(
            inicio.elapsed() < std::time::Duration::from_secs(45),
            "o arranque não entregou ao Login em quinze segundos; ficou em «{url}»"
        );
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;
    }

    let html = conteudo_estavel(&page).await;
    assert!(
        html.contains("oc-login__submit"),
        "chegou ao Login e não há formulário de entrada"
    );

    // E chegou lá **pelo arranque**: o marcador prova-o.
    assert!(
        tem_marcador(&page).await,
        "chegou ao Login sem passar pelo arranque"
    );

    // O Login em si não é servido a quem não arrancou.
    let (estado, destino) = o_portao_encaminha(&harness.workspace_url, "/login").await;
    assert!(
        (300..400).contains(&estado) && destino.contains("/boot"),
        "o Login foi servido sem arranque: {estado} → «{destino}»"
    );
}

/// Com sessão activa, o arranque entrega ao Workspace sem passar pelo Login.
///
/// O controlo positivo do teste anterior: o que levou ao Login foi não haver
/// sessão, e não o arranque levar sempre ao Login.
#[tokio::test]
async fn com_sessao_o_arranque_entrega_ao_workspace() {
    let harness = harness!();
    harness.sign_in(&[TechnicalRole::ResearchMember]).await;

    // A sessão existe; o marcador não. Uma abertura a frio de quem já entrou.
    let page = harness
        .browser
        .new_page(format!("{}/boot", harness.workspace_url))
        .await
        .expect("página");
    page.wait_for_navigation().await.expect("navegação");

    let inicio = std::time::Instant::now();
    loop {
        let url = page.url().await.expect("url").unwrap_or_default();
        if !url.contains("/boot") {
            assert!(
                !url.contains("/login"),
                "com sessão activa o arranque não devia passar pelo Login: «{url}»"
            );
            break;
        }
        assert!(
            inicio.elapsed() < std::time::Duration::from_secs(45),
            "o arranque não entregou em quinze segundos; ficou em «{url}»"
        );
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;
    }

    let html = page.content().await.expect("conteúdo");
    assert!(
        html.contains("oc-side__nav"),
        "chegou ao Workspace e não há navegação: {}",
        &html[..html.len().min(300)]
    );
}

/// Um destino profundo sobrevive ao arranque.
///
/// Abrir `/calendar` a frio não pode custar o destino. Uma pessoa que segue uma
/// ligação para um sítio concreto tem de lá chegar depois de o sistema arrancar
/// — e não à porta de entrada.
#[tokio::test]
async fn um_destino_profundo_sobrevive_ao_arranque() {
    let harness = harness!();
    harness.sign_in(&[TechnicalRole::ResearchMember]).await;

    let page = harness
        .browser
        .new_page(format!("{}/calendar", harness.workspace_url))
        .await
        .expect("página");
    page.wait_for_navigation().await.expect("navegação");

    // A passagem pelo arranque é transitória: quando se lê o endereço, a
    // entrega pode já ter acontecido. Afirmá-la aqui seria correr contra ela.
    //
    // O que se prova é o resultado — chegar ao destino pedido, e não à porta de
    // entrada — que é a propriedade que interessa a quem seguiu a ligação.
    let inicio = std::time::Instant::now();
    loop {
        let url = page.url().await.expect("url").unwrap_or_default();
        if url.ends_with("/calendar") {
            break;
        }
        assert!(
            inicio.elapsed() < std::time::Duration::from_secs(45),
            "o destino profundo não foi restituído; ficou em «{url}»"
        );
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;
    }
}

/// Um marcador forjado salta a apresentação, e mais nada.
///
/// # A propriedade que isto guarda
///
///     O arranque-concluído pode ser guardado como estado de apresentação.
///     A autoridade sobre prontidão não pode.
///
/// Forjar o marcador dispensa o Splash — e é tudo o que consegue. Não
/// autentica: a sessão continua a ser resolvida, e sem sessão o destino é o
/// Login.
#[tokio::test]
async fn um_marcador_forjado_nao_autentica_ninguem() {
    let harness = harness!();

    let page = harness
        .browser
        .new_page("about:blank")
        .await
        .expect("página");
    page.execute(
        chromiumoxide::cdp::browser_protocol::network::ClearBrowserCookiesParams::default(),
    )
    .await
    .expect("limpar cookies");

    // O marcador, escrito à mão. Nada o assina, e não precisa: o que ele pode
    // fazer é tão pouco que forjá-lo não compensa.
    let host = harness
        .workspace_url
        .trim_start_matches("http://")
        .to_owned();
    page.execute(
        chromiumoxide::cdp::browser_protocol::network::SetCookieParams::builder()
            .name("oc_boot")
            .value("1")
            .domain(host.split(':').next().unwrap_or("127.0.0.1"))
            .path("/")
            .build()
            .expect("cookie"),
    )
    .await
    .expect("gravar marcador");

    page.goto(format!("{}/calendar", harness.workspace_url))
        .await
        .expect("abrir");

    let inicio = std::time::Instant::now();
    let destino = loop {
        let url = page.url().await.ok().flatten().unwrap_or_default();
        if url.contains("/login") || url.contains("/calendar") {
            break url;
        }
        assert!(
            inicio.elapsed() < std::time::Duration::from_secs(45),
            "não chegou a lado nenhum; ficou em «{url}»"
        );
        tokio::time::sleep(std::time::Duration::from_millis(120)).await;
    };

    assert!(
        destino.contains("/login"),
        "um marcador forjado levou a «{destino}» sem sessão nenhuma"
    );

    let html = conteudo_estavel(&page).await;
    assert!(
        !html.contains("oc-side__nav"),
        "o Workspace foi servido a quem só tinha um cookie inventado"
    );
}

/// O controlo positivo do teste acima.
///
/// Com marcador **e** sessão, o Workspace abre. Sem isto, a recusa acima podia
/// estar a acontecer porque nada passa, e não porque falta a sessão.
#[tokio::test]
async fn com_marcador_e_sessao_o_workspace_abre() {
    let harness = harness!();
    harness.sign_in(&[TechnicalRole::ResearchMember]).await;

    let page = harness.open("/calendar").await;
    let url = page.url().await.expect("url").unwrap_or_default();
    assert!(
        url.ends_with("/calendar"),
        "com sessão e marcador o destino devia abrir: «{url}»"
    );
    let html = conteudo_estavel(&page).await;
    assert!(
        html.contains("oc-side__nav"),
        "o Workspace não abriu para quem tem sessão"
    );
}

/// A navegação interna não repete o arranque.
///
/// O arranque é o ciclo de entrada, e não um monitor contínuo. Reaparecer a cada
/// passo seria transformar uma cortesia numa interrupção.
#[tokio::test]
async fn a_navegacao_interna_nao_repete_o_arranque() {
    let harness = harness!();
    harness.sign_in(&[TechnicalRole::ResearchMember]).await;

    for caminho in ["/", "/calendar", "/notifications", "/help"] {
        let page = harness.open(caminho).await;
        let url = page.url().await.expect("url").unwrap_or_default();
        assert!(
            !url.contains("/boot"),
            "o arranque reapareceu ao navegar para {caminho}: «{url}»"
        );
        page.close().await.ok();
    }
}

/// Nenhum destino de regresso sai do Ocinye OS.
///
/// Verificado ao nível do HTTP: o que interessa é o que o servidor responde, e
/// não o que o browser faz depois. Um destino hostil não pode sequer aparecer
/// num cabeçalho de encaminhamento.
#[tokio::test]
async fn nenhum_destino_de_regresso_sai_do_ocinye() {
    let harness = harness!();

    let cliente = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .expect("cliente");

    for hostil in [
        "https://exemplo.mau",
        "//exemplo.mau",
        "javascript:alert(1)",
        "/%5Cexemplo.mau",
        "https%3A%2F%2Fexemplo.mau",
    ] {
        let resposta = cliente
            .get(format!(
                "{}/boot?return_to={}",
                harness.workspace_url,
                urlencoding_de_teste(hostil)
            ))
            .header("Accept", "text/html")
            .send()
            .await
            .expect("pedido");

        let corpo = resposta.text().await.unwrap_or_default();
        assert!(
            !corpo.contains("exemplo.mau"),
            "«{hostil}» chegou à página de arranque como destino"
        );
    }
}

/// Codifica um valor para a consulta, no teste.
fn urlencoding_de_teste(valor: &str) -> String {
    valor
        .chars()
        .map(|c| match c {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' | '.' | '~' => c.to_string(),
            outro => outro
                .to_string()
                .bytes()
                .map(|b| format!("%{b:02X}"))
                .collect(),
        })
        .collect()
}

/// O arranque mostra os estados que o Core reporta, e não uma lista escrita à mão.
///
/// # O que isto guarda
///
/// A tentação numa superfície de arranque é escrever os módulos no HTML e pintá-los
/// de verde. Fica bonito, é estável, e mente: um módulo que caia continua verde,
/// e um módulo novo nunca aparece.
///
/// O que aqui se prova é que o que se vê veio do Core — comparando a página com
/// a resposta de `/ready` obtida em separado.
#[tokio::test]
async fn os_modulos_no_arranque_vem_do_core() {
    let harness = harness!();

    // A verdade, pedida directamente ao Core.
    let prontidao: serde_json::Value = reqwest::get(format!("{}/ready", harness.core_url))
        .await
        .expect("pedido ao Core")
        .json()
        .await
        .expect("projecção");

    let componentes = prontidao
        .get("components")
        .and_then(|c| c.as_array())
        .expect("a projecção tem de trazer componentes");
    assert!(
        !componentes.is_empty(),
        "uma prontidão sem componentes não permite provar nada"
    );

    // O que a página de arranque mostra.
    let pagina = reqwest::get(format!("{}/boot", harness.workspace_url))
        .await
        .expect("arranque")
        .text()
        .await
        .expect("html");

    // O Calendar está fechado e disponível: tem de aparecer assim.
    let calendar = componentes
        .iter()
        .find(|c| c["component"] == "calendar")
        .expect("o Calendar tem de estar na projecção");
    assert_eq!(
        calendar["state"], "available",
        "o Calendar está fechado e disponível; o Core reporta «{}»",
        calendar["state"]
    );

    // Um componente disponível não é uma limitação, e portanto não aparece na
    // lista de limitações do arranque.
    assert!(
        !pagina.contains("Calendar</span>") || !pagina.contains("oc-boot__list--blocking"),
        "o Calendar apareceu como bloqueio estando disponível"
    );

    // E os que o Core diz estarem indisponíveis aparecem — com a razão que ele
    // deu, e não com uma inventada aqui.
    for componente in componentes {
        let estado = componente["state"].as_str().unwrap_or_default();
        if estado == "available" {
            continue;
        }
        let razao = componente["reason"].as_str().unwrap_or_default();
        if razao.is_empty() {
            continue;
        }
        assert!(
            pagina.contains(razao),
            "o Core disse «{razao}» sobre {} e o arranque não o mostrou",
            componente["component"]
        );
    }
}

/// O arranque é utilizável sem rato e com movimento reduzido.
///
/// Não é polimento adiado: uma superfície que só se opera com rato exclui
/// quem não o usa, e o arranque é a porta de entrada de toda a gente.
#[tokio::test]
async fn o_arranque_e_utilizavel_por_teclado() {
    let harness = harness!();

    // Um arranque bloqueado, para haver botão de tentar novamente. Consegue-se
    // pedindo com um contrato que o Core não fala.
    let pagina = reqwest::get(format!("{}/boot", harness.workspace_url))
        .await
        .expect("arranque")
        .text()
        .await
        .expect("html");

    // O título é um título, e não um `div` com letra grande.
    assert!(
        pagina.contains("<h1") && pagina.contains("oc-boot__title"),
        "o estado do arranque tem de ser um cabeçalho"
    );

    // E o estado é anunciado como estado, e não como alerta: um alerta
    // interrompe quem está a ler.
    assert!(
        pagina.contains(r#"role="status""#),
        "o painel de arranque tem de declarar que descreve um estado"
    );
    assert!(
        !pagina.contains(r#"role="alert""#),
        "o arranque não é um alerta"
    );
}

/// O arranque cabe num ecrã pequeno.
#[tokio::test]
async fn o_arranque_cabe_num_ecra_pequeno() {
    let harness = harness!();

    let page = harness
        .browser
        .new_page("about:blank")
        .await
        .expect("página");
    page.execute(
        chromiumoxide::cdp::browser_protocol::emulation::SetDeviceMetricsOverrideParams::builder()
            .width(390)
            .height(844)
            .device_scale_factor(3.0)
            .mobile(true)
            .build()
            .expect("métricas"),
    )
    .await
    .expect("emular telemóvel");

    // Um arranque que **não** entrega, para a página ficar quieta enquanto se
    // mede: um Core que diz `Blocked` não tem para onde seguir.
    //
    // A primeira versão pedia `?contract=999999` ao arranque, julgando que isso
    // bloquearia. Não bloqueia — o arranque declara ao Core o contrato que este
    // binário fala, e uma consulta na sua própria URL não muda nada. A página
    // seguia, a medição corria contra a entrega, e o teste falhava dois em cada
    // três com um erro que se lê como avaria do browser.
    let (core, _t) = core_de_mentira(projeccao("blocked", serde_json::json!([])), 503).await;
    let ws = workspace_contra(&harness, &core).await;
    page.goto(format!("{ws}/boot")).await.expect("abrir");

    let transbordo: bool = page
        .evaluate("document.documentElement.scrollWidth > document.documentElement.clientWidth + 1")
        .await
        .expect("medir")
        .into_value()
        .expect("booleano");

    assert!(
        !transbordo,
        "o arranque transborda horizontalmente num ecrã de 390px"
    );
}

/// Um Core que diz `Blocked` bloqueia o arranque, e o Login não aparece.
#[tokio::test]
async fn um_core_bloqueado_nao_mostra_o_login() {
    let harness = harness!();
    let (core, _t) = core_de_mentira(
        projeccao(
            "blocked",
            serde_json::json!([{
                "component": "persistence",
                "state": "unavailable",
                "criticality": "critical",
                "reason": "Registado, mas sem resposta."
            }]),
        ),
        503,
    )
    .await;
    let ws = workspace_contra(&harness, &core).await;

    let pagina = reqwest::get(format!("{ws}/boot"))
        .await
        .expect("arranque")
        .text()
        .await
        .expect("html");

    assert!(
        pagina.contains("NÃO FOI POSSÍVEL INICIAR"),
        "um Core bloqueado devia bloquear o arranque"
    );
    assert!(!pagina.contains("oc-login__submit"), "o Login apareceu");
    assert!(!pagina.contains("oc-side__nav"), "a shell apareceu");
    assert!(pagina.contains("Tentar novamente"), "falta tentar de novo");
    assert!(
        !pagina.contains("http-equiv=\"refresh\""),
        "um arranque bloqueado tentou seguir na mesma"
    );
    assert!(
        pagina.contains("Registado, mas sem resposta."),
        "a razão do Core não chegou a quem lê"
    );
}

/// Um Core sem resposta não é um Core bloqueado.
///
/// A distinção que mais custa a manter e mais importa: uma é uma decisão, a
/// outra é a ausência dela. Uma interface que as juntasse afirmaria que o Core
/// decidiu quando o Core nem sequer respondeu.
#[tokio::test]
async fn um_core_sem_resposta_nao_se_confunde_com_bloqueado() {
    let harness = harness!();
    // Um porto onde não está ninguém.
    let ws = workspace_contra(&harness, "http://127.0.0.1:1").await;

    let pagina = reqwest::get(format!("{ws}/boot"))
        .await
        .expect("arranque")
        .text()
        .await
        .expect("html");

    assert!(
        pagina.contains("NÃO FOI POSSÍVEL CONTACTAR"),
        "um Core sem resposta devia dizê-lo"
    );
    assert!(
        !pagina.contains("NÃO FOI POSSÍVEL INICIAR"),
        "sem resposta foi apresentado como decisão do Core"
    );
    assert!(pagina.contains("Tentar novamente"));
}

/// Um Core degradado deixa seguir, e diz o que falta.
#[tokio::test]
async fn um_core_degradado_deixa_seguir_e_diz_o_que_falta() {
    let harness = harness!();
    let (core, _t) = core_de_mentira(
        projeccao(
            "degraded",
            serde_json::json!([{
                "component": "mail",
                "state": "not_configured",
                "criticality": "optional",
                "reason": "Não configurado nesta instalação."
            }]),
        ),
        200,
    )
    .await;
    let ws = workspace_contra(&harness, &core).await;

    let pagina = reqwest::get(format!("{ws}/boot"))
        .await
        .expect("arranque")
        .text()
        .await
        .expect("html");

    assert!(
        pagina.contains("PRONTO COM LIMITAÇÕES"),
        "um Core degradado devia deixar seguir com limitações"
    );
    assert!(
        pagina.contains("Não configurado nesta instalação."),
        "a limitação factual não chegou a quem lê"
    );
    assert!(
        pagina.contains("http-equiv=\"refresh\""),
        "degradado não é erro fatal: tem de seguir"
    );
    assert!(
        !pagina.contains("Tentar novamente"),
        "um arranque que segue não oferece tentar de novo"
    );
}

/// Um arranque bloqueado não grava o marcador.
///
/// Gravá-lo faria a tentativa seguinte saltar a apresentação de um problema que
/// continua lá — e a pessoa iria bater com a cara na porta, sem explicação.
#[tokio::test]
async fn um_arranque_bloqueado_nao_grava_o_marcador() {
    let harness = harness!();
    let (core, _t) = core_de_mentira(projeccao("blocked", serde_json::json!([])), 503).await;
    let ws = workspace_contra(&harness, &core).await;

    let cliente = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .expect("cliente");

    let bloqueado = cliente
        .get(format!("{ws}/boot"))
        .send()
        .await
        .expect("arranque bloqueado");
    assert!(
        !bloqueado
            .headers()
            .get_all("set-cookie")
            .iter()
            .any(|v| v.to_str().unwrap_or_default().contains("oc_boot")),
        "um arranque bloqueado gravou o marcador"
    );

    // O controlo positivo, contra o Core verdadeiro: um arranque que segue
    // **grava**. Sem isto, a ausência acima podia ser porque nunca se grava.
    let pronto = cliente
        .get(format!("{}/boot", harness.workspace_url))
        .send()
        .await
        .expect("arranque pronto");
    assert!(
        pronto
            .headers()
            .get_all("set-cookie")
            .iter()
            .any(|v| v.to_str().unwrap_or_default().contains("oc_boot")),
        "um arranque que segue devia gravar o marcador"
    );
}

/// Um marcador forjado não contorna um arranque bloqueado.
#[tokio::test]
async fn um_marcador_forjado_nao_contorna_o_bloqueio() {
    let harness = harness!();
    let (core, _t) = core_de_mentira(projeccao("blocked", serde_json::json!([])), 503).await;
    let ws = workspace_contra(&harness, &core).await;

    let cliente = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .expect("cliente");

    let pagina = cliente
        .get(format!("{ws}/boot"))
        .header("Cookie", "oc_boot=1")
        .send()
        .await
        .expect("arranque")
        .text()
        .await
        .expect("html");

    assert!(
        pagina.contains("NÃO FOI POSSÍVEL INICIAR"),
        "um marcador forjado contornou o bloqueio"
    );
}

/// A resposta do arranque nunca é guardada.
#[tokio::test]
async fn o_arranque_nunca_e_guardado() {
    let harness = harness!();

    let resposta = reqwest::get(format!("{}/boot", harness.workspace_url))
        .await
        .expect("arranque");
    let cache = resposta
        .headers()
        .get("cache-control")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_owned();

    assert!(
        cache.contains("no-store"),
        "o arranque tem de proibir armazenamento, e disse «{cache}»"
    );
}

// ── Um Core verdadeiro com a prontidão trocada ──────────────────────────

/// O Core real, com `/ready` respondido por nós.
///
/// # Porque um duplo completo não servia
///
/// As viagens que faltam precisam das duas coisas ao mesmo tempo: uma sessão
/// verdadeira — que só se obtém autenticando contra o Core a sério — e uma
/// prontidão escolhida, que o Core a sério não tem como fingir. Um duplo
/// completo dá a segunda e tira a primeira: sem `/api/v1/auth/login` não há
/// entrada, e sem entrada não há topbar para observar.
///
/// Isto encaminha tudo para o Core verdadeiro e responde só `/ready`. É a
/// substituição mínima: exactamente a superfície de onde a prontidão vem, e
/// nem um pedido a mais.
async fn core_com_prontidao_trocada(
    core_real: &str,
    projeccao: serde_json::Value,
    estado: u16,
) -> (String, Prontidao) {
    let listener = tokio::net::TcpListener::bind(std::net::SocketAddr::from((
        std::net::Ipv4Addr::LOCALHOST,
        0,
    )))
    .await
    .expect("ouvinte");
    let url = format!("http://{}", listener.local_addr().expect("endereço"));
    let destino = core_real.to_owned();

    let prontidao = Prontidao(Arc::new(std::sync::Mutex::new((projeccao, estado))));
    let lida = prontidao.clone();

    let router = axum::Router::new()
        .route(
            "/ready",
            axum::routing::get(move || {
                let lida = lida.clone();
                async move {
                    let (corpo, estado) = lida.actual();
                    (
                        axum::http::StatusCode::from_u16(estado).expect("estado"),
                        axum::Json(corpo),
                    )
                }
            }),
        )
        .fallback(move |pedido: axum::extract::Request| {
            let destino = destino.clone();
            async move { encaminhar(&destino, pedido).await }
        });

    tokio::spawn(async move {
        let _ = axum::serve(listener, router).await;
    });
    (url, prontidao)
}

/// A prontidão que este Core devolve, e que se pode mudar a meio.
///
/// Existe porque as viagens que faltam são sobre **transições**: alguém está a
/// trabalhar e o Core degrada-se; alguém está preso e o Core recupera. Uma
/// prontidão fixa prova estados, não prova travessias — e é nas travessias que
/// uma interface costuma mentir, por continuar a mostrar o que era verdade há
/// um minuto.
#[derive(Clone)]
struct Prontidao(Arc<std::sync::Mutex<(serde_json::Value, u16)>>);

impl Prontidao {
    fn actual(&self) -> (serde_json::Value, u16) {
        self.0.lock().expect("prontidão").clone()
    }

    fn passa_a(&self, corpo: serde_json::Value, estado: u16) {
        *self.0.lock().expect("prontidão") = (corpo, estado);
    }
}

/// Passa um pedido adiante tal como chegou, e devolve a resposta tal como veio.
async fn encaminhar(destino: &str, pedido: axum::extract::Request) -> axum::response::Response {
    use axum::body::Body;
    use axum::response::IntoResponse;

    let (partes, corpo) = pedido.into_parts();
    let caminho = partes
        .uri
        .path_and_query()
        .map_or_else(|| partes.uri.path().to_owned(), ToString::to_string);
    let bytes = axum::body::to_bytes(corpo, 4 * 1024 * 1024)
        .await
        .unwrap_or_default();

    let mut cabecalhos = partes.headers.clone();
    // O anfitrião é o do destino, e nunca o nosso: reencaminhá-lo faria o Core
    // responder a alguém que não existe.
    cabecalhos.remove(axum::http::header::HOST);

    let resposta = reqwest::Client::new()
        .request(partes.method.clone(), format!("{destino}{caminho}"))
        .headers(cabecalhos)
        .body(bytes.to_vec())
        .send()
        .await;

    match resposta {
        Ok(r) => {
            let estado = r.status();
            let cabecalhos = r.headers().clone();
            let corpo = r.bytes().await.unwrap_or_default();
            let mut saida = axum::response::Response::new(Body::from(corpo));
            *saida.status_mut() = estado;
            // `content-length` vem do original e pode não descrever o que
            // acabámos de montar; o transporte volta a calculá-lo.
            for (nome, valor) in cabecalhos.iter() {
                if nome != axum::http::header::CONTENT_LENGTH
                    && nome != axum::http::header::TRANSFER_ENCODING
                {
                    saida.headers_mut().append(nome, valor.clone());
                }
            }
            saida
        }
        Err(erro) => (
            axum::http::StatusCode::BAD_GATEWAY,
            format!("o encaminhamento falhou: {erro}"),
        )
            .into_response(),
    }
}

/// Apaga só o marcador de arranque, deixando a sessão onde está.
///
/// Limpar todos os cookies apagaria também a sessão, e a viagem passaria a ser
/// «alguém sem sessão volta ao princípio» — que já está provada e não é esta.
async fn esquecer_o_marcador(page: &Page, workspace_url: &str) {
    use chromiumoxide::cdp::browser_protocol::network::DeleteCookiesParams;
    page.execute(
        DeleteCookiesParams::builder()
            .name("oc_boot")
            .url(workspace_url)
            .build()
            .expect("parâmetros"),
    )
    .await
    .expect("apagar o marcador");
}

// ── Fluxos obrigatórios de sessão ───────────────────────────────────────

/// Uma sessão a meio não é libertada pelo arranque.
///
/// # A pergunta
///
/// O arranque entrega a um destino. Uma pessoa que entrou com uma credencial
/// temporária deve ao Core uma palavra-passe definitiva antes de fazer seja o
/// que for, e o produto encaminha-a para o primeiro acesso. A pergunta é se o
/// arranque, ao entregar directamente a um destino profundo, a deixa passar por
/// baixo desse dever.
///
/// # Porque a resposta é não, e não é por sorte
///
/// O arranque não consulta a sessão. Entrega ao destino, e é o destino que
/// decide quem entra. Um arranque que resolvesse a sessão sozinho teria de
/// reimplementar essa decisão, e passaria a haver dois sítios a dizer quem pode
/// trabalhar — que é exactamente como se cria uma passagem por baixo.
#[tokio::test]
async fn uma_sessao_a_meio_nao_e_libertada_pelo_arranque() {
    let harness = harness!();
    let credenciais = harness.entrar_com_credencial_temporaria().await;

    // Depois de entrar, o produto já a pôs onde ela tem de estar.
    let inicial = harness.open("/").await;
    assert!(
        conteudo_estavel(&inicial).await.contains("PRIMEIRO ACESSO"),
        "entrar com credencial temporária devia levar ao primeiro acesso"
    );

    // Agora o que interessa: esquecer o arranque e abrir um destino profundo.
    // O portão vai encaminhar para `/boot`, o arranque vai entregar a `/tasks`,
    // e `/tasks` tem de continuar a recusar.
    esquecer_o_marcador(&inicial, &harness.workspace_url).await;
    let profundo = harness.open("/calendar").await;
    let visto = conteudo_estavel(&profundo).await;

    assert!(
        visto.contains("PRIMEIRO ACESSO"),
        "o arranque entregou a `/calendar` e o dever ao Core desapareceu pelo caminho: {}",
        &visto[..visto.len().min(400)]
    );

    // E o marcador ficou: o arranque aconteceu, mesmo tendo a viagem acabado
    // num sítio diferente do pedido. São duas decisões independentes.
    assert!(
        tem_marcador(&profundo).await,
        "o arranque correu e devia ter deixado marca"
    );

    let _ = credenciais;
}

/// O portão de arranque não é uma segunda porta de autorização.
///
/// Ao nível do HTTP, sem browser: quem tem marcador de arranque mas nenhuma
/// sessão continua a ser mandado ao Login, e quem tem marcador e uma sessão a
/// meio continua a ser mandado ao primeiro acesso. O marcador atesta que o
/// arranque aconteceu, e mais nada — se alguma vez começar a atestar
/// identidade, é aqui que se vê.
#[tokio::test]
async fn o_marcador_de_arranque_nao_vale_por_sessao() {
    let harness = harness!();

    let cliente = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .expect("cliente");
    let resposta = cliente
        .get(format!("{}/calendar", harness.workspace_url))
        .header("Accept", "text/html")
        .header("Cookie", "oc_boot=1")
        .send()
        .await
        .expect("pedido");

    assert_eq!(
        resposta.status().as_u16(),
        303,
        "com marcador e sem sessão, `/calendar` tinha de encaminhar"
    );
    let destino = resposta
        .headers()
        .get("location")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    assert_eq!(
        destino, "/login",
        "o marcador de arranque passou a valer por sessão"
    );
}

// ── Histórico ───────────────────────────────────────────────────────────

/// Quantas entradas tem o histórico desta aba, quando a página deixa perguntar.
///
/// Durante a entrega do arranque o contexto está a ser substituído e o Chrome
/// responde «cannot find context» — que é o observador a chegar a meio, não o
/// produto. Repete até haver resposta, e diz que não houve em vez de devolver
/// um número inventado.
async fn comprimento_do_historico(page: &Page) -> i64 {
    for _ in 0..40 {
        if let Ok(valor) = page.evaluate("history.length").await {
            if let Ok(n) = valor.into_value::<i64>() {
                return n;
            }
        }
        tokio::time::sleep(Duration::from_millis(120)).await;
    }
    panic!("a página nunca respondeu ao comprimento do histórico");
}

/// Voltar atrás depois do arranque não prende a pessoa no arranque.
///
/// # O defeito que isto procura
///
/// O arranque entrega por actualização de meta. Se essa entrega deixasse uma
/// entrada no histórico, voltar atrás traria a pessoa ao arranque, o arranque
/// voltaria a entregar, e o botão de retroceder deixaria de funcionar — um ciclo
/// de que só se sai fechando o separador. É um defeito clássico dos ecrãs de
/// arranque, e a razão pela qual a entrega é feita abaixo do segundo.
///
/// Este comentário dizia que o Chrome **substitui** a entrada quando o atraso é
/// curto. A CI desmentiu: com 0,6 s o Chrome acrescentou-a, e o retroceder foi
/// parar ao arranque. A entrega passou a fazer-se com `location.replace` em
/// `app.js`, e é o comprimento do histórico que o prova — sem depender de o
/// browser se portar de uma maneira que só se confirmava nas máquinas rápidas.
#[tokio::test]
async fn voltar_atras_depois_do_arranque_nao_prende_a_pessoa() {
    let harness = harness!();
    let (_pessoa, _credenciais) = harness.sign_in(&[TechnicalRole::ResearchMember]).await;

    let page = harness.open("/").await;
    esquecer_o_marcador(&page, &harness.workspace_url).await;

    // Quantas entradas havia antes de atravessar o arranque.
    //
    // # Porque a contagem, e não só o retroceder
    //
    // O resto desta viagem observa o retroceder ao longo de quatro segundos, e
    // isso apanha o defeito quando a corrida corre mal — apanhou-o na CI, e
    // nunca nesta estação, oito execuções seguidas. Uma propriedade que só se
    // manifesta numa máquina mais lenta não está a ser medida: está a ser
    // sorteada.
    //
    // O histórico responde à mesma pergunta sem depender de tempo nenhum.
    // Atravessar o arranque tem de custar **uma** entrada, a do destino. Se o
    // arranque deixar a sua, custa duas — e é essa a entrada onde o retroceder
    // aterra.
    //
    // # O que este guarda prova, e onde
    //
    // Nesta estação ele não pode falhar: o Chrome do macOS já substitui a
    // entrada com 0,6 s, e a contagem dá o mesmo com e sem `location.replace`.
    // Retirei a entrega por `replace` e a contagem continuou a bater — está
    // dito para que ninguém leia o verde local como prova. Onde o browser
    // acrescenta a entrada, que foi o que aconteceu no runner, é aqui que
    // aparece.
    let antes = comprimento_do_historico(&page).await;

    // Ir a um destino profundo passando pelo arranque, e voltar atrás.
    page.goto(format!("{}/calendar", harness.workspace_url))
        .await
        .expect("abrir o calendário");
    for _ in 0..80 {
        let url = page.url().await.ok().flatten().unwrap_or_default();
        if url.ends_with("/calendar") {
            break;
        }
        tokio::time::sleep(Duration::from_millis(120)).await;
    }

    // Esperar que a página esteja estável antes de lhe falar. Avaliar durante a
    // entrega do arranque encontra o contexto a ser substituído, e o Chrome
    // responde «cannot find context» — que é a corrida do observador, e não o
    // produto a falhar.
    let _ = conteudo_estavel(&page).await;

    let depois = comprimento_do_historico(&page).await;
    assert_eq!(
        depois,
        antes + 1,
        "atravessar o arranque custou {} entradas de histórico em vez de uma: o \
         `/boot` ficou na pilha, e é lá que o retroceder vai aterrar",
        depois - antes
    );

    let mut retrocedeu = false;
    for _ in 0..40 {
        if page.evaluate("history.back()").await.is_ok() {
            retrocedeu = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(120)).await;
    }
    assert!(retrocedeu, "não foi possível retroceder no histórico");

    // Onde é que ficou, observado ao longo de quatro segundos.
    //
    // Uma leitura única não chegava: se a entrega ficasse no histórico, o
    // arranque reapareceria e voltaria a empurrar para a frente, e uma leitura
    // feita depois disso encontraria o destino outra vez — o mesmo que se vê
    // quando tudo está bem. É a sequência que distingue as duas coisas.
    let mut visto = Vec::new();
    for _ in 0..34 {
        visto.push(caminho_de(&page).await);
        tokio::time::sleep(Duration::from_millis(120)).await;
    }
    let assentou = visto.last().cloned().unwrap_or_default();

    // Retroceder tem de mudar de sítio. Se o endereço final é o mesmo de onde
    // se partiu, o botão de retroceder não serve para nada: o arranque
    // reapareceu e devolveu a pessoa ao destino.
    assert_ne!(
        assentou, "/calendar",
        "retroceder devolveu a pessoa ao mesmo sítio — o arranque reapareceu no \
         histórico e empurrou-a de volta. A sequência foi: {visto:?}"
    );

    // E nem sequer de passagem: uma reaparição do arranque, mesmo breve, é um
    // ecrã que a pessoa não pediu.
    assert!(
        !visto.iter().any(|c| c == "/boot"),
        "retroceder passou pelo arranque: {visto:?}"
    );
}

/// O caminho da página, sem a cadeia de consulta.
///
/// Comparar endereços inteiros engana: `/boot?return_to=/calendar` termina em
/// `/calendar` e passa por ele em qualquer comparação de sufixo. Custou-me uma
/// reversão que não falhou quando devia.
async fn caminho_de(page: &Page) -> String {
    let url = page.url().await.ok().flatten().unwrap_or_default();
    let sem_esquema = url.split_once("//").map_or(url.as_str(), |(_, r)| r);
    let caminho = sem_esquema.find('/').map_or("/", |i| &sem_esquema[i..]);
    caminho
        .split_once('?')
        .map_or(caminho, |(c, _)| c)
        .to_owned()
}

/// Entra pelo formulário verdadeiro num Workspace indicado.
///
/// O `login_as` do harness fala sempre com o Workspace do harness. As viagens
/// de transição precisam de entrar noutro — o que está apontado ao Core com a
/// prontidão trocada — e os cookies são por origem: uma sessão obtida num porto
/// não vale no outro.
async fn entrar_em(harness: &Harness, workspace_url: &str, credenciais: &Credenciais) -> Page {
    let page = harness
        .browser
        .new_page(format!("{workspace_url}/login"))
        .await
        .expect("página");
    esperar_pelo_login(&page).await;
    set_field(&page, "input[name=username]", &credenciais.username).await;
    set_field(&page, "input[name=password]", &credenciais.password).await;
    submit(&page, "form").await;
    wait_until_left(&page, "/login").await;
    page
}

/// Lê o rótulo do estado do Core na topbar.
///
/// # Espera pelo rótulo, e não por um documento qualquer
///
/// `conteudo_estavel` devolve assim que há conteúdo — e num runner lento isso
/// pode ser o documento a meio, com o `<head>` escrito e a shell ainda não.
/// Lido nesse instante, nenhum dos rótulos existe, e o teste falha a
/// dizer que a barra não diz nada quando o que aconteceu foi termos chegado
/// cedo de mais. Aconteceu na CI, e não aqui.
///
/// A espera é por estado observável: um dos rótulos. Se ao fim do limite
/// não houver nenhum, aí sim é notícia, e a mensagem leva o princípio do
/// documento para se poder ver o que chegou em vez dele.
async fn estado_na_topbar(page: &Page) -> String {
    const ROTULOS: [&str; 3] = ["CORE SEM RESPOSTA", "CORE INDISPONÍVEL", "CORE OK"];

    let inicio = std::time::Instant::now();
    let mut html = String::new();
    while inicio.elapsed() < Duration::from_secs(30) {
        html = conteudo_estavel(page).await;
        for rotulo in ROTULOS {
            if html.contains(rotulo) {
                return rotulo.to_owned();
            }
        }
        tokio::time::sleep(Duration::from_millis(120)).await;
    }
    format!("(nenhum em 30s) · {}", &html[..html.len().min(200)])
}

// ── A barra acompanha o Core, e não a última boa notícia ────────────────

/// A topbar diz o que o Core respondeu agora, e não o que respondeu ao entrar.
///
/// # A travessia
///
/// Uma pessoa entra com tudo bem, fica a trabalhar, e o Core degrada-se por
/// baixo dela. Depois bloqueia. Depois cala-se. A barra tem de acompanhar as
/// quatro coisas, e as duas últimas têm de continuar a ser diferentes uma da
/// outra: um Core que **decidiu** que não pode servir não é um Core que não
/// respondeu.
///
/// # Porque é uma viagem só
///
/// Porque a travessia é a propriedade. Quatro testes independentes provariam
/// quatro estados iniciais, e o defeito que isto procura — a barra ficar presa
/// no que era verdade quando a página abriu pela primeira vez — só aparece
/// quando o estado muda com a sessão já aberta.
#[tokio::test]
async fn a_topbar_acompanha_o_core_ao_longo_da_sessao() {
    let harness = harness!();
    let (_pessoa, credenciais) = harness.sign_in(&[TechnicalRole::ResearchMember]).await;

    let tudo_bem = projeccao("ready", serde_json::json!([]));
    let (core_url, prontidao) = core_com_prontidao_trocada(&harness.core_url, tudo_bem, 200).await;
    let workspace_url = workspace_contra(&harness, &core_url).await;

    let page = entrar_em(&harness, &workspace_url, &credenciais).await;
    assert_eq!(
        estado_na_topbar(&page).await,
        "CORE OK",
        "com o Core pronto, a barra tinha de o dizer"
    );

    // ── Degrada-se ──────────────────────────────────────────────────────
    prontidao.passa_a(
        projeccao(
            "degraded",
            serde_json::json!([{
                "component": "mail",
                "criticality": "optional",
                "state": "unavailable",
                "reason": "o correio não está configurado",
            }]),
        ),
        200,
    );
    page.goto(format!("{workspace_url}/")).await.expect("abrir");

    // As duas metades da propriedade, medidas na mesma travessia.
    //
    // A primeira é o que o Core responde: a instalação **continua** `degraded`,
    // e a asserção está aqui para que ninguém torne o distintivo verde
    // suavizando o Core. Se um dia isto passar a `ready`, este teste diz porquê
    // em vez de passar a aplaudir.
    let resposta: serde_json::Value = reqwest::get(format!("{core_url}/ready"))
        .await
        .expect("pedir a prontidão")
        .json()
        .await
        .expect("ler a prontidão");
    assert_eq!(
        resposta["overall"], "degraded",
        "o cenário deixou de ser `degraded`, e então não prova nada: {resposta}"
    );

    // A segunda é o que a pessoa lê. `degraded` por uma capacidade opcional é
    // uma afirmação sobre a instalação; o distintivo diz **CORE**, e o Core está
    // inteiro — `decide()` devolveria `blocked` se algum crítico faltasse.
    assert_eq!(
        estado_na_topbar(&page).await,
        "CORE OK",
        "o Core está inteiro e a barra apresenta-o como limitado por falta de correio"
    );

    // E com o tratamento visual são: `CORE OK` ao lado de um ponto amarelo seria
    // a mesma imprecisão, dita a meio.
    let barra = conteudo_estavel(&page).await;
    assert!(
        !barra.contains("oc-core-pill--limited") && !barra.contains("oc-core-pill--off"),
        "o distintivo diz `CORE OK` com o indicador de aviso"
    );

    // ── Bloqueia ────────────────────────────────────────────────────────
    prontidao.passa_a(
        projeccao(
            "blocked",
            serde_json::json!([{
                "component": "persistence",
                "criticality": "critical",
                "state": "unavailable",
                "reason": "a base de dados não respondeu",
            }]),
        ),
        503,
    );
    page.goto(format!("{workspace_url}/")).await.expect("abrir");
    assert_eq!(
        estado_na_topbar(&page).await,
        "CORE INDISPONÍVEL",
        "o Core decidiu que não pode servir, e a barra tinha de o dizer"
    );

    // ── Cala-se ─────────────────────────────────────────────────────────
    //
    // Um corpo que não é uma projecção de prontidão. Não é uma decisão do
    // Core; é ausência de decisão, e a barra tem de dizer outra coisa.
    prontidao.passa_a(serde_json::json!({ "isto": "não é uma prontidão" }), 200);
    page.goto(format!("{workspace_url}/")).await.expect("abrir");
    assert_eq!(
        estado_na_topbar(&page).await,
        "CORE SEM RESPOSTA",
        "sem uma decisão do Core, a barra não pode dizer que ele decidiu alguma coisa"
    );
}

// ── Recuperação ─────────────────────────────────────────────────────────

/// Quem ficou preso num arranque bloqueado passa quando o Core recupera.
///
/// # Porque isto precisa de ser provado
///
/// O arranque bloqueado oferece «tentar novamente», e uma tentativa que não
/// mudasse nada seria pior do que não oferecer nada: prometeria uma saída que
/// não existe. Isto atravessa a promessa inteira — bloqueado, o botão, o Core a
/// recuperar entretanto, e a passagem.
#[tokio::test]
async fn um_core_que_recupera_deixa_passar_quem_estava_preso() {
    let harness = harness!();

    let bloqueado = projeccao(
        "blocked",
        serde_json::json!([{
            "component": "persistence",
            "criticality": "critical",
            "state": "unavailable",
            "reason": "a base de dados não respondeu",
        }]),
    );
    let (core_url, prontidao) = core_com_prontidao_trocada(&harness.core_url, bloqueado, 503).await;
    let workspace_url = workspace_contra(&harness, &core_url).await;

    let page = harness
        .browser
        .new_page(format!("{workspace_url}/"))
        .await
        .expect("página");

    // Fica no arranque, e não segue.
    let mut visto = String::new();
    for _ in 0..40 {
        visto = conteudo_estavel(&page).await;
        if visto.contains("oc-boot__retry") {
            break;
        }
        tokio::time::sleep(Duration::from_millis(120)).await;
    }
    assert!(
        visto.contains("oc-boot__retry"),
        "um arranque bloqueado tinha de oferecer tentar novamente: {}",
        &visto[..visto.len().min(400)]
    );
    assert!(
        !tem_marcador(&page).await,
        "um arranque bloqueado não deixa marca"
    );

    // O Core recupera enquanto a pessoa está a olhar para o ecrã.
    prontidao.passa_a(projeccao("ready", serde_json::json!([])), 200);

    submit(&page, "form.oc-boot__actions").await;

    // E agora passa — para o Login, porque não há sessão nenhuma.
    esperar_pelo_login(&page).await;
    assert!(
        tem_marcador(&page).await,
        "o arranque seguiu e tinha de deixar marca"
    );
}

// ── O Capability Runtime, atravessado por uma pessoa ────────────────────

/// Uma pessoa valida e normaliza bibliografia, e o WebAssembly corre por baixo.
///
/// # O caminho que isto atravessa
///
/// ```text
/// Chrome → Workspace → HTTP → Ocinye Core → Capability Runtime → WASM
///        ← Workspace ← HTTP ← Ocinye Core ←
/// ```
///
/// Tudo verdadeiro: o browser é o Chrome da máquina, o Workspace é o router
/// real, o Core é o router real, e o componente é o `.wasm` construído a partir
/// de `wasm/capabilities/bibtex-import`. Nada aqui é simulado — se o
/// isolamento não executar, este teste não passa.
///
/// # Porque não basta um 200
///
/// Porque um 200 diria que o pedido chegou, e não que a leitura aconteceu. O
/// que se procura na página é o **resultado da transformação**: a chave lida, e
/// o campo que entrou em maiúsculas a sair em minúsculas. Só o componente
/// consegue produzir isso.
#[tokio::test]
async fn uma_pessoa_valida_bibliografia_e_o_wasm_corre_por_baixo() {
    let harness = harness!();
    let (pessoa, _credenciais) = harness.sign_in(&[TechnicalRole::ResearchMember]).await;
    let workspace = harness.owns_a_workspace(pessoa).await;

    let pagina = harness.open("/bibliography/tools").await;
    esperar_por(&pagina, "Ferramentas bibliográficas").await;

    // Deliberadamente por normalizar: tipo e campo em maiúsculas, espaçamento
    // irregular. O que sair tem de estar arrumado.
    let bibtex = "@ARTICLE{mucai2024,\n  TITLE = {Vento no Mucai},\n     author={Ana Mucai},\n  year = {2024}\n}";

    escolher(&pagina, "select[name=workspace_id]", &workspace.to_string()).await;
    set_field(&pagina, "textarea[name=bibtex]", bibtex).await;
    submit(&pagina, "form.oc-form").await;

    esperar_por(&pagina, "RESULTADO").await;
    let html = conteudo_estavel(&pagina).await;

    assert!(
        html.contains("mucai2024"),
        "a chave lida não aparece: {}",
        &html[..html.len().min(600)]
    );
    assert!(html.contains("Legível"), "a revisão devia dizer-se legível");

    // A prova de que o componente correu: o que entrou em maiúsculas sai em
    // minúsculas, e alinhado. Nenhuma camada do Workspace faz isto.
    let normalizado = entre(&html, "data-oc=\"normalizado\"", "</textarea>");
    assert!(
        normalizado.contains("@article{mucai2024,"),
        "o tipo devia vir em minúsculas: «{normalizado}»"
    );
    assert!(
        normalizado.contains("title") && !normalizado.contains("TITLE"),
        "o campo devia vir em minúsculas: «{normalizado}»"
    );
}

/// Bibliografia que não se consegue ler é dita, e não fingida.
#[tokio::test]
async fn uma_bibliografia_partida_e_explicada_a_pessoa() {
    let harness = harness!();
    let (pessoa, _credenciais) = harness.sign_in(&[TechnicalRole::ResearchMember]).await;
    let workspace = harness.owns_a_workspace(pessoa).await;

    let pagina = harness.open("/bibliography/tools").await;
    esperar_por(&pagina, "Ferramentas bibliográficas").await;

    escolher(&pagina, "select[name=workspace_id]", &workspace.to_string()).await;
    set_field(
        &pagina,
        "textarea[name=bibtex]",
        "@article{bom, title = {Um}}\n@misc{isto_nao_fecha",
    )
    .await;
    submit(&pagina, "form.oc-form").await;

    esperar_por(&pagina, "RESULTADO").await;
    let html = conteudo_estavel(&pagina).await;

    assert!(
        html.contains("Com problemas"),
        "devia dizer que há problemas"
    );
    assert!(
        html.contains("Não foi possível ler"),
        "devia dizer o que não leu"
    );
    assert!(
        html.contains("bom"),
        "o que se leu devia continuar a aparecer"
    );
}

/// BibTeX hostil aparece como texto, e nunca como marcação.
///
/// # O modelo de ameaça
///
/// O que entra nesta caixa veio de fora — de um sítio, de um correio, de um
/// modelo. É a entrada menos confiável que este ecrã vê, e volta para a página
/// depois de atravessar duas fronteiras. Se em alguma delas alguém decidir que
/// é HTML, o `<script>` corre no browser de quem colou.
#[tokio::test]
async fn bibtex_hostil_aparece_como_texto() {
    let harness = harness!();
    let (pessoa, _credenciais) = harness.sign_in(&[TechnicalRole::ResearchMember]).await;
    let workspace = harness.owns_a_workspace(pessoa).await;

    let pagina = harness.open("/bibliography/tools").await;
    esperar_por(&pagina, "Ferramentas bibliográficas").await;

    let hostil = "@article{x, title = {<script>window.__ocinye_xss = 1;</script>}, year = {2024}}";
    escolher(&pagina, "select[name=workspace_id]", &workspace.to_string()).await;
    set_field(&pagina, "textarea[name=bibtex]", hostil).await;
    submit(&pagina, "form.oc-form").await;

    esperar_por(&pagina, "RESULTADO").await;

    // A prova que interessa não é o HTML: é o browser. Se o script tivesse
    // corrido, a variável existiria.
    let correu: Option<bool> = pagina
        .evaluate("window.__ocinye_xss === 1")
        .await
        .ok()
        .and_then(|v| v.into_value().ok());
    assert_ne!(
        correu,
        Some(true),
        "o `<script>` que veio no BibTeX correu no browser"
    );

    // O `<script>` não ter corrido não chega. Um `<script>` posto na página por
    // `innerHTML` nunca corre — o browser não o executa — e continua a ser
    // conteúdo do BibTeX a virar marcação. A pergunta certa é se o texto que
    // veio no pedido chegou a criar um elemento.
    let virou_elemento: Option<bool> = pagina
        .evaluate("!!document.querySelector('[data-oc=\"revisao\"] script')")
        .await
        .ok()
        .and_then(|v| v.into_value().ok());
    assert_ne!(
        virou_elemento,
        Some(true),
        "o `<script>` que veio no BibTeX virou um elemento na página"
    );

    // E o texto tem de continuar lá, na lista das referências lidas — escapado,
    // não desaparecido. Perguntar isto à página inteira não mede nada: a área de
    // texto do normalizado escapa sempre, por ser uma `<textarea>`, e responderia
    // que sim mesmo com a lista a escrever marcação crua.
    let na_lista: Option<String> = pagina
        .evaluate("document.querySelector('[data-oc=\"revisao\"] ul:last-of-type').textContent")
        .await
        .ok()
        .and_then(|v| v.into_value().ok());
    assert!(
        na_lista
            .as_deref()
            .is_some_and(|t| t.contains("<script>window.__ocinye_xss")),
        "o título hostil devia aparecer como texto na lista das referências, e \
         aparece como {na_lista:?}"
    );
}

/// Sem ambiente onde acrescentar referências, a ferramenta di-lo.
#[tokio::test]
async fn sem_ambiente_a_ferramenta_diz_que_nao_ha_onde_trabalhar() {
    let harness = harness!();
    let (_pessoa, _credenciais) = harness.sign_in(&[TechnicalRole::ResearchMember]).await;

    let pagina = harness.open("/bibliography/tools").await;
    esperar_por(&pagina, "Ferramentas bibliográficas").await;
    let html = conteudo_estavel(&pagina).await;

    assert!(
        html.contains("Sem Research Workspace onde trabalhar"),
        "sem ambiente, a ferramenta tinha de o dizer: {}",
        &html[..html.len().min(500)]
    );
    assert!(
        !html.contains("name=\"bibtex\""),
        "não devia haver caixa onde escrever sem sítio para onde enviar"
    );
}

/// O calendário da barra mostra o mês, e não a agenda.
///
/// # Porque a prova é uma ausência
///
/// Este painel foi um centro de agenda: mostrava os compromissos de hoje, os
/// próximos e os lembretes. Passou a ser apresentação pura por decisão
/// institucional — uma segunda superfície a responder «o que tenho marcado»
/// acaba por discordar da primeira.
///
/// Uma decisão dessas desfaz-se sozinha com o tempo, uma linha de cada vez, se
/// nada a guardar. O que este teste mede é que o painel **não** mostra uma
/// actividade que existe e que a pessoa pode ver — e para isso a actividade tem
/// de existir mesmo, senão a ausência não prova nada.
#[tokio::test]
async fn o_calendario_da_barra_nao_le_a_agenda() {
    let harness = harness!();
    let (_pessoa, _credenciais) = harness.sign_in(&[TechnicalRole::ResearchMember]).await;

    let hoje = chrono::Utc::now().date_naive();
    let titulo = unique_title("Visível na agenda");
    harness.create_event_via_ui(&titulo, hoje, 10).await;

    // O controlo positivo: a actividade existe mesmo e o Calendário mostra-a.
    let calendario = harness
        .open(&format!("/calendar?view=day&on={hoje}"))
        .await
        .content()
        .await
        .expect("conteúdo");
    assert!(
        calendario.contains(&titulo),
        "a actividade não chegou ao Calendário: a ausência no painel não provaria nada"
    );

    let painel = harness.open("/").await;
    clicar(&painel, r#"[data-oc="clock"]"#).await;
    esperar_por(&painel, "Abrir Calendário").await;
    let html = conteudo_estavel(&painel).await;

    assert!(
        !html.contains(&titulo),
        "o calendário da barra mostrou uma actividade: voltou a ler a agenda"
    );

    // E mostra o que lhe compete: o mês corrente e o dia de hoje.
    assert!(
        html.contains(&crate::tempo_mes_e_ano(hoje)),
        "o painel não diz em que mês estamos"
    );
    assert!(
        html.contains("oc-datepop__dia-cel--hoje"),
        "o painel não marca o dia de hoje"
    );
}

/// O calendário da barra abre, fecha e devolve o foco.
#[tokio::test]
async fn o_calendario_da_barra_abre_e_fecha() {
    let harness = harness!();
    let (_pessoa, _credenciais) = harness.sign_in(&[TechnicalRole::ResearchMember]).await;

    let page = harness.open("/").await;

    // Fechado à partida. Um painel que abre sozinho tapa o que a pessoa veio ver.
    let inicial = conteudo_estavel(&page).await;
    assert!(
        inicial.contains(r#"aria-expanded="false""#),
        "o relógio diz-se aberto antes de alguém lhe tocar"
    );

    clicar(&page, r#"[data-oc="clock"]"#).await;
    esperar_por(&page, "Abrir Calendário").await;

    // Escape fecha.
    page.evaluate(
        "document.dispatchEvent(new KeyboardEvent('keydown',{key:'Escape',bubbles:true}))",
    )
    .await
    .expect("escape");
    let fechado = esperar_ate_condicao(
        &page,
        r#"document.querySelector('[data-oc="temporal-centre"]').hasAttribute('hidden')"#,
    )
    .await;
    assert!(fechado, "Escape não fechou o calendário da barra");

    // E o foco volta ao relógio, senão a tecla seguinte vai para lado nenhum.
    let no_relogio = page
        .evaluate(r#"document.activeElement === document.querySelector('[data-oc="clock"]')"#)
        .await
        .ok()
        .and_then(|v| v.into_value::<bool>().ok())
        .unwrap_or(false);
    assert!(no_relogio, "Escape fechou o painel e perdeu o foco");

    // Reabre, e um clique fora fecha.
    clicar(&page, r#"[data-oc="clock"]"#).await;
    esperar_por(&page, "Abrir Calendário").await;
    page.evaluate("document.body.click()").await.expect("fora");
    let fechado = esperar_ate_condicao(
        &page,
        r#"document.querySelector('[data-oc="temporal-centre"]').hasAttribute('hidden')"#,
    )
    .await;
    assert!(fechado, "um clique fora não fechou o calendário da barra");
}

/// Uma pessoa marca uma actividade e ela aparece no dia e na hora certos.
///
/// É a jornada inteira: o editor, o Core a validar e a guardar, e o Calendário
/// a mostrar. Nenhuma das metades prova sozinha o que interessa — um formulário
/// que submete sem erro e um calendário que desenha uma grelha podem ambos
/// estar certos com a actividade a ir parar ao dia errado.
#[tokio::test]
async fn uma_pessoa_marca_uma_actividade_e_ela_aparece_onde_devia() {
    let harness = harness!();
    let (_pessoa, _credenciais) = harness.sign_in(&[TechnicalRole::ResearchMember]).await;

    let hoje = chrono::Utc::now().date_naive();
    let titulo = unique_title("Conselho científico");

    let editor = harness.open("/calendar/events/new").await;
    esperar_por(&editor, "Nova actividade").await;

    set_field(&editor, "input[name=title]", &titulo).await;
    set_field(&editor, "input[name=location]", "Sala do conselho").await;
    set_field(&editor, "input[name=starts_at]", &format!("{hoje}T14:00")).await;
    set_field(&editor, "input[name=ends_at]", &format!("{hoje}T15:30")).await;
    submit(&editor, "form.oc-editor__form").await;

    let destino = wait_until_left(&editor, "/calendar/events/new").await;
    assert!(
        destino.contains("/calendar/events/"),
        "marcar não levou a lado nenhum: {destino}"
    );

    // No Dia, à hora certa: o bloco tem de estar na faixa das 14:00, que na
    // grelha de meias-horas é a linha 28.
    let dia = harness
        .open(&format!("/calendar?view=day&on={hoje}"))
        .await
        .content()
        .await
        .expect("conteúdo");
    assert!(dia.contains(&titulo), "a actividade não aparece no Dia");
    // A faixa tem de corresponder à hora que a própria página mostra.
    //
    // # Porque não se compara com «14:00»
    //
    // Porque foi o que fiz primeiro, e falhou: a hora que se escreve no editor é
    // interpretada numa zona, e o Calendário desenha a partir do instante. As
    // duas coincidem ou não conforme a zona de quem marca — e um teste que
    // dependa disso passa numa máquina e falha noutra sem nada estar errado.
    //
    // A propriedade que interessa é interna: **o bloco está onde a sua própria
    // etiqueta diz que está**. Se alguém mudar a aritmética da colocação, a
    // etiqueta e a posição deixam de bater certo, e é isso que se apanha aqui.
    // A fronteira é a etiqueta que abre o bloco, e não o nome da classe: partir
    // por `oc-cal-bloco` parte também dentro de `oc-cal-bloco__hora`, porque um
    // é prefixo do outro, e a hora fica no pedaço anterior ao do título.
    let bloco = dia
        .split(r#"<a class="oc-cal-bloco"#)
        .find(|p| p.contains(&titulo))
        .unwrap_or_default();

    let mostrada = bloco
        .split(r#"oc-cal-bloco__hora">"#)
        .nth(1)
        .and_then(|p| p.split('<').next())
        .unwrap_or_default()
        .trim()
        .to_owned();
    assert!(
        mostrada.len() == 5 && mostrada.contains(':'),
        "o bloco não mostra uma hora legível: «{mostrada}»"
    );

    let (h, m) = mostrada.split_once(':').expect("hora e minuto");
    let faixa = h.parse::<usize>().expect("hora") * 2
        + usize::from(m.parse::<usize>().expect("minuto") >= 30);
    assert!(
        bloco.contains(&format!("oc-cal-l{faixa} ")),
        "o bloco diz «{mostrada}» e está na faixa errada: esperava a {faixa}"
    );

    // E no Mês, na célula do dia certo.
    let mes = harness
        .open(&format!("/calendar?view=month&on={hoje}"))
        .await
        .content()
        .await
        .expect("conteúdo");
    let celula = mes
        .split(r#"<a href="/calendar?view=today&amp;on="#)
        .find(|c| c.contains(&titulo))
        .unwrap_or_default();
    assert_eq!(
        &celula[..10.min(celula.len())],
        hoje.to_string().as_str(),
        "a actividade caiu na célula errada do Mês"
    );
}

/// Uma pessoa marca uma actividade com participantes, e tudo fica no sítio.
///
/// A jornada inteira: o editor abre com um horário que se aceita, procura-se
/// alguém pelo nome, escolhe-se, e o Core valida, autoriza e guarda. Nenhuma
/// metade prova o que interessa sozinha — um formulário que submete sem erro e
/// uma actividade que aparece podem ambos estar certos com o participante a não
/// ter chegado a lado nenhum.
#[tokio::test]
async fn uma_pessoa_marca_com_participantes() {
    let harness = harness!();
    let (_pessoa, credenciais) = harness.sign_in(&[TechnicalRole::ResearchMember]).await;

    // Alguém para convidar. A segunda entrada substitui a sessão, portanto
    // volta-se à primeira antes de marcar.
    let (_outra, _) = harness.sign_in(&[TechnicalRole::ResearchMember]).await;
    harness.login_as(&credenciais).await;

    let editor = harness.open("/calendar/events/new").await;
    esperar_por(&editor, "Nova actividade").await;

    // O horário já vem preenchido: é o que separa escrever o título de tomar
    // quatro decisões.
    let inicio = valor_de(&editor, "input[name=starts_at]").await;
    let fim = valor_de(&editor, "input[name=ends_at]").await;
    assert!(
        inicio.len() >= 16 && fim.len() >= 16,
        "o editor abriu com os campos temporais vazios: «{inicio}» e «{fim}»"
    );
    assert_ne!(
        inicio, fim,
        "o início e o fim propostos são o mesmo instante"
    );

    // Um participante, escolhido pela procura.
    let titulo = unique_title("Conselho com participantes");
    set_field(&editor, "input[name=title]", &titulo).await;

    let escolheu = editor
        .evaluate(
            "(() => { const b = document.querySelector('[data-oc=\"pessoa\"]');              if (!b) return false; b.click(); return true; })()",
        )
        .await
        .ok()
        .and_then(|v| v.into_value::<bool>().ok())
        .unwrap_or(false);
    assert!(escolheu, "não havia ninguém para convidar");

    let escolhidos = editor
        .evaluate("document.querySelectorAll('input[name=participants]').length")
        .await
        .ok()
        .and_then(|v| v.into_value::<i64>().ok())
        .unwrap_or(0);
    assert_eq!(
        escolhidos, 1,
        "o participante escolhido não entrou no pedido"
    );

    submit(&editor, "form.oc-editor__form").await;
    let destino = wait_until_left(&editor, "/calendar/events/new").await;
    assert!(
        destino.contains("/calendar/events/"),
        "marcar com participantes não levou a lado nenhum: {destino}"
    );

    // E a actividade existe, com o título que se escreveu.
    //
    // Espera-se **pelo título**, e não por conteúdo genérico.
    //
    // Estava `content()` imediatamente a seguir à navegação, e apanhava a página
    // a meio: passava aqui e falhava na CI, onde o runner é mais lento e a
    // janela se abre. Esperar por conteúdo estável corrigiria o sintoma;
    // esperar pelo que se procura é a observação certa — se o título não
    // aparecer dentro do limite, é notícia, e a mensagem di-lo.
    esperar_por(&editor, &titulo).await;
    let detalhe = conteudo_estavel(&editor).await;
    assert!(
        detalhe.contains(&titulo),
        "a actividade criada não é a que se pediu"
    );
}

/// Lê o valor de um campo, como o browser o tem.
async fn valor_de(page: &Page, seletor: &str) -> String {
    for _ in 0..40 {
        if let Ok(valor) = page
            .evaluate(format!(
                "(document.querySelector('{seletor}') || {{}}).value || ''"
            ))
            .await
        {
            if let Ok(texto) = valor.into_value::<String>() {
                if !texto.is_empty() {
                    return texto;
                }
            }
        }
        tokio::time::sleep(Duration::from_millis(120)).await;
    }
    String::new()
}

/// Espera que uma expressão passe a ser verdadeira na página.
async fn esperar_ate_condicao(page: &Page, expressao: &str) -> bool {
    for _ in 0..40 {
        if let Ok(valor) = page.evaluate(expressao).await {
            if valor.into_value::<bool>().unwrap_or(false) {
                return true;
            }
        }
        tokio::time::sleep(Duration::from_millis(120)).await;
    }
    false
}

/// O mesmo rótulo de mês que a Experience escreve, para o teste não o repetir.
fn tempo_mes_e_ano(data: chrono::NaiveDate) -> String {
    ocinye_workspace::ui::tempo::mes_e_ano(data)
}

// ═══════════════════════════════════════════════════════════════════════════
// Capturas para revisão visual
// ═══════════════════════════════════════════════════════════════════════════
//
// Estas entradas não afirmam nada. Levantam a stack, preparam um cenário
// determinado, navegam e gravam PNGs para alguém olhar — o portão de qualidade
// premium é humano, e nenhuma asserção o substitui.
//
// # Porque vivem aqui e não numa ferramenta à parte
//
// Porque o harness acima carrega três incidentes já resolvidos: o perfil por
// browser, o limite de concorrência, e o lugar do semáforo preso ao `Drop` em
// vez de ao construtor. Uma ferramenta paralela herdava-os outra vez.
//
// # Porque são `#[ignore]`
//
// Porque gravam ficheiros. Uma verificação que escreve no disco a cada execução
// é uma verificação que ninguém corre à vontade, e o `verify.sh` prova
// comportamento — não é o sítio onde se decide se uma grelha está bonita.
//
// Correr com `./scripts/capturas.sh`.

/// Onde as capturas ficam. Fora da árvore versionada, sempre.
fn pasta_das_capturas() -> std::path::PathBuf {
    let destino = std::env::var("OCINYE_TEST_CAPTURAS_DIR")
        .unwrap_or_else(|_| "/tmp/ocinye-capturas".to_owned());
    std::fs::create_dir_all(&destino).expect("criar a pasta das capturas");
    std::path::PathBuf::from(destino)
}

/// A janela que estas capturas assumem.
///
/// O `chromiumoxide` abre a 800×600 por omissão, e foi assim que as primeiras
/// capturas saíram: um mês onde só cabiam três semanas. Isso não é o Calendário
/// a estar mal desenhado, é a janela a não ser a de ninguém.
///
/// O Ocinye é uma estação de trabalho. 1440×900 é o tamanho onde a decisão de
/// densidade tem de ser tomada, e é contra ele que a revisão visual acontece.
const JANELA: (i64, i64) = (1440, 900);

/// Grava a página inteira, e diz onde ficou.
async fn capturar(page: &Page, nome: &str) {
    capturar_com(page, nome, JANELA, true).await;
}

/// Só o que cabe na janela.
///
/// A página inteira é o que se quer quase sempre, mas uma vista alta — o Ano
/// são doze meses — dá uma imagem tão comprida que deixa de se poder olhar
/// para ela. Aqui o enquadramento é o da janela, que é o enquadramento de quem
/// usa.
async fn capturar_visivel(page: &Page, nome: &str) {
    capturar_com(page, nome, JANELA, false).await;
}

/// O mesmo, numa janela escolhida — para ver o que acontece quando aperta.
async fn capturar_em(page: &Page, nome: &str, janela: (i64, i64)) {
    capturar_com(page, nome, janela, true).await;
}

async fn capturar_com(page: &Page, nome: &str, janela: (i64, i64), inteira: bool) {
    use chromiumoxide::cdp::browser_protocol::emulation::SetDeviceMetricsOverrideParams;

    // As métricas são por página e não no harness: as viagens de browser
    // partilham-no, e uma janela diferente mudaria o que elas observam.
    page.execute(
        SetDeviceMetricsOverrideParams::builder()
            .width(janela.0)
            .height(janela.1)
            // Um píxel por píxel. A dois, uma janela de 1440 dá uma imagem de 2880 —
            // mais nítida e larga de mais para caber em qualquer visualizador de
            // revisão, e uma captura que não se consegue abrir não é um portão.
            .device_scale_factor(1.0)
            .mobile(false)
            .build()
            .expect("métricas da janela"),
    )
    .await
    .expect("aplicar as métricas");

    // Deixar a página assentar. Uma captura tirada a meio de uma transição
    // mostra um estado que ninguém vê, e é pior do que não a ter: leva a
    // corrigir um problema que não existe.
    let _ = conteudo_estavel(page).await;
    tokio::time::sleep(Duration::from_millis(400)).await;

    let caminho = pasta_das_capturas().join(format!("{nome}.png"));
    page.save_screenshot(
        chromiumoxide::page::ScreenshotParams::builder()
            .full_page(inteira)
            .build(),
        &caminho,
    )
    .await
    .unwrap_or_else(|erro| panic!("gravar {nome}: {erro}"));
    println!("CAPTURA {}", caminho.display());
}

/// O estado actual do Calendário, antes de lhe tocar.
///
/// Serve a auditoria visual: é contra estas imagens que se compara o que vier
/// a seguir.
#[tokio::test]
#[ignore = "grava ficheiros; serve a revisão visual, não a verificação"]
async fn capturas_do_calendario() {
    // O mesmo macro das viagens: se faltar a base ou o Chrome, sai a dizer
    // porquê. Aqui isso é uma conveniência legítima — não há cobertura a
    // afirmar, há imagens a produzir.
    let harness = harness!();
    let (pessoa, _credenciais) = harness.sign_in(&[TechnicalRole::ResearchMember]).await;

    // Um ambiente de investigação, para o âmbito não pessoal ter destino. Sem
    // ele o selector não oferece a opção — e é assim que deve ser.
    let _ambiente = harness.owns_a_workspace(pessoa).await;

    let hoje = chrono::Utc::now().date_naive();

    // Vazio primeiro: é o estado que mais depressa denuncia uma grelha que não
    // existe, porque não há eventos a disfarçar a estrutura.
    for (vista, nome) in [
        ("month", "01-mes-vazio"),
        ("week", "02-semana-vazia"),
        ("agenda", "03-agenda-vazia"),
    ] {
        let page = harness.open(&format!("/calendar?view={vista}")).await;
        capturar(&page, nome).await;
    }

    // Agora com conteúdo, incluindo um dia carregado: a densidade é onde as
    // grelhas más se partem.
    harness
        .create_event_via_ui("Reunião do conselho", hoje, 9)
        .await;
    harness
        .create_event_via_ui("Revisão de bibliografia", hoje, 11)
        .await;
    harness
        .create_event_via_ui("Seminário de investigação", hoje, 14)
        .await;
    harness
        .create_event_via_ui("Ponto de situação", hoje, 15)
        .await;
    harness
        .create_event_via_ui("Defesa de projecto", hoje + chrono::Duration::days(3), 10)
        .await;
    harness
        .create_event_via_ui("Entrega do relatório", hoje + chrono::Duration::days(9), 16)
        .await;

    for (vista, nome) in [
        ("month", "04-mes-com-eventos"),
        ("week", "05-semana-com-eventos"),
        ("day", "06-dia-com-eventos"),
        ("agenda", "07-agenda-com-eventos"),
    ] {
        let page = harness.open(&format!("/calendar?view={vista}")).await;
        capturar(&page, nome).await;
    }

    // Um dia escolhido que não é hoje: os dois estados têm de se distinguir.
    let outro = hoje + chrono::Duration::days(3);
    let page = harness
        .open(&format!("/calendar?view=month&on={outro}"))
        .await;
    capturar(&page, "09-mes-dia-escolhido").await;

    // E o caso que os confunde: escolhido **é** hoje.
    let page = harness
        .open(&format!("/calendar?view=month&on={hoje}"))
        .await;
    capturar(&page, "10-mes-escolhido-e-hoje").await;

    // Densidade. Uma célula não pode crescer sem fim nem esconder o que não
    // coube: passado o limite conta-se o resto, e o resto abre-se no dia.
    for hora in [8, 9, 10, 11, 12, 13, 14] {
        harness
            .create_event_via_ui(&format!("Compromisso das {hora}"), hoje, hora)
            .await;
    }
    let page = harness.open("/calendar?view=month").await;
    capturar(&page, "11-mes-densidade").await;

    // O Ano, com o mesmo conjunto de actividades.
    let page = harness.open("/calendar?view=year").await;
    capturar_visivel(&page, "14-ano").await;

    // E numa janela apertada, que é onde a densidade se parte.
    let page = harness.open("/calendar?view=month").await;
    capturar_em(&page, "12-mes-janela-estreita", (1024, 720)).await;

    // A Agenda com conteúdo, agrupada.
    let page = harness.open("/calendar?view=agenda").await;
    capturar(&page, "15-agenda-com-eventos").await;

    // O calendário do sistema, aberto a partir do relógio.
    let inicio = harness.open("/").await;
    clicar(&inicio, r#"[data-oc="clock"]"#).await;
    esperar_por(&inicio, "Abrir Calendário").await;
    capturar_visivel(&inicio, "16-popover-do-sistema").await;

    // Alguém para convidar, para a secção de participantes existir.
    let (_convidada, _) = harness.sign_in(&[TechnicalRole::ResearchMember]).await;
    harness.login_as(&_credenciais).await;

    // O editor de actividade, tal como abre: já com horário proposto.
    let editor = harness.open("/calendar/events/new").await;
    capturar(&editor, "17-editor-vazio").await;

    // A procura de participantes aberta.
    set_field(&editor, r#"[data-oc="procura-pessoa"]"#, "a").await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    capturar(&editor, "22-editor-procura-participante").await;

    // E com participantes escolhidos.
    let _ = editor
        .evaluate(
            "document.querySelectorAll('[data-oc=\"pessoa\"]').forEach((b, i) => { if (i < 2) b.click(); })",
        )
        .await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    capturar(&editor, "23-editor-participantes").await;

    // Preenchido.
    set_field(&editor, "input[name=title]", "Reunião do conselho").await;
    set_field(
        &editor,
        "textarea[name=description]",
        "Ponto de situação trimestral.",
    )
    .await;
    set_field(&editor, "input[name=location]", "Sala do conselho").await;
    set_field(&editor, "input[name=starts_at]", &format!("{hoje}T09:00")).await;
    set_field(&editor, "input[name=ends_at]", &format!("{hoje}T10:30")).await;
    capturar(&editor, "18-editor-preenchido").await;

    // Dia inteiro: as horas saem, os dias entram.
    clicar(&editor, r#"[data-oc="all-day"]"#).await;
    tokio::time::sleep(Duration::from_millis(200)).await;
    capturar(&editor, "19-editor-dia-inteiro").await;

    // Âmbito não pessoal: aparece o selector que corresponde, e só esse.
    clicar(&editor, r#"[data-oc="all-day"]"#).await;
    // O âmbito institucional existe sempre — não depende de a pessoa ter
    // unidades ou ambientes, e por isso é o que serve para mostrar a secção
    // com uma escolha diferente de «Pessoal» em qualquer instalação.
    escolher(&editor, r#"[data-oc="scope"]"#, "institution").await;
    tokio::time::sleep(Duration::from_millis(200)).await;
    capturar(&editor, "20-editor-ambito-nao-pessoal").await;

    // E numa janela estreita.
    let estreito = harness.open("/calendar/events/new").await;
    capturar_em(&estreito, "21-editor-janela-estreita", (1024, 720)).await;
}

/// O Ano abre. Um ano inteiro não é um pedido que o Core recuse.
///
/// # Porque esta viagem existe
///
/// Os testes de unidade provam que o intervalo cabe no tecto do Core. Não
/// provam que o Core o aceita: são duas afirmações sobre lados diferentes da
/// fronteira, e a segunda só se observa atravessando-a.
///
/// O Ano pedia 367 dias — 366 do ano bissexto mais as margens de fuso — e o
/// Core recusava com `422`. A vista mostrava «Não foi possível ler a agenda»,
/// que é a mensagem de uma leitura falhada, quando o pedido é que era
/// impossível. Passou por todos os portões e apareceu no browser.
#[tokio::test]
async fn o_ano_inteiro_nao_e_um_pedido_impossivel() {
    let harness = harness!();
    let (_pessoa, _credenciais) = harness.sign_in(&[TechnicalRole::ResearchMember]).await;

    // Um ano bissexto, que é o que aperta o tecto.
    for ano in [2026, 2028] {
        let html = harness
            .open(&format!("/calendar?view=year&on={ano}-06-15"))
            .await
            .content()
            .await
            .expect("conteúdo");

        assert!(
            !html.contains("Não foi possível ler a agenda"),
            "o Ano de {ano} não abriu"
        );
        assert_eq!(
            html.matches("oc-cal-mini\"").count(),
            12,
            "o Ano de {ano} não mostrou doze meses"
        );
    }
}
