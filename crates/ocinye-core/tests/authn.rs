//! O que o `TokenVerifier` aceita, e tudo o que recusa.
//!
//! # Porque é que esta suite existe
//!
//! `GHSA-h395-gr6q-cpjc` (`CVE-2026-25537`) apanhou o `jsonwebtoken` a tratar um
//! claim standard com tipo JSON errado como se estivesse ausente. Uma validação
//! ligada deixava de ser aplicada sem que ninguém desse por isso: não havia erro,
//! não havia registo, havia um token aceite.
//!
//! O Ocinye não era explorável no dia em que o advisory saiu — mas por
//! coincidência de configuração, e não por desenho. O `exp` estava em
//! `required_spec_claims`, e a lista de claims obrigatórios recusava-o antes de
//! a validação temporal chegar a ser saltada. Tirar `exp` dessa lista, ou ligar
//! `validate_nbf`, teria aberto exactamente o buraco do advisory.
//!
//! Uma propriedade de segurança que depende de uma linha que ninguém sabe ser
//! carregante é uma propriedade por acidente. Esta suite fixa as duas coisas: o
//! comportamento da biblioteca corrigida, e o perfil de confiança que o Ocinye
//! escolheu — para que ambos passem a falhar em voz alta se mudarem.
//!
//! # O fornecedor de identidade é local
//!
//! `verify()` faz descoberta OIDC por HTTP antes de olhar para o token, e uma
//! suite que dependesse de um IdP público seria lenta, intermitente e incapaz de
//! publicar os JWKS deformados de que os casos negativos precisam. O IdP aqui é
//! um servidor em porta efémera, levantado por teste, que serve exactamente o
//! que cada caso precisa de dizer.

use std::collections::BTreeMap;
use std::net::{Ipv4Addr, SocketAddr};
use std::time::Duration;

use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};
use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use base64::Engine;
use jsonwebtoken::{Algorithm, EncodingKey, Header};
use ocinye_core::authn::TokenVerifier;
use ocinye_core::config::OidcConfig;
use serde_json::{json, Value};

const AUDIENCE: &str = "ocinye-core";

/// Chave RSA de teste, em DER PKCS#1. Gerada para esta suite e para mais nada:
/// não abre nada, não corresponde a nenhum ambiente, e o repositório é o único
/// sítio onde alguma vez existiu.
const RSA_ONE_DER: &str = concat!(
    "MIIEowIBAAKCAQEAoybe5QYRnOWky5BC5WhT3rn51BQBBpP6Zs4RUS4IpnbDsNFROAsLsV86T3",
    "yrladHZ2/KexN0IQ2cmHV+B4dLzFLf5jQHKAY/I4xC8OqtLss0rVtAwrVbFlubgtozPPBFE9WF",
    "zUH0EF//pw7C38kU3e26TjbTW+Ia7JPhq2+YpnydAwz7bslLb8RgakzHK9yqGcL8BYzU+gHs5L",
    "yqvkDdwwjDu14LvjP6AN23L8AlOwxQlEJ6Q3AlwVM+Y28sMUETkSf5kuEv6fyOLP4VOsUdroNV",
    "WX6Yiek9s9GJMzS3RwgkpB6etJkUuMGOPcbuQKxrHLePdgiFKsztkei7pGiBrQIDAQABAoIBAH",
    "knlV/nSM8FLYJJB5RKC/Yaju4xSK20177eCbgKAjxzd1Bnr/N5KF64A5ohT13bkce7GVaaFKOy",
    "8vW4UjWO//ekF0ZgnmvXb62bK96xtpdIKT8Ow6GTgSeyMWJTXLdFCVb4Ods5vd8nGrbY3FfIf+",
    "lp7u7EAeJ6nlnOW0euoGIIeg70kMfQHvwatOsrbJF/WD5/olCHQHETkaZnyvnjJjBhEod8Sw/d",
    "njjD1+xJatuiUacMDTvXTPzSIStkw271LBjzueHm0ugmQsc1h6GZxoRX6UoxiKGh9OhsuQrIdB",
    "7CdGMTk2V63asMQrjZN1saEmvwCpo/Jhmo17VbrJRDs5UCgYEA0EVTcym5rgTW4jRaYQhsXCX9",
    "mIFclfbOyIi7BtX0XrQd0ct9cwzA1Zl4QXUZPehOXXO990WLz3nWTpCzNob8VkJv5jiRRXxvvz",
    "h3QHtgGXdLXH+4WUa3/2Qic4V+mWh25xolTqpudz0iKcvCBhx3uNigr/wGAjuJdVeTj6ESjSsC",
    "gYEAyIqKEhCh+hIBSCruHP7H0ghJX9VVze0Z+SWM0vuj2kn/1tauhXiu1xY/DR+StswYQYdo0K",
    "eMLn17170y++OyVLu6Vp9m1JEHimlsX3Wcw4uzeU92bzgcd8cA+/ADmxKNHu/2/uPFRqGuSmK2",
    "UrJHASYA7KWPhVWUq4T2g6d4MIcCgYAfOeWzJJRkVPFq5PKQDdVBU6jcDrk0+tYyFt4DvlxvPo",
    "4iAGKEt2rKG8J9/fKU55moRUw8IL8/kkLhcKyOBlsbC8b/O4f8ZRXUE8E9d8IGz24LJlMbf5Di",
    "x76ql19N3O3G5JKnYsJBOnc+0P/8LpR8sG4XNLAp8YdAgqrrI/lGFQKBgQCtxP4m8Sqp2fgqZK",
    "i3jz3ujSenVuBm89DRxuvj1hEBEaYSryvCk/RMhAZWhMlIhElwXrlANC6QCkPJiTQoeAEK0zB/",
    "5tff0VvLX9Z8URlpiCAYOfbSILjqQRJlPHCjasvFxwErlIpYzbiM1ERQHBLSt++NH1jhnGqaxT",
    "gmlHLc+QKBgA4c724GJ1QRUqN6NIajXzrtMJZnv69kDOac3EIu/HDyERIawnAu8pD1BXPXxhFC",
    "b4Iu3Uyiwx9JQ0geNjjqoEvlOM2g0MEbogF21xvcrF6Rw9MYilRbGSHjx5azNhmwAF366469Lq",
    "tC4dEF+1OGA2XxTeR/0WZaczx01vDkYoSx",
);

