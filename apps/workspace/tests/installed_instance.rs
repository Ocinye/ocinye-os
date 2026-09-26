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
//! Variáveis: `OCINYE_INSTALLED_URL` (https://…), `OCINYE_INSTALLED_EMAIL`,
//! `OCINYE_INSTALLED_CREDENTIAL_FILE`, `OCINYE_INSTALLED_PROFILE`,
//! `OCINYE_INSTALLED_RESOLVE` (`nome:porto:ip`, para um domínio de teste) e
//! `OCINYE_TEST_CHROME`.

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
        "{seletor} não existe"
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
    let base = var("OCINYE_INSTALLED_URL");
    let email = var("OCINYE_INSTALLED_EMAIL");
    let credencial = std::fs::read_to_string(var("OCINYE_INSTALLED_CREDENTIAL_FILE"))
        .expect("credencial")
        .trim()
        .to_owned();
    let perfil = var("OCINYE_INSTALLED_PROFILE");

    let mut config = BrowserConfig::builder()
        .chrome_executable(chrome())
        .user_data_dir(std::env::temp_dir().join(format!("ocinye-install-{}", std::process::id())))
        .no_sandbox()
        // O certificado é auto-assinado pelo instalador; o que se prova aqui é a
        // Instância, e não uma autoridade de certificação.
        .arg("--ignore-certificate-errors");
    if let Ok(regra) = std::env::var("OCINYE_INSTALLED_RESOLVE") {
        config = config.arg(format!("--host-resolver-rules=MAP {regra}"));
    }
    let (browser, mut handler) = Browser::launch(config.build().expect("config"))
        .await
        .expect("Chrome");
    tokio::spawn(async move { while handler.next().await.is_some() {} });

    // ── Entrar com a credencial temporária ──────────────────────────────
    let page = browser
        .new_page(format!("{base}/login"))
        .await
        .expect("login");
    escrever(&page, "input[name=email]", &email).await;
    escrever(&page, "input[name=password]", &credencial).await;
    submeter(&page, "form").await;
    esperar_por(&page, "Defina a sua palavra-passe").await;

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
    // O perfil decide as aplicações opcionais: Ideias é de investigação.
    let ideias = condicao(&page, &tem("/ideas")).await;
    assert_eq!(
        ideias,
        perfil == "research",
        "o lançador do perfil {perfil} {} Ideias",
        if ideias { "traz" } else { "não traz" }
    );

    // ── Ficheiros ───────────────────────────────────────────────────────
    page.goto(format!("{base}/files")).await.expect("ficheiros");
    esperar_por(&page, "Ficheiros").await;

    // ── Uma nota, guardada pelo Core ────────────────────────────────────
    page.goto(format!("{base}/notes")).await.expect("notas");
    esperar_por(&page, "Notas").await;
    submeter(&page, "form[action=\"/notes\"]").await;
    assert!(
        condicao(&page, "location.pathname.startsWith('/notes/')").await,
        "criar uma nota não abriu o editor"
    );
    escrever(&page, "[data-oc-notes-title]", "Primeira nota da Instância").await;
    esperar_por(&page, "Guardado").await;

    // ── O Prompt, sem fornecedor ────────────────────────────────────────
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
    let turno: String = page
        .evaluate("document.querySelector('.oc-turn--ocinye').innerText")
        .await
        .expect("turno")
        .into_value()
        .expect("turno");
    assert!(
        turno.contains("AI_NO_PROVIDER_AVAILABLE"),
        "sem fornecedor, o Prompt devia concluir degradado com a razão tipada:\n{turno}"
    );
}
