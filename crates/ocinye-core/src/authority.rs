//! A fronteira onde a autoridade se estabelece, imediatamente antes do efeito.
//!
//! # Identidade persiste; autoridade volta a estabelecer-se
//!
//! Um [`Principal`] é um **retrato** da autoridade de alguém no instante em que
//! foi construído: conta activa, papéis, pertenças, concessões. Nada nele diz
//! quando foi tirado, e nada o invalida quando o mundo muda por baixo.
//!
//! Um plano guarda quem o pediu. Isso é identidade, e é durável — a pessoa
//! continua a ser a mesma. O que ela pode fazer não é durável, e é por isso que
//! não se guarda.
//!
//! ```text
//! ActorRef            identidade durável: quem
//!    ↓  resolve
//! CurrentAuthority    autoridade corrente: o que pode agora
//!    ↓
//! autorizar
//!    ↓
//! efeito
//! ```
//!
//! # Porque isto não vive dentro de cada módulo
//!
//! Porque uma convenção por módulo é uma convenção que um módulo novo não
//! herda. A pergunta «este principal ainda vale?» tem de ter um sítio, e este é
//! o sítio. Um `reload` espalhado por vinte operações seria vinte oportunidades
//! de alguém escrever a vigésima primeira sem ele.
//!
//! # O que isto **não** é
//!
//! Não é um segundo motor de política. Resolve factos; quem decide continua a
//! ser `ocinye_domain::policy`.

use sqlx::PgPool;
use uuid::Uuid;

use crate::error::{CoreError, CoreResult};
use crate::modules::identity;
use ocinye_domain::Principal;

/// Quem, de forma durável.
///
/// O que sobrevive entre o planeamento e a execução. Não traz permissões, e é
/// esse o ponto: não há nada aqui que possa envelhecer sem se notar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActorRef {
    /// A pessoa.
    pub person_id: Uuid,
    /// A organização em que age.
    pub organisation_id: Uuid,
}

impl ActorRef {
    /// A identidade de um principal, sem a sua autoridade.
    ///
    /// Deitar fora as permissões é o objectivo: o que sobra é o suficiente para
    /// voltar a perguntar, e insuficiente para responder.
    #[must_use]
    pub const fn of(principal: &Principal) -> Self {
        Self {
            person_id: principal.person_id,
            organisation_id: principal.organisation_id,
        }
    }
}

/// Autoridade estabelecida agora, à fonte canónica.
///
/// # Porque é um tipo e não um `Principal` qualquer
///
/// Porque assim uma operação sensível pode **exigir** que a autoridade tenha
/// sido resolvida, e não apenas esperar que quem chama se tenha lembrado. Só
/// [`resolve`] o constrói; não há outro caminho, e o campo é privado.
///
/// Não substitui a autorização do domínio. Diz «isto é quem a pessoa é agora»,
/// e a política continua a decidir o que isso permite.
#[derive(Debug, Clone)]
pub struct CurrentAuthority(Principal);

impl CurrentAuthority {
    /// O principal corrente, para a política decidir sobre ele.
    #[must_use]
    pub const fn principal(&self) -> &Principal {
        &self.0
    }
}

/// Estabelece a autoridade corrente de um actor.
///
/// # Fecha em caso de dúvida
///
/// Se a pessoa não existe, se a consulta falha, ou se a conta deixou de estar
/// activa, isto devolve erro — e quem chama não executa. Não há aqui um caminho
/// em que «não consegui saber» se traduza em «então deixa passar».
///
/// A conta inactiva é recusada aqui e não deixada à política porque é a
/// pergunta mais barata e a mais absoluta: uma conta suspensa não age, e não
/// interessa em que unidade estava.
///
/// # Errors
///
/// Devolve [`CoreError::PermissionDenied`] quando o actor já não existe ou a
/// conta não está activa, e o erro da consulta quando a fonte não responde.
pub async fn resolve(pool: &PgPool, actor: ActorRef) -> CoreResult<CurrentAuthority> {
    let Some(person) = identity::person_by_id(pool, actor.person_id).await? else {
        return Err(CoreError::PermissionDenied(
            "Esta conta já não existe.".to_owned(),
        ));
    };

    let principal = identity::principal_for_person(pool, &person).await?;

    if !principal.is_active {
        return Err(CoreError::PermissionDenied(
            "Esta conta não está activa.".to_owned(),
        ));
    }

    // A organização não muda debaixo de um plano. Se mudasse, o plano estaria a
    // agir noutra instituição com a identidade desta.
    if principal.organisation_id != actor.organisation_id {
        return Err(CoreError::PermissionDenied(
            "Esta conta já não pertence a esta organização.".to_owned(),
        ));
    }

    Ok(CurrentAuthority(principal))
}