/// Uma segunda chave RSA, para separar «esta assinatura não confere» de «esta
/// chave não é a que o `kid` indica». Sem ela, os dois casos passariam pela
/// mesma razão e nenhum deles estaria provado.
const RSA_TWO_DER: &str = concat!(
    "MIIEogIBAAKCAQEAtezmDEiM3l4ha5qH/w/GjPkWk6FHbMqgXGek0Wsw9RZyNi+wBp0DJPOj87",
    "3VDCX6SqDhTMNnHP7RTq0w9EucXSH8u+tcOsJ0giIk96cDqjluKgLCP6fH/ehQPy07jltzUa7x",
    "YQxVa8aLZnh9DkHlSWWDYkmRNmy8z0c3crPwxFUH6+U9M4+ZuFQU0MCKxvxpXL3N9vhVUQkFXJ",
    "fuen0G2NeUcocAzzGCdzqlljbchkbJBH2ZiBUBMXqdRnRucH4uArWFVJWisFC3nNSgD0Xwu4iv",
    "nkwQNuHluzAJXht884ffaGbB8r3FuaHSjGH+rz3bK0+/T9MjqkhZD6MsmAZkEQIDAQABAoIBAE",
    "OXQZ1csyHG/Um6Pz7PgKhNv0qYSC4uVkgepci8t5LHhSWGsDK00T9AWjCr3eTGk2h4v1xC2SOX",
    "tPIzPJX3JaAgiYLUHg8iwiC7SNrOLXloHvryzXLTBmXakz31s0PfzEeT5NB2waHqQU6QA35xkx",
    "ui4J560L3Uxo2IWxx9qD147XA7gw8v78yIW1cSiJ0LOWM0bHfWI9xFjEmUrynxlw5qiVSTw7lB",
    "CjBOsKixyuQ+OcwT+LSGyYKiWtqsh7WWhyaYtP6xvArT0I1yWEGqRdA6FKeL8vgV2Y8LoA0zcD",
    "LgP/1I1uiiwAAU5bh5WF85ydU02IljNEyyACo+wIKCIgECgYEA5W58FWxOWdnm1uCvposwI2IK",
    "6M+IO6nSdbRHRiiNyZgc44/lqfxMgT3AI/EX79OUHSXOVwUBbnjqjT10tW68A/ba/+bUiDRWY+",
    "NplQMCL7DOfZ63ic007BoH7iXvZ6xWlOJ0TXvHN8RAGyfOeOCuARb7/R1Rt9EOMttHuZ9NYOEC",
    "gYEAyv4W5HuZB3fPg/yOxAt6c1otKc5pTQz7cmLBlrPvoNbcGz3Qs1hJDJt33imGzhpB5nBdr4",
    "jf+JsFO8sO5opyErGi6FlBWnG3yAZj/dT8+GRGOBRJNZ0x8lWEw2Oe2OpJv2DV0updrXBEkFHc",
    "bUWxHeoTX/mg9JTPkpFEsyp8+TECgYBhISoU47oz3cUVWR/jsO9bO2mE8D8rixSXUhAOiqKkIu",
    "qwCD5hfXdiC6NpU+sfVAJTaKr3Oh5GB5i83LSpDs04HGzQg4ecdhPQy/jp5dEqIX75vZTUGPT7",
    "s3WCA7tbt6Eb/Nbh6rIHUOUW2f5WPEBOI83gdWYgHLgXHbA6zE804QKBgE/xdW6SRpgsVStzwX",
    "3vz0+yierXAZ14qWZJAlipkIvVUmXgGFspP8uZRsHVtONib+WjByJkU2CDHLKJq9qbV4WJM9Ex",
    "pY1LD8fUzHhg8yeXxhe71YEp4UvA5kJU73AaBOU767ne1vnBJx93PcpDEhfn4a2AkCUCniTo1R",
    "eCeilhAoGAZlk08AK+2G5wXm5Kjnegi2XGubo1aFC4sC0mpS0uJeH4DImbWCM2TMxCOdLy1Y+I",
    "FBoefwMi28mNFKedfZd7RTLRfs28YybTLttjJ/BBTAOy87A4nrhkPRNdork17JgrvjwfjV2/NC",
    "XoUMl5YgGrZeWU+7SKY1JvP16iqYTIxxQ=",
);

