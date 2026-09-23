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

/// O painel inicial (Home). A saudação interpola `{name}`; o subtítulo conta.
const HOME: &[Entry] = catalogo! {
    "home.greeting.morning": { pt: "Bom dia, {name}", en: "Good morning, {name}", fr: "Bonjour, {name}" },
    "home.greeting.afternoon": { pt: "Boa tarde, {name}", en: "Good afternoon, {name}", fr: "Bonjour, {name}" },
    "home.greeting.evening": { pt: "Boa noite, {name}", en: "Good evening, {name}", fr: "Bonsoir, {name}" },
    "home.summary.empty": {
        pt: "Nada precisa da sua atenção neste momento.",
        en: "Nothing needs your attention right now.",
        fr: "Rien ne requiert votre attention pour le moment."
    },
    "home.summary.tasks.one": { pt: "Tem 1 tarefa atribuída", en: "You have 1 assigned task", fr: "Vous avez 1 tâche attribuée" },
    "home.summary.tasks.other": { pt: "Tem {count} tarefas atribuídas", en: "You have {count} assigned tasks", fr: "Vous avez {count} tâches attribuées" },
    "home.summary.research.one": { pt: "1 item de investigação a que tem acesso", en: "1 research item you can access", fr: "1 élément de recherche accessible" },
    "home.summary.research.other": { pt: "{count} itens de investigação a que tem acesso", en: "{count} research items you can access", fr: "{count} éléments de recherche accessibles" },
    "home.summary.join": { pt: "e", en: "and", fr: "et" },
    "home.summary.suffix": { pt: ".", en: ".", fr: "." },
    "home.kpi.units": { pt: "Unidades", en: "Units", fr: "Unités" },
    "home.kpi.units.hint": { pt: "activas", en: "active", fr: "actives" },
    "home.kpi.ideas": { pt: "Ideias", en: "Ideas", fr: "Idées" },
    "home.kpi.ideas.hint": { pt: "em investigação", en: "in research", fr: "en recherche" },
    "home.kpi.projects": { pt: "Projectos", en: "Projects", fr: "Projets" },
    "home.kpi.projects.hint": { pt: "em execução", en: "in progress", fr: "en cours" },
    "home.kpi.datasets": { pt: "Datasets", en: "Datasets", fr: "Jeux de données" },
    "home.kpi.datasets.hint": { pt: "catalogados", en: "catalogued", fr: "catalogués" },
    "home.kpi.unavailable": { pt: "indisponível", en: "unavailable", fr: "indisponible" },
    "home.kpi.no_answer": {
        pt: "O Ocinye Core não respondeu a esta contagem.",
        en: "Ocinye Core did not answer this count.",
        fr: "Ocinye Core n’a pas répondu à ce décompte."
    },
    "home.continue.title": { pt: "Continuar trabalho", en: "Continue work", fr: "Continuer le travail" },
    "home.continue.aside": { pt: "Research Workspaces", en: "Research Workspaces", fr: "Research Workspaces" },
    "home.continue.empty": {
        pt: "Ainda não há trabalho de investigação a que tenha acesso. Crie uma ideia para começar.",
        en: "There is no research work you can access yet. Create an idea to begin.",
        fr: "Aucun travail de recherche accessible pour l’instant. Créez une idée pour commencer."
    },
    "home.view_all": { pt: "Ver tudo", en: "View all", fr: "Tout voir" },
    "home.tasks.title": { pt: "Tarefas pendentes", en: "Pending tasks", fr: "Tâches en attente" },
    "home.tasks.empty": { pt: "Não tem tarefas abertas.", en: "You have no open tasks.", fr: "Vous n’avez aucune tâche ouverte." },
    "home.tasks.no_due": { pt: "sem prazo", en: "no due date", fr: "sans échéance" },
    "home.activity.title": { pt: "Actividade recente", en: "Recent activity", fr: "Activité récente" },
    "home.activity.empty": { pt: "Ainda não há actividade.", en: "No activity yet.", fr: "Aucune activité pour l’instant." },
    "home.ai.eyebrow": { pt: "OCINYE AI", en: "OCINYE AI", fr: "OCINYE AI" },
    "home.ai.available": { pt: "Inteligência disponível", en: "Intelligence available", fr: "Intelligence disponible" },
    "home.ai.unavailable": { pt: "Inteligência ainda não disponível", en: "Intelligence not yet available", fr: "Intelligence pas encore disponible" },
    "home.ai.default_message": {
        pt: "Nenhum nó de IA Ocinye está actualmente disponível.",
        en: "No Ocinye AI node is currently available.",
        fr: "Aucun nœud d’IA Ocinye n’est actuellement disponible."
    },
    "home.ai.open_prompt": { pt: "Abrir Prompt", en: "Open Prompt", fr: "Ouvrir Prompt" },
    "home.ai.hub": { pt: "Hub de IA", en: "AI Hub", fr: "Hub IA" },
    "home.quick.title": { pt: "Acesso rápido", en: "Quick access", fr: "Accès rapide" },
    "home.quick.prompt": { pt: "Prompt IA", en: "AI Prompt", fr: "Prompt IA" },
    "home.new_idea": { pt: "Nova Ideia", en: "New Idea", fr: "Nouvelle idée" },
    "home.new_project": { pt: "Novo Projecto", en: "New Project", fr: "Nouveau projet" },
    "home.new_dataset": { pt: "Novo Dataset", en: "New Dataset", fr: "Nouveau jeu de données" },
    "home.prompt_ocinye": { pt: "Prompt Ocinye", en: "Ocinye Prompt", fr: "Prompt Ocinye" },
    "home.no_permission.idea": {
        pt: "Não tem autorização para criar ideias.",
        en: "You are not authorised to create ideas.",
        fr: "Vous n’êtes pas autorisé à créer des idées."
    },
};

/// As classificações institucionais. O valor persistido é o enum (`INTERNAL`); o
/// rótulo mostra-se traduzido, em maiúsculas por convenção de código (i18n §12).
const CLASSIFICATION: &[Entry] = catalogo! {
    "classification.public": { pt: "PÚBLICO", en: "PUBLIC", fr: "PUBLIC" },
    "classification.internal": { pt: "INTERNO", en: "INTERNAL", fr: "INTERNE" },
    "classification.confidential": { pt: "CONFIDENCIAL", en: "CONFIDENTIAL", fr: "CONFIDENTIEL" },
    "classification.restricted": { pt: "RESTRITO", en: "RESTRICTED", fr: "RESTREINT" },
    "classification.public.help": { pt: "Publicável fora da instituição", en: "Publishable outside the institution", fr: "Publiable hors de l’institution" },
    "classification.internal.help": { pt: "Legível por qualquer membro activo", en: "Readable by any active member", fr: "Lisible par tout membre actif" },
    "classification.confidential.help": { pt: "Requer pertença à unidade ou ao workspace", en: "Requires unit or workspace membership", fr: "Nécessite l’appartenance à l’unité ou à l’espace" },
    "classification.restricted.help": { pt: "Requer pertença explícita ao workspace", en: "Requires explicit workspace membership", fr: "Nécessite une appartenance explicite à l’espace" },
    "classification.unknown": { pt: "Classificação", en: "Classification", fr: "Classification" },
};

/// Os estados de uma tarefa. O valor persistido é o enum; o rótulo traduz-se.
const TASK_STATE: &[Entry] = catalogo! {
    "task.state.todo": { pt: "Por fazer", en: "To do", fr: "À faire" },
    "task.state.in_progress": { pt: "Em curso", en: "In progress", fr: "En cours" },
    "task.state.blocked": { pt: "Bloqueada", en: "Blocked", fr: "Bloquée" },
    "task.state.in_review": { pt: "Em revisão", en: "In review", fr: "En revue" },
    "task.state.done": { pt: "Concluída", en: "Done", fr: "Terminée" },
    "task.state.cancelled": { pt: "Cancelada", en: "Cancelled", fr: "Annulée" },
    "task.priority.high": { pt: "Alta", en: "High", fr: "Haute" },
    "task.priority.normal": { pt: "Normal", en: "Normal", fr: "Normale" },
    "task.priority.low": { pt: "Baixa", en: "Low", fr: "Basse" },
};

/// O ecrã «O Meu Trabalho».
const MY_WORK: &[Entry] = catalogo! {
    "my_work.title": { pt: "O Meu Trabalho", en: "My Work", fr: "Mon travail" },
    "my_work.subtitle": {
        pt: "Tudo o que lhe está atribuído ou que segue de perto.",
        en: "Everything assigned to you or that you follow closely.",
        fr: "Tout ce qui vous est attribué ou que vous suivez de près."
    },
    "my_work.tabs.aria": { pt: "Secções do meu trabalho", en: "My work sections", fr: "Sections de mon travail" },
    "my_work.tab.tasks": { pt: "Tarefas", en: "Tasks", fr: "Tâches" },
    "my_work.tab.activity": { pt: "Actividade", en: "Activity", fr: "Activité" },
    "my_work.tab.ideas": { pt: "Ideias", en: "Ideas", fr: "Idées" },
    "my_work.tab.projects": { pt: "Projectos", en: "Projects", fr: "Projets" },
    "my_work.tab.documents": { pt: "Documentos", en: "Documents", fr: "Documents" },
    "my_work.tab.datasets": { pt: "Datasets", en: "Datasets", fr: "Jeux de données" },
    "my_work.tab.favourites": { pt: "Favoritos", en: "Favourites", fr: "Favoris" },
    "my_work.tab.notes": { pt: "Notas", en: "Notes", fr: "Notes" },
    "my_work.tasks.title": { pt: "Tarefas atribuídas", en: "Assigned tasks", fr: "Tâches attribuées" },
    "my_work.tasks.empty": { pt: "Não tem tarefas atribuídas.", en: "You have no assigned tasks.", fr: "Aucune tâche ne vous est attribuée." },
    "my_work.no_due": { pt: "sem prazo", en: "no due date", fr: "sans échéance" },
    "my_work.research.title": { pt: "Investigação que sigo", en: "Research I follow", fr: "Recherche que je suis" },
    "my_work.research.empty": {
        pt: "Ainda não pertence a nenhum Research Workspace.",
        en: "You do not belong to any Research Workspace yet.",
        fr: "Vous n’appartenez encore à aucun Research Workspace."
    },
    "my_work.documents.title": { pt: "Documentos recentes", en: "Recent documents", fr: "Documents récents" },
    "my_work.unavailable": { pt: "indisponível", en: "unavailable", fr: "indisponible" },
    "my_work.documents.body": {
        pt: "O Ocinye Core ainda não serve os documentos abertos recentemente por uma pessoa. Os documentos existem e estão acessíveis a partir de cada Research Workspace.",
        en: "Ocinye Core does not yet serve the documents a person opened recently. The documents exist and are reachable from each Research Workspace.",
        fr: "Ocinye Core ne fournit pas encore les documents récemment ouverts par une personne. Les documents existent et sont accessibles depuis chaque Research Workspace."
    },
    "my_work.units.title": { pt: "Unidades seguidas", en: "Followed units", fr: "Unités suivies" },
    "my_work.units.body": {
        pt: "Seguir uma unidade ainda não existe no Ocinye Core. As unidades a que pertence estão em Unidades.",
        en: "Following a unit does not exist yet in Ocinye Core. The units you belong to are in Units.",
        fr: "Suivre une unité n’existe pas encore dans Ocinye Core. Les unités auxquelles vous appartenez sont dans Unités."
    },
    "my_work.my_activity.title": { pt: "A minha actividade", en: "My activity", fr: "Mon activité" },
    "my_work.my_activity.empty": { pt: "Sem actividade recente.", en: "No recent activity.", fr: "Aucune activité récente." },
};

/// O Knowledge Hub («Conhecimento»).
const KNOWLEDGE: &[Entry] = catalogo! {
    "knowledge.title": { pt: "Conhecimento", en: "Knowledge", fr: "Connaissance" },
    "knowledge.subtitle": { pt: "A memória institucional da Ocinye.", en: "Ocinye’s institutional memory.", fr: "La mémoire institutionnelle d’Ocinye." },
    "knowledge.tabs.aria": { pt: "Secções do conhecimento", en: "Knowledge sections", fr: "Sections de la connaissance" },
    "knowledge.tab.all": { pt: "Tudo", en: "All", fr: "Tout" },
    "knowledge.tab.bibliography": { pt: "Bibliografia", en: "Bibliography", fr: "Bibliographie" },
    "knowledge.tab.sources": { pt: "Fontes", en: "Sources", fr: "Sources" },
    "knowledge.tab.notes": { pt: "Notas", en: "Notes", fr: "Notes" },
    "knowledge.tab.documents": { pt: "Documentos", en: "Documents", fr: "Documents" },
    "knowledge.tab.results": { pt: "Resultados", en: "Results", fr: "Résultats" },
    "knowledge.tab.publications": { pt: "Publicações", en: "Publications", fr: "Publications" },
    "knowledge.counter.bibliography": { pt: "Bibliografia", en: "Bibliography", fr: "Bibliographie" },
    "knowledge.counter.documents": { pt: "Documentos", en: "Documents", fr: "Documents" },
    "knowledge.counter.datasets": { pt: "Datasets", en: "Datasets", fr: "Jeux de données" },
    "knowledge.counter.results": { pt: "Resultados", en: "Results", fr: "Résultats" },
    "knowledge.here": { pt: "o acervo institucional", en: "the institutional collection", fr: "le fonds institutionnel" },
    "knowledge.recent.title": { pt: "Adicionado recentemente", en: "Recently added", fr: "Ajouté récemment" },
    "knowledge.recent.empty": {
        pt: "Ainda não há conhecimento registado a que tenha acesso.",
        en: "There is no recorded knowledge you can access yet.",
        fr: "Aucune connaissance enregistrée n’est encore accessible."
    },
    "knowledge.no_screen": { pt: "Este acervo ainda não tem um ecrã próprio.", en: "This collection has no screen of its own yet.", fr: "Ce fonds n’a pas encore d’écran propre." },
    "knowledge.not_in_core": { pt: "Esta entidade ainda não existe no Ocinye Core.", en: "This entity does not exist in Ocinye Core yet.", fr: "Cette entité n’existe pas encore dans Ocinye Core." },
    "knowledge.not_implemented": { pt: "Não implementado", en: "Not implemented", fr: "Non implémenté" },
};

/// A superfície de assistência contextual (partilhada por vários ecrãs).
const ASSIST: &[Entry] = catalogo! {
    "assist.title": { pt: "Assistência do Ocinye", en: "Ocinye Assistance", fr: "Assistance d’Ocinye" },
    "assist.placeholder": {
        pt: "Perguntar ou pedir algo sobre {here}…",
        en: "Ask or request something about {here}…",
        fr: "Demander quelque chose à propos de {here}…"
    },
    "assist.submit": { pt: "Pedir", en: "Ask", fr: "Demander" },
    "assist.no_inference": {
        pt: "Nenhum nó de IA está disponível nesta instalação, por isso perguntar e executar ainda não podem ser servidos. A pesquisa funciona, e todas as acções deste ecrã continuam disponíveis.",
        en: "No AI node is available in this installation, so asking and running cannot be served yet. Search works, and every action on this screen remains available.",
        fr: "Aucun nœud d’IA n’est disponible dans cette installation, donc demander et exécuter ne peuvent pas encore être servis. La recherche fonctionne, et toutes les actions de cet écran restent disponibles."
    },
    "assist.suggest.idea.1": { pt: "Resume o estado desta Ideia", en: "Summarise the state of this Idea", fr: "Résume l’état de cette idée" },
    "assist.suggest.idea.2": { pt: "Que fontes estão relacionadas com esta Ideia?", en: "Which sources relate to this Idea?", fr: "Quelles sources sont liées à cette idée ?" },
    "assist.suggest.idea.3": { pt: "O que falta antes de passar a revisão?", en: "What is missing before it moves to review?", fr: "Que manque-t-il avant le passage en revue ?" },
    "assist.suggest.idea.4": { pt: "Cria uma tarefa para rever a bibliografia", en: "Create a task to review the bibliography", fr: "Crée une tâche pour revoir la bibliographie" },
    "assist.suggest.project.1": { pt: "Resume o estado deste Projecto", en: "Summarise the state of this Project", fr: "Résume l’état de ce projet" },
    "assist.suggest.project.2": { pt: "Que tarefas continuam abertas?", en: "Which tasks are still open?", fr: "Quelles tâches sont encore ouvertes ?" },
    "assist.suggest.project.3": { pt: "Que documentos estão ligados a este Projecto?", en: "Which documents are linked to this Project?", fr: "Quels documents sont liés à ce projet ?" },
    "assist.suggest.project.4": { pt: "Cria uma nota de decisão", en: "Create a decision note", fr: "Crée une note de décision" },
    "assist.suggest.knowledge.1": { pt: "Encontra fontes sobre armazenamento", en: "Find sources about storage", fr: "Trouve des sources sur le stockage" },
    "assist.suggest.knowledge.2": { pt: "Que notas existem sobre este tema?", en: "What notes exist on this topic?", fr: "Quelles notes existent sur ce sujet ?" },
    "assist.suggest.knowledge.3": { pt: "Resume as notas deste ambiente", en: "Summarise the notes of this environment", fr: "Résume les notes de cet environnement" },
};

