//! Segundo factor obrigatório para identidades privilegiadas (ADR-0107).
//!
//! # O que este módulo detém
//!
//! O TOTP (RFC 6238) e os códigos de recuperação. Duas metades opostas do mesmo
//! problema: o seed do TOTP tem de ser **recuperável** — o código recalcula-se a
//! cada trinta segundos, e um verificador não serve —, e por isso é **selado**;
//! um código de recuperação prova-se e nunca se lê de volta, e por isso é
//! **resumido** com Argon2id, como uma palavra-passe.
//!
//! # A fronteira do factor
//!
//! Uma palavra-passe, por si só, não estabelece autoridade privilegiada
//! ([`mfa_required`]). Depois de a palavra-passe ser aceite, a sessão de uma
//! identidade com MFA obrigatório fica num estado que não trabalha até o segundo
//! factor ser satisfeito — o portão vive na fronteira central, não aqui. Este
//! módulo só sabe enrolar, confirmar e verificar.
//!
//! # O que nunca sai daqui em claro
//!
//! O seed, depois de selado. Os códigos de recuperação, depois de mostrados uma
//! vez. Nem um nem outro chega a log, a auditoria ou a mensagem de erro.

use chrono::{DateTime, Duration, Utc};
use data_encoding::BASE32_NOPAD;
use hmac::{Hmac, Mac};
use ocinye_contracts::{SessionState, TechnicalRole};
use ocinye_observability::CorrelationIds;
use rand::rand_core::UnwrapErr;
use rand::rngs::SysRng;
use rand::RngExt as _;
use sha1::Sha1;
use sqlx::PgPool;
use uuid::Uuid;

use super::authentication::{AttemptContext, IssuedSession, SESSION_LIFETIME_HOURS};
use super::credentials as creds;
use super::model::Person;
use crate::audit::{self, action, AuditEntry};
use crate::error::{CoreError, CoreResult};
use crate::password::sealed::{self, SealingDomain, SealingKey};
use crate::password::{Hasher, Secret};
use ocinye_domain::Principal;

type HmacSha1 = Hmac<Sha1>;

/// Dígitos de um código TOTP. Seis é o que os autenticadores mostram.
const DIGITS: u32 = 6;
/// A janela de tempo de um código, em segundos.
const PERIOD: u64 = 30;
/// Quantos passos de tolerância de relógio se aceitam de cada lado. Um passo —
/// trinta segundos — cobre a deriva normal entre o telefone e o servidor sem
/// alargar a janela em que um código roubado ainda vale.
const SKEW: i64 = 1;
/// Bytes do seed. Cento e sessenta bits, o recomendado pela RFC 4226.
const SEED_BYTES: usize = 20;
/// Quantos códigos de recuperação se emitem de cada vez.
const RECOVERY_CODES: usize = 10;
/// Grupos e tamanho de grupo de um código de recuperação: `XXXXX-XXXXX-XXXXX`.
const RECOVERY_GROUPS: usize = 3;
const RECOVERY_GROUP_LEN: usize = 5;
/// Alfabeto dos códigos de recuperação: sem `0/O/1/I/L` para não os confundir
/// quando alguém os copia do ecrã para o papel.
const RECOVERY_ALPHABET: &[u8] = b"23456789ABCDEFGHJKMNPQRSTUVWXYZ";

/// A política de quem precisa de MFA, resolvida da autoridade do próprio
/// principal.
///
/// Ou a identidade é privilegiada, ou tem `PlatformAdmin` efectivo — as duas
/// dimensões são independentes (ADR-0100), e **qualquer** uma exige o segundo
/// factor. Uma palavra-passe sozinha não basta para nenhuma.
#[must_use]
pub fn mfa_required(principal: &Principal) -> bool {
    principal.identity_kind.is_privileged() || principal.has_role(&[TechnicalRole::PlatformAdmin])
}

// ── TOTP puro (RFC 6238) ──────────────────────────────────────────────────

/// HOTP de um contador (RFC 4226): HMAC-SHA1 truncado a `DIGITS` dígitos.
fn hotp(seed: &[u8], counter: u64) -> u32 {
    let mut mac = HmacSha1::new_from_slice(seed).expect("HMAC-SHA1 aceita qualquer comprimento");
    mac.update(&counter.to_be_bytes());
    let digest = mac.finalize().into_bytes();

    // Truncamento dinâmico: os 4 bits baixos do último byte escolhem o offset.
    let offset = (digest[digest.len() - 1] & 0x0f) as usize;
    let binario = (u32::from(digest[offset] & 0x7f) << 24)
        | (u32::from(digest[offset + 1]) << 16)
        | (u32::from(digest[offset + 2]) << 8)
        | u32::from(digest[offset + 3]);

    binario % 10_u32.pow(DIGITS)
}

