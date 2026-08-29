//! O contrato de embeddings.
//!
//! # Porque é um contrato próprio, e não uma opção da inferência
//!
//! Porque «se consegue conversar, consegue produzir embeddings» é uma suposição
//! e não um facto. São modelos diferentes, com dimensões diferentes, limites
//! diferentes e — o que mais importa aqui — **classes de confiança diferentes**:
//! uma instalação pode ter um modelo de conversa local e um serviço de
//! embeddings externo, ou o contrário, e a política sobre o que pode sair da
//! instituição tem de os distinguir.
//!
//! # A propriedade que este módulo protege
//!
//! > **Um conjunto nunca mistura embeddings produzidos por identidades de
//! > modelo ou perfis diferentes.**
//!
//! Compatibilidade semântica não é «o mesmo tamanho de vector». Dois modelos com
//! 1024 dimensões produzem espaços diferentes, e compará-los dá números que
//! parecem distâncias e não são.

use std::time::Duration;

use async_trait::async_trait;

/// De onde o provider é, do ponto de vista da instituição.
///
/// # Porque isto não é um detalhe de configuração
///
/// Porque decide se conteúdo institucional pode sair. Um provider sob controlo
/// da Ocinye processa segundo a autorização normal; um provider externo recebe
/// menos, e por omissão não recebe nada que não seja explicitamente autorizado
/// no deployment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Locality {
    /// Corre sob controlo da Ocinye.
    OcinyeControlled,
    /// Um serviço de terceiros.
    External,
}

impl Locality {
    /// Como fica guardado.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::OcinyeControlled => "ocinye_controlled",
            Self::External => "external",
        }
    }

    /// Lê o que está guardado.
    ///
    /// Um valor que não se reconheça é `External`, e não o contrário: uma
    /// classe de confiança ilegível não pode ser a mais permissiva.
    #[must_use]
    pub fn parse(raw: &str) -> Self {
        match raw {
            "ocinye_controlled" => Self::OcinyeControlled,
            _ => Self::External,
        }
    }
}

/// Quem produziu um vector, e com que modelo.
///
/// # Porque a revisão viaja com o nome
///
/// Porque «text-embedding-3-large» não é uma identidade: é uma família que muda
/// por baixo. Sem revisão, um conjunto produzido em Março e outro em Setembro
/// dizem chamar-se o mesmo e não são comparáveis — e a incompatibilidade só
/// aparece como resultados subtilmente errados, que é a pior maneira de
/// aparecer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmbeddingIdentity {
    /// O adaptador.
    pub provider: String,
    /// O modelo.
    pub model: String,
    /// A revisão do modelo.
    pub revision: String,
    /// Quantas dimensões produz.
    pub dimensions: i32,
    /// De onde é, para a política decidir o que lhe pode ser enviado.
    pub locality: Locality,
}

impl EmbeddingIdentity {
    /// Se dois conjuntos podem ser comparados.
    ///
    /// Tudo tem de coincidir, e não só a dimensão.
    #[must_use]
    pub fn compatible_with(&self, other: &Self) -> bool {
        self.provider == other.provider
            && self.model == other.model
            && self.revision == other.revision
            && self.dimensions == other.dimensions
    }
}

/// O que correu mal ao pedir embeddings.
#[derive(Debug, thiserror::Error)]
pub enum EmbeddingError {
    /// O provider não respondeu a tempo.
    #[error("the embedding provider did not answer within {0:?}")]
    Timeout(Duration),
    /// O provider recusou, e disse porquê.
    #[error("the embedding provider refused: {0}")]
    Refused(String),
    /// A resposta não tem a forma que o contrato exige.
    #[error("the embedding provider answered with {0}")]
    Malformed(String),
    /// O provider devolveu vectores de uma dimensão que não é a que declarou.
    ///
    /// Separado de [`Self::Malformed`] de propósito: é o erro que apanha um
    /// conjunto a ser envenenado com vectores incomparáveis, e quem o lê num
    /// registo tem de perceber imediatamente o que aconteceu.
    #[error("expected vectors of {expected} dimensions, got {actual}")]
    WrongDimensions {
        /// O que a identidade declara.
        expected: i32,
        /// O que chegou.
        actual: i32,
    },
    /// Pediu-se mais do que o provider aceita de uma vez.
    #[error("the batch of {0} exceeds what this provider accepts")]
    BatchTooLarge(usize),
}

/// O resultado de um pedido de embeddings.
pub type EmbeddingResult<T> = Result<T, EmbeddingError>;

/// Um serviço que transforma texto em vectores.
#[async_trait]
pub trait EmbeddingProvider: Send + Sync {
    /// Quem é, e com que modelo.
    fn identity(&self) -> EmbeddingIdentity;

    /// Quantos textos aceita de uma vez.
    fn max_batch(&self) -> usize;

    /// O maior texto que aceita, em caracteres.
    fn max_input_chars(&self) -> usize;

    /// Quanto tempo se espera.
    fn deadline(&self) -> Duration;

    /// Produzir os vectores.
    ///
    /// # Errors
    ///
    /// Devolve [`EmbeddingError`] a descrever o que falhou.
    async fn embed(&self, texts: &[String]) -> EmbeddingResult<Vec<Vec<f32>>>;
}