/// A pesquisa institucional e os rótulos de tipo de entidade.
const SEARCH: &[Entry] = catalogo! {
    "search.title": { pt: "Pesquisar no Ocinye", en: "Search in Ocinye", fr: "Rechercher dans Ocinye" },
    "search.subtitle": {
        pt: "A pesquisa devolve apenas aquilo a que tem acesso. Um artefacto que não possa consultar não aparece, nem nas contagens.",
        en: "Search returns only what you can access. An artefact you cannot see does not appear, not even in the counts.",
        fr: "La recherche ne renvoie que ce à quoi vous avez accès. Un artefact que vous ne pouvez pas consulter n’apparaît pas, ni dans les décomptes."
    },
    "search.field.label": { pt: "Pesquisar", en: "Search", fr: "Rechercher" },
    "search.field.placeholder": {
        pt: "Ideias, projectos, bibliografia, documentos, datasets…",
        en: "Ideas, projects, bibliography, documents, datasets…",
        fr: "Idées, projets, bibliographie, documents, jeux de données…"
    },
    "search.submit": { pt: "Pesquisar", en: "Search", fr: "Rechercher" },
    "search.mode.aria": { pt: "Modo de pesquisa", en: "Search mode", fr: "Mode de recherche" },
    "search.mode.textual": { pt: "Textual", en: "Textual", fr: "Textuelle" },
    "search.mode.semantic": { pt: "Semântica", en: "Semantic", fr: "Sémantique" },
    "search.mode.semantic_unavailable": {
        pt: "Semântica — ainda não disponível",
        en: "Semantic — not yet available",
        fr: "Sémantique — pas encore disponible"
    },
    "search.semantic.default_message": {
        pt: "A pesquisa semântica depende de uma capacidade de embeddings, que não está actualmente disponível.",
        en: "Semantic search depends on an embeddings capability, which is not currently available.",
        fr: "La recherche sémantique dépend d’une capacité d’embeddings, qui n’est pas disponible actuellement."
    },
    "search.count.none": { pt: "Nenhum resultado", en: "No results", fr: "Aucun résultat" },
    "search.count.one": { pt: "{count} resultado", en: "{count} result", fr: "{count} résultat" },
    "search.count.other": { pt: "{count} resultados", en: "{count} results", fr: "{count} résultats" },
    "search.empty.title": { pt: "Pesquisar no Ocinye", en: "Search in Ocinye", fr: "Rechercher dans Ocinye" },
    "search.empty.body": {
        pt: "Escreva um termo para procurar em ideias, projectos, bibliografia, notas, documentos e datasets.",
        en: "Type a term to search ideas, projects, bibliography, notes, documents and datasets.",
        fr: "Saisissez un terme pour chercher dans les idées, projets, bibliographie, notes, documents et jeux de données."
    },
    "search.no_results.title": { pt: "Nenhum resultado", en: "No results", fr: "Aucun résultat" },
    "search.no_results.body": {
        pt: "Nada corresponde a «{query}» entre os artefactos a que tem acesso.",
        en: "Nothing matches “{query}” among the artefacts you can access.",
        fr: "Rien ne correspond à « {query} » parmi les artefacts auxquels vous avez accès."
    },
    "search.in_file_content": { pt: "No conteúdo dos ficheiros", en: "In file content", fr: "Dans le contenu des fichiers" },
    "search.file": { pt: "FICHEIRO", en: "FILE", fr: "FICHIER" },
    "entity.idea": { pt: "Ideia", en: "Idea", fr: "Idée" },
    "entity.project": { pt: "Projecto", en: "Project", fr: "Projet" },
    "entity.source": { pt: "Referência", en: "Reference", fr: "Référence" },
    "entity.note": { pt: "Nota", en: "Note", fr: "Note" },
    "entity.document": { pt: "Documento", en: "Document", fr: "Document" },
    "entity.dataset": { pt: "Dataset", en: "Dataset", fr: "Jeu de données" },
    "entity.unit": { pt: "Unidade", en: "Unit", fr: "Unité" },
};

/// As Notas pessoais — lista, editor, histórico, partilha, lixo. O conteúdo das
/// notas (título, corpo, etiquetas) é do membro e não se traduz.
const NOTES: &[Entry] = catalogo! {
    "notes.title": { pt: "Notas", en: "Notes", fr: "Notes" },
    "notes.subtitle": { pt: "As suas notas. Cada nota é sua, e guarda a sua própria história.", en: "Your notes. Each note is yours, and keeps its own history.", fr: "Vos notes. Chaque note est la vôtre et garde son propre historique." },
    "notes.trash": { pt: "Lixo", en: "Trash", fr: "Corbeille" },
    "notes.shared_with_me": { pt: "Partilhadas comigo", en: "Shared with me", fr: "Partagées avec moi" },
    "notes.new": { pt: "Nova nota", en: "New note", fr: "Nouvelle note" },
    "notes.folder.all": { pt: "Todas", en: "All", fr: "Toutes" },
    "notes.folder.new_placeholder": { pt: "Nova pasta", en: "New folder", fr: "Nouveau dossier" },
    "notes.folder.new_aria": { pt: "Nome da nova pasta", en: "New folder name", fr: "Nom du nouveau dossier" },
    "notes.folder.create": { pt: "Criar", en: "Create", fr: "Créer" },
    "notes.filter.tag": { pt: "Etiqueta: ", en: "Tag: ", fr: "Étiquette : " },
    "notes.filter.view_all": { pt: "Ver todas", en: "View all", fr: "Tout voir" },
    "notes.filter.folder": { pt: "Pasta: ", en: "Folder: ", fr: "Dossier : " },
    "notes.folder.fallback": { pt: "Pasta", en: "Folder", fr: "Dossier" },
    "notes.folder.delete": { pt: "Apagar pasta", en: "Delete folder", fr: "Supprimer le dossier" },
    "notes.untitled": { pt: "Sem título", en: "Untitled", fr: "Sans titre" },
    "notes.updated_at": { pt: "Actualizada a {date}", en: "Updated {date}", fr: "Modifiée le {date}" },
    "notes.empty.tag.title": { pt: "Nenhuma nota com esta etiqueta", en: "No note with this tag", fr: "Aucune note avec cette étiquette" },
    "notes.empty.tag.body": { pt: "Nenhuma das suas notas tem esta etiqueta. Veja todas as notas ou etiquete uma.", en: "None of your notes has this tag. View all notes or tag one.", fr: "Aucune de vos notes n’a cette étiquette. Voyez toutes les notes ou étiquetez-en une." },
    "notes.empty.folder.title": { pt: "Esta pasta está vazia", en: "This folder is empty", fr: "Ce dossier est vide" },
    "notes.empty.folder.body": { pt: "Nenhuma das suas notas está nesta pasta. Arrume uma aqui pelo editor, ou veja todas as notas.", en: "None of your notes is in this folder. File one here from the editor, or view all notes.", fr: "Aucune de vos notes n’est dans ce dossier. Classez-en une depuis l’éditeur, ou voyez toutes les notes." },
    "notes.empty.all.title": { pt: "Ainda não há notas", en: "No notes yet", fr: "Aucune note pour l’instant" },
    "notes.empty.all.body": { pt: "Uma nota é o sítio para uma ideia solta, um apontamento de reunião ou uma lista de tarefas. Comece uma.", en: "A note is the place for a loose idea, a meeting jot or a task list. Start one.", fr: "Une note est l’endroit pour une idée en vrac, une prise de notes de réunion ou une liste de tâches. Commencez-en une." },
    "notes.shared.subtitle": { pt: "Notas que outra pessoa partilhou consigo. Cada uma continua a ser dela.", en: "Notes someone shared with you. Each remains theirs.", fr: "Des notes qu’une autre personne a partagées avec vous. Chacune reste la sienne." },
    "notes.shared.my_notes": { pt: "As minhas notas", en: "My notes", fr: "Mes notes" },
    "notes.shared.empty.title": { pt: "Ainda não há notas partilhadas", en: "No shared notes yet", fr: "Aucune note partagée pour l’instant" },
    "notes.shared.empty.body": { pt: "Quando alguém partilhar uma nota consigo, ela aparece aqui — para ler, ou para editar, conforme o acesso que lhe deram.", en: "When someone shares a note with you, it appears here — to read, or to edit, depending on the access they gave you.", fr: "Quand quelqu’un partage une note avec vous, elle apparaît ici — pour lire ou pour modifier, selon l’accès accordé." },
    "notes.trash.subtitle": { pt: "Notas apagadas. Restaure uma para a trazer de volta, ou elimine-a definitivamente.", en: "Deleted notes. Restore one to bring it back, or delete it permanently.", fr: "Notes supprimées. Restaurez-en une pour la récupérer, ou supprimez-la définitivement." },
    "notes.trash.restore": { pt: "Restaurar", en: "Restore", fr: "Restaurer" },
    "notes.trash.purge": { pt: "Eliminar definitivamente", en: "Delete permanently", fr: "Supprimer définitivement" },
    "notes.trash.empty.title": { pt: "O Lixo está vazio", en: "The Trash is empty", fr: "La corbeille est vide" },
    "notes.trash.empty.body": { pt: "Nenhuma nota apagada. Quando apagar uma, ela fica aqui até a restaurar ou eliminar definitivamente.", en: "No deleted notes. When you delete one, it stays here until you restore or permanently delete it.", fr: "Aucune note supprimée. Quand vous en supprimez une, elle reste ici jusqu’à sa restauration ou sa suppression définitive." },
    "notes.role.editor": { pt: "Edição", en: "Edit", fr: "Modification" },
    "notes.role.viewer": { pt: "Leitura", en: "Read", fr: "Lecture" },
    "notes.editor.title": { pt: "Nota", en: "Note", fr: "Note" },
    "notes.editor.back": { pt: "Voltar às notas", en: "Back to notes", fr: "Retour aux notes" },
    "notes.editor.delete": { pt: "Apagar", en: "Delete", fr: "Supprimer" },
    "notes.editor.shared_banner": { pt: "Esta nota foi partilhada consigo. Pode editá-la; o dono continua a ser quem a criou.", en: "This note was shared with you. You can edit it; the owner is still whoever created it.", fr: "Cette note vous a été partagée. Vous pouvez la modifier ; le propriétaire reste celui qui l’a créée." },
    "notes.editor.live": { pt: "Esta nota foi actualizada noutro sítio. Recarregue para ver a versão actual.", en: "This note was updated elsewhere. Reload to see the current version.", fr: "Cette note a été modifiée ailleurs. Rechargez pour voir la version actuelle." },
    "notes.editor.no_folder": { pt: "Sem pasta", en: "No folder", fr: "Aucun dossier" },
    "notes.editor.folder_aria": { pt: "Pasta da nota", en: "Note folder", fr: "Dossier de la note" },
    "notes.editor.title_aria": { pt: "Título da nota", en: "Note title", fr: "Titre de la note" },
    "notes.editor.tags_placeholder": { pt: "Adicionar etiquetas", en: "Add tags", fr: "Ajouter des étiquettes" },
    "notes.editor.tags_aria": { pt: "Etiquetas da nota", en: "Note tags", fr: "Étiquettes de la note" },
    "notes.editor.toolbar_aria": { pt: "Formatação", en: "Formatting", fr: "Mise en forme" },
    "notes.activity.title": { pt: "Actividade", en: "Activity", fr: "Activité" },
    "notes.activity.created": { pt: "Criou a nota", en: "Created the note", fr: "A créé la note" },
    "notes.activity.shared": { pt: "Partilhou a nota", en: "Shared the note", fr: "A partagé la note" },
    "notes.activity.revoked": { pt: "Revogou uma partilha", en: "Revoked a share", fr: "A révoqué un partage" },
    "notes.activity.deleted": { pt: "Apagou a nota", en: "Deleted the note", fr: "A supprimé la note" },
    "notes.activity.restored": { pt: "Restaurou a nota", en: "Restored the note", fr: "A restauré la note" },
    "notes.activity.updated": { pt: "Editou a nota", en: "Edited the note", fr: "A modifié la note" },
    "notes.activity.other": { pt: "Actividade", en: "Activity", fr: "Activité" },
    "notes.someone": { pt: "Alguém", en: "Someone", fr: "Quelqu’un" },
    "notes.history.title": { pt: "Histórico", en: "History", fr: "Historique" },
    "notes.history.hint": { pt: "Cada gravação deixa uma versão. Abra uma para a ver, e restaure-a se quiser — sem perder as posteriores.", en: "Each save leaves a version. Open one to view it, and restore it if you like — without losing the later ones.", fr: "Chaque enregistrement laisse une version. Ouvrez-en une pour la voir, et restaurez-la si vous voulez — sans perdre les suivantes." },
    "notes.history.unknown_author": { pt: "Autor desconhecido", en: "Unknown author", fr: "Auteur inconnu" },
    "notes.history.version": { pt: "Versão {n}", en: "Version {n}", fr: "Version {n}" },
    "notes.revision.subtitle": { pt: "Versão {n} desta nota — uma fotografia do que era então.", en: "Version {n} of this note — a snapshot of what it was then.", fr: "Version {n} de cette note — un instantané de ce qu’elle était alors." },
    "notes.revision.back": { pt: "Voltar à nota", en: "Back to the note", fr: "Retour à la note" },
    "notes.revision.restore": { pt: "Restaurar esta versão", en: "Restore this version", fr: "Restaurer cette version" },
    "notes.revision.restore_hint": { pt: "Restaurar repõe esta versão como a mais recente, sem apagar as que vieram depois.", en: "Restoring makes this version the most recent, without deleting the ones that came after.", fr: "La restauration rétablit cette version comme la plus récente, sans supprimer celles venues après." },
    "notes.reader.back": { pt: "Voltar", en: "Back", fr: "Retour" },
    "notes.reader.readonly_banner": { pt: "Esta nota foi partilhada consigo só para leitura.", en: "This note was shared with you for reading only.", fr: "Cette note vous a été partagée en lecture seule." },
    "notes.share.title": { pt: "Partilha", en: "Sharing", fr: "Partage" },
    "notes.share.hint": { pt: "Dê a uma pessoa acesso a esta nota — só de leitura, ou também de edição.", en: "Give a person access to this note — read-only, or editing too.", fr: "Donnez à une personne l’accès à cette note — en lecture seule, ou aussi en modification." },
    "notes.share.person_aria": { pt: "Pessoa", en: "Person", fr: "Personne" },
    "notes.share.access_aria": { pt: "Acesso", en: "Access", fr: "Accès" },
    "notes.share.submit": { pt: "Partilhar", en: "Share", fr: "Partager" },
    "notes.share.none_left": { pt: "Não há mais ninguém com quem partilhar esta nota.", en: "There is no one else to share this note with.", fr: "Il n’y a personne d’autre avec qui partager cette note." },
    "notes.share.revoke": { pt: "Revogar", en: "Revoke", fr: "Révoquer" },
};

/// «Meus Recursos» — governança de recursos do membro.
const RESOURCES: &[Entry] = catalogo! {
    "resources.title": { pt: "Meus Recursos", en: "My Resources", fr: "Mes ressources" },
    "resources.intro": {
        pt: "Quanto de capacidade institucional pode consumir, e quanto já consumiu. Ter direito a um recurso não é ter acesso a dados — são sistemas separados.",
        en: "How much institutional capacity you may consume, and how much you already have. A right to a resource is not access to data — they are separate systems.",
        fr: "Quelle capacité institutionnelle vous pouvez consommer, et combien vous en avez déjà. Un droit à une ressource n’est pas un accès aux données — ce sont des systèmes distincts."
    },
    "resources.origin.help": {
        pt: "O limite não é um número mágico: soma-se de um perfil de alocação e das concessões que lhe foram feitas por cima.",
        en: "The limit is not a magic number: it sums from an allocation profile and the grants made on top of it.",
        fr: "La limite n’est pas un nombre magique : elle s’additionne d’un profil d’allocation et des attributions faites par-dessus."
    },
    "resources.storage.title": { pt: "Armazenamento pessoal", en: "Personal storage", fr: "Stockage personnel" },
    "resources.storage.subtitle": {
        pt: "Os ficheiros, imagens de notas e anexos de correio que lhe pertencem.",
        en: "The files, note images and mail attachments that belong to you.",
        fr: "Les fichiers, images de notes et pièces jointes de courrier qui vous appartiennent."
    },
    "resources.no_limit_assigned": { pt: "Sem limite de armazenamento atribuído.", en: "No storage limit assigned.", fr: "Aucune limite de stockage attribuée." },
    "resources.in_use": { pt: "Em uso", en: "In use", fr: "Utilisé" },
    "resources.limit": { pt: "Limite", en: "Limit", fr: "Limite" },
    "resources.available": { pt: "Disponível", en: "Available", fr: "Disponible" },
    "resources.no_limit": { pt: "sem limite", en: "no limit", fr: "sans limite" },
    "resources.origin.title": { pt: "Como se chega a este limite", en: "How this limit is reached", fr: "Comment cette limite est atteinte" },
    "resources.no_allocation": { pt: "Ainda não tem nenhuma alocação de armazenamento atribuída.", en: "You have no storage allocation assigned yet.", fr: "Aucune allocation de stockage ne vous est encore attribuée." },
    "resources.expires": { pt: "expira {date}", en: "expires {date}", fr: "expire le {date}" },
    "resources.state.normal": { pt: "Normal", en: "Normal", fr: "Normale" },
    "resources.state.warning": { pt: "Aviso", en: "Warning", fr: "Alerte" },
    "resources.state.critical": { pt: "Crítico", en: "Critical", fr: "Critique" },
    "resources.state.over_quota": { pt: "Acima da quota", en: "Over quota", fr: "Quota dépassé" },
    "resources.state.unknown": { pt: "Desconhecido", en: "Unknown", fr: "Inconnu" },
    "resources.source.profile": { pt: "Perfil", en: "Profile", fr: "Profil" },
    "resources.source.override": { pt: "Substituição", en: "Override", fr: "Dérogation" },
    "resources.source.temporary": { pt: "Concessão temporária", en: "Temporary grant", fr: "Attribution temporaire" },
    "resources.source.other": { pt: "Origem", en: "Source", fr: "Origine" },
    "resources.footnote": {
        pt: "O armazenamento é, para já, o único recurso medido e imposto. Computação, GPU e outros recursos governam-se pela mesma fundação e aparecerão aqui à medida que forem ligados.",
        en: "Storage is, for now, the only metered and enforced resource. Compute, GPU and other resources are governed by the same foundation and will appear here as they are switched on.",
        fr: "Le stockage est, pour l’instant, la seule ressource mesurée et imposée. Le calcul, le GPU et les autres ressources relèvent de la même fondation et apparaîtront ici à mesure qu’ils seront activés."
    },
};