/// O passo de tempo TOTP que um código satisfaz, se algum, num dado instante.
///
/// Aceita a deriva de relógio de [`SKEW`] passos de cada lado, e devolve o passo
/// que correspondeu — é essa identidade que a protecção de replay persiste, para
/// que um passo já gasto não volte a valer. Um seed ilegível ou um código não
/// numérico é `None`. Procura do passo mais recente para o mais antigo, para que
/// o maior passo válido seja o escolhido quando mais de um serve.
#[must_use]
fn matched_step(seed_base32: &str, codigo: &str, agora: DateTime<Utc>) -> Option<u64> {
    let seed = BASE32_NOPAD
        .decode(seed_base32.trim().to_ascii_uppercase().as_bytes())
        .ok()?;
    // Normaliza o que a pessoa escreveu: espaços e traços que um autenticador
    // por vezes mostra não fazem parte do código.
    let limpo: String = codigo.chars().filter(char::is_ascii_digit).collect();
    let valor = limpo.parse::<u32>().ok()?;

    let passo = (agora.timestamp().max(0) as u64) / PERIOD;
    for delta in (-SKEW..=SKEW).rev() {
        let Some(contador) = passo.checked_add_signed(delta) else {
            continue;
        };
        if hotp(&seed, contador) == valor {
            return Some(contador);
        }
    }
    None
}

/// Verifica um código TOTP contra um seed em base32, num dado instante, **sem**
/// protecção de replay — para testes e para o ponto único onde o replay é
/// aplicado ([`verify_challenge`], [`confirm_enrollment`]).
///
/// A resposta a «este código serve?» é sempre sim ou não.
#[must_use]
pub fn verify_totp(seed_base32: &str, codigo: &str, agora: DateTime<Utc>) -> bool {
    matched_step(seed_base32, codigo, agora).is_some()
}

/// Um seed novo, em base32.
fn generate_seed() -> String {
    let bytes: [u8; SEED_BYTES] = UnwrapErr(SysRng).random();
    BASE32_NOPAD.encode(&bytes)
}

/// A URI `otpauth://` que um autenticador lê de um QR.
///
/// `issuer` sem espaços de propósito — evita a codificação percent e o risco de
/// um leitor a interpretar mal. A conta é o endereço institucional.
fn otpauth_uri(issuer: &str, conta: &str, seed_base32: &str) -> String {
    format!(
        "otpauth://totp/{issuer}:{conta}?secret={seed_base32}&issuer={issuer}\
         &algorithm=SHA1&digits={DIGITS}&period={PERIOD}",
        conta = percent(conta),
    )
}

/// Codificação percent mínima, só do que uma etiqueta `otpauth` precisa.
fn percent(valor: &str) -> String {
    let mut saida = String::with_capacity(valor.len());
    for b in valor.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~' | b'@') {
            saida.push(b as char);
        } else {
            saida.push_str(&format!("%{b:02X}"));
        }
    }
    saida
}

/// Códigos de recuperação novos, em claro. Só existem neste retorno; o que fica
/// guardado é o verificador de cada um.
fn generate_recovery_codes() -> Vec<String> {
    (0..RECOVERY_CODES)
        .map(|_| {
            let grupos: Vec<String> = (0..RECOVERY_GROUPS)
                .map(|_| {
                    (0..RECOVERY_GROUP_LEN)
                        .map(|_| {
                            let i = (UnwrapErr(SysRng).random::<u32>() as usize)
                                % RECOVERY_ALPHABET.len();
                            RECOVERY_ALPHABET[i] as char
                        })
                        .collect::<String>()
                })
                .collect();
            grupos.join("-")
        })
        .collect()
}

// ── O enrolamento, a confirmação, o desafio ────────────────────────────────

/// O que o enrolamento devolve, uma única vez.
#[derive(Debug)]
pub struct Enrollment {
    /// O seed em base32, para quem escreve a chave à mão.
    pub secret_base32: String,
    /// A URI `otpauth://`, para o QR.
    pub otpauth_uri: String,
}

