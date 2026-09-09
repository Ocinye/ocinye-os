//! Segredos que o sistema tem de poder voltar a ler.
//!
//! # Porque isto existe ao lado do Argon2
//!
//! O resto de `password/` guarda **verificadores**: provam uma senha sem a
//! conter, e não há forma de a recuperar. É a propriedade certa para uma senha
//! do Ocinye, e é por isso que nenhum administrador consegue ler a de outro
//! membro.
//!
//! Uma senha de IMAP é outra coisa: tem de ser **apresentada** ao servidor de
//! correio a cada sessão. Um verificador não serve, e fingir que serve seria não
//! ter correio.
//!
//! Isto é o custo declarado no [ADR-0409]: o Core passa a deter segredos
//! recuperáveis, e a superfície fica contida — a decifra acontece num sítio só,
//! no momento de abrir a sessão, e o texto em claro nunca sai desse ponto.
//!
//! [ADR-0409]: ../../../../docs/adrs/0409-mailbox-credentials-per-member.md

use base64::Engine as _;
use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};
use hkdf::Hkdf;
use rand::rand_core::UnwrapErr;
use rand::rngs::SysRng;
use rand::RngExt as _;
use sha2::Sha256;

use crate::{CoreError, CoreResult};

/// Quantos bytes tem a chave. Duzentos e cinquenta e seis bits.
const CHAVE_BYTES: usize = 32;

/// Quantos bytes tem o nonce do `ChaCha20-Poly1305`.
const NONCE_BYTES: usize = 12;

/// A versão do formato de um criptograma selado.
///
/// Vai como primeiro byte do criptograma, para que abrir um segredo saiba que
/// esquema o fechou **sem adivinhar** (ADR-0107). Hoje só há um: chave efectiva
/// derivada por HKDF a partir da raiz. Um esquema futuro incrementa este número,
/// e um byte desconhecido é recusado — nunca interpretado à sorte.
const ESQUEMA_V1: u8 = 1;

/// O domínio de um segredo: que classe de material está a ser selada.
///
/// Cada domínio deriva uma **subchave efectiva distinta** da mesma raiz, com uma
/// etiqueta explícita e versionada. Correio e MFA nunca partilham subchave, e um
/// criptograma de um domínio não se abre com a subchave do outro — mesmo sendo a
/// mesma raiz (ADR-0107).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SealingDomain {
    /// Credenciais de caixa de correio (ADR-0409).
    Mail,
    /// Seeds TOTP das identidades com MFA (ADR-0107).
    MfaTotp,
}

impl SealingDomain {
    /// A etiqueta HKDF do domínio. Explícita, versionada, e nunca reordenada:
    /// mudá-la trocaria a subchave e tornaria ilegível o que já foi selado.
    const fn label(self) -> &'static [u8] {
        match self {
            Self::Mail => b"ocinye/sealing/mail/v1",
            Self::MfaTotp => b"ocinye/sealing/mfa-totp/v1",
        }
    }
}

/// A **raiz** institucional de selagem desta instalação.
///
/// Vive fora da base de dados, na configuração (`OCINYE_SEALING_KEY`). Quem
/// obtiver um despejo da base obtém criptogramas; quem obtiver a raiz sem a base
/// não obtém nada. A raiz **nunca** cifra directamente: cada classe de segredo
/// usa uma subchave derivada por [`SealingDomain`] (ADR-0107). É o único material
/// criptográfico durável do sistema, e a continuidade transporta-o como um só.
#[derive(Clone)]
pub struct SealingKey(Key);

impl std::fmt::Debug for SealingKey {
    /// Nunca imprime a chave.
    ///
    /// Um `derive(Debug)` numa chave é a maneira de ela acabar num registo de
    /// erro sem ninguém ter decidido isso.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SealingKey(<oculta>)")
    }
}

impl SealingKey {
    /// Lê a chave da sua forma configurada: 32 bytes em base64.
    ///
    /// # Errors
    ///
    /// Recusa uma chave que não seja exactamente de 32 bytes. Uma chave curta
    /// não é uma chave fraca — é uma chave que o algoritmo não aceita, e
    /// preenchê-la até ao tamanho seria inventar entropia que ninguém deu.
    pub fn from_base64(valor: &str) -> CoreResult<Self> {
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(valor.trim())
            .map_err(|_| {
                CoreError::Validation("A chave de selagem não está em base64 válido.".to_owned())
            })?;

        let exacta: [u8; CHAVE_BYTES] = bytes.as_slice().try_into().map_err(|_| {
            CoreError::Validation(format!(
                "A chave de selagem tem de ter {CHAVE_BYTES} bytes; esta tem {}.",
                bytes.len()
            ))
        })?;

        Ok(Self(exacta.into()))
    }

    /// Uma chave nova, para quem está a instalar.
    #[must_use]
    pub fn generate() -> String {
        let bytes: [u8; CHAVE_BYTES] = UnwrapErr(SysRng).random();
        base64::engine::general_purpose::STANDARD.encode(bytes)
    }

