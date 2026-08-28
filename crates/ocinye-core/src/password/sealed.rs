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
use rand::rand_core::UnwrapErr;
use rand::rngs::SysRng;
use rand::RngExt as _;

use crate::{CoreError, CoreResult};

/// Quantos bytes tem a chave. Duzentos e cinquenta e seis bits.
const CHAVE_BYTES: usize = 32;

/// Quantos bytes tem o nonce do `ChaCha20-Poly1305`.
const NONCE_BYTES: usize = 12;

/// A chave com que os segredos desta instalação são fechados.
///
/// Vive fora da base de dados, na configuração. Quem obtiver um despejo da base
/// obtém criptogramas; quem obtiver a chave sem a base não obtém nada.
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
                CoreError::Validation(
                    "A chave de cifra do correio não está em base64 válido.".to_owned(),
                )
            })?;

        let exacta: [u8; CHAVE_BYTES] = bytes.as_slice().try_into().map_err(|_| {
            CoreError::Validation(format!(
                "A chave de cifra do correio tem de ter {CHAVE_BYTES} bytes; esta tem {}.",
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
pub fn seal(chave: &SealingKey, claro: &str) -> CoreResult<Sealed> {
    // O mesmo gerador que gera as credenciais temporárias do Ocinye: do sistema,
    // e não um `thread_rng` que se possa vir a semear de forma previsível.
    // `UnwrapErr` dá a face infalível do gerador do sistema, tal como em
    // `password::generate`: um nonce que falhasse a gerar não teria por onde
    // continuar, e tratar isso como erro recuperável seria fingir uma escolha.
    let nonce_bytes: [u8; NONCE_BYTES] = UnwrapErr(SysRng).random();

    let cifra = ChaCha20Poly1305::new(&chave.0);
    let nonce = Nonce::from(nonce_bytes);
    let ciphertext = cifra
        .encrypt(&nonce, claro.as_bytes())
        .map_err(|_| CoreError::Internal("a cifra do segredo falhou".into()))?;

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
pub fn open(chave: &SealingKey, fechado: &Sealed) -> CoreResult<String> {
    if fechado.nonce.len() != NONCE_BYTES {
        return Err(CoreError::Internal("segredo com nonce inválido".into()));
    }

    let cifra = ChaCha20Poly1305::new(&chave.0);
    let nonce_bytes: [u8; NONCE_BYTES] = fechado.nonce.as_slice().try_into().map_err(|_| {
        CoreError::Validation("A credencial guardada não tem um nonce válido.".to_owned())
    })?;
    let nonce = Nonce::from(nonce_bytes);
    let claro = cifra
        .decrypt(&nonce, fechado.ciphertext.as_ref())
        .map_err(|_| CoreError::Internal("o segredo não pôde ser aberto".into()))?;

    String::from_utf8(claro).map_err(|_| CoreError::Internal("segredo ilegível".into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chave() -> SealingKey {
        SealingKey::from_base64(&SealingKey::generate()).expect("chave")
    }

    /// O que se fecha, abre-se.
    #[test]
    fn um_segredo_fechado_volta_a_ler_se() {
        let k = chave();
        let fechado = seal(&k, "senha-do-imap").expect("cifrar");
        assert_eq!(open(&k, &fechado).expect("decifrar"), "senha-do-imap");
    }

    /// E não se lê com outra chave.
    #[test]
    fn outra_chave_nao_abre() {
        let fechado = seal(&chave(), "senha-do-imap").expect("cifrar");
        assert!(
            open(&chave(), &fechado).is_err(),
            "um segredo abriu-se com uma chave que não o fechou"
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
        let mut fechado = seal(&k, "senha-do-imap").expect("cifrar");
        fechado.ciphertext[0] ^= 0x01;
        assert!(
            open(&k, &fechado).is_err(),
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
        let a = seal(&k, "a-mesma").expect("cifrar");
        let b = seal(&k, "a-mesma").expect("cifrar");
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