/// Se esta pessoa já tem um seed TOTP **confirmado**.
///
/// # Errors
///
/// Erro de base de dados.
pub async fn has_confirmed_totp(pool: &PgPool, person_id: Uuid) -> CoreResult<bool> {
    let existe: bool = sqlx::query_scalar(
        "SELECT EXISTS (
             SELECT 1 FROM mfa_totp_secrets
              WHERE person_id = $1 AND confirmed_at IS NOT NULL
         )",
    )
    .bind(person_id)
    .fetch_one(pool)
    .await?;
    Ok(existe)
}

/// Começa o enrolamento: gera um seed, sela-o com a subchave `mfa-totp` e
/// guarda-o **por confirmar**, substituindo qualquer enrolamento anterior ainda
/// não confirmado.
///
/// Não conclui nada: o enrolamento só se fecha quando um código válido prova que
/// o autenticador do membro já gera o código certo ([`confirm_enrollment`]).
///
/// # Errors
///
/// [`CoreError::CapabilityUnavailable`] sem raiz de selagem; erro de validação
/// se já houver um seed confirmado (repor é outra operação); erro de base.
pub async fn begin_enrollment(
    pool: &PgPool,
    raiz: Option<&SealingKey>,
    person: &Person,
    issuer: &str,
) -> CoreResult<Enrollment> {
    let raiz = raiz.ok_or_else(|| {
        CoreError::CapabilityUnavailable(
            "O Ocinye OS não tem chave de selagem configurada; o MFA não pode ser \
             enrolado nesta instalação."
                .to_owned(),
        )
    })?;

    if has_confirmed_totp(pool, person.id).await? {
        return Err(CoreError::Validation(
            "Esta identidade já tem MFA activo. Para trocar o autenticador, reponha o MFA primeiro."
                .to_owned(),
        ));
    }

    // Idempotente: se já há um seed por confirmar, reaproveita-se — não se gera
    // outro. O QR e a chave manual têm de mostrar **o mesmo** seed, e voltar a
    // esta página, ou pedir a chave manual, não pode trocá-lo por baixo dos pés
    // (ADR-0107). Um seed novo só nasce quando não existe nenhum a confirmar.
    let seed = match open_seed(pool, raiz, person.id).await? {
        Some(existente) => existente,
        None => {
            let novo = generate_seed();
            let selado = sealed::seal(raiz, SealingDomain::MfaTotp, &novo)?;
            sqlx::query(
                "INSERT INTO mfa_totp_secrets (person_id, nonce, ciphertext, confirmed_at)
                 VALUES ($1, $2, $3, NULL)
                 ON CONFLICT (person_id) DO UPDATE
                     SET nonce = EXCLUDED.nonce,
                         ciphertext = EXCLUDED.ciphertext,
                         confirmed_at = NULL,
                         updated_at = now()",
            )
            .bind(person.id)
            .bind(&selado.nonce)
            .bind(&selado.ciphertext)
            .execute(pool)
            .await?;
            novo
        }
    };

    Ok(Enrollment {
        otpauth_uri: otpauth_uri(issuer, &person.email, &seed),
        secret_base32: seed,
    })
}

/// Confirma o enrolamento com um código, e emite os códigos de recuperação.
///
/// Verifica o código contra o seed por confirmar; se servir, marca-o confirmado
/// e substitui os códigos de recuperação por um conjunto novo, devolvido **uma
/// única vez**. O que fica guardado são os verificadores.
///
/// # Errors
///
/// [`CoreError::NotFound`] se não há enrolamento a confirmar;
/// [`CoreError::Validation`] se o código não serve; erro de base ou de selagem.
pub async fn confirm_enrollment(
    pool: &PgPool,
    raiz: Option<&SealingKey>,
    hasher: &Hasher,
    actor: &Principal,
    person: &Person,
    codigo: &str,
    ids: &CorrelationIds,
) -> CoreResult<Vec<String>> {
    let raiz = raiz.ok_or_else(|| {
        CoreError::CapabilityUnavailable(
            "O Ocinye OS não tem chave de selagem configurada.".to_owned(),
        )
    })?;

    let seed = open_seed(pool, raiz, person.id)
        .await?
        .ok_or_else(|| {
            CoreError::NotFound("Não há um enrolamento de MFA a confirmar.".to_owned())
        })?;

    let Some(passo) = matched_step(&seed, codigo, Utc::now()) else {
        return Err(CoreError::Validation(
            "O código não confere. Verifique a hora do dispositivo e tente o código actual."
                .to_owned(),
        ));
    };
    // O passo que confirmou fica registado como aceite, para não poder ser
    // reusado como o primeiro desafio logo a seguir (ADR-0107).
    let step = i64::try_from(passo).unwrap_or(i64::MAX);

    let codigos = generate_recovery_codes();

    let mut tx = pool.begin().await?;
    sqlx::query(
        "UPDATE mfa_totp_secrets
            SET confirmed_at = now(), last_accepted_step = $2, updated_at = now()
          WHERE person_id = $1",
    )
    .bind(person.id)
    .bind(step)
    .execute(&mut *tx)
    .await?;
    substituir_recuperacao(&mut tx, hasher, person.id, &codigos).await?;
    audit::record(
        &mut tx,
        Some(actor),
        ids,
        AuditEntry::new(action::MFA_ENROLLED, "person")
            .resource(person.id)
            .detail("recovery_codes_issued", codigos.len() as i64),
    )
    .await?;
    tx.commit().await?;

    Ok(codigos)
}