/// Chave de curva elíptica P-256, em DER PKCS#8, para a família EC.
const EC_DER: &str = concat!(
    "MIIBeQIBADCCAQMGByqGSM49AgEwgfcCAQEwLAYHKoZIzj0BAQIhAP////8AAAABAAAAAAAAAA",
    "AAAAAA////////////////MFsEIP////8AAAABAAAAAAAAAAAAAAAA///////////////8BCBa",
    "xjXYqjqT57PrvVV2mIa8ZR0GsMxTsPY7zjw+J9JgSwMVAMSdNgiG5wSTamZ44ROdJreBn36QBE",
    "EEaxfR8uEsQkf4vOblY6RA8ncDfYEt6zOg9KE5RdiYwpZP40Li/hp/m47n60p8D54WK84zV2sx",
    "Xs7LtkBoN79R9QIhAP////8AAAAA//////////+85vqtpxeehPO5ysL8YyVRAgEBBG0wawIBAQ",
    "QgC+NpC2MZoRSzeSOVWhaO1RVyjY4dlkajLO8KWNKKiUChRANCAAQTCSsP8imuTywDCfhJPPla",
    "xwx9NImro9/yVqwWo9rhRsZd8n1zMdyp1xmVX2LgWF8DtJkFCdRfNc1HVDIkVVVS",
);

/// Componentes públicos das mesmas chaves, como o JWKS os publica.
const RSA_ONE_N: &str = "oybe5QYRnOWky5BC5WhT3rn51BQBBpP6Zs4RUS4IpnbDsNFROAsLsV86T3yrladHZ2_KexN0IQ2cmHV-B4dLzFLf5jQHKAY_I4xC8OqtLss0rVtAwrVbFlubgtozPPBFE9WFzUH0EF__pw7C38kU3e26TjbTW-Ia7JPhq2-YpnydAwz7bslLb8RgakzHK9yqGcL8BYzU-gHs5LyqvkDdwwjDu14LvjP6AN23L8AlOwxQlEJ6Q3AlwVM-Y28sMUETkSf5kuEv6fyOLP4VOsUdroNVWX6Yiek9s9GJMzS3RwgkpB6etJkUuMGOPcbuQKxrHLePdgiFKsztkei7pGiBrQ";
const RSA_TWO_N: &str = "tezmDEiM3l4ha5qH_w_GjPkWk6FHbMqgXGek0Wsw9RZyNi-wBp0DJPOj873VDCX6SqDhTMNnHP7RTq0w9EucXSH8u-tcOsJ0giIk96cDqjluKgLCP6fH_ehQPy07jltzUa7xYQxVa8aLZnh9DkHlSWWDYkmRNmy8z0c3crPwxFUH6-U9M4-ZuFQU0MCKxvxpXL3N9vhVUQkFXJfuen0G2NeUcocAzzGCdzqlljbchkbJBH2ZiBUBMXqdRnRucH4uArWFVJWisFC3nNSgD0Xwu4ivnkwQNuHluzAJXht884ffaGbB8r3FuaHSjGH-rz3bK0-_T9MjqkhZD6MsmAZkEQ";
const RSA_E: &str = "AQAB";
const EC_X: &str = "EwkrD_Iprk8sAwn4STz5WscMfTSJq6Pf8lasFqPa4UY";
const EC_Y: &str = "xl3yfXMx3KnXGZVfYuBYXwO0mQUJ1F81zUdUMiRVVVI";

// ── O fornecedor de identidade de teste ─────────────────────────────────────

/// Um IdP que serve descoberta e um JWKS escolhido pelo teste.
struct Idp {
    issuer: String,
}

impl Idp {
    /// Levanta um IdP que publica exactamente este JWKS.
    ///
    /// O ouvinte é ligado antes do router porque o `jwks_uri` da descoberta tem
    /// de conter o porto, e o porto só existe depois de o sistema o atribuir.
    async fn start(jwks: Value) -> Self {
        let listener = tokio::net::TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0)))
            .await
            .expect("ouvinte do IdP");
        let issuer = format!("http://{}", listener.local_addr().expect("endereço"));

        let discovery = json!({
            "issuer": issuer,
            "jwks_uri": format!("{issuer}/jwks"),
        });

        let router = Router::new()
            .route(
                "/.well-known/openid-configuration",
                get(|State(state): State<(Value, Value)>| async move { Json(state.0) }),
            )
            .route(
                "/jwks",
                get(|State(state): State<(Value, Value)>| async move { Json(state.1) }),
            )
            .with_state((discovery, jwks));

        tokio::spawn(async move {
            let _ = axum::serve(listener, router).await;
        });

        Self { issuer }
    }

    fn verifier(&self) -> std::sync::Arc<TokenVerifier> {
        TokenVerifier::new(OidcConfig {
            issuer: self.issuer.clone(),
            audience: AUDIENCE.to_owned(),
            jwks_cache: Duration::from_secs(300),
        })
        .expect("verificador")
    }
}

// ── Chaves e JWKs ───────────────────────────────────────────────────────────

fn der(base64: &str) -> Vec<u8> {
    STANDARD.decode(base64).expect("DER de teste")
}

fn rsa_key(pkcs1: &str) -> EncodingKey {
    EncodingKey::from_rsa_der(&der(pkcs1))
}

