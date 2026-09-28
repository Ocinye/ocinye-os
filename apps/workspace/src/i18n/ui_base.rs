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
};
