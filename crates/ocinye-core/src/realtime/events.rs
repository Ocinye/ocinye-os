//! O que atravessa o plano realtime, e com que forma.
//!
//! # Porque isto é tipado
//!
//! Porque um socket sem contrato torna-se uma API onde cada lado adivinha o que
//! o outro manda, e onde acrescentar um campo é uma alteração que nada verifica
//! (ADR-0012 §10). Aqui, um evento novo é uma variante nova — e o compilador
//! obriga quem o lê a decidir o que faz com ele.
//!
//! # Durável e efémero não se parecem por acaso
//!
//! Um `MessageCreated` anuncia uma coisa **que já está no PostgreSQL**: se este
//! evento se perder, a mensagem continua a existir e aparece no `reconnect`. Um
//! `TypingChanged` não anuncia nada durável — se se perder, perdeu-se um gesto,
//! e não há nada a recuperar.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::presence::Presence;

/// Um canal a que se pode subscrever.
///
/// # Porque um tipo e não uma `String`
///
/// Porque cada canal tem de declarar quem o pode ouvir, e a fronteira recusa por
/// omissão. Com uma `String`, subscrever um canal novo seria escrever texto; com
/// isto, é acrescentar uma variante — e a função que autoriza deixa de compilar
/// até alguém decidir a regra (ADR-0012 §4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "canal", rename_all = "snake_case")]
pub enum Channel {
    /// Tudo o que acontece numa conversa. Só para quem participa nela agora.
    Conversation {
        /// A conversa.
        id: Uuid,
    },
    /// O que diz respeito a uma pessoa e a mais ninguém: menções, convites,
    /// contagens por ler. Só para a própria.
    Person {
        /// A pessoa.
        id: Uuid,
    },
}

impl Channel {
    /// A chave por onde este canal viaja no Redis.
    #[must_use]
    pub fn topico(self) -> String {
        match self {
            Self::Conversation { id } => format!("oc:rt:conversa:{id}"),
            Self::Person { id } => format!("oc:rt:pessoa:{id}"),
        }
    }
}

/// O que o Core anuncia.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "tipo", rename_all = "snake_case")]
pub enum ServerEvent {
    /// Uma mensagem nova. Já persistida quando isto sai.
    MessageCreated {
        /// Onde.
        conversation_id: Uuid,
        /// Qual. O corpo vai buscar-se ao Core, com autorização.
        message_id: Uuid,
        /// Quem escreveu.
        author_id: Uuid,
    },
    /// Uma mensagem alterada ou removida pelo autor.
    MessageUpdated {
        /// Onde.
        conversation_id: Uuid,
        /// Qual.
        message_id: Uuid,
    },
    /// Uma reacção acrescentada ou retirada.
    ReactionChanged {
        /// Onde.
        conversation_id: Uuid,
        /// A mensagem que recebeu, ou perdeu, a reacção.
        message_id: Uuid,
    },
    /// Nome, participantes ou papéis de uma conversa mudaram.
    ConversationUpdated {
        /// Qual.
        conversation_id: Uuid,
    },
    /// Alguém entrou, saiu ou foi removido.
    ///
    /// Quem for removido recebe-o e perde o canal no mesmo instante.
    ParticipationChanged {
        /// Onde.
        conversation_id: Uuid,
        /// Quem.
        person_id: Uuid,
        /// Se pertence agora. `false` retira o canal no mesmo instante.
        pertence: bool,
    },
    /// A presença de alguém mudou. Efémero.
    PresenceChanged {
        /// Quem.
        person_id: Uuid,
        /// O estado resolvido, e não os sinais que o produziram.
        estado: Presence,
    },
    /// Quem está a escrever numa conversa. Efémero, e com TTL curto.
    TypingChanged {
        /// Onde.
        conversation_id: Uuid,
        /// Quem.
        person_id: Uuid,
        /// Se está a escrever agora.
        a_escrever: bool,
    },
    /// Alguém avançou a leitura. Durável: a operação já persistiu.
    ReadStateChanged {
        /// Onde.
        conversation_id: Uuid,
        /// Quem.
        person_id: Uuid,
        /// Até onde. Move-se para a frente e nunca para trás.
        lido_ate: chrono::DateTime<chrono::Utc>,
    },
    /// O plano realtime perdeu o Redis, ou recuperou-o.
    ///
    /// # Porque isto é um evento e não silêncio
    ///
    /// Porque uma interface que não recebe nada não sabe distinguir «ninguém
    /// falou» de «deixei de ouvir». Sem este aviso, mostraria uma conversa
    /// parada com ar de normalidade (ADR-0012 §9).
    RealtimeDegraded {
        /// Se o tempo real está a funcionar.
        activo: bool,
    },
}

