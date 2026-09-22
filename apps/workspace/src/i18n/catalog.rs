//! O catálogo de mensagens — uma linha por chave, o `pt` canónico à cabeça.
//!
//! # A ordem de trabalho
//!
//! O português define o significado; o inglês e o francês são projecções fiéis.
//! Uma chave nova nasce em `pt`, e só depois se traduz — nunca se traduz uma
//! frase partida (briefing i18n §7, §80).
//!
//! # Namespaces
//!
//! A chave descreve a semântica, não a palavra: `nav.home`, não `"Início"`. Os
//! grupos abaixo são fatias por superfície; [`GROUPS`] junta-os, e é sobre eles
//! que o portão de paridade corre.

use super::Entry;
use crate::catalogo;
use std::collections::HashMap;
use std::sync::OnceLock;

/// A navegação: barra lateral, secções, barra superior, migalhas.
const NAV: &[Entry] = catalogo! {
    "nav.section.personal": { pt: "Pessoal", en: "Personal", fr: "Personnel" },
    "nav.section.research": { pt: "Investigação", en: "Research", fr: "Recherche" },
    "nav.section.knowledge": { pt: "Conhecimento", en: "Knowledge", fr: "Connaissance" },
    "nav.home": { pt: "Home", en: "Home", fr: "Accueil" },
    "nav.my_work": { pt: "O Meu Trabalho", en: "My Work", fr: "Mon travail" },
    "nav.notes": { pt: "Notas", en: "Notes", fr: "Notes" },
    "nav.calendar": { pt: "Calendário", en: "Calendar", fr: "Calendrier" },
    "nav.messages": { pt: "Mensagens", en: "Messages", fr: "Messages" },
    "nav.mail": { pt: "Correio", en: "Mail", fr: "Courrier" },
    "nav.resources": { pt: "Meus Recursos", en: "My Resources", fr: "Mes ressources" },
    "nav.units": { pt: "Unidades", en: "Units", fr: "Unités" },
    "nav.ideas": { pt: "Ideias", en: "Ideas", fr: "Idées" },
    "nav.projects": { pt: "Projectos", en: "Projects", fr: "Projets" },
    "nav.knowledge": { pt: "Conhecimento", en: "Knowledge", fr: "Connaissance" },
    "nav.files": { pt: "Ficheiros", en: "Files", fr: "Fichiers" },
    "nav.bibliography": { pt: "Bibliografia", en: "Bibliography", fr: "Bibliographie" },
    "nav.data": { pt: "Dados", en: "Data", fr: "Données" },
    "nav.ai": { pt: "Ocinye AI", en: "Ocinye AI", fr: "Ocinye AI" },
    "nav.agents": { pt: "Agentes", en: "Agents", fr: "Agents" },
    "nav.compute": { pt: "Computação", en: "Compute", fr: "Calcul" },
    "nav.activity": { pt: "Actividade", en: "Activity", fr: "Activité" },
    "nav.admin": { pt: "Administração", en: "Administration", fr: "Administration" },
    "nav.audit": { pt: "Audit Log", en: "Audit Log", fr: "Journal d’audit" },
    "nav.prompt": { pt: "Prompt Ocinye", en: "Ocinye Prompt", fr: "Prompt Ocinye" },
    "nav.search": { pt: "Pesquisar", en: "Search", fr: "Rechercher" },
    "nav.ask": {
        pt: "Pesquisar, perguntar ou executar",
        en: "Search, ask or run",
        fr: "Rechercher, demander ou exécuter"
    },
    "nav.section.intelligence": { pt: "Inteligência", en: "Intelligence", fr: "Intelligence" },
    "nav.section.institutional": { pt: "Institucional", en: "Institutional", fr: "Institutionnel" },
    "nav.settings": { pt: "Definições", en: "Settings", fr: "Paramètres" },
    "nav.help": { pt: "Ajuda", en: "Help", fr: "Aide" },
    "nav.search.placeholder": {
        pt: "Pesquisar, perguntar ou executar no Ocinye…",
        en: "Search, ask or run in Ocinye…",
        fr: "Rechercher, demander ou exécuter dans Ocinye…"
    },
    "nav.create": { pt: "Criar", en: "Create", fr: "Créer" },
    "nav.notifications": { pt: "Notificações", en: "Notifications", fr: "Notifications" },
    "nav.account": { pt: "Conta", en: "Account", fr: "Compte" },
    "nav.core.ok": { pt: "Core OK", en: "Core OK", fr: "Core OK" },
    "nav.skip_to_content": { pt: "Saltar para o conteúdo", en: "Skip to content", fr: "Aller au contenu" },
    "nav.collapse_sidebar": { pt: "Recolher a barra lateral", en: "Collapse sidebar", fr: "Réduire la barre latérale" },
    "nav.sign_out": { pt: "Terminar sessão", en: "Sign out", fr: "Se déconnecter" },
};