    /// Deriva a subchave efectiva de um domínio, por HKDF-SHA256.
    ///
    /// A raiz é o IKM; a etiqueta do domínio é o `info`. Domínios diferentes dão
    /// subchaves independentes, e a subchave nunca é persistida nem sai daqui —
    /// vive o tempo de uma operação (ADR-0107).
    fn subkey(&self, dominio: SealingDomain) -> Key {
        let hk = Hkdf::<Sha256>::new(None, self.0.as_slice());
        let mut okm = [0_u8; CHAVE_BYTES];
        // `expand` só falha se o comprimento pedido exceder 255*HashLen; 32 bytes
        // de SHA-256 nunca o fazem, por isso a falha é impossível por construção.
        hk.expand(dominio.label(), &mut okm)
            .expect("HKDF-SHA256 de 32 bytes está sempre dentro do limite");
        Key::from(okm)
    }
}

/// Um segredo fechado: o criptograma e o nonce com que foi fechado.
///
/// Os dois viajam juntos porque separados não servem para nada, e guardá-los em
/// sítios diferentes é a maneira de um deles se perder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sealed {
    /// O nonce, único por registo.
    pub nonce: Vec<u8>,
    /// O criptograma, com a etiqueta de autenticação incluída.
    pub ciphertext: Vec<u8>,
}

/// Fecha um segredo.
///
/// # Porque o nonce é novo de cada vez
///
/// Porque reutilizar um nonce com a mesma chave em `ChaCha20-Poly1305` revela a
/// diferença entre os dois textos em claro. Duas caixas com a mesma senha não
/// podem produzir o mesmo criptograma, e não produzem.
///
/// # Errors
///
/// Devolve erro quando a cifra falha, o que na prática significa que o sistema
/// não tem entropia — e nesse caso não se escreve nada.
pub fn seal(raiz: &SealingKey, dominio: SealingDomain, claro: &str) -> CoreResult<Sealed> {
    // O mesmo gerador que gera as credenciais temporárias do Ocinye: do sistema,
    // e não um `thread_rng` que se possa vir a semear de forma previsível.
    // `UnwrapErr` dá a face infalível do gerador do sistema, tal como em
    // `password::generate`: um nonce que falhasse a gerar não teria por onde
    // continuar, e tratar isso como erro recuperável seria fingir uma escolha.
    let nonce_bytes: [u8; NONCE_BYTES] = UnwrapErr(SysRng).random();

    let cifra = ChaCha20Poly1305::new(&raiz.subkey(dominio));
    let nonce = Nonce::from(nonce_bytes);
    let corpo = cifra
        .encrypt(&nonce, claro.as_bytes())
        .map_err(|_| CoreError::Internal("a cifra do segredo falhou".into()))?;

    // O criptograma leva o número do esquema à frente, para se identificar.
    let mut ciphertext = Vec::with_capacity(1 + corpo.len());
    ciphertext.push(ESQUEMA_V1);
    ciphertext.extend_from_slice(&corpo);

    Ok(Sealed {
        nonce: nonce_bytes.to_vec(),
        ciphertext,
    })
}