/// Nomes de mês e de dia, e os moldes de data. A ordem das palavras e os
/// conectores mudam por língua («26 de Agosto de 2026» / «26 August 2026» /
/// «26 août 2026»), por isso o molde é por locale, não uma concatenação fixa
/// (i18n §22, §25).
const DATE: &[Entry] = catalogo! {
    "date.month.1": { pt: "Janeiro", en: "January", fr: "janvier" },
    "date.month.2": { pt: "Fevereiro", en: "February", fr: "février" },
    "date.month.3": { pt: "Março", en: "March", fr: "mars" },
    "date.month.4": { pt: "Abril", en: "April", fr: "avril" },
    "date.month.5": { pt: "Maio", en: "May", fr: "mai" },
    "date.month.6": { pt: "Junho", en: "June", fr: "juin" },
    "date.month.7": { pt: "Julho", en: "July", fr: "juillet" },
    "date.month.8": { pt: "Agosto", en: "August", fr: "août" },
    "date.month.9": { pt: "Setembro", en: "September", fr: "septembre" },
    "date.month.10": { pt: "Outubro", en: "October", fr: "octobre" },
    "date.month.11": { pt: "Novembro", en: "November", fr: "novembre" },
    "date.month.12": { pt: "Dezembro", en: "December", fr: "décembre" },
    "date.weekday.1": { pt: "Segunda-feira", en: "Monday", fr: "lundi" },
    "date.weekday.2": { pt: "Terça-feira", en: "Tuesday", fr: "mardi" },
    "date.weekday.3": { pt: "Quarta-feira", en: "Wednesday", fr: "mercredi" },
    "date.weekday.4": { pt: "Quinta-feira", en: "Thursday", fr: "jeudi" },
    "date.weekday.5": { pt: "Sexta-feira", en: "Friday", fr: "vendredi" },
    "date.weekday.6": { pt: "Sábado", en: "Saturday", fr: "samedi" },
    "date.weekday.7": { pt: "Domingo", en: "Sunday", fr: "dimanche" },
    "date.weekday_short.1": { pt: "Seg", en: "Mon", fr: "lun" },
    "date.weekday_short.2": { pt: "Ter", en: "Tue", fr: "mar" },
    "date.weekday_short.3": { pt: "Qua", en: "Wed", fr: "mer" },
    "date.weekday_short.4": { pt: "Qui", en: "Thu", fr: "jeu" },
    "date.weekday_short.5": { pt: "Sex", en: "Fri", fr: "ven" },
    "date.weekday_short.6": { pt: "Sáb", en: "Sat", fr: "sam" },
    "date.weekday_short.7": { pt: "Dom", en: "Sun", fr: "dim" },
    "date.month_year": { pt: "{m} {y}", en: "{m} {y}", fr: "{m} {y}" },
    "date.long": { pt: "{d} de {m} de {y}", en: "{d} {m} {y}", fr: "{d} {m} {y}" },
    "date.weekday_long": { pt: "{w}, {d} de {m}", en: "{w}, {d} {m}", fr: "{w} {d} {m}" },
    "date.range.same_month": { pt: "{d1} – {d2} de {m} de {y}", en: "{d1}–{d2} {m} {y}", fr: "{d1} – {d2} {m} {y}" },
    "date.range.same_year": { pt: "{d1} de {m1} – {d2} de {m2} de {y}", en: "{d1} {m1} – {d2} {m2} {y}", fr: "{d1} {m1} – {d2} {m2} {y}" },
    "date.range.cross_year": { pt: "{d1} de {m1} de {y1} – {d2} de {m2} de {y2}", en: "{d1} {m1} {y1} – {d2} {m2} {y2}", fr: "{d1} {m1} {y1} – {d2} {m2} {y2}" },
};

/// O Correio. O conteúdo das mensagens (assunto, corpo, remetentes, endereços)
/// é dado e não se traduz; só o chrome da aplicação.
const MAIL: &[Entry] = catalogo! {
    "mail.title": { pt: "Correio", en: "Mail", fr: "Courrier" },
    "mail.subtitle": { pt: "Correio institucional da Ocinye.", en: "Ocinye institutional mail.", fr: "Courrier institutionnel d’Ocinye." },
    "mail.compose": { pt: "Escrever", en: "Compose", fr: "Écrire" },
    "mail.refresh": { pt: "Actualizar", en: "Refresh", fr: "Actualiser" },
    "mail.settings": { pt: "Definições de correio", en: "Mail settings", fr: "Paramètres du courrier" },
    "mail.mailboxes": { pt: "Caixas de correio", en: "Mailboxes", fr: "Boîtes aux lettres" },
    "mail.folders": { pt: "pastas", en: "folders", fr: "dossiers" },
    "mail.shared": { pt: "Partilhada", en: "Shared", fr: "Partagée" },
    "mail.folder.inbox": { pt: "Caixa de entrada", en: "Inbox", fr: "Boîte de réception" },
    "mail.folder.starred": { pt: "Assinaladas", en: "Starred", fr: "Suivis" },
    "mail.folder.drafts": { pt: "Rascunhos", en: "Drafts", fr: "Brouillons" },
    "mail.folder.sent": { pt: "Enviados", en: "Sent", fr: "Envoyés" },
    "mail.folder.archive": { pt: "Arquivo", en: "Archive", fr: "Archives" },
    "mail.folder.spam": { pt: "Spam", en: "Spam", fr: "Indésirables" },
    "mail.folder.trash": { pt: "Lixo", en: "Trash", fr: "Corbeille" },
    "mail.list.empty.folder": { pt: "Nenhuma mensagem nesta pasta", en: "No messages in this folder", fr: "Aucun message dans ce dossier" },
    "mail.list.empty.folder.body": { pt: "Esta pasta não tem mensagens indexadas no Ocinye OS.", en: "This folder has no messages indexed in Ocinye OS.", fr: "Ce dossier n’a aucun message indexé dans Ocinye OS." },
    "mail.list.empty.search": { pt: "Nenhuma mensagem corresponde", en: "No message matches", fr: "Aucun message ne correspond" },
    "mail.list.empty.search.body": { pt: "Nenhuma mensagem desta caixa corresponde ao termo pesquisado.", en: "No message in this mailbox matches the search term.", fr: "Aucun message de cette boîte ne correspond au terme recherché." },
    "mail.list.empty.none": { pt: "Ainda não há correio para mostrar", en: "No mail to show yet", fr: "Aucun courrier à afficher pour l’instant" },
    "mail.not_linked.title": { pt: "A sua caixa ainda não está ligada", en: "Your mailbox is not linked yet", fr: "Votre boîte n’est pas encore connectée" },
    "mail.not_linked.body": { pt: "A sua caixa de correio ainda não está ligada.", en: "Your mailbox is not linked yet.", fr: "Votre boîte aux lettres n’est pas encore connectée." },
    "mail.no_institutional.title": { pt: "Ainda não há nenhuma caixa institucional associada a si", en: "No institutional mailbox is associated with you yet", fr: "Aucune boîte institutionnelle ne vous est encore associée" },
    "mail.no_institutional.body": { pt: "Ainda não tem uma caixa institucional ligada.", en: "You do not have an institutional mailbox linked yet.", fr: "Vous n’avez pas encore de boîte institutionnelle connectée." },
    "mail.link_my_mailbox": { pt: "Ligar a minha caixa", en: "Link my mailbox", fr: "Connecter ma boîte" },
    "mail.link_mailbox": { pt: "Ligar caixa", en: "Link mailbox", fr: "Connecter la boîte" },
    "mail.star": { pt: "Assinalar", en: "Star", fr: "Suivre" },
    "mail.starred_label": { pt: "Assinalada", en: "Starred", fr: "Suivi" },
    "mail.mark_read": { pt: "Marcar como lida", en: "Mark as read", fr: "Marquer comme lu" },
    "mail.mark_unread": { pt: "Marcar como não lida", en: "Mark as unread", fr: "Marquer comme non lu" },
    "mail.attachments": { pt: "Anexos", en: "Attachments", fr: "Pièces jointes" },
    "mail.attachment.download": { pt: "Descarregar", en: "Download", fr: "Télécharger" },
    "mail.attachment.unavailable": { pt: "A descarga de anexos ainda não está disponível.", en: "Attachment download is not available yet.", fr: "Le téléchargement des pièces jointes n’est pas encore disponible." },
    "mail.links_to": { pt: "Esta mensagem liga para: ", en: "This message links to: ", fr: "Ce message renvoie vers : " },
    "mail.give_screen_to_reading": { pt: "Dar o ecrã à leitura", en: "Give the screen to reading", fr: "Donner l’écran à la lecture" },
    "mail.adjust_folders_width": { pt: "Ajustar a largura das pastas", en: "Adjust folders width", fr: "Ajuster la largeur des dossiers" },
    "mail.adjust_list_width": { pt: "Ajustar a largura da lista", en: "Adjust list width", fr: "Ajuster la largeur de la liste" },
    "mail.layout": { pt: "Disposição do Correio", en: "Mail layout", fr: "Disposition du courrier" },
    "mail.reset_layout": { pt: "Repor a disposição", en: "Reset layout", fr: "Réinitialiser la disposition" },
    // Compositor
    "mail.compose.new": { pt: "Nova mensagem", en: "New message", fr: "Nouveau message" },
    "mail.compose.from": { pt: "De", en: "From", fr: "De" },
    "mail.compose.to": { pt: "Para", en: "To", fr: "À" },
    "mail.compose.recipient_placeholder": { pt: "Nome ou endereço", en: "Name or address", fr: "Nom ou adresse" },
    "mail.compose.cc": { pt: "Cc", en: "Cc", fr: "Cc" },
    "mail.compose.bcc": { pt: "Bcc", en: "Bcc", fr: "Cci" },
    "mail.compose.subject": { pt: "Assunto", en: "Subject", fr: "Objet" },
    "mail.compose.body_placeholder": { pt: "Escreva a mensagem…", en: "Write your message…", fr: "Rédigez votre message…" },
    "mail.compose.send": { pt: "Enviar", en: "Send", fr: "Envoyer" },
    "mail.compose.save_draft": { pt: "Guardar rascunho", en: "Save draft", fr: "Enregistrer le brouillon" },
    "mail.compose.discard": { pt: "Descartar", en: "Discard", fr: "Abandonner" },
    "mail.compose.cancel": { pt: "Cancelar", en: "Cancel", fr: "Annuler" },
    "mail.compose.expand": { pt: "Expandir o compositor", en: "Expand the composer", fr: "Agrandir le compositeur" },
    "mail.compose.expand_short": { pt: "Expandir", en: "Expand", fr: "Agrandir" },
    "mail.compose.close": { pt: "Fechar o compositor", en: "Close the composer", fr: "Fermer le compositeur" },
    "mail.compose.close_short": { pt: "Fechar", en: "Close", fr: "Fermer" },
    "mail.compose.attach": { pt: "Anexar ficheiro", en: "Attach file", fr: "Joindre un fichier" },
    "mail.compose.remove_attachment": { pt: "Retirar anexo", en: "Remove attachment", fr: "Retirer la pièce jointe" },
    "mail.compose.unsent_changes": { pt: "Esta mensagem ainda não foi enviada e contém alterações.", en: "This message has not been sent and contains changes.", fr: "Ce message n’a pas été envoyé et contient des modifications." },
    "mail.compose.save_as_draft_q": { pt: "Guardar esta mensagem como rascunho?", en: "Save this message as a draft?", fr: "Enregistrer ce message comme brouillon ?" },
    "mail.compose.nothing_lost": { pt: "nada se perdeu", en: "nothing is lost", fr: "rien n’est perdu" },
    // Barra de formatação
    "mail.fmt.bold": { pt: "Negrito", en: "Bold", fr: "Gras" },
    "mail.fmt.italic": { pt: "Itálico", en: "Italic", fr: "Italique" },
    "mail.fmt.underline": { pt: "Sublinhado", en: "Underline", fr: "Souligné" },
    "mail.fmt.strike": { pt: "Rasurado", en: "Strikethrough", fr: "Barré" },
    "mail.fmt.list": { pt: "Lista", en: "List", fr: "Liste" },
    "mail.fmt.numbered": { pt: "Lista numerada", en: "Numbered list", fr: "Liste numérotée" },
    "mail.fmt.quote": { pt: "Citação", en: "Quote", fr: "Citation" },
    "mail.fmt.link": { pt: "Ligação", en: "Link", fr: "Lien" },
    "mail.fmt.clear": { pt: "Limpar formatação", en: "Clear formatting", fr: "Effacer la mise en forme" },
    "mail.fmt.toolbar": { pt: "Formatação", en: "Formatting", fr: "Mise en forme" },
    // Assistência
    "mail.ai.title": { pt: "Assistência de escrita", en: "Writing assistance", fr: "Assistance à la rédaction" },
    "mail.ai.unavailable": { pt: "A assistência de escrita não está disponível nesta instalação.", en: "Writing assistance is not available in this installation.", fr: "L’assistance à la rédaction n’est pas disponible dans cette installation." },
    "mail.ai.proofread": { pt: "Corrigir", en: "Proofread", fr: "Corriger" },
    "mail.ai.clarify": { pt: "Mais claro", en: "Clearer", fr: "Plus clair" },
    "mail.ai.formal": { pt: "Mais formal", en: "More formal", fr: "Plus formel" },
    "mail.ai.shorter": { pt: "Mais curto", en: "Shorter", fr: "Plus court" },
    "mail.ai.translate": { pt: "Traduzir", en: "Translate", fr: "Traduire" },
    // Definições de correio
    "mail.settings.title": { pt: "Definições de correio", en: "Mail settings", fr: "Paramètres du courrier" },
    "mail.settings.subtitle": { pt: "As suas preferências e o estado do serviço.", en: "Your preferences and the service status.", fr: "Vos préférences et l’état du service." },
    "mail.settings.your_mailboxes": { pt: "As suas caixas", en: "Your mailboxes", fr: "Vos boîtes" },
    "mail.settings.service_status": { pt: "Estado do serviço", en: "Service status", fr: "État du service" },
    "mail.settings.available": { pt: "Disponível", en: "Available", fr: "Disponible" },
    "mail.settings.unavailable": { pt: "Indisponível", en: "Unavailable", fr: "Indisponible" },
    "mail.settings.linked": { pt: "Ligada", en: "Linked", fr: "Connectée" },
    "mail.settings.adapter": { pt: "Adaptador", en: "Adapter", fr: "Adaptateur" },
    "mail.settings.institutional_email": { pt: "E-mail institucional", en: "Institutional email", fr: "Courriel institutionnel" },
    "mail.settings.institutional_email_short": { pt: "E-mail institucional", en: "Institutional email", fr: "Courriel institutionnel" },
    "mail.settings.password_note": { pt: "A senha de cada caixa é sua, fica cifrada, e nunca volta a ser mostrada.", en: "Each mailbox password is yours, is encrypted, and is never shown again.", fr: "Le mot de passe de chaque boîte est le vôtre, chiffré, et n’est jamais réaffiché." },
    "mail.settings.save_and_link": { pt: "Guardar e ligar", en: "Save and link", fr: "Enregistrer et connecter" },
    "mail.settings.disconnect": { pt: "Desligar e esquecer a senha", en: "Disconnect and forget the password", fr: "Déconnecter et oublier le mot de passe" },
    "mail.settings.signature": { pt: "Assinatura institucional", en: "Institutional signature", fr: "Signature institutionnelle" },
    "mail.settings.signature_added": { pt: "Assinatura institucional — acrescentada ao enviar", en: "Institutional signature — added when sending", fr: "Signature institutionnelle — ajoutée à l’envoi" },
    "mail.settings.personal_line": { pt: "Linha pessoal (opcional)", en: "Personal line (optional)", fr: "Ligne personnelle (facultative)" },
    "mail.settings.remote_content": { pt: "Conteúdo remoto", en: "Remote content", fr: "Contenu distant" },
    "mail.settings.remote_content.allowlist": { pt: "Carregar de remetentes que eu permitir", en: "Load from senders I allow", fr: "Charger des expéditeurs que j’autorise" },
    "mail.settings.save": { pt: "Guardar", en: "Save", fr: "Enregistrer" },
    "mail.reply": { pt: "Responder", en: "Reply", fr: "Répondre" },
    "mail.unstar": { pt: "Retirar destaque", en: "Unstar", fr: "Ne plus suivre" },
    "mail.select_message": { pt: "Seleccione uma mensagem", en: "Select a message", fr: "Sélectionnez un message" },
    "mail.message": { pt: "Mensagem", en: "Message", fr: "Message" },
    "mail.unread_badge": { pt: "Não lida", en: "Unread", fr: "Non lu" },
    "mail.has_attachments": { pt: "Tem anexos", en: "Has attachments", fr: "Contient des pièces jointes" },
    "mail.no_messages": { pt: "Não tem mensagens", en: "No messages", fr: "Aucun message" },
    "mail.collapse_folders": { pt: "Recolher as pastas", en: "Collapse folders", fr: "Réduire les dossiers" },
    "mail.search_this_box": { pt: "Pesquisar nesta caixa", en: "Search this mailbox", fr: "Rechercher dans cette boîte" },
    "mail.search_this_box.placeholder": { pt: "Pesquisar nesta caixa…", en: "Search this mailbox…", fr: "Rechercher dans cette boîte…" },
    "mail.by_linking": { pt: "Por ligar", en: "Not linked", fr: "À connecter" },
    "mail.back_to_mail": { pt: "Voltar ao correio", en: "Back to mail", fr: "Retour au courrier" },
    "mail.no_send_permission": { pt: "Não possui autorização para enviar a partir desta caixa.", en: "You are not authorised to send from this mailbox.", fr: "Vous n’êtes pas autorisé à envoyer depuis cette boîte." },
    "mail.no_mailbox_to_send": { pt: "Não tem nenhuma caixa a partir da qual possa enviar.", en: "You have no mailbox to send from.", fr: "Vous n’avez aucune boîte pour envoyer." },
    "mail.remove_attachment_named": { pt: "Retirar {file}", en: "Remove {file}", fr: "Retirer {file}" },
    "mail.settings.servers": { pt: "Servidores", en: "Servers", fr: "Serveurs" },
    "mail.settings.password": { pt: "Senha da caixa", en: "Mailbox password", fr: "Mot de passe de la boîte" },
    "mail.settings.password_app": { pt: "Palavra-passe da caixa (ou App Password)", en: "Mailbox password (or App Password)", fr: "Mot de passe de la boîte (ou mot de passe d’application)" },
    "mail.settings.preferences": { pt: "Preferências", en: "Preferences", fr: "Préférences" },
    "mail.settings.reset_layout": { pt: "Repor disposição", en: "Reset layout", fr: "Réinitialiser la disposition" },
    "mail.settings.reset_done": { pt: "Reposta. Volte ao Correio para a ver.", en: "Reset. Go back to Mail to see it.", fr: "Réinitialisée. Retournez au Courrier pour la voir." },
    "mail.settings.use_official_signature": { pt: "Utilizar a assinatura institucional oficial", en: "Use the official institutional signature", fr: "Utiliser la signature institutionnelle officielle" },
    "mail.settings.personal_line_hint": { pt: "Uma linha sua, acrescentada acima da assinatura oficial.", en: "A line of your own, added above the official signature.", fr: "Une ligne à vous, ajoutée au-dessus de la signature officielle." },
    "mail.settings.remote.never": { pt: "Nunca carregar", en: "Never load", fr: "Ne jamais charger" },
    "mail.settings.remote.ask": { pt: "Perguntar em cada mensagem", en: "Ask on each message", fr: "Demander à chaque message" },
    "mail.not_configured": { pt: "O correio institucional não está configurado", en: "Institutional mail is not configured", fr: "Le courrier institutionnel n’est pas configuré" },
    "mail.not_configured.body": { pt: "O correio institucional não está configurado nesta instalação.", en: "Institutional mail is not configured in this installation.", fr: "Le courrier institutionnel n’est pas configuré dans cette installation." },
    "mail.not_configured.short": { pt: "O correio institucional ainda não foi configurado.", en: "Institutional mail has not been configured yet.", fr: "Le courrier institutionnel n’a pas encore été configuré." },
    "mail.service_down": { pt: "O serviço de correio não está a responder", en: "The mail service is not responding", fr: "Le service de courrier ne répond pas" },
    "mail.service_down.body": { pt: "O serviço de correio não está a responder nesta instalação.", en: "The mail service is not responding in this installation.", fr: "Le service de courrier ne répond pas dans cette installation." },
    "mail.service_unavailable": { pt: "O serviço de correio não está disponível.", en: "The mail service is not available.", fr: "Le service de courrier n’est pas disponible." },
    "mail.send_unavailable": { pt: "O serviço de envio não está disponível.", en: "The sending service is not available.", fr: "Le service d’envoi n’est pas disponible." },
    "mail.header_subtitle": { pt: "O correio institucional da Ocinye, dentro do Ocinye Workspace.", en: "Ocinye institutional mail, inside the Ocinye Workspace.", fr: "Le courrier institutionnel d’Ocinye, au sein de l’Ocinye Workspace." },
    "mail.reader.to": { pt: "Para: ", en: "To: ", fr: "À : " },
    "mail.reader.cc": { pt: "Cc: ", en: "Cc: ", fr: "Cc : " },
    "mail.ai.why": { pt: "Porquê", en: "Why", fr: "Pourquoi" },
    "mail.settings.reading": { pt: "Leitura", en: "Reading", fr: "Lecture" },
    "mail.settings.sending": { pt: "Envio", en: "Sending", fr: "Envoi" },
    "mail.search_submit": { pt: "Pesquisar", en: "Search", fr: "Rechercher" },
};