/// Verbos e rótulos comuns, partilhados por muitas superfícies.
const ACTIONS: &[Entry] = catalogo! {
    "action.save": { pt: "Guardar", en: "Save", fr: "Enregistrer" },
    "action.cancel": { pt: "Cancelar", en: "Cancel", fr: "Annuler" },
    "action.continue": { pt: "Continuar", en: "Continue", fr: "Continuer" },
    "action.back": { pt: "Voltar", en: "Back", fr: "Retour" },
    "action.delete": { pt: "Apagar", en: "Delete", fr: "Supprimer" },
    "action.remove": { pt: "Remover", en: "Remove", fr: "Retirer" },
    "action.edit": { pt: "Editar", en: "Edit", fr: "Modifier" },
    "action.open": { pt: "Abrir", en: "Open", fr: "Ouvrir" },
    "action.close": { pt: "Fechar", en: "Close", fr: "Fermer" },
    "action.confirm": { pt: "Confirmar", en: "Confirm", fr: "Confirmer" },
    "action.create": { pt: "Criar", en: "Create", fr: "Créer" },
    "action.rename": { pt: "Mudar o nome", en: "Rename", fr: "Renommer" },
    "action.retry": { pt: "Tentar de novo", en: "Try again", fr: "Réessayer" },
};

/// O menu «+ Criar» (Global Create). As acções e atalhos são idênticos entre
/// línguas; só o rótulo muda (i18n §28).
const CREATE: &[Entry] = catalogo! {
    "create.idea": { pt: "Nova Ideia", en: "New Idea", fr: "Nouvelle idée" },
    "create.project": { pt: "Novo Projecto", en: "New Project", fr: "Nouveau projet" },
    "create.note": { pt: "Nova Nota", en: "New Note", fr: "Nouvelle note" },
    "create.reference": { pt: "Nova Referência", en: "New Reference", fr: "Nouvelle référence" },
    "create.dataset": { pt: "Novo Dataset", en: "New Dataset", fr: "Nouveau jeu de données" },
    "create.task": { pt: "Nova Tarefa", en: "New Task", fr: "Nouvelle tâche" },
    "create.agent": { pt: "Novo Agente IA", en: "New AI Agent", fr: "Nouvel agent IA" },
};

/// Estados de erro e de página cheia — nenhum ecrã fica em língua trocada.
const ERRORS: &[Entry] = catalogo! {
    "error.not_found.title": { pt: "Página não encontrada", en: "Page not found", fr: "Page introuvable" },
    "error.not_found.body": {
        pt: "Este endereço não corresponde a nenhum ecrã do Ocinye Workspace. Pode ter sido movido, ou o endereço pode estar incompleto.",
        en: "This address matches no screen in the Ocinye Workspace. It may have moved, or the address may be incomplete.",
        fr: "Cette adresse ne correspond à aucun écran de l’espace de travail Ocinye. Elle a pu être déplacée, ou l’adresse est incomplète."
    },
    "error.generic.title": {
        pt: "Ocorreu um erro inesperado",
        en: "An unexpected error occurred",
        fr: "Une erreur inattendue s’est produite"
    },
    "error.generic.body": {
        pt: "A operação não foi concluída. Nada foi alterado. Se o problema persistir, indique a referência abaixo a quem opera o Ocinye OS.",
        en: "The operation did not complete. Nothing was changed. If the problem persists, give the reference below to whoever operates Ocinye OS.",
        fr: "L’opération n’a pas abouti. Rien n’a été modifié. Si le problème persiste, communiquez la référence ci-dessous à l’équipe qui exploite Ocinye OS."
    },
    "error.reference": { pt: "Referência: ", en: "Reference: ", fr: "Référence : " },
    "error.forbidden.title": {
        pt: "Não possui acesso a este recurso",
        en: "You do not have access to this resource",
        fr: "Vous n’avez pas accès à cette ressource"
    },
    "error.forbidden.body": {
        pt: "O seu acesso é definido pelas unidades e Research Workspaces de que faz parte. Se precisa deste recurso para o seu trabalho, peça acesso a quem administra a sua unidade.",
        en: "Your access is defined by the units and Research Workspaces you belong to. If you need this resource for your work, ask whoever administers your unit for access.",
        fr: "Votre accès est défini par les unités et les Research Workspaces dont vous faites partie. Si vous avez besoin de cette ressource, demandez l’accès à l’administrateur de votre unité."
    },
    "error.go_home": { pt: "Ir para a Home", en: "Go to Home", fr: "Aller à l’accueil" },
};

