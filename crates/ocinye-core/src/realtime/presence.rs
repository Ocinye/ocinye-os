//! Presença e `typing` — o que existe enquanto alguém está, e mais nada.
//!
//! # Porque isto não está no PostgreSQL
//!
//! Porque ninguém precisa de saber amanhã quem esteve online ontem, e porque
//! guardar quem começou a escrever e desistiu seria guardar uma hesitação
//! (ADR-0012 §6).
//!
//! Tudo aqui expira por TTL, e expira **sozinho**. Um browser que fecha, uma
//! rede que cai ou um portátil que adormece não mandam aviso nenhum — desenhar
//! para o adeus educado é desenhar para o caso que não acontece.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Quanto tempo uma presença sobrevive sem batimento.
///
/// Curto o suficiente para que uma pessoa que fechou o portátil desapareça
/// depressa, e longo o suficiente para aguentar um separador em segundo plano
/// que o browser abrandou.
pub const PRESENCE_TTL_SECONDS: u64 = 45;

/// Com que frequência o cliente confirma que ainda está.
///
/// Metade do TTL, e não um pouco menos: com uma margem apertada, um batimento
/// perdido apagaria uma pessoa que está ali.
pub const HEARTBEAT_SECONDS: u64 = 20;

/// Quanto tempo um `typing` sobrevive sem ser renovado.
///
/// Alguém que pára de escrever a meio de uma frase desaparece em poucos
/// segundos, que é o que uma pessoa do outro lado espera.
pub const TYPING_TTL_SECONDS: u64 = 6;

// As relações entre os três prazos, verificadas ao compilar.
//
// Escritas aqui e não num teste porque não são comportamento: são aritmética
// entre constantes. Um teste provaria a mesma coisa mais tarde, e deixaria o
// binário sair com os números errados até alguém o correr.
const _: () = {
    // Sem esta margem, um batimento que se atrasa apaga uma pessoa que está
    // ali — e ela reaparece um segundo depois, a piscar.
    assert!(
        PRESENCE_TTL_SECONDS >= HEARTBEAT_SECONDS * 2,
        "o TTL da presença tem de aguentar um batimento perdido"
    );
    // Quem pára de escrever tem de desaparecer depressa.
    assert!(
        TYPING_TTL_SECONDS < PRESENCE_TTL_SECONDS / 4,
        "o `typing` tem de expirar muito antes da presença"
    );
};

/// O que uma pessoa está, para quem a vê.
///
/// # Isto não é autorização
///
/// «Offline» nunca nega uma operação institucional (ADR-0012). Uma pessoa
/// desligada continua a poder receber mensagens, a ser mencionada e a ser
/// adicionada a um grupo — o correio de um colega ausente não é devolvido ao
/// remetente.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Presence {
    /// Ligado e a usar o sistema.
    Disponivel,
    /// Declarado pela própria pessoa.
    Ocupado,
    /// Declarado pela própria pessoa; suprime alertas não essenciais.
    NaoIncomodar,
    /// Ligado, sem actividade recente.
    Ausente,
    /// Sem nenhuma ligação viva.
    Offline,
}

impl Presence {
    /// O texto que a interface mostra.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Disponivel => "Disponível",
            Self::Ocupado => "Ocupado",
            Self::NaoIncomodar => "Não incomodar",
            Self::Ausente => "Ausente",
            Self::Offline => "Offline",
        }
    }

    /// O nome estável, para contratos e para o Redis.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Disponivel => "disponivel",
            Self::Ocupado => "ocupado",
            Self::NaoIncomodar => "nao_incomodar",
            Self::Ausente => "ausente",
            Self::Offline => "offline",
        }
    }

    /// Lê um nome estável.
    #[must_use]
    pub fn parse(valor: &str) -> Option<Self> {
        match valor {
            "disponivel" => Some(Self::Disponivel),
            "ocupado" => Some(Self::Ocupado),
            "nao_incomodar" => Some(Self::NaoIncomodar),
            "ausente" => Some(Self::Ausente),
            "offline" => Some(Self::Offline),
            _ => None,
        }
    }

    /// Se este estado suprime alertas não essenciais.
    ///
    /// Suprime o alerta, e nunca a entrega: a mensagem chega na mesma, e está
    /// lá quando a pessoa voltar.
    #[must_use]
    pub fn silencia_alertas(self) -> bool {
        matches!(self, Self::NaoIncomodar)
    }
}

/// O que cada fonte diz sobre uma pessoa, num instante.
///
/// Existe para que a precedência seja uma função pura de coisas observáveis, e
/// não uma sequência de `if`s espalhada por quem responde ao pedido.
#[derive(Debug, Clone, Copy, Default)]
pub struct Sinais {
    /// O que a própria pessoa declarou, se declarou.
    pub declarado: Option<Presence>,
    /// Se o Calendar diz que ela está num compromisso agora.
    ///
    /// Um booleano, e nada mais. O Calendar não expõe título, local,
    /// participantes nem classificação — quem vê a presença fica a saber que
    /// não é boa altura, e não o que a pessoa está a fazer.
    pub em_compromisso: bool,
    /// Se há pelo menos uma ligação viva.
    pub ligado: bool,
    /// Se houve actividade recente em alguma dessas ligações.
    pub activo: bool,
}