fn ec_key() -> EncodingKey {
    EncodingKey::from_ec_der(&der(EC_DER))
}

/// Um JWK de RSA como um IdP o publicaria.
fn rsa_jwk(kid: &str, n: &str, alg: &str) -> Value {
    json!({
        "kty": "RSA", "use": "sig", "kid": kid, "alg": alg, "n": n, "e": RSA_E,
    })
}

fn ec_jwk(kid: &str, alg: &str) -> Value {
    json!({
        "kty": "EC", "use": "sig", "kid": kid, "alg": alg,
        "crv": "P-256", "x": EC_X, "y": EC_Y,
    })
}

fn jwks(keys: Vec<Value>) -> Value {
    json!({ "keys": keys })
}

/// O JWKS de que quase todos os testes partem: uma única chave RSA, `RS256`.
fn one_rsa_key() -> Value {
    jwks(vec![rsa_jwk("um", RSA_ONE_N, "RS256")])
}

// ── Cunhagem de tokens ──────────────────────────────────────────────────────

fn agora() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("relógio")
        .as_secs() as i64
}

/// Os claims de que um token bem formado precisa, para o teste depois alterar
/// exactamente um deles e mais nenhum.
fn claims_validos(issuer: &str) -> BTreeMap<String, Value> {
    let mut claims = BTreeMap::new();
    claims.insert("sub".to_owned(), json!("pessoa-de-teste"));
    claims.insert("iss".to_owned(), json!(issuer));
    claims.insert("aud".to_owned(), json!(AUDIENCE));
    claims.insert("exp".to_owned(), json!(agora() + 3600));
    claims.insert("name".to_owned(), json!("Pessoa de Teste"));
    claims
}

/// Assina claims arbitrários. Os claims são um mapa livre e não uma estrutura
/// tipada, porque metade destes testes existe precisamente para enviar valores
/// que nenhuma estrutura tipada aceitaria escrever.
fn cunhar(
    kid: &str,
    algoritmo: Algorithm,
    chave: &EncodingKey,
    claims: &BTreeMap<String, Value>,
) -> String {
    let mut header = Header::new(algoritmo);
    header.kid = Some(kid.to_owned());
    jsonwebtoken::encode(&header, claims, chave).expect("assinar")
}

/// Um token que deve ser aceite: chave certa, `kid` certo, claims certos.
fn token_bom(issuer: &str) -> String {
    cunhar(
        "um",
        Algorithm::RS256,
        &rsa_key(RSA_ONE_DER),
        &claims_validos(issuer),
    )
}

// ── O que deve passar ───────────────────────────────────────────────────────

#[tokio::test]
async fn um_token_bem_formado_e_aceite() {
    // O controlo positivo de toda a suite. Sem ele, cada recusa abaixo podia
    // estar a acontecer por uma razão que nada tem que ver com o que o teste diz
    // estar a provar — uma chave mal codificada, um IdP que não responde, um
    // backend criptográfico que não resolveu.
    let idp = Idp::start(one_rsa_key()).await;
    let verificado = idp
        .verifier()
        .verify(&token_bom(&idp.issuer))
        .await
        .expect("um token válido tem de ser aceite");

    assert_eq!(verificado.claims.sub, "pessoa-de-teste");
    assert_eq!(verificado.claims.display_name(), "Pessoa de Teste");
}

#[tokio::test]
async fn a_familia_de_curva_eliptica_tambem_e_aceite() {
    // O perfil de confiança anterior aceitava RSA e curva elíptica. A troca de
    // backend criptográfico não pode ter estreitado isso em silêncio.
    let idp = Idp::start(jwks(vec![ec_jwk("ec", "ES256")])).await;
    let token = cunhar(
        "ec",
        Algorithm::ES256,
        &ec_key(),
        &claims_validos(&idp.issuer),
    );

    let verificado = idp.verifier().verify(&token).await.expect("ES256 é aceite");
    assert_eq!(verificado.claims.sub, "pessoa-de-teste");
}

#[tokio::test]
async fn a_chave_certa_e_escolhida_de_entre_varias() {
    // Com duas chaves publicadas, o `kid` tem de decidir qual — e a que assinou
    // é a segunda, para que acertar não possa ser efeito de escolher a primeira.
    let idp = Idp::start(jwks(vec![
        rsa_jwk("um", RSA_ONE_N, "RS256"),
        rsa_jwk("dois", RSA_TWO_N, "RS256"),
    ]))
    .await;
    let token = cunhar(
        "dois",
        Algorithm::RS256,
        &rsa_key(RSA_TWO_DER),
        &claims_validos(&idp.issuer),
    );

    idp.verifier()
        .verify(&token)
        .await
        .expect("a chave dois assinou e o kid diz dois");
}

// ── O que tem de ser recusado ───────────────────────────────────────────────

/// Recusa o token e devolve a mensagem, para que o teste possa afirmar que a
/// recusa foi de autenticação e não um erro interno disfarçado.
async fn recusa(idp: &Idp, token: &str) -> String {
    let erro = idp
        .verifier()
        .verify(token)
        .await
        .expect_err("este token tinha de ser recusado");
    format!("{erro:?}")
}