/// Os Ficheiros. Nomes de ficheiro e de pasta, e o conteúdo, são do membro e não
/// se traduzem; só o chrome.
const FILES: &[Entry] = catalogo! {
    "files.title": { pt: "Ficheiros", en: "Files", fr: "Fichiers" },
    "files.my_files": { pt: "Meus ficheiros", en: "My files", fr: "Mes fichiers" },
    "files.my_files_root": { pt: "Meus ficheiros (raiz)", en: "My files (root)", fr: "Mes fichiers (racine)" },
    "files.new_folder": { pt: "Nova pasta", en: "New folder", fr: "Nouveau dossier" },
    "files.create_folder": { pt: "Criar pasta", en: "Create folder", fr: "Créer un dossier" },
    "files.folder_name": { pt: "Nome da pasta", en: "Folder name", fr: "Nom du dossier" },
    "files.folder_name_placeholder": { pt: "Nome da pasta…", en: "Folder name…", fr: "Nom du dossier…" },
    "files.new_folder_name": { pt: "Novo nome da pasta", en: "New folder name", fr: "Nouveau nom du dossier" },
    "files.rename": { pt: "Mudar nome", en: "Rename", fr: "Renommer" },
    "files.new_name": { pt: "Novo nome", en: "New name", fr: "Nouveau nom" },
    "files.delete_folder": { pt: "Eliminar pasta", en: "Delete folder", fr: "Supprimer le dossier" },
    "files.delete_permanently": { pt: "Eliminar definitivamente", en: "Delete permanently", fr: "Supprimer définitivement" },
    "files.move_to": { pt: "Mover para", en: "Move to", fr: "Déplacer vers" },
    "files.move_selection_to": { pt: "Mover a selecção para", en: "Move selection to", fr: "Déplacer la sélection vers" },
    "files.file_actions": { pt: "Acções do ficheiro", en: "File actions", fr: "Actions du fichier" },
    "files.filter": { pt: "Filtrar ficheiros", en: "Filter files", fr: "Filtrer les fichiers" },
    "files.filter_versions": { pt: "Filtrar versões", en: "Filter versions", fr: "Filtrer les versions" },
    "files.favourite": { pt: "Marcar como favorito", en: "Mark as favourite", fr: "Marquer comme favori" },
    "files.unfavourite": { pt: "Remover dos favoritos", en: "Remove from favourites", fr: "Retirer des favoris" },
    "files.grid_or_list": { pt: "Grelha ou lista", en: "Grid or list", fr: "Grille ou liste" },
    "files.toggle_grid_list": { pt: "Alternar entre grelha e lista", en: "Toggle grid and list", fr: "Basculer entre grille et liste" },
    "files.presentation": { pt: "Apresentação", en: "Presentation", fr: "Présentation" },
    "files.upload_new_version": { pt: "Carregar nova versão", en: "Upload new version", fr: "Téléverser une nouvelle version" },
    "files.upload_version": { pt: "Carregar versão", en: "Upload version", fr: "Téléverser la version" },
    "files.upload_destination": { pt: "Destino do carregamento", en: "Upload destination", fr: "Destination du téléversement" },
    "files.drop_here": { pt: "Largue ficheiros aqui", en: "Drop files here", fr: "Déposez des fichiers ici" },
    "files.or_choose": { pt: "ou escolha do computador. Ficam nesta pasta.", en: "or choose from your computer. They go in this folder.", fr: "ou choisissez depuis votre ordinateur. Ils vont dans ce dossier." },
    "files.select_named": { pt: "Seleccionar {name}", en: "Select {name}", fr: "Sélectionner {name}" },
    "files.version_history": { pt: "Histórico de versões", en: "Version history", fr: "Historique des versions" },
    "files.version": { pt: "Versão", en: "Version", fr: "Version" },
    "files.versions": { pt: "Versões", en: "Versions", fr: "Versions" },
    "files.no_versions": { pt: "Este ficheiro ainda não tem versões.", en: "This file has no versions yet.", fr: "Ce fichier n’a pas encore de versions." },
    "files.also_current": { pt: "É também a versão corrente deste ficheiro.", en: "It is also the current version of this file.", fr: "C’est aussi la version actuelle de ce fichier." },
    "files.sha256": { pt: "Soma SHA-256", en: "SHA-256 checksum", fr: "Somme SHA-256" },
    "files.location": { pt: "Localização", en: "Location", fr: "Emplacement" },
    "files.classification": { pt: "Classificação", en: "Classification", fr: "Classification" },
    "files.classification.environment": { pt: "Classificação do ambiente", en: "Environment classification", fr: "Classification de l’environnement" },
    "files.classification.effective": { pt: "Classificação efectiva", en: "Effective classification", fr: "Classification effective" },
    "files.classification.inherit": { pt: "Herdar do ambiente", en: "Inherit from environment", fr: "Hériter de l’environnement" },
    "files.content": { pt: "Conteúdo", en: "Content", fr: "Contenu" },
    "files.content.searchable": { pt: "Pesquisável", en: "Searchable", fr: "Recherchable" },
    "files.content.searchable_note": { pt: "O conteúdo deste ficheiro pode ser encontrado por pesquisa.", en: "This file’s content can be found by search.", fr: "Le contenu de ce fichier peut être trouvé par recherche." },
    "files.content.not_searchable": { pt: "Conteúdo não pesquisável", en: "Content not searchable", fr: "Contenu non recherchable" },
    "files.content.not_analysed": { pt: "Conteúdo não analisado", en: "Content not analysed", fr: "Contenu non analysé" },
    "files.content.unreadable": { pt: "Não foi possível ler o conteúdo", en: "The content could not be read", fr: "Le contenu n’a pas pu être lu" },
    "files.preview": { pt: "Pré-visualização do ficheiro", en: "File preview", fr: "Aperçu du fichier" },
    "files.preview.none_for": { pt: "Sem pré-visualização para ", en: "No preview for ", fr: "Aucun aperçu pour " },
    "files.preview.too_big": { pt: "Grande de mais para mostrar aqui", en: "Too big to show here", fr: "Trop volumineux pour l’afficher ici" },
    "files.preview.too_big_note": { pt: "São {size}. Descarregue o ficheiro para o ver inteiro.", en: "It is {size}. Download the file to see it in full.", fr: "Il fait {size}. Téléchargez le fichier pour le voir en entier." },
    "files.environments": { pt: "Ambientes de investigação", en: "Research environments", fr: "Environnements de recherche" },
    "files.open_environment": { pt: "Abrir ambiente", en: "Open environment", fr: "Ouvrir l’environnement" },
    "files.choose_environment": { pt: "Escolha o ambiente cujos ficheiros quer ver.", en: "Choose the environment whose files you want to see.", fr: "Choisissez l’environnement dont vous voulez voir les fichiers." },
    "files.choose_environment_upload": { pt: "Escolha um ambiente acima para carregar o primeiro.", en: "Choose an environment above to upload the first one.", fr: "Choisissez un environnement ci-dessus pour téléverser le premier." },
    "files.institutional_of": { pt: "Os ficheiros institucionais de ", en: "The institutional files of ", fr: "Les fichiers institutionnels de " },
    "files.institutional_in": { pt: "Ficheiro institucional em ", en: "Institutional file in ", fr: "Fichier institutionnel dans " },
    "files.empty.folder": { pt: "Esta pasta está vazia", en: "This folder is empty", fr: "Ce dossier est vide" },
    "files.empty.folder.body": { pt: "Carregue um ficheiro ou crie uma pasta para começar.", en: "Upload a file or create a folder to begin.", fr: "Téléversez un fichier ou créez un dossier pour commencer." },
    "files.empty.nothing_here": { pt: "Ainda não há nada aqui", en: "Nothing here yet", fr: "Rien ici pour l’instant" },
    "files.empty.favourites": { pt: "Ainda não há favoritos", en: "No favourites yet", fr: "Aucun favori pour l’instant" },
    "files.empty.favourites.body": { pt: "Marque um ficheiro como favorito para o encontrar aqui.", en: "Mark a file as a favourite to find it here.", fr: "Marquez un fichier comme favori pour le retrouver ici." },
    "files.empty.recent": { pt: "Ainda não há nada recente", en: "Nothing recent yet", fr: "Rien de récent pour l’instant" },
    "files.empty.recent.body": { pt: "Os ficheiros que carregar ou mexer aparecem aqui.", en: "Files you upload or touch appear here.", fr: "Les fichiers que vous téléversez ou modifiez apparaissent ici." },
    "files.empty.environments": { pt: "Ainda não há ficheiros nos seus ambientes", en: "No files in your environments yet", fr: "Aucun fichier dans vos environnements pour l’instant" },
    "files.empty.no_environment": { pt: "Não alcança nenhum ambiente", en: "You reach no environment", fr: "Vous n’atteignez aucun environnement" },
    "files.empty.no_upload": { pt: "Não tem onde carregar ficheiros", en: "You have nowhere to upload files", fr: "Vous n’avez nulle part où téléverser des fichiers" },
    "files.empty.none_accessible": { pt: "Nenhum ficheiro acessível.", en: "No accessible file.", fr: "Aucun fichier accessible." },
    "files.trash.empty": { pt: "O Lixo está vazio", en: "The Trash is empty", fr: "La corbeille est vide" },
    "files.trash.empty.body": { pt: "Os ficheiros que apagar aparecem aqui, e pode restaurá-los.", en: "Files you delete appear here, and you can restore them.", fr: "Les fichiers que vous supprimez apparaissent ici, et vous pouvez les restaurer." },
    // Tipos de ficheiro (rótulos de MIME)
    "files.type.public": { pt: "Público", en: "Public", fr: "Public" },
    "files.type.spreadsheet": { pt: "Folha de cálculo", en: "Spreadsheet", fr: "Feuille de calcul" },
    "files.type.video": { pt: "Vídeo", en: "Video", fr: "Vidéo" },
    "files.type.image_png": { pt: "Imagem PNG", en: "PNG image", fr: "Image PNG" },
    "files.type.image_jpeg": { pt: "Imagem JPEG", en: "JPEG image", fr: "Image JPEG" },
    "files.type.image_gif": { pt: "Imagem GIF", en: "GIF image", fr: "Image GIF" },
    "files.type.image_svg": { pt: "Imagem SVG", en: "SVG image", fr: "Image SVG" },
    "files.type.image_webp": { pt: "Imagem WebP", en: "WebP image", fr: "Image WebP" },
    "files.version_n": { pt: "versão {n}", en: "version {n}", fr: "version {n}" },
    "files.version_n_page": { pt: "versão {n} · página {p}", en: "version {n} · page {p}", fr: "version {n} · page {p}" },
    "files.tab.favourites": { pt: "Favoritos", en: "Favourites", fr: "Favoris" },
    "files.tab.recent": { pt: "Recentes", en: "Recent", fr: "Récents" },
    "files.tab.trash": { pt: "Lixo", en: "Trash", fr: "Corbeille" },
    "files.upload": { pt: "Carregar", en: "Upload", fr: "Téléverser" },
    "files.close": { pt: "Fechar", en: "Close", fr: "Fermer" },
    "files.restore": { pt: "Restaurar", en: "Restore", fr: "Restaurer" },
    "files.move": { pt: "Mover", en: "Move", fr: "Déplacer" },
    "files.delete": { pt: "Eliminar", en: "Delete", fr: "Supprimer" },
    "files.folder_pill": { pt: "Pasta", en: "Folder", fr: "Dossier" },
    "files.favourite_star": { pt: "Favorito", en: "Favourite", fr: "Favori" },
    "files.open": { pt: "Abrir", en: "Open", fr: "Ouvrir" },
    "files.environment": { pt: "Ambiente", en: "Environment", fr: "Environnement" },
    "files.view": { pt: "Ver", en: "View", fr: "Voir" },
    "files.folders_aria": { pt: "Pastas", en: "Folders", fr: "Dossiers" },
    "files.file_label": { pt: "Ficheiro", en: "File", fr: "Fichier" },
    "files.class_option.public": { pt: "Público", en: "Public", fr: "Public" },
    "files.class_option.internal": { pt: "Interno", en: "Internal", fr: "Interne" },
    "files.class_option.confidential": { pt: "Confidencial", en: "Confidential", fr: "Confidentiel" },
    "files.class_option.restricted": { pt: "Restrito", en: "Restricted", fr: "Restreint" },
};

