//! Uma Instância instalada, vista por uma pessoa (Parte 9 da generalização).
//!
//! Este teste **não** levanta o Ocinye OS: conduz um que `install/ocinye`
//! instalou num anfitrião Linux descartável, por HTTPS, como um browser a sério.
//! Corre só quando lhe dizem onde — `scripts/install-e2e.sh` di-lo, depois de
//! instalar num anfitrião limpo — e é por isso `#[ignore]` na suite normal.
//!
//! O percurso: entrar com a credencial temporária que o instalador entregou →
//! definir a palavra-passe → enrolar o segundo factor pela chave manual → sessão
//! privilegiada → Aplicações do perfil → Ficheiros → uma Nota guardada → o
//! Prompt a responder, sem fornecedor, com o estado degradado tipado.
//!
//! Variáveis: `OCINYE_TEST_INSTALLED_URL` (https://…), `OCINYE_TEST_INSTALLED_EMAIL`,
//! `OCINYE_TEST_INSTALLED_CREDENTIAL_FILE`, `OCINYE_TEST_INSTALLED_PROFILE`,
//! `OCINYE_TEST_INSTALLED_RESOLVE` (`nome ip`, para um domínio de teste),
//! `OCINYE_TEST_INSTALLED_STATE_FILE` (onde guardar a palavra-passe e o seed do
//! segundo factor, para quem volta a entrar numa Instância restaurada) e
//! `OCINYE_TEST_CHROME`.
//!
//! O segundo teste, [`uma_instancia_restaurada_reconhece_quem_la_estava`], é a
//! prova de restauro (Parte 11): a mesma pessoa entra na Instância restaurada
//! noutro anfitrião com a mesma palavra-passe e o mesmo segundo factor — cujo
//! seed está selado, pelo que entrar prova que a raiz de selagem viajou — e
//! encontra a nota e o ficheiro que lá deixou.

use std::time::{Duration, Instant};

use chromiumoxide::browser::{Browser, BrowserConfig};
use chromiumoxide::Page;
use futures::StreamExt;

const PRAZO: Duration = Duration::from_secs(20);

fn var(nome: &str) -> String {
    std::env::var(nome).unwrap_or_else(|_| panic!("{nome} não está definida"))
}

fn chrome() -> String {
    std::env::var("OCINYE_TEST_CHROME").unwrap_or_else(|_| {
        [
            "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
            "/usr/bin/google-chrome",
            "/usr/bin/chromium",
        ]
        .into_iter()
        .find(|c| std::path::Path::new(c).exists())
        .expect("não há Chrome; defina OCINYE_TEST_CHROME")
        .to_owned()
    })
}

async fn texto(page: &Page) -> String {
    page.evaluate("document.body ? document.body.innerText : ''")
        .await
        .ok()
        .and_then(|v| v.into_value::<String>().ok())
        .unwrap_or_default()
}