#[tokio::test]
async fn uma_assinatura_forjada_nao_passa() {
    // Assinado pela chave dois, apresentado como sendo da chave um.
    let idp = Idp::start(one_rsa_key()).await;
    let token = cunhar(
        "um",
        Algorithm::RS256,
        &rsa_key(RSA_TWO_DER),
        &claims_validos(&idp.issuer),
    );
    recusa(&idp, &token).await;
}

#[tokio::test]
async fn um_token_adulterado_depois_de_assinado_nao_passa() {
    let idp = Idp::start(one_rsa_key()).await;
    let token = token_bom(&idp.issuer);

    // Trocar o `sub` no corpo, deixando a assinatura como estava.
    let partes: Vec<&str> = token.split('.').collect();
    let mut claims: Value =
        serde_json::from_slice(&URL_SAFE_NO_PAD.decode(partes[1]).expect("corpo")).expect("json");
    claims["sub"] = json!("outra-pessoa");
    let corpo = URL_SAFE_NO_PAD.encode(serde_json::to_vec(&claims).expect("json"));
    let adulterado = format!("{}.{}.{}", partes[0], corpo, partes[2]);

    recusa(&idp, &adulterado).await;
}

#[tokio::test]
async fn um_emissor_errado_nao_passa() {
    let idp = Idp::start(one_rsa_key()).await;
    let mut claims = claims_validos(&idp.issuer);
    claims.insert("iss".to_owned(), json!("https://outro-fornecedor.example"));
    recusa(
        &idp,
        &cunhar("um", Algorithm::RS256, &rsa_key(RSA_ONE_DER), &claims),
    )
    .await;
}

#[tokio::test]
async fn um_emissor_que_e_apenas_um_prefixo_nao_passa() {
    // A comparação de emissor é por igualdade. Um emissor que apenas começa
    // pelo nosso — o truque clássico de `https://idp.example.attacker.test` —
    // não pode ser aceite por parecer-se.
    let idp = Idp::start(one_rsa_key()).await;
    let mut claims = claims_validos(&idp.issuer);
    claims.insert(
        "iss".to_owned(),
        json!(format!("{}.atacante.test", idp.issuer)),
    );
    recusa(
        &idp,
        &cunhar("um", Algorithm::RS256, &rsa_key(RSA_ONE_DER), &claims),
    )
    .await;
}

#[tokio::test]
async fn uma_audiencia_errada_nao_passa() {
    let idp = Idp::start(one_rsa_key()).await;
    let mut claims = claims_validos(&idp.issuer);
    claims.insert("aud".to_owned(), json!("outro-servico"));
    recusa(
        &idp,
        &cunhar("um", Algorithm::RS256, &rsa_key(RSA_ONE_DER), &claims),
    )
    .await;
}

#[tokio::test]
async fn um_token_expirado_nao_passa() {
    let idp = Idp::start(one_rsa_key()).await;
    let mut claims = claims_validos(&idp.issuer);
    // Bem para lá da tolerância de relógio, para que a recusa seja por ter
    // expirado e não por o teste ter tido sorte com o arredondamento.
    claims.insert("exp".to_owned(), json!(agora() - 86_400));
    recusa(
        &idp,
        &cunhar("um", Algorithm::RS256, &rsa_key(RSA_ONE_DER), &claims),
    )
    .await;
}

#[tokio::test]
async fn um_token_sem_kid_nao_passa() {
    let idp = Idp::start(one_rsa_key()).await;
    let header = Header::new(Algorithm::RS256);
    let token = jsonwebtoken::encode(&header, &claims_validos(&idp.issuer), &rsa_key(RSA_ONE_DER))
        .expect("assinar");
    recusa(&idp, &token).await;
}

#[tokio::test]
async fn um_kid_desconhecido_nao_cai_para_a_unica_chave_publicada() {
    // O teste afiado da procura por `kid`: o JWKS tem exactamente uma chave, e é
    // a que assinou este token. Só o `kid` não confere. Se houvesse um recuo
    // para «a primeira chave», ou para «a única chave», ou para «alguma chave com
    // o algoritmo certo», este token passaria.
    let idp = Idp::start(one_rsa_key()).await;
    let token = cunhar(
        "um-kid-que-o-idp-nao-publica",
        Algorithm::RS256,
        &rsa_key(RSA_ONE_DER),
        &claims_validos(&idp.issuer),
    );
    recusa(&idp, &token).await;

    // Controlo positivo, no mesmo IdP e com a mesma chave: o que falhou acima
    // foi o `kid`, e nada mais.
    idp.verifier()
        .verify(&token_bom(&idp.issuer))
        .await
        .expect("a mesma chave, com o kid certo, é aceite");
}

#[tokio::test]
async fn um_token_estruturalmente_invalido_nao_passa() {
    let idp = Idp::start(one_rsa_key()).await;
    for lixo in ["", ".", "a.b.c", "nem-sequer-tem-pontos", "a.b.c.d"] {
        recusa(&idp, lixo).await;
    }
}

#[tokio::test]
async fn um_sub_vazio_nao_passa() {
    // Um token assinado pelo IdP certo mas sem sujeito não identifica ninguém, e
    // um identificador vazio propagar-se-ia como se fosse uma pessoa.
    let idp = Idp::start(one_rsa_key()).await;
    let mut claims = claims_validos(&idp.issuer);
    claims.insert("sub".to_owned(), json!("   "));
    recusa(
        &idp,
        &cunhar("um", Algorithm::RS256, &rsa_key(RSA_ONE_DER), &claims),
    )
    .await;
}