/// O que o cliente pode pedir pelo socket.
///
/// # O que **não** está aqui
///
/// Enviar uma mensagem. Um comando durável que entrasse por aqui teria de
/// atravessar exactamente a mesma Core Operation que a entrada HTTP atravessa
/// (ADR-0012 §5) — e enquanto isso não for necessário, a porta mais estreita é
/// a que não existe.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "tipo", rename_all = "snake_case")]
pub enum ClientCommand {
    /// Passar a receber um canal. Recusado sem autorização verificada agora.
    Subscribe {
        /// Qual.
        canal: Channel,
    },
    /// Deixar de receber.
    Unsubscribe {
        /// Qual.
        canal: Channel,
    },
    /// «Ainda cá estou.» Renova a presença.
    Heartbeat,
    /// Declarar um estado. Só a própria pessoa declara o seu.
    Declare {
        /// O que a pessoa quer que se veja. `Offline` não é declarável: seria
        /// dizer que não se está enquanto se está.
        estado: Presence,
    },
    /// Começou ou parou de escrever. Efémero, com TTL.
    Typing {
        /// Onde. A pessoa é a do socket, e nunca vem no comando.
        conversation_id: Uuid,
        /// Começou ou parou.
        a_escrever: bool,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn os_topicos_de_canais_diferentes_nunca_colidem() {
        // O mesmo UUID pode ser de uma conversa e de uma pessoa. Se os tópicos
        // colidissem, uma menção privada cairia dentro de uma conversa.
        let id = Uuid::from_u128(1);
        assert_ne!(
            Channel::Conversation { id }.topico(),
            Channel::Person { id }.topico()
        );
    }

    #[test]
    fn um_evento_sobrevive_a_uma_ida_e_volta() {
        let evento = ServerEvent::TypingChanged {
            conversation_id: Uuid::from_u128(7),
            person_id: Uuid::from_u128(9),
            a_escrever: true,
        };
        let texto = serde_json::to_string(&evento).expect("serializar");
        assert!(texto.contains("\"tipo\":\"typing_changed\""));

        let de_volta: ServerEvent = serde_json::from_str(&texto).expect("ler");
        assert!(matches!(
            de_volta,
            ServerEvent::TypingChanged {
                a_escrever: true,
                ..
            }
        ));
    }

    #[test]
    fn um_comando_desconhecido_e_recusado_e_nao_ignorado() {
        // Um socket que aceitasse `{"tipo":"seja_o_que_for"}` seria uma API sem
        // contrato — e o dia em que um cliente antigo mandasse um comando que
        // mudou de forma, ele passaria a não fazer nada, em silêncio.
        let erro = serde_json::from_str::<ClientCommand>(r#"{"tipo":"apagar_tudo"}"#);
        assert!(erro.is_err(), "o socket aceitou um comando que não existe");
    }

    #[test]
    fn nenhum_evento_transporta_o_corpo_de_uma_mensagem() {
        // O corpo vai buscar-se ao Core, com autorização. Pô-lo aqui faria com
        // que quem apanhasse o tráfego do Redis lesse conversas — e o Redis não
        // é onde vive o que é confidencial (ADR-0012 §1).
        let evento = ServerEvent::MessageCreated {
            conversation_id: Uuid::from_u128(1),
            message_id: Uuid::from_u128(2),
            author_id: Uuid::from_u128(3),
        };
        let texto = serde_json::to_string(&evento).expect("serializar");
        assert!(
            !texto.contains("body") && !texto.contains("corpo"),
            "um evento realtime transportou texto de mensagem: {texto}"
        );
    }
}