/// O passo de primeira entrada: a escolha de idioma.
const FIRST_ENTRY: &[Entry] = catalogo! {
    "first_entry.language.title": {
        pt: "Escolha o seu idioma",
        en: "Choose your language",
        fr: "Choisissez votre langue"
    },
    "first_entry.language.subtitle": {
        pt: "Pode mudar mais tarde em Definições.",
        en: "You can change this later in Settings.",
        fr: "Vous pourrez le changer plus tard dans les Paramètres."
    },
    "first_entry.language.continue": { pt: "Continuar", en: "Continue", fr: "Continuer" },
};

/// Definições → Idioma e região.
const SETTINGS: &[Entry] = catalogo! {
    "settings.title": { pt: "Definições", en: "Settings", fr: "Paramètres" },
    "settings.subtitle": {
        pt: "A sua conta e as suas credenciais no Ocinye OS.",
        en: "Your account and your credentials in Ocinye OS.",
        fr: "Votre compte et vos identifiants dans Ocinye OS."
    },
    "settings.tab.account": { pt: "Conta", en: "Account", fr: "Compte" },
    "settings.tab.security": { pt: "Segurança", en: "Security", fr: "Sécurité" },
    "settings.tab.language": { pt: "Idioma e região", en: "Language & region", fr: "Langue et région" },
    "settings.tabs.aria": {
        pt: "Secções das definições",
        en: "Settings sections",
        fr: "Sections des paramètres"
    },
    "settings.language_region.title": {
        pt: "Idioma e região",
        en: "Language & region",
        fr: "Langue et région"
    },
    "settings.language.label": { pt: "Idioma", en: "Language", fr: "Langue" },
    "settings.language.help": {
        pt: "O idioma em que o Ocinye se mostra. Não muda o significado de nada — só a língua da interface.",
        en: "The language Ocinye shows itself in. It changes nothing’s meaning — only the interface language.",
        fr: "La langue dans laquelle Ocinye s’affiche. Elle ne change le sens de rien — seulement la langue de l’interface."
    },
    "settings.language.save": { pt: "Guardar idioma", en: "Save language", fr: "Enregistrer la langue" },
    "settings.language.saved": { pt: "Idioma guardado.", en: "Language saved.", fr: "Langue enregistrée." },
};

/// Todos os grupos de produção. O portão de paridade corre sobre isto.
///
/// Não inclui grupos de teste: uma chave só-`pt` de teste (para provar a queda)
/// não é um buraco de produção, e não deve fazer o portão soar.
pub const GROUPS: &[&[Entry]] = &[NAV, ACTIONS, CREATE, ERRORS, FIRST_ENTRY, SETTINGS];

/// Um grupo só de teste, para exercitar a queda ao canónico (briefing i18n §77).
#[cfg(test)]
const TEST_GROUP: &[Entry] = catalogo! {
    "test.fallback.only_pt": { pt: "Só português" },
};

/// Procura uma entrada pela chave, em tempo constante.
///
/// O mapa constrói-se uma vez, à primeira chamada, a partir de todos os grupos.
#[must_use]
pub fn entry(key: &str) -> Option<&'static Entry> {
    static MAPA: OnceLock<HashMap<&'static str, &'static Entry>> = OnceLock::new();
    MAPA.get_or_init(|| {
        let mut mapa = HashMap::new();
        for grupo in grupos() {
            for entrada in *grupo {
                mapa.insert(entrada.key, entrada);
            }
        }
        mapa
    })
    .get(key)
    .copied()
}

/// Os grupos activos — os de produção, e em teste também o grupo de teste.
fn grupos() -> Vec<&'static &'static [Entry]> {
    #[allow(unused_mut)]
    let mut v: Vec<&'static &'static [Entry]> = GROUPS.iter().collect();
    #[cfg(test)]
    v.push(&TEST_GROUP);
    v
}