// ── Confusão de algoritmo ───────────────────────────────────────────────────

#[tokio::test]
async fn o_cabecalho_do_token_nao_escolhe_o_algoritmo_de_confianca() {
    // O ataque clássico de confusão de algoritmo: o IdP publica uma chave RSA,
    // e o atacante — que conhece o módulo público, porque está publicado — assina
    // com HMAC usando esse módulo como segredo e declara `alg: HS256`.
    //
    // A biblioteca que escolhesse o algoritmo pelo cabeçalho verificaria com
    // sucesso, porque a chave pública é o segredo partilhado. O Ocinye tira o
    // algoritmo do JWK, e o cabeçalho só pode conferir ou ser recusado.
    let idp = Idp::start(one_rsa_key()).await;
    let modulo = URL_SAFE_NO_PAD.decode(RSA_ONE_N).expect("módulo");

    let mut header = Header::new(Algorithm::HS256);
    header.kid = Some("um".to_owned());
    let token = jsonwebtoken::encode(
        &header,
        &claims_validos(&idp.issuer),
        &EncodingKey::from_secret(&modulo),
    )
    .expect("assinar com HMAC");

    recusa(&idp, &token).await;
}

#[tokio::test]
async fn um_algoritmo_diferente_do_que_a_chave_declara_nao_passa() {
    // Este é o teste que distingue mesmo as duas regras, e o de cima não era.
    //
    // No ataque de HMAC acima, a recusa também aconteceria numa implementação
    // que tirasse o algoritmo do cabeçalho, porque uma chave de RSA não serve
    // como segredo partilhado e a biblioteca recusa por família. Provava a
    // biblioteca, não o Ocinye.
    //
    // Aqui a chave é a mesma e a família é a mesma: o IdP publica uma chave
    // `RS256` e o token diz `RS512`, assinado com essa mesma chave. Nada falha
    // por incompatibilidade. Só falha se o algoritmo de confiança vier da chave.
    let idp = Idp::start(one_rsa_key()).await;
    let mut header = Header::new(Algorithm::RS512);
    header.kid = Some("um".to_owned());
    let token = jsonwebtoken::encode(&header, &claims_validos(&idp.issuer), &rsa_key(RSA_ONE_DER))
        .expect("assinar com RS512");

    recusa(&idp, &token).await;

    // Controlo positivo: a mesma chave, o mesmo IdP, com o algoritmo que a
    // chave declara. O que falhou acima foi o algoritmo, e mais nada.
    idp.verifier()
        .verify(&token_bom(&idp.issuer))
        .await
        .expect("RS256 pela mesma chave é aceite");
}

#[tokio::test]
async fn um_jwk_de_hmac_nao_entra_no_universo_de_confianca() {
    // O HMAC nunca foi aceite e continua a não ser. Um IdP comprometido que
    // publicasse um segredo partilhado como se fosse uma chave de assinatura não
    // conseguiria alargar o que o Ocinye confia.
    let idp = Idp::start(jwks(vec![json!({
        "kty": "oct", "use": "sig", "kid": "um", "alg": "HS256",
        "k": URL_SAFE_NO_PAD.encode(b"um segredo partilhado qualquer"),
    })]))
    .await;

    let mut header = Header::new(Algorithm::HS256);
    header.kid = Some("um".to_owned());
    let token = jsonwebtoken::encode(
        &header,
        &claims_validos(&idp.issuer),
        &EncodingKey::from_secret(b"um segredo partilhado qualquer"),
    )
    .expect("assinar");

    recusa(&idp, &token).await;
}

#[tokio::test]
async fn um_jwk_sem_algoritmo_declarado_e_recusado() {
    // O algoritmo vem da chave. Uma chave que não diz qual é não pode ter o
    // cabeçalho do token a dizê-lo por ela.
    let idp = Idp::start(jwks(vec![json!({
        "kty": "RSA", "use": "sig", "kid": "um", "n": RSA_ONE_N, "e": RSA_E,
    })]))
    .await;
    recusa(&idp, &token_bom(&idp.issuer)).await;
}

// ── Confusão de tipo de chave ───────────────────────────────────────────────

#[tokio::test]
async fn uma_chave_rsa_nao_e_usada_como_curva_eliptica() {
    // O JWK tem material de RSA e declara um algoritmo de curva elíptica. As duas
    // metades não podem ser combinadas para formar algo que verifique.
    let idp = Idp::start(jwks(vec![rsa_jwk("um", RSA_ONE_N, "ES256")])).await;
    let token = cunhar(
        "um",
        Algorithm::ES256,
        &ec_key(),
        &claims_validos(&idp.issuer),
    );
    recusa(&idp, &token).await;
}

#[tokio::test]
async fn uma_chave_de_curva_eliptica_nao_e_usada_como_rsa() {
    let idp = Idp::start(jwks(vec![ec_jwk("um", "RS256")])).await;
    recusa(&idp, &token_bom(&idp.issuer)).await;
}

#[tokio::test]
async fn um_jwk_sem_material_de_chave_e_recusado() {
    let idp = Idp::start(jwks(vec![json!({
        "kty": "RSA", "use": "sig", "kid": "um", "alg": "RS256", "n": "", "e": "",
    })]))
    .await;
    recusa(&idp, &token_bom(&idp.issuer)).await;
}