async fn esperar_por(page: &Page, agulha: &str) {
    let inicio = Instant::now();
    loop {
        if texto(page).await.contains(agulha) {
            return;
        }
        assert!(
            inicio.elapsed() < PRAZO,
            "«{agulha}» não apareceu em {PRAZO:?}; a página diz:\n{}",
            texto(page).await
        );
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
}

async fn condicao(page: &Page, expressao: &str) -> bool {
    let inicio = Instant::now();
    while inicio.elapsed() < PRAZO {
        let ok = page
            .evaluate(expressao)
            .await
            .ok()
            .and_then(|v| v.into_value::<bool>().ok())
            .unwrap_or(false);
        if ok {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    false
}

async fn escrever(page: &Page, seletor: &str, valor: &str) {
    let valor = serde_json::to_string(valor).expect("json");
    let seletor_js = serde_json::to_string(seletor).expect("json");
    assert!(
        condicao(page, &format!("!!document.querySelector({seletor_js})")).await,
        "{seletor} não existe em {}; a página diz:\n{}",
        page.url().await.ok().flatten().unwrap_or_default(),
        texto(page).await
    );
    page.evaluate(format!(
        "(() => {{ const c = document.querySelector({seletor_js}); c.value = {valor}; \
         c.dispatchEvent(new Event('input', {{ bubbles: true }})); return true; }})()"
    ))
    .await
    .expect("escrever");
}

async fn submeter(page: &Page, formulario: &str) {
    let alvo = format!("{formulario} button[type=submit]");
    let seletor_js = serde_json::to_string(&alvo).expect("json");
    assert!(
        condicao(page, &format!("!!document.querySelector({seletor_js})")).await,
        "{alvo} não existe"
    );
    page.find_element(&alvo)
        .await
        .expect("botão")
        .click()
        .await
        .expect("submeter");
}

fn totp(seed_base32: &str) -> String {
    use data_encoding::BASE32_NOPAD;
    use hmac::{Hmac, Mac};
    use sha1::Sha1;
    let seed = BASE32_NOPAD
        .decode(seed_base32.trim().to_ascii_uppercase().as_bytes())
        .expect("base32");
    let passo = u64::try_from(chrono::Utc::now().timestamp().max(0)).unwrap_or(0) / 30;
    let mut mac = <Hmac<Sha1> as Mac>::new_from_slice(&seed).expect("hmac");
    mac.update(&passo.to_be_bytes());
    let h = mac.finalize().into_bytes();
    let o = usize::from(h[19] & 0x0f);
    let n = (u32::from(h[o] & 0x7f) << 24)
        | (u32::from(h[o + 1]) << 16)
        | (u32::from(h[o + 2]) << 8)
        | u32::from(h[o + 3]);
    format!("{:06}", n % 1_000_000)
}

#[tokio::test]
#[ignore = "conduz uma Instância instalada; corre por scripts/install-e2e.sh"]
async fn uma_instancia_instalada_abre_entra_e_trabalha() {
    let base = var("OCINYE_TEST_INSTALLED_URL");
    let email = var("OCINYE_TEST_INSTALLED_EMAIL");
    let credencial = std::fs::read_to_string(var("OCINYE_TEST_INSTALLED_CREDENTIAL_FILE"))
        .expect("credencial")
        .trim()
        .to_owned();
    let perfil = var("OCINYE_TEST_INSTALLED_PROFILE");

    let mut config = BrowserConfig::builder()
        .chrome_executable(chrome())
        .user_data_dir(std::env::temp_dir().join(format!("ocinye-install-{}", std::process::id())))
        .no_sandbox()
        // O certificado é auto-assinado pelo instalador; o que se prova aqui é a
        // Instância, e não uma autoridade de certificação.
        .arg("ignore-certificate-errors");
    if let Ok(regra) = std::env::var("OCINYE_TEST_INSTALLED_RESOLVE") {
        config = config.arg(("host-resolver-rules", format!("MAP {regra}").as_str()));
    }
    let (browser, mut handler) = Browser::launch(config.build().expect("config"))
        .await
        .expect("Chrome");
    tokio::spawn(async move { while handler.next().await.is_some() {} });

    // Tempos de cada passo, para a certificação de hardware (Parte 15): quanto
    // espera uma pessoa, medido no browser, e não um pedido isolado.
    let mut tempos: Vec<(&str, u128)> = Vec::new();

    // ── Entrar com a credencial temporária ──────────────────────────────
    let t = Instant::now();
    let page = browser
        .new_page(format!("{base}/login"))
        .await
        .expect("login");
    escrever(&page, "input[name=email]", &email).await;
    escrever(&page, "input[name=password]", &credencial).await;
    submeter(&page, "form").await;
    esperar_por(&page, "Defina a sua palavra-passe").await;
    tempos.push(("entrar", t.elapsed().as_millis()));

    let nova = format!("Instalada-{}-2026!", std::process::id());
    escrever(&page, "#new-pass", &nova).await;
    escrever(&page, "#confirm-pass", &nova).await;
    submeter(&page, "form[action=\"/first-access\"]").await;
    esperar_por(&page, "Configurar o segundo factor").await;

    // ── O segundo factor, pela chave manual ─────────────────────────────
    page.goto(format!("{base}/mfa?show_key=1"))
        .await
        .expect("chave");
    esperar_por(&page, "Chave manual").await;
    let seed: String = page
        .evaluate("document.querySelector('[data-oc=\"secret\"]').getAttribute('data-oc-value')")
        .await
        .expect("seed")
        .into_value()
        .expect("seed");
    escrever(&page, "#mfa-code", &totp(&seed)).await;
    submeter(&page, "form[action=\"/mfa/confirm\"]").await;
    if let Ok(estado) = std::env::var("OCINYE_TEST_INSTALLED_STATE_FILE") {
        std::fs::write(&estado, format!("{nova}\n{seed}\n")).expect("guardar o estado");
    }
    esperar_por(&page, "Guardar códigos de recuperação").await;
    page.find_element("input[name=acknowledged]")
        .await
        .expect("reconhecimento")
        .click()
        .await
        .expect("marcar");
    submeter(&page, "form[action=\"/mfa/acknowledge\"]").await;
    esperar_por(&page, "SESSÃO PRIVILEGIADA").await;

    // ── As aplicações do perfil ─────────────────────────────────────────
    page.goto(format!("{base}/")).await.expect("home");
    page.find_element(r#"[data-oc="launcher-open"]"#)
        .await
        .expect("lançador")
        .click()
        .await
        .expect("abrir");
    let tem = |rota: &str| {
        format!(r#"!!document.querySelector('[data-oc="launcher-item"][href="{rota}"]')"#)
    };
    assert!(
        condicao(&page, &tem("/files")).await,
        "Ficheiros não está no lançador"
    );
    assert!(
        condicao(&page, &tem("/notes")).await,
        "Notas não está no lançador"
    );
    // O perfil persistido é o que o instalador recebeu — lido da própria
    // Administração › Instância, e não inferido do lançador: as aplicações de
    // investigação dependem também de o membro ter um papel de investigação, e o
    // administrador privilegiado não o tem.
    page.goto(format!("{base}/admin/instance"))
        .await
        .expect("instância");
    let persistido: String = {
        assert!(
            condicao(&page, "!!document.querySelector('#instance-profile')").await,
            "a Administração › Instância não abriu"
        );
        page.evaluate("document.querySelector('#instance-profile').value")
            .await
            .expect("perfil")
            .into_value()
            .expect("perfil")
    };
    assert_eq!(persistido, perfil, "o perfil persistido não é o instalado");
    assert!(
        ocinye_contracts::InstanceProfile::ALL
            .iter()
            .any(|p| p.as_str() == perfil),
        "perfil desconhecido: {perfil}"
    );

    // ── Ficheiros ───────────────────────────────────────────────────────
    let t = Instant::now();
    page.goto(format!("{base}/files")).await.expect("ficheiros");
    esperar_por(&page, "Ficheiros").await;
    tempos.push(("abrir_ficheiros", t.elapsed().as_millis()));

    // ── Um ficheiro, pelo carregamento de «Meus ficheiros» ──────────────
    page.evaluate(
        "(() => { const i = document.querySelector('[data-oc=\"fs-carregar\"]'); \
           const dt = new DataTransfer(); \
           dt.items.add(new File(['prova de instalação'], 'prova-instalacao.txt', \
             { type: 'text/plain' })); \
           i.files = dt.files; \
           i.dispatchEvent(new Event('change', { bubbles: true })); return true; })()",
    )
    .await
    .expect("carregar");
    let inicio = Instant::now();
    loop {
        tokio::time::sleep(Duration::from_secs(1)).await;
        page.goto(format!("{base}/files")).await.expect("ficheiros");
        if texto(&page).await.contains("prova-instalacao.txt") {
            break;
        }
        assert!(
            inicio.elapsed() < PRAZO,
            "o ficheiro carregado não apareceu em Meus ficheiros"
        );
    }

    // ── Uma nota, guardada pelo Core ────────────────────────────────────
    let t = Instant::now();
    page.goto(format!("{base}/notes")).await.expect("notas");
    esperar_por(&page, "Notas").await;
    submeter(&page, "form[action=\"/notes\"]").await;
    assert!(
        condicao(&page, "location.pathname.startsWith('/notes/')").await,
        "criar uma nota não abriu o editor"
    );
    escrever(&page, "[data-oc-notes-title]", "Primeira nota da Instância").await;
    esperar_por(&page, "Guardado").await;
    tempos.push(("criar_e_guardar_nota", t.elapsed().as_millis()));

    // ── O Prompt, sem fornecedor ────────────────────────────────────────
    let t = Instant::now();
    page.goto(format!("{base}/ai/prompt"))
        .await
        .expect("prompt");
    escrever(
        &page,
        r#"[data-oc="prompt-textarea"]"#,
        "Resume o estado da Instância",
    )
    .await;
    page.find_element(r#"[data-oc="prompt-send"]"#)
        .await
        .expect("enviar")
        .click()
        .await
        .expect("enviar");
    assert!(
        condicao(&page, "!!document.querySelector('.oc-turn--ocinye')").await,
        "o Prompt não respondeu"
    );
    tempos.push(("prompt_responde", t.elapsed().as_millis()));
    if let Ok(ficheiro) = std::env::var("OCINYE_TEST_INSTALLED_TIMINGS_FILE") {
        let linhas: String = tempos
            .iter()
            .map(|(passo, ms)| format!("{passo} {ms}\n"))
            .collect();
        std::fs::write(ficheiro, linhas).expect("guardar os tempos");
    }
    // O turno é do **sistema**, nunca de um modelo, e diz porquê por uma razão
    // tipada. Para o administrador privilegiado a razão é a autorização — ele
    // não tem `ai.use`, de propósito —; para um membro seria a ausência de
    // fornecedor (provado na suite de browser). Em nenhum dos dois casos houve
    // inferência.
    let turno: String = page
        .evaluate(
            "(() => { const t = document.querySelector('.oc-turn--ocinye'); \
               t.querySelectorAll('details').forEach(d => d.open = true); \
               return t.innerText; })()",
        )
        .await
        .expect("turno")
        .into_value()
        .expect("turno");
    assert!(
        [
            "AI_NO_PROVIDER_AVAILABLE",
            "AI_NO_COMPATIBLE_MODEL",
            "AI_PERMISSION_DENIED"
        ]
        .iter()
        .any(|razao| turno.contains(razao)),
        "o Prompt não concluiu com uma razão tipada:\n{turno}"
    );
    assert!(
        !turno.to_lowercase().contains("modelo:"),
        "sem fornecedor, nenhum modelo pode ter respondido:\n{turno}"
    );
}

#[tokio::test]
#[ignore = "conduz uma Instância restaurada; corre por scripts/restore-e2e.sh"]
async fn uma_instancia_restaurada_reconhece_quem_la_estava() {
    let base = var("OCINYE_TEST_INSTALLED_URL");
    let email = var("OCINYE_TEST_INSTALLED_EMAIL");
    let estado = std::fs::read_to_string(var("OCINYE_TEST_INSTALLED_STATE_FILE")).expect("estado");
    let mut linhas = estado.lines();
    let senha = linhas.next().expect("palavra-passe").to_owned();
    let seed = linhas.next().expect("seed").to_owned();

    let mut config = BrowserConfig::builder()
        .chrome_executable(chrome())
        .user_data_dir(std::env::temp_dir().join(format!("ocinye-restore-{}", std::process::id())))
        .no_sandbox()
        .arg("ignore-certificate-errors");
    if let Ok(regra) = std::env::var("OCINYE_TEST_INSTALLED_RESOLVE") {
        config = config.arg(("host-resolver-rules", format!("MAP {regra}").as_str()));
    }
    let (browser, mut handler) = Browser::launch(config.build().expect("config"))
        .await
        .expect("Chrome");
    tokio::spawn(async move { while handler.next().await.is_some() {} });

    // A mesma palavra-passe, verificada contra o verificador restaurado.
    let page = browser
        .new_page(format!("{base}/login"))
        .await
        .expect("login");
    escrever(&page, "input[name=email]", &email).await;
    escrever(&page, "input[name=password]", &senha).await;
    submeter(&page, "form").await;
    esperar_por(&page, "Confirme o segundo factor").await;

    // O mesmo segundo factor: o seed está selado com a raiz de selagem da origem.
    // Aceitá-lo aqui é a prova de que ela viajou e abre o que devia.
    escrever(&page, "#mfa-code", &totp(&seed)).await;
    submeter(&page, "form[action=\"/mfa/challenge\"]").await;
    esperar_por(&page, "SESSÃO PRIVILEGIADA").await;

    page.goto(format!("{base}/notes")).await.expect("notas");
    esperar_por(&page, "Primeira nota da Instância").await;
    page.goto(format!("{base}/files")).await.expect("ficheiros");
    esperar_por(&page, "prova-instalacao.txt").await;
}