/// Verifica um código TOTP contra o seed **confirmado** desta pessoa.
///
/// # Errors
///
/// Erro de base ou de selagem. Um seed ausente ou um código errado é `Ok(false)`.
pub async fn verify_challenge(
    pool: &PgPool,
    raiz: Option<&SealingKey>,
    person_id: Uuid,
    codigo: &str,
) -> CoreResult<bool> {
    let Some(raiz) = raiz else {
        // Sem raiz, nenhum seed abre — e um factor que não se consegue verificar
        // recusa-se, nunca se assume satisfeito (fail closed, ADR-0107).
        return Ok(false);
    };

    let linha: Option<(Vec<u8>, Vec<u8>, Option<i64>)> = sqlx::query_as(
        "SELECT nonce, ciphertext, last_accepted_step
           FROM mfa_totp_secrets
          WHERE person_id = $1 AND confirmed_at IS NOT NULL",
    )
    .bind(person_id)
    .fetch_optional(pool)
    .await?;
    let Some((nonce, ciphertext, last_step)) = linha else {
        return Ok(false);
    };

    let seed = sealed::open(
        raiz,
        SealingDomain::MfaTotp,
        &sealed::Sealed { nonce, ciphertext },
    )?;
    let Some(passo) = matched_step(&seed, codigo, Utc::now()) else {
        return Ok(false);
    };

    // Replay: um passo já aceite — ou anterior ao último aceite — não vale outra
    // vez, mesmo dentro da janela de tolerância (ADR-0107). A condição vai no
    // próprio UPDATE, para que dois desafios concorrentes com o mesmo código não
    // passem os dois: só a escrita que avança o passo é que conta.
    let step = i64::try_from(passo).unwrap_or(i64::MAX);
    if last_step.is_some_and(|ultimo| step <= ultimo) {
        return Ok(false);
    }
    let avancou = sqlx::query(
        "UPDATE mfa_totp_secrets
            SET last_accepted_step = $2, updated_at = now()
          WHERE person_id = $1
            AND (last_accepted_step IS NULL OR last_accepted_step < $2)",
    )
    .bind(person_id)
    .bind(step)
    .execute(pool)
    .await?
    .rows_affected();

    Ok(avancou == 1)
}

/// Consome um código de recuperação, atomicamente.
///
/// Percorre os verificadores vivos e, ao encontrar o que corresponde, marca
/// **essa** linha consumida com um `UPDATE` condicional no estado — pelo que dois
/// pedidos concorrentes com o mesmo código gastam-no uma vez só.
///
/// # Errors
///
/// Erro de base. Um código que não corresponde a nenhum vivo é `Ok(false)`.
pub async fn consume_recovery_code(
    pool: &PgPool,
    hasher: &Hasher,
    actor: &Principal,
    person_id: Uuid,
    codigo: &str,
    ids: &CorrelationIds,
) -> CoreResult<bool> {
    let normal = normalizar_recuperacao(codigo);
    if normal.is_empty() {
        return Ok(false);
    }
    let secret = Secret::new(normal);

    let vivos: Vec<(Uuid, String)> = sqlx::query_as(
        "SELECT id, verifier FROM mfa_recovery_codes
          WHERE person_id = $1 AND state = 'active'",
    )
    .bind(person_id)
    .fetch_all(pool)
    .await?;

    for (id, verifier) in &vivos {
        if hasher.verify(&secret, verifier) {
            // A marca de consumo e o registo numa transacção só. Condicional no
            // estado: se outra chamada já o consumiu, não afecta linha nenhuma —
            // o mesmo código não se gasta duas vezes.
            let mut tx = pool.begin().await?;
            let afetadas = sqlx::query(
                "UPDATE mfa_recovery_codes
                    SET state = 'consumed', consumed_at = now()
                  WHERE id = $1 AND state = 'active'",
            )
            .bind(id)
            .execute(&mut *tx)
            .await?
            .rows_affected();

            if afetadas == 1 {
                audit::record(
                    &mut tx,
                    Some(actor),
                    ids,
                    AuditEntry::new(action::RECOVERY_CODE_USED, "person").resource(person_id),
                )
                .await?;
                tx.commit().await?;
                return Ok(true);
            }
            // Já estava consumido: desfaz e segue — pode haver outro vivo.
            drop(tx);
        }
    }
    Ok(false)
}