/// O Calendário e o painel de notificações.
const CALENDAR: &[Entry] = catalogo! {
    "calendar.title": { pt: "Calendário", en: "Calendar", fr: "Calendrier" },
    "calendar.open": { pt: "Abrir Calendário", en: "Open Calendar", fr: "Ouvrir le calendrier" },
    "calendar.system": { pt: "Calendário do sistema", en: "System calendar", fr: "Calendrier du système" },
    "calendar.views": { pt: "Vistas do calendário", en: "Calendar views", fr: "Vues du calendrier" },
    "calendar.new_activity": { pt: "Nova actividade", en: "New activity", fr: "Nouvelle activité" },
    "calendar.create_activity": { pt: "Criar actividade", en: "Create activity", fr: "Créer une activité" },
    "calendar.edit_activity": { pt: "Alterar actividade", en: "Edit activity", fr: "Modifier l’activité" },
    "calendar.save_changes": { pt: "Guardar alterações", en: "Save changes", fr: "Enregistrer les modifications" },
    "calendar.field.title": { pt: "Título", en: "Title", fr: "Titre" },
    "calendar.field.description": { pt: "Descrição", en: "Description", fr: "Description" },
    "calendar.field.location": { pt: "Localização", en: "Location", fr: "Lieu" },
    "calendar.field.location_placeholder": { pt: "Sala, edifício ou ligação", en: "Room, building or link", fr: "Salle, bâtiment ou lien" },
    "calendar.field.classification": { pt: "Classificação", en: "Classification", fr: "Classification" },
    "calendar.field.timezone": { pt: "Zona horária", en: "Timezone", fr: "Fuseau horaire" },
    "calendar.field.start": { pt: "Início", en: "Start", fr: "Début" },
    "calendar.all_day": { pt: "Dia inteiro", en: "All day", fr: "Toute la journée" },
    "calendar.all_day_days": { pt: "Dia inteiro · {days} dias", en: "All day · {days} days", fr: "Toute la journée · {days} jours" },
    "calendar.belongs.environment": { pt: "Ambiente de investigação", en: "Research environment", fr: "Environnement de recherche" },
    "calendar.belongs.institution": { pt: "Instituição", en: "Institution", fr: "Institution" },
    "calendar.belongs_to": { pt: "Pertence a", en: "Belongs to", fr: "Appartient à" },
    "calendar.empty": { pt: "Nenhuma actividade", en: "No activity", fr: "Aucune activité" },
    "calendar.empty.period": { pt: "Nenhuma actividade para este período", en: "No activity for this period", fr: "Aucune activité pour cette période" },
    "calendar.empty.period.dot": { pt: "Nenhuma actividade para este período.", en: "No activity for this period.", fr: "Aucune activité pour cette période." },
    "calendar.cancelled_note": { pt: "Esta actividade foi cancelada. Fica visível para quem a esperava.", en: "This activity was cancelled. It stays visible to those who expected it.", fr: "Cette activité a été annulée. Elle reste visible pour ceux qui l’attendaient." },
    "calendar.unreadable": { pt: "Não foi possível ler a agenda.", en: "The calendar could not be read.", fr: "L’agenda n’a pas pu être lu." },
    "calendar.next_90_days": { pt: "Próximos 90 dias", en: "Next 90 days", fr: "90 prochains jours" },
    "calendar.search_person": { pt: "Procurar uma pessoa", en: "Search for a person", fr: "Rechercher une personne" },
    "calendar.person_placeholder": { pt: "Nome ou endereço institucional", en: "Name or institutional address", fr: "Nom ou adresse institutionnelle" },
    "calendar.no_person_match": { pt: "Ninguém corresponde a essa procura", en: "No one matches that search", fr: "Personne ne correspond à cette recherche" },
    "calendar.no_person_match.dot": { pt: "Ninguém corresponde a essa procura.", en: "No one matches that search.", fr: "Personne ne correspond à cette recherche." },
    "calendar.view_more": { pt: "Ver {n}", en: "View {n}", fr: "Voir {n}" },
    "calendar.grid_aria": { pt: "Mês", en: "Month", fr: "Mois" },
    "calendar.field.title_placeholder": { pt: "Reunião do conselho", en: "Board meeting", fr: "Réunion du conseil" },
    "calendar.section.when": { pt: "Quando", en: "When", fr: "Quand" },
    "calendar.field.end": { pt: "Fim", en: "End", fr: "Fin" },
    "calendar.field.first_day": { pt: "Primeiro dia", en: "First day", fr: "Premier jour" },
    "calendar.field.last_day": { pt: "Último dia", en: "Last day", fr: "Dernier jour" },
    "calendar.section.participants": { pt: "Participantes", en: "Participants", fr: "Participants" },
    "calendar.field.scope": { pt: "Âmbito", en: "Scope", fr: "Portée" },
    "calendar.scope.personal": { pt: "Pessoal", en: "Personal", fr: "Personnel" },
    "calendar.scope.unit": { pt: "Unidade", en: "Unit", fr: "Unité" },
    "calendar.edit": { pt: "Alterar", en: "Edit", fr: "Modifier" },
    "calendar.field.local": { pt: "Local", en: "Location", fr: "Lieu" },
    "calendar.cancel": { pt: "Cancelar", en: "Cancel", fr: "Annuler" },
    "calendar.open_item": { pt: "Abrir", en: "Open", fr: "Ouvrir" },
    "notifications.title": { pt: "Notificações", en: "Notifications", fr: "Notifications" },
    "notifications.unread": { pt: "Por ler", en: "Unread", fr: "Non lues" },
    "notifications.nothing_unread": { pt: "Nada por ler.", en: "Nothing unread.", fr: "Rien à lire." },
    "notifications.mark_read": { pt: "Marcar como lida", en: "Mark as read", fr: "Marquer comme lue" },
    "notifications.empty": { pt: "Ainda não há notificações.", en: "No notifications yet.", fr: "Aucune notification pour l’instant." },
    "notifications.unreadable": { pt: "Não foi possível ler as notificações.", en: "Notifications could not be read.", fr: "Les notifications n’ont pas pu être lues." },
};

/// As Mensagens internas. O texto das mensagens é do membro e não se traduz.
const MESSAGING: &[Entry] = catalogo! {
    "messaging.title": { pt: "Mensagens", en: "Messages", fr: "Messages" },
    "messaging.new_conversation": { pt: "Nova conversa", en: "New conversation", fr: "Nouvelle conversation" },
    "messaging.start_conversation": { pt: "Comece uma conversa", en: "Start a conversation", fr: "Démarrez une conversation" },
    "messaging.start": { pt: "Começar", en: "Start", fr: "Démarrer" },
    "messaging.choose_conversation": { pt: "Escolha uma conversa", en: "Choose a conversation", fr: "Choisissez une conversation" },
    "messaging.none_yet": { pt: "Ainda não falou com ninguém por aqui.", en: "You have not talked to anyone here yet.", fr: "Vous n’avez encore parlé à personne ici." },
    "messaging.no_messages_yet": { pt: "Ainda não há mensagens. Escreva a primeira.", en: "No messages yet. Write the first one.", fr: "Aucun message pour l’instant. Écrivez le premier." },
    "messaging.unreadable": { pt: "Não foi possível ler as conversas", en: "Conversations could not be read", fr: "Les conversations n’ont pas pu être lues" },
    "messaging.write": { pt: "Escrever mensagem", en: "Write a message", fr: "Écrire un message" },
    "messaging.write_placeholder": { pt: "Escrever mensagem…", en: "Write a message…", fr: "Écrire un message…" },
    "messaging.new_messages": { pt: "Novas mensagens", en: "New messages", fr: "Nouveaux messages" },
    "messaging.conversation_type": { pt: "Tipo de conversa", en: "Conversation type", fr: "Type de conversation" },
    "messaging.with_one_person": { pt: "Com uma pessoa", en: "With one person", fr: "Avec une personne" },
    "messaging.group_name": { pt: "Nome do grupo", en: "Group name", fr: "Nom du groupe" },
    "messaging.add_someone": { pt: "Acrescentar alguém", en: "Add someone", fr: "Ajouter quelqu’un" },
    "messaging.leave_group": { pt: "Sair do grupo", en: "Leave group", fr: "Quitter le groupe" },
    "messaging.stop_replying": { pt: "Deixar de responder", en: "Stop replying", fr: "Ne plus répondre" },
    "messaging.search_person": { pt: "Procurar uma pessoa", en: "Search for a person", fr: "Rechercher une personne" },
    "messaging.person_placeholder": { pt: "Nome ou endereço institucional…", en: "Name or institutional address…", fr: "Nom ou adresse institutionnelle…" },
    "messaging.min_two_letters": { pt: "Escreva pelo menos duas letras.", en: "Type at least two letters.", fr: "Saisissez au moins deux lettres." },
    "messaging.copy_text": { pt: "Copiar o texto", en: "Copy the text", fr: "Copier le texte" },
    "messaging.choose_emoji": { pt: "Escolher um emoji", en: "Choose an emoji", fr: "Choisir un emoji" },
    "messaging.ask_ocinye": { pt: "Pedir ajuda ao Ocinye", en: "Ask Ocinye for help", fr: "Demander de l’aide à Ocinye" },
    "messaging.suggestion": { pt: "Sugestão", en: "Suggestion", fr: "Suggestion" },
    "messaging.use_suggestion": { pt: "Usar sugestão", en: "Use suggestion", fr: "Utiliser la suggestion" },
    "messaging.keep_original": { pt: "Manter o original", en: "Keep the original", fr: "Garder l’original" },
    "messaging.ai.clarify": { pt: "Mais claro", en: "Clearer", fr: "Plus clair" },
    "messaging.ai.shorter": { pt: "Mais curto", en: "Shorter", fr: "Plus court" },
    "messaging.ai.formal": { pt: "Mais formal", en: "More formal", fr: "Plus formel" },
    "messaging.remove_named": { pt: "Retirar {name}", en: "Remove {name}", fr: "Retirer {name}" },
    "messaging.emoji.attention": { pt: "atenção", en: "attention", fr: "attention" },
    "messaging.emoji.heart": { pt: "coração", en: "heart", fr: "cœur" },
    "messaging.someone": { pt: "Alguém", en: "Someone", fr: "Quelqu’un" },
    "messaging.conversations_aria": { pt: "Conversas", en: "Conversations", fr: "Conversations" },
    "messaging.conversation_aria": { pt: "Conversa", en: "Conversation", fr: "Conversation" },
    "messaging.close": { pt: "Fechar", en: "Close", fr: "Fermer" },
    "messaging.people_aria": { pt: "Pessoas", en: "People", fr: "Personnes" },
    "messaging.participants": { pt: "Participantes", en: "Participants", fr: "Participants" },
    "messaging.reply": { pt: "Responder", en: "Reply", fr: "Répondre" },
    "messaging.react": { pt: "Reagir", en: "React", fr: "Réagir" },
    "messaging.emoji_aria": { pt: "Emoji", en: "Emoji", fr: "Emoji" },
};

/// As superfícies anteriores à sessão: arranque, login, primeiro acesso, MFA.
const AUTH: &[Entry] = catalogo! {
    "auth.system_status": { pt: "Estado do Sistema", en: "System status", fr: "État du système" },
    "auth.sign_out": { pt: "Terminar sessão", en: "Sign out", fr: "Se déconnecter" },
    // Login
    "login.institutional_session": { pt: "Sessão institucional", en: "Institutional session", fr: "Session institutionnelle" },
    "login.sign_in": { pt: "Iniciar sessão", en: "Sign in", fr: "Se connecter" },
    "login.institutional_address": { pt: "Endereço institucional", en: "Institutional address", fr: "Adresse institutionnelle" },
    "login.create_account": { pt: "Criar conta", en: "Create account", fr: "Créer un compte" },
    "login.granted_by_admin": { pt: "Acesso concedido pela Administração da Ocinye", en: "Access granted by Ocinye Administration", fr: "Accès accordé par l’administration d’Ocinye" },
    "login.unavailable": { pt: "não está acessível", en: "is not reachable", fr: "n’est pas accessible" },
    "login.core_down_note": { pt: "O Ocinye Core não está acessível neste momento. A autenticação não é possível até que esteja.", en: "Ocinye Core is not reachable right now. Authentication is not possible until it is.", fr: "Ocinye Core n’est pas accessible pour l’instant. L’authentification n’est pas possible tant qu’il ne l’est pas." },
    "login.foot.shut_down": { pt: "Desligar", en: "Shut down", fr: "Éteindre" },
    "login.foot.restart": { pt: "Reiniciar", en: "Restart", fr: "Redémarrer" },
    "login.foot.system_status": { pt: "Estado do Sistema", en: "System status", fr: "État du système" },
    // Boot
    "boot.asking_core": { pt: "A perguntar ao Ocinye Core.", en: "Asking Ocinye Core.", fr: "Interrogation d’Ocinye Core." },
    "boot.retry": { pt: "Tentar novamente", en: "Try again", fr: "Réessayer" },
    // First access
    "first_access.set_password": { pt: "Defina a sua palavra-passe", en: "Set your password", fr: "Définissez votre mot de passe" },
    "first_access.set_password_button": { pt: "Definir palavra-passe", en: "Set password", fr: "Définir le mot de passe" },
    "first_access.new_password": { pt: "Nova palavra-passe", en: "New password", fr: "Nouveau mot de passe" },
    "first_access.confirm_password": { pt: "Confirmar nova palavra-passe", en: "Confirm new password", fr: "Confirmer le nouveau mot de passe" },
    "first_access.min_length": { pt: "Mínimo de {min} caracteres", en: "At least {min} characters", fr: "Au moins {min} caractères" },
    "first_access.min_length_dot": { pt: "Mínimo de {min} caracteres.", en: "At least {min} characters.", fr: "Au moins {min} caractères." },
    "first_access.too_common": { pt: "Esta palavra-passe é demasiado comum.", en: "This password is too common.", fr: "Ce mot de passe est trop courant." },
    "first_access.long_phrases": { pt: "Frases longas são aceites, com espaços e acentos.", en: "Long phrases are accepted, with spaces and accents.", fr: "Les phrases longues sont acceptées, avec espaces et accents." },
    "first_access.no_symbols_required": { pt: "Não são exigidos símbolos nem maiúsculas.", en: "No symbols or capitals are required.", fr: "Aucun symbole ni majuscule n’est exigé." },
    "first_access.common_rejected": { pt: "Palavras-passe comuns ou previsíveis são recusadas.", en: "Common or predictable passwords are rejected.", fr: "Les mots de passe courants ou prévisibles sont refusés." },
    // MFA
    "mfa.setup": { pt: "Configurar o segundo factor", en: "Set up two-factor", fr: "Configurer la double authentification" },
    "mfa.confirm": { pt: "Confirme o segundo factor", en: "Confirm two-factor", fr: "Confirmez la double authentification" },
    "mfa.qr_alt": { pt: "Código QR de configuração do segundo factor", en: "Two-factor setup QR code", fr: "QR code de configuration de la double authentification" },
    "mfa.manual_key": { pt: "Chave manual", en: "Manual key", fr: "Clé manuelle" },
    "mfa.show_manual_key": { pt: "Mostrar chave manual", en: "Show manual key", fr: "Afficher la clé manuelle" },
    "mfa.authenticator_code": { pt: "Código do autenticador", en: "Authenticator code", fr: "Code de l’authentificateur" },
    "mfa.six_digit_code": { pt: "Código de seis dígitos", en: "Six-digit code", fr: "Code à six chiffres" },
    "mfa.recovery_code": { pt: "Código de recuperação", en: "Recovery code", fr: "Code de récupération" },
    "mfa.enter_with_recovery": { pt: "Entrar com código de recuperação", en: "Sign in with a recovery code", fr: "Se connecter avec un code de récupération" },
    "mfa.no_authenticator": { pt: "Não tenho o autenticador à mão", en: "I don’t have my authenticator", fr: "Je n’ai pas mon authentificateur" },
    "mfa.recovery_codes": { pt: "Guardar códigos de recuperação", en: "Save recovery codes", fr: "Enregistrer les codes de récupération" },
    "mfa.copy_codes": { pt: "Copiar códigos", en: "Copy codes", fr: "Copier les codes" },
    "mfa.save_file": { pt: "Guardar ficheiro", en: "Save file", fr: "Enregistrer le fichier" },
    "mfa.shown_once": { pt: "Mostrados uma única vez", en: "Shown only once", fr: "Affichés une seule fois" },
    "mfa.saved_confirm": { pt: "Guardei os códigos de recuperação num local seguro.", en: "I have saved the recovery codes in a safe place.", fr: "J’ai enregistré les codes de récupération en lieu sûr." },
    "auth.password": { pt: "Palavra-passe", en: "Password", fr: "Mot de passe" },
    "first_access.eyebrow": { pt: "OCINYE CORE · PRIMEIRO ACESSO", en: "OCINYE CORE · FIRST ACCESS", fr: "OCINYE CORE · PREMIER ACCÈS" },
    "first_access.wordmark_sub": { pt: "PRIMEIRO ACESSO", en: "FIRST ACCESS", fr: "PREMIER ACCÈS" },
    "mfa.wordmark_sub": { pt: "SEGUNDO FACTOR", en: "SECOND FACTOR", fr: "SECOND FACTEUR" },
    // «Mostrar»: revelar a palavra-passe que se está a definir.
    "first_access.show": { pt: "Mostrar", en: "Show", fr: "Afficher" },
    "mfa.confirm_button": { pt: "Confirmar", en: "Confirm", fr: "Confirmer" },
    "mfa.copy_short": { pt: "Copiar", en: "Copy", fr: "Copier" },
    "mfa.finish": { pt: "Concluir", en: "Finish", fr: "Terminer" },
    "mfa.sign_in": { pt: "Entrar", en: "Sign in", fr: "Se connecter" },
    "mfa.frame.challenge": { pt: "OCINYE CORE · SEGUNDO FACTOR", en: "OCINYE CORE · SECOND FACTOR", fr: "OCINYE CORE · SECOND FACTEUR" },
    "mfa.enroll_lead": { pt: "Abra a sua aplicação autenticadora e leia o código. Depois escreva o código de seis dígitos que ela mostrar.", en: "Open your authenticator app and scan the code. Then type the six-digit code it shows.", fr: "Ouvrez votre application d’authentification et scannez le code. Saisissez ensuite le code à six chiffres qu’elle affiche." },
    "mfa.manual_key_note": { pt: "Introduza esta chave na aplicação autenticadora, com o tipo «baseada em tempo» (TOTP).", en: "Enter this key into the authenticator app, with the “time-based” type (TOTP).", fr: "Saisissez cette clé dans l’application d’authentification, avec le type « basé sur le temps » (TOTP)." },
    "mfa.if_cannot_read_qr": { pt: " — se não puder ler o QR.", en: " — if you cannot read the QR.", fr: " — si vous ne pouvez pas lire le QR." },
    // Barra de estado do Core no login (aspecto de terminal, em maiúsculas).
    "login.core.operational": { pt: "OCINYE CORE · OPERACIONAL", en: "OCINYE CORE · OPERATIONAL", fr: "OCINYE CORE · OPÉRATIONNEL" },
    "login.core.unavailable": { pt: "OCINYE CORE · INDISPONÍVEL", en: "OCINYE CORE · UNAVAILABLE", fr: "OCINYE CORE · INDISPONIBLE" },
    // Molduras dos ecrãs de MFA (mesmo aspecto de terminal do «eyebrow»).
    "mfa.frame.setup": { pt: "OCINYE CORE · CONFIGURAR MFA", en: "OCINYE CORE · SET UP MFA", fr: "OCINYE CORE · CONFIGURER LE MFA" },
    "mfa.frame.recovery": { pt: "OCINYE CORE · CÓDIGOS DE RECUPERAÇÃO", en: "OCINYE CORE · RECOVERY CODES", fr: "OCINYE CORE · CODES DE RÉCUPÉRATION" },
    // Ecrã de arranque: cada estado do Core, título e explicação.
    "boot.ready.title": { pt: "SISTEMA OPERACIONAL", en: "SYSTEM OPERATIONAL", fr: "SYSTÈME OPÉRATIONNEL" },
    "boot.ready.body": { pt: "O Ocinye Core está operacional. As capacidades assinaladas abaixo, quando as houver, aguardam disponibilidade — a IA e a computação aguardam a ligação do primeiro nó computacional da Ocinye — e não são avaria: o trabalho institucional segue por inteiro.", en: "Ocinye Core is operational. The capabilities flagged below, when there are any, await availability — AI and compute await the connection of Ocinye’s first compute node — and are not a fault: institutional work carries on in full.", fr: "Ocinye Core est opérationnel. Les capacités signalées ci-dessous, s’il y en a, attendent leur disponibilité — l’IA et le calcul attendent la connexion du premier nœud de calcul d’Ocinye — et ne sont pas une panne : le travail institutionnel se poursuit pleinement." },
    "boot.degraded.title": { pt: "SISTEMA OPERACIONAL COM UMA AVARIA", en: "SYSTEM OPERATIONAL WITH A FAULT", fr: "SYSTÈME OPÉRATIONNEL AVEC UNE PANNE" },
    "boot.degraded.body": { pt: "O Ocinye Core respondeu. Uma capacidade que está configurada e devia responder não está a responder; o trabalho institucional segue, mas há uma avaria assinalada abaixo para resolver.", en: "Ocinye Core responded. A capability that is configured and should respond is not responding; institutional work carries on, but there is a fault flagged below to resolve.", fr: "Ocinye Core a répondu. Une capacité configurée et censée répondre ne répond pas ; le travail institutionnel se poursuit, mais une panne est signalée ci-dessous à résoudre." },
    "boot.blocked.title": { pt: "NÃO FOI POSSÍVEL INICIAR O OCINYE OS", en: "COULD NOT START OCINYE OS", fr: "IMPOSSIBLE DE DÉMARRER OCINYE OS" },
    "boot.blocked.body": { pt: "O Ocinye Core respondeu que não está em condições de operar. Uma dependência essencial não está disponível.", en: "Ocinye Core responded that it is not fit to operate. An essential dependency is unavailable.", fr: "Ocinye Core a répondu qu’il n’est pas en mesure de fonctionner. Une dépendance essentielle est indisponible." },
    "boot.unreachable.title": { pt: "NÃO FOI POSSÍVEL CONTACTAR O OCINYE CORE", en: "COULD NOT CONTACT OCINYE CORE", fr: "IMPOSSIBLE DE CONTACTER OCINYE CORE" },
    "boot.unreachable.body": { pt: "Não houve resposta do Ocinye Core. Isto é diferente de o Core ter dito que não está pronto: aqui não chegámos a saber.", en: "There was no response from Ocinye Core. This is different from the Core saying it is not ready: here we never got to know.", fr: "Il n’y a eu aucune réponse d’Ocinye Core. C’est différent du Core disant qu’il n’est pas prêt : ici, nous n’avons pas pu savoir." },
    "boot.checking.title": { pt: "A VERIFICAR O SISTEMA", en: "CHECKING THE SYSTEM", fr: "VÉRIFICATION DU SYSTÈME" },
};