/// Chama um provider e verifica o que ele promete.
///
/// # Porque o Core verifica o seu próprio contrato
///
/// Pela mesma razão da inferência: um provider que cumpre o contrato é
/// exactamente aquele para quem nada disto é preciso. A dimensão é verificada
/// **aqui**, do lado do Core, porque um vector com o tamanho errado dentro de um
/// conjunto não dá erro nenhum — dá resultados que parecem certos.
///
/// # Errors
///
/// Devolve [`EmbeddingError`] quando o lote é grande de mais, quando o provider
/// falha, ou quando o que ele devolve não corresponde ao que declarou.
pub async fn embed_checked(
    provider: &dyn EmbeddingProvider,
    texts: &[String],
) -> EmbeddingResult<Vec<Vec<f32>>> {
    if texts.len() > provider.max_batch() {
        return Err(EmbeddingError::BatchTooLarge(texts.len()));
    }

    let identidade = provider.identity();
    let limite = provider.max_input_chars();

    // Cortado aqui, e não deixado ao provider: um provider que trunca em
    // silêncio produz um vector de metade de um texto e diz que é do texto.
    let recortados: Vec<String> = texts
        .iter()
        .map(|texto| texto.chars().take(limite).collect())
        .collect();

    let vectores = tokio::time::timeout(provider.deadline(), provider.embed(&recortados))
        .await
        .map_err(|_| EmbeddingError::Timeout(provider.deadline()))??;

    if vectores.len() != texts.len() {
        return Err(EmbeddingError::Malformed(format!(
            "{} vectores para {} textos",
            vectores.len(),
            texts.len()
        )));
    }

    for vector in &vectores {
        let dimensao = i32::try_from(vector.len()).unwrap_or(i32::MAX);
        if dimensao != identidade.dimensions {
            return Err(EmbeddingError::WrongDimensions {
                expected: identidade.dimensions,
                actual: dimensao,
            });
        }
    }

    Ok(vectores)
}

/// Um provider determinístico, para exercitar o contrato.
///
/// # O que ele é, e o que não é
///
/// É uma implementação do **mesmo** contrato que uma real: passa por
/// `embed_checked`, declara identidade, dimensões, lotes e prazo, e o Core
/// verifica-o como verificaria qualquer outro. É isso que o torna útil — uma
/// prova que não atravesse o contrato não prova o contrato.
///
/// **Não é um modelo da Ocinye.** Não produz significado: produz um vector
/// estável a partir do texto, para que a mesma frase caia sempre no mesmo sítio
/// e frases que partilham palavras caiam perto. Chega para provar que a
/// recuperação semântica funciona de ponta a ponta; não chega para nada mais, e
/// a identidade que declara di-lo em voz alta.
pub struct DeterministicEmbeddings {
    /// Quantas dimensões produz.
    pub dimensions: i32,
    /// A revisão declarada, para se poder simular uma troca de modelo.
    pub revision: String,
    /// De onde diz ser.
    pub locality: Locality,
}

impl Default for DeterministicEmbeddings {
    fn default() -> Self {
        Self {
            dimensions: 64,
            revision: "1".to_owned(),
            locality: Locality::OcinyeControlled,
        }
    }
}

#[async_trait]
impl EmbeddingProvider for DeterministicEmbeddings {
    fn identity(&self) -> EmbeddingIdentity {
        EmbeddingIdentity {
            provider: "deterministic-fixture".to_owned(),
            // Dito no nome, e não só na documentação: se isto aparecer num
            // registo de proveniência, quem o lê tem de perceber imediatamente
            // que não é um modelo institucional.
            model: "not-a-model".to_owned(),
            revision: self.revision.clone(),
            dimensions: self.dimensions,
            locality: self.locality,
        }
    }

    fn max_batch(&self) -> usize {
        16
    }

    fn max_input_chars(&self) -> usize {
        4_000
    }

    fn deadline(&self) -> Duration {
        Duration::from_secs(5)
    }

    async fn embed(&self, texts: &[String]) -> EmbeddingResult<Vec<Vec<f32>>> {
        let dimensoes = usize::try_from(self.dimensions).unwrap_or(64);
        Ok(texts
            .iter()
            .map(|texto| {
                // Saco de palavras sobre um espaço fixo, normalizado. Duas
                // frases que partilhem palavras ficam próximas; a mesma frase
                // cai sempre no mesmo sítio.
                let mut vector = vec![0.0_f32; dimensoes];
                for palavra in texto.to_lowercase().split_whitespace() {
                    let mut soma: u64 = 1469598103934665603;
                    for byte in palavra.bytes() {
                        soma ^= u64::from(byte);
                        soma = soma.wrapping_mul(1099511628211);
                    }
                    let posicao = usize::try_from(soma % dimensoes as u64).unwrap_or(0);
                    vector[posicao] += 1.0;
                }
                let norma: f32 = vector.iter().map(|v| v * v).sum::<f32>().sqrt();
                if norma > 0.0 {
                    for valor in &mut vector {
                        *valor /= norma;
                    }
                }
                vector
            })
            .collect())
    }
}

/// O provider que esta instalação configurou, se algum.
///
/// # Porque devolve `None` em silêncio
///
/// Porque «esta instalação não tem embeddings» é um estado normal e não um
/// erro. A pesquisa lexical não depende disto, e declarar indisponibilidade é
/// mais honesto do que falhar a arrancar.
///
/// Um nome que não se reconheça também dá `None`, e não um provider por
/// omissão: adivinhar aqui seria escolher por alguém qual o modelo que descreve
/// a memória da instituição.
#[must_use]
pub fn from_config(config: &crate::config::AiConfig) -> Option<Box<dyn EmbeddingProvider>> {
    match config.embedding_provider.as_str() {
        "deterministic" => Some(Box::new(DeterministicEmbeddings::default())),
        _ => None,
    }
}