/// Fecha o portão de MFA: revoga a sessão que o atravessava e emite uma
/// **nova**, `active` e com a garantia de MFA satisfeita.
///
/// Revoga e recria em vez de promover no lugar, pelo mesmo motivo que a mudança
/// de palavra-passe (ADR-0107, briefing §30): o token que a sessão-portão levava
/// deixa de valer, e a sessão que fica é fronteira limpa de autenticação. O
/// token antigo é negado depois disto.
///
/// # Errors
///
/// Erro de base de dados.
pub async fn issue_assured_session(
    pool: &PgPool,
    person: &Person,
    gate_session_id: Uuid,
    context: &AttemptContext,
    ids: &CorrelationIds,
) -> CoreResult<IssuedSession> {
    let mut tx = pool.begin().await?;
    creds::revoke_session(&mut *tx, gate_session_id, "mfa_satisfied").await?;
    let (session_id, token) = creds::create_session(
        &mut *tx,
        person.id,
        SessionState::Active,
        true,
        Duration::hours(SESSION_LIFETIME_HOURS),
        context.user_agent.as_deref(),
        context.ip_prefix.as_deref(),
    )
    .await?;
    audit::record(
        &mut tx,
        None,
        ids,
        AuditEntry::new(action::SIGN_IN, "person")
            .resource(person.id)
            .actor(person.id, person.organisation_id)
            .detail("mfa", "satisfied")
            .detail("session_id", session_id.to_string()),
    )
    .await?;
    tx.commit().await?;

    Ok(IssuedSession {
        token,
        state: SessionState::Active,
        person_id: person.id,
        display_name: person.preferred_name().to_owned(),
    })
}

/// Lê e abre o seed selado **por confirmar** de uma pessoa, se existir.
///
/// O seed já confirmado abre-se em [`verify_challenge`], que precisa de ler o
/// passo aceite na mesma linha — por isso essa leitura vive lá, e esta serve só
/// o enrolamento.
async fn open_seed(
    pool: &PgPool,
    raiz: &SealingKey,
    person_id: Uuid,
) -> CoreResult<Option<String>> {
    let linha: Option<(Vec<u8>, Vec<u8>)> = sqlx::query_as(
        "SELECT nonce, ciphertext FROM mfa_totp_secrets
          WHERE person_id = $1 AND confirmed_at IS NULL",
    )
    .bind(person_id)
    .fetch_optional(pool)
    .await?;

    match linha {
        None => Ok(None),
        Some((nonce, ciphertext)) => {
            let seed = sealed::open(
                raiz,
                SealingDomain::MfaTotp,
                &sealed::Sealed { nonce, ciphertext },
            )?;
            Ok(Some(seed))
        }
    }
}

/// Substitui os códigos de recuperação de uma pessoa pelos verificadores dos
/// novos. Regenerar invalida os anteriores.
async fn substituir_recuperacao(
    tx: &mut crate::Tx<'_>,
    hasher: &Hasher,
    person_id: Uuid,
    codigos: &[String],
) -> CoreResult<()> {
    sqlx::query("DELETE FROM mfa_recovery_codes WHERE person_id = $1")
        .bind(person_id)
        .execute(&mut **tx)
        .await?;
    for codigo in codigos {
        let verifier = hasher.hash(&Secret::new(normalizar_recuperacao(codigo)))?;
        sqlx::query("INSERT INTO mfa_recovery_codes (person_id, verifier) VALUES ($1, $2)")
            .bind(person_id)
            .bind(verifier)
            .execute(&mut **tx)
            .await?;
    }
    Ok(())
}