/// Ecrãs de aviso do sistema — indisponível, recusado, conflito.
const NOTICE: &[Entry] = catalogo! {
    "notice.unavailable.title": { pt: "Esta operação não está disponível agora", en: "This operation is not available right now", fr: "Cette opération n’est pas disponible pour l’instant" },
    "notice.unavailable.default": { pt: "Um serviço de que esta operação depende não está a responder nesta instalação — quem administra o sistema saberá qual.", en: "A service this operation depends on is not responding on this installation — whoever administers the system will know which.", fr: "Un service dont dépend cette opération ne répond pas sur cette installation — l’équipe qui administre le système saura lequel." },
    "notice.unavailable.aside": { pt: "A capacidade existe no Ocinye OS. Não é um problema com o que fez nem com o seu acesso.", en: "The capability exists in Ocinye OS. It is not a problem with what you did or with your access.", fr: "La capacité existe dans Ocinye OS. Ce n’est pas un problème lié à ce que vous avez fait ni à votre accès." },
    "notice.rejected.title": { pt: "O pedido não foi aceite", en: "The request was not accepted", fr: "La demande n’a pas été acceptée" },
    "notice.rejected.aside": { pt: "Nada correu mal. O Ocinye OS percebeu o pedido e não o pode registar tal como foi feito.", en: "Nothing went wrong. Ocinye OS understood the request and cannot record it as it was made.", fr: "Rien ne s’est mal passé. Ocinye OS a compris la demande et ne peut pas l’enregistrer telle qu’elle a été faite." },
    "notice.conflict.title": { pt: "Isto foi alterado noutra sessão", en: "This was changed in another session", fr: "Ceci a été modifié dans une autre session" },
    "notice.conflict.aside": { pt: "Nada se perdeu. Recarregue para ver a versão actual antes de voltar a gravar.", en: "Nothing was lost. Reload to see the current version before saving again.", fr: "Rien n’a été perdu. Rechargez pour voir la version actuelle avant d’enregistrer de nouveau." },
    "notice.go_my_work": { pt: "O Meu Trabalho", en: "My Work", fr: "Mon travail" },
};