#[tokio::test]
async fn um_jwks_vazio_nao_aceita_nada() {
    let idp = Idp::start(jwks(vec![])).await;
    recusa(&idp, &token_bom(&idp.issuer)).await;
}

// ── Os claims que o Ocinye exige ────────────────────────────────────────────
//
// Estes três testes não são decoração. São o que torna visível a linha que
// salvou o Ocinye deste advisory: `required_spec_claims` contém `exp`, e é por
// isso que um `exp` deformado nunca chegou a passar por aqui. Se alguém a
// remover por parecer redundante, estes testes dizem-no.

#[tokio::test]
async fn um_token_sem_exp_e_recusado() {
    let idp = Idp::start(one_rsa_key()).await;
    let mut claims = claims_validos(&idp.issuer);
    claims.remove("exp");
    recusa(
        &idp,
        &cunhar("um", Algorithm::RS256, &rsa_key(RSA_ONE_DER), &claims),
    )
    .await;
}

#[tokio::test]
async fn um_token_sem_emissor_e_recusado() {
    let idp = Idp::start(one_rsa_key()).await;
    let mut claims = claims_validos(&idp.issuer);
    claims.remove("iss");
    recusa(
        &idp,
        &cunhar("um", Algorithm::RS256, &rsa_key(RSA_ONE_DER), &claims),
    )
    .await;
}

#[tokio::test]
async fn um_token_sem_audiencia_e_recusado() {
    let idp = Idp::start(one_rsa_key()).await;
    let mut claims = claims_validos(&idp.issuer);
    claims.remove("aud");
    recusa(
        &idp,
        &cunhar("um", Algorithm::RS256, &rsa_key(RSA_ONE_DER), &claims),
    )
    .await;
}

// ── A regressão do advisory ─────────────────────────────────────────────────

#[tokio::test]
async fn um_exp_com_o_tipo_errado_nao_passa_pelo_verificador() {
    // Pelo caminho real do Ocinye. Aqui a recusa vem da lista de claims
    // obrigatórios, e não da correcção do advisory — o teste abaixo separa as
    // duas coisas.
    let idp = Idp::start(one_rsa_key()).await;
    let mut claims = claims_validos(&idp.issuer);
    claims.insert("exp".to_owned(), json!("depois do almoço"));
    recusa(
        &idp,
        &cunhar("um", Algorithm::RS256, &rsa_key(RSA_ONE_DER), &claims),
    )
    .await;
}

/// O perfil que reproduz a condição do advisory.
///
/// Não é o perfil do Ocinye e não deve tornar-se. A produção exige `exp` e não
/// valida `nbf`; ligar `validate_nbf` em produção só para poder dizer que o CVE
/// foi testado seria mudar a política para caber no teste.
///
/// O que este perfil faz é o contrário: monta exactamente a forma vulnerável —
/// uma validação **ligada** sobre um claim **não obrigatório** — que é onde a
/// versão 9.3.1 tratava «tipo errado» como «ausente» e saltava a verificação
/// sem dizer nada.
fn perfil_do_advisory(algoritmo: Algorithm, claim: &str) -> jsonwebtoken::Validation {
    let mut validation = jsonwebtoken::Validation::new(algoritmo);
    validation.required_spec_claims = std::collections::HashSet::new();
    validation.validate_aud = false;
    validation.validate_exp = claim == "exp";
    validation.validate_nbf = claim == "nbf";
    validation
}

fn chave_de_verificacao() -> jsonwebtoken::DecodingKey {
    jsonwebtoken::DecodingKey::from_rsa_components(RSA_ONE_N, RSA_E).expect("chave pública")
}

#[test]
fn um_exp_com_o_tipo_errado_e_recusado_e_nao_tratado_como_ausente() {
    // Esta é a propriedade que o `CVE-2026-25537` violava, isolada da política
    // do Ocinye. Na 9.3.1 este token era **aceite**: `exp` não estava na lista
    // de obrigatórios, `validate_exp` estava ligado, e um `exp` que não era um
    // número caía no mesmo ramo que um `exp` inexistente — nenhuma verificação
    // temporal chegava a correr.
    let mut claims = BTreeMap::new();
    claims.insert("sub".to_owned(), json!("pessoa-de-teste"));
    claims.insert("exp".to_owned(), json!("depois do almoço"));
    let token = cunhar("um", Algorithm::RS256, &rsa_key(RSA_ONE_DER), &claims);

    let erro = jsonwebtoken::decode::<Value>(
        &token,
        &chave_de_verificacao(),
        &perfil_do_advisory(Algorithm::RS256, "exp"),
    )
    .expect_err("um exp com o tipo errado tem de ser recusado");

    assert!(
        matches!(erro.kind(), jsonwebtoken::errors::ErrorKind::InvalidClaimFormat(c) if c == "exp"),
        "a recusa tem de ser por formato do claim, e não por outra razão qualquer: {erro:?}"
    );
}