/// Normaliza um código de recuperação para comparação: sem traços, sem espaços,
/// em maiúsculas. O que se resume é a forma normalizada, e é a mesma que se
/// verifica.
fn normalizar_recuperacao(codigo: &str) -> String {
    codigo
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_uppercase())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Vectores da RFC 6238, apêndice B, para SHA1. O seed do teste é a string
    /// ASCII "12345678901234567890" (o «Key» da RFC), em base32.
    ///
    /// A RFC tabela dá 8 dígitos; o Ocinye usa 6, por isso comparam-se os **6
    /// últimos** de cada valor esperado.
    #[test]
    fn vectores_rfc6238() {
        let seed = BASE32_NOPAD.encode(b"12345678901234567890");
        // (instante unix, TOTP de 8 dígitos da RFC)
        let casos = [
            (59_i64, "94287082"),
            (1_111_111_109, "07081804"),
            (1_111_111_111, "14050471"),
            (1_234_567_890, "89005924"),
            (2_000_000_000, "69279037"),
            (20_000_000_000, "65353130"),
        ];
        for (unix, esperado_8) in casos {
            let esperado_6 = &esperado_8[esperado_8.len() - 6..];
            let agora = DateTime::from_timestamp(unix, 0).expect("instante");
            assert!(
                verify_totp(&seed, esperado_6, agora),
                "o vector RFC de {unix} não confere: esperava {esperado_6}"
            );
        }
    }

    /// A tolerância de relógio aceita um passo de cada lado, e não dois.
    #[test]
    fn a_janela_aceita_um_passo_de_cada_lado() {
        let seed = generate_seed();
        let base = 1_700_000_000_i64;
        let agora = DateTime::from_timestamp(base, 0).unwrap();
        let codigo_de = |t: i64| -> String {
            let seed_bytes = BASE32_NOPAD.decode(seed.as_bytes()).unwrap();
            format!(
                "{:0width$}",
                hotp(&seed_bytes, (t as u64) / PERIOD),
                width = DIGITS as usize
            )
        };
        // Um passo antes e um passo depois conferem.
        assert!(verify_totp(&seed, &codigo_de(base - PERIOD as i64), agora));
        assert!(verify_totp(&seed, &codigo_de(base + PERIOD as i64), agora));
        // Dois passos, não.
        assert!(!verify_totp(
            &seed,
            &codigo_de(base + 2 * PERIOD as i64),
            agora
        ));
    }

    /// Um código não numérico ou um seed ilegível é `false`, não pânico.
    #[test]
    fn entrada_invalida_e_falsa_e_nao_entra_em_panico() {
        let seed = generate_seed();
        let agora = Utc::now();
        assert!(!verify_totp(&seed, "abcxyz", agora));
        assert!(!verify_totp(&seed, "", agora));
        assert!(!verify_totp("nao-e-base32-!!", "123456", agora));
    }

    /// Espaços e traços na entrada não impedem um código de conferir.
    #[test]
    fn o_codigo_tolera_espacos_e_tracos() {
        let seed = BASE32_NOPAD.encode(b"12345678901234567890");
        let agora = DateTime::from_timestamp(59, 0).unwrap();
        // 287082 são os 6 últimos do vector das 59s.
        assert!(verify_totp(&seed, "287 082", agora));
        assert!(verify_totp(&seed, "287-082", agora));
    }

    /// Os códigos de recuperação têm a forma esperada e não se repetem.
    #[test]
    fn os_codigos_de_recuperacao_tem_forma_e_variam() {
        let codigos = generate_recovery_codes();
        assert_eq!(codigos.len(), RECOVERY_CODES);
        for c in &codigos {
            assert_eq!(
                c.len(),
                RECOVERY_GROUPS * RECOVERY_GROUP_LEN + (RECOVERY_GROUPS - 1)
            );
            assert_eq!(c.matches('-').count(), RECOVERY_GROUPS - 1);
        }
        let unicos: std::collections::HashSet<_> = codigos.iter().collect();
        assert_eq!(
            unicos.len(),
            codigos.len(),
            "dois códigos de recuperação iguais"
        );
    }

    /// A URI otpauth carrega os parâmetros que um autenticador espera.
    #[test]
    fn a_uri_otpauth_tem_os_parametros() {
        let uri = otpauth_uri("Ocinye", "fidel.admin@ocinye.com", "ABCDEF");
        assert!(uri.starts_with("otpauth://totp/Ocinye:"));
        assert!(uri.contains("secret=ABCDEF"));
        assert!(uri.contains("issuer=Ocinye"));
        assert!(uri.contains("algorithm=SHA1"));
        assert!(uri.contains("digits=6"));
        assert!(uri.contains("period=30"));
    }
}