/// A Ajuda, a superfície universal (Ask), a Computação e os papéis.
const MISC_A: &[Entry] = catalogo! {
    // Help
    "help.subtitle": { pt: "Ajuda do Ocinye Workspace · estado actual do produto.", en: "Ocinye Workspace help · current product state.", fr: "Aide de l’espace de travail Ocinye · état actuel du produit." },
    "help.on_this_page": { pt: "Nesta página", en: "On this page", fr: "Sur cette page" },
    "help.go_to": { pt: "Ir para ", en: "Go to ", fr: "Aller à " },
    "help.start": { pt: "Começar", en: "Get started", fr: "Commencer" },
    "help.section.account_security": { pt: "Conta e Segurança", en: "Account and Security", fr: "Compte et sécurité" },
    "help.section.knowledge_data": { pt: "Conhecimento e Dados", en: "Knowledge and Data", fr: "Connaissance et données" },
    "help.section.time_calendar": { pt: "Tempo e Calendário", en: "Time and Calendar", fr: "Temps et calendrier" },
    "help.section.ai_agents_compute": { pt: "Ocinye AI, Agentes e Computação", en: "Ocinye AI, Agents and Compute", fr: "Ocinye AI, Agents et Calcul" },
    "help.section.system_states": { pt: "Estados do sistema", en: "System states", fr: "États du système" },
    "help.state.not_in_product": { pt: "A capacidade ainda não existe no produto actual.", en: "The capability does not exist in the current product yet.", fr: "La capacité n’existe pas encore dans le produit actuel." },
    "help.state.not_in_state": { pt: "A operação existe, mas não pode ser executada no estado actual.", en: "The operation exists, but cannot run in the current state.", fr: "L’opération existe, mais ne peut pas s’exécuter dans l’état actuel." },
    "help.state.no_access": { pt: "O recurso existe, mas o seu acesso não permite utilizá-lo.", en: "The resource exists, but your access does not allow using it.", fr: "La ressource existe, mais votre accès ne permet pas de l’utiliser." },
    // Ask (universal command surface)
    "ask.write_naturally": { pt: "Escreva naturalmente. Pode também escolher o que pretende:", en: "Write naturally. You can also choose what you want:", fr: "Écrivez naturellement. Vous pouvez aussi choisir ce que vous voulez :" },
    "ask.write_what": { pt: "Escreva o que procura", en: "Write what you are looking for", fr: "Écrivez ce que vous cherchez" },
    "ask.find_always": { pt: "Encontrar. Funciona sempre.", en: "Find. Always works.", fr: "Trouver. Fonctionne toujours." },
    "ask.ask_something": { pt: "Sobre o trabalho da instituição.", en: "About the institution’s work.", fr: "À propos du travail de l’institution." },
    "ask.do_something": { pt: "Pedir que algo seja feito.", en: "Ask for something to be done.", fr: "Demander que quelque chose soit fait." },
    "ask.no_results": { pt: "Nenhum resultado", en: "No results", fr: "Aucun résultat" },
    "ask.no_access": { pt: "Sem acesso", en: "No access", fr: "Accès refusé" },
    "ask.no_assist_access": { pt: "Não possui acesso à assistência", en: "You do not have access to assistance", fr: "Vous n’avez pas accès à l’assistance" },
    "ask.core_no_answer": { pt: "O Ocinye Core não respondeu", en: "Ocinye Core did not answer", fr: "Ocinye Core n’a pas répondu" },
    "ask.not_done": { pt: "O pedido não foi concluído. Nada foi alterado.", en: "The request did not complete. Nothing was changed.", fr: "La demande n’a pas abouti. Rien n’a été modifié." },
    "ask.will_do": { pt: "O Ocinye vai realizar {count} acção(ões)", en: "Ocinye will perform {count} action(s)", fr: "Ocinye va réaliser {count} action(s)" },
    "ask.status.awaiting": { pt: "Aguarda confirmação", en: "Awaiting confirmation", fr: "En attente de confirmation" },
    "ask.status.not_available": { pt: "Ainda não disponível", en: "Not yet available", fr: "Pas encore disponible" },
    "ask.status.done": { pt: "Concluída", en: "Done", fr: "Terminée" },
    "ask.status.not_run": { pt: "Não executada", en: "Not run", fr: "Non exécutée" },
    "ask.status.unavailable": { pt: "Indisponível", en: "Unavailable", fr: "Indisponible" },
    "ask.kind.simulation": { pt: "Simulação", en: "Simulation", fr: "Simulation" },
    "ask.kind.minor": { pt: "Alteração menor", en: "Minor change", fr: "Modification mineure" },
    "ask.kind.institutional": { pt: "Alteração institucional", en: "Institutional change", fr: "Modification institutionnelle" },
    "ask.kind.external": { pt: "Efeito externo", en: "External effect", fr: "Effet externe" },
    // Compute
    "compute.subtitle": { pt: "O registo de nós computacionais da Ocinye. Zero nós é um estado válido.", en: "Ocinye’s registry of compute nodes. Zero nodes is a valid state.", fr: "Le registre des nœuds de calcul d’Ocinye. Zéro nœud est un état valide." },
    "compute.add_node": { pt: "Adicionar Nó", en: "Add node", fr: "Ajouter un nœud" },
    "compute.sections": { pt: "Secções de Computação", en: "Compute sections", fr: "Sections du calcul" },
    "compute.none_available": { pt: "Nenhum nó de computação Ocinye está actualmente disponível.", en: "No Ocinye compute node is currently available.", fr: "Aucun nœud de calcul Ocinye n’est actuellement disponible." },
    // Roles
    "roles.platform_admin": { pt: "Administrador da plataforma", en: "Platform administrator", fr: "Administrateur de la plateforme" },
    "roles.org_admin": { pt: "Administrador da organização", en: "Organisation administrator", fr: "Administrateur de l’organisation" },
    "roles.unit_manager": { pt: "Gestor de unidade", en: "Unit manager", fr: "Responsable d’unité" },
    "roles.research_lead": { pt: "Líder de investigação", en: "Research lead", fr: "Responsable de recherche" },
    "roles.researcher": { pt: "Investigador", en: "Researcher", fr: "Chercheur" },
    "roles.collaborator": { pt: "Colaborador", en: "Collaborator", fr: "Collaborateur" },
    "roles.external": { pt: "Colaborador externo", en: "External collaborator", fr: "Collaborateur externe" },
    "roles.auditor": { pt: "Auditor", en: "Auditor", fr: "Auditeur" },
    "roles.lead_desc": { pt: "lidera ideias e projectos", en: "leads ideas and projects", fr: "dirige les idées et les projets" },
    "roles.scope.common": { pt: "acesso científico comum", en: "common scientific access", fr: "accès scientifique commun" },
    "roles.scope.narrow": { pt: "âmbito estreito", en: "narrow scope", fr: "portée étroite" },
    "roles.scope.assigned_only": { pt: "só o que for atribuído", en: "only what is assigned", fr: "uniquement ce qui est attribué" },
    "roles.scope.evidence": { pt: "evidência, sem conteúdo", en: "evidence, no content", fr: "preuve, sans contenu" },
    // Ask — títulos, campos e resultados
    "ask.title": { pt: "Pesquisar, perguntar ou executar", en: "Search, ask or act", fr: "Rechercher, demander ou exécuter" },
    "ask.subtitle": { pt: "Escreva o que procura, o que quer saber, ou o que pretende que seja feito. Nada é executado sem a sua confirmação.", en: "Write what you are looking for, what you want to know, or what you want done. Nothing runs without your confirmation.", fr: "Écrivez ce que vous cherchez, ce que vous voulez savoir, ou ce que vous voulez faire exécuter. Rien ne s’exécute sans votre confirmation." },
    "ask.empty_body": { pt: "Pesquisar funciona sempre. Perguntar e executar dependem de uma capacidade de IA do Ocinye OS.", en: "Search always works. Ask and act depend on an Ocinye OS AI capability.", fr: "La recherche fonctionne toujours. Demander et exécuter dépendent d’une capacité d’IA d’Ocinye OS." },
    "ask.field_label": { pt: "O que procura ou pretende", en: "What you are looking for or want", fr: "Ce que vous cherchez ou voulez" },
    "ask.placeholder": { pt: "Pesquisar, perguntar ou executar no Ocinye…", en: "Search, ask or act in Ocinye…", fr: "Rechercher, demander ou exécuter dans Ocinye…" },
    "ask.submit": { pt: "Executar", en: "Run", fr: "Exécuter" },
    "ask.mode.search": { pt: "Pesquisar", en: "Search", fr: "Rechercher" },
    "ask.mode.ask": { pt: "Perguntar", en: "Ask", fr: "Demander" },
    "ask.mode.act": { pt: "Executar", en: "Act", fr: "Exécuter" },
    "ask.withheld": { pt: "{count} resultado(s) que pode consultar não podem ser processados por um modelo, pela sua classificação.", en: "{count} result(s) you can view cannot be processed by a model, because of their classification.", fr: "{count} résultat(s) que vous pouvez consulter ne peuvent pas être traités par un modèle, en raison de leur classification." },
    "ask.no_results_body": { pt: "Nada no acervo institucional a que tenha acesso corresponde a este termo.", en: "Nothing in the institutional holdings you can access matches this term.", fr: "Rien dans le fonds institutionnel auquel vous avez accès ne correspond à ce terme." },
    "ask.untitled": { pt: "(sem título)", en: "(untitled)", fr: "(sans titre)" },
    "ask.count": { pt: "{count} resultado(s).", en: "{count} result(s).", fr: "{count} résultat(s)." },
    "ask.risk.read_only": { pt: "Consulta", en: "Read-only", fr: "Consultation" },
    "ask.risk.privileged": { pt: "Privilegiada", en: "Privileged", fr: "Privilégiée" },
    "ask.approval_note": { pt: "Uma ou mais destas acções têm efeito externo ou alteram estado institucional. Nada acontece sem a sua confirmação.", en: "One or more of these actions have an external effect or change institutional state. Nothing happens without your confirmation.", fr: "Une ou plusieurs de ces actions ont un effet externe ou modifient l’état institutionnel. Rien ne se produit sans votre confirmation." },
    "ask.confirm": { pt: "Confirmar", en: "Confirm", fr: "Confirmer" },
    "ask.cancel": { pt: "Cancelar", en: "Cancel", fr: "Annuler" },
    "ask.result_title": { pt: "Resultado", en: "Result", fr: "Résultat" },
    "ask.status.failed": { pt: "Falhou", en: "Failed", fr: "Échec" },
    "ask.see_ai_status": { pt: "Ver o estado da inteligência", en: "See intelligence status", fr: "Voir l’état de l’intelligence" },
    // Compute — colunas, separadores e métricas
    "compute.col.node": { pt: "Nó", en: "Node", fr: "Nœud" },
    "compute.col.state": { pt: "Estado", en: "State", fr: "État" },
    "compute.col.location": { pt: "Localização", en: "Location", fr: "Emplacement" },
    "compute.col.storage": { pt: "Armazenamento", en: "Storage", fr: "Stockage" },
    "compute.col.health": { pt: "Saúde", en: "Health", fr: "Santé" },
    "compute.none_available_full": { pt: "Nenhum nó de computação Ocinye está actualmente disponível. A plataforma funciona integralmente sem nenhum.", en: "No Ocinye compute node is currently available. The platform works fully without any.", fr: "Aucun nœud de calcul Ocinye n’est actuellement disponible. La plateforme fonctionne intégralement sans aucun." },
    "compute.tab.nodes": { pt: "Nós", en: "Nodes", fr: "Nœuds" },
    "compute.tab.jobs": { pt: "Trabalhos", en: "Jobs", fr: "Tâches" },
    "compute.tab.resources": { pt: "Recursos", en: "Resources", fr: "Ressources" },
    "compute.tab.environments": { pt: "Ambientes", en: "Environments", fr: "Environnements" },
    "compute.registered_count": { pt: "{count} nós registados", en: "{count} registered nodes", fr: "{count} nœuds enregistrés" },
    "compute.metric.active_jobs": { pt: "Trabalhos activos", en: "Active jobs", fr: "Tâches actives" },
    "compute.metric.gpu_available": { pt: "GPU disponível", en: "GPU available", fr: "GPU disponible" },
    "compute.metric.cpu_available": { pt: "CPU disponível", en: "CPU available", fr: "CPU disponible" },
    "compute.metric.storage": { pt: "Armazenamento", en: "Storage", fr: "Stockage" },
    // Help — título, secções e prosa
    "help.title": { pt: "Ajuda", en: "Help", fr: "Aide" },
    "help.section.boot": { pt: "Quando o sistema arranca", en: "When the system starts", fr: "Au démarrage du système" },
    "help.link.home": { pt: "Home", en: "Home", fr: "Accueil" },
    "help.start.p1": { pt: "O Ocinye Workspace é onde o trabalho da instituição acontece: unidades, ideias, projectos, conhecimento, dados e correio. A barra da esquerda mostra a instituição inteira — os ecrãs a que não tem acesso aparecem esbatidos, para que saiba que existem.", en: "The Ocinye Workspace is where the institution’s work happens: units, ideas, projects, knowledge, data and mail. The left-hand bar shows the whole institution — the screens you cannot access appear dimmed, so that you know they exist.", fr: "L’espace de travail Ocinye est là où se déroule le travail de l’institution : unités, idées, projets, connaissance, données et courrier. La barre de gauche montre l’institution entière — les écrans auxquels vous n’avez pas accès apparaissent estompés, pour que vous sachiez qu’ils existent." },
    "help.start.p2": { pt: "A Home reúne o que precisa da sua atenção. O Meu Trabalho mostra o que lhe está atribuído e a investigação em que participa — não tudo o que consegue ver, que é outra coisa e mais.", en: "Home gathers what needs your attention. My Work shows what is assigned to you and the research you take part in — not everything you can see, which is another thing and more.", fr: "L’accueil rassemble ce qui requiert votre attention. Mon travail montre ce qui vous est attribué et la recherche à laquelle vous participez — non pas tout ce que vous pouvez voir, qui est autre chose et davantage." },
    "help.start.p3": { pt: "A barra de pesquisa no topo procura em toda a instituição, dentro do que lhe é acessível. Abre também com ⌘K.", en: "The search bar at the top searches across the whole institution, within what is accessible to you. It also opens with ⌘K.", fr: "La barre de recherche en haut cherche dans toute l’institution, dans les limites de ce qui vous est accessible. Elle s’ouvre aussi avec ⌘K." },
    "help.research.p1": { pt: "Uma Unidade é o âmbito institucional onde a investigação acontece. As ideias e os projectos nascem dentro de uma, e a filiação numa unidade é o que dá acesso ao trabalho que lá vive.", en: "A Unit is the institutional scope where research happens. Ideas and projects are born inside one, and membership of a unit is what grants access to the work that lives there.", fr: "Une Unité est le cadre institutionnel où se déroule la recherche. Les idées et les projets naissent au sein d’une unité, et l’appartenance à une unité est ce qui donne accès au travail qui s’y trouve." },
    "help.research.p2": { pt: "Uma Ideia é exploratória. Nem todas se tornam projectos, e isso é um desfecho legítimo. Quando uma ideia amadurece até candidatura, pode ser promovida a Projecto — e o Research Workspace acompanha-a, com tudo o que foi reunido enquanto se explorava.", en: "An Idea is exploratory. Not all become projects, and that is a legitimate outcome. When an idea matures into a candidate, it can be promoted to a Project — and the Research Workspace comes with it, carrying everything gathered while it was explored.", fr: "Une Idée est exploratoire. Toutes ne deviennent pas des projets, et c’est une issue légitime. Lorsqu’une idée mûrit jusqu’à devenir candidate, elle peut être promue en Projet — et le Research Workspace l’accompagne, avec tout ce qui a été réuni pendant l’exploration." },
    "help.research.p3": { pt: "Por isso não existe «criar projecto do zero»: um projecto nasce de uma ideia, e essa origem fica registada.", en: "That is why there is no “create a project from scratch”: a project is born from an idea, and that origin is recorded.", fr: "C’est pourquoi il n’existe pas de « créer un projet de zéro » : un projet naît d’une idée, et cette origine est consignée." },
    "help.knowledge.p1": { pt: "Referências, notas, documentos e datasets pertencem ao Research Workspace onde a investigação que os usa acontece. As páginas Conhecimento, Bibliografia e Dados reúnem o que alcança em todos eles — reúnem apenas, não mudam a quem pertencem.", en: "References, notes, documents and datasets belong to the Research Workspace where the research that uses them happens. The Knowledge, Bibliography and Data pages gather what you can reach across all of them — they only gather, they do not change who owns them.", fr: "Les références, notes, documents et jeux de données appartiennent au Research Workspace où se déroule la recherche qui les utilise. Les pages Connaissance, Bibliographie et Données rassemblent ce que vous pouvez atteindre dans chacun d’eux — elles rassemblent seulement, elles ne changent pas à qui ils appartiennent." },
    "help.knowledge.p2": { pt: "Por isso, ao criar uma referência ou um dataset a partir dessas páginas, escolhe primeiro o ambiente de destino. Só aparecem os ambientes onde tem autorização para criar.", en: "So when you create a reference or a dataset from those pages, you first choose the destination environment. Only the environments where you are authorised to create appear.", fr: "Ainsi, lorsque vous créez une référence ou un jeu de données depuis ces pages, vous choisissez d’abord l’environnement de destination. Seuls apparaissent les environnements où vous êtes autorisé à créer." },
    "help.knowledge.p3": { pt: "Cada recurso tem uma classificação — PUBLIC, INTERNAL, CONFIDENTIAL ou RESTRICTED — e ela pode ser mais restrita do que a do ambiente que o contém. Se um recurso não aparece, é porque a sua classificação ou a filiação necessária não o alcançam; nunca porque desapareceu.", en: "Each resource has a classification — PUBLIC, INTERNAL, CONFIDENTIAL or RESTRICTED — and it may be more restrictive than that of the environment that contains it. If a resource does not appear, it is because its classification or the required membership does not reach it; never because it vanished.", fr: "Chaque ressource a une classification — PUBLIC, INTERNAL, CONFIDENTIAL ou RESTRICTED — et elle peut être plus restrictive que celle de l’environnement qui la contient. Si une ressource n’apparaît pas, c’est que sa classification ou l’appartenance requise ne l’atteignent pas ; jamais parce qu’elle a disparu." },
    "help.knowledge.p4": { pt: "Resultados ainda não existe no Ocinye OS. Aparece no ecrã de Conhecimento como não implementado, e não como zero — zero diria que a consulta correu e não encontrou nada.", en: "Results does not exist in Ocinye OS yet. It appears on the Knowledge screen as not implemented, and not as zero — zero would say the query ran and found nothing.", fr: "Résultats n’existe pas encore dans Ocinye OS. Cela apparaît sur l’écran Connaissance comme non implémenté, et non comme zéro — zéro dirait que la requête s’est exécutée et n’a rien trouvé." },
    "help.boot.p1": { pt: "Ao abrir o Ocinye OS, a primeira coisa que aparece é o estado do sistema. Não é um ecrã de espera: é o Ocinye Core a dizer se está em condições de operar, antes de lhe pedir a palavra-passe.", en: "When you open Ocinye OS, the first thing that appears is the system state. It is not a waiting screen: it is Ocinye Core saying whether it is fit to operate, before asking you for your password.", fr: "À l’ouverture d’Ocinye OS, la première chose qui apparaît est l’état du système. Ce n’est pas un écran d’attente : c’est Ocinye Core qui indique s’il est en mesure de fonctionner, avant de vous demander votre mot de passe." },
    "help.boot.p2": { pt: "«Sistema operacional» significa que o núcleo está a operar. Pode haver capacidades futuras ainda por chegar — a IA e a computação aguardam o primeiro nó computacional da Ocinye —, e isso não é avaria: são ausências esperadas, e o arranque diz quais. «Sistema operacional com uma avaria» é diferente: uma capacidade que está configurada e devia responder não está a responder — o correio, por exemplo. O trabalho institucional segue nos dois casos.", en: "“System operational” means the core is operating. There may be future capabilities still to come — AI and compute await Ocinye’s first compute node —, and that is not a fault: they are expected absences, and the startup screen says which. “System operational with a fault” is different: a capability that is configured and should respond is not responding — mail, for example. Institutional work carries on in both cases.", fr: "« Système opérationnel » signifie que le noyau fonctionne. Certaines capacités futures peuvent rester à venir — l’IA et le calcul attendent le premier nœud de calcul d’Ocinye —, et ce n’est pas une panne : ce sont des absences attendues, et le démarrage indique lesquelles. « Système opérationnel avec une panne » est différent : une capacité configurée et censée répondre ne répond pas — le courrier, par exemple. Le travail institutionnel se poursuit dans les deux cas." },
    "help.boot.p3": { pt: "«Não foi possível iniciar» significa que uma dependência essencial não está disponível, e por isso não há como entrar. «Sem resposta» é outra coisa: não chegámos a saber o que o Core diria. A diferença importa — numa sabe-se o que se passa, na outra não.", en: "“Could not start” means an essential dependency is unavailable, and so there is no way in. “No response” is another thing: we never got to learn what the Core would say. The difference matters — in one you know what is happening, in the other you do not.", fr: "« Impossible de démarrer » signifie qu’une dépendance essentielle est indisponible, et qu’il n’y a donc pas d’entrée possible. « Sans réponse » est autre chose : nous n’avons pas pu savoir ce que le Core dirait. La différence compte — dans l’un on sait ce qui se passe, dans l’autre non." },
    "help.boot.p4": { pt: "Nos dois casos há um botão para tentar de novo, e ele volta mesmo a perguntar. Se o sistema entretanto ficou em condições, segue.", en: "In both cases there is a button to try again, and it really does ask again. If the system has meanwhile become fit, it proceeds.", fr: "Dans les deux cas, un bouton permet de réessayer, et il redemande réellement. Si le système est entre-temps devenu apte, il poursuit." },
    "help.boot.p5": { pt: "Depois de entrar, a barra superior continua a mostrar o mesmo estado. O arranque não volta a aparecer a cada passo: é a porta de entrada, e não um vigilante.", en: "After you sign in, the top bar keeps showing the same state. The startup screen does not reappear at every step: it is the front door, not a watchman.", fr: "Après connexion, la barre supérieure continue d’afficher le même état. L’écran de démarrage ne réapparaît pas à chaque étape : c’est la porte d’entrée, non un gardien." },
    "help.boot.p6": { pt: "Se seguiu uma ligação para um sítio concreto, é para lá que vai depois de o sistema arrancar e de a sua sessão ser verificada — e não para a página inicial.", en: "If you followed a link to a specific place, that is where you go after the system starts and your session is verified — and not to the home page.", fr: "Si vous avez suivi un lien vers un endroit précis, c’est là que vous allez une fois le système démarré et votre session vérifiée — et non vers la page d’accueil." },
    "help.time.p1": { pt: "A hora na barra superior abre o Centro Temporal: o que tem hoje, o que vem a seguir, e os lembretes por ver. É um relance e um sítio de onde agir — as vistas completas vivem no Calendário.", en: "The time in the top bar opens the Time Centre: what you have today, what comes next, and the reminders still to see. It is a glance and a place to act from — the full views live in the Calendar.", fr: "L’heure dans la barre supérieure ouvre le Centre temporel : ce que vous avez aujourd’hui, ce qui vient ensuite, et les rappels non encore vus. C’est un coup d’œil et un endroit d’où agir — les vues complètes vivent dans le Calendrier." },
    "help.time.p2": { pt: "O Calendário tem quatro vistas da mesma agenda: Hoje, Semana, Mês e Agenda. Todas mostram exactamente o que tem acesso a ver; o que muda entre elas é a forma, nunca o conteúdo.", en: "The Calendar has four views of the same schedule: Today, Week, Month and Agenda. All show exactly what you have access to see; what changes between them is the form, never the content.", fr: "Le Calendrier a quatre vues du même agenda : Aujourd’hui, Semaine, Mois et Agenda. Toutes montrent exactement ce que vous avez accès à voir ; ce qui change entre elles est la forme, jamais le contenu." },
    "help.time.p3": { pt: "Um evento pode ter hora ou ser de dia inteiro. Com hora, indica também a zona horária — «14:00 em Paris» continua a ser 14:00 em Paris para quem estiver em Luanda, e o Ocinye Core guarda as duas coisas. Se escolher uma hora que não existe nesse dia, por causa da mudança para o horário de Verão, o sistema di-lo e pede outra em vez de escolher por si.", en: "An event may have a time or be all-day. With a time, it also states the time zone — “14:00 in Paris” stays 14:00 in Paris for someone in Luanda, and Ocinye Core keeps both. If you choose a time that does not exist on that day, because of the switch to summer time, the system says so and asks for another rather than choosing for you.", fr: "Un événement peut avoir une heure ou être sur la journée entière. Avec une heure, il précise aussi le fuseau horaire — « 14h00 à Paris » reste 14h00 à Paris pour quelqu’un à Luanda, et Ocinye Core conserve les deux. Si vous choisissez une heure qui n’existe pas ce jour-là, à cause du passage à l’heure d’été, le système le signale et en demande une autre au lieu de choisir à votre place." },
    "help.time.p4": { pt: "Uma actividade pode ser pessoal, de uma unidade, de um Research Workspace ou da instituição. A agenda pessoal é sua e de mais ninguém — nem a administração a vê.", en: "An activity may be personal, of a unit, of a Research Workspace or of the institution. Your personal schedule is yours and no one else’s — not even administration sees it.", fr: "Une activité peut être personnelle, d’une unité, d’un Research Workspace ou de l’institution. Votre agenda personnel n’appartient qu’à vous — même l’administration ne le voit pas." },
    "help.time.p5": { pt: "Cancelar uma actividade não a apaga: ela fica visível como cancelada, porque quem a esperava precisa de saber que não vai acontecer.", en: "Cancelling an activity does not delete it: it stays visible as cancelled, because those who expected it need to know it will not happen.", fr: "Annuler une activité ne la supprime pas : elle reste visible comme annulée, car ceux qui l’attendaient doivent savoir qu’elle n’aura pas lieu." },
    "help.time.p6": { pt: "Os prazos das suas tarefas aparecem no Calendário sem deixarem de ser tarefas. Mudar o prazo na tarefa muda o que o Calendário mostra; não há duas datas para manter.", en: "Your tasks’ deadlines appear in the Calendar without ceasing to be tasks. Changing the deadline on the task changes what the Calendar shows; there are not two dates to keep.", fr: "Les échéances de vos tâches apparaissent dans le Calendrier sans cesser d’être des tâches. Changer l’échéance sur la tâche change ce que le Calendrier affiche ; il n’y a pas deux dates à tenir." },
    "help.time.p7": { pt: "Um lembrete não é uma actividade: é um pedido para ser avisado. O Ocinye OS entrega-o mesmo com o Workspace fechado, e o aviso aparece no sino. Uma notificação informa — quando a abre, o Ocinye Core volta a verificar se ainda pode ver aquilo.", en: "A reminder is not an activity: it is a request to be notified. Ocinye OS delivers it even with the Workspace closed, and the notice appears in the bell. A notification informs — when you open it, Ocinye Core checks again whether you can still see that.", fr: "Un rappel n’est pas une activité : c’est une demande d’être averti. Ocinye OS le délivre même l’espace de travail fermé, et l’avis apparaît dans la cloche. Une notification informe — lorsque vous l’ouvrez, Ocinye Core vérifie de nouveau si vous pouvez encore voir cela." },
    "help.mail.p1": { pt: "O Ocinye Mail é o correio institucional, dentro do Workspace. Ler e enviar são serviços distintos e podem falhar em separado.", en: "Ocinye Mail is the institutional mail, inside the Workspace. Reading and sending are distinct services and can fail separately.", fr: "Ocinye Mail est le courrier institutionnel, au sein de l’espace de travail. Lire et envoyer sont des services distincts et peuvent échouer séparément." },
    "help.mail.p2": { pt: "Uma caixa vazia e um serviço não configurado são coisas diferentes, e o ecrã distingue-as. Se o correio ainda não foi configurado nesta instalação, a página di-lo — não mostra uma caixa vazia como se ninguém lhe tivesse escrito. Configurar é tarefa de quem administra.", en: "An empty mailbox and an unconfigured service are different things, and the screen distinguishes them. If mail has not yet been configured on this installation, the page says so — it does not show an empty mailbox as if no one had written to you. Configuring is a task for whoever administers.", fr: "Une boîte vide et un service non configuré sont des choses différentes, et l’écran les distingue. Si le courrier n’a pas encore été configuré sur cette installation, la page le dit — elle n’affiche pas une boîte vide comme si personne ne vous avait écrit. Configurer est la tâche de qui administre." },
    "help.ai.p1": { pt: "O Ocinye OS é operado com IA e governado pelo Core: um agente propõe e orquestra, e o Core autoriza e executa. Um agente nunca alcança mais do que a pessoa que o usa.", en: "Ocinye OS is operated with AI and governed by the Core: an agent proposes and orchestrates, and the Core authorises and executes. An agent never reaches more than the person who uses it.", fr: "Ocinye OS est opéré avec l’IA et gouverné par le Core : un agent propose et orchestre, et le Core autorise et exécute. Un agent n’atteint jamais plus que la personne qui l’utilise." },
    "help.ai.p2": { pt: "Existir e estar disponível são coisas diferentes. As capacidades estão implementadas; a inferência precisa de um nó de IA da Ocinye registado. Enquanto não houver nenhum, a plataforma declara a IA indisponível — e não recorre a um fornecedor externo em silêncio.", en: "Existing and being available are different things. The capabilities are implemented; inference needs a registered Ocinye AI node. Until there is one, the platform declares AI unavailable — and does not silently resort to an external provider.", fr: "Exister et être disponible sont des choses différentes. Les capacités sont implémentées ; l’inférence a besoin d’un nœud d’IA Ocinye enregistré. Tant qu’il n’y en a aucun, la plateforme déclare l’IA indisponible — et ne recourt pas en silence à un fournisseur externe." },
    "help.ai.p3": { pt: "Zero nós de computação e zero agentes são estados válidos, não erros. Todo o restante Workspace funciona sem IA nenhuma.", en: "Zero compute nodes and zero agents are valid states, not errors. All the rest of the Workspace works without any AI.", fr: "Zéro nœud de calcul et zéro agent sont des états valides, non des erreurs. Tout le reste de l’espace de travail fonctionne sans aucune IA." },
    "help.inst.p1": { pt: "Actividade e Audit Log parecem-se e servem para coisas diferentes. A Actividade conta o que aconteceu no trabalho — quem actualizou uma ideia, quem juntou uma nota. O Audit Log é o registo técnico e imutável das operações, para controlo institucional.", en: "Activity and the Audit Log look alike and serve different purposes. Activity tells what happened in the work — who updated an idea, who added a note. The Audit Log is the technical, immutable record of operations, for institutional control.", fr: "L’Activité et le Journal d’audit se ressemblent et servent des fins différentes. L’Activité raconte ce qui s’est passé dans le travail — qui a mis à jour une idée, qui a ajouté une note. Le Journal d’audit est le registre technique et immuable des opérations, pour le contrôle institutionnel." },
    "help.inst.p2": { pt: "A Administração gere pessoas, papéis e filiações. Quem administra a plataforma não ganha, por isso, acesso ao conteúdo científico: ler investigação vem da filiação, não do papel.", en: "Administration manages people, roles and memberships. Whoever administers the platform does not thereby gain access to scientific content: reading research comes from membership, not from the role.", fr: "L’Administration gère les personnes, les rôles et les appartenances. Qui administre la plateforme n’obtient pas pour autant l’accès au contenu scientifique : lire la recherche découle de l’appartenance, non du rôle." },
    "help.account.p1": { pt: "Em Definições encontra a sua conta e as suas credenciais. Pode mudar a palavra-passe e ver as sessões abertas em seu nome, terminando qualquer uma delas.", en: "In Settings you find your account and your credentials. You can change your password and see the sessions open in your name, ending any of them.", fr: "Dans les Paramètres, vous trouvez votre compte et vos identifiants. Vous pouvez changer votre mot de passe et voir les sessions ouvertes en votre nom, en mettant fin à l’une d’elles." },
    "help.account.p2": { pt: "Mudar a palavra-passe exige a actual — uma sessão aberta não é prova suficiente de quem está a escrever. Ao mudá-la, todas as suas sessões terminam e esta é substituída por uma nova, sem ter de voltar a entrar.", en: "Changing your password requires the current one — an open session is not sufficient proof of who is typing. When you change it, all your sessions end and this one is replaced by a new one, without your having to sign in again.", fr: "Changer le mot de passe exige l’actuel — une session ouverte n’est pas une preuve suffisante de qui écrit. En le changeant, toutes vos sessions prennent fin et celle-ci est remplacée par une nouvelle, sans devoir vous reconnecter." },
    "help.account.p3": { pt: "Papéis, filiações e acessos não se alteram aqui. São concedidos por quem tem autoridade para isso, e ficam registados com autor — é o que torna o acesso auditável em vez de acidental.", en: "Roles, memberships and access are not changed here. They are granted by those with the authority to do so, and are recorded with an author — that is what makes access auditable rather than accidental.", fr: "Les rôles, appartenances et accès ne se modifient pas ici. Ils sont accordés par ceux qui en ont l’autorité, et sont consignés avec un auteur — c’est ce qui rend l’accès auditable plutôt qu’accidentel." },
    "help.states.intro": { pt: "O Workspace distingue cinco situações que se parecem no ecrã e significam coisas diferentes. Saber qual está a ver poupa-lhe tempo.", en: "The Workspace distinguishes five situations that look alike on screen and mean different things. Knowing which you are looking at saves you time.", fr: "L’espace de travail distingue cinq situations qui se ressemblent à l’écran et signifient des choses différentes. Savoir laquelle vous regardez vous fait gagner du temps." },
    "help.states.footer": { pt: "Um controlo esbatido nunca é um erro da sua parte. Passe o rato por cima e ele diz qual destes estados o explica.", en: "A dimmed control is never a mistake on your part. Hover over it and it says which of these states explains it.", fr: "Un contrôle estompé n’est jamais une erreur de votre part. Survolez-le et il indique lequel de ces états l’explique." },
    "help.state_label.no_data": { pt: "Sem dados", en: "No data", fr: "Aucune donnée" },
    "help.state_label.no_permission": { pt: "Sem permissão", en: "No permission", fr: "Aucune autorisation" },
    "help.state_label.not_configured": { pt: "Não configurado", en: "Not configured", fr: "Non configuré" },
    "help.state_label.not_implemented": { pt: "Não implementado", en: "Not implemented", fr: "Non implémenté" },
    "help.state_label.unavailable": { pt: "Indisponível", en: "Unavailable", fr: "Indisponible" },
    "help.state.no_data_meaning": { pt: "A funcionalidade existe e a consulta devolveu zero resultados.", en: "The feature exists and the query returned zero results.", fr: "La fonctionnalité existe et la requête a renvoyé zéro résultat." },
    "help.state.not_configured_meaning": { pt: "A capacidade existe no Ocinye OS, mas esta instalação ainda não tem o serviço necessário configurado.", en: "The capability exists in Ocinye OS, but this installation does not yet have the required service configured.", fr: "La capacité existe dans Ocinye OS, mais cette installation n’a pas encore le service requis configuré." },
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
    // Conta
    "settings.account.section": { pt: "A sua conta", en: "Your account", fr: "Votre compte" },
    "settings.field.name": { pt: "Nome", en: "Name", fr: "Nom" },
    "settings.field.email": { pt: "Correio institucional", en: "Institutional address", fr: "Adresse institutionnelle" },
    "settings.field.status": { pt: "Estado", en: "Status", fr: "Statut" },
    "settings.field.institution": { pt: "Instituição", en: "Institution", fr: "Institution" },
    "settings.account.managed_note": { pt: "Estes dados são geridos pela Administração da Ocinye. Papéis, filiações e acessos não se alteram aqui — são concedidos por quem tem autoridade para isso, e ficam registados.", en: "These details are managed by Ocinye Administration. Roles, memberships and access are not changed here — they are granted by those with the authority to do so, and are recorded.", fr: "Ces données sont gérées par l’Administration d’Ocinye. Les rôles, appartenances et accès ne se modifient pas ici — ils sont accordés par ceux qui en ont l’autorité, et sont consignés." },
    // Segurança — palavra-passe
    "settings.password.section": { pt: "Palavra-passe", en: "Password", fr: "Mot de passe" },
    "settings.password.current": { pt: "Palavra-passe actual", en: "Current password", fr: "Mot de passe actuel" },
    "settings.password.current_hint": { pt: "A que usa hoje", en: "The one you use today", fr: "Celui que vous utilisez aujourd’hui" },
    "settings.password.new": { pt: "Nova palavra-passe", en: "New password", fr: "Nouveau mot de passe" },
    "settings.password.new_hint": { pt: "Mínimo de 15 caracteres", en: "At least 15 characters", fr: "15 caractères minimum" },
    "settings.password.confirm": { pt: "Confirmar", en: "Confirm", fr: "Confirmer" },
    "settings.password.confirm_hint": { pt: "Repita a nova palavra-passe", en: "Repeat the new password", fr: "Répétez le nouveau mot de passe" },
    "settings.password.note": { pt: "Ao mudar a palavra-passe, todas as suas sessões terminam e esta é substituída por uma nova. Continua a trabalhar sem voltar a entrar.", en: "When you change your password, all your sessions end and this one is replaced by a new one. You keep working without signing in again.", fr: "Lorsque vous changez votre mot de passe, toutes vos sessions prennent fin et celle-ci est remplacée par une nouvelle. Vous continuez à travailler sans vous reconnecter." },
    "settings.password.change": { pt: "Mudar palavra-passe", en: "Change password", fr: "Changer le mot de passe" },
    // Segurança — sessões
    "settings.sessions.section": { pt: "As minhas sessões", en: "My sessions", fr: "Mes sessions" },
    "settings.sessions.unreadable": { pt: "A lista de sessões não pôde ser lida. Isto não quer dizer que não existam outras sessões — quer dizer que não sabemos quais são.", en: "The list of sessions could not be read. This does not mean there are no other sessions — it means we do not know which they are.", fr: "La liste des sessions n’a pas pu être lue. Cela ne signifie pas qu’il n’existe pas d’autres sessions — cela signifie que nous ne savons pas lesquelles." },
    "settings.sessions.none": { pt: "Não há sessões activas para além desta.", en: "There are no active sessions besides this one.", fr: "Il n’y a aucune session active hormis celle-ci." },
    "settings.sessions.current": { pt: "Sessão actual", en: "Current session", fr: "Session actuelle" },
    "settings.sessions.end_this": { pt: "Terminar esta", en: "End this one", fr: "Mettre fin à celle-ci" },
    "settings.sessions.end": { pt: "Terminar", en: "End", fr: "Mettre fin" },
    "settings.sessions.note": { pt: "Terminar a sessão actual encerra este acesso e volta ao início de sessão.", en: "Ending the current session closes this access and returns to sign-in.", fr: "Mettre fin à la session actuelle ferme cet accès et ramène à la connexion." },
    // Segurança — códigos de recuperação
    "settings.recovery.title": { pt: "Códigos de recuperação", en: "Recovery codes", fr: "Codes de récupération" },
    "settings.recovery.section": { pt: "Segundo factor", en: "Second factor", fr: "Second facteur" },
    "settings.recovery.new_saved": { pt: "Guardou-os? Estes são os códigos novos. Os anteriores deixaram de valer. Não voltarão a ser mostrados.", en: "Saved them? These are the new codes. The previous ones no longer work. They will not be shown again.", fr: "Enregistrés ? Voici les nouveaux codes. Les précédents ne fonctionnent plus. Ils ne seront plus affichés." },
    "settings.recovery.copy": { pt: "Copiar códigos", en: "Copy codes", fr: "Copier les codes" },
    "settings.recovery.download": { pt: "Guardar ficheiro", en: "Save file", fr: "Enregistrer le fichier" },
    "settings.recovery.regen_intro": { pt: "Regenerar emite dez códigos novos e invalida os que tem. Confirme com a palavra-passe e um código do autenticador.", en: "Regenerating issues ten new codes and invalidates the ones you have. Confirm with your password and an authenticator code.", fr: "La régénération émet dix nouveaux codes et invalide ceux que vous avez. Confirmez avec votre mot de passe et un code de l’authentificateur." },
    "settings.recovery.code_ph": { pt: "Código do autenticador", en: "Authenticator code", fr: "Code de l’authentificateur" },
    "settings.recovery.regen_button": { pt: "Regenerar códigos de recuperação", en: "Regenerate recovery codes", fr: "Régénérer les codes de récupération" },
    "settings.recovery.no_mfa": { pt: "Esta conta não tem um segundo factor activo, por isso não há códigos de recuperação para regenerar.", en: "This account has no active second factor, so there are no recovery codes to regenerate.", fr: "Ce compte n’a pas de second facteur actif, il n’y a donc pas de codes de récupération à régénérer." },
    // Imagem de perfil
    "settings.avatar.section": { pt: "Imagem de perfil", en: "Profile picture", fr: "Photo de profil" },
    "settings.avatar.updated": { pt: "Imagem de perfil actualizada.", en: "Profile picture updated.", fr: "Photo de profil mise à jour." },
    "settings.avatar.initials_note": { pt: "As iniciais são sempre o recurso: se a imagem não carregar, é o seu nome que aparece.", en: "Initials are always the fallback: if the image does not load, it is your name that appears.", fr: "Les initiales sont toujours le recours : si l’image ne se charge pas, c’est votre nom qui apparaît." },
    "settings.avatar.presets_label": { pt: "Avatares Ocinye", en: "Ocinye avatars", fr: "Avatars Ocinye" },
    "settings.avatar.preset_alt": { pt: "Avatar Ocinye {name}", en: "Ocinye avatar {name}", fr: "Avatar Ocinye {name}" },
    "settings.avatar.use_initials": { pt: "Usar iniciais", en: "Use initials", fr: "Utiliser les initiales" },
    "settings.avatar.photo_label": { pt: "Fotografia", en: "Photograph", fr: "Photographie" },
    "settings.avatar.replace": { pt: "Substituir fotografia", en: "Replace photograph", fr: "Remplacer la photographie" },
    "settings.avatar.upload": { pt: "Carregar fotografia", en: "Upload photograph", fr: "Téléverser la photographie" },
    "settings.avatar.photo_note": { pt: "JPEG, PNG ou WebP, até 8 MiB. A fotografia é recortada num quadrado ao centro e guardada pela Ocinye — não é enviada para nenhum serviço externo, e a informação de câmara e localização que a acompanhe não é conservada.", en: "JPEG, PNG or WebP, up to 8 MiB. The photograph is cropped to a centred square and kept by Ocinye — it is not sent to any external service, and any camera and location information it carries is not retained.", fr: "JPEG, PNG ou WebP, jusqu’à 8 Mio. La photographie est recadrée en un carré centré et conservée par Ocinye — elle n’est envoyée à aucun service externe, et les informations d’appareil et de localisation qu’elle contient ne sont pas conservées." },
    // Componentes partilhados
    "action.not_yet_available": { pt: "Ainda não disponível", en: "Not yet available", fr: "Pas encore disponible" },
    "table.previous": { pt: "← Anterior", en: "← Previous", fr: "← Précédent" },
    "table.next": { pt: "Seguinte →", en: "Next →", fr: "Suivant →" },
    "table.toggle_density": { pt: "Alternar densidade das linhas", en: "Toggle row density", fr: "Basculer la densité des lignes" },
};

/// Todos os grupos de produção. O portão de paridade corre sobre isto.
///
/// Não inclui grupos de teste: uma chave só-`pt` de teste (para provar a queda)
/// não é um buraco de produção, e não deve fazer o portão soar.
pub const GROUPS: &[&[Entry]] = &[
    NOTICE,
    MISC_A,
    AUTH,
    MESSAGING,
    CALENDAR,
    FILES,
    MAIL,
    DATE,
    NAV,
    ACTIONS,
    CREATE,
    HOME,
    MY_WORK,
    KNOWLEDGE,
    NOTES,
    RESOURCES,
    SEARCH,
    ASSIST,
    CLASSIFICATION,
    TASK_STATE,
    ERRORS,
    FIRST_ENTRY,
    SETTINGS,
];

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
