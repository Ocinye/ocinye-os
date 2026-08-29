//! O veredicto sobre um objecto lido do armazenamento.
//!
//! # Porque isto vive no Core, e não no comando que o imprime
//!
//! Porque é uma decisão sobre estado institucional — se os bytes que a
//! instituição cita são os bytes que ela guardou — e o Core é quem decide
//! (§3). O comando lê, imprime e escolhe o código de saída; o que conta como
//! igual decide-se aqui, e testa-se sem armazenamento nenhum a correr.

/// O que se concluiu de um objecto que **chegou a ser lido**.
///
/// Não há aqui variante para «em falta» de propósito: um objecto que não se leu
/// não produziu veredicto. Confundir «não observei» com «observei e está mal»
/// é exactamente o erro que o probe de saúde existe para impedir.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Veredicto {
    /// A soma coincide, e o tamanho também.
    Igual,
    /// A base não guardou soma para este objecto.
    ///
    /// Foi lido, e não foi comparado. Contá-lo como igual seria dar por
    /// verificado o que nunca foi medido.
    SemSoma,
    /// O conteúdo é outro.
    OutroConteudo {
        /// A soma que a base regista.
        esperada: String,
        /// A soma do que estava lá.
        obtida: String,
    },
    /// A soma bate certo e o tamanho registado não.
    ///
    /// Improvável, e por isso mesmo digno de nota: quer dizer que a linha e o
    /// objecto se desencontraram por outra via que não o conteúdo.
    OutroTamanho {
        /// Bytes que a base regista.
        esperados: i64,
        /// Bytes que estavam lá.
        obtidos: i64,
    },
}

/// A soma que a base guarda quando nunca mediu nada.
///
/// Escrita como constante para que a comparação seja um facto e não um padrão
/// reconhecido no meio de um `if`.
const SOMA_AUSENTE: &str = "0000000000000000000000000000000000000000000000000000000000000000";

/// Confere um objecto lido contra o que a base regista sobre ele.
#[must_use]
pub fn conferir(soma_registada: &str, tamanho_registado: i64, conteudo: &[u8]) -> Veredicto {
    if soma_registada.is_empty() || soma_registada == SOMA_AUSENTE {
        return Veredicto::SemSoma;
    }

    let soma = crate::storage::sha256_hex(conteudo);
    if soma != soma_registada {
        return Veredicto::OutroConteudo {
            esperada: soma_registada.to_owned(),
            obtida: soma,
        };
    }

    let tamanho = i64::try_from(conteudo.len()).unwrap_or(i64::MAX);
    if tamanho != tamanho_registado {
        return Veredicto::OutroTamanho {
            esperados: tamanho_registado,
            obtidos: tamanho,
        };
    }

    Veredicto::Igual
}

#[cfg(test)]
mod tests {
    use super::*;

    fn soma(dados: &[u8]) -> String {
        crate::storage::sha256_hex(dados)
    }

    /// O controlo positivo: o que está certo passa.
    ///
    /// Sem ele, os testes abaixo passariam com uma função que recusasse tudo.
    #[test]
    fn o_conteudo_certo_e_igual() {
        let dados = b"resultado experimental";
        assert_eq!(
            conferir(&soma(dados), dados.len() as i64, dados),
            Veredicto::Igual
        );
    }

    /// Um byte diferente é outro conteúdo.
    ///
    /// É a razão de o comando existir: a linha sobrevive à migração, o objecto
    /// não, e a instituição continua a citar um dataset que já não é aquele.
    #[test]
    fn um_byte_diferente_e_outro_conteudo() {
        let original = b"resultado experimental";
        let adulterado = b"resultado experimentaL";
        let veredicto = conferir(&soma(original), original.len() as i64, adulterado);
        assert!(
            matches!(veredicto, Veredicto::OutroConteudo { .. }),
            "um objecto adulterado passou por igual: {veredicto:?}"
        );
    }

    /// Uma soma que a base nunca guardou não conta como igual.
    ///
    /// A tentação é tratar zeros como «sem opinião» e seguir. Mas o comando
    /// existe para dizer o que foi verificado, e o que não foi tem de sair
    /// dessa conta.
    #[test]
    fn uma_soma_ausente_nao_e_uma_soma_que_bate() {
        assert_eq!(conferir(SOMA_AUSENTE, 3, b"abc"), Veredicto::SemSoma);
        assert_eq!(conferir("", 3, b"abc"), Veredicto::SemSoma);
    }

    /// Um objecto vazio com soma correcta continua a ser conferível.
    ///
    /// O caso limite que apanharia uma implementação que confundisse «sem
    /// bytes» com «sem soma».
    #[test]
    fn um_objecto_vazio_com_soma_certa_e_igual() {
        assert_eq!(conferir(&soma(b""), 0, b""), Veredicto::Igual);
    }

    /// O tamanho registado também é uma afirmação.
    #[test]
    fn um_tamanho_que_nao_bate_e_dito() {
        let dados = b"quatro";
        let veredicto = conferir(&soma(dados), 999, dados);
        assert_eq!(
            veredicto,
            Veredicto::OutroTamanho {
                esperados: 999,
                obtidos: 6
            }
        );
    }
}
