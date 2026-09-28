//! Textos da base da interface (documento e estados comuns). Claude Design.
//! Acrescentado a `GROUPS` pelo `apply.sh`.

use crate::i18n::Entry;

/// Base: documento, salto para o conteúdo, estados comuns.
pub const UI_BASE: &[Entry] = crate::catalogo! {
    "doc.title": { pt: "{page} · Ocinye OS", en: "{page} · Ocinye OS", fr: "{page} · Ocinye OS" },
    "doc.skip": { pt: "Saltar para o conteúdo", en: "Skip to content", fr: "Aller au contenu" },
    "state.core_error": { pt: "Não foi possível carregar. Referência {ref}.", en: "Could not load. Reference {ref}.", fr: "Chargement impossible. Référence {ref}." },
    "state.denied": { pt: "Não tem acesso a este conteúdo.", en: "You do not have access to this content.", fr: "Vous n’avez pas accès à ce contenu." },
    "state.empty": { pt: "Ainda não há nada aqui.", en: "Nothing here yet.", fr: "Rien ici pour l’instant." },
    "state.loading": { pt: "A carregar…", en: "Loading…", fr: "Chargement…" },
    "state.inactive": { pt: "Esta aplicação não está activa nesta Instância.", en: "This application is not active on this Instance.", fr: "Cette application n’est pas active sur cette instance." },
    "state.unavailable": { pt: "Indisponível nesta Instância.", en: "Unavailable on this Instance.", fr: "Indisponible sur cette instance." },
    "time.now": { pt: "agora", en: "now", fr: "à l’instant" },
    "time.minutes.one": { pt: "há {count} min", en: "{count} min ago", fr: "il y a {count} min" },
    "time.minutes.other": { pt: "há {count} min", en: "{count} min ago", fr: "il y a {count} min" },
    "time.hours.one": { pt: "há {count} h", en: "{count} h ago", fr: "il y a {count} h" },
    "time.hours.other": { pt: "há {count} h", en: "{count} h ago", fr: "il y a {count} h" },
    "time.yesterday": { pt: "ontem", en: "yesterday", fr: "hier" },
    "time.days.one": { pt: "há {count} dia", en: "{count} day ago", fr: "il y a {count} jour" },
    "time.days.other": { pt: "há {count} dias", en: "{count} days ago", fr: "il y a {count} jours" },
    "state.privileged": { pt: "Sessão privilegiada", en: "Privileged session", fr: "Session privilégiée" },
    "error.code": { pt: "ERRO {code}", en: "ERROR {code}", fr: "ERREUR {code}" },
    "error.404.title": { pt: "Esta página não existe", en: "This page does not exist", fr: "Cette page n’existe pas" },
    "error.404.body": { pt: "O endereço pode estar errado, ou o conteúdo já não está disponível para si.", en: "The address may be wrong, or the content is no longer available to you.", fr: "L’adresse est peut-être erronée, ou le contenu ne vous est plus accessible." },
    "error.403.title": { pt: "Sem permissão", en: "No permission", fr: "Accès non autorisé" },
    "error.403.body": { pt: "A sua conta não tem acesso a esta área. Se precisar dela, peça acesso à administração da Instância.", en: "Your account does not have access to this area. If you need it, ask the Instance administration for access.", fr: "Votre compte n’a pas accès à cette zone. Si vous en avez besoin, demandez l’accès à l’administration de l’instance." },
    "error.502.title": { pt: "O Ocinye OS não respondeu", en: "Ocinye OS did not respond", fr: "Ocinye OS n’a pas répondu" },
    "error.502.body": { pt: "Um serviço necessário não está a responder neste momento. Os seus dados não foram alterados. Tente de novo dentro de instantes.", en: "A required service is not responding right now. Your data has not been changed. Try again in a moment.", fr: "Un service nécessaire ne répond pas pour le moment. Vos données n’ont pas été modifiées. Réessayez dans un instant." },
    "error.home": { pt: "Voltar ao Desktop", en: "Back to the Desktop", fr: "Retour au Desktop" },
    "error.retry": { pt: "Tentar de novo", en: "Try again", fr: "Réessayer" },
    "error.sign_in": { pt: "Iniciar sessão", en: "Sign in", fr: "Se connecter" },
    "error.ref": { pt: "Referência {ref}", en: "Reference {ref}", fr: "Référence {ref}" },
};