#[test]
fn um_nbf_com_o_tipo_errado_e_recusado_e_nao_tratado_como_ausente() {
    let mut claims = BTreeMap::new();
    claims.insert("sub".to_owned(), json!("pessoa-de-teste"));
    claims.insert("nbf".to_owned(), json!("logo à tarde"));
    let token = cunhar("um", Algorithm::RS256, &rsa_key(RSA_ONE_DER), &claims);

    let erro = jsonwebtoken::decode::<Value>(
        &token,
        &chave_de_verificacao(),
        &perfil_do_advisory(Algorithm::RS256, "nbf"),
    )
    .expect_err("um nbf com o tipo errado tem de ser recusado");

    assert!(
        matches!(erro.kind(), jsonwebtoken::errors::ErrorKind::InvalidClaimFormat(c) if c == "nbf"),
        "a recusa tem de ser por formato do claim: {erro:?}"
    );
}

#[test]
fn o_mesmo_perfil_aceita_os_claims_bem_formados() {
    // O controlo positivo dos dois testes acima. Sem ele, ambos podiam estar a
    // recusar por o perfil rejeitar tudo, ou por a chave não conferir, e a
    // propriedade do advisory continuaria por provar.
    for (claim, valor) in [("exp", agora() + 3600), ("nbf", agora() - 3600)] {
        let mut claims = BTreeMap::new();
        claims.insert("sub".to_owned(), json!("pessoa-de-teste"));
        claims.insert(claim.to_owned(), json!(valor));
        let token = cunhar("um", Algorithm::RS256, &rsa_key(RSA_ONE_DER), &claims);

        jsonwebtoken::decode::<Value>(
            &token,
            &chave_de_verificacao(),
            &perfil_do_advisory(Algorithm::RS256, claim),
        )
        .unwrap_or_else(|erro| panic!("um {claim} bem formado tem de ser aceite: {erro:?}"));
    }
}

#[test]
fn um_nbf_no_futuro_continua_a_ser_recusado_quando_e_validado() {
    // O tipo certo, o valor errado. A correcção do advisory não pode ter
    // substituído a verificação temporal por uma verificação de tipo.
    let mut claims = BTreeMap::new();
    claims.insert("sub".to_owned(), json!("pessoa-de-teste"));
    claims.insert("nbf".to_owned(), json!(agora() + 86_400));
    let token = cunhar("um", Algorithm::RS256, &rsa_key(RSA_ONE_DER), &claims);

    let erro = jsonwebtoken::decode::<Value>(
        &token,
        &chave_de_verificacao(),
        &perfil_do_advisory(Algorithm::RS256, "nbf"),
    )
    .expect_err("um token ainda não válido tem de ser recusado");

    assert!(
        matches!(
            erro.kind(),
            jsonwebtoken::errors::ErrorKind::ImmatureSignature
        ),
        "a recusa tem de ser por o token ainda não ser válido: {erro:?}"
    );
}

// ── O fornecedor não configurado ────────────────────────────────────────────

#[tokio::test]
async fn sem_fornecedor_configurado_nada_e_aceite() {
    // O Ocinye não tem OIDC ligado nesta instalação (ADR-0103: a autenticação é
    // do Core). Um verificador sem emissor não pode ter um modo permissivo, uma
    // descoberta automática, ou um emissor por omissão: não havendo com que
    // comparar, a única resposta correcta é recusar.
    let verificador = TokenVerifier::new(OidcConfig {
        issuer: String::new(),
        audience: AUDIENCE.to_owned(),
        jwks_cache: Duration::from_secs(300),
    })
    .expect("verificador");

    assert!(!verificador.is_configured());

    // Um token que seria válido perante um IdP a sério — para que a recusa seja
    // por não haver fornecedor, e não por o token ser mau.
    let idp = Idp::start(one_rsa_key()).await;
    let erro = verificador
        .verify(&token_bom(&idp.issuer))
        .await
        .expect_err("sem fornecedor configurado, nenhum token é aceite");

    // A recusa tem de ser de autenticação, e não uma avaria interna. A
    // diferença não é cosmética: um erro interno traduz-se em 500, diz a quem
    // sondar que há aqui um caminho que existe e está partido, e não é verdade.
    // Não há fornecedor; a resposta certa é «não estás autenticado».
    assert!(
        matches!(erro, ocinye_core::error::CoreError::Unauthenticated(_)),
        "recusa por falta de fornecedor tem de ser de autenticação: {erro:?}"
    );

    // E o mesmo token, perante o IdP que o emitiu, é aceite. É o que separa
    // «o verificador recusa por não estar configurado» de «o verificador recusa
    // sempre», que não provaria nada.
    idp.verifier()
        .verify(&token_bom(&idp.issuer))
        .await
        .expect("o mesmo token é aceite pelo fornecedor que o emitiu");
}

#[tokio::test]
async fn um_fornecedor_que_nao_responde_nao_deixa_passar_nada() {
    // Falhar a consultar o IdP não é o mesmo que o IdP ter dito que está tudo
    // bem. Um porto onde não está ninguém tem de recusar.
    let verificador = TokenVerifier::new(OidcConfig {
        issuer: "http://127.0.0.1:1".to_owned(),
        audience: AUDIENCE.to_owned(),
        jwks_cache: Duration::from_secs(300),
    })
    .expect("verificador");

    let idp = Idp::start(one_rsa_key()).await;
    verificador
        .verify(&token_bom(&idp.issuer))
        .await
        .expect_err("um fornecedor inalcançável recusa");
}