/// Abre um segredo fechado.
///
/// # Porque uma falha aqui não diz o que correu mal
///
/// Chave errada, criptograma alterado e nonce trocado devolvem o mesmo erro. A
/// diferença entre eles é informação para quem está a tentar adivinhar, e não
/// para quem está a usar o sistema.
///
/// # Errors
///
/// Devolve erro quando a autenticação falha — o que inclui a chave errada e o
/// criptograma alterado. Nunca devolve texto em claro que não tenha sido
/// escrito por esta chave.
pub fn open(raiz: &SealingKey, dominio: SealingDomain, fechado: &Sealed) -> CoreResult<String> {
    if fechado.nonce.len() != NONCE_BYTES {
        return Err(CoreError::Internal("segredo com nonce inválido".into()));
    }

    // O esquema vem à frente. Um byte desconhecido é recusado, não adivinhado:
    // decifrar um formato que não se reconhece é interpretar bytes à sorte.
    let (esquema, corpo) = fechado
        .ciphertext
        .split_first()
        .ok_or_else(|| CoreError::Internal("segredo sem esquema".into()))?;
    if *esquema != ESQUEMA_V1 {
        return Err(CoreError::Internal(
            "esquema de selagem desconhecido".into(),
        ));
    }

    let cifra = ChaCha20Poly1305::new(&raiz.subkey(dominio));
    let nonce_bytes: [u8; NONCE_BYTES] = fechado.nonce.as_slice().try_into().map_err(|_| {
        CoreError::Validation("A credencial guardada não tem um nonce válido.".to_owned())
    })?;
    let nonce = Nonce::from(nonce_bytes);
    let claro = cifra
        .decrypt(&nonce, corpo)
        .map_err(|_| CoreError::Internal("o segredo não pôde ser aberto".into()))?;

    String::from_utf8(claro).map_err(|_| CoreError::Internal("segredo ilegível".into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chave() -> SealingKey {
        SealingKey::from_base64(&SealingKey::generate()).expect("chave")
    }

    /// O que se fecha, abre-se — no mesmo domínio.
    #[test]
    fn um_segredo_fechado_volta_a_ler_se() {
        let k = chave();
        let fechado = seal(&k, SealingDomain::Mail, "senha-do-imap").expect("cifrar");
        assert_eq!(
            open(&k, SealingDomain::Mail, &fechado).expect("decifrar"),
            "senha-do-imap"
        );
    }

    /// E não se lê com outra raiz.
    #[test]
    fn outra_chave_nao_abre() {
        let fechado = seal(&chave(), SealingDomain::Mail, "senha-do-imap").expect("cifrar");
        assert!(
            open(&chave(), SealingDomain::Mail, &fechado).is_err(),
            "um segredo abriu-se com uma raiz que não o fechou"
        );
    }

    /// A separação de domínio: o que a raiz fecha para um domínio não se abre
    /// com a subchave de outro, ainda que a raiz seja a mesma (ADR-0107).
    #[test]
    fn um_dominio_nao_abre_o_do_outro() {
        let k = chave();
        let selado_mail = seal(&k, SealingDomain::Mail, "seed").expect("cifrar");
        assert!(
            open(&k, SealingDomain::MfaTotp, &selado_mail).is_err(),
            "a subchave de MFA abriu um criptograma do correio"
        );

        let selado_mfa = seal(&k, SealingDomain::MfaTotp, "seed").expect("cifrar");
        assert!(
            open(&k, SealingDomain::Mail, &selado_mfa).is_err(),
            "a subchave do correio abriu um criptograma de MFA"
        );
    }

    /// As subchaves dos dois domínios são de facto diferentes.
    #[test]
    fn subchaves_de_dominios_diferem() {
        let k = chave();
        assert_ne!(
            k.subkey(SealingDomain::Mail).as_slice(),
            k.subkey(SealingDomain::MfaTotp).as_slice(),
            "os dois domínios derivaram a mesma subchave"
        );
        // E a derivação é determinística: a mesma raiz e o mesmo domínio dão
        // sempre a mesma subchave, ou o que foi selado ontem não abriria hoje.
        assert_eq!(
            k.subkey(SealingDomain::Mail).as_slice(),
            k.subkey(SealingDomain::Mail).as_slice()
        );
    }

    /// Um esquema desconhecido é recusado, não interpretado à sorte.
    #[test]
    fn um_esquema_desconhecido_e_recusado() {
        let k = chave();
        let mut fechado = seal(&k, SealingDomain::Mail, "seed").expect("cifrar");
        fechado.ciphertext[0] = 0xFF;
        assert!(
            open(&k, SealingDomain::Mail, &fechado).is_err(),
            "um esquema desconhecido foi aceite"
        );
    }

    /// Um criptograma alterado é recusado, e não devolve lixo.
    ///
    /// # Porque isto é a razão de ser da autenticação
    ///
    /// Sem ela, quem tivesse escrita na base podia mudar um byte e o sistema
    /// apresentaria o resultado ao servidor de correio como se fosse a senha.
    /// Com `Poly1305`, a decifra recusa.
    #[test]
    fn um_criptograma_alterado_e_recusado() {
        let k = chave();
        let mut fechado = seal(&k, SealingDomain::Mail, "senha-do-imap").expect("cifrar");
        // Altera o corpo (a seguir ao byte de esquema), não o esquema.
        fechado.ciphertext[1] ^= 0x01;
        assert!(
            open(&k, SealingDomain::Mail, &fechado).is_err(),
            "um criptograma alterado passou por senha"
        );
    }

    /// A mesma senha, fechada duas vezes, dá criptogramas diferentes.
    ///
    /// Duas caixas com a mesma senha não podem ser reconhecidas como tal por
    /// quem só vê a base.
    #[test]
    fn a_mesma_senha_nao_produz_o_mesmo_criptograma() {
        let k = chave();
        let a = seal(&k, SealingDomain::Mail, "a-mesma").expect("cifrar");
        let b = seal(&k, SealingDomain::Mail, "a-mesma").expect("cifrar");
        assert_ne!(a.nonce, b.nonce, "o nonce repetiu-se");
        assert_ne!(a.ciphertext, b.ciphertext, "o criptograma repetiu-se");
    }

    /// Uma chave do tamanho errado é recusada, e não completada.
    #[test]
    fn uma_chave_do_tamanho_errado_e_recusada() {
        let curta = base64::engine::general_purpose::STANDARD.encode([0_u8; 16]);
        assert!(SealingKey::from_base64(&curta).is_err());

        let longa = base64::engine::general_purpose::STANDARD.encode([0_u8; 64]);
        assert!(SealingKey::from_base64(&longa).is_err());

        assert!(SealingKey::from_base64("isto não é base64!!").is_err());
    }

    /// A chave nunca se imprime.
    #[test]
    fn a_chave_nao_aparece_no_debug() {
        let k = chave();
        let impresso = format!("{k:?}");
        assert!(impresso.contains("oculta"));
        assert!(!impresso.contains('='), "a chave apareceu no Debug");
    }
}
