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
pub const GROUPS: &[&[Entry]] = &[
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
