//! O que uma revisão de bibliografia BibTeX diz.
//!
//! # Porque isto é um contrato e não a saída do componente
//!
//! Quem executa a leitura é um componente WebAssembly isolado, e a forma como
//! ele fala é assunto entre ele e o Capability Runtime. O que atravessa a
//! fronteira institucional — para a interface, para um agente, para a API — é
//! isto: tipos do Ocinye OS, sem uma palavra de Wasmtime, de WASI ou do formato
//! interno do componente.
//!
//! Se um dia o componente for substituído por outro, ou por código nativo, esta
//! superfície não muda.
//!
//! # O que uma revisão afirma, e o que não afirma
//!
//! Afirma que a **estrutura** foi lida: entradas com tipo, chave e campos, e
//! quais não se conseguiram ler. Afirma uma **forma canónica** para o que se
//! leu.
//!
//! Não afirma que a referência existe, que o DOI resolve, que o autor escreveu
//! aquilo ou que o ano está certo. Nada disso se sabe sem consultar fontes
//! externas, e esta operação não consulta nenhuma — é a mesma offline.

use serde::{Deserialize, Serialize};

/// Quanto BibTeX se aceita de uma vez.
///
/// # Porque o limite vive aqui
///
/// Porque há três sítios a querer conhecê-lo — a interface, que recusa antes de
/// gastar um pedido; o transporte, que recusa antes de gastar o Core; e o Core,
/// que é quem decide de verdade — e três constantes acabariam por discordar. A
/// camada de fora pode recusar mais cedo; o número é este.
///
/// Duzentos e cinquenta mil caracteres são cerca de mil e quinhentas
/// referências, muito acima de qualquer bibliografia que alguém cole numa
/// caixa de texto, e muito abaixo do que faria o componente sofrer.
pub const MAX_BIBTEX_BYTES: usize = 250_000;

/// Quantas entradas uma revisão devolve.
///
/// Uma entrada pequena não pode produzir uma resposta ilimitada. O limite de
/// entrada já o impede na prática; este é o segundo fecho, do lado da saída,
/// para o caso de um componente futuro ser mais generoso do que este.
pub const MAX_ENTRIES: usize = 2_000;

/// Uma entrada bibliográfica que se conseguiu ler.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BibliographyEntry {
    /// O tipo de entrada, em minúsculas: `article`, `book`, `inproceedings`.
    pub entry_type: String,
    /// A chave de citação.
    pub citation_key: String,
    /// O título, quando a entrada o traz.
    pub title: Option<String>,
    /// Os autores, separados como o BibTeX os separa.
    pub authors: Vec<String>,
    /// O ano, quando é um número.
    pub year: Option<i32>,
    /// A revista, as actas ou o livro que a contém.
    pub container_title: Option<String>,
    /// O DOI tal como foi escrito. **Não verificado.**
    pub doi: Option<String>,
}

/// O resultado de rever uma bibliografia.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BibliographyReview {
    /// As entradas lidas.
    pub entries: Vec<BibliographyEntry>,
    /// O princípio de cada entrada que não se conseguiu ler.
    ///
    /// Um excerto, e nunca o bloco inteiro: um diagnóstico não pode tornar-se
    /// uma segunda cópia do que se escreveu.
    pub unreadable: Vec<String>,
    /// O que se leu, escrito numa forma canónica.
    ///
    /// Só o que se leu. Uma entrada em [`Self::unreadable`] não aparece aqui —
    /// normalizar é dar forma ao que se entendeu, e inventar uma forma para o
    /// que não se entendeu seria apresentar como arrumado aquilo que ninguém
    /// leu.
    pub normalized: String,
}

impl BibliographyReview {
    /// Se tudo o que estava escrito foi lido.
    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.unreadable.is_empty()
    }

    /// Quantas entradas foram lidas.
    #[must_use]
    pub fn read_count(&self) -> usize {
        self.entries.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn revisao(entradas: usize, ilegiveis: usize) -> BibliographyReview {
        BibliographyReview {
            entries: (0..entradas)
                .map(|i| BibliographyEntry {
                    entry_type: "article".to_owned(),
                    citation_key: format!("k{i}"),
                    title: None,
                    authors: Vec::new(),
                    year: None,
                    container_title: None,
                    doi: None,
                })
                .collect(),
            unreadable: (0..ilegiveis)
                .map(|i| format!("@misc{{partido{i}"))
                .collect(),
            normalized: String::new(),
        }
    }

    #[test]
    fn uma_revisao_sem_ilegiveis_esta_completa() {
        assert!(revisao(3, 0).is_complete());
        assert!(!revisao(3, 1).is_complete());
    }

    /// Uma revisão vazia está completa, e é preciso que esteja.
    ///
    /// Bibliografia vazia não tem nada por ler. Dizer que está incompleta faria
    /// a interface mostrar um problema onde não há nenhum.
    #[test]
    fn uma_bibliografia_vazia_nao_tem_nada_por_ler() {
        let vazia = revisao(0, 0);
        assert!(vazia.is_complete());
        assert_eq!(vazia.read_count(), 0);
    }

    /// Os limites existem, e são um número que se pode ler.
    ///
    /// Em `const`: o compilador confere-o, e um limite que alguém baixasse até
    /// deixar de servir para nada passa a não compilar em vez de passar a não
    /// recusar nada.
    const _LIMITES_UTEIS: () = {
        assert!(MAX_BIBTEX_BYTES > 10_000);
        assert!(MAX_ENTRIES > 100);
    };
}
