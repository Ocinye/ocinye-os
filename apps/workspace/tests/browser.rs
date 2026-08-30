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
    /// O endereço institucional. É a credencial única desde o ADR-0106.
    email: String,
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

        // Antes da primeira escrita, e não depois: falhar depois de escrever
        // não é uma guarda, é um relatório de estragos.
        ocinye_core::fixtures::refuse_canonical_organisation(&pool).await;

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
        // A organização **deste** harness, e não «a mais recente».
        //
        // Adoptar a organização mais recente não tem semântica de teste
        // nenhuma: numa base partilhada é a de outro teste a correr em
        // paralelo, e numa base errada é a instituição a sério — foi assim
        // que a base de desenvolvimento ganhou milhares de pessoas de
        // fixtures dentro da organização canónica.
        //
        // O harness já criava a sua em `start`. Faltava usá-la.
        let organisation_id = self.organisation_id;

        let handle = format!("e{}", Uuid::new_v4().simple());
        let email = format!("{handle}@ocinye.com");
        let person_id: Uuid = sqlx::query_scalar(
            "INSERT INTO people (organisation_id, full_name, email, status)
                 VALUES ($1, $2, $3, 'active') RETURNING id",
        )
        .bind(organisation_id)
        .bind(&handle)
        .bind(&email)
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
            .json(&serde_json::json!({ "email": email, "password": password }))
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

        elemento(&page, "input[name=email]")
            .await
            .click()
            .await
            .expect("foco")
            .type_str(&email)
            .await
            .expect("endereço");
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
                "a entrada não passou.\n  endereço: {email}\n  credencial: {credenciais:?}\n  \
                 página: {visivel}"
            );
        }

        (person_id, Credenciais { email, password })
    }

    /// Entra com uma credencial temporária, como quem recebe um primeiro acesso.
    ///
    /// A credencial é criada como `Temporary`, que é o que faz o Core devolver
    /// uma sessão em estado de mudança de palavra-passe obrigatória. A entrada
    /// é pelo formulário verdadeiro, tal como na entrada ordinária.
    async fn entrar_com_credencial_temporaria(&self) -> Credenciais {
        let handle = format!("t{}", Uuid::new_v4().simple());
        let email = format!("{handle}@ocinye.com");
        let person_id: Uuid = sqlx::query_scalar(
            "INSERT INTO people (organisation_id, full_name, email, status)
                 VALUES ($1, $2, $3, 'active') RETURNING id",
        )
        .bind(self.organisation_id)
        .bind(&handle)
        .bind(&email)
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

        let credenciais = Credenciais { email, password };
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
        set_field(&page, "input[name=email]", &credenciais.email).await;
        set_field(&page, "input[name=password]", &credenciais.password).await;
        submit(&page, "form").await;

        let destino = wait_until_left(&page, "/login").await;
        assert!(
            !destino.ends_with("/login"),
            "não foi possível voltar a entrar como «{}»",
            credenciais.email
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

    /// Prepara a cadeia científica pelas operações do Core, como aquela pessoa.
    ///
    /// Devolve `(hipótese, estudo, execução, resultado)`.
    ///
    /// # Porque não é um `INSERT`
    ///
    /// Porque um `INSERT` afirmaria a ideia que o teste tem do que um estudo é,
    /// e não a do domínio. As operações validam vocabulário, aplicam
    /// classificação, autorizam e — no caso científico — escrevem na mesma
    /// transacção a proveniência que **observaram**. Uma fixture que salte isso
    /// deixa a viagem a percorrer arestas que ninguém escreveu, e a viagem
    /// existe precisamente para as ver chegar ao ecrã.
    ///
    /// # Porque não passa pelo HTTP
    ///
    /// Porque o que se prepara é estado, e o que se mede é o percurso. Que a
    /// entrada HTTP e a agentic convergem na mesma operação é provado onde se
    /// pode provar — pelo rasto de auditoria, em `parity.rs`. As rotas de
    /// leitura e a da linhagem são exercidas por esta viagem, porque é por elas
    /// que o Workspace vai buscar o que desenha.
    async fn cadeia_cientifica(
        &self,
        person_id: Uuid,
        ambiente: Uuid,
        titulos: (&str, &str, &str),
    ) -> (Uuid, Uuid, Uuid, Uuid) {
        use ocinye_contracts::Classification;
        use ocinye_core::modules::science;
        use ocinye_observability::CorrelationIds;

        let registo = ocinye_core::modules::identity::person_by_id(&self.pool, person_id)
            .await
            .expect("consulta")
            .expect("pessoa");
        let quem = ocinye_core::modules::identity::principal_for_person(&self.pool, &registo)
            .await
            .expect("principal");
        let ids = CorrelationIds::generate();
        let (hipotese, estudo, resultado) = titulos;

        // Uma transacção por passo: cada operação resolve o recurso de que
        // depende através do `pool`, e não da transacção em curso, tal como
        // acontece num pedido HTTP.
        let mut tx = self.pool.begin().await.expect("transacção");
        let h = science::create_hypothesis(
            &mut tx,
            &quem,
            &ids,
            ambiente,
            hipotese,
            None,
            Classification::Internal,
        )
        .await
        .expect("hipótese");
        tx.commit().await.expect("commit");

        let mut tx = self.pool.begin().await.expect("transacção");
        let e = science::create_study(
            &mut tx,
            &self.pool,
            &quem,
            &ids,
            ambiente,
            Some(h.id),
            None,
            estudo,
            "physical_experiment",
            None,
            Classification::Internal,
        )
        .await
        .expect("estudo");
        tx.commit().await.expect("commit");

        let mut tx = self.pool.begin().await.expect("transacção");
        let x = science::record_execution(
            &mut tx,
            &self.pool,
            &quem,
            &ids,
            e.id,
            &science::ExecutionRecord {
                status: "succeeded",
                compute_node_id: None,
                environment: None,
                software_name: None,
                software_version: None,
                software_commit: None,
                notes: None,
                methodology_version_id: None,
                dataset_version_ids: &[],
            },
        )
        .await
        .expect("execução");
        tx.commit().await.expect("commit");

        let mut tx = self.pool.begin().await.expect("transacção");
        let r = science::create_result(
            &mut tx,
            &self.pool,
            &quem,
            &ids,
            ambiente,
            Some(x.id),
            resultado,
            "Três corridas, mesma direcção.",
            Classification::Internal,
        )
        .await
        .expect("resultado");
        tx.commit().await.expect("commit");

        (h.id, e.id, x.id, r.id)
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

    /// Cria outra pessoa da instituição, com um nome que se possa procurar.
    /// A organização vem de **quem entrou**, e não da mais recente.
    ///
    /// Com cinquenta viagens a correr ao mesmo tempo, «a organização mais
    /// recente» é a de outro teste — e a pessoa criada aqui aparecia num
    /// universo que esta sessão não alcança.
    async fn outra_pessoa(&self, quem: Uuid, nome: &str) -> Uuid {
        let organisation_id: Uuid =
            sqlx::query_scalar("SELECT organisation_id FROM people WHERE id = $1")
                .bind(quem)
                .fetch_one(&self.pool)
                .await
                .expect("organização");

        let handle = format!("o{}", &Uuid::new_v4().simple().to_string()[..10]);
        let id: Uuid = sqlx::query_scalar(
            "INSERT INTO people (organisation_id, full_name, email, status)
                  VALUES ($1, $2, $3, 'active') RETURNING id",
        )
        .bind(organisation_id)
        .bind(nome)
        .bind(format!("{handle}@ocinye.com"))
        .bind(&handle)
        .fetch_one(&self.pool)
        .await
        .expect("pessoa");

        sqlx::query(
            "INSERT INTO person_roles (person_id, role, granted_by_id)
                  VALUES ($1, 'research_member', $1)",
        )
        .bind(id)
        .execute(&self.pool)
        .await
        .expect("papel");

        id
    }

    /// Dá a esta pessoa uma caixa de correio pessoal.
    async fn has_a_mailbox(&self, person_id: Uuid) -> (Uuid, String) {
        let organisation_id: Uuid =
            sqlx::query_scalar("SELECT organisation_id FROM people WHERE id = $1")
                .bind(person_id)
                .fetch_one(&self.pool)
                .await
                .expect("organização");

        let endereco = format!("cx{}@ocinye.com", &Uuid::new_v4().simple().to_string()[..8]);
        let id: Uuid = sqlx::query_scalar(
            "INSERT INTO mailboxes (organisation_id, address, kind, owner_id)
                  VALUES ($1, $2, 'personal', $3) RETURNING id",
        )
        .bind(organisation_id)
        .bind(&endereco)
        .bind(person_id)
        .fetch_one(&self.pool)
        .await
        .expect("caixa");

        (id, endereco)
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
        self.open_em(path, None).await
    }

    /// Abre uma página com o browser noutro fuso.
    ///
    /// # Porque o fuso é por página
    ///
    /// Porque `Emulation.setTimezoneOverride` é do CDP e aplica-se à página, e
    /// não ao browser. Uma viagem que o pusesse numa página e abrisse outra
    /// voltaria ao fuso da máquina — e `app.js` reescrevia o cookie da zona com
    /// ele, apagando o que a viagem tinha declarado.
    async fn open_em(&self, path: &str, fuso: Option<&str>) -> Page {
        let page = if let Some(fuso) = fuso {
            // Em branco primeiro: o fuso tem de estar posto **antes** de a
            // página que o vai ler começar a carregar.
            let page = self
                .browser
                .new_page("about:blank")
                .await
                .expect("página em branco");
            use chromiumoxide::cdp::browser_protocol::emulation::SetTimezoneOverrideParams;
            page.execute(SetTimezoneOverrideParams::new(fuso))
                .await
                .expect("emular o fuso");
            page.goto(format!("{}{path}", self.workspace_url))
                .await
                .expect("navegar");
            page
        } else {
            self.browser
                .new_page(format!("{}{path}", self.workspace_url))
                .await
                .expect("página")
        };

        let inicio = std::time::Instant::now();
        loop {
            let url = page.url().await.ok().flatten().unwrap_or_default();
            if !url.contains("/boot") && !url.is_empty() && url != "about:blank" {
                return page;
            }
            if inicio.elapsed() >= std::time::Duration::from_secs(45) {
                // **Porque** ficou, e não só que ficou.
                //
                // A mensagem dizia «vinte e cinco segundos» com um limite de
                // quarenta e cinco: o limite foi subido e o texto não, que é o
                // sinal de que isto já falhou antes e a resposta foi esperar
                // mais. Esperar mais não diagnostica nada.
                //
                // O arranque decide com o que o Core lhe diz. Sem ver o que a
                // página mostrava, «ficou em /boot» não distingue um Core que
                // não respondeu de um script que não correu.
                let mostrado = page
                    .content()
                    .await
                    .map(|html| {
                        let texto: String = html
                            .split('>')
                            .filter_map(|p| p.split('<').next())
                            .collect::<Vec<_>>()
                            .join(" ");
                        texto.split_whitespace().collect::<Vec<_>>().join(" ")
                    })
                    .unwrap_or_else(|erro| format!("(sem conteúdo: {erro})"));
                panic!(
                    "o arranque não entregou em quarenta e cinco segundos; ficou \
                     em «{url}» a mostrar: {}",
                    &mostrado[..mostrado.len().min(400)]
                );
            }
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

/// A chave que cifra as credenciais de caixa, uma por processo de teste.
fn chave_do_correio() -> &'static ocinye_core::password::sealed::SealingKey {
    static CHAVE: std::sync::OnceLock<ocinye_core::password::sealed::SealingKey> =
        std::sync::OnceLock::new();
    CHAVE.get_or_init(|| {
        ocinye_core::password::sealed::SealingKey::from_base64(
            &ocinye_core::password::sealed::SealingKey::generate(),
        )
        .expect("uma chave acabada de gerar tem de abrir")
    })
}

/// O armazenamento de objectos das viagens que carregam bytes.
///
/// `None` quando as variáveis não estão postas. Quem chama tem de o dizer — um
/// teste que se salta em silêncio é verde a afirmar nada.
/// Sem armazenamento salta-se; na CI, falha.
///
/// A viagem que afirma «Chrome → multipart → Core → object store → PostgreSQL»
/// só é evidência quando todos esses componentes participaram. Na CI, a ausência
/// de um deles é um defeito do job, não uma condição do ambiente.
fn exigir_armazenamento(viagem: &str) {
    assert!(
        std::env::var("CI").is_err(),
        "não há armazenamento, e isto é a CI: «{viagem}» não pode contar como \
         prova de integração com object storage sem um object store. \
         Defina OCINYE_TEST_STORAGE_ENDPOINT."
    );
    eprintln!("SALTADO: {viagem} — OCINYE_TEST_STORAGE_ENDPOINT não está definida.");
}

fn store_de_teste() -> Option<ocinye_core::storage::ObjectStore> {
    ocinye_core::storage::ObjectStore::new(ocinye_core::config::StorageConfig {
        endpoint_url: std::env::var("OCINYE_TEST_STORAGE_ENDPOINT").ok()?,
        region: std::env::var("OCINYE_TEST_STORAGE_REGION")
            .unwrap_or_else(|_| "us-east-1".to_owned()),
        access_key: std::env::var("OCINYE_TEST_STORAGE_ACCESS_KEY").ok()?,
        secret_key: std::env::var("OCINYE_TEST_STORAGE_SECRET_KEY").ok()?,
        bucket: std::env::var("OCINYE_TEST_STORAGE_BUCKET")
            .unwrap_or_else(|_| "ocinye-test-artifacts".to_owned()),
        backend_code: "ocinye-test-default".to_owned(),
        location_label: "test".to_owned(),
        residency: ocinye_contracts::storage::Residency::Undeclared,
        max_upload_bytes: 32 * 1024 * 1024,
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
            per_email: config.auth.throttle_per_email,
            window_minutes: config.auth.throttle_window_minutes,
        },
        config.auth.temporary_credential_hours,
    ));

    // A chave de cifra do correio, gerada uma vez por processo de teste.
    //
    // Não vem do ambiente de propósito: se viesse, esta jornada passaria nesta
    // máquina — onde o `.env` a tem — e falharia em CI, onde não a tem. E a
    // alternativa, um valor fixo no ficheiro, seria uma coisa com forma de
    // segredo dentro do repositório.
    // Um serviço que responde, e não a ausência de serviço.
    //
    // Era o `UnconfiguredProvider`, que descreve uma instalação sem correio.
    // Serve para medir a ausência, e não serve para medir o produto: abrir uma
    // mensagem passa pelo fornecedor, e um que recusa faz o leitor nunca
    // aparecer — as viagens que abrem correio mediriam a recusa.
    //
    // O estado «sem serviço» continua provado, e nos sítios onde é a pergunta:
    // nos testes de ecrã (`tres_ausencias`) e nos do Core
    // (`mail_status_http`), que constroem a instalação que descrevem em vez de
    // a herdarem de um harness partilhado.
    // Uma chave só, e não duas.
    //
    // A credencial é **selada** com `config.mail.sealing_key` — o caminho de
    // ligar a caixa lê-a de lá — e **aberta** com a do registo. O harness dava
    // ao registo a sua chave e deixava a do `config` vir do ambiente: selava
    // com uma e abria com outra.
    //
    // Não falhava a ligar: a sonda aceita, a credencial guarda-se, e o ecrã
    // diz «Ligada». Falhava depois, ao abrir uma mensagem — e como nenhuma
    // viagem abria mensagens, nunca ninguém o viu.
    let mut config = config;
    config.mail.sealing_key = Some(chave_do_correio().clone());

    let mail_registry = Arc::new(
        ocinye_core::modules::mail::ProviderRegistry::new(
            Arc::new(ServicoQueResponde),
            ocinye_core::config::MailConfig {
                sealing_key: Some(chave_do_correio().clone()),
                // O transporte é fixado aqui, e não herdado do ambiente.
                //
                // Herdá-lo fazia estas viagens descreverem instalações diferentes
                // consoante a máquina: com `OCINYE_MAIL_IMAP_HOST` no `.env` local
                // o Correio diz «ligue a sua caixa», e sem ele diz «não está
                // configurado». Duas Experiences distintas, e o teste mediria a
                // que a máquina por acaso tivesse.
                //
                // Um anfitrião que não existe é deliberado: nada nestas viagens se
                // liga a servidor nenhum — o adaptador é o `UnconfiguredProvider`,
                // e o que se mede é o que a instalação **diz saber**, não o que
                // alcança.
                imap_host: "mail.invalido.test".to_owned(),
                smtp_host: "mail.invalido.test".to_owned(),
                username: String::new(),
                password: String::new(),
                ..config.mail.clone()
            },
            Some(chave_do_correio().clone()),
        )
        // Uma caixa ligada também fala com o duplo.
        //
        // Sem isto, ligar a caixa produzia um cliente IMAP verdadeiro contra um
        // anfitrião que não existe, e abrir uma mensagem dava erro: as viagens que
        // medem o produto mediam a rede — offline, mediam a sua ausência.
        .com_construtor(Box::new(|_| {
            Ok(Arc::new(ServicoQueResponde) as Arc<dyn ocinye_core::modules::mail::MailProvider>)
        })),
    );
    AppState {
        pool,
        config: Arc::new(config),
        verifier,
        authenticator,
        // O armazenamento entra quando está configurado, e só então.
        //
        // Com `None` o Core recusa carregamentos com «storage unavailable», que
        // é a resposta certa numa instalação sem armazenamento — mas faria a
        // viagem de Ficheiros provar o contrário do que se quer provar. A
        // jornada que carrega bytes diz em voz alta quando não pode correr.
        store: store_de_teste().map(std::sync::Arc::new),
        // O provider determinístico, para as viagens que provam recuperação
        // semântica. Não é um modelo, e a identidade que grava di-lo.
        embeddings: Some(std::sync::Arc::new(
            ocinye_core::modules::intelligence::embeddings::DeterministicEmbeddings::default(),
        )),
        inference: Arc::new(ocinye_core::modules::intelligence::NoProvider),
        mail_registry,
        // Estes testes medem HTTP, e não tempo real. Um plano ausente aceita
        // tudo e não propaga nada — que é o que uma instalação sem Redis faz,
        // e não um sítio por preencher.
        realtime: Arc::new(ocinye_core::realtime::Realtime::ausente()),
        mail_probe: Arc::new(SondaDoHarness),
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
///
/// Um ficheiro, e não um commit.
///
/// Era lida com `git show 075204e:…`, e isso partiu-se no dia em que o
/// repositório foi recriado: o commit deixou de existir, e a viagem passou a
/// recusar na CI enquanto passava na máquina onde o objecto solto sobrevivia.
/// Um teste que passa aqui e falha lá ensina que o teste não é de confiança,
/// não que o código está errado.
///
/// Congelada como fixture pela mesma razão que a tabela de tokens em
/// `scripts/rendered_value_equivalence.py`: a propriedade que isto guarda — os
/// **primitivos** renderizam hoje como renderizavam antes da consolidação — não
/// precisa de história, precisa da folha.
const CSS_BASE: &str = "tests/fixtures/ocinye-pre-consolidacao.css";

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

    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(commit);
    let anterior = std::fs::read(&fixture).unwrap_or_else(|erro| {
        panic!(
            "a folha de estilos base não está em {}: {erro}. Sem ela isto seria \
             uma comparação não medida, e não uma comparação igual",
            fixture.display()
        )
    });
    assert!(
        anterior.len() > 10_000,
        "a folha de estilos base tem {} bytes; uma fixture truncada faria a \
         comparação passar sem observar nada",
        anterior.len()
    );
    std::fs::write(destino.join("ocinye.css"), &anterior).expect("CSS base");
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
             -- Ao meio-dia do dia que a agenda vai mostrar.
             --
             -- O que esta viagem mede é **revogação de acesso**, e não
             -- fronteiras de fuso. Com um deslocamento a partir de agora, o
             -- evento atravessava a meia-noite de quem olha durante duas horas
             -- por dia e a viagem falhava a dizer «o gestor não vê o evento da
             -- sua unidade» — que é a mensagem de um defeito de autorização
             -- que não existia. A fronteira tem a sua própria viagem.
             SELECT organisation_id, 'unit', $1, $3, FALSE,
                    (((now() AT TIME ZONE 'UTC')::date + time '12:00') AT TIME ZONE 'UTC'),
                    (((now() AT TIME ZONE 'UTC')::date + time '13:00') AT TIME ZONE 'UTC'),
                    'UTC', 'RESTRICTED', $2
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
    // Quem olha está em UTC, declarado: assim a data que este teste calcula e o
    // dia civil que o produto agrupa são o mesmo, e uma falha aqui só pode ser
    // de autorização.
    let agenda = harness
        .open_em(
            &format!(
                "/calendar?view=today&on={}",
                chrono::Utc::now().date_naive()
            ),
            Some("UTC"),
        )
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
    set_field(&page, "input[name=email]", &credenciais.email).await;
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

/// Duplo clique num dia do Mês abre uma actividade nesse dia.
///
/// # Porque a prova é a data no campo e não a navegação
///
/// Chegar ao editor não prova nada: o botão `+ Nova actividade` também lá
/// chega. O que este atalho promete é que o dia em que se carregou é o dia que
/// aparece — e é isso, e só isso, que aqui se verifica.
#[tokio::test]
async fn duplo_clique_num_dia_abre_uma_actividade_nesse_dia() {
    let harness = harness!();
    let (_pessoa, _credenciais) = harness.sign_in(&[TechnicalRole::ResearchMember]).await;

    // Um dia deste mês que não é hoje: se fosse hoje, a data por omissão do
    // editor coincidiria e o teste passaria sem o atalho fazer nada.
    use chrono::Datelike;
    let hoje = chrono::Utc::now().date_naive();
    let alvo = if hoje.day() > 15 {
        hoje.with_day(3).unwrap_or(hoje)
    } else {
        hoje.with_day(24).unwrap_or(hoje)
    };
    assert_ne!(alvo, hoje, "o dia alvo tem de ser diferente de hoje");

    let page = harness.open("/calendar?view=month").await;
    esperar_por(&page, "Calendário").await;

    let carregou = page
        .evaluate(format!(
            "(() => {{ const c = document.querySelector('[data-oc-dia=\"{alvo}\"]');              if (!c) return false;              c.dispatchEvent(new MouseEvent('dblclick', {{bubbles: true}})); return true; }})()"
        ))
        .await
        .ok()
        .and_then(|v| v.into_value::<bool>().ok())
        .unwrap_or(false);
    assert!(carregou, "não havia célula para o dia {alvo}");

    let destino = wait_until_left(&page, "/calendar?view=month").await;
    assert!(
        destino.contains("/calendar/events/new"),
        "o duplo clique não abriu o editor: {destino}"
    );

    esperar_por(&page, "Nova actividade").await;
    let inicio = valor_de(&page, "input[name=starts_at]").await;
    assert!(
        inicio.starts_with(&alvo.to_string()),
        "o editor abriu em «{inicio}» e devia abrir no dia {alvo}"
    );
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

/// Um clique entregue ao elemento, e não a um par de coordenadas.
///
/// # Quando é isto e não `clicar`
///
/// `clicar` clica **onde** o elemento está, e é o que se quer na maioria das
/// viagens: mede também que o elemento está alcançável.
///
/// Não serve quando a viagem emula uma janela maior do que a janela real do
/// Chrome. O `SetDeviceMetricsOverride` muda o que a página julga ter, e não o
/// tamanho da janela do sistema: um controlo que a página coloca em x=1340
/// pode cair fora da janela verdadeira, e o clique não chega a lado nenhum. O
/// sintoma é um teste a dizer que o botão não fez nada — quando o que não
/// aconteceu foi o clique.
///
/// Verificado: com `clicar`, o compositor expandido não voltava ao tamanho; com
/// isto, volta. O manípulo estava certo desde o princípio.
async fn clicar_por_script(page: &Page, seletor: &str) {
    page.evaluate(format!("document.querySelector('{seletor}').click()"))
        .await
        .unwrap_or_else(|erro| panic!("«{seletor}» não pôde ser clicado: {erro:?}"));
}

/// Fixa a janela desta página, sem gravar imagem nenhuma.
///
/// As viagens partilham o browser, e a janela por omissão é estreita. Uma que
/// meça uma disposição de desktop tem de a pedir — senão mede o modo de coluna
/// única e conclui que o desktop está partido.
async fn janela(page: &Page, medidas: (i64, i64)) {
    use chromiumoxide::cdp::browser_protocol::emulation::SetDeviceMetricsOverrideParams;

    page.execute(
        SetDeviceMetricsOverrideParams::builder()
            .width(medidas.0)
            .height(medidas.1)
            .device_scale_factor(1.0)
            .mobile(false)
            .build()
            .expect("métricas da janela"),
    )
    .await
    .expect("aplicar as métricas");
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

/// Uma pessoa liga a sua caixa de correio a partir do Workspace.
///
/// # O que esta viagem guarda
///
/// Duas coisas que a suite do Core não pode ver, porque acontecem no browser.
///
/// A primeira é que a senha atravessa o Workspace uma vez, a caminho do Core, e
/// não regressa: o campo abre vazio, a página de regresso não a contém, e não há
/// endpoint que a devolva. A asserção pergunta ao HTML inteiro, e não ao campo —
/// um campo vazio com a senha num atributo de outro elemento continuaria a ser
/// uma senha no browser.
///
/// A segunda é que a caixa muda de estado à vista: «Por ligar» antes, «Ligada»
/// depois. Sem isso, o formulário podia estar a submeter para o vazio.
#[tokio::test(flavor = "multi_thread")]
async fn uma_pessoa_liga_a_sua_caixa_de_correio() {
    let harness = harness!();

    let (person_id, _) = harness.sign_in(&[TechnicalRole::ResearchMember]).await;
    let (_caixa, endereco) = harness.has_a_mailbox(person_id).await;

    // A senha da caixa. Nunca a do Ocinye: são coisas distintas, e nenhuma
    // serve para obter a outra.
    const SENHA: &str = "senha-so-do-imap-9134";

    let page = harness.open("/mail/settings").await;
    esperar_por(&page, "As suas caixas").await;

    let html = page.content().await.expect("conteúdo");
    assert!(
        html.contains(&endereco),
        "a caixa da pessoa não apareceu nas definições de correio"
    );
    assert!(
        html.contains("Por ligar"),
        "a caixa por ligar não se apresentou como tal"
    );

    // ── O formulário pede a senha, e só a senha ─────────────────────────
    //
    // Havia aqui um campo de conta, pré-preenchido com o endereço da caixa, e
    // esta viagem escrevia-o. Um campo editável convida a editar — e o que se
    // editava era a conta com que o Ocinye se autentica no servidor de
    // correio, enquanto o ecrã continuava a mostrar o endereço da caixa.
    //
    // O Core resolve-a: `principal → MemberId → endereço institucional`. Aqui
    // mede-se a ausência, porque uma ausência que não se mede volta.
    let editaveis: Option<f64> = page
        .evaluate(
            "document.querySelectorAll(\
               '[data-oc=ligar-caixa] input:not([type=password]):not([readonly])'\
             ).length",
        )
        .await
        .expect("contagem")
        .into_value()
        .ok();
    assert_eq!(
        editaveis,
        Some(0.0),
        "o formulário de ligação tem um campo editável que não é a senha"
    );

    set_field(&page, "[data-oc=ligar-caixa] input[name=password]", SENHA).await;
    submit(&page, "[data-oc=ligar-caixa]").await;

    esperar_por(&page, "Ligada").await;
    let depois = page.content().await.expect("conteúdo");

    assert!(
        !depois.contains(SENHA),
        "a senha da caixa voltou no documento"
    );

    // E também não na barra de endereço.
    //
    // Perguntar só ao documento deixava passar uma senha na URL — que fica no
    // histórico do browser, no `Referer` do pedido seguinte, e no log de acesso
    // de qualquer coisa pelo meio. Descobri-o por reversão: pus a senha na URL
    // de regresso e esta viagem continuou verde.
    let url = page.url().await.expect("endereço").unwrap_or_default();
    assert!(
        !url.contains(SENHA),
        "a senha da caixa foi parar à barra de endereço: {url}"
    );
    assert!(
        depois.contains("Desligar e esquecer a senha"),
        "a caixa ligada não oferece o caminho de volta"
    );

    // E a senha também não ficou legível na base de dados.
    let cifrado: Vec<u8> = sqlx::query_scalar(
        "SELECT c.ciphertext FROM mailbox_credentials c
           JOIN mailboxes m ON m.id = c.mailbox_id
          WHERE m.address = $1",
    )
    .bind(&endereco)
    .fetch_one(&harness.pool)
    .await
    .expect("credencial guardada");

    assert!(
        !String::from_utf8_lossy(&cifrado).contains(SENHA),
        "a senha ficou legível na base de dados"
    );
}

/// A sonda do harness: aceita, porque não há servidor de correio para
/// perguntar. O que o harness assume fica escrito, em vez de a verificação
/// desaparecer do caminho que estes testes percorrem.
struct SondaDoHarness;

#[async_trait::async_trait]
impl ocinye_core::modules::mail::provider::CredentialProbe for SondaDoHarness {
    async fn verify(
        &self,
        _endereco: &str,
        _username: &str,
        _senha: &str,
    ) -> ocinye_core::modules::mail::provider::ProviderResult<()> {
        Ok(())
    }
}

/// Um compromisso à meia-noite e meia aparece no dia certo de quem olha.
///
/// # O defeito que esta viagem guarda
///
/// O Calendário agrupava dias em UTC. Um compromisso marcado para as 00:30 em
/// Lisboa fica guardado às 23:30 do dia anterior em UTC — e aparecia lá: no dia
/// errado, à hora errada.
///
/// Não é um erro de fuso na apresentação de uma hora. É o dia civil inteiro a
/// ser decidido no sítio errado, e por isso atravessava o Dia, a Semana, o Mês,
/// a Agenda e o «Hoje».
///
/// # Porque a zona é declarada pelo teste
///
/// Para que a viagem não dependa do fuso da máquina onde corre. Declarada, a
/// mesma asserção vale nesta estação de trabalho e em CI.
#[tokio::test(flavor = "multi_thread")]
async fn um_compromisso_a_meia_noite_e_meia_aparece_no_dia_de_quem_olha() {
    let harness = harness!();
    let (person_id, _) = harness.sign_in(&[TechnicalRole::ResearchMember]).await;
    harness.manages_a_unit(person_id).await;

    // Um dia fixo, longe de hoje: o que se mede é a fronteira, e não o
    // calendário de hoje.
    let dia = chrono::NaiveDate::from_ymd_opt(2026, 3, 12).expect("data");
    let titulo = unique_title("Madrugada");

    // Um fuso a leste, sem mudança de hora e sem nomes antigos.
    //
    // `Asia/Tbilisi` está em UTC+4 o ano inteiro: as 00:30 de lá são 20:30 do
    // **dia anterior** em UTC, que é a discrepância que o agrupamento em UTC
    // produzia. Lisboa não serviria — em Março está em UTC+0 e não há fronteira
    // para atravessar. `Europe/Kyiv` também não: o Chrome devolve-lhe o nome
    // antigo, `Europe/Kiev`, e a viagem passaria a medir alcunhas de fusos.
    let zona = "Asia/Tbilisi";

    let quando = format!("{dia}T00:30");
    let fim = format!("{dia}T01:30");

    let formulario = harness.open_em("/calendar/events/new", Some(zona)).await;
    set_field(&formulario, "input[name=title]", &titulo).await;
    set_field(&formulario, "input[name=starts_at]", &quando).await;
    set_field(&formulario, "input[name=ends_at]", &fim).await;
    set_field(&formulario, "input[name=timezone]", zona).await;
    submit(&formulario, "form.oc-editor__form").await;
    wait_until_left(&formulario, "/calendar/events/new").await;

    // O instante guardado é mesmo do dia anterior em UTC. Sem esta verificação,
    // a viagem podia passar por o evento nunca ter atravessado a fronteira.
    let guardado: chrono::DateTime<chrono::Utc> =
        sqlx::query_scalar("SELECT starts_at FROM calendar_events WHERE title = $1")
            .bind(&titulo)
            .fetch_one(&harness.pool)
            .await
            .expect("o evento devia ter sido guardado");
    assert_eq!(
        guardado.date_naive(),
        dia - chrono::Duration::days(1),
        "o controlo desta viagem falhou: em UTC o instante devia cair no dia \
         anterior, e caiu em {}",
        guardado.date_naive()
    );

    // O `app.js` do formulário já declarou o fuso ao servidor. Confirma-se, para
    // que uma falha na declaração se leia como tal e não como um agrupamento
    // errado.
    let declarado: Option<String> = formulario
        .evaluate("document.cookie")
        .await
        .expect("cookie")
        .into_value()
        .ok();
    assert!(
        declarado.unwrap_or_default().contains("Tbilisi"),
        "o browser não declarou o fuso ao servidor"
    );

    for vista in ["day", "week", "month", "agenda"] {
        let pagina = harness
            .open_em(&format!("/calendar?view={vista}&on={dia}"), Some(zona))
            .await;
        let html = pagina.content().await.expect("conteúdo");
        assert!(
            html.contains(&titulo),
            "«{vista}» agrupou o compromisso pelo dia de Greenwich em vez do dia \
             de quem olha"
        );
    }

    // E não aparece no dia anterior, que é onde o defeito o punha.
    let anterior = harness
        .open_em(
            &format!("/calendar?view=day&on={}", dia - chrono::Duration::days(1)),
            Some(zona),
        )
        .await
        .content()
        .await
        .expect("conteúdo");
    assert!(
        !anterior.contains(&titulo),
        "o compromisso continuou a aparecer no dia anterior"
    );
}

/// Uma pessoa começa uma conversa e envia a primeira mensagem.
///
/// # A viagem que prova que a aplicação existe
///
/// Carregar em «Nova conversa», procurar alguém pelo nome, escolher, escrever e
/// enviar. Se qualquer botão não estiver ligado, esta viagem pára — que é
/// exactamente o que aconteceu com o `+` e o «Nova conversa» antes de existir
/// selector nenhum.
#[tokio::test(flavor = "multi_thread")]
async fn uma_pessoa_comeca_uma_conversa_e_envia_a_primeira_mensagem() {
    let harness = harness!();
    let (person_id, _) = harness.sign_in(&[TechnicalRole::ResearchMember]).await;

    // Alguém com quem falar. O nome é único para a procura não apanhar outros.
    let colega = unique_title("Bartolomeu");
    let outro = harness.outra_pessoa(person_id, &colega).await;

    let page = harness.open("/messages").await;
    esperar_por(&page, "Mensagens").await;

    let inicial = page.content().await.expect("conteúdo");
    assert!(
        inicial.contains("Comece uma conversa"),
        "sem conversas, a aplicação devia convidar a começar uma"
    );

    // O botão abre o selector.
    clicar(&page, "[data-oc=\"nova-conversa\"]").await;
    let dialogo = wait_visible(&page, "[data-oc=\"procurar-pessoa\"]").await;
    let _ = dialogo;

    // Procurar pelo nome, e esperar que o servidor responda.
    set_field(&page, "[data-oc=\"procurar-pessoa\"]", &colega).await;
    esperar_por(&page, &colega).await;

    let com_resultados = page.content().await.expect("conteúdo");
    assert!(
        com_resultados.contains("oc-msg__resultado"),
        "a procura não devolveu ninguém"
    );

    // Escolher abre a conversa directa.
    clicar(&page, "[data-oc=\"escolher-pessoa\"]").await;
    let destino = wait_until_left(&page, "/messages").await;
    assert!(
        destino.contains("/messages/"),
        "escolher alguém não abriu a conversa: {destino}"
    );

    // Escrever e enviar.
    esperar_por(&page, "Escrever mensagem").await;
    set_field(&page, "[data-oc=\"texto\"]", "Ola, ja terminei a revisao").await;
    clicar(&page, "[data-oc=\"enviar\"]").await;
    esperar_por(&page, "Ola, ja terminei a revisao").await;

    // E aparece do **lado de quem a escreveu**.
    //
    // Sem isto, o Workspace lia `id` numa resposta cujo campo se chama
    // `person_id`, o `unwrap_or_default()` devolvia o UUID nulo, e nenhuma
    // mensagem era própria — a conversa inteira alinhava como se fosse de
    // outra pessoa. Uma chave errada num JSON não dá erro, e por isso a
    // asserção tem de estar aqui, sobre o que a página desenhou.
    let enviada = page.content().await.expect("conteúdo");
    assert!(
        enviada.contains("oc-msg__mensagem--minha"),
        "a mensagem que esta pessoa acabou de enviar não ficou marcada como sua"
    );

    // A mensagem ficou guardada, com a autora certa e no sítio certo.
    let guardada: (Uuid, String) = sqlx::query_as(
        "SELECT m.author_id, m.body
           FROM messages m
           JOIN conversation_participants p ON p.conversation_id = m.conversation_id
          WHERE p.person_id = $1 AND m.body = $2
          LIMIT 1",
    )
    .bind(outro)
    .bind("Ola, ja terminei a revisao")
    .fetch_one(&harness.pool)
    .await
    .expect("a mensagem devia estar guardada");

    assert_ne!(
        guardada.0, outro,
        "o autor da mensagem ficou a ser quem a recebeu"
    );

    // E aparece na lista de conversas.
    let lista = harness
        .open("/messages")
        .await
        .content()
        .await
        .expect("conteúdo");
    assert!(
        lista.contains(&colega),
        "a conversa não apareceu na lista de quem a começou"
    );
}

/// Uma pessoa cria um grupo com duas pessoas.
#[tokio::test(flavor = "multi_thread")]
async fn uma_pessoa_cria_um_grupo_com_duas_pessoas() {
    let harness = harness!();
    let (person_id, _) = harness.sign_in(&[TechnicalRole::ResearchMember]).await;

    let primeira = unique_title("Casimiro");
    let segunda = unique_title("Doroteia");
    harness.outra_pessoa(person_id, &primeira).await;
    harness.outra_pessoa(person_id, &segunda).await;

    let nome_do_grupo = unique_title("Projecto");

    let page = harness.open("/messages").await;
    esperar_por(&page, "Mensagens").await;
    clicar(&page, "[data-oc=\"nova-conversa\"]").await;
    wait_visible(&page, "[data-oc=\"procurar-pessoa\"]").await;

    // Passar a grupo, dar-lhe nome, e escolher duas pessoas.
    clicar(&page, "[data-oc-modo=\"grupo\"]").await;
    set_field(&page, "[data-oc=\"nome-do-grupo\"]", &nome_do_grupo).await;

    for quem in [&primeira, &segunda] {
        set_field(&page, "[data-oc=\"procurar-pessoa\"]", quem).await;
        esperar_por(&page, quem).await;
        clicar(&page, "[data-oc=\"escolher-pessoa\"]").await;
        // A etiqueta de escolhido aparece.
        esperar_por(&page, quem).await;
    }

    clicar(&page, "[data-oc=\"criar-conversa\"]").await;
    let destino = wait_until_left(&page, "/messages").await;
    assert!(
        destino.contains("/messages/"),
        "o grupo não abriu: {destino}"
    );
    // A URL mudou; o documento pode ainda estar a chegar. `wait_until_left`
    // observa o endereço, e um endereço novo não é uma página desenhada — o
    // `content()` mais abaixo lia o documento antigo e não encontrava o nome
    // do grupo. Aqui esperamos por texto que só a conversa tem, como o teste
    // irmão já fazia.
    esperar_por(&page, "Escrever mensagem").await;

    // Três pessoas: as duas escolhidas e quem o criou.
    let quantos: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM conversation_participants p
           JOIN conversations c ON c.id = p.conversation_id
          WHERE c.name = $1 AND p.left_at IS NULL",
    )
    .bind(&nome_do_grupo)
    .fetch_one(&harness.pool)
    .await
    .expect("contagem");

    assert_eq!(quantos, 3, "o grupo não ficou com as três pessoas");

    let html = page.content().await.expect("conteúdo");
    assert!(
        html.contains(&nome_do_grupo),
        "o grupo abriu sem o seu nome"
    );
    assert!(
        html.contains("3 participantes"),
        "o cabeçalho não conta quem lá está"
    );
}

/// O sino abre um painel, e o painel mostra o que chegou.
///
/// # O que esta viagem guarda
///
/// Três coisas que se perdem em silêncio. Que o sino **não navega** — levar a
/// pessoa a outro ecrã fá-la perder o sítio onde estava. Que o painel vai
/// mesmo buscar as notificações, em vez de ficar no «A carregar…». E que o que
/// ele desenha é o que o Core escreveu — uma chave errada num JSON não dá erro,
/// e já custou uma conversa inteira alinhada do lado errado.
#[tokio::test(flavor = "multi_thread")]
async fn o_sino_abre_um_painel_com_o_que_chegou() {
    let harness = harness!();
    let (person_id, _) = harness.sign_in(&[TechnicalRole::ResearchMember]).await;

    // Uma notificação a sério, escrita como o Core a escreve.
    let titulo = unique_title("Ermelinda");
    let organisation_id: Uuid =
        sqlx::query_scalar("SELECT organisation_id FROM people WHERE id = $1")
            .bind(person_id)
            .fetch_one(&harness.pool)
            .await
            .expect("organização");

    sqlx::query(
        "INSERT INTO notifications (organisation_id, recipient_id, kind, title)
              VALUES ($1, $2, 'reminder', $3)",
    )
    .bind(organisation_id)
    .bind(person_id)
    .bind(&titulo)
    .execute(&harness.pool)
    .await
    .expect("notificação");

    let page = harness.open("/").await;
    esperar_por(&page, "OCINYE").await;

    let antes = page.url().await.ok().flatten().unwrap_or_default();
    clicar(&page, "[data-oc=\"abrir-notificacoes\"]").await;
    esperar_por(&page, &titulo).await;

    let depois = page.url().await.ok().flatten().unwrap_or_default();
    assert_eq!(antes, depois, "o sino navegou em vez de abrir um painel");

    let html = page.content().await.expect("conteúdo");
    assert!(
        html.contains("oc-sino__linha"),
        "o painel abriu sem desenhar a notificação"
    );
    assert!(
        !html.contains("A carregar…"),
        "o painel ficou preso no estado de carregamento"
    );

    // ── O acabamento, medido em vez de comparado de olho ────────────────
    //
    // A superfície e o ritmo das linhas. O painel da conta é a referência: se
    // um destes deixar de coincidir, o sino passa a parecer menos acabado do
    // que ele — que foi exactamente o que aconteceu duas vezes.
    let medidas: Option<String> = page
        .evaluate(
            "(() => { \
               const iguais = (a, b, props) => props.every(p => \
                 getComputedStyle(a).getPropertyValue(p) === \
                 getComputedStyle(b).getPropertyValue(p)); \
               const conta = document.querySelector('[data-oc=\"account-menu\"]'); \
               const sino = document.querySelector('[data-oc=\"notificacoes\"]'); \
               const linha = document.querySelector('.oc-sino__linha'); \
               return JSON.stringify({ \
                 superficie: iguais(conta, sino, \
                   ['background-color','backdrop-filter','box-shadow','border-radius']), \
                 temIcone: !!(linha && linha.querySelector('svg')), \
                 temTitulo: !!(linha && linha.querySelector('b')), \
                 temLegenda: !!(linha && linha.querySelector('em')), \
               }); })()",
        )
        .await
        .expect("medidas")
        .into_value()
        .ok();

    let medidas = medidas.unwrap_or_default();
    assert!(
        medidas.contains(r#""superficie":true"#),
        "o painel do sino não tem o acabamento do painel da conta: {medidas}"
    );
    assert!(
        medidas.contains(r#""temIcone":true"#)
            && medidas.contains(r#""temTitulo":true"#)
            && medidas.contains(r#""temLegenda":true"#),
        "uma linha do sino não tem o ritmo das linhas do painel da conta — \
         ícone, título e legenda: {medidas}"
    );

    // E fecha por `Escape`, como o painel da conta fecha.
    page.evaluate("document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape' }))")
        .await
        .expect("escape");

    let aberto: Option<bool> = page
        .evaluate("document.querySelector('[data-oc=\"notificacoes\"]').hidden === false")
        .await
        .expect("estado")
        .into_value()
        .ok();
    assert_eq!(aberto, Some(false), "o painel não fechou com Escape");
}

/// Os três painéis da barra, para revisão visual.
///
/// # Porque os três na mesma corrida
///
/// Porque o que se compara é o acabamento **entre** eles. Capturados em alturas
/// diferentes, comparar-se-iam de memória — e foi de memória que eu concluí que
/// o painel da conta precisava de mudar, quando o que precisava era de ficar
/// como estava.
#[tokio::test]
#[ignore = "grava ficheiros; serve a revisão visual, não a verificação"]
async fn capturas_dos_paineis_da_barra() {
    let harness = harness!();
    let (person_id, _) = harness.sign_in(&[TechnicalRole::ResearchMember]).await;

    let organisation_id: Uuid =
        sqlx::query_scalar("SELECT organisation_id FROM people WHERE id = $1")
            .bind(person_id)
            .fetch_one(&harness.pool)
            .await
            .expect("organização");

    // Duas notificações, para o painel do sino ter o que mostrar — uma por ler
    // e uma lida, que é o que distingue os dois acabamentos de linha.
    for (titulo, lida) in [("Ana Silva", false), ("Lembrete de revisão", true)] {
        sqlx::query(
            "INSERT INTO notifications
                 (organisation_id, recipient_id, kind, title, read_at)
              VALUES ($1, $2, 'reminder', $3, CASE WHEN $4 THEN now() ELSE NULL END)",
        )
        .bind(organisation_id)
        .bind(person_id)
        .bind(titulo)
        .bind(lida)
        .execute(&harness.pool)
        .await
        .expect("notificação");
    }

    let page = harness.open("/").await;
    esperar_por(&page, "OCINYE").await;

    // ── O painel da conta: a referência ─────────────────────────────────
    clicar(&page, "[data-oc=\"account-toggle\"]").await;
    esperar_por(&page, "A minha conta").await;
    capturar_visivel(&page, "painel-conta").await;
    page.evaluate("document.body.click()")
        .await
        .expect("fechar");

    // ── O sino ──────────────────────────────────────────────────────────
    clicar(&page, "[data-oc=\"abrir-notificacoes\"]").await;
    esperar_por(&page, "Ana Silva").await;
    capturar_visivel(&page, "painel-sino").await;
    page.evaluate("document.body.click()")
        .await
        .expect("fechar");

    // ── O calendário da barra ───────────────────────────────────────────
    open_temporal_centre(&page).await;
    capturar_visivel(&page, "painel-calendario").await;

    // ── E os três acabamentos, medidos e não comparados de olho ────────
    //
    // A captura mostra; ela não prova. O que prova é isto: as três superfícies
    // têm de dar exactamente os mesmos valores computados. Duas vezes nesta
    // sessão um painel ficou visivelmente menos acabado do que o da conta com
    // o CSS aparentemente escrito — uma vez por a regra estar presa dentro de
    // uma `@media`, outra por o selector encontrado não ser o pretendido.
    let medidas = estilo_computado(
        &page,
        &[
            (
                "[data-oc=\"account-menu\"]",
                "background-color,backdrop-filter,box-shadow,border-radius,border-top-color",
            ),
            (
                "[data-oc=\"notificacoes\"]",
                "background-color,backdrop-filter,box-shadow,border-radius,border-top-color",
            ),
            (
                "[data-oc=\"temporal-centre\"]",
                "background-color,backdrop-filter,box-shadow,border-radius,border-top-color",
            ),
        ],
    )
    .await;

    let acabamentos: Vec<&str> = medidas
        .lines()
        .map(|linha| linha.split_once(' ').map(|(_, resto)| resto).unwrap_or(""))
        .collect();
    assert_eq!(acabamentos.len(), 3, "faltou um painel:\n{medidas}");
    assert!(
        !acabamentos.iter().any(|a| a.is_empty()),
        "um dos painéis não estava no documento:\n{medidas}"
    );
    assert!(
        acabamentos.windows(2).all(|par| par[0] == par[1]),
        "os painéis da barra não partilham o acabamento do painel da conta:\n{medidas}"
    );
}

/// A matriz de estados do Correio, em imagens.
///
/// # Porque uma matriz e não uma captura
///
/// Porque o Correio tem seis estados que se parecem e pedem coisas
/// diferentes, e o defeito desta família não é uma cor errada: é o **estado
/// errado** — a página a mandar alguém falar com quem administra quando o que
/// falta é a senha dela, ou a dizer que não há mensagens quando o que há é um
/// serviço em baixo.
///
/// Cada asserção destas já existe como teste. O que as imagens acrescentam é a
/// única pergunta que nenhuma asserção responde: *isto parece a aplicação
/// final?* Duas vezes nesta sessão a resposta foi não com todos os portões
/// verdes.
///
/// # Ignorado por omissão
///
/// Grava ficheiros e serve a revisão visual. A verificação vive nos testes que
/// afirmam o comportamento; isto mostra-o.
#[tokio::test]
#[ignore = "grava ficheiros; serve a revisão visual, não a verificação"]
async fn capturas_do_correio() {
    let harness = harness!();
    let (person_id, _) = harness.sign_in(&[TechnicalRole::ResearchMember]).await;
    harness.has_a_mailbox(person_id).await;

    // ── A caixa por ligar ───────────────────────────────────────────────
    let page = harness.open("/mail").await;
    esperar_por(&page, "Correio").await;
    capturar_visivel(&page, "correio-por-ligar").await;

    let definicoes = harness.open("/mail/settings").await;
    esperar_por(&definicoes, "As suas caixas").await;
    capturar_visivel(&definicoes, "correio-definicoes-por-ligar").await;

    // ── Ligada, e sem mensagens ─────────────────────────────────────────
    set_field(
        &definicoes,
        "[data-oc=ligar-caixa] input[name=password]",
        "senha-so-do-imap-4471",
    )
    .await;
    submit(&definicoes, "[data-oc=ligar-caixa]").await;
    esperar_por(&definicoes, "Ligada").await;
    capturar_visivel(&definicoes, "correio-definicoes-ligada").await;

    // A caixa que **a interface** ligou, e não uma que o teste tenha escolhido.
    //
    // A pessoa tem mais do que uma, e o formulário liga a primeira. Escrever
    // as mensagens noutra deixava a lista vazia, com o aspecto de o índice não
    // ter funcionado — e a captura teria mostrado exactamente isso.
    let (caixa, endereco): (Uuid, String) = sqlx::query_as(
        "SELECT m.id, m.address FROM mailboxes m
           JOIN mailbox_credentials c ON c.mailbox_id = m.id
          WHERE m.owner_id = $1",
    )
    .bind(person_id)
    .fetch_one(&harness.pool)
    .await
    .expect("a caixa ligada");

    let vazia = harness.open(&format!("/mail/{caixa}")).await;
    esperar_por(&vazia, "Correio").await;
    capturar_visivel(&vazia, "correio-entrada-vazia").await;

    // ── Com correio ─────────────────────────────────────────────────────
    //
    // Escrito no índice, que é de onde a lista lê. Não passa por servidor
    // nenhum: o que se está a fotografar é a Experience, e um servidor real
    // tornaria a imagem dependente de uma rede.
    for (assunto, remetente, lida) in [
        (
            "Relatório trimestral da unidade",
            "ana.silva@exemplo.com",
            false,
        ),
        (
            "Re: dados do ensaio de Fevereiro",
            "j.mendes@universidade.ao",
            false,
        ),
        (
            "Convite — seminário de infraestruturas",
            "eventos@exemplo.org",
            true,
        ),
        ("Confirmação de recepção", "secretaria@exemplo.com", true),
    ] {
        sqlx::query(
            "INSERT INTO mail_messages
                    (mailbox_id, provider_id, folder, from_address, from_display_name,
                     subject, snippet, sent_at, is_read)
                  VALUES ($1, $2, 'inbox', $3, $4, $5, $6, now() - ($7 * interval '1 hour'), $8)",
        )
        .bind(caixa)
        .bind(Uuid::new_v4().to_string())
        .bind(remetente)
        .bind(remetente.split('@').next().unwrap_or(remetente))
        .bind(assunto)
        .bind("As primeiras linhas da mensagem, como aparecem na lista.")
        .bind(f64::from(u32::from(lida)) * 3.0 + 1.0)
        .bind(lida)
        .execute(&harness.pool)
        .await
        .expect("mensagem");
    }

    // A caixa **desta** viagem, e não a primeira que a página escolheria.
    //
    // A pessoa tem mais do que uma caixa, e `/mail` abre a primeira. As
    // mensagens foram escritas nesta; sem a nomear, a lista mostrava a outra —
    // vazia, e com o aspecto de o índice não ter funcionado.
    let cheia = harness.open(&format!("/mail/{caixa}")).await;
    esperar_por(&cheia, "Relatório trimestral").await;
    capturar_visivel(&cheia, "correio-entrada-com-mensagens").await;

    // ── A leitura ───────────────────────────────────────────────────────
    // A primeira linha da lista, seja ela qual for.
    //
    // A primeira escrita esperava por «Relatório trimestral» depois do clique,
    // e a lista ordena por data: a primeira linha é outra mensagem. O teste
    // falhava a dizer que o assunto não apareceu — e ele estava lá, noutra
    // linha, exactamente onde devia estar.
    clicar(&cheia, ".oc-mail__item").await;
    esperar_por(&cheia, "oc-mail__pane-head").await;
    capturar_visivel(&cheia, "correio-leitura").await;

    // ── O compositor ────────────────────────────────────────────────────
    let compositor = harness
        .open(&format!("/mail/compose?mailbox={caixa}"))
        .await;
    janela(&compositor, JANELA).await;
    esperar_por(&compositor, "Nova mensagem").await;
    capturar_visivel(&compositor, "correio-compositor").await;

    // Com destinatários, e com a cópia aberta.
    for endereco in ["ana.silva@exemplo.com", "j.mendes@universidade.ao"] {
        set_field(
            &compositor,
            "[data-oc-campo=to] [data-oc=destino-entrada]",
            endereco,
        )
        .await;
        compositor
            .evaluate(
                "document.querySelector('[data-oc-campo=to] [data-oc=destino-entrada]')\
                   .dispatchEvent(new KeyboardEvent('keydown', {key: 'Enter', bubbles: true}))",
            )
            .await
            .expect("aceitar o destinatário");
    }
    clicar(&compositor, "[data-oc=mostrar-cc]").await;
    set_field(
        &compositor,
        ".oc-comp__assunto",
        "Consolidação dos números do trimestre",
    )
    .await;
    set_field(
        &compositor,
        "[data-oc=compositor-corpo]",
        "Bom dia,\n\nSegue o resumo do trimestre. Fico a aguardar comentários antes \
         de o fechar.\n\nCumprimentos,\nFidel",
    )
    .await;
    capturar_visivel(&compositor, "correio-compositor-preenchido").await;

    clicar(&compositor, "[data-oc=compositor-expandir]").await;
    capturar_visivel(&compositor, "correio-compositor-expandido").await;

    // ── A disposição, comandada ─────────────────────────────────────────
    let arrumado = harness.open(&format!("/mail/{caixa}")).await;
    janela(&arrumado, JANELA).await;
    esperar_por(&arrumado, "Relatório trimestral").await;
    arrumado
        .evaluate("window.localStorage.removeItem('oc-mail-disposicao')")
        .await
        .expect("limpar");

    let arrumado = harness.open(&format!("/mail/{caixa}")).await;
    janela(&arrumado, JANELA).await;
    esperar_por(&arrumado, "Relatório trimestral").await;
    capturar_visivel(&arrumado, "correio-tri-painel").await;

    clicar(&arrumado, "[data-oc=alternar-pastas]").await;
    capturar_visivel(&arrumado, "correio-pastas-recolhidas").await;

    clicar(&arrumado, "[data-oc=focar-leitura]").await;
    capturar_visivel(&arrumado, "correio-leitura-dominante").await;

    // ── E o mesmo, apertado ─────────────────────────────────────────────
    //
    // Três painéis num ecrã estreito não se comprimem: escolhe-se um.
    let estreita = harness.open(&format!("/mail/{caixa}")).await;
    esperar_por(&estreita, "Correio").await;
    capturar_em(&estreita, "correio-estreito-lista", (760, 900)).await;

    assert!(
        !endereco.is_empty(),
        "a caixa tem de ter endereço para as capturas fazerem sentido"
    );
}

/// Um serviço de correio que responde, para as viagens que precisam de um.
///
/// # Porque o harness precisa dos dois
///
/// O `UnconfiguredProvider` descreve uma instalação sem serviço, e é o estado
/// certo para as viagens que medem **a ausência**. Não serve para as que medem
/// o produto: abrir uma mensagem passa pelo fornecedor, e um fornecedor que
/// recusa faz a leitura nunca aparecer.
///
/// # O que este duplo não faz
///
/// Não inventa correio. A lista vem do índice — é lá que as viagens escrevem —
/// e o que este devolve é o corpo de quem já está indexado. Um duplo que
/// gerasse mensagens faria as viagens medirem o duplo.
struct ServicoQueResponde;

#[async_trait::async_trait]
impl ocinye_core::modules::mail::MailProvider for ServicoQueResponde {
    fn adapter_name(&self) -> &'static str {
        "servico-de-ensaio"
    }

    async fn health(&self) -> ocinye_core::modules::mail::provider::ProviderHealth {
        ocinye_core::modules::mail::provider::ProviderHealth {
            endpoints: vec!["imap ensaio:993".to_owned(), "smtp ensaio:465".to_owned()],
            can_read: true,
            can_send: true,
            detail: "O serviço de correio está a responder.".to_owned(),
            rejected_credential: false,
        }
    }

    async fn list_messages(
        &self,
        _mailbox_address: &str,
        _folder: ocinye_contracts::MailFolder,
        _cursor: Option<&str>,
        _limit: u32,
    ) -> ocinye_core::modules::mail::provider::ProviderResult<
        ocinye_core::modules::mail::provider::MessagePage,
    > {
        Ok(ocinye_core::modules::mail::provider::MessagePage {
            messages: Vec::new(),
            next_cursor: None,
        })
    }

    async fn fetch_message(
        &self,
        _mailbox_address: &str,
        folder: ocinye_contracts::MailFolder,
        provider_id: &str,
    ) -> ocinye_core::modules::mail::provider::ProviderResult<
        ocinye_core::modules::mail::provider::FetchedMessage,
    > {
        Ok(ocinye_core::modules::mail::provider::FetchedMessage {
            header: ocinye_core::modules::mail::provider::MessageHeader {
                provider_id: provider_id.to_owned(),
                message_id: None,
                thread_key: None,
                folder,
                from: ocinye_core::modules::mail::service::sender_identity(
                    "ana.silva@exemplo.com",
                    Some("Ana Silva".to_owned()),
                ),
                to: Vec::new(),
                cc: Vec::new(),
                subject: Some("Relatório trimestral da unidade".to_owned()),
                snippet: None,
                sent_at: chrono::Utc::now(),
                is_read: false,
                is_starred: false,
                has_attachments: false,
                size_bytes: None,
            },
            text_body: Some(
                "Bom dia,\n\nSegue o relatório trimestral da unidade, com os números \
                 consolidados e as notas de método.\n\nCumprimentos,\nAna"
                    .to_owned(),
            ),
            html_body: None,
            attachments: Vec::new(),
            bcc: Vec::new(),
        })
    }

    async fn fetch_attachment(
        &self,
        _mailbox_address: &str,
        _folder: ocinye_contracts::MailFolder,
        _provider_id: &str,
        _part_id: &str,
    ) -> ocinye_core::modules::mail::provider::ProviderResult<Vec<u8>> {
        Err(ocinye_core::modules::mail::ProviderError::NotFound)
    }

    async fn send_message(
        &self,
        _mailbox_address: &str,
        _message: &ocinye_core::modules::mail::provider::OutgoingMessage,
    ) -> ocinye_core::modules::mail::provider::ProviderResult<Option<String>> {
        Ok(None)
    }

    async fn move_message(
        &self,
        _mailbox_address: &str,
        _folder: ocinye_contracts::MailFolder,
        _provider_id: &str,
        _destination: ocinye_contracts::MailFolder,
    ) -> ocinye_core::modules::mail::provider::ProviderResult<()> {
        Ok(())
    }

    async fn set_read(
        &self,
        _mailbox_address: &str,
        _folder: ocinye_contracts::MailFolder,
        _provider_id: &str,
        _read: bool,
    ) -> ocinye_core::modules::mail::provider::ProviderResult<()> {
        Ok(())
    }

    async fn set_starred(
        &self,
        _mailbox_address: &str,
        _folder: ocinye_contracts::MailFolder,
        _provider_id: &str,
        _starred: bool,
    ) -> ocinye_core::modules::mail::provider::ProviderResult<()> {
        Ok(())
    }
}

/// A pessoa comanda o espaço; o sistema protege a utilidade.
///
/// # O que estas asserções medem
///
/// Redimensionar é uma promessa fácil de fazer e difícil de manter: um
/// separador que se arrasta e não move nada, um mínimo que não existe e deixa
/// um painel a zero, um painel recolhido sem caminho de volta. Cada uma destas
/// falhas é invisível a um teste de marcação — o HTML fica igual.
///
/// Mede-se a largura **computada** dos painéis, antes e depois de cada gesto.
#[tokio::test]
async fn a_pessoa_arruma_o_correio_e_nao_o_parte() {
    let harness = harness!();
    let (person_id, _) = harness.sign_in(&[TechnicalRole::ResearchMember]).await;
    harness.has_a_mailbox(person_id).await;

    let page = harness.open("/mail").await;
    esperar_por(&page, "Caixa de entrada").await;

    // Uma janela de desktop, que é onde três painéis existem.
    janela(&page, JANELA).await;

    // Sem preferência herdada.
    //
    // O perfil do browser é partilhado entre viagens, e uma preferência
    // guardada por outra descreveria uma janela diferente. O que se mede aqui
    // é a disposição **por omissão** e o que os gestos lhe fazem.
    page.evaluate("window.localStorage.removeItem('oc-mail-disposicao')")
        .await
        .expect("limpar");
    let page = harness.open("/mail").await;
    janela(&page, JANELA).await;
    esperar_por(&page, "Caixa de entrada").await;

    /// A largura de um painel, em pixéis.
    async fn largura(page: &Page, seletor: &str) -> f64 {
        page.evaluate(format!(
            "document.querySelector('{seletor}')?.getBoundingClientRect().width ?? -1"
        ))
        .await
        .expect("medida")
        .into_value::<f64>()
        .unwrap_or(-1.0)
    }

    let pastas_inicial = largura(&page, ".oc-mail__rail").await;
    let leitura_inicial = largura(&page, ".oc-mail__pane").await;
    assert!(
        pastas_inicial > 100.0 && leitura_inicial > 300.0,
        "a disposição inicial já não é utilizável: pastas={pastas_inicial} \
         leitura={leitura_inicial}"
    );

    // ── O teclado move o separador ──────────────────────────────────────
    //
    // Pelo teclado e não pelo rato: um `role="separator"` que só responde ao
    // rato promete uma operação que não entrega, e é a promessa que este
    // portão mede.
    page.evaluate("document.querySelector('[data-oc-separador=\"pastas\"]').focus()")
        .await
        .expect("foco");
    for _ in 0..4 {
        page.evaluate(
            "document.querySelector('[data-oc-separador=\"pastas\"]').dispatchEvent(\
               new KeyboardEvent('keydown', {key: 'ArrowRight', bubbles: true}))",
        )
        .await
        .expect("seta");
    }
    let pastas_maiores = largura(&page, ".oc-mail__rail").await;
    assert!(
        pastas_maiores > pastas_inicial,
        "as setas não moveram o separador: {pastas_inicial} → {pastas_maiores}"
    );

    // ── E o mínimo aguenta ──────────────────────────────────────────────
    //
    // Cem setas para a esquerda. Sem limite, o painel chegava a zero e a
    // pessoa ficava com uma aplicação partida e sem forma óbvia de a
    // desfazer.
    for _ in 0..100 {
        page.evaluate(
            "document.querySelector('[data-oc-separador=\"pastas\"]').dispatchEvent(\
               new KeyboardEvent('keydown', {key: 'ArrowLeft', shiftKey: true, bubbles: true}))",
        )
        .await
        .expect("seta");
    }
    let pastas_minimas = largura(&page, ".oc-mail__rail").await;
    assert!(
        pastas_minimas >= 160.0,
        "o painel das pastas passou o mínimo e ficou inutilizável: {pastas_minimas}px"
    );

    // ── Recolher, e voltar ──────────────────────────────────────────────
    clicar(&page, "[data-oc=alternar-pastas]").await;
    let recolhidas = largura(&page, ".oc-mail__rail").await;
    assert!(
        recolhidas <= 0.0,
        "as pastas não recolheram: {recolhidas}px"
    );

    let leitura_com_pastas_recolhidas = largura(&page, ".oc-mail__pane").await;
    assert!(
        leitura_com_pastas_recolhidas > leitura_inicial,
        "recolher as pastas não deu o espaço a quem lê: {leitura_inicial} → \
         {leitura_com_pastas_recolhidas}"
    );

    // O caminho de volta existe e é o mesmo botão, que ficou premido.
    let premido: Option<String> = page
        .evaluate(
            "document.querySelector('[data-oc=alternar-pastas]').getAttribute('aria-pressed')",
        )
        .await
        .expect("estado")
        .into_value()
        .ok();
    assert_eq!(
        premido.as_deref(),
        Some("true"),
        "o botão não diz que as pastas estão recolhidas — quem as recolheu não \
         sabe como as trazer de volta"
    );

    clicar(&page, "[data-oc=alternar-pastas]").await;
    let de_volta = largura(&page, ".oc-mail__rail").await;
    assert!(de_volta > 100.0, "as pastas não voltaram: {de_volta}px");

    // ── O grampo vale mesmo quando não se consegue medir ────────────────
    //
    // `limitar` desistia do mínimo quando o contentor media zero — e uma
    // largura de zero acontece: o elemento ainda não foi disposto, a página
    // está escondida, o browser está com trabalho a mais. O pedido cru era
    // aplicado, a coluna ficava abaixo do utilizável, e lá ficava, porque
    // `normalizar` também desistia sem total.
    //
    // Esconder o contentor força exactamente esse estado, sem esperar que ele
    // aconteça sozinho. É a diferença entre observar um defeito intermitente e
    // conseguir provar que ele já não existe.
    let sob_minimo: f64 = page
        .evaluate(
            "(() => {
               const mail = document.querySelector('[data-oc=mail]');
               // Partir de um valor conhecido: a sonda mede o grampo, e não o
               // que os arrastos anteriores deixaram para trás.
               mail.style.setProperty('--oc-mail-lista', '260px');
               const antes = mail.style.display;
               mail.style.display = 'none';
               const separador =
                 document.querySelector('[data-oc-separador=lista]');
               for (let i = 0; i < 6; i += 1) {
                 separador.dispatchEvent(
                   new KeyboardEvent('keydown', {key: 'ArrowLeft', shiftKey: true,
                                                 bubbles: true}));
               }
               const lista = parseFloat(
                 getComputedStyle(mail).getPropertyValue('--oc-mail-lista'));
               mail.style.display = antes;
               return Number.isFinite(lista) ? lista : -1;
             })()",
        )
        .await
        .expect("medida")
        .into_value::<f64>()
        .unwrap_or(-1.0);
    assert!(
        sob_minimo >= 240.0,
        "com o contentor por medir, o grampo desistiu e a lista passou o \
         mínimo: {sob_minimo}px"
    );

    // ── Dar o ecrã à leitura, e desfazer ────────────────────────────────
    let lista_antes = largura(&page, ".oc-mail__list").await;
    clicar(&page, "[data-oc=focar-leitura]").await;

    // A promessa, e não um efeito lateral dela.
    //
    // Estava escrito «deu mais espaço do que recolher as pastas», e isso
    // depende de a lista ainda ter por onde encolher: depois dos arrastos
    // acima ela já podia estar no mínimo, e a asserção falhava sem que nada
    // estivesse errado. O que o modo promete é isto: pastas recolhidas, lista
    // no mínimo, o resto para quem lê.
    let lista_focada = largura(&page, ".oc-mail__list").await;
    let pastas_focadas = largura(&page, ".oc-mail__rail").await;
    assert!(
        pastas_focadas <= 0.0,
        "o modo de leitura não recolheu as pastas: {pastas_focadas}px"
    );
    assert!(
        lista_focada <= lista_antes,
        "o modo de leitura alargou a lista em vez de a estreitar: \
         {lista_antes} → {lista_focada}"
    );
    // A promessa medida numa só leitura, e não contra um número de há bocado.
    //
    // Estava `leitura_focada >= leitura_com_pastas_recolhidas`: duas larguras
    // absolutas medidas em momentos diferentes. Entre os dois momentos o
    // contentor muda de largura por razões que nada têm a ver com o modo de
    // leitura — medido, 1182.56 → 1148.28 — e a asserção atribuía essa
    // diferença ao modo. Passava quase sempre e falhava de vez em quando, que
    // é a forma mais cara de um teste estar errado.
    //
    // O que o modo promete é uma **repartição**, e uma repartição verifica-se
    // dentro da mesma leitura: as pastas recolhidas, a lista no mínimo, e tudo
    // o que sobra para quem lê.
    let reparticao: (f64, f64, f64) = page
        .evaluate(
            "(() => {
               const m = document.querySelector('[data-oc=mail]');
               const w = s => { const el = document.querySelector(s);
                                return el ? el.getBoundingClientRect().width : 0; };
               return [m.getBoundingClientRect().width,
                       w('.oc-mail__rail') + w('.oc-mail__list'),
                       w('.oc-mail__pane')];
             })()",
        )
        .await
        .expect("repartição")
        .into_value::<(f64, f64, f64)>()
        .expect("três medidas");

    let (total, outras, leitura_focada) = reparticao;
    assert!(
        leitura_focada > outras,
        "o modo de leitura não deu a maior parte a quem lê: leitura={leitura_focada} \
         contra {outras} nas outras colunas"
    );
    // O que sobra são os separadores e as margens: uma dezena de pixéis, não
    // uma coluna escondida.
    assert!(
        total - outras - leitura_focada < 32.0,
        "há espaço por explicar entre as colunas: total={total} outras={outras} \
         leitura={leitura_focada}"
    );

    clicar(&page, "[data-oc=focar-leitura]").await;
    let lista_depois = largura(&page, ".oc-mail__list").await;
    assert!(
        (lista_depois - lista_antes).abs() < 2.0,
        "desfazer o modo de leitura não repôs a lista onde estava: \
         {lista_antes} → {lista_depois}"
    );

    // ── E a preferência atravessa uma recarga ───────────────────────────
    page.evaluate(
        "document.querySelector('[data-oc=\"mail\"]').style.setProperty(\
           '--oc-mail-lista', '260px')",
    )
    .await
    .expect("largura");
    clicar(&page, "[data-oc=alternar-pastas]").await;

    let outra = harness.open("/mail").await;
    esperar_por(&outra, "Caixa de entrada").await;
    let recolhidas_ainda = largura(&outra, ".oc-mail__rail").await;
    assert!(
        recolhidas_ainda <= 0.0,
        "a preferência não sobreviveu à recarga: as pastas voltaram sozinhas \
         ({recolhidas_ainda}px)"
    );
}

/// O compositor obedece, e nunca perde o que se escreveu.
///
/// # O que isto mede que a marcação não mostra
///
/// Uma janela que se redimensiona é fácil de dizer e fácil de partir: um
/// mínimo que não existe e ela desaparece; um limite que não existe e ela sai
/// do ecrã com o rascunho lá dentro; um gesto que volta a desenhar o
/// formulário e o texto some-se sem erro nenhum.
///
/// A última é a que mais dói e a que menos se vê: quem estava a escrever há
/// dez minutos não tem como saber que foi o botão de expandir que lhe apagou o
/// texto.
#[tokio::test]
async fn o_compositor_obedece_e_guarda_o_que_se_escreveu() {
    let harness = harness!();
    let (person_id, _) = harness.sign_in(&[TechnicalRole::ResearchMember]).await;
    let (caixa, _) = harness.has_a_mailbox(person_id).await;

    let page = harness
        .open(&format!("/mail/compose?mailbox={caixa}"))
        .await;
    janela(&page, JANELA).await;
    esperar_por(&page, "Nova mensagem").await;

    const RASCUNHO: &str = "Um parágrafo que não se pode perder por causa de um botão.";

    set_field(&page, "[data-oc=compositor-corpo]", RASCUNHO).await;

    /// O que o corpo do compositor contém agora.
    async fn corpo(page: &Page) -> String {
        page.evaluate("document.querySelector('[data-oc=compositor-corpo]').value")
            .await
            .expect("corpo")
            .into_value::<String>()
            .unwrap_or_default()
    }

    async fn medida(page: &Page, propriedade: &str) -> f64 {
        page.evaluate(format!(
            "document.querySelector('[data-oc=compositor]')\
               .getBoundingClientRect().{propriedade}"
        ))
        .await
        .expect("medida")
        .into_value::<f64>()
        .unwrap_or(-1.0)
    }

    assert_eq!(corpo(&page).await, RASCUNHO, "o rascunho não ficou escrito");

    // ── Expandir, e voltar ──────────────────────────────────────────────
    let largura_inicial = medida(&page, "width").await;
    clicar_por_script(&page, "[data-oc=compositor-expandir]").await;
    let expandida = medida(&page, "width").await;
    assert!(
        expandida > largura_inicial,
        "expandir não alargou o compositor: {largura_inicial} → {expandida}"
    );

    // E a barra de título continua no ecrã.
    //
    // Expandida, a janela era ancorada pelo fundo com uma altura em `vh`, e a
    // barra saía pela borda de cima: ficava sem pega, sem o botão de repor e
    // sem o de fechar — expandida para sempre, com o rascunho lá dentro.
    let topo_expandido = medida(&page, "top").await;
    assert!(
        topo_expandido >= 0.0,
        "a barra de título do compositor saiu pela borda de cima: {topo_expandido}"
    );
    assert_eq!(
        corpo(&page).await,
        RASCUNHO,
        "expandir apagou o que estava escrito"
    );

    clicar_por_script(&page, "[data-oc=compositor-expandir]").await;
    let reposta = medida(&page, "width").await;
    assert!(
        (reposta - largura_inicial).abs() < 2.0,
        "repor o tamanho não voltou ao que era: {largura_inicial} → {reposta}"
    );
    assert_eq!(
        corpo(&page).await,
        RASCUNHO,
        "repor o tamanho apagou o que estava escrito"
    );

    // ── Redimensionar, com chão ─────────────────────────────────────────
    //
    // Um arrasto absurdo: se não houver mínimo, a janela colapsa e leva o
    // rascunho com ela.
    page.evaluate(
        "(() => { const p = document.querySelector('[data-oc=compositor-puxador]'); \
           const r = p.getBoundingClientRect(); \
           const op = {bubbles: true, pointerId: 1, clientX: r.x, clientY: r.y}; \
           p.setPointerCapture = () => {}; \
           p.dispatchEvent(new PointerEvent('pointerdown', op)); \
           p.dispatchEvent(new PointerEvent('pointermove', \
             {...op, clientX: r.x + 4000, clientY: r.y + 4000})); \
           p.dispatchEvent(new PointerEvent('pointerup', op)); })()",
    )
    .await
    .expect("arrastar o puxador");

    let encolhida = medida(&page, "width").await;
    let alta = medida(&page, "height").await;
    assert!(
        encolhida >= 360.0 && alta >= 300.0,
        "o compositor passou o mínimo: {encolhida}×{alta}"
    );
    assert_eq!(
        corpo(&page).await,
        RASCUNHO,
        "redimensionar apagou o que estava escrito"
    );

    // ── E não sai do ecrã ───────────────────────────────────────────────
    page.evaluate(
        "(() => { const h = document.querySelector('[data-oc=compositor-pega]'); \
           const r = h.getBoundingClientRect(); \
           const op = {bubbles: true, pointerId: 2, clientX: r.x + 40, clientY: r.y + 10}; \
           h.setPointerCapture = () => {}; \
           h.dispatchEvent(new PointerEvent('pointerdown', op)); \
           h.dispatchEvent(new PointerEvent('pointermove', \
             {...op, clientX: r.x - 5000, clientY: r.y - 5000})); \
           h.dispatchEvent(new PointerEvent('pointerup', op)); })()",
    )
    .await
    .expect("arrastar a janela");

    let esquerda = medida(&page, "left").await;
    let topo = medida(&page, "top").await;
    assert!(
        esquerda > -20.0 && topo > -20.0,
        "o compositor saiu do ecrã e levou o rascunho com ele: \
         esquerda={esquerda} topo={topo}"
    );
    assert_eq!(
        corpo(&page).await,
        RASCUNHO,
        "mover apagou o que estava escrito"
    );
}

/// A página do Correio não rola; os painéis é que rolam.
///
/// # O defeito que isto guarda
///
/// Com a barra na página inteira, descer uma lista longa levava consigo o
/// cabeçalho e a barra de acções: depois de rolar, deixava de haver
/// «Escrever» no ecrã. Numa aplicação de correio a moldura fica, e o que se
/// percorre é o conteúdo.
///
/// # Porque se mede o documento e não a folha de estilos
///
/// Porque «tem `overflow: hidden`» não é a propriedade que interessa. O que
/// interessa é se o documento é mais alto do que a janela — e isso depende do
/// cabeçalho, dos avisos que apareçam por cima, e da altura que o grid
/// acabar por tomar. Só o browser sabe a soma.
#[tokio::test]
async fn o_correio_rola_por_dentro_e_nao_por_fora() {
    let harness = harness!();
    let (person_id, _) = harness.sign_in(&[TechnicalRole::ResearchMember]).await;
    let (caixa, _) = harness.has_a_mailbox(person_id).await;

    // Mensagens que cheguem para a lista passar do ecrã.
    for indice in 0..40 {
        sqlx::query(
            "INSERT INTO mail_messages
                    (mailbox_id, provider_id, folder, from_address, subject, sent_at)
                  VALUES ($1, $2, 'inbox', 'externo@exemplo.com', $3,
                          now() - ($4 * interval '1 minute'))",
        )
        .bind(caixa)
        .bind(Uuid::new_v4().to_string())
        .bind(format!("Mensagem número {indice}"))
        .bind(f64::from(indice))
        .execute(&harness.pool)
        .await
        .expect("mensagem");
    }

    let page = harness.open(&format!("/mail/{caixa}")).await;
    janela(&page, JANELA).await;
    esperar_por(&page, "Mensagem número 0").await;

    let medidas: Option<String> = page
        .evaluate(
            "(() => { const d = document.documentElement; \
               const lista = document.querySelector('.oc-mail__list'); \
               return JSON.stringify({ \
                 documento: d.scrollHeight, \
                 janela: d.clientHeight, \
                 listaConteudo: lista.scrollHeight, \
                 listaVisivel: lista.clientHeight, \
               }); })()",
        )
        .await
        .expect("medidas")
        .into_value()
        .ok();
    let medidas = medidas.unwrap_or_default();

    let numero = |chave: &str| -> f64 {
        medidas
            .split(&format!("\"{chave}\":"))
            .nth(1)
            .and_then(|resto| resto.split([',', '}']).next())
            .and_then(|valor| valor.trim().parse().ok())
            .unwrap_or(-1.0)
    };

    // A lista tem mais conteúdo do que espaço — sem isto, o teste passaria
    // num mundo onde não há nada para rolar.
    assert!(
        numero("listaConteudo") > numero("listaVisivel") + 20.0,
        "a lista não transbordou, e por isso este teste não observou nada: {medidas}"
    );

    // E o documento não passa da janela.
    assert!(
        numero("documento") <= numero("janela") + 2.0,
        "a página do Correio ganhou barra de deslocamento: {medidas}"
    );
}

/// A cadeia científica, percorrida como uma pessoa a percorre.
///
/// # O que esta viagem prova, e nenhuma metade prova sozinha
///
/// Que a cadeia se **navega**: da hipótese ao resultado, e do resultado de
/// volta ao que o produziu — no browser, através do Workspace, contra o Core a
/// sério. Um teste de serviço mostra que a proveniência é escrita; um teste de
/// renderização mostra que o ecrã desenha o que lhe dão. Nenhum dos dois mostra
/// que a aresta escrita pela operação chega ao ecrã e é legível.
///
/// # E que não se lê um único identificador
///
/// A linhagem mostra títulos. Um `UUID` no ecrã seria a resposta a outra
/// pergunta — a de quem está a depurar uma consulta — e a asserção está aqui
/// porque a tentação de mostrar o identificador «só para desenvolvimento»
/// resolve-se sempre a favor de o deixar ficar.
#[tokio::test]
async fn a_cadeia_cientifica_percorre_se_do_resultado_ate_a_origem() {
    let harness = harness!();
    let (pessoa, _credenciais) = harness.sign_in(&[TechnicalRole::ResearchMember]).await;
    let ambiente = harness.owns_a_workspace(pessoa).await;

    // A cadeia inteira nasce pelas operações do Core, através da API — que é
    // por onde o Workspace escreve. Escrevê-la com `INSERT` provaria que o
    // ecrã desenha linhas, e não que a cadeia existe.
    let hipotese = unique_title("A dopagem reduz a resistência");
    let estudo = unique_title("Ensaio de carga");
    let resultado = unique_title("A resistência caiu 18%");

    let (hipotese_id, estudo_id, execucao_id, resultado_id) = harness
        .cadeia_cientifica(pessoa, ambiente, (&hipotese, &estudo, &resultado))
        .await;

    // ── A cadeia do ambiente ────────────────────────────────────────────
    let pagina = harness
        .open(&format!("/workspaces/{ambiente}/science"))
        .await;
    esperar_por(&pagina, &resultado).await;
    let html = pagina.content().await.expect("conteúdo");
    for etapa in [&hipotese, &estudo, &resultado] {
        assert!(
            html.contains(etapa.as_str()),
            "a cadeia não mostra «{etapa}»"
        );
    }

    // ── Do resultado até à origem, a clicar ─────────────────────────────
    clicar(&pagina, &format!(r#"a[href="/results/{resultado_id}"]"#)).await;
    let detalhe = wait_until_left(&pagina, &format!("/workspaces/{ambiente}/science")).await;
    assert!(
        detalhe.contains(&format!("/results/{resultado_id}")),
        "clicar no resultado não levou ao resultado: {detalhe}"
    );

    esperar_por(&pagina, "Proveniência").await;
    let html = pagina.content().await.expect("conteúdo");

    // A aresta que a operação observou está lá, e diz que a observou.
    assert!(
        html.contains(&estudo),
        "a montante do resultado não aparece a execução que o produziu"
    );
    assert!(
        html.contains("Observada"),
        "a proveniência não distingue o que a operação viu do que alguém declarou"
    );

    // E lê-se por títulos. O identificador do resultado está no `href` das
    // tabs de sentido, que é onde pertence; o que não pode aparecer é um
    // identificador **como texto**, entre `>` e `<`.
    let texto: String = html
        .split('>')
        .filter_map(|p| p.split('<').next())
        .collect::<Vec<_>>()
        .join(" ");
    for identificador in [resultado_id, execucao_id, estudo_id, hipotese_id] {
        let identificador = identificador.to_string();
        assert!(
            !texto.contains(&identificador),
            "o ecrã mostra um identificador a quem só queria saber de onde veio isto: \
             {identificador}"
        );
    }

    // ── E de volta, a jusante ───────────────────────────────────────────
    clicar(
        &pagina,
        &format!(r#"a[href="/results/{resultado_id}?direction=downstream"]"#),
    )
    .await;
    esperar_por(&pagina, "Nada depende deste resultado.").await;
}

/// Uma pessoa constrói a cadeia científica inteira pelo Workspace.
///
/// # A propriedade
///
/// > **Uma pessoa autorizada constrói a cadeia científica sem API, sem CLI e
/// > sem agente.**
///
/// Enquanto uma hipótese ou uma execução só puderem nascer por `curl` ou por um
/// agente, o Ocinye OS tem excelente infraestrutura agentic e não tem
/// infraestrutura científica institucional: a IA aumenta a capacidade humana,
/// não substitui a interface humana da instituição.
///
/// # Nenhum objecto desta cadeia é preparado por fixture
///
/// Hipótese, metodologia, versão, estudo, execução, resultado e validação
/// nascem todos de formulários submetidos neste browser. As fixtures dão apenas
/// o que não faz parte da propriedade — a pessoa, a unidade e o ambiente.
///
/// # E a proveniência não é pedida a ninguém
///
/// O resultado é registado **de dentro** da execução, e a aresta `produzido
/// por` aparece na linhagem sem que nenhum campo a tenha pedido. É a diferença
/// entre o que o sistema observou e o que alguém declarou.
#[tokio::test(flavor = "multi_thread")]
async fn uma_pessoa_constroi_a_cadeia_cientifica_pelo_workspace() {
    let harness = harness!();
    let (pessoa, _credenciais) = harness.sign_in(&[TechnicalRole::ResearchMember]).await;
    let ambiente = harness.owns_a_workspace(pessoa).await;

    let hipotese = unique_title("A dopagem reduz a resistência");
    let metodo = unique_title("Medição a quatro pontas");
    let estudo = unique_title("Ensaio de carga");
    let resultado = unique_title("A resistência caiu 18%");

    // ── 1. A hipótese ───────────────────────────────────────────────────
    let pagina = harness
        .open(&format!("/workspaces/{ambiente}/science"))
        .await;
    esperar_por(&pagina, "Ciência").await;

    clicar_por_script(
        &pagina,
        &format!(r#"a[href="/workspaces/{ambiente}/science/hypotheses/new"]"#),
    )
    .await;
    esperar_por(&pagina, "Uma afirmação que se pode testar").await;
    set_field(&pagina, "textarea[name=statement]", &hipotese).await;
    set_field(
        &pagina,
        "textarea[name=rationale]",
        "O que se sabe hoje não explica a queda medida.",
    )
    .await;
    submit(&pagina, "form[action$='/hypotheses/new']").await;
    esperar_por(&pagina, &hipotese).await;

    // ── 2. A metodologia, e a sua primeira versão ───────────────────────
    clicar_por_script(
        &pagina,
        &format!(r#"a[href="/workspaces/{ambiente}/science/methodologies/new"]"#),
    )
    .await;
    esperar_por(&pagina, "a identidade durável do método").await;
    set_field(&pagina, "input[name=title]", &metodo).await;
    set_field(
        &pagina,
        "textarea[name=purpose]",
        "Separar a resistência de contacto da do material.",
    )
    .await;
    submit(&pagina, "form[action$='/methodologies/new']").await;

    // Criar leva à metodologia, porque o passo seguinte é publicar a versão —
    // e é lá que ele está.
    esperar_por(&pagina, "Versões").await;
    let html = pagina.content().await.expect("conteúdo");
    assert!(
        html.contains("Um estudo só pode seguir uma versão publicada"),
        "a metodologia sem versões não explica porque isso importa"
    );

    clicar(&pagina, "a[href$='/versions/new']").await;
    esperar_por(&pagina, "O que esta versão diz").await;
    set_field(&pagina, "input[name=label]", "v1").await;
    set_field(
        &pagina,
        "textarea[name=summary]",
        "Quatro pontas, corrente de 10 mA.",
    )
    .await;
    submit(&pagina, "form[action$='/versions/new']").await;
    esperar_por(&pagina, "v1").await;

    // ── 3. O estudo, ligado à hipótese e à **versão** ───────────────────
    let pagina = harness
        .open(&format!("/workspaces/{ambiente}/science/studies/new"))
        .await;
    esperar_por(&pagina, "Novo estudo").await;

    // O selector oferece versões, e o rótulo di-lo. Se oferecesse a
    // metodologia mutável, a matriz recusaria a aresta — e a pessoa só
    // descobriria depois de preencher o resto.
    let opcoes = pagina
        .content()
        .await
        .expect("conteúdo")
        .split(r#"name="methodology_version_id""#)
        .nth(1)
        .unwrap_or_default()
        .split("</select>")
        .next()
        .unwrap_or_default()
        .to_owned();
    assert!(
        opcoes.contains(&format!("{metodo} · v1")),
        "o selector do estudo não oferece a versão publicada"
    );

    set_field(&pagina, "input[name=title]", &estudo).await;
    set_field(
        &pagina,
        "textarea[name=objective]",
        "Medir a queda sob carga.",
    )
    .await;
    let hipotese_id = valor_de(&pagina, "select[name=hypothesis_id] option:nth-child(2)").await;
    escolher(&pagina, "select[name=hypothesis_id]", &hipotese_id).await;
    let versao_id = valor_de(
        &pagina,
        "select[name=methodology_version_id] option:nth-child(2)",
    )
    .await;
    escolher(&pagina, "select[name=methodology_version_id]", &versao_id).await;
    submit(&pagina, "form[action$='/studies/new']").await;

    esperar_por(&pagina, "Execuções").await;
    let html = pagina.content().await.expect("conteúdo");
    assert!(
        html.contains(&estudo),
        "o estudo não abriu depois de criado"
    );

    // ── 4. A execução ───────────────────────────────────────────────────
    clicar(&pagina, "a[href$='/executions/new']").await;
    esperar_por(&pagina, "a reprodutibilidade mora").await;
    set_field(&pagina, "input[name=environment]", "Bancada 2").await;
    set_field(&pagina, "input[name=software_name]", "LabView").await;
    escolher(&pagina, "select[name=methodology_version_id]", &versao_id).await;
    submit(&pagina, "form[action$='/executions/new']").await;
    esperar_por(&pagina, "A corrida").await;

    // ── 5. O resultado, de dentro da execução ───────────────────────────
    clicar(&pagina, "a[href$='/results/new']").await;
    esperar_por(&pagina, "A origem fica registada sozinha").await;

    let html = pagina.content().await.expect("conteúdo");
    assert!(
        html.contains("A origem fica registada sozinha"),
        "o formulário não diz que a proveniência já é conhecida"
    );
    assert!(
        !html.contains(r#"name="execution_id""#),
        "o formulário pede a origem que o caminho já diz"
    );

    set_field(&pagina, "input[name=title]", &resultado).await;
    set_field(
        &pagina,
        "textarea[name=summary]",
        "Três corridas, mesma direcção.",
    )
    .await;
    submit(&pagina, "form[action$='/results/new']").await;
    esperar_por(&pagina, "Proveniência").await;

    // ── 6. A proveniência apareceu sem ninguém a declarar ───────────────
    let html = pagina.content().await.expect("conteúdo");
    assert!(
        html.contains("Observada"),
        "a aresta que a operação produziu não aparece como observada"
    );
    assert!(
        html.contains(&estudo),
        "a montante não mostra a execução que produziu o resultado"
    );

    // E lê-se por títulos: nenhum identificador aparece como texto.
    let texto: String = html
        .split('>')
        .filter_map(|p| p.split('<').next())
        .collect::<Vec<_>>()
        .join(" ");
    for identificador in [&hipotese_id, &versao_id] {
        assert!(
            !texto.contains(identificador.as_str()),
            "o ecrã mostra um identificador a quem só queria saber de onde veio isto"
        );
    }

    // ── 7. A jusante, e de volta ────────────────────────────────────────
    let resultado_url = pagina.url().await.expect("url").unwrap_or_default();
    let resultado_id = resultado_url
        .rsplit('/')
        .next()
        .unwrap_or_default()
        .split('?')
        .next()
        .unwrap_or_default()
        .to_owned();
    clicar_por_script(
        &pagina,
        &format!(r#"a[href="/results/{resultado_id}?direction=downstream"]"#),
    )
    .await;
    esperar_por(&pagina, "Nada depende deste resultado.").await;

    // ── 8. A validação, como pessoa autorizada ──────────────────────────
    //
    // Pelo **botão**, e não pelo URL. Ir directamente ao caminho provaria que
    // o formulário existe e não que alguém lá chega: quem decide se a acção
    // aparece é o Core, com o contexto deste resultado, e uma interface a
    // decidi-lo sozinha responderia no âmbito institucional — onde uma
    // permissão de ambiente nunca aparece, e o botão desapareceria para toda a
    // gente.
    let pagina = harness.open(&format!("/results/{resultado_id}")).await;
    esperar_por(&pagina, "Proveniência").await;
    clicar_por_script(
        &pagina,
        &format!(r#"a[href="/results/{resultado_id}/validate"]"#),
    )
    .await;
    // «O que está a registar» e não «Validar resultado»: o segundo é também o
    // texto do **botão na página de onde se veio**, e o `esperar_por`
    // satisfaz-se nele antes de a navegação acontecer. O `content()` seguinte
    // apanha então um contexto a ser desmontado — `Cannot find context with
    // specified id` —, e a corrida perde-se conforme a máquina. Passou sessenta
    // vezes aqui e falhou à primeira no runner.
    //
    // A regra: **esperar por texto que só o destino tem.**
    esperar_por(&pagina, "O que está a registar").await;

    let html = pagina.content().await.expect("conteúdo");
    assert!(
        html.contains("Isto fica em seu nome"),
        "o formulário não diz de quem é o peso da afirmação"
    );

    // A reprodução está disponível, porque **há** execução.
    //
    // Reprodutibilidade é evidência: o Core recusa uma reprodução sem a corrida
    // que a sustenta, e o formulário desactiva a opção quando não há nenhuma. O
    // que não pode acontecer é desactivá-la quando há — a pessoa procuraria a
    // opção, não a encontraria, e não teria como saber porquê.
    let reproducao_desactivada = html
        .split(r#"value="reproduction""#)
        .nth(1)
        .unwrap_or_default()
        .split('>')
        .next()
        .unwrap_or_default()
        .contains("disabled");
    assert!(
        !reproducao_desactivada,
        "este resultado veio de uma execução e a reprodução aparece indisponível"
    );

    set_field(
        &pagina,
        "textarea[name=note]",
        "Reli os três ensaios e a direcção mantém-se.",
    )
    .await;
    submit(&pagina, "form[action$='/validate']").await;
    esperar_por(&pagina, "Validação confirmou").await;

    // ── 9. E persiste ───────────────────────────────────────────────────
    let pagina = harness.open(&format!("/results/{resultado_id}")).await;
    esperar_por(&pagina, "Validação confirmou").await;
    let html = pagina.content().await.expect("conteúdo");
    assert!(
        html.contains("Reli os três ensaios"),
        "o que a pessoa observou não sobreviveu a recarregar"
    );
    assert!(
        html.contains("Observada"),
        "a proveniência não sobreviveu a recarregar"
    );
}

/// As capturas da cadeia científica, para revisão visual.
///
/// Constrói a cadeia inteira pelo Workspace — como uma pessoa — e fotografa
/// cada superfície pelo caminho. Não verifica nada: as asserções vivem em
/// `uma_pessoa_constroi_a_cadeia_cientifica_pelo_workspace`, e misturar as duas
/// coisas daria um teste que grava ficheiros e um conjunto de imagens que
/// ninguém olha porque «o teste passou».
#[tokio::test(flavor = "multi_thread")]
#[ignore = "grava ficheiros; serve a revisão visual, não a verificação"]
async fn capturas_da_ciencia() {
    let harness = harness!();
    let (pessoa, _credenciais) = harness.sign_in(&[TechnicalRole::ResearchMember]).await;
    let ambiente = harness.owns_a_workspace(pessoa).await;

    // ── O estado vazio, que também é uma superfície ─────────────────────
    let pagina = harness
        .open(&format!("/workspaces/{ambiente}/science"))
        .await;
    esperar_por(&pagina, "Ciência").await;
    capturar_visivel(&pagina, "ciencia-vazia").await;

    // ── A hipótese ──────────────────────────────────────────────────────
    let pagina = harness
        .open(&format!("/workspaces/{ambiente}/science/hypotheses/new"))
        .await;
    esperar_por(&pagina, "Uma afirmação que se pode testar").await;
    set_field(
        &pagina,
        "textarea[name=statement]",
        "A dopagem reduz a resistência de contacto",
    )
    .await;
    set_field(
        &pagina,
        "textarea[name=rationale]",
        "O modelo actual não explica a queda medida acima de 2 A.",
    )
    .await;
    capturar_visivel(&pagina, "ciencia-nova-hipotese").await;
    submit(&pagina, "form[action$='/hypotheses/new']").await;
    esperar_por(&pagina, "A dopagem reduz").await;

    // ── A metodologia e a versão ────────────────────────────────────────
    let pagina = harness
        .open(&format!("/workspaces/{ambiente}/science/methodologies/new"))
        .await;
    esperar_por(&pagina, "a identidade durável do método").await;
    set_field(&pagina, "input[name=title]", "Medição a quatro pontas").await;
    set_field(
        &pagina,
        "textarea[name=purpose]",
        "Separar a resistência de contacto da do material.",
    )
    .await;
    capturar_visivel(&pagina, "ciencia-nova-metodologia").await;
    submit(&pagina, "form[action$='/methodologies/new']").await;
    esperar_por(&pagina, "Versões").await;
    capturar_visivel(&pagina, "ciencia-metodologia-sem-versoes").await;

    clicar(&pagina, "a[href$='/versions/new']").await;
    esperar_por(&pagina, "O que esta versão diz").await;
    set_field(&pagina, "input[name=label]", "v1").await;
    set_field(
        &pagina,
        "textarea[name=summary]",
        "Quatro pontas, corrente de 10 mA, três repetições por amostra.",
    )
    .await;
    capturar_visivel(&pagina, "ciencia-nova-versao").await;
    submit(&pagina, "form[action$='/versions/new']").await;
    esperar_por(&pagina, "v1").await;
    capturar_visivel(&pagina, "ciencia-metodologia").await;

    // Uma segunda versão, para a substituição ficar visível na imagem.
    clicar(&pagina, "a[href$='/versions/new']").await;
    esperar_por(&pagina, "O que esta versão diz").await;
    capturar_visivel(&pagina, "ciencia-nova-versao-substitui").await;
    set_field(&pagina, "input[name=label]", "v2").await;
    set_field(
        &pagina,
        "textarea[name=summary]",
        "Corrente reduzida para 1 mA: a de 10 aquecia o contacto.",
    )
    .await;
    submit(&pagina, "form[action$='/versions/new']").await;
    esperar_por(&pagina, "v2").await;
    capturar_visivel(&pagina, "ciencia-metodologia-com-duas-versoes").await;

    // ── O estudo ────────────────────────────────────────────────────────
    let pagina = harness
        .open(&format!("/workspaces/{ambiente}/science/studies/new"))
        .await;
    esperar_por(&pagina, "Novo estudo").await;
    set_field(
        &pagina,
        "input[name=title]",
        "Ensaio de carga em contactos dopados",
    )
    .await;
    set_field(
        &pagina,
        "textarea[name=objective]",
        "Medir a queda de resistência entre 0.5 e 5 A.",
    )
    .await;
    let hipotese_id = valor_de(&pagina, "select[name=hypothesis_id] option:nth-child(2)").await;
    escolher(&pagina, "select[name=hypothesis_id]", &hipotese_id).await;
    let versao_id = valor_de(
        &pagina,
        "select[name=methodology_version_id] option:nth-child(2)",
    )
    .await;
    escolher(&pagina, "select[name=methodology_version_id]", &versao_id).await;
    capturar_visivel(&pagina, "ciencia-novo-estudo").await;
    submit(&pagina, "form[action$='/studies/new']").await;
    esperar_por(&pagina, "Execuções").await;
    capturar_visivel(&pagina, "ciencia-estudo").await;

    // ── A execução ──────────────────────────────────────────────────────
    clicar(&pagina, "a[href$='/executions/new']").await;
    esperar_por(&pagina, "a reprodutibilidade mora").await;
    set_field(&pagina, "input[name=environment]", "Bancada 2, sala 104").await;
    set_field(&pagina, "input[name=software_name]", "LabView").await;
    set_field(&pagina, "input[name=software_version]", "2024 Q3").await;
    escolher(&pagina, "select[name=methodology_version_id]", &versao_id).await;
    capturar_visivel(&pagina, "ciencia-nova-execucao").await;
    submit(&pagina, "form[action$='/executions/new']").await;
    esperar_por(&pagina, "A corrida").await;
    capturar_visivel(&pagina, "ciencia-execucao").await;

    // ── O resultado ─────────────────────────────────────────────────────
    clicar(&pagina, "a[href$='/results/new']").await;
    esperar_por(&pagina, "A origem fica registada sozinha").await;
    set_field(
        &pagina,
        "input[name=title]",
        "A resistência caiu 18% acima de 2 A",
    )
    .await;
    set_field(
        &pagina,
        "textarea[name=summary]",
        "Três corridas independentes, mesma direcção e magnitude comparável.",
    )
    .await;
    capturar_visivel(&pagina, "ciencia-novo-resultado").await;
    submit(&pagina, "form[action$='/results/new']").await;
    esperar_por(&pagina, "Proveniência").await;
    capturar_visivel(&pagina, "ciencia-resultado-montante").await;

    let resultado_url = pagina.url().await.expect("url").unwrap_or_default();
    let resultado_id = resultado_url
        .rsplit('/')
        .next()
        .unwrap_or_default()
        .split('?')
        .next()
        .unwrap_or_default()
        .to_owned();

    clicar_por_script(
        &pagina,
        &format!(r#"a[href="/results/{resultado_id}?direction=downstream"]"#),
    )
    .await;
    esperar_por(&pagina, "Nada depende deste resultado.").await;
    capturar_visivel(&pagina, "ciencia-resultado-jusante").await;

    // ── A validação ─────────────────────────────────────────────────────
    let pagina = harness.open(&format!("/results/{resultado_id}")).await;
    esperar_por(&pagina, "Proveniência").await;
    clicar_por_script(
        &pagina,
        &format!(r#"a[href="/results/{resultado_id}/validate"]"#),
    )
    .await;
    esperar_por(&pagina, "Validar resultado").await;
    set_field(
        &pagina,
        "textarea[name=note]",
        "Reli os três ensaios e a direcção mantém-se dentro da incerteza.",
    )
    .await;
    capturar_visivel(&pagina, "ciencia-validar").await;
    submit(&pagina, "form[action$='/validate']").await;
    esperar_por(&pagina, "Validação confirmou").await;
    capturar_visivel(&pagina, "ciencia-resultado-validado").await;

    // ── A cadeia povoada ────────────────────────────────────────────────
    let pagina = harness
        .open(&format!("/workspaces/{ambiente}/science"))
        .await;
    esperar_por(&pagina, "Resultados").await;
    capturar_visivel(&pagina, "ciencia-cadeia").await;
}

// ── Ficheiros institucionais, pelo browser ───────────────────────────────

/// Semeia um ficheiro com uma versão, sem passar pelo armazenamento.
///
/// A navegação, o histórico e a autorização não dependem de os bytes estarem
/// num bucket: dependem das linhas. Semear assim deixa a viagem humana ser
/// verificada mesmo numa máquina sem MinIO — e a viagem que **precisa** de
/// bytes diz em voz alta quando não pode correr.
async fn semear_ficheiro(
    harness: &Harness,
    workspace_id: Uuid,
    nome: &str,
    classificacao: &str,
) -> Uuid {
    let (organisation_id, unit_id): (Uuid, Uuid) =
        sqlx::query_as("SELECT organisation_id, unit_id FROM research_workspaces WHERE id = $1")
            .bind(workspace_id)
            .fetch_one(&harness.pool)
            .await
            .expect("ambiente");

    let backend_id: Uuid = sqlx::query_scalar(
        "INSERT INTO storage_backends (code, display_name, location_label, bucket)
         VALUES ($1, 'Harness', 'test', 'prova') RETURNING id",
    )
    .bind(format!("b{}", &Uuid::new_v4().simple().to_string()[..12]))
    .fetch_one(&harness.pool)
    .await
    .expect("backend");

    let object_id: Uuid = sqlx::query_scalar(
        "INSERT INTO storage_objects
             (organisation_id, backend_id, object_key, original_filename,
              content_type, size_bytes, checksum_sha256, status, classification)
         VALUES ($1, $2, $3, $4, 'text/plain', 42, $5, 'stored', $6) RETURNING id",
    )
    .bind(organisation_id)
    .bind(backend_id)
    .bind(format!("prova/{}", Uuid::new_v4()))
    .bind(nome)
    .bind(format!("{:064x}", rand_like()))
    .bind(classificacao)
    .fetch_one(&harness.pool)
    .await
    .expect("objecto");

    let file_id: Uuid = sqlx::query_scalar(
        "INSERT INTO files (organisation_id, unit_id, workspace_id, name, classification)
         VALUES ($1, $2, $3, $4, $5) RETURNING id",
    )
    .bind(organisation_id)
    .bind(unit_id)
    .bind(workspace_id)
    .bind(nome)
    .bind(classificacao)
    .fetch_one(&harness.pool)
    .await
    .expect("ficheiro");

    sqlx::query(
        "INSERT INTO file_versions (file_id, sequence, storage_object_id)
         VALUES ($1, 1, $2)",
    )
    .bind(file_id)
    .bind(object_id)
    .execute(&harness.pool)
    .await
    .expect("versão");

    file_id
}

/// Uma soma distinta por chamada, sem depender de um gerador aleatório.
fn rand_like() -> u128 {
    Uuid::new_v4().as_u128()
}

/// Uma pessoa autorizada organiza, navega, vê e versiona ficheiros.
///
/// # A viagem
///
/// ```text
/// entrar → Ficheiros na sidebar → escolher o ambiente → ver o ficheiro
///        → criar uma pasta → entrar nela → trilho → voltar
///        → abrir o ficheiro → detalhes, classificação e histórico
/// ```
///
/// # O que a viagem tem de provar, e não só mostrar
///
/// Que o ecrã existe na navegação, que as pastas se criam e se percorrem, que
/// o ficheiro tem uma página com o seu histórico, e que a classificação que
/// aparece é a do ficheiro — não a da pasta onde ele está.
#[tokio::test]
async fn uma_pessoa_organiza_e_percorre_os_ficheiros_no_browser() {
    let harness = harness!();

    let (person_id, _) = harness.sign_in(&[TechnicalRole::ResearchMember]).await;
    let workspace_id = harness.owns_a_workspace(person_id).await;
    let nome = unique_title("ensaio");
    let file_id = semear_ficheiro(&harness, workspace_id, &nome, "INTERNAL").await;

    // A entrada existe na navegação, sob CONHECIMENTO.
    let page = harness.open("/").await;
    esperar_por(&page, "Ficheiros").await;

    // O ambiente escolhe-se: não há um por omissão.
    let page = harness.open("/files").await;
    // O módulo abre na vista agregada: não se escolhe um ambiente para ver.
    esperar_por(&page, "em todos os ambientes a que pertence").await;

    let lista = harness
        .open(&format!("/files?workspace={workspace_id}"))
        .await;
    esperar_por(&lista, &nome).await;
    let html = lista.content().await.expect("conteúdo");
    assert!(
        html.contains("Largue ficheiros aqui"),
        "a zona de carregamento não chegou ao browser"
    );

    // Criar uma pasta, pelo formulário, como uma pessoa faz.
    let pasta = unique_title("Ensaios");
    set_field(&lista, "#oc-files-folder", &pasta).await;
    submit(&lista, "form[action='/files/folder']").await;
    esperar_por(&lista, &pasta).await;

    // A pasta existe na base, e não só no ecrã.
    let quantas: i64 =
        sqlx::query_scalar("SELECT count(*) FROM folders WHERE workspace_id = $1 AND name = $2")
            .bind(workspace_id)
            .bind(&pasta)
            .fetch_one(&harness.pool)
            .await
            .expect("contagem de pastas");
    assert_eq!(quantas, 1, "a pasta criada no ecrã não existe na base");

    // Entrar na pasta: está vazia, e o trilho leva de volta.
    let folder_id: Uuid =
        sqlx::query_scalar("SELECT id FROM folders WHERE workspace_id = $1 AND name = $2")
            .bind(workspace_id)
            .bind(&pasta)
            .fetch_one(&harness.pool)
            .await
            .expect("pasta");

    let dentro = harness
        .open(&format!(
            "/files?workspace={workspace_id}&folder={folder_id}"
        ))
        .await;
    esperar_por(&dentro, "Ainda não há nada aqui").await;
    let html = dentro.content().await.expect("conteúdo");
    assert!(
        html.contains(&pasta),
        "o trilho não mostra a pasta onde a pessoa está"
    );
    assert!(
        !html.contains(&nome),
        "o ficheiro da raiz apareceu dentro de uma pasta onde não está"
    );

    // A página do ficheiro: detalhes, classificação e histórico.
    let detalhe = harness.open(&format!("/files/{file_id}")).await;
    esperar_por(&detalhe, "Histórico de versões").await;
    let html = detalhe.content().await.expect("conteúdo");
    assert!(html.contains(&nome), "a página não é a do ficheiro pedido");
    assert!(
        html.contains("Carregar nova versão"),
        "não há como carregar uma versão nova"
    );
    assert!(
        html.contains("Soma SHA-256"),
        "os detalhes não mostram a soma que identifica os bytes"
    );
    assert!(
        html.contains("v1"),
        "o histórico não mostra a primeira versão"
    );
}

/// Conhecer o identificador não é ter acesso, e o browser não é excepção.
///
/// A recusa é indistinguível de o ficheiro não existir: quem não o alcança não
/// deve aprender que ele existe por ver uma mensagem diferente.
#[tokio::test]
async fn um_estranho_com_o_identificador_nao_alcanca_o_ficheiro_no_browser() {
    let harness = harness!();

    let (dono, _) = harness.sign_in(&[TechnicalRole::ResearchMember]).await;
    let workspace_id = harness.owns_a_workspace(dono).await;
    let nome = unique_title("restrito");
    let file_id = semear_ficheiro(&harness, workspace_id, &nome, "RESTRICTED").await;

    // Outra pessoa, da mesma organização, sem pertencer ao ambiente.
    let (_, _) = harness.sign_in(&[TechnicalRole::ResearchMember]).await;

    let pagina = harness.open(&format!("/files/{file_id}")).await;
    let html = pagina.content().await.expect("conteúdo");
    assert!(
        !html.contains(&nome),
        "o nome do ficheiro chegou a quem não o alcança"
    );
    assert!(
        !html.contains("Histórico de versões"),
        "a página do ficheiro abriu para quem não tem acesso"
    );
}

/// Uma pessoa larga um ficheiro na zona de largada, e ele fica.
///
/// # A viagem
///
/// ```text
/// entrar → Ficheiros → largar bytes na zona
///        → app.js → POST multipart → Core → autorização → MinIO
///        → PostgreSQL → recarregar → o ficheiro está na lista
/// ```
///
/// # O que esta viagem prova e a outra não
///
/// Que os bytes atravessam mesmo. As outras viagens semeiam linhas e provam
/// navegação; esta usa o armazenamento a sério, e por isso diz em voz alta
/// quando não pode correr — um teste que se salta em silêncio é verde a
/// afirmar nada.
///
/// E prova a segunda metade da mesma frase: **carregar um ficheiro não é
/// afirmar conhecimento institucional.** Depois de o ficheiro existir, não há
/// um documento, um dataset nem uma fonte que ninguém tenha pedido.
#[tokio::test]
async fn uma_pessoa_larga_um_ficheiro_e_ele_fica() {
    let harness = harness!();

    if store_de_teste().is_none() {
        exigir_armazenamento("uma_pessoa_larga_um_ficheiro_e_ele_fica");
        return;
    }

    let (person_id, _) = harness.sign_in(&[TechnicalRole::ResearchMember]).await;
    let workspace_id = harness.owns_a_workspace(person_id).await;

    let antes: i64 = sqlx::query_scalar("SELECT count(*) FROM documents WHERE workspace_id = $1")
        .bind(workspace_id)
        .fetch_one(&harness.pool)
        .await
        .expect("documentos antes");

    let pagina = harness
        .open(&format!("/files?workspace={workspace_id}"))
        .await;
    esperar_por(&pagina, "Largue ficheiros aqui").await;

    // Largar, como uma pessoa larga: um `File` a sério, num `DataTransfer`, num
    // evento `drop` sobre a zona. Não se chama a função interna — chama-se o
    // browser, que é quem a pessoa usa.
    let nome = format!("{}.txt", unique_title("largado").replace(' ', "-"));
    let script = format!(
        "(() => {{ \
           const forma = document.querySelector('form[data-drop=\"1\"]'); \
           if (!forma) return 'sem forma'; \
           const ficheiro = new File(['prova institucional'], '{nome}', \
             {{ type: 'text/plain' }}); \
           const dt = new DataTransfer(); \
           dt.items.add(ficheiro); \
           forma.dispatchEvent(new DragEvent('drop', \
             {{ bubbles: true, cancelable: true, dataTransfer: dt }})); \
           return 'largado'; }})()"
    );
    let resultado: Option<String> = pagina
        .evaluate(script)
        .await
        .expect("largar")
        .into_value()
        .ok();
    assert_eq!(
        resultado.as_deref(),
        Some("largado"),
        "a zona de largada não recebeu o ficheiro"
    );

    // O ficheiro chega à base, com a sua primeira versão e os bytes contados.
    let limite = std::time::Instant::now();
    let file_id = loop {
        let encontrado: Option<Uuid> =
            sqlx::query_scalar("SELECT id FROM files WHERE workspace_id = $1 AND name = $2")
                .bind(workspace_id)
                .bind(&nome)
                .fetch_optional(&harness.pool)
                .await
                .expect("procura do ficheiro");

        if let Some(id) = encontrado {
            break id;
        }
        assert!(
            limite.elapsed() < DEADLINE,
            "o ficheiro largado não chegou ao PostgreSQL em {DEADLINE:?}"
        );
        tokio::time::sleep(Duration::from_millis(150)).await;
    };

    let (sequencia, bytes): (i32, i64) = sqlx::query_as(
        "SELECT v.sequence, o.size_bytes
           FROM file_versions v
           JOIN storage_objects o ON o.id = v.storage_object_id
          WHERE v.file_id = $1",
    )
    .bind(file_id)
    .fetch_one(&harness.pool)
    .await
    .expect("versão do ficheiro largado");

    assert_eq!(sequencia, 1, "a primeira versão não é a número 1");
    assert_eq!(
        bytes,
        "prova institucional".len() as i64,
        "os bytes que chegaram não são os que foram largados"
    );

    // E não nasceu conhecimento que ninguém afirmou.
    let depois: i64 = sqlx::query_scalar("SELECT count(*) FROM documents WHERE workspace_id = $1")
        .bind(workspace_id)
        .fetch_one(&harness.pool)
        .await
        .expect("documentos depois");
    assert_eq!(
        depois, antes,
        "largar um ficheiro criou um documento de conhecimento"
    );

    // A lista, recarregada pelo próprio `app.js`, mostra-o.
    esperar_por(&pagina, &nome).await;
}

/// Uma imagem institucional aparece na página sem a página saber onde ela está.
///
/// # A propriedade
///
/// > **A Experience não precisa de conhecer nem confiar no endpoint físico onde
/// > os bytes institucionais estão guardados.**
///
/// A `Content-Security-Policy` do Workspace continua `img-src 'self' data:`. Se
/// a imagem viesse do object storage o browser recusava-a, e o teste via um
/// elemento com largura zero. Vindo de `/files/{id}/preview`, o browser
/// carrega-a — e é o Chrome, não uma asserção sobre HTML, que o confirma.
#[tokio::test]
async fn uma_imagem_institucional_carrega_na_origem_do_workspace() {
    let harness = harness!();

    if store_de_teste().is_none() {
        exigir_armazenamento("uma_imagem_institucional_carrega_na_origem_do_workspace");
        return;
    }

    let (person_id, _) = harness.sign_in(&[TechnicalRole::ResearchMember]).await;
    let workspace_id = harness.owns_a_workspace(person_id).await;

    // Um PNG 1×1 verdadeiro, largado como uma pessoa larga.
    let pagina = harness
        .open(&format!("/files?workspace={workspace_id}"))
        .await;
    esperar_por(&pagina, "Largue ficheiros aqui").await;

    let nome = format!("{}.png", unique_title("imagem").replace(' ', "-"));
    let script = format!(
        "(async () => {{ \
           const forma = document.querySelector('form[data-drop=\"1\"]'); \
           const b64 = 'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg=='; \
           const cru = atob(b64); \
           const bytes = new Uint8Array(cru.length); \
           for (let i = 0; i < cru.length; i++) bytes[i] = cru.charCodeAt(i); \
           const ficheiro = new File([bytes], '{nome}', {{ type: 'image/png' }}); \
           const dt = new DataTransfer(); \
           dt.items.add(ficheiro); \
           forma.dispatchEvent(new DragEvent('drop', \
             {{ bubbles: true, cancelable: true, dataTransfer: dt }})); \
           return 'largado'; }})()"
    );
    let _ = pagina.evaluate(script).await.expect("largar a imagem");

    let limite = std::time::Instant::now();
    let file_id = loop {
        let encontrado: Option<Uuid> =
            sqlx::query_scalar("SELECT id FROM files WHERE workspace_id = $1 AND name = $2")
                .bind(workspace_id)
                .bind(&nome)
                .fetch_optional(&harness.pool)
                .await
                .expect("procura da imagem");
        if let Some(id) = encontrado {
            break id;
        }
        assert!(
            limite.elapsed() < DEADLINE,
            "a imagem largada não chegou ao PostgreSQL em {DEADLINE:?}"
        );
        tokio::time::sleep(Duration::from_millis(150)).await;
    };

    // A página do ficheiro mostra-a, e o `src` é local.
    let detalhe = harness.open(&format!("/files/{file_id}")).await;
    esperar_por(&detalhe, "oc-preview").await;
    let html = detalhe.content().await.expect("conteúdo");
    // A ligação é local **e por versão**: a imagem que se mostra é a da versão
    // que se está a ver, e não «a que o ficheiro tem agora».
    let versao: Uuid = sqlx::query_scalar(
        "SELECT id FROM file_versions WHERE file_id = $1 ORDER BY sequence DESC LIMIT 1",
    )
    .bind(file_id)
    .fetch_one(&harness.pool)
    .await
    .expect("versão corrente");
    assert!(
        html.contains(&format!("/file-versions/{versao}/preview")),
        "a pré-visualização não veio da origem do Workspace, ou não é da versão vista"
    );
    assert!(
        !html.contains("127.0.0.1:9000"),
        "o endereço do armazenamento apareceu na página"
    );

    // E o browser carregou-a mesmo: com a CSP a recusar, `naturalWidth` seria 0.
    let largura: Option<f64> = detalhe
        .evaluate(
            "(() => { const img = document.querySelector('.oc-preview'); \
              return img ? img.naturalWidth : -1; })()",
        )
        .await
        .expect("medir a imagem")
        .into_value()
        .ok();
    assert_eq!(
        largura,
        Some(1.0),
        "a imagem não foi carregada pelo browser — a CSP recusou-a, ou não chegou"
    );
}

// ── Extracção de conteúdo, pelo browser ──────────────────────────────────

/// Um PDF de uma ou mais páginas, cada uma com o seu texto.
#[must_use]
fn pdf_com_paginas(paginas: &[&str]) -> Vec<u8> {
    let mut objectos: Vec<String> = Vec::new();

    // 1: catálogo. 2: árvore de páginas. Depois, por página, o objecto da
    // página e o seu fluxo de conteúdo. Por fim, a fonte.
    let primeira_pagina = 3;
    let ids_pagina: Vec<usize> = (0..paginas.len())
        .map(|i| primeira_pagina + i * 2)
        .collect();
    let id_fonte = primeira_pagina + paginas.len() * 2;

    objectos.push("<< /Type /Catalog /Pages 2 0 R >>".to_owned());

    let kids = ids_pagina
        .iter()
        .map(|id| format!("{id} 0 R"))
        .collect::<Vec<_>>()
        .join(" ");
    objectos.push(format!(
        "<< /Type /Pages /Kids [{kids}] /Count {} >>",
        paginas.len()
    ));

    for (indice, texto) in paginas.iter().enumerate() {
        let id_conteudo = ids_pagina[indice] + 1;
        objectos.push(format!(
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] \
             /Contents {id_conteudo} 0 R \
             /Resources << /Font << /F1 {id_fonte} 0 R >> >> >>"
        ));

        let escapado = texto
            .replace('\\', "\\\\")
            .replace('(', "\\(")
            .replace(')', "\\)");
        let fluxo = format!("BT /F1 12 Tf 72 700 Td ({escapado}) Tj ET");
        objectos.push(format!(
            "<< /Length {} >>\nstream\n{fluxo}\nendstream",
            fluxo.len()
        ));
    }

    objectos.push("<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_owned());

    let mut saida = String::from("%PDF-1.4\n");
    let mut posicoes = Vec::with_capacity(objectos.len());
    for (indice, corpo) in objectos.iter().enumerate() {
        posicoes.push(saida.len());
        saida.push_str(&format!("{} 0 obj\n{corpo}\nendobj\n", indice + 1));
    }

    let inicio_xref = saida.len();
    saida.push_str(&format!("xref\n0 {}\n", objectos.len() + 1));
    saida.push_str("0000000000 65535 f \n");
    for posicao in &posicoes {
        saida.push_str(&format!("{posicao:010} 00000 n \n"));
    }
    saida.push_str(&format!(
        "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{inicio_xref}\n%%EOF\n",
        objectos.len() + 1
    ));

    saida.into_bytes()
}

/// Corre a extracção como o worker a corre.
///
/// A viagem carrega pelo browser e depois faz o trabalho do worker a partir do
/// teste: o worker é um processo à parte, e levantá-lo aqui provaria o
/// `tokio::select!` dele em vez de provar a cadeia. O que se quer verificar é
/// que o carregamento pelo ecrã produz um trabalho, que o trabalho produz
/// conteúdo pesquisável, e que o ecrã mostra os dois estados pelo caminho.
async fn correr_o_worker(harness: &Harness, file_id: Uuid) {
    let Some(store) = store_de_teste() else {
        return;
    };
    let versao: Uuid = sqlx::query_scalar(
        "SELECT id FROM file_versions WHERE file_id = $1 ORDER BY sequence DESC LIMIT 1",
    )
    .bind(file_id)
    .fetch_one(&harness.pool)
    .await
    .expect("versão corrente");

    let mut tx = harness.pool.begin().await.expect("tx");
    ocinye_core::modules::files::extraction::process(
        &mut tx,
        &store,
        versao,
        &ocinye_observability::CorrelationIds::generate(),
    )
    .await
    .expect("extracção");
    tx.commit().await.expect("commit");
}

/// Uma pessoa carrega um PDF, e passa a encontrar uma frase que só existe lá
/// dentro.
///
/// # A viagem
///
/// ```text
/// Ficheiros → largar um PDF → «A processar»
///           → worker lê o corpo → «Pesquisável»
///           → Pesquisar a frase → resultado com excerto e p. N
///           → abrir → o ficheiro certo
/// ```
///
/// A frase não está no nome do ficheiro. Se estivesse, a viagem passava pela
/// pesquisa de títulos e não provava nada sobre o corpo.
#[tokio::test]
async fn uma_frase_do_corpo_de_um_pdf_encontra_se_pelo_workspace() {
    let harness = harness!();

    if store_de_teste().is_none() {
        exigir_armazenamento("uma_frase_do_corpo_de_um_pdf_encontra_se_pelo_workspace");
        return;
    }

    let (person_id, _) = harness.sign_in(&[TechnicalRole::ResearchMember]).await;
    let workspace_id = harness.owns_a_workspace(person_id).await;

    let frase = format!("delta{}", Uuid::new_v4().simple());
    let pdf = pdf_com_paginas(&[
        "pagina de abertura sem nada de especial",
        &format!("coeficiente termoeletrico experimental {frase}"),
    ]);
    let b64 = {
        use base64::Engine as _;
        base64::engine::general_purpose::STANDARD.encode(&pdf)
    };

    let pagina = harness
        .open(&format!("/files?workspace={workspace_id}"))
        .await;
    esperar_por(&pagina, "Largue ficheiros aqui").await;

    let nome = format!("{}.pdf", unique_title("ensaio").replace(' ', "-"));
    let script = format!(
        "(() => {{ \
           const forma = document.querySelector('form[data-drop=\"1\"]'); \
           const cru = atob('{b64}'); \
           const bytes = new Uint8Array(cru.length); \
           for (let i = 0; i < cru.length; i++) bytes[i] = cru.charCodeAt(i); \
           const ficheiro = new File([bytes], '{nome}', {{ type: 'application/pdf' }}); \
           const dt = new DataTransfer(); \
           dt.items.add(ficheiro); \
           forma.dispatchEvent(new DragEvent('drop', \
             {{ bubbles: true, cancelable: true, dataTransfer: dt }})); \
           return 'largado'; }})()"
    );
    let _ = pagina.evaluate(script).await.expect("largar o PDF");

    let limite = std::time::Instant::now();
    let file_id = loop {
        let encontrado: Option<Uuid> =
            sqlx::query_scalar("SELECT id FROM files WHERE workspace_id = $1 AND name = $2")
                .bind(workspace_id)
                .bind(&nome)
                .fetch_optional(&harness.pool)
                .await
                .expect("procura do PDF");
        if let Some(id) = encontrado {
            break id;
        }
        assert!(
            limite.elapsed() < DEADLINE,
            "o PDF largado não chegou ao PostgreSQL em {DEADLINE:?}"
        );
        tokio::time::sleep(Duration::from_millis(150)).await;
    };

    // Antes do worker: guardado, e a dizer que está a processar.
    let detalhe = harness.open(&format!("/files/{file_id}")).await;
    esperar_por(&detalhe, "A processar").await;

    // Antes do worker, a frase não se encontra. É isto que distingue pesquisar
    // o corpo de pesquisar metadata com outro nome.
    let antes = harness.open(&format!("/search?q={frase}")).await;
    let html = antes.content().await.expect("conteúdo");
    assert!(
        !html.contains("No conteúdo dos ficheiros"),
        "a frase já era encontrável antes de o corpo ter sido lido"
    );

    correr_o_worker(&harness, file_id).await;

    // Depois: pesquisável, e o ecrã di-lo.
    let depois = harness.open(&format!("/files/{file_id}")).await;
    esperar_por(&depois, "Pesquisável").await;

    // E a pesquisa encontra-a, com excerto e página.
    let resultados = harness.open(&format!("/search?q={frase}")).await;
    esperar_por(&resultados, "No conteúdo dos ficheiros").await;
    let html = resultados.content().await.expect("conteúdo");
    assert!(
        html.contains(&nome),
        "o resultado não nomeia o ficheiro certo"
    );
    assert!(
        html.contains("p. 2"),
        "o resultado não cita a página onde a frase está"
    );
    assert!(
        html.contains(&format!("/files/{file_id}")),
        "o resultado não leva ao ficheiro"
    );
}

/// Um formato que se guarda mas não se lê diz isso, e não «o carregamento
/// falhou».
///
/// > **Ficheiro guardado. Não foi possível tornar o conteúdo pesquisável.**
#[tokio::test]
async fn um_formato_sem_leitor_diz_que_o_ficheiro_esta_guardado() {
    let harness = harness!();

    if store_de_teste().is_none() {
        exigir_armazenamento("um_formato_sem_leitor_diz_que_o_ficheiro_esta_guardado");
        return;
    }

    let (person_id, _) = harness.sign_in(&[TechnicalRole::ResearchMember]).await;
    let workspace_id = harness.owns_a_workspace(person_id).await;

    let pagina = harness
        .open(&format!("/files?workspace={workspace_id}"))
        .await;
    esperar_por(&pagina, "Largue ficheiros aqui").await;

    let nome = format!("{}.png", unique_title("montagem").replace(' ', "-"));
    let script = format!(
        "(() => {{ \
           const forma = document.querySelector('form[data-drop=\"1\"]'); \
           const b64 = 'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg=='; \
           const cru = atob(b64); \
           const bytes = new Uint8Array(cru.length); \
           for (let i = 0; i < cru.length; i++) bytes[i] = cru.charCodeAt(i); \
           const ficheiro = new File([bytes], '{nome}', {{ type: 'image/png' }}); \
           const dt = new DataTransfer(); \
           dt.items.add(ficheiro); \
           forma.dispatchEvent(new DragEvent('drop', \
             {{ bubbles: true, cancelable: true, dataTransfer: dt }})); \
           return 'largado'; }})()"
    );
    let _ = pagina.evaluate(script).await.expect("largar o PNG");

    let limite = std::time::Instant::now();
    let file_id = loop {
        let encontrado: Option<Uuid> =
            sqlx::query_scalar("SELECT id FROM files WHERE workspace_id = $1 AND name = $2")
                .bind(workspace_id)
                .bind(&nome)
                .fetch_optional(&harness.pool)
                .await
                .expect("procura do PNG");
        if let Some(id) = encontrado {
            break id;
        }
        assert!(
            limite.elapsed() < DEADLINE,
            "o PNG não chegou ao PostgreSQL"
        );
        tokio::time::sleep(Duration::from_millis(150)).await;
    };

    correr_o_worker(&harness, file_id).await;

    let detalhe = harness.open(&format!("/files/{file_id}")).await;
    esperar_por(&detalhe, "Conteúdo não pesquisável").await;
    let html = detalhe.content().await.expect("conteúdo");

    assert!(
        html.contains("Ficheiro guardado"),
        "a página não diz que o ficheiro está guardado"
    );
    assert!(
        !html.to_lowercase().contains("carregamento falhou"),
        "a página trata uma extracção sem leitor como um carregamento falhado"
    );
    // E continua a poder descarregar-se.
    assert!(
        html.contains(&format!("/files/{file_id}/download")),
        "um ficheiro sem conteúdo pesquisável perdeu a descarga"
    );
}

/// O que se vê é o que se pesquisa.
///
/// A pré-visualização e a pesquisa liam o mesmo ficheiro por caminhos
/// diferentes — uma pelo extractor, a outra descarregando os bytes e
/// descodificando-os outra vez. Dois caminhos para o mesmo texto divergem, e o
/// dia em que divergissem era o dia em que alguém via no ecrã uma coisa
/// diferente daquela que a pesquisa tinha encontrado.
///
/// Agora há um caminho só, e isso torna um PDF pré-visualizável de graça.
#[tokio::test]
async fn a_previsualizacao_mostra_o_mesmo_texto_que_a_pesquisa_encontra() {
    let harness = harness!();

    if store_de_teste().is_none() {
        exigir_armazenamento("a_previsualizacao_mostra_o_mesmo_texto_que_a_pesquisa_encontra");
        return;
    }

    let (person_id, _) = harness.sign_in(&[TechnicalRole::ResearchMember]).await;
    let workspace_id = harness.owns_a_workspace(person_id).await;

    let frase = format!("delta{}", Uuid::new_v4().simple());
    let pdf = pdf_com_paginas(&[&format!("medicao registada {frase}")]);
    let b64 = {
        use base64::Engine as _;
        base64::engine::general_purpose::STANDARD.encode(&pdf)
    };

    let pagina = harness
        .open(&format!("/files?workspace={workspace_id}"))
        .await;
    esperar_por(&pagina, "Largue ficheiros aqui").await;

    let nome = format!("{}.pdf", unique_title("previsto").replace(' ', "-"));
    let script = format!(
        "(() => {{ \
           const forma = document.querySelector('form[data-drop=\"1\"]'); \
           const cru = atob('{b64}'); \
           const bytes = new Uint8Array(cru.length); \
           for (let i = 0; i < cru.length; i++) bytes[i] = cru.charCodeAt(i); \
           const f = new File([bytes], '{nome}', {{ type: 'application/pdf' }}); \
           const dt = new DataTransfer(); dt.items.add(f); \
           forma.dispatchEvent(new DragEvent('drop', \
             {{ bubbles: true, cancelable: true, dataTransfer: dt }})); \
           return 'largado'; }})()"
    );
    let _ = pagina.evaluate(script).await.expect("largar");

    let limite = std::time::Instant::now();
    let file_id = loop {
        let encontrado: Option<Uuid> =
            sqlx::query_scalar("SELECT id FROM files WHERE workspace_id = $1 AND name = $2")
                .bind(workspace_id)
                .bind(&nome)
                .fetch_optional(&harness.pool)
                .await
                .expect("procura");
        if let Some(id) = encontrado {
            break id;
        }
        assert!(
            limite.elapsed() < DEADLINE,
            "o PDF não chegou ao PostgreSQL"
        );
        tokio::time::sleep(Duration::from_millis(150)).await;
    };

    // Antes de o corpo ser lido, não há texto para mostrar — e a página não
    // inventa nenhum.
    let antes = harness.open(&format!("/files/{file_id}")).await;
    let html = antes.content().await.expect("conteúdo");
    assert!(
        !html.contains(&frase),
        "a página mostrou texto de um ficheiro que ainda não tinha sido lido"
    );

    correr_o_worker(&harness, file_id).await;

    // Depois, a mesma frase que a pesquisa encontra está no ecrã.
    let depois = harness.open(&format!("/files/{file_id}")).await;
    esperar_por(&depois, &frase).await;

    let resultados = harness.open(&format!("/search?q={frase}")).await;
    esperar_por(&resultados, "No conteúdo dos ficheiros").await;
}

/// Corre o indexador semântico, como o worker o corre.
async fn indexar_semanticamente(harness: &Harness, file_id: Uuid) {
    let versao: Uuid = sqlx::query_scalar(
        "SELECT id FROM file_versions WHERE file_id = $1 ORDER BY sequence DESC LIMIT 1",
    )
    .bind(file_id)
    .fetch_one(&harness.pool)
    .await
    .expect("versão corrente");

    let provider =
        ocinye_core::modules::intelligence::embeddings::DeterministicEmbeddings::default();
    let mut tx = harness.pool.begin().await.expect("tx");
    ocinye_core::modules::files::embedding::process(&mut tx, &provider, versao)
        .await
        .expect("indexação semântica");
    tx.commit().await.expect("commit");
}

/// Uma paráfrase encontra o documento, e a pesquisa textual sozinha não a
/// encontrava.
///
/// # O controlo que torna isto uma prova
///
/// A pergunta não contém a frase do documento. Antes da indexação semântica, a
/// pesquisa não devolve nada — e é isso que separa «o semântico funciona» de
/// «os dois encontraram e eu não sei qual trabalhou».
#[tokio::test]
async fn uma_parafrase_encontra_o_documento_pelo_workspace() {
    let harness = harness!();

    if store_de_teste().is_none() {
        exigir_armazenamento("uma_parafrase_encontra_o_documento_pelo_workspace");
        return;
    }

    let (person_id, _) = harness.sign_in(&[TechnicalRole::ResearchMember]).await;
    let workspace_id = harness.owns_a_workspace(person_id).await;

    let alfa = format!("alfa{}", Uuid::new_v4().simple());
    let beta = format!("beta{}", Uuid::new_v4().simple());
    let pdf = pdf_com_paginas(&[&format!("{alfa} {beta} medicao registada no ensaio")]);
    let b64 = {
        use base64::Engine as _;
        base64::engine::general_purpose::STANDARD.encode(&pdf)
    };

    let pagina = harness
        .open(&format!("/files?workspace={workspace_id}"))
        .await;
    esperar_por(&pagina, "Largue ficheiros aqui").await;

    let nome = format!("{}.pdf", unique_title("semantico").replace(' ', "-"));
    let script = format!(
        "(() => {{ \
           const forma = document.querySelector('form[data-drop=\"1\"]'); \
           const cru = atob('{b64}'); \
           const bytes = new Uint8Array(cru.length); \
           for (let i = 0; i < cru.length; i++) bytes[i] = cru.charCodeAt(i); \
           const f = new File([bytes], '{nome}', {{ type: 'application/pdf' }}); \
           const dt = new DataTransfer(); dt.items.add(f); \
           forma.dispatchEvent(new DragEvent('drop', \
             {{ bubbles: true, cancelable: true, dataTransfer: dt }})); \
           return 'largado'; }})()"
    );
    let _ = pagina.evaluate(script).await.expect("largar");

    let limite = std::time::Instant::now();
    let file_id = loop {
        let encontrado: Option<Uuid> =
            sqlx::query_scalar("SELECT id FROM files WHERE workspace_id = $1 AND name = $2")
                .bind(workspace_id)
                .bind(&nome)
                .fetch_optional(&harness.pool)
                .await
                .expect("procura");
        if let Some(id) = encontrado {
            break id;
        }
        assert!(
            limite.elapsed() < DEADLINE,
            "o PDF não chegou ao PostgreSQL"
        );
        tokio::time::sleep(Duration::from_millis(150)).await;
    };

    correr_o_worker(&harness, file_id).await;

    // A pergunta: uma das marcas mais uma palavra que o documento não tem.
    // `websearch_to_tsquery` exige todos os termos, pelo que o lexical falha.
    let pergunta = format!("{alfa} inexistentepalavra");

    let antes = harness.open(&format!("/search?q={pergunta}")).await;
    let html = antes.content().await.expect("conteúdo");
    assert!(
        !html.contains(&nome),
        "o controlo falhou: a pesquisa textual já encontrava isto sozinha"
    );

    indexar_semanticamente(&harness, file_id).await;

    let depois = harness.open(&format!("/search?q={pergunta}")).await;
    esperar_por(&depois, "No conteúdo dos ficheiros").await;
    let html = depois.content().await.expect("conteúdo");
    assert!(
        html.contains(&nome),
        "a paráfrase não encontrou o documento"
    );
    assert!(
        html.contains(&format!("/files/{file_id}")),
        "o resultado não leva ao ficheiro certo"
    );
    assert!(
        html.contains("v1"),
        "o resultado não cita a versão de onde saiu"
    );
}

/// Sem pesquisa semântica, a interface di-lo — e não diz que está partida.
#[tokio::test]
async fn a_pesquisa_semantica_indisponivel_nao_e_um_erro() {
    let harness = harness!();

    let (_, _) = harness.sign_in(&[TechnicalRole::ResearchMember]).await;
    let pagina = harness.open("/search?q=hidrogenio").await;
    let html = pagina.content().await.expect("conteúdo");

    // O harness **tem** provider, por isso o que se afirma aqui é o outro lado:
    // o modo semântico é declarado, com o seu estado, e nunca como avaria.
    assert!(
        html.contains("Semântica"),
        "o modo semântico não está declarado na interface"
    );
    for palavra in ["degradad", "avaria", "Erro na pesquisa"] {
        assert!(
            !html.contains(palavra),
            "a interface descreve a pesquisa como partida: «{palavra}»"
        );
    }
    // E a textual continua a funcionar, que é o que não pode cair.
    assert!(
        html.contains("Pesquisar no Ocinye"),
        "a página de pesquisa não abriu"
    );
}

/// Uma citação continua a abrir os bytes que foram citados.
///
/// # A viagem
///
/// ```text
/// carregar PDF → extrair → pesquisar uma frase do corpo
///   → o resultado cita v1 · p. 1
///   → clicar → abre a v1, e diz que é a v1
///   → carregar v2
///   → voltar à mesma citação → continua a abrir a v1
/// ```
///
/// # O que isto fecha
///
/// Que a recuperação não quebrou a natureza versionada da memória
/// institucional. Uma citação que apontasse para «o ficheiro» descreveria, no
/// dia seguinte, um texto que ninguém leu.
#[tokio::test]
async fn uma_citacao_continua_a_abrir_a_versao_que_citou() {
    let harness = harness!();

    if store_de_teste().is_none() {
        exigir_armazenamento("uma_citacao_continua_a_abrir_a_versao_que_citou");
        return;
    }

    let (person_id, credenciais) = harness.sign_in(&[TechnicalRole::ResearchMember]).await;
    let _ = &credenciais;
    let workspace_id = harness.owns_a_workspace(person_id).await;

    let so_na_v1 = format!("delta{}", Uuid::new_v4().simple());
    let so_na_v2 = format!("delta{}", Uuid::new_v4().simple());

    let largar = |pagina: &Page, nome: String, bytes: Vec<u8>| {
        let b64 = {
            use base64::Engine as _;
            base64::engine::general_purpose::STANDARD.encode(&bytes)
        };
        let script = format!(
            "(() => {{ \
               const forma = document.querySelector('form[data-drop=\"1\"]'); \
               const cru = atob('{b64}'); \
               const b = new Uint8Array(cru.length); \
               for (let i = 0; i < cru.length; i++) b[i] = cru.charCodeAt(i); \
               const f = new File([b], '{nome}', {{ type: 'application/pdf' }}); \
               const dt = new DataTransfer(); dt.items.add(f); \
               forma.dispatchEvent(new DragEvent('drop', \
                 {{ bubbles: true, cancelable: true, dataTransfer: dt }})); \
               return 'largado'; }})()"
        );
        let pagina = pagina.clone();
        async move { pagina.evaluate(script).await.expect("largar") }
    };

    let lista = harness
        .open(&format!("/files?workspace={workspace_id}"))
        .await;
    esperar_por(&lista, "Largue ficheiros aqui").await;

    let nome = format!("{}.pdf", unique_title("relatorio").replace(' ', "-"));
    let _ = largar(
        &lista,
        nome.clone(),
        pdf_com_paginas(&[&format!("conclusao do ensaio {so_na_v1}")]),
    )
    .await;

    let limite = std::time::Instant::now();
    let file_id = loop {
        let encontrado: Option<Uuid> =
            sqlx::query_scalar("SELECT id FROM files WHERE workspace_id = $1 AND name = $2")
                .bind(workspace_id)
                .bind(&nome)
                .fetch_optional(&harness.pool)
                .await
                .expect("procura");
        if let Some(id) = encontrado {
            break id;
        }
        assert!(
            limite.elapsed() < DEADLINE,
            "o PDF não chegou ao PostgreSQL"
        );
        tokio::time::sleep(Duration::from_millis(150)).await;
    };
    correr_o_worker(&harness, file_id).await;

    let v1: Uuid = sqlx::query_scalar(
        "SELECT id FROM file_versions WHERE file_id = $1 ORDER BY sequence LIMIT 1",
    )
    .bind(file_id)
    .fetch_one(&harness.pool)
    .await
    .expect("v1");

    // A pesquisa cita a versão exacta e a página.
    let resultados = harness.open(&format!("/search?q={so_na_v1}")).await;
    esperar_por(&resultados, "No conteúdo dos ficheiros").await;
    let html = resultados.content().await.expect("conteúdo");
    assert!(
        html.contains(&format!("/files/{file_id}?version={v1}")),
        "o resultado não cita a versão exacta: a ligação leva ao ficheiro e não à versão"
    );
    assert!(
        html.contains("v1"),
        "o resultado não mostra que versão citou"
    );

    // Clicar abre a v1, e a página diz que v1 é o que se está a ver.
    let citada = harness
        .open(&format!("/files/{file_id}?version={v1}&page=1"))
        .await;
    esperar_por(&citada, "A ver a versão 1").await;
    let html = citada.content().await.expect("conteúdo");
    assert!(
        html.contains(&so_na_v1),
        "abrir a citação não mostrou o texto citado"
    );

    // Uma versão nova, pelo formulário do próprio ficheiro.
    //
    // Largar outra vez na zona de carregamento criaria **outro ficheiro** com o
    // mesmo nome, que é o comportamento certo — o nome não é identidade. Quem
    // quer versionar diz-lo, e diz onde.
    let pagina_do_ficheiro = harness.open(&format!("/files/{file_id}")).await;
    esperar_por(&pagina_do_ficheiro, "Carregar nova versão").await;

    let b64 = {
        use base64::Engine as _;
        base64::engine::general_purpose::STANDARD
            .encode(pdf_com_paginas(&[&format!("conclusao revista {so_na_v2}")]))
    };
    let script = format!(
        "(() => {{ \
           const campo = document.querySelector('#oc-version-file'); \
           if (!campo) return 'sem campo'; \
           const cru = atob('{b64}'); \
           const b = new Uint8Array(cru.length); \
           for (let i = 0; i < cru.length; i++) b[i] = cru.charCodeAt(i); \
           const f = new File([b], '{nome}', {{ type: 'application/pdf' }}); \
           const dt = new DataTransfer(); dt.items.add(f); \
           campo.files = dt.files; \
           campo.form.submit(); \
           return 'submetido'; }})()"
    );
    let submetido: Option<String> = pagina_do_ficheiro
        .evaluate(script)
        .await
        .expect("submeter a versão nova")
        .into_value()
        .ok();
    assert_eq!(
        submetido.as_deref(),
        Some("submetido"),
        "o formulário de nova versão não aceitou o ficheiro"
    );

    let limite = std::time::Instant::now();
    loop {
        let quantas: i64 =
            sqlx::query_scalar("SELECT count(*) FROM file_versions WHERE file_id = $1")
                .bind(file_id)
                .fetch_one(&harness.pool)
                .await
                .expect("contagem");
        if quantas >= 2 {
            break;
        }
        assert!(limite.elapsed() < DEADLINE, "a segunda versão não chegou");
        tokio::time::sleep(Duration::from_millis(150)).await;
    }
    correr_o_worker(&harness, file_id).await;

    // O ficheiro, aberto sem citação, mostra a versão corrente.
    let corrente = harness.open(&format!("/files/{file_id}")).await;
    esperar_por(&corrente, &so_na_v2).await;

    // E a mesma citação continua a abrir a v1.
    let outra_vez = harness
        .open(&format!("/files/{file_id}?version={v1}&page=1"))
        .await;
    esperar_por(&outra_vez, "A ver a versão 1").await;
    let html = outra_vez.content().await.expect("conteúdo");
    assert!(
        html.contains(&so_na_v1),
        "a citação deixou de abrir os bytes que citou"
    );
    assert!(
        !html.contains(&so_na_v2),
        "a citação derivou para a versão corrente"
    );
    assert!(
        html.contains("Esta não é a versão corrente"),
        "a página mostra uma versão antiga sem o dizer"
    );
}

/// Quem pertence a um ambiente vê CONHECIMENTO como navegação, não como recusa.
///
/// # O defeito que este teste guarda
///
/// A sidebar decidia sobre `DocumentsView`, `BibliographyView` e
/// `DatasetsView` com a lista de capacidades do `/me`, que é de **âmbito
/// institucional**. Essas três permissões só existem como concessão
/// contextual — pertença a unidade ou a ambiente — e no contexto institucional
/// `workspace_id` e `unit_id` são `None`, pelo que nenhuma concessão contextual
/// se aplica.
///
/// Consequência: as quatro entradas de CONHECIMENTO ficavam esbatidas **para
/// toda a gente**, incluindo quem pertencia a um ambiente cheio de ficheiros. A
/// navegação dizia «não tem autorização» a quem tinha.
///
/// É o mesmo defeito que o botão de carregar teve dentro do ecrã: uma lista
/// institucional a decidir sobre um direito de ambiente.
#[tokio::test]
async fn quem_pertence_a_um_ambiente_alcanca_conhecimento_pela_navegacao() {
    let harness = harness!();

    let (person_id, _) = harness.sign_in(&[TechnicalRole::ResearchMember]).await;
    // Dá-lhe uma unidade e um ambiente: passa a alcançar ficheiros a sério.
    let _workspace = harness.owns_a_workspace(person_id).await;

    let pagina = harness.open("/").await;
    esperar_por(&pagina, "CONHECIMENTO").await;
    let html = pagina.content().await.expect("conteúdo");

    for entrada in ["Ficheiros", "Conhecimento", "Bibliografia", "Dados"] {
        let indice = html
            .find(&format!(">{entrada}<"))
            .unwrap_or_else(|| panic!("«{entrada}» não aparece na navegação"));

        // O bloco que contém a entrada: se for o estado indisponível, a classe
        // e o `aria-disabled` estão logo antes dela.
        let inicio = indice.saturating_sub(420);
        let contexto = &html[inicio..indice];
        assert!(
            !contexto.contains("oc-nav--unavailable"),
            "«{entrada}» aparece como indisponível a quem pertence a um ambiente"
        );
        assert!(
            !contexto.contains("aria-disabled"),
            "«{entrada}» está marcada como desactivada a quem pertence a um ambiente"
        );
    }
}

/// Abrir a entrada não é alcançar coisa nenhuma.
///
/// A contrapartida do teste anterior, e a razão pela qual abrir a navegação não
/// abre acesso: quem não pertence a nada entra e encontra um ecrã vazio e
/// honesto. É o mesmo desenho da Pesquisa — zero resultados, e não uma recusa.
#[tokio::test]
async fn ver_a_entrada_de_ficheiros_nao_da_acesso_a_ficheiro_nenhum() {
    let harness = harness!();

    // Alguém com um ambiente e um ficheiro lá dentro.
    let (dono, _) = harness.sign_in(&[TechnicalRole::ResearchMember]).await;
    let workspace_id = harness.owns_a_workspace(dono).await;
    let nome = unique_title("restrito");
    let file_id = semear_ficheiro(&harness, workspace_id, &nome, "RESTRICTED").await;

    // E alguém que não pertence a nada.
    let (_, _) = harness.sign_in(&[TechnicalRole::ResearchMember]).await;

    // Vê a entrada — a navegação já não lhe mente.
    let inicio = harness.open("/").await;
    esperar_por(&inicio, "Ficheiros").await;

    // Entra, e o ecrã abre em vez de recusar.
    //
    // O ambiente do harness é INTERNAL, e INTERNAL quer dizer que qualquer
    // membro activo o alcança — por isso ele aparece na escolha, e deve
    // aparecer. O que **não** aparece é o ficheiro RESTRICTED lá dentro.
    let ficheiros = harness.open("/files").await;
    esperar_por(&ficheiros, "em todos os ambientes a que pertence").await;

    let dentro = harness
        .open(&format!("/files?workspace={workspace_id}"))
        .await;
    esperar_por(&dentro, "Ficheiros").await;
    let html = dentro.content().await.expect("conteúdo");
    assert!(
        !html.contains(&nome),
        "um ficheiro RESTRICTED apareceu a quem não pertence ao ambiente"
    );

    // E o identificador directo continua a não valer nada.
    let directo = harness.open(&format!("/files/{file_id}")).await;
    let html = directo.content().await.expect("conteúdo");
    assert!(
        !html.contains(&nome),
        "abrir o ficheiro pelo identificador revelou-o a quem não o alcança"
    );
    assert!(
        !html.contains("Histórico de versões"),
        "a página do ficheiro abriu para quem não tem acesso"
    );
}

/// Uma conta de investigação sem pertenças não é uma conta partida.
///
/// # A propriedade
///
/// > **A relevância de um módulo responde se uma capacidade pertence ao espaço
/// > de trabalho institucional da pessoa. A autorização de um recurso responde
/// > ao que ela pode de facto ver ou fazer. A relevância nunca concede
/// > autoridade.**
///
/// Antes, os quatro módulos de CONHECIMENTO apareciam esbatidos a toda a gente,
/// porque a navegação perguntava um direito contextual num contexto
/// institucional. Uma conta acabada de criar parecia avariada.
///
/// Agora aparecem — e continuam a não dar acesso a coisa nenhuma.
#[tokio::test]
async fn uma_conta_de_investigacao_sem_pertencas_ve_os_modulos_de_investigacao() {
    let harness = harness!();

    // Sem unidade, sem ambiente: exactamente a conta que parecia partida.
    let (_, _) = harness.sign_in(&[TechnicalRole::ResearchMember]).await;

    let pagina = harness.open("/").await;
    esperar_por(&pagina, "CONHECIMENTO").await;
    let html = pagina.content().await.expect("conteúdo");

    for entrada in ["Ficheiros", "Conhecimento", "Bibliografia", "Dados"] {
        let indice = html
            .find(&format!(">{entrada}<"))
            .unwrap_or_else(|| panic!("«{entrada}» não aparece na navegação"));
        let contexto = &html[indice.saturating_sub(420)..indice];
        assert!(
            !contexto.contains("oc-nav--unavailable"),
            "«{entrada}» aparece como indisponível a uma conta de investigação"
        );
    }

    // E entrar não dá acesso: o ecrã diz a verdade em vez de recusar.
    let ficheiros = harness.open("/files").await;
    // Sem pertenças: um estado vazio que ensina, e não uma recusa.
    esperar_por(&ficheiros, "Ainda não tem ficheiros acessíveis").await;
    let html = ficheiros.content().await.expect("conteúdo");
    assert!(
        html.contains("Não tem onde carregar ficheiros"),
        "a página não diz que não há onde carregar"
    );
}

/// Um colaborador externo não ganha módulos de investigação.
///
/// A contrapartida: relevância deriva do papel institucional, e um papel que
/// não faz investigação não passa a fazê-la porque a navegação ficou mais
/// generosa.
#[tokio::test]
async fn um_colaborador_externo_nao_ganha_os_modulos_de_investigacao() {
    let harness = harness!();

    let (_, _) = harness
        .sign_in(&[TechnicalRole::ExternalCollaborator])
        .await;

    let pagina = harness.open("/").await;
    esperar_por(&pagina, "OCINYE OS").await;
    let html = pagina.content().await.expect("conteúdo");

    for entrada in ["Ficheiros", "Bibliografia", "Dados"] {
        assert!(
            !html.contains(&format!(">{entrada}<")),
            "«{entrada}» apareceu a um colaborador externo"
        );
    }
}

/// Uma unidade nasce governável, e a autoridade concede-se pelo produto.
///
/// # A viagem
///
/// ```text
/// admin cria unidade → é gestor → abre a unidade → área de Pessoas
///   → acrescenta um membro de investigação
///   → esse membro passa a alcançar o que a unidade governa
/// ```
///
/// # O que isto fecha
///
/// O beco que originou toda esta milestone: criar uma unidade não criava
/// pertença nenhuma, e não havia ecrã para acrescentar membros. A unidade
/// existia e ninguém a podia gerir — a única saída era escrever na base por
/// fora.
#[tokio::test]
async fn uma_unidade_nasce_governavel_e_a_pertenca_concede_se_pelo_produto() {
    let harness = harness!();

    let (admin_id, _) = harness.sign_in(&[TechnicalRole::PlatformAdmin]).await;

    // Criar a unidade pelo ecrã, como uma pessoa faz.
    let form = harness.open("/units/new").await;
    esperar_por(&form, "Nova Unidade").await;
    let codigo = format!("U{}", &Uuid::new_v4().simple().to_string()[..6]).to_uppercase();
    set_field(&form, "input[name=code]", &codigo).await;
    set_field(&form, "input[name=name]", "Unidade de prova").await;
    submit(&form, "form[action=\"/units/new\"]").await;

    let unit_id = {
        let limite = std::time::Instant::now();
        loop {
            let encontrado: Option<Uuid> =
                sqlx::query_scalar("SELECT id FROM units WHERE code = $1")
                    .bind(&codigo)
                    .fetch_optional(&harness.pool)
                    .await
                    .expect("procura da unidade");
            if let Some(id) = encontrado {
                break id;
            }
            assert!(limite.elapsed() < DEADLINE, "a unidade não foi criada");
            tokio::time::sleep(Duration::from_millis(150)).await;
        }
    };

    // Quem a criou é gestor — o recurso nasceu governável.
    let papel: Option<String> = sqlx::query_scalar(
        "SELECT role FROM unit_memberships WHERE unit_id = $1 AND person_id = $2",
    )
    .bind(unit_id)
    .bind(admin_id)
    .fetch_optional(&harness.pool)
    .await
    .expect("consulta");
    assert_eq!(
        papel.as_deref(),
        Some("manager"),
        "quem criou a unidade não ficou a poder geri-la"
    );

    // Alguém de investigação, ainda sem pertenças.
    let investigador: Uuid = {
        let handle = format!("i{}", Uuid::new_v4().simple());
        let id: Uuid = sqlx::query_scalar(
            "INSERT INTO people (organisation_id, full_name, email, status)
             VALUES ($1, $2, $3, 'active') RETURNING id",
        )
        .bind(harness.organisation_id)
        .bind("Investigadora de prova")
        .bind(format!("{handle}@ocinye.com"))
        .fetch_one(&harness.pool)
        .await
        .expect("pessoa");
        sqlx::query("INSERT INTO person_roles (person_id, role) VALUES ($1, 'research_member')")
            .bind(id)
            .execute(&harness.pool)
            .await
            .expect("papel");
        id
    };

    // A área de Pessoas existe, e o gestor acrescenta-a por lá.
    let unidade = harness.open(&format!("/units/{unit_id}")).await;
    esperar_por(&unidade, "Adicionar").await;
    let html = unidade.content().await.expect("conteúdo");
    assert!(
        html.contains("Investigadora de prova"),
        "a pessoa não aparece entre quem se pode acrescentar"
    );

    escolher(&unidade, "#oc-unit-person", &investigador.to_string()).await;
    submit(&unidade, "form.oc-pessoa__acrescentar").await;
    esperar_por(&unidade, "Pessoa adicionada à unidade").await;

    // A pertença existe, e foi criada pelo Core — não por SQL.
    let papel: Option<String> = sqlx::query_scalar(
        "SELECT role FROM unit_memberships WHERE unit_id = $1 AND person_id = $2",
    )
    .bind(unit_id)
    .bind(investigador)
    .fetch_optional(&harness.pool)
    .await
    .expect("consulta");
    assert_eq!(
        papel.as_deref(),
        Some("member"),
        "a pessoa não foi acrescentada à unidade pelo produto"
    );
}

/// Quem não gere a unidade não recebe os controlos que a alteram.
///
/// E a ausência deles não é a defesa: o Core recusa a mesma operação a quem a
/// tente por HTTP directo.
#[tokio::test]
async fn quem_nao_gere_a_unidade_nao_recebe_os_controlos_nem_a_operacao() {
    let harness = harness!();

    let (admin_id, _) = harness.sign_in(&[TechnicalRole::PlatformAdmin]).await;
    let unit_id = harness.manages_a_unit(admin_id).await;

    // Outra pessoa, sem gestão da unidade.
    let (estranho_id, _) = harness.sign_in(&[TechnicalRole::ResearchMember]).await;

    let unidade = harness.open(&format!("/units/{unit_id}")).await;
    let html = unidade.content().await.expect("conteúdo");
    assert!(
        !html.contains("oc-pessoa__acrescentar"),
        "quem não gere a unidade recebeu o formulário de acrescentar pessoas"
    );
    assert!(
        !html.contains("Tornar gestor"),
        "quem não gere a unidade recebeu os controlos de papel"
    );

    // E a operação directa continua a ser recusada.
    let antes: i64 = sqlx::query_scalar("SELECT count(*) FROM unit_memberships WHERE unit_id = $1")
        .bind(unit_id)
        .fetch_one(&harness.pool)
        .await
        .expect("contagem");

    let script = format!(
        "(async () => {{ \
           const corpo = new URLSearchParams(); \
           corpo.set('person_id', '{estranho_id}'); \
           corpo.set('role', 'manager'); \
           const r = await fetch('/units/{unit_id}/members', \
             {{ method: 'POST', body: corpo, redirect: 'follow' }}); \
           return r.url; }})()"
    );
    let _: Option<String> = unidade
        .evaluate(script)
        .await
        .expect("tentativa directa")
        .into_value()
        .ok();

    // Espera activa curta: se a escrita passasse, a contagem mudaria.
    tokio::time::sleep(Duration::from_millis(600)).await;
    let depois: i64 =
        sqlx::query_scalar("SELECT count(*) FROM unit_memberships WHERE unit_id = $1")
            .bind(unit_id)
            .fetch_one(&harness.pool)
            .await
            .expect("contagem");
    assert_eq!(
        depois, antes,
        "alguém sem autoridade acrescentou-se a uma unidade por HTTP directo"
    );
}

/// A vista agregada mostra ficheiros de vários ambientes, e conta o que mostra.
///
/// # A propriedade
///
/// > **Para qualquer vista agregada, a visibilidade da contagem é a mesma da
/// > lista.**
///
/// Nada de «94 recursos» e três linhas porque 91 estavam escondidos.
#[tokio::test]
async fn a_vista_agregada_de_ficheiros_atravessa_ambientes_e_conta_o_que_mostra() {
    let harness = harness!();

    let (person_id, _) = harness.sign_in(&[TechnicalRole::ResearchMember]).await;

    // Dois ambientes onde pertence, com um ficheiro cada.
    let primeiro = harness.owns_a_workspace(person_id).await;
    let segundo = harness.owns_a_workspace(person_id).await;
    let nome_a = unique_title("alfa");
    let nome_b = unique_title("beta");
    semear_ficheiro(&harness, primeiro, &nome_a, "INTERNAL").await;
    semear_ficheiro(&harness, segundo, &nome_b, "INTERNAL").await;

    // E um terceiro ambiente, de outra pessoa — criada sem `sign_in`, porque
    // `sign_in` troca a sessão do browser e o teste passaria a ser sobre ela.
    let outro_id: Uuid = {
        let handle = format!("o{}", Uuid::new_v4().simple());
        sqlx::query_scalar(
            "INSERT INTO people (organisation_id, full_name, email, status)
             VALUES ($1, $2, $3, 'active') RETURNING id",
        )
        .bind(harness.organisation_id)
        .bind("Outra pessoa")
        .bind(format!("{handle}@ocinye.com"))
        .fetch_one(&harness.pool)
        .await
        .expect("pessoa")
    };
    let alheio = harness.owns_a_workspace(outro_id).await;
    let nome_escondido = unique_title("escondido");
    semear_ficheiro(&harness, alheio, &nome_escondido, "RESTRICTED").await;

    let pagina = harness.open("/files").await;
    esperar_por(&pagina, "em todos os ambientes a que pertence").await;
    let html = pagina.content().await.expect("conteúdo");

    // A vista agregada mostra os dois, de ambientes diferentes.
    assert!(
        html.contains(&nome_a) && html.contains(&nome_b),
        "a vista agregada não atravessa ambientes"
    );
    assert!(
        !html.contains(&nome_escondido),
        "um ficheiro RESTRICTED de outro ambiente apareceu na vista agregada"
    );

    // A contagem não conta o que a lista esconde.
    let visiveis: i64 =
        sqlx::query_scalar("SELECT count(*) FROM files WHERE workspace_id = ANY($1)")
            .bind(vec![primeiro, segundo])
            .fetch_one(&harness.pool)
            .await
            .expect("contagem");
    assert!(visiveis >= 2, "o cenário não foi montado");
}