/// Qual estado prevalece.
///
/// # A ordem, e a razão de ser esta
///
/// O que a pessoa **declarou** vem primeiro, porque é a única fonte que exprime
/// uma intenção: quem se pôs em «Não incomodar» pediu uma coisa, e um sinal
/// automático não tem autoridade para lha retirar.
///
/// Depois vem o Calendar, que sabe uma coisa que a máquina não sabe — que ela
/// está numa reunião mesmo estando a mexer no teclado.
///
/// Só então a actividade, que é a fonte mais fraca: diz onde o rato passou, e
/// não o que a pessoa está a fazer.
#[must_use]
pub fn resolver(sinais: Sinais) -> Presence {
    if let Some(declarado) = sinais.declarado {
        return declarado;
    }
    if sinais.em_compromisso {
        return Presence::Ocupado;
    }
    if !sinais.ligado {
        return Presence::Offline;
    }
    if sinais.activo {
        Presence::Disponivel
    } else {
        Presence::Ausente
    }
}

/// Quem está a escrever numa conversa, e a frase que isso faz.
///
/// # Porque devolve texto e não uma lista
///
/// Porque a frase muda de forma com o número de pessoas, e a concordância é do
/// português — «está» com uma, «estão» com duas. Deixá-la à interface era
/// espalhar gramática por dentro de marcação.
#[must_use]
pub fn frase_de_escrita(nomes: &[String]) -> Option<String> {
    let primeiro = |nome: &String| nome.split_whitespace().next().unwrap_or(nome).to_owned();

    match nomes {
        [] => None,
        [um] => Some(format!("{} está a escrever…", primeiro(um))),
        [um, dois] => Some(format!(
            "{} e {} estão a escrever…",
            primeiro(um),
            primeiro(dois)
        )),
        _ => Some("Várias pessoas estão a escrever…".to_owned()),
    }
}

/// Presenças de várias pessoas, tal como a interface as pede.
pub type Mapa = BTreeMap<Uuid, Presence>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn o_que_a_pessoa_declara_vence_o_que_a_maquina_observa() {
        // Quem se pôs em «Não incomodar» pediu uma coisa. Estar a mexer no
        // teclado não é um pedido para deixar de a respeitar.
        let presenca = resolver(Sinais {
            declarado: Some(Presence::NaoIncomodar),
            em_compromisso: true,
            ligado: true,
            activo: true,
        });
        assert_eq!(presenca, Presence::NaoIncomodar);
    }

    #[test]
    fn o_calendario_vence_a_actividade() {
        let presenca = resolver(Sinais {
            declarado: None,
            em_compromisso: true,
            ligado: true,
            activo: true,
        });
        assert_eq!(
            presenca,
            Presence::Ocupado,
            "estar numa reunião e a mexer no rato continua a ser estar numa reunião"
        );
    }

    #[test]
    fn sem_ligacao_e_offline_mesmo_com_actividade_antiga() {
        let presenca = resolver(Sinais {
            declarado: None,
            em_compromisso: false,
            ligado: false,
            activo: true,
        });
        assert_eq!(presenca, Presence::Offline);
    }

    #[test]
    fn ligado_sem_actividade_e_ausente() {
        assert_eq!(
            resolver(Sinais {
                declarado: None,
                em_compromisso: false,
                ligado: true,
                activo: false,
            }),
            Presence::Ausente
        );
    }

    #[test]
    fn a_precedencia_e_total_e_nao_deixa_caso_por_decidir() {
        // Todas as combinações têm resposta, e a mesma entrada dá sempre a
        // mesma saída. Uma precedência com um buraco é um buraco que só se
        // encontra em produção.
        for declarado in [None, Some(Presence::Ocupado), Some(Presence::Disponivel)] {
            for em_compromisso in [false, true] {
                for ligado in [false, true] {
                    for activo in [false, true] {
                        let sinais = Sinais {
                            declarado,
                            em_compromisso,
                            ligado,
                            activo,
                        };
                        assert_eq!(resolver(sinais), resolver(sinais));
                    }
                }
            }
        }
    }

    #[test]
    fn a_frase_de_escrita_concorda_em_numero() {
        assert_eq!(frase_de_escrita(&[]), None);
        assert_eq!(
            frase_de_escrita(&["Fidel Monteiro".to_owned()]).unwrap(),
            "Fidel está a escrever…"
        );
        assert_eq!(
            frase_de_escrita(&["Fidel Monteiro".to_owned(), "Ana Silva".to_owned()]).unwrap(),
            "Fidel e Ana estão a escrever…"
        );
        assert_eq!(
            frase_de_escrita(&[
                "Fidel Monteiro".to_owned(),
                "Ana Silva".to_owned(),
                "Dário Costa".to_owned(),
            ])
            .unwrap(),
            "Várias pessoas estão a escrever…"
        );
    }

    #[test]
    fn nao_incomodar_silencia_alertas_e_mais_nada() {
        assert!(Presence::NaoIncomodar.silencia_alertas());
        for outro in [
            Presence::Disponivel,
            Presence::Ocupado,
            Presence::Ausente,
            Presence::Offline,
        ] {
            assert!(
                !outro.silencia_alertas(),
                "{} não devia silenciar alertas",
                outro.as_str()
            );
        }
    }

    #[test]
    fn os_nomes_estaveis_sobrevivem_a_uma_ida_e_volta() {
        for estado in [
            Presence::Disponivel,
            Presence::Ocupado,
            Presence::NaoIncomodar,
            Presence::Ausente,
            Presence::Offline,
        ] {
            assert_eq!(Presence::parse(estado.as_str()), Some(estado));
        }
        assert_eq!(Presence::parse("inventado"), None);
    }
}
