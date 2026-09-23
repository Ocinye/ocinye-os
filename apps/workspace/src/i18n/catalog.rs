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
    "home.ai.unavailable_body": {
        pt: "O Prompt Ocinye está operacional. Não existe neste momento nenhum nó de IA Ocinye activo. Pode continuar a usar o Prompt; os pedidos que precisem de inferência recebem uma resposta de estado até existir capacidade que os sirva. Nenhum fornecedor externo é usado em substituição.",
        en: "The Ocinye Prompt is operational. There is no active Ocinye AI node at the moment. You can keep using the Prompt; requests that need inference receive a status response until a capability that serves them exists. No external provider is used as a substitute.",
        fr: "Le Prompt Ocinye est opérationnel. Il n’y a pour l’instant aucun nœud d’IA Ocinye actif. Vous pouvez continuer à utiliser le Prompt ; les demandes nécessitant une inférence reçoivent une réponse d’état jusqu’à ce qu’une capacité les serve. Aucun fournisseur externe n’est utilisé en remplacement."
    },
    "home.ai.available_body": {
        pt: "A inteligência está disponível. Abra o Prompt para a usar.",
        en: "Intelligence is available. Open the Prompt to use it.",
        fr: "L’intelligence est disponible. Ouvrez le Prompt pour l’utiliser."
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
    "notes.my_notes": { pt: "As minhas notas", en: "My notes", fr: "Mes notes" },
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
    "table.density": { pt: "Densidade", en: "Density", fr: "Densité" },
    "table.slices_aria": { pt: "Recortes da lista", en: "List views", fr: "Vues de la liste" },
};

/// O hub de Ocinye AI e a criação/detalhe de agentes. Os valores de enum
/// (`GENERAL`, `personal`, `INTERNAL`) e as rotas são maquinaria e ficam
/// literais; só o texto de chrome passa por aqui (§84).
const AI: &[Entry] = catalogo! {
    "ai.subtitle": {
        pt: "A inteligência artificial é uma capacidade transversal da Ocinye.",
        en: "Artificial intelligence is a cross-cutting capability of Ocinye.",
        fr: "L’intelligence artificielle est une capacité transversale d’Ocinye."
    },
    "ai.tab.overview": { pt: "Visão geral", en: "Overview", fr: "Vue d’ensemble" },
    "ai.tab.architecture": { pt: "Arquitectura", en: "Architecture", fr: "Architecture" },
    "ai.tab.capabilities": { pt: "Capacidades", en: "Capabilities", fr: "Capacités" },
    "ai.tab.models": { pt: "Modelos", en: "Models", fr: "Modèles" },
    "ai.sections": { pt: "Secções de Ocinye AI", en: "Ocinye AI sections", fr: "Sections d’Ocinye AI" },
    "ai.create_agent": { pt: "Criar Agente", en: "Create Agent", fr: "Créer un agent" },
    "ai.create_agent_title": { pt: "Criar Agente IA", en: "Create AI Agent", fr: "Créer un agent IA" },
    "ai.open_prompt": { pt: "Abrir Prompt", en: "Open Prompt", fr: "Ouvrir Prompt" },
    "ai.none_available_full": {
        pt: "Nenhum nó de IA Ocinye está actualmente disponível. A plataforma funciona integralmente sem um, e nenhum fornecedor externo é usado em substituição.",
        en: "No Ocinye AI node is currently available. The platform works fully without one, and no external provider is used in its place.",
        fr: "Aucun nœud d’IA Ocinye n’est actuellement disponible. La plateforme fonctionne pleinement sans lui, et aucun fournisseur externe n’est utilisé à sa place."
    },
    "ai.unavailable_title": { pt: "Inteligência ainda não disponível", en: "Intelligence not yet available", fr: "Intelligence pas encore disponible" },
    "ai.configure": { pt: "Configurar IA", en: "Configure AI", fr: "Configurer l’IA" },
    "ai.view_compute": { pt: "Ver computação", en: "View compute", fr: "Voir le calcul" },
    "ai.counter.agents": { pt: "Agentes IA", en: "AI agents", fr: "Agents IA" },
    "ai.counter.models": { pt: "Modelos", en: "Models", fr: "Modèles" },
    "ai.counter.conversations": { pt: "Conversas", en: "Conversations", fr: "Conversations" },
    "ai.counter.resources": { pt: "Recursos", en: "Resources", fr: "Ressources" },
    "ai.view_agents": { pt: "Ver agentes", en: "View agents", fr: "Voir les agents" },
    "ai.view_models": { pt: "Ver modelos", en: "View models", fr: "Voir les modèles" },
    "ai.open_prompt_action": { pt: "Abrir prompt", en: "Open prompt", fr: "Ouvrir le prompt" },
    "ai.no_capability": {
        pt: "Nenhum nó de IA Ocinye está registado. O agente será guardado e ficará executável quando uma capacidade compatível estiver activa.",
        en: "No Ocinye AI node is registered. The agent will be saved and will become runnable when a compatible capability is active.",
        fr: "Aucun nœud d’IA Ocinye n’est enregistré. L’agent sera enregistré et deviendra exécutable dès qu’une capacité compatible sera active."
    },
    "ai.new.subtitle": {
        pt: "Um agente actua dentro do âmbito e da classificação que lhe forem dados.",
        en: "An agent acts within the scope and classification it is given.",
        fr: "Un agent agit dans le périmètre et la classification qui lui sont attribués."
    },
    "ai.no_capability_title": { pt: "Sem capacidade de execução", en: "No execution capability", fr: "Aucune capacité d’exécution" },
    "ai.no_capability_body": {
        pt: "Nenhum nó de IA Ocinye está actualmente registado. O agente será guardado e ficará disponível para execução quando uma capacidade de IA compatível estiver activa.",
        en: "No Ocinye AI node is currently registered. The agent will be saved and will become available to run when a compatible AI capability is active.",
        fr: "Aucun nœud d’IA Ocinye n’est actuellement enregistré. L’agent sera enregistré et deviendra disponible à l’exécution dès qu’une capacité d’IA compatible sera active."
    },
    "ai.section.identity": { pt: "Identidade", en: "Identity", fr: "Identité" },
    "ai.agent.name": { pt: "Nome do agente", en: "Agent name", fr: "Nom de l’agent" },
    "ai.agent.name_placeholder": { pt: "Ex.: Assistente de Pesquisa", en: "E.g. Research Assistant", fr: "Ex. : Assistant de recherche" },
    "ai.agent.purpose": { pt: "Propósito", en: "Purpose", fr: "Objet" },
    "ai.agent.purpose_placeholder": { pt: "Para que serve este agente", en: "What this agent is for", fr: "À quoi sert cet agent" },
    "ai.agent.instructions": { pt: "Instruções gerais", en: "General instructions", fr: "Instructions générales" },
    "ai.agent.instructions_placeholder": {
        pt: "Como deve responder e a que se deve limitar",
        en: "How it should respond and what it should limit itself to",
        fr: "Comment il doit répondre et à quoi il doit se limiter"
    },
    "ai.agent.capability": { pt: "Capacidade principal", en: "Primary capability", fr: "Capacité principale" },
    "ai.capability_hint": {
        pt: "O agente pede uma capacidade. O Ocinye AI Gateway escolhe o modelo que a serve, como configuração.",
        en: "The agent requests a capability. The Ocinye AI Gateway chooses the model that serves it, as configuration.",
        fr: "L’agent demande une capacité. L’Ocinye AI Gateway choisit le modèle qui la sert, comme configuration."
    },
    "ai.section.scope": { pt: "Âmbito de acesso", en: "Access scope", fr: "Périmètre d’accès" },
    "ai.scope.legend": { pt: "Âmbito do agente", en: "Agent scope", fr: "Périmètre de l’agent" },
    "ai.scope.personal": { pt: "Pessoal", en: "Personal", fr: "Personnel" },
    "ai.scope.unit": { pt: "Unidade", en: "Unit", fr: "Unité" },
    "ai.scope.institutional": { pt: "Institucional", en: "Institutional", fr: "Institutionnel" },
    "ai.scope.help": {
        pt: "O âmbito de Research Workspace fica disponível ao criar o agente dentro de um workspace. O Ocinye Core recusa um âmbito para o qual não possua a permissão correspondente.",
        en: "The Research Workspace scope becomes available when creating the agent inside a workspace. Ocinye Core refuses a scope for which you do not hold the corresponding permission.",
        fr: "Le périmètre Research Workspace devient disponible en créant l’agent au sein d’un workspace. Ocinye Core refuse un périmètre pour lequel vous ne détenez pas la permission correspondante."
    },
    "ai.section.knowledge": { pt: "Conhecimento", en: "Knowledge", fr: "Connaissance" },
    "ai.knowledge.documents": { pt: "Documentos institucionais", en: "Institutional documents", fr: "Documents institutionnels" },
    "ai.source.datasets": { pt: "Datasets", en: "Datasets", fr: "Jeux de données" },
    "ai.security.title": { pt: "Segurança", en: "Security", fr: "Sécurité" },
    "ai.security.body": {
        pt: "O agente lê apenas até INTERNAL, e nunca mais do que quem o cria. Material CONFIDENTIAL e RESTRICTED fica inacessível, independentemente do que for pedido. Cada acesso a dados classificados é registado no Audit Log.",
        en: "The agent reads only up to INTERNAL, and never more than whoever creates it. CONFIDENTIAL and RESTRICTED material stays inaccessible, regardless of what is asked. Every access to classified data is recorded in the Audit Log.",
        fr: "L’agent ne lit que jusqu’à INTERNAL, et jamais plus que celui qui le crée. Le matériel CONFIDENTIAL et RESTRICTED reste inaccessible, quelle que soit la demande. Chaque accès à des données classifiées est consigné dans l’Audit Log."
    },
    "ai.available_when_created": {
        pt: "O agente fica disponível para execução assim que for criado.",
        en: "The agent becomes available to run as soon as it is created.",
        fr: "L’agent devient disponible à l’exécution dès sa création."
    },
    "ai.source.documents": { pt: "Documentos", en: "Documents", fr: "Documents" },
    "ai.sources.none": { pt: "Nenhuma", en: "None", fr: "Aucune" },
    "ai.detail.pill": { pt: "AGENTE", en: "AGENT", fr: "AGENT" },
    "ai.back_to_agents": { pt: "← Voltar aos agentes", en: "← Back to agents", fr: "← Retour aux agents" },
    "ai.detail.definition": { pt: "Definição", en: "Definition", fr: "Définition" },
    "ai.metric.capability": { pt: "Capacidade", en: "Capability", fr: "Capacité" },
    "ai.metric.scope": { pt: "Âmbito", en: "Scope", fr: "Périmètre" },
    "ai.metric.classification_ceiling": { pt: "Tecto de classificação", en: "Classification ceiling", fr: "Plafond de classification" },
    "ai.metric.knowledge_sources": { pt: "Fontes de conhecimento", en: "Knowledge sources", fr: "Sources de connaissance" },
    "ai.metric.created_by": { pt: "Criado por", en: "Created by", fr: "Créé par" },
    "ai.detail.instructions": { pt: "Instruções", en: "Instructions", fr: "Instructions" },
    "ai.detail.execution": { pt: "Execução", en: "Execution", fr: "Exécution" },
    "ai.execution.available": {
        pt: "Existe capacidade de inferência que pode servir este agente.",
        en: "There is inference capability that can serve this agent.",
        fr: "Il existe une capacité d’inférence pouvant servir cet agent."
    },
    "ai.execution.unavailable": {
        pt: "Nenhum nó de IA da Ocinye está disponível: o agente está definido e configurado, e correrá assim que existir uma capacidade que o sirva. A definição não depende de haver modelo.",
        en: "No Ocinye AI node is available: the agent is defined and configured, and will run as soon as a capability that serves it exists. The definition does not depend on a model existing.",
        fr: "Aucun nœud d’IA d’Ocinye n’est disponible : l’agent est défini et configuré, et s’exécutera dès qu’une capacité pouvant le servir existera. La définition ne dépend pas de l’existence d’un modèle."
    },
};

/// A Administração de membros: criar, credencial, acesso, segurança, unidades,
/// research workspaces, papéis, grants e estado da conta. A prosa sensível à
/// segurança traduz-se com o mesmo significado exacto — `en` e `fr` dizem o que
/// o `pt` diz, sem suavizar nem omitir (briefing §73, §84).
const ADMIN: &[Entry] = catalogo! {
    // Posições institucionais — verdade organizacional, não concedem acesso (ADR-0100).
    "admin.position.researcher": { pt: "Investigador", en: "Researcher", fr: "Chercheur" },
    "admin.position.engineer": { pt: "Engenheiro", en: "Engineer", fr: "Ingénieur" },
    "admin.position.principal_investigator": { pt: "Investigador principal", en: "Principal investigator", fr: "Chercheur principal" },
    "admin.position.unit_lead": { pt: "Responsável de unidade", en: "Unit lead", fr: "Responsable d’unité" },
    "admin.position.fellow": { pt: "Bolseiro", en: "Fellow", fr: "Boursier" },
    "admin.position.student": { pt: "Estudante", en: "Student", fr: "Étudiant" },
    "admin.position.director": { pt: "Director", en: "Director", fr: "Directeur" },
    "admin.position.founder": { pt: "Fundador", en: "Founder", fr: "Fondateur" },
    "admin.position.external_collaborator": { pt: "Colaborador externo", en: "External collaborator", fr: "Collaborateur externe" },
    "admin.position.label": { pt: "Posição institucional", en: "Institutional position", fr: "Position institutionnelle" },
    "admin.position.no_access_suffix": { pt: " — não concede acesso.", en: " — grants no access.", fr: " — ne donne aucun accès." },
    // Adicionar membro
    "admin.new.title": { pt: "Adicionar membro", en: "Add member", fr: "Ajouter un membre" },
    "admin.new.subtitle": { pt: "O Ocinye Core gera uma palavra-passe temporária. O membro terá de definir a sua no primeiro acesso.", en: "Ocinye Core generates a temporary password. The member will have to set their own on first access.", fr: "Ocinye Core génère un mot de passe temporaire. Le membre devra définir le sien au premier accès." },
    "admin.new.identity": { pt: "Identidade", en: "Identity", fr: "Identité" },
    "admin.new.organisation": { pt: "Organização", en: "Organisation", fr: "Organisation" },
    "admin.new.access": { pt: "Acesso", en: "Access", fr: "Accès" },
    "admin.new.full_name": { pt: "Nome completo", en: "Full name", fr: "Nom complet" },
    "admin.new.full_name_ph": { pt: "Ex.: Ana Maria Fernandes", en: "e.g. Ana Maria Fernandes", fr: "Ex. : Ana Maria Fernandes" },
    "admin.new.email_ph": { pt: "ana.fernandes@ocinye.com", en: "ana.fernandes@ocinye.com", fr: "ana.fernandes@ocinye.com" },
    "admin.new.email_hint": { pt: "É a identidade e a credencial de entrada. A convenção da instituição é primeiro.ultimo@ocinye.com, em minúsculas.", en: "It is the identity and the sign-in credential. The institution’s convention is first.last@ocinye.com, in lower case.", fr: "C’est l’identité et l’identifiant de connexion. La convention de l’institution est prenom.nom@ocinye.com, en minuscules." },
    "admin.new.position_truth": { pt: "Verdade organizacional. ", en: "Organisational truth. ", fr: "Vérité organisationnelle. " },
    "admin.new.position_no_access": { pt: "Não concede acesso a nada.", en: "Grants access to nothing.", fr: "Ne donne accès à rien." },
    "admin.new.initial_unit": { pt: "Unidade inicial", en: "Initial unit", fr: "Unité initiale" },
    "admin.new.no_units": { pt: "Ainda não existem unidades", en: "There are no units yet", fr: "Il n’existe pas encore d’unités" },
    "admin.new.no_unit_option": { pt: "Sem unidade", en: "No unit", fr: "Aucune unité" },
    "admin.new.role_hint": { pt: "Na dúvida, escolha o mais estreito. Alargar depois é um pedido; estreitar é uma conversa.", en: "When in doubt, choose the narrowest. Widening later is a request; narrowing is a conversation.", fr: "En cas de doute, choisissez le plus étroit. Élargir ensuite est une demande ; restreindre est une conversation." },
    "admin.new.whats_next": { pt: "O que acontece a seguir", en: "What happens next", fr: "Ce qui se passe ensuite" },
    "admin.new.whats_next_body": { pt: "É gerada uma palavra-passe temporária, válida 24 horas e apresentada uma única vez. Entregue-a por canal seguro. O membro não entra no Workspace com ela: serve só para definir a sua.", en: "A temporary password is generated, valid for 24 hours and shown only once. Hand it over through a secure channel. The member does not enter the Workspace with it: it serves only to set their own.", fr: "Un mot de passe temporaire est généré, valable 24 heures et affiché une seule fois. Remettez-le par un canal sûr. Le membre n’entre pas dans l’espace de travail avec : il sert uniquement à définir le sien." },
    "admin.new.submit": { pt: "Criar membro", en: "Create member", fr: "Créer le membre" },
    // Credencial emitida — apresentada uma única vez
    "admin.issued.title": { pt: "Utilizador criado", en: "User created", fr: "Utilisateur créé" },
    "admin.issued.subtitle": { pt: "A conta existe. Falta entregar o acesso.", en: "The account exists. Access is still to be handed over.", fr: "Le compte existe. L’accès reste à remettre." },
    "admin.issued.temp_password": { pt: "Palavra-passe temporária", en: "Temporary password", fr: "Mot de passe temporaire" },
    "admin.issued.valid_until": { pt: "Válida até", en: "Valid until", fr: "Valable jusqu’au" },
    "admin.issued.shown_once": { pt: "Esta palavra-passe só é apresentada uma vez.", en: "This password is shown only once.", fr: "Ce mot de passe n’est affiché qu’une seule fois." },
    "admin.issued.transmit_note": { pt: " Transmita-a ao membro através de um canal seguro — presencialmente, por voz, ou por mensagem efémera cifrada. Nunca por email, SMS ou chat. Depois de fechar esta página, ninguém a consegue recuperar.", en: " Transmit it to the member through a secure channel — in person, by voice, or by an encrypted ephemeral message. Never by email, SMS or chat. Once you close this page, no one can recover it.", fr: " Transmettez-le au membre par un canal sûr — en personne, de vive voix, ou par un message éphémère chiffré. Jamais par courriel, SMS ou messagerie. Une fois cette page fermée, personne ne peut le récupérer." },
    "admin.issued.done": { pt: "Concluído", en: "Done", fr: "Terminé" },
    // Estado da credencial e da conta
    "admin.security.credential": { pt: "Credencial", en: "Credential", fr: "Identifiant" },
    "admin.account.status": { pt: "Estado da conta", en: "Account status", fr: "État du compte" },
    "admin.password.permanent": { pt: "Palavra-passe definitiva", en: "Permanent password", fr: "Mot de passe définitif" },
    "admin.password.self_set": { pt: "Definida pelo próprio", en: "Set by the member", fr: "Défini par le membre" },
    "admin.password.not_yet_set": { pt: "Ainda não definida", en: "Not set yet", fr: "Pas encore défini" },
    "admin.password.set_at": { pt: "Definida em", en: "Set on", fr: "Défini le" },
    "admin.credential.temporary": { pt: "Credencial temporária", en: "Temporary credential", fr: "Identifiant temporaire" },
    "admin.credential.expired_on": { pt: "Expirada em {date}", en: "Expired on {date}", fr: "Expiré le {date}" },
    "admin.credential.expires_prefix": { pt: "Expira em ", en: "Expires on ", fr: "Expire le " },
    "admin.last_sign_in": { pt: "Último acesso", en: "Last sign-in", fr: "Dernière connexion" },
    "admin.recent_failures": { pt: "Falhas recentes (7 dias)", en: "Recent failures (7 days)", fr: "Échecs récents (7 jours)" },
    "admin.sessions.active": { pt: "Sessões activas", en: "Active sessions", fr: "Sessions actives" },
    "admin.sessions.none": { pt: "Sem sessões activas.", en: "No active sessions.", fr: "Aucune session active." },
    "admin.action.revoke": { pt: "Revogar", en: "Revoke", fr: "Révoquer" },
    // Dar / reemitir acesso — prosa sensível à segurança
    "admin.access.give": { pt: "Dar acesso", en: "Give access", fr: "Donner l’accès" },
    "admin.access.reissue": { pt: "Reemitir acesso", en: "Reissue access", fr: "Réémettre l’accès" },
    "admin.access.give_note": { pt: "Esta pessoa existe na instituição e ainda não tem como entrar. Dar-lhe acesso emite uma credencial temporária e não lhe altera papéis, unidades nem autoridade. A palavra-passe definitiva é definida pelo próprio no primeiro acesso — nunca por quem administra.", en: "This person exists in the institution and still has no way to sign in. Giving them access issues a temporary credential and does not change their roles, units or authority. The permanent password is set by them on first access — never by whoever administers.", fr: "Cette personne existe dans l’institution et n’a encore aucun moyen de se connecter. Lui donner accès émet un identifiant temporaire et ne modifie ni ses rôles, ni ses unités, ni son autorité. Le mot de passe définitif est défini par elle-même au premier accès — jamais par qui administre." },
    "admin.access.reissue_note": { pt: "A credencial temporária anterior expirou e esta pessoa ficou sem como entrar. Reemitir invalida a credencial expirada e emite uma nova; não lhe altera papéis, unidades nem autoridade, e a palavra-passe definitiva continua a ser definida pelo próprio no primeiro acesso.", en: "The previous temporary credential expired and this person was left with no way to sign in. Reissuing invalidates the expired credential and issues a new one; it does not change their roles, units or authority, and the permanent password is still set by them on first access.", fr: "L’identifiant temporaire précédent a expiré et cette personne s’est retrouvée sans moyen de se connecter. Le réémettre invalide l’identifiant expiré et en émet un nouveau ; il ne modifie ni ses rôles, ni ses unités, ni son autorité, et le mot de passe définitif reste défini par elle-même au premier accès." },
    // Acesso — papéis, grants, permissões
    "admin.roles.technical": { pt: "Papéis técnicos", en: "Technical roles", fr: "Rôles techniques" },
    "admin.role.technical": { pt: "Papel técnico", en: "Technical role", fr: "Rôle technique" },
    "admin.roles.none_assigned": { pt: "Sem papéis atribuídos.", en: "No roles assigned.", fr: "Aucun rôle attribué." },
    "admin.grants.explicit": { pt: "Grants explícitos", en: "Explicit grants", fr: "Attributions explicites" },
    "admin.grants.none": { pt: "Nenhum. O acesso deste membro vem apenas de papéis e memberships.", en: "None. This member’s access comes only from roles and memberships.", fr: "Aucune. L’accès de ce membre provient uniquement des rôles et des appartenances." },
    "admin.state.active": { pt: "activo", en: "active", fr: "actif" },
    "admin.state.revoked": { pt: "revogado", en: "revoked", fr: "révoqué" },
    "admin.permissions.institutional": { pt: "Permissões institucionais", en: "Institutional permissions", fr: "Autorisations institutionnelles" },
    "admin.permissions.none": { pt: "Nenhuma permissão de âmbito institucional. Não significa nenhum acesso: pode ter permissões dentro de unidades ou de research workspaces.", en: "No institution-scoped permission. It does not mean no access: they may have permissions within units or research workspaces.", fr: "Aucune autorisation de portée institutionnelle. Cela ne signifie aucun accès : le membre peut avoir des autorisations au sein d’unités ou de research workspaces." },
    // Origem de um acesso (source_label)
    "admin.source.technical_role": { pt: "papel técnico", en: "technical role", fr: "rôle technique" },
    "admin.source.unit_membership": { pt: "membership de unidade", en: "unit membership", fr: "appartenance à une unité" },
    "admin.source.workspace_membership": { pt: "membership de research workspace", en: "research workspace membership", fr: "appartenance à un research workspace" },
    "admin.source.explicit_grant": { pt: "grant explícito", en: "explicit grant", fr: "attribution explicite" },
    "admin.source.unknown": { pt: "origem desconhecida", en: "unknown source", fr: "origine inconnue" },
    // Overview / resumo
    "admin.overview.summary": { pt: "Em resumo", en: "In summary", fr: "En résumé" },
    "admin.mfa.not_required": { pt: "Não exigido", en: "Not required", fr: "Non exigé" },
    "admin.mfa.required_enrolled": { pt: "Exigido e enrolado", en: "Required and enrolled", fr: "Exigé et configuré" },
    "admin.mfa.required_pending": { pt: "Exigido, por enrolar", en: "Required, not yet enrolled", fr: "Exigé, à configurer" },
    "admin.roles.none_short": { pt: "Nenhum", en: "None", fr: "Aucun" },
    // Detalhe do membro
    "admin.member.pill": { pt: "MEMBRO", en: "MEMBER", fr: "MEMBRE" },
    "admin.member.position_prefix": { pt: "Posição institucional: ", en: "Institutional position: ", fr: "Position institutionnelle : " },
    "admin.member.sections_aria": { pt: "Secções do membro", en: "Member sections", fr: "Sections du membre" },
    "admin.tab.overview": { pt: "Overview", en: "Overview", fr: "Vue d’ensemble" },
    "admin.tab.access": { pt: "Acesso", en: "Access", fr: "Accès" },
    "admin.tab.audit": { pt: "Audit", en: "Audit", fr: "Audit" },
    "admin.tab.activity_unavailable": { pt: "A actividade por membro ainda não é uma consulta do Core. A actividade institucional está em «Actividade».", en: "Per-member activity is not a Core query yet. Institutional activity is in “Activity”.", fr: "L’activité par membre n’est pas encore une requête du Core. L’activité institutionnelle est dans « Activité »." },
    "admin.tab.audit_unavailable": { pt: "A auditoria por membro ainda não é uma consulta do Core. O registo institucional está em «Audit».", en: "Per-member auditing is not a Core query yet. The institutional record is in “Audit”.", fr: "L’audit par membre n’est pas encore une requête du Core. Le registre institutionnel est dans « Audit »." },
    // Unidades
    "admin.units.memberships": { pt: "Pertenças a unidades", en: "Unit memberships", fr: "Appartenances aux unités" },
    "admin.units.none_assigned": { pt: "Nenhuma unidade atribuída.", en: "No unit assigned.", fr: "Aucune unité attribuée." },
    "admin.units.assign": { pt: "Atribuir unidade", en: "Assign unit", fr: "Attribuer une unité" },
    "admin.units.none_org": { pt: "Ainda não existem unidades. Crie uma em ", en: "There are no units yet. Create one in ", fr: "Il n’existe pas encore d’unités. Créez-en une dans " },
    "admin.units.before_assign": { pt: " antes de atribuir.", en: " before assigning.", fr: " avant d’attribuer." },
    "admin.units.all_assigned": { pt: "Este membro já pertence a todas as unidades existentes.", en: "This member already belongs to all existing units.", fr: "Ce membre appartient déjà à toutes les unités existantes." },
    "admin.units.choose": { pt: "Escolher unidade…", en: "Choose a unit…", fr: "Choisir une unité…" },
    // Papéis de pertença (unidade e workspace)
    "admin.role.member": { pt: "Membro", en: "Member", fr: "Membre" },
    "admin.role.manager": { pt: "Gestor", en: "Manager", fr: "Gestionnaire" },
    "admin.ws_role.viewer": { pt: "Leitor", en: "Viewer", fr: "Lecteur" },
    "admin.ws_role.lead": { pt: "Lead", en: "Lead", fr: "Responsable" },
    // Colunas de tabela
    "admin.col.role": { pt: "Papel", en: "Role", fr: "Rôle" },
    "admin.col.actions": { pt: "Acções", en: "Actions", fr: "Actions" },
    "admin.col.workspace": { pt: "Workspace", en: "Workspace", fr: "Espace" },
    "admin.col.permission": { pt: "Permissão", en: "Permission", fr: "Autorisation" },
    "admin.col.scope": { pt: "Âmbito", en: "Scope", fr: "Portée" },
    "admin.action.assign": { pt: "Atribuir", en: "Assign", fr: "Attribuer" },
    // Research workspaces
    "admin.ws.memberships": { pt: "Pertenças a research workspaces", en: "Research workspace memberships", fr: "Appartenances aux research workspaces" },
    "admin.ws.none_assigned": { pt: "Nenhum research workspace atribuído.", en: "No research workspace assigned.", fr: "Aucun research workspace attribué." },
    "admin.ws.assign": { pt: "Atribuir research workspace", en: "Assign research workspace", fr: "Attribuer un research workspace" },
    "admin.ws.none_org": { pt: "Ainda não existem research workspaces. Criam-se dentro de uma ideia ou projecto, não aqui.", en: "There are no research workspaces yet. They are created inside an idea or project, not here.", fr: "Il n’existe pas encore de research workspaces. Ils se créent au sein d’une idée ou d’un projet, pas ici." },
    "admin.ws.all_assigned": { pt: "Este membro já pertence a todos os research workspaces visíveis.", en: "This member already belongs to all visible research workspaces.", fr: "Ce membre appartient déjà à tous les research workspaces visibles." },
    "admin.ws.choose": { pt: "Escolher workspace…", en: "Choose a workspace…", fr: "Choisir un espace…" },
    // Gerir papéis técnicos
    "admin.roles.manage": { pt: "Gerir papéis técnicos", en: "Manage technical roles", fr: "Gérer les rôles techniques" },
    "admin.roles.none_assigned_technical": { pt: "Sem papéis técnicos atribuídos.", en: "No technical roles assigned.", fr: "Aucun rôle technique attribué." },
    "admin.roles.all_assigned": { pt: "Este membro já tem todos os papéis do catálogo.", en: "This member already has every role in the catalogue.", fr: "Ce membre a déjà tous les rôles du catalogue." },
    "admin.roles.choose": { pt: "Escolher papel…", en: "Choose a role…", fr: "Choisir un rôle…" },
    "admin.roles.manage_requires": { pt: "Conceder ou revogar papéis técnicos — incluindo tornar um membro administrador da plataforma ou da organização — exige uma sessão de administrador da plataforma com segundo factor activo. Os papéis actuais deste membro estão acima.", en: "Granting or revoking technical roles — including making a member a platform or organisation administrator — requires a platform administrator session with an active second factor. This member’s current roles are above.", fr: "Accorder ou révoquer des rôles techniques — y compris faire d’un membre un administrateur de la plateforme ou de l’organisation — exige une session d’administrateur de la plateforme avec un second facteur actif. Les rôles actuels de ce membre figurent ci-dessus." },
    "admin.action.grant": { pt: "Conceder", en: "Grant", fr: "Accorder" },
    "admin.reason.audit_ph": { pt: "Razão (fica no registo de auditoria)", en: "Reason (kept in the audit log)", fr: "Motif (conservé dans le journal d’audit)" },
    "admin.reason.ph": { pt: "Razão", en: "Reason", fr: "Motif" },
    // Gerir grants institucionais
    "admin.grants.manage": { pt: "Gerir grants institucionais", en: "Manage institutional grants", fr: "Gérer les attributions institutionnelles" },
    "admin.grants.none_active": { pt: "Sem grants institucionais activos. O acesso deste membro vem apenas de papéis e memberships.", en: "No active institutional grants. This member’s access comes only from roles and memberships.", fr: "Aucune attribution institutionnelle active. L’accès de ce membre provient uniquement des rôles et des appartenances." },
    "admin.grants.catalog_unavailable": { pt: "Catálogo de permissões indisponível.", en: "Permissions catalogue unavailable.", fr: "Catalogue des autorisations indisponible." },
    "admin.grants.choose": { pt: "Escolher permissão…", en: "Choose a permission…", fr: "Choisir une autorisation…" },
    "admin.grants.grant_submit": { pt: "Conceder grant", en: "Grant", fr: "Accorder l’attribution" },
    // Transições de estado da conta
    "admin.transition.suspend": { pt: "Suspender — barra o acesso, preserva a autoria", en: "Suspend — bars access, preserves authorship", fr: "Suspendre — bloque l’accès, préserve la paternité" },
    "admin.transition.disable": { pt: "Desactivar — barra permanentemente, mantém o histórico", en: "Disable — bars permanently, keeps the history", fr: "Désactiver — bloque définitivement, conserve l’historique" },
    "admin.transition.disable_short": { pt: "Desactivar — barra permanentemente", en: "Disable — bars permanently", fr: "Désactiver — bloque définitivement" },
    "admin.transition.reactivate": { pt: "Reactivar — devolve o acesso", en: "Reactivate — restores access", fr: "Réactiver — rétablit l’accès" },
    // Gerir credencial e estado
    "admin.account.manage": { pt: "Gerir credencial e estado", en: "Manage credential and status", fr: "Gérer l’identifiant et l’état" },
    "admin.account.reset_note": { pt: "Repor a palavra-passe emite uma credencial temporária nova, invalida a definitiva e termina todas as sessões abertas. A palavra-passe nova é mostrada uma única vez, no ecrã seguinte.", en: "Resetting the password issues a new temporary credential, invalidates the permanent one and ends all open sessions. The new password is shown only once, on the next screen.", fr: "Réinitialiser le mot de passe émet un nouvel identifiant temporaire, invalide le mot de passe définitif et met fin à toutes les sessions ouvertes. Le nouveau mot de passe est affiché une seule fois, à l’écran suivant." },
    "admin.account.reset_submit": { pt: "Repor palavra-passe", en: "Reset password", fr: "Réinitialiser le mot de passe" },
    "admin.account.no_transitions": { pt: "Não há transições de estado disponíveis a partir do estado actual.", en: "There are no state transitions available from the current state.", fr: "Aucune transition d’état n’est disponible à partir de l’état actuel." },
    "admin.account.change_state": { pt: "Alterar estado para…", en: "Change status to…", fr: "Changer l’état pour…" },
    "admin.action.apply": { pt: "Aplicar", en: "Apply", fr: "Appliquer" },
};

/// Os oito ecrãs de lista (Unidades, Ideias, Projectos, Bibliografia, Dados,
/// Agentes, Membros, Audit Log) e os seus formulários de criação. Os valores de
/// domínio das linhas vêm do Core e não se traduzem; o que aqui vive é o chrome —
/// cabeçalhos de coluna, separadores, estados vazios, rótulos e placeholders.
const LISTS: &[Entry] = catalogo! {
    // O filtro partilhado da tabela institucional (interpola o nome plural).
    "table.filter": { pt: "Filtrar {noun}…", en: "Filter {noun}…", fr: "Filtrer {noun}…" },
    "table.filter_page": {
        pt: "Filtrar {noun} nesta página…",
        en: "Filter {noun} on this page…",
        fr: "Filtrer {noun} sur cette page…"
    },
    // Cabeçalhos de coluna (mono, sem transformação de caixa na folha de estilos).
    "lists.col.unit": { pt: "UNIDADE", en: "UNIT", fr: "UNITÉ" },
    "lists.col.code": { pt: "CÓDIGO", en: "CODE", fr: "CODE" },
    "lists.col.lead": { pt: "RESPONSÁVEL", en: "LEAD", fr: "RESPONSABLE" },
    "lists.col.members": { pt: "MEMBROS", en: "MEMBERS", fr: "MEMBRES" },
    "lists.col.ideas": { pt: "IDEIAS", en: "IDEAS", fr: "IDÉES" },
    "lists.col.projects": { pt: "PROJECTOS", en: "PROJECTS", fr: "PROJETS" },
    "lists.col.state": { pt: "ESTADO", en: "STATE", fr: "ÉTAT" },
    "lists.col.title": { pt: "TÍTULO", en: "TITLE", fr: "TITRE" },
    "lists.col.priority": { pt: "PRIORIDADE", en: "PRIORITY", fr: "PRIORITÉ" },
    "lists.col.classification": { pt: "CLASSIFICAÇÃO", en: "CLASSIFICATION", fr: "CLASSIFICATION" },
    "lists.col.updated": { pt: "ACTUALIZADA", en: "UPDATED", fr: "MISE À JOUR" },
    "lists.col.project": { pt: "PROJECTO", en: "PROJECT", fr: "PROJET" },
    "lists.col.progress": { pt: "PROGRESSO", en: "PROGRESS", fr: "PROGRÈS" },
    "lists.col.start": { pt: "INÍCIO", en: "START", fr: "DÉBUT" },
    "lists.col.end": { pt: "FIM", en: "END", fr: "FIN" },
    "lists.col.authors": { pt: "AUTORES", en: "AUTHORS", fr: "AUTEURS" },
    "lists.col.year": { pt: "ANO", en: "YEAR", fr: "ANNÉE" },
    "lists.col.origin": { pt: "ORIGEM", en: "SOURCE", fr: "ORIGINE" },
    "lists.col.type": { pt: "TIPO", en: "TYPE", fr: "TYPE" },
    "lists.col.citations": { pt: "CITAÇÕES", en: "CITATIONS", fr: "CITATIONS" },
    "lists.col.name": { pt: "NOME", en: "NAME", fr: "NOM" },
    "lists.col.registered": { pt: "REGISTO", en: "REGISTERED", fr: "INSCRIPTION" },
    "lists.col.version": { pt: "VERSÃO", en: "VERSION", fr: "VERSION" },
    "lists.col.size": { pt: "TAMANHO", en: "SIZE", fr: "TAILLE" },
    "lists.col.access": { pt: "ACESSO", en: "ACCESS", fr: "ACCÈS" },
    "lists.col.agent": { pt: "AGENTE", en: "AGENT", fr: "AGENT" },
    "lists.col.purpose": { pt: "PROPÓSITO", en: "PURPOSE", fr: "OBJET" },
    "lists.col.scope": { pt: "ÂMBITO", en: "SCOPE", fr: "PORTÉE" },
    "lists.col.capability": { pt: "CAPACIDADE", en: "CAPABILITY", fr: "CAPACITÉ" },
    "lists.col.created": { pt: "CRIADO", en: "CREATED", fr: "CRÉÉ" },
    "lists.col.email": { pt: "E-MAIL", en: "EMAIL", fr: "E-MAIL" },
    "lists.col.position": { pt: "POSIÇÃO", en: "POSITION", fr: "POSITION" },
    "lists.col.activity": { pt: "ACTIVIDADE", en: "ACTIVITY", fr: "ACTIVITÉ" },
    "lists.col.date": { pt: "DATA", en: "DATE", fr: "DATE" },
    "lists.col.user": { pt: "UTILIZADOR", en: "USER", fr: "UTILISATEUR" },
    "lists.col.action": { pt: "ACÇÃO", en: "ACTION", fr: "ACTION" },
    "lists.col.resource": { pt: "RECURSO", en: "RESOURCE", fr: "RESSOURCE" },
    "lists.col.context": { pt: "CONTEXTO", en: "CONTEXT", fr: "CONTEXTE" },
    "lists.col.outcome": { pt: "RESULTADO", en: "OUTCOME", fr: "RÉSULTAT" },
    "lists.col.correlation_id": { pt: "CORRELATION ID", en: "CORRELATION ID", fr: "ID DE CORRÉLATION" },
    // Nomes plurais para o campo de filtro da tabela.
    "lists.noun.units": { pt: "unidades", en: "units", fr: "unités" },
    "lists.noun.ideas": { pt: "ideias", en: "ideas", fr: "idées" },
    "lists.noun.projects": { pt: "projectos", en: "projects", fr: "projets" },
    "lists.noun.sources": { pt: "referências", en: "references", fr: "références" },
    "lists.noun.datasets": { pt: "datasets", en: "datasets", fr: "jeux de données" },
    "lists.noun.agents": { pt: "agentes", en: "agents", fr: "agents" },
    "lists.noun.members": { pt: "membros", en: "members", fr: "membres" },
    "lists.noun.events": { pt: "eventos", en: "events", fr: "événements" },
    "lists.noun.tasks": { pt: "tarefas", en: "tasks", fr: "tâches" },
    // Separadores partilhados dos recortes.
    "lists.tab.all_f": { pt: "Todas", en: "All", fr: "Toutes" },
    "lists.tab.all_m": { pt: "Todos", en: "All", fr: "Tous" },
    "lists.tab.mine_f": { pt: "Minhas", en: "Mine", fr: "Les miennes" },
    "lists.tab.mine_m": { pt: "Meus", en: "Mine", fr: "Les miens" },
    "lists.tab.followed_f": { pt: "Seguidas", en: "Followed", fr: "Suivies" },
    "lists.tab.archived_f": { pt: "Arquivadas", en: "Archived", fr: "Archivées" },
    "lists.tab.completed_m": { pt: "Concluídos", en: "Completed", fr: "Terminés" },
    "lists.tab.favourites_f": { pt: "Favoritas", en: "Favourites", fr: "Favoris" },
    "lists.tab.favourites_m": { pt: "Favoritos", en: "Favourites", fr: "Favoris" },
    "lists.slice.unit": { pt: "Da Unidade", en: "By Unit", fr: "De l’unité" },
    "lists.slice.unit.none": {
        pt: "Não pertence a nenhuma unidade que possa usar como recorte.",
        en: "You do not belong to any unit you can use as a view.",
        fr: "Vous n’appartenez à aucune unité utilisable comme vue."
    },
    "lists.slice.state_not_query": {
        pt: "Recortar por estado ainda não é uma consulta do Core.",
        en: "Slicing by state is not yet a Core query.",
        fr: "Découper par état n’est pas encore une requête du Core."
    },
    "lists.slice.followed_ideas": {
        pt: "Seguir ideias ainda não existe no Ocinye OS.",
        en: "Following ideas does not exist yet in Ocinye OS.",
        fr: "Suivre des idées n’existe pas encore dans Ocinye OS."
    },
    "lists.favourites_none": {
        pt: "Marcar favoritos ainda não existe no Ocinye OS.",
        en: "Marking favourites does not exist yet in Ocinye OS.",
        fr: "Marquer des favoris n’existe pas encore dans Ocinye OS."
    },
    // Selector de unidade.
    "lists.unit_pick.label": { pt: "UNIDADE", en: "UNIT", fr: "UNITÉ" },
    "lists.unit_pick.placeholder": { pt: "Escolha uma unidade…", en: "Choose a unit…", fr: "Choisissez une unité…" },
    "lists.unit_pick.apply": { pt: "Aplicar", en: "Apply", fr: "Appliquer" },
    "lists.choose_unit_prompt": {
        pt: "Escolha a unidade cujo trabalho quer ver.",
        en: "Choose the unit whose work you want to see.",
        fr: "Choisissez l’unité dont vous voulez voir le travail."
    },
    "lists.no_unit_chosen.title": { pt: "Nenhuma unidade escolhida", en: "No unit chosen", fr: "Aucune unité choisie" },
    "lists.no_unit_chosen.body": {
        pt: "O Ocinye OS não escolhe uma unidade por si. Escolha acima qual delas quer ver.",
        en: "Ocinye OS does not choose a unit for you. Choose above which one you want to see.",
        fr: "Ocinye OS ne choisit pas d’unité à votre place. Choisissez ci-dessus celle que vous voulez voir."
    },
    "lists.subtitle.unit": { pt: "Unidade: {unit}.", en: "Unit: {unit}.", fr: "Unité : {unit}." },
    // Botões de criação e navegação partilhados.
    "lists.action.unauthorised": {
        pt: "Não tem autorização para esta acção.",
        en: "You are not authorised for this action.",
        fr: "Vous n’êtes pas autorisé pour cette action."
    },
    "lists.new.unit": { pt: "Nova Unidade", en: "New Unit", fr: "Nouvelle unité" },
    "lists.new.agent": { pt: "Novo Agente", en: "New Agent", fr: "Nouvel agent" },
    "lists.new.member": { pt: "Adicionar Utilizador", en: "Add User", fr: "Ajouter un utilisateur" },
    "lists.edit.unit": { pt: "Editar Unidade", en: "Edit Unit", fr: "Modifier l’unité" },
    "lists.see_units": { pt: "Ver Unidades", en: "View Units", fr: "Voir les unités" },
    "lists.see_units_lc": { pt: "Ver unidades", en: "View units", fr: "Voir les unités" },
    "lists.see_ideas": { pt: "Ver Ideias", en: "View Ideas", fr: "Voir les idées" },
    // Subtítulos e estados vazios de cada lista.
    "lists.units.subtitle": {
        pt: "Todas as unidades institucionais da Ocinye.",
        en: "All of Ocinye’s institutional units.",
        fr: "Toutes les unités institutionnelles d’Ocinye."
    },
    "lists.units.empty": {
        pt: "Ainda não existem unidades. Uma unidade é criada por um administrador da organização.",
        en: "There are no units yet. A unit is created by an organisation administrator.",
        fr: "Il n’existe pas encore d’unités. Une unité est créée par un administrateur de l’organisation."
    },
    "lists.units.mine_none": {
        pt: "As unidades a que pertence ainda não são um recorte desta lista.",
        en: "The units you belong to are not yet a view of this list.",
        fr: "Les unités auxquelles vous appartenez ne sont pas encore une vue de cette liste."
    },
    "lists.units.followed_none": {
        pt: "Seguir unidades ainda não existe no Ocinye OS.",
        en: "Following units does not exist yet in Ocinye OS.",
        fr: "Suivre des unités n’existe pas encore dans Ocinye OS."
    },
    "lists.units.archived_none": {
        pt: "Ver apenas as unidades arquivadas ainda não é um recorte desta lista.",
        en: "Seeing only archived units is not yet a view of this list.",
        fr: "Voir uniquement les unités archivées n’est pas encore une vue de cette liste."
    },
    "lists.ideas.subtitle": {
        pt: "Uma ideia é explorada antes de se tornar projecto.",
        en: "An idea is explored before it becomes a project.",
        fr: "Une idée est explorée avant de devenir un projet."
    },
    "lists.ideas.empty": {
        pt: "Ainda não existem ideias. Uma ideia pertence a uma unidade e é o ponto de partida da investigação.",
        en: "There are no ideas yet. An idea belongs to a unit and is the starting point of research.",
        fr: "Il n’existe pas encore d’idées. Une idée appartient à une unité et est le point de départ de la recherche."
    },
    "lists.projects.subtitle": {
        pt: "Projectos institucionais em execução e planeamento.",
        en: "Institutional projects in progress and planning.",
        fr: "Projets institutionnels en cours et en planification."
    },
    "lists.projects.empty": {
        pt: "Ainda não existem projectos. Um projecto nasce da promoção de uma ideia.",
        en: "There are no projects yet. A project is born from promoting an idea.",
        fr: "Il n’existe pas encore de projets. Un projet naît de la promotion d’une idée."
    },
    "lists.biblio.subtitle": {
        pt: "Referências ligadas a ideias, projectos e unidades.",
        en: "References linked to ideas, projects and units.",
        fr: "Références liées aux idées, projets et unités."
    },
    "lists.biblio.empty": {
        pt: "Ainda não há referências. A bibliografia é acrescentada dentro de um Research Workspace.",
        en: "There are no references yet. Bibliography is added inside a Research Workspace.",
        fr: "Il n’y a pas encore de références. La bibliographie s’ajoute au sein d’un Research Workspace."
    },
    "lists.biblio.mine_none": {
        pt: "As referências que criou ainda não são um recorte desta lista.",
        en: "The references you created are not yet a view of this list.",
        fr: "Les références que vous avez créées ne sont pas encore une vue de cette liste."
    },
    "lists.biblio.unit_none": {
        pt: "Filtrar a bibliografia por unidade ainda não é um recorte desta lista.",
        en: "Filtering the bibliography by unit is not yet a view of this list.",
        fr: "Filtrer la bibliographie par unité n’est pas encore une vue de cette liste."
    },
    "lists.tools.link": { pt: "Ferramentas", en: "Tools", fr: "Outils" },
    "lists.datasets.subtitle": {
        pt: "Datasets institucionais com versão, proveniência e classificação.",
        en: "Institutional datasets with version, provenance and classification.",
        fr: "Jeux de données institutionnels avec version, provenance et classification."
    },
    "lists.datasets.empty": {
        pt: "Ainda não há datasets catalogados. Um dataset é catalogado dentro de um Research Workspace.",
        en: "There are no catalogued datasets yet. A dataset is catalogued inside a Research Workspace.",
        fr: "Il n’y a pas encore de jeux de données catalogués. Un jeu de données est catalogué au sein d’un Research Workspace."
    },
    "lists.datasets.mine_none": {
        pt: "Os datasets que criou ainda não são um recorte desta lista.",
        en: "The datasets you created are not yet a view of this list.",
        fr: "Les jeux de données que vous avez créés ne sont pas encore une vue de cette liste."
    },
    "lists.datasets.unit_none": {
        pt: "Filtrar datasets por unidade ainda não é um recorte desta lista.",
        en: "Filtering datasets by unit is not yet a view of this list.",
        fr: "Filtrer les jeux de données par unité n’est pas encore une vue de cette liste."
    },
    "lists.agents.subtitle": {
        pt: "Agentes de IA criados e configurados pelos membros.",
        en: "AI agents created and configured by members.",
        fr: "Agents IA créés et configurés par les membres."
    },
    "lists.agents.empty": {
        pt: "Ainda não existem agentes. Um agente é definido por capacidade; só responderá quando existir um nó de IA da Ocinye.",
        en: "There are no agents yet. An agent is defined by capability; it will only answer once an Ocinye AI node exists.",
        fr: "Il n’existe pas encore d’agents. Un agent est défini par capacité ; il ne répondra que lorsqu’un nœud d’IA Ocinye existera."
    },
    "lists.agents.mine_none": {
        pt: "Os agentes que criou ainda não são um recorte desta lista.",
        en: "The agents you created are not yet a view of this list.",
        fr: "Les agents que vous avez créés ne sont pas encore une vue de cette liste."
    },
    "lists.agents.unit_none": {
        pt: "Filtrar agentes por unidade ainda não é um recorte desta lista.",
        en: "Filtering agents by unit is not yet a view of this list.",
        fr: "Filtrer les agents par unité n’est pas encore une vue de cette liste."
    },
    "lists.agents.tab_institutional": { pt: "Institucionais", en: "Institutional", fr: "Institutionnels" },
    "lists.agents.institutional_none": {
        pt: "Distinguir agentes institucionais dos pessoais ainda não é um recorte desta lista.",
        en: "Telling institutional agents from personal ones is not yet a view of this list.",
        fr: "Distinguer les agents institutionnels des agents personnels n’est pas encore une vue de cette liste."
    },
    // Membros.
    "lists.members.tab": { pt: "Membros", en: "Members", fr: "Membres" },
    "lists.members.subtitle": {
        pt: "Administração · membros da instituição.",
        en: "Administration · members of the institution.",
        fr: "Administration · membres de l’institution."
    },
    "lists.members.empty": {
        pt: "Ainda não há membros registados.",
        en: "There are no registered members yet.",
        fr: "Il n’y a pas encore de membres enregistrés."
    },
    "lists.members.tab_roles": { pt: "Funções", en: "Roles", fr: "Rôles" },
    "lists.members.roles_none": {
        pt: "Gerir funções por ecrã próprio ainda não existe.",
        en: "Managing roles from a screen of their own does not exist yet.",
        fr: "Gérer les rôles depuis un écran dédié n’existe pas encore."
    },
    "lists.members.tab_access": { pt: "Acessos", en: "Access", fr: "Accès" },
    "lists.members.access_none": {
        pt: "O acesso de cada membro gere-se no seu separador «Segurança». Não há convite por email; uma vista de todos os acessos num só ecrã ainda não existe.",
        en: "Each member’s access is managed in their «Security» tab. There is no email invitation; a view of all access on one screen does not exist yet.",
        fr: "L’accès de chaque membre se gère dans son onglet « Sécurité ». Il n’y a pas d’invitation par courriel ; une vue de tous les accès sur un seul écran n’existe pas encore."
    },
    "lists.members.tab_services": { pt: "Serviços", en: "Services", fr: "Services" },
    "lists.members.services_none": {
        pt: "A administração de serviços ainda não existe.",
        en: "Service administration does not exist yet.",
        fr: "L’administration des services n’existe pas encore."
    },
    // Audit Log.
    "lists.audit.subtitle": {
        pt: "Registo técnico e imutável de operações do Ocinye OS.",
        en: "Technical, immutable record of Ocinye OS operations.",
        fr: "Registre technique et immuable des opérations d’Ocinye OS."
    },
    "lists.audit.empty": {
        pt: "Sem eventos de auditoria para os filtros aplicados.",
        en: "No audit events for the filters applied.",
        fr: "Aucun événement d’audit pour les filtres appliqués."
    },
    "lists.audit.tab_auth": { pt: "Autenticação", en: "Authentication", fr: "Authentification" },
    "lists.audit.tab_permissions": { pt: "Permissões", en: "Permissions", fr: "Permissions" },
    "lists.audit.tab_ai": { pt: "IA", en: "AI", fr: "IA" },
    "lists.audit.category_none": {
        pt: "Recortar o registo por categoria ainda não é uma consulta do Core.",
        en: "Slicing the record by category is not yet a Core query.",
        fr: "Découper le registre par catégorie n’est pas encore une requête du Core."
    },
    "lists.audit.denied": { pt: "NEGADO", en: "DENIED", fr: "REFUSÉ" },
    "lists.audit.warn": { pt: "AVISO", en: "WARNING", fr: "AVERTISSEMENT" },
    // Selector de destino de criação.
    "lists.research_workspace": { pt: "Research Workspace", en: "Research Workspace", fr: "Research Workspace" },
    "lists.destination.hint": {
        pt: "Só aparecem ambientes onde tem autorização para criar.",
        en: "Only environments where you are authorised to create appear.",
        fr: "Seuls apparaissent les environnements où vous êtes autorisé à créer."
    },
    "lists.no_destination.title": {
        pt: "Não tem onde criar {what}",
        en: "You have nowhere to create {what}",
        fr: "Vous n’avez nulle part où créer {what}"
    },
    "lists.no_destination.body": {
        pt: "Estes artefactos pertencem a um Research Workspace, e não pertence a nenhum onde possa criar. A filiação é concedida por quem gere a unidade.",
        en: "These artefacts belong to a Research Workspace, and you belong to none where you can create. Membership is granted by whoever manages the unit.",
        fr: "Ces artefacts appartiennent à un Research Workspace, et vous n’appartenez à aucun où vous pouvez créer. L’appartenance est accordée par celui qui gère l’unité."
    },
    // Campos de formulário (rótulos e placeholders).
    "lists.field.title": { pt: "Título", en: "Title", fr: "Titre" },
    "lists.field.authors": { pt: "Autores", en: "Authors", fr: "Auteurs" },
    "lists.field.year": { pt: "Ano", en: "Year", fr: "Année" },
    "lists.field.publication": { pt: "Publicação", en: "Publication", fr: "Publication" },
    "lists.field.abstract": { pt: "Resumo", en: "Abstract", fr: "Résumé" },
    "lists.field.summary": { pt: "Resumo", en: "Summary", fr: "Résumé" },
    "lists.field.classification": { pt: "Classificação", en: "Classification", fr: "Classification" },
    "lists.field.code": { pt: "Código", en: "Code", fr: "Code" },
    "lists.field.description": { pt: "Descrição", en: "Description", fr: "Description" },
    "lists.field.keywords": { pt: "Palavras-chave", en: "Keywords", fr: "Mots-clés" },
    "lists.field.usage_restrictions": { pt: "Restrições de uso", en: "Usage restrictions", fr: "Restrictions d’usage" },
    "lists.field.priority": { pt: "Prioridade", en: "Priority", fr: "Priorité" },
    "lists.field.due": { pt: "Prazo", en: "Due date", fr: "Échéance" },
    "lists.field.project_code": { pt: "Código do projecto", en: "Project code", fr: "Code du projet" },
    "lists.field.objectives": { pt: "Objectivos", en: "Objectives", fr: "Objectifs" },
    "lists.field.name": { pt: "Nome", en: "Name", fr: "Nom" },
    "lists.field.research_areas": { pt: "Áreas de investigação", en: "Research areas", fr: "Domaines de recherche" },
    "lists.field.research_question": { pt: "Pergunta de investigação", en: "Research question", fr: "Question de recherche" },
    "lists.field.hypothesis": { pt: "Hipótese", en: "Hypothesis", fr: "Hypothèse" },
    "lists.field.motivation": { pt: "Motivação", en: "Motivation", fr: "Motivation" },
    "lists.field.unit": { pt: "Unidade", en: "Unit", fr: "Unité" },
    "lists.priority.critical": { pt: "Crítica", en: "Critical", fr: "Critique" },
    "lists.ph.work_title": { pt: "Título da obra", en: "Title of the work", fr: "Titre de l’œuvre" },
    "lists.ph.semicolon": {
        pt: "separados por ponto e vírgula",
        en: "separated by semicolons",
        fr: "séparés par des points-virgules"
    },
    "lists.ph.year": { pt: "Ex.: 2024", en: "e.g. 2024", fr: "p. ex. 2024" },
    "lists.ph.venue": {
        pt: "Revista, conferência ou colecção",
        en: "Journal, conference or collection",
        fr: "Revue, conférence ou collection"
    },
    "lists.ph.doi": { pt: "10.xxxx/xxxxx", en: "10.xxxx/xxxxx", fr: "10.xxxx/xxxxx" },
    "lists.ph.abstract_work": { pt: "Resumo da obra", en: "Abstract of the work", fr: "Résumé de l’œuvre" },
    "lists.ph.dataset_code": { pt: "Ex.: DS-0001", en: "e.g. DS-0001", fr: "p. ex. DS-0001" },
    "lists.ph.dataset_name": { pt: "Nome do conjunto", en: "Name of the set", fr: "Nom de l’ensemble" },
    "lists.ph.dataset_desc": {
        pt: "O que o conjunto contém e como foi obtido",
        en: "What the set contains and how it was obtained",
        fr: "Ce que l’ensemble contient et comment il a été obtenu"
    },
    "lists.ph.comma_separated": {
        pt: "separadas por vírgulas",
        en: "comma-separated",
        fr: "séparées par des virgules"
    },
    "lists.ph.usage_limits": {
        pt: "Limites de utilização, quando existam",
        en: "Usage limits, where they exist",
        fr: "Limites d’utilisation, lorsqu’elles existent"
    },
    "lists.ph.task_title": { pt: "O que precisa de ser feito", en: "What needs to be done", fr: "Ce qui doit être fait" },
    "lists.ph.task_desc": { pt: "Detalhes, quando ajudam", en: "Details, when they help", fr: "Détails, lorsqu’ils aident" },
    "lists.ph.project_code": { pt: "Ex.: PPEC-2026-001", en: "e.g. PPEC-2026-001", fr: "p. ex. PPEC-2026-001" },
    "lists.ph.project_title": {
        pt: "Deixe vazio para manter o título da ideia",
        en: "Leave empty to keep the idea’s title",
        fr: "Laissez vide pour garder le titre de l’idée"
    },
    "lists.ph.objectives": {
        pt: "O que o projecto se propõe alcançar",
        en: "What the project aims to achieve",
        fr: "Ce que le projet se propose d’atteindre"
    },
    "lists.ph.unit_name": {
        pt: "Ex.: Unidade de Energias Renováveis",
        en: "e.g. Renewable Energy Unit",
        fr: "p. ex. Unité d’énergies renouvelables"
    },
    "lists.ph.unit_investigates": {
        pt: "O que esta unidade investiga",
        en: "What this unit investigates",
        fr: "Ce que cette unité étudie"
    },
    "lists.ph.idea_title": { pt: "O que se quer investigar", en: "What you want to investigate", fr: "Ce que vous voulez étudier" },
    "lists.ph.research_question": {
        pt: "Que pergunta é que isto responde",
        en: "What question this answers",
        fr: "À quelle question cela répond"
    },
    "lists.ph.hypothesis": {
        pt: "A hipótese, quando já existe uma",
        en: "The hypothesis, when there is one",
        fr: "L’hypothèse, lorsqu’il en existe une"
    },
    "lists.ph.motivation": {
        pt: "Porque é que isto importa à instituição",
        en: "Why this matters to the institution",
        fr: "Pourquoi cela importe à l’institution"
    },
    "lists.ph.idea_summary": { pt: "Resumo da ideia", en: "Summary of the idea", fr: "Résumé de l’idée" },
    "lists.ph.bibtex": {
        pt: "@article{chave, title = {…}, author = {…}, year = {…}}",
        en: "@article{chave, title = {…}, author = {…}, year = {…}}",
        fr: "@article{chave, title = {…}, author = {…}, year = {…}}"
    },
    // Cabeçalhos de secção dos formulários.
    "lists.new_source.section": { pt: "A REFERÊNCIA", en: "THE REFERENCE", fr: "LA RÉFÉRENCE" },
    "lists.new_source.intro": {
        pt: "Uma referência pertence ao Research Workspace onde a investigação que a cita acontece.",
        en: "A reference belongs to the Research Workspace where the research that cites it happens.",
        fr: "Une référence appartient au Research Workspace où se déroule la recherche qui la cite."
    },
    "lists.new_dataset.section": { pt: "O DATASET", en: "THE DATASET", fr: "LE JEU DE DONNÉES" },
    "lists.new_dataset.intro": {
        pt: "Um dataset pertence ao Research Workspace que o produz ou o usa, e herda dele o contexto institucional.",
        en: "A dataset belongs to the Research Workspace that produces or uses it, and inherits its institutional context.",
        fr: "Un jeu de données appartient au Research Workspace qui le produit ou l’utilise, et en hérite le contexte institutionnel."
    },
    "lists.new_dataset.class_note": {
        pt: "A classificação do dataset pode ser mais restrita do que a do ambiente, e governa quem o alcança.",
        en: "The dataset’s classification can be more restrictive than the environment’s, and governs who reaches it.",
        fr: "La classification du jeu de données peut être plus restrictive que celle de l’environnement, et gouverne qui y accède."
    },
    "lists.new_task.section": { pt: "A TAREFA", en: "THE TASK", fr: "LA TÂCHE" },
    "lists.new_task.intro": {
        pt: "Uma tarefa é uma unidade de trabalho dentro de um Research Workspace — a ideia ou o projecto a que pertence.",
        en: "A task is a unit of work inside a Research Workspace — the idea or project it belongs to.",
        fr: "Une tâche est une unité de travail au sein d’un Research Workspace — l’idée ou le projet auquel elle appartient."
    },
    "lists.new_task.responsible_note": {
        pt: "O responsável escolhe-se no detalhe da tarefa: quem pode ser atribuído depende do ambiente.",
        en: "The assignee is chosen in the task detail: who can be assigned depends on the environment.",
        fr: "Le responsable se choisit dans le détail de la tâche : qui peut être assigné dépend de l’environnement."
    },
    "lists.new_project.section": { pt: "A IDEIA A PROMOVER", en: "THE IDEA TO PROMOTE", fr: "L’IDÉE À PROMOUVOIR" },
    "lists.new_project.intro": {
        pt: "Um projecto nasce da promoção de uma ideia. O Research Workspace acompanha-a, com tudo o que foi reunido enquanto se explorava.",
        en: "A project is born from promoting an idea. The Research Workspace comes with it, with everything gathered while exploring.",
        fr: "Un projet naît de la promotion d’une idée. Le Research Workspace l’accompagne, avec tout ce qui a été réuni pendant l’exploration."
    },
    "lists.new_project.eligible_idea": { pt: "Ideia elegível", en: "Eligible idea", fr: "Idée éligible" },
    "lists.new_project.eligible_hint": {
        pt: "Só aparecem ideias em estado de candidatura a projecto, dentro do que lhe está acessível.",
        en: "Only ideas in project-candidate state appear, within what is accessible to you.",
        fr: "Seules apparaissent les idées à l’état de candidature à projet, parmi ce qui vous est accessible."
    },
    "lists.new_project.none_title": {
        pt: "Não existem ideias elegíveis para promoção",
        en: "There are no ideas eligible for promotion",
        fr: "Il n’existe pas d’idées éligibles à la promotion"
    },
    "lists.new_project.none_body": {
        pt: "Um projecto nasce de uma ideia que chegou a candidatura. Nenhuma das ideias a que tem acesso está nesse estado.",
        en: "A project is born from an idea that reached candidature. None of the ideas you can access is in that state.",
        fr: "Un projet naît d’une idée parvenue à candidature. Aucune des idées auxquelles vous avez accès n’est dans cet état."
    },
    "lists.new_unit.section": { pt: "A UNIDADE", en: "THE UNIT", fr: "L’UNITÉ" },
    "lists.new_unit.intro": {
        pt: "Uma unidade é o âmbito institucional onde a investigação acontece. As ideias, os projectos e as filiações vivem dentro de uma.",
        en: "A unit is the institutional scope where research happens. Ideas, projects and memberships live inside one.",
        fr: "Une unité est le cadre institutionnel où se déroule la recherche. Les idées, les projets et les appartenances vivent au sein d’une unité."
    },
    "lists.new_unit.code_generated": {
        pt: "Gerado automaticamente a partir do nome, no formato U<SIGLA>-NNN.",
        en: "Generated automatically from the name, in the format U<CODE>-NNN.",
        fr: "Généré automatiquement à partir du nom, au format U<SIGLE>-NNN."
    },
    "lists.edit_unit.subtitle": {
        pt: "O nome, a descrição e as áreas mudam. O código não — é a identidade da unidade.",
        en: "The name, description and areas change. The code does not — it is the unit’s identity.",
        fr: "Le nom, la description et les domaines changent. Le code, non — c’est l’identité de l’unité."
    },
    "lists.edit_unit.code_note": {
        pt: "O código é a identidade da unidade e não muda.",
        en: "The code is the unit’s identity and does not change.",
        fr: "Le code est l’identité de l’unité et ne change pas."
    },
    "lists.new_idea.section": { pt: "A IDEIA", en: "THE IDEA", fr: "L’IDÉE" },
    "lists.new_idea.intro": {
        pt: "Uma ideia é exploratória. Nem todas se tornam projectos, e isso é um desfecho legítimo.",
        en: "An idea is exploratory. Not all become projects, and that is a legitimate outcome.",
        fr: "Une idée est exploratoire. Toutes ne deviennent pas des projets, et c’est une issue légitime."
    },
    "lists.new_idea.class_note": {
        pt: "A classificação governa tudo o que for acrescentado a este Research Workspace.",
        en: "The classification governs everything added to this Research Workspace.",
        fr: "La classification gouverne tout ce qui est ajouté à ce Research Workspace."
    },
    "lists.new_idea.no_units_title": {
        pt: "Ainda não existem unidades",
        en: "There are no units yet",
        fr: "Il n’existe pas encore d’unités"
    },
    "lists.new_idea.no_units_body": {
        pt: "Uma ideia pertence sempre a uma unidade científica. Peça a um administrador que crie a primeira.",
        en: "An idea always belongs to a scientific unit. Ask an administrator to create the first one.",
        fr: "Une idée appartient toujours à une unité scientifique. Demandez à un administrateur d’en créer la première."
    },
    // Botões «Criar …» dos formulários.
    "lists.create.reference_btn": { pt: "Criar Referência", en: "Create Reference", fr: "Créer la référence" },
    "lists.create.dataset_btn": { pt: "Criar Dataset", en: "Create Dataset", fr: "Créer le jeu de données" },
    "lists.create.task_btn": { pt: "Criar Tarefa", en: "Create Task", fr: "Créer la tâche" },
    "lists.promote_btn": { pt: "Promover a Projecto", en: "Promote to Project", fr: "Promouvoir en projet" },
    "lists.create.unit_btn": { pt: "Criar Unidade", en: "Create Unit", fr: "Créer l’unité" },
    "lists.create.idea_btn": { pt: "Criar Ideia", en: "Create Idea", fr: "Créer l’idée" },
    // Ferramentas bibliográficas.
    "lists.tools.title": { pt: "Ferramentas bibliográficas", en: "Bibliographic tools", fr: "Outils bibliographiques" },
    "lists.tools.intro": {
        pt: "Valida a estrutura de referências BibTeX e produz uma versão normalizada. A leitura acontece no Ocinye OS, sem consultar serviços externos: nenhum DOI é verificado e nenhuma referência é confirmada.",
        en: "It validates the structure of BibTeX references and produces a normalised version. Reading happens in Ocinye OS, without consulting external services: no DOI is verified and no reference is confirmed.",
        fr: "Il valide la structure des références BibTeX et produit une version normalisée. La lecture se fait dans Ocinye OS, sans consulter de services externes : aucun DOI n’est vérifié et aucune référence n’est confirmée."
    },
    "lists.tools.section": { pt: "BIBLIOGRAFIA", en: "BIBLIOGRAPHY", fr: "BIBLIOGRAPHIE" },
    "lists.tools.validate": { pt: "Validar e normalizar", en: "Validate and normalise", fr: "Valider et normaliser" },
    "lists.tools.none_title": {
        pt: "Sem Research Workspace onde trabalhar",
        en: "No Research Workspace to work in",
        fr: "Aucun Research Workspace où travailler"
    },
    "lists.tools.none_body": {
        pt: "Rever bibliografia acontece dentro de um ambiente de investigação onde possa acrescentar referências.",
        en: "Reviewing bibliography happens inside a research environment where you can add references.",
        fr: "Réviser la bibliographie se fait au sein d’un environnement de recherche où vous pouvez ajouter des références."
    },
    // Resultado da revisão BibTeX.
    "lists.review.section": { pt: "RESULTADO", en: "RESULT", fr: "RÉSULTAT" },
    "lists.review.readable": { pt: "Legível", en: "Readable", fr: "Lisible" },
    "lists.review.problems": { pt: "Com problemas", en: "With problems", fr: "Avec des problèmes" },
    "lists.review.all_readable": {
        pt: "{count} referência(s) lidas, todas legíveis.",
        en: "{count} reference(s) read, all readable.",
        fr: "{count} référence(s) lues, toutes lisibles."
    },
    "lists.review.some_unread": {
        pt: "{read} referência(s) lidas · {unread} por ler.",
        en: "{read} reference(s) read · {unread} to read.",
        fr: "{read} référence(s) lues · {unread} à lire."
    },
    "lists.review.unreadable_head": { pt: "Não foi possível ler:", en: "Could not read:", fr: "Impossible de lire :" },
    "lists.review.read_head": { pt: "Referências lidas:", en: "References read:", fr: "Références lues :" },
    "lists.review.normalised": { pt: "BibTeX normalizado", en: "Normalised BibTeX", fr: "BibTeX normalisé" },
};

/// O ciclo de vida científico: hipóteses, metodologias e versões, estudos,
/// execuções, resultados, e a proveniência que os liga. O `pt` é o vocabulário
/// europeu exacto do ecrã; `en` britânico/internacional e `fr` europeu dizem o
/// mesmo, com paridade de significado (briefing §84).
const SCIENCE: &[Entry] = catalogo! {
    // Cabeçalhos e acções da cadeia.
    "science.title": { pt: "Ciência", en: "Science", fr: "Science" },
    "science.new_hypothesis": { pt: "Nova hipótese", en: "New hypothesis", fr: "Nouvelle hypothèse" },
    "science.new_methodology": { pt: "Nova metodologia", en: "New methodology", fr: "Nouvelle méthodologie" },
    "science.new_study": { pt: "Novo estudo", en: "New study", fr: "Nouvelle étude" },
    "science.new_version": { pt: "Nova versão", en: "New version", fr: "Nouvelle version" },
    "science.back_to_workspace": { pt: "Voltar ao ambiente", en: "Back to workspace", fr: "Retour à l’espace" },
    "science.back_to_science": { pt: "Voltar à ciência", en: "Back to science", fr: "Retour à la science" },
    "science.validate_result": { pt: "Validar resultado", en: "Validate result", fr: "Valider le résultat" },
    "science.record_execution": { pt: "Registar execução", en: "Record execution", fr: "Enregistrer l’exécution" },
    "science.record_result": { pt: "Registar resultado", en: "Record result", fr: "Enregistrer le résultat" },
    "science.action.record": { pt: "Registar", en: "Record", fr: "Enregistrer" },
    "science.option.none": { pt: "Nenhuma", en: "None", fr: "Aucune" },
    // Estado vazio da cadeia.
    "science.empty.title": {
        pt: "Ainda não há trabalho científico registado",
        en: "No scientific work has been recorded yet",
        fr: "Aucun travail scientifique n’a encore été enregistré"
    },
    "science.empty.body_creator": {
        pt: "A cadeia começa por uma hipótese: o que se quer testar, e porquê. Depois vêm a metodologia, o estudo, a execução e o resultado — e cada um deles guarda de onde veio.",
        en: "The chain begins with a hypothesis: what you want to test, and why. Then come the methodology, the study, the execution and the result — and each of them keeps a record of where it came from.",
        fr: "La chaîne commence par une hypothèse : ce que l’on veut tester, et pourquoi. Viennent ensuite la méthodologie, l’étude, l’exécution et le résultat — et chacun d’eux garde la trace de son origine."
    },
    "science.empty.body_viewer": {
        pt: "Quando alguém enunciar uma hipótese neste ambiente, a cadeia aparece aqui.",
        en: "When someone states a hypothesis in this workspace, the chain will appear here.",
        fr: "Lorsqu’une personne énoncera une hypothèse dans cet espace, la chaîne apparaîtra ici."
    },
    "science.empty.first_hypothesis": {
        pt: "Enunciar a primeira hipótese",
        en: "State the first hypothesis",
        fr: "Énoncer la première hypothèse"
    },
    // As etapas da cadeia.
    "science.stage.hypotheses": { pt: "Hipóteses", en: "Hypotheses", fr: "Hypothèses" },
    "science.stage.methodologies": { pt: "Metodologias", en: "Methodologies", fr: "Méthodologies" },
    "science.stage.studies": { pt: "Estudos", en: "Studies", fr: "Études" },
    "science.stage.results": { pt: "Resultados", en: "Results", fr: "Résultats" },
    "science.stage.empty": { pt: "Ainda nada.", en: "Nothing yet.", fr: "Rien pour l’instant." },
    // Detalhe de um resultado e a sua proveniência.
    "science.result.summary_head": { pt: "O que este resultado diz", en: "What this result says", fr: "Ce que dit ce résultat" },
    "science.result.validations_head": { pt: "Validações e reproduções", en: "Validations and reproductions", fr: "Validations et reproductions" },
    "science.result.no_validations": {
        pt: "Ninguém validou nem reproduziu este resultado.",
        en: "No one has validated or reproduced this result.",
        fr: "Personne n’a validé ni reproduit ce résultat."
    },
    "science.result.provenance_head": { pt: "Proveniência", en: "Provenance", fr: "Provenance" },
    "science.lineage.upstream": { pt: "Montante", en: "Upstream", fr: "En amont" },
    "science.lineage.downstream": { pt: "Jusante", en: "Downstream", fr: "En aval" },
    "science.lineage.aria": { pt: "Sentido da linhagem", en: "Lineage direction", fr: "Sens de la lignée" },
    "science.lineage.empty_upstream": {
        pt: "Nada aponta para a origem deste resultado.",
        en: "Nothing points to the origin of this result.",
        fr: "Rien ne pointe vers l’origine de ce résultat."
    },
    "science.lineage.empty_downstream": {
        pt: "Nada depende deste resultado.",
        en: "Nothing depends on this result.",
        fr: "Rien ne dépend de ce résultat."
    },
    "science.provenance.observed": { pt: "Observada", en: "Observed", fr: "Observée" },
    "science.provenance.declared": { pt: "Declarada", en: "Declared", fr: "Déclarée" },
    "science.lineage.truncated": {
        pt: "A travessia atingiu o limite de profundidade. Abre um dos recursos acima para continuar a partir dele.",
        en: "The traversal reached the depth limit. Open one of the resources above to continue from it.",
        fr: "Le parcours a atteint la limite de profondeur. Ouvrez l’une des ressources ci-dessus pour continuer à partir d’elle."
    },
    // Formulário de validação.
    "science.validate.no_execution": {
        pt: "Este resultado não tem nenhuma execução registada que sirva de prova. Regista a execução que o reproduziu antes de o dar por reproduzido.",
        en: "This result has no recorded execution to serve as proof. Record the execution that reproduced it before marking it as reproduced.",
        fr: "Ce résultat n’a aucune exécution enregistrée pouvant servir de preuve. Enregistrez l’exécution qui l’a reproduit avant de le déclarer reproduit."
    },
    "science.validate.subtitle": { pt: "Sobre «{title}».", en: "About “{title}”.", fr: "À propos de « {title} »." },
    "science.validate.callout_title": {
        pt: "Isto fica em seu nome",
        en: "This is recorded in your name",
        fr: "Ceci est enregistré en votre nom"
    },
    "science.validate.callout_body": {
        pt: "Uma validação é uma afirmação institucional sobre o que a Ocinye sabe. O registo guarda quem a fez, e é por isso que nenhum agente a pode fazer por si.",
        en: "A validation is an institutional assertion about what Ocinye knows. The record keeps who made it, and that is why no agent can make it for you.",
        fr: "Une validation est une affirmation institutionnelle sur ce que sait Ocinye. Le registre conserve qui l’a faite, et c’est pourquoi aucun agent ne peut la faire à votre place."
    },
    "science.claim_head": { pt: "A afirmação", en: "The assertion", fr: "L’affirmation" },
    "science.validate.kind_label": { pt: "O que está a registar", en: "What you are recording", fr: "Ce que vous enregistrez" },
    "science.validate.kind.validation": { pt: "Validação", en: "Validation", fr: "Validation" },
    "science.validate.kind.reproduction": { pt: "Reprodução", en: "Reproduction", fr: "Reproduction" },
    "science.validate.outcome_label": { pt: "Desfecho", en: "Outcome", fr: "Issue" },
    "science.validate.outcome.confirmed": { pt: "Confirmou", en: "Confirmed", fr: "A confirmé" },
    "science.validate.outcome.contradicted": { pt: "Contradisse", en: "Contradicted", fr: "A contredit" },
    "science.validate.outcome.inconclusive": { pt: "Foi inconclusiva", en: "Was inconclusive", fr: "A été non concluante" },
    "science.validate.execution_label": {
        pt: "A execução que serviu de prova",
        en: "The execution that served as proof",
        fr: "L’exécution qui a servi de preuve"
    },
    "science.validate.note_label": { pt: "O que observou", en: "What you observed", fr: "Ce que vous avez observé" },
    "science.validate.note_placeholder": {
        pt: "O que viu, e em que condições",
        en: "What you saw, and under what conditions",
        fr: "Ce que vous avez vu, et dans quelles conditions"
    },
    // As classificações num selector (femininas, a concordar com «classificação»).
    "science.class.internal": { pt: "Interna", en: "Internal", fr: "Interne" },
    "science.class.public": { pt: "Pública", en: "Public", fr: "Publique" },
    "science.class.confidential": { pt: "Confidencial", en: "Confidential", fr: "Confidentielle" },
    "science.class.restricted": { pt: "Restrita", en: "Restricted", fr: "Restreinte" },
    // Campos partilhados por vários formulários.
    "science.field.name_label": { pt: "Como se chama", en: "What it is called", fr: "Comment cela s’appelle" },
    // Formulário de hipótese.
    "science.hypothesis.subtitle": {
        pt: "Uma afirmação que se pode testar. Enunciá-la é o princípio da cadeia — e uma hipótese que não se sustenta é um desfecho científico, não um erro.",
        en: "An assertion that can be tested. Stating it is the start of the chain — and a hypothesis that does not hold up is a scientific outcome, not an error.",
        fr: "Une affirmation que l’on peut tester. L’énoncer est le début de la chaîne — et une hypothèse qui ne tient pas est un résultat scientifique, non une erreur."
    },
    "science.hypothesis.statement_label": { pt: "O que se afirma", en: "What is asserted", fr: "Ce qui est affirmé" },
    "science.hypothesis.statement_placeholder": {
        pt: "Ex.: a dopagem reduz a resistência de contacto",
        en: "E.g. doping reduces contact resistance",
        fr: "Ex. : le dopage réduit la résistance de contact"
    },
    "science.hypothesis.rationale_label": { pt: "Porque vale a pena testar", en: "Why it is worth testing", fr: "Pourquoi cela vaut la peine d’être testé" },
    "science.hypothesis.rationale_placeholder": {
        pt: "O que se sabe hoje, e o que falta saber",
        en: "What is known today, and what remains to be known",
        fr: "Ce que l’on sait aujourd’hui, et ce qu’il reste à savoir"
    },
    "science.hypothesis.submit": { pt: "Enunciar", en: "State", fr: "Énoncer" },
    // Formulário de metodologia.
    "science.methodology.subtitle": {
        pt: "A metodologia é a identidade durável do método: o nome pelo qual a instituição o conhece daqui a cinco anos. O que ela diz hoje vive numa versão, e publica-se a seguir.",
        en: "The methodology is the durable identity of the method: the name by which the institution will know it five years from now. What it says today lives in a version, published next.",
        fr: "La méthodologie est l’identité durable de la méthode : le nom sous lequel l’institution la connaîtra dans cinq ans. Ce qu’elle dit aujourd’hui vit dans une version, publiée ensuite."
    },
    "science.methodology.method_head": { pt: "O método", en: "The method", fr: "La méthode" },
    "science.methodology.title_placeholder": {
        pt: "Ex.: medição a quatro pontas",
        en: "E.g. four-point measurement",
        fr: "Ex. : mesure à quatre pointes"
    },
    "science.methodology.purpose_label": { pt: "Para que serve", en: "What it is for", fr: "À quoi elle sert" },
    "science.methodology.purpose_placeholder": {
        pt: "Que pergunta este método responde",
        en: "What question this method answers",
        fr: "À quelle question cette méthode répond"
    },
    // Detalhe de metodologia e versões.
    "science.versions_head": { pt: "Versões", en: "Versions", fr: "Versions" },
    "science.methodology.no_versions": {
        pt: "Ainda nenhuma versão. Um estudo só pode seguir uma versão publicada, porque é a versão que a proveniência cita.",
        en: "No version yet. A study can only follow a published version, because it is the version that provenance cites.",
        fr: "Aucune version pour l’instant. Une étude ne peut suivre qu’une version publiée, car c’est la version que cite la provenance."
    },
    // Formulário de versão.
    "science.version.subtitle": { pt: "De «{title}».", en: "Of “{title}”.", fr: "De « {title} »." },
    "science.version.in_force": { pt: "Em vigor: {label}", en: "In force: {label}", fr: "En vigueur : {label}" },
    "science.version.in_force_body": {
        pt: "Publicar substitui-a. A anterior fica no histórico e continua a valer para tudo o que já a citou — um resultado produzido com ela continua a dizer que foi com ela.",
        en: "Publishing replaces it. The previous one stays in the history and remains valid for everything that already cited it — a result produced with it still says it was produced with it.",
        fr: "Publier la remplace. La précédente reste dans l’historique et continue de valoir pour tout ce qui l’a déjà citée — un résultat produit avec elle continue de dire qu’il l’a été avec elle."
    },
    "science.version.version_head": { pt: "A versão", en: "The version", fr: "La version" },
    "science.version.label_placeholder": { pt: "Ex.: v2, 2026-rev-b", en: "E.g. v2, 2026-rev-b", fr: "Ex. : v2, 2026-rev-b" },
    "science.version.summary_label": { pt: "O que esta versão diz", en: "What this version says", fr: "Ce que dit cette version" },
    "science.version.summary_placeholder": {
        pt: "O que muda em relação à anterior, ou o que o método faz",
        en: "What changes from the previous one, or what the method does",
        fr: "Ce qui change par rapport à la précédente, ou ce que fait la méthode"
    },
    "science.version.submit": { pt: "Publicar", en: "Publish", fr: "Publier" },
    // Formulário de estudo.
    "science.study.subtitle": {
        pt: "Um estudo põe uma hipótese à prova por um método. O que ele seguiu fica registado com a versão exacta — porque o método melhora, e o que se fez não muda por isso.",
        en: "A study puts a hypothesis to the test through a method. What it followed is recorded with the exact version — because the method improves, and what was done does not change for that.",
        fr: "Une étude met une hypothèse à l’épreuve par une méthode. Ce qu’elle a suivi est enregistré avec la version exacte — car la méthode s’améliore, et ce qui a été fait n’en change pas pour autant."
    },
    "science.study.study_head": { pt: "O estudo", en: "The study", fr: "L’étude" },
    "science.study.title_placeholder": {
        pt: "Ex.: ensaio de carga em contactos dopados",
        en: "E.g. load test on doped contacts",
        fr: "Ex. : essai de charge sur contacts dopés"
    },
    "science.study.kind_label": { pt: "Género", en: "Kind", fr: "Type" },
    "science.study.kind.physical": { pt: "Experiência física", en: "Physical experiment", fr: "Expérience physique" },
    "science.study.kind.simulation": { pt: "Simulação", en: "Simulation", fr: "Simulation" },
    "science.study.kind.analysis": { pt: "Análise", en: "Analysis", fr: "Analyse" },
    "science.study.objective_label": { pt: "O que se propõe descobrir", en: "What it sets out to discover", fr: "Ce qu’elle se propose de découvrir" },
    "science.study.objective_placeholder": {
        pt: "O que este estudo tem de mostrar para responder à pergunta",
        en: "What this study must show to answer the question",
        fr: "Ce que cette étude doit montrer pour répondre à la question"
    },
    "science.study.chain_head": { pt: "A cadeia", en: "The chain", fr: "La chaîne" },
    "science.study.hypothesis_label": { pt: "Hipótese que testa", en: "Hypothesis it tests", fr: "Hypothèse qu’elle teste" },
    "science.study.methodology_label": {
        pt: "Versão de metodologia que segue",
        en: "Methodology version it follows",
        fr: "Version de méthodologie qu’elle suit"
    },
    "science.study.no_methodology_hint": {
        pt: "Nenhuma metodologia deste ambiente tem versão publicada. Um estudo pode ficar sem ela, e a linhagem dirá que método seguiu quando alguém publicar uma.",
        en: "No methodology in this workspace has a published version. A study can go without one, and the lineage will say which method it followed when someone publishes one.",
        fr: "Aucune méthodologie de cet espace n’a de version publiée. Une étude peut s’en passer, et la lignée dira quelle méthode elle a suivie lorsqu’une personne en publiera une."
    },
    "science.study.submit": { pt: "Desenhar", en: "Design", fr: "Concevoir" },
    "science.no_published_methodology": {
        pt: "Nenhuma metodologia publicada neste ambiente",
        en: "No methodology published in this workspace",
        fr: "Aucune méthodologie publiée dans cet espace"
    },
    // Detalhe de estudo e execuções.
    "science.executions_head": { pt: "Execuções", en: "Executions", fr: "Exécutions" },
    "science.study.no_executions": {
        pt: "Ainda nenhuma corrida. É a execução, e não o estudo, que produz um resultado — e são duas execuções, e não dois estudos, que se comparam quando se reproduz.",
        en: "No run yet. It is the execution, not the study, that produces a result — and it is two executions, not two studies, that are compared when reproducing.",
        fr: "Aucune exécution pour l’instant. C’est l’exécution, et non l’étude, qui produit un résultat — et ce sont deux exécutions, et non deux études, que l’on compare lorsqu’on reproduit."
    },
    "science.execution.numbered": { pt: "Execução {sequence}", en: "Execution {sequence}", fr: "Exécution {sequence}" },
    "science.execution.lower_numbered": { pt: "execução {sequence}", en: "execution {sequence}", fr: "exécution {sequence}" },
    // Formulário de execução.
    "science.execution.subtitle": {
        pt: "Uma corrida de «{title}». É aqui que a reprodutibilidade mora: o mesmo estudo corre duas vezes e dá duas execuções, e são elas que se comparam.",
        en: "A run of “{title}”. This is where reproducibility lives: the same study runs twice and yields two executions, and it is they that are compared.",
        fr: "Une exécution de « {title} ». C’est ici que réside la reproductibilité : la même étude s’exécute deux fois et donne deux exécutions, et ce sont elles que l’on compare."
    },
    "science.run_head": { pt: "A corrida", en: "The run", fr: "L’exécution" },
    "science.execution.status_label": { pt: "Estado", en: "State", fr: "État" },
    "science.execution.status.succeeded": { pt: "Correu bem", en: "Succeeded", fr: "Réussie" },
    "science.execution.status.running": { pt: "A correr", en: "Running", fr: "En cours" },
    "science.execution.status.failed": { pt: "Falhou", en: "Failed", fr: "Échouée" },
    "science.execution.status.aborted": { pt: "Interrompida", en: "Aborted", fr: "Interrompue" },
    "science.execution.status.recorded": { pt: "Só registada", en: "Recorded only", fr: "Seulement enregistrée" },
    "science.execution.environment": { pt: "Onde correu", en: "Where it ran", fr: "Où elle s’est exécutée" },
    "science.execution.environment_placeholder": {
        pt: "A máquina, o laboratório, o serviço",
        en: "The machine, the laboratory, the service",
        fr: "La machine, le laboratoire, le service"
    },
    "science.execution.software_label": { pt: "Que software", en: "What software", fr: "Quel logiciel" },
    "science.execution.software_placeholder": { pt: "Ex.: OpenFOAM", en: "E.g. OpenFOAM", fr: "Ex. : OpenFOAM" },
    "science.execution.software_version_label": { pt: "Que versão do software", en: "What software version", fr: "Quelle version du logiciel" },
    "science.execution.software_version_placeholder": { pt: "Ex.: 11", en: "E.g. 11", fr: "Ex. : 11" },
    "science.execution.notes_label": { pt: "O que houve a registar", en: "What there was to record", fr: "Ce qu’il y avait à consigner" },
    "science.execution.notes_placeholder": {
        pt: "Condições, desvios, o que correu mal",
        en: "Conditions, deviations, what went wrong",
        fr: "Conditions, écarts, ce qui a mal tourné"
    },
    "science.execution.used_head": { pt: "O que esta corrida usou", en: "What this run used", fr: "Ce que cette exécution a utilisé" },
    "science.execution.methodology_version_label": { pt: "Versão de metodologia", en: "Methodology version", fr: "Version de méthodologie" },
    "science.execution.dataset_version_label": { pt: "Versão de dataset", en: "Dataset version", fr: "Version de jeu de données" },
    "science.execution.no_dataset_version": {
        pt: "Nenhum dataset com versão neste ambiente",
        en: "No dataset with a version in this workspace",
        fr: "Aucun jeu de données versionné dans cet espace"
    },
    "science.execution.provenance_hint": {
        pt: "O que escolher aqui fica na proveniência como observado por esta operação — não como algo que alguém afirmou depois.",
        en: "What you choose here is kept in the provenance as observed by this operation — not as something someone asserted afterwards.",
        fr: "Ce que vous choisissez ici est conservé dans la provenance comme observé par cette opération — non comme quelque chose que quelqu’un a affirmé après coup."
    },
    // Ficha de uma execução.
    "science.execution.software": { pt: "Software", en: "Software", fr: "Logiciel" },
    "science.execution.version": { pt: "Versão", en: "Version", fr: "Version" },
    "science.execution.commit": { pt: "Commit", en: "Commit", fr: "Commit" },
    "science.execution.notes": { pt: "Notas", en: "Notes", fr: "Notes" },
    "science.execution.produced_head": { pt: "O que produziu", en: "What it produced", fr: "Ce qu’elle a produit" },
    "science.execution.no_results": { pt: "Ainda nenhum resultado.", en: "No result yet.", fr: "Aucun résultat pour l’instant." },
    // Formulário de resultado.
    "science.result.subtitle": {
        pt: "O que esta corrida mostrou — incluindo quando mostrou que a hipótese não se sustenta, que é um resultado como outro qualquer.",
        en: "What this run showed — including when it showed that the hypothesis does not hold up, which is a result like any other.",
        fr: "Ce que cette exécution a montré — y compris lorsqu’elle a montré que l’hypothèse ne tient pas, ce qui est un résultat comme un autre."
    },
    "science.result.origin_callout_title": {
        pt: "A origem fica registada sozinha",
        en: "The origin is recorded on its own",
        fr: "L’origine est enregistrée d’elle-même"
    },
    "science.result.origin_callout_body": {
        pt: "Este resultado nasce da execução {sequence}, e o Ocinye OS escreve essa ligação no mesmo acto. Não há um passo a seguir para indicar de onde veio.",
        en: "This result comes from execution {sequence}, and Ocinye OS writes that link in the same act. There is no next step to indicate where it came from.",
        fr: "Ce résultat naît de l’exécution {sequence}, et Ocinye OS écrit ce lien dans le même acte. Il n’y a pas d’étape suivante pour indiquer d’où il vient."
    },
    "science.result.result_head": { pt: "O resultado", en: "The result", fr: "Le résultat" },
    "science.result.title_placeholder": { pt: "O que se pode dizer numa linha", en: "What can be said in one line", fr: "Ce que l’on peut dire en une ligne" },
    "science.result.summary_label": { pt: "O que diz", en: "What it says", fr: "Ce qu’il dit" },
    "science.result.summary_placeholder": {
        pt: "O que se observou, e em que condições",
        en: "What was observed, and under what conditions",
        fr: "Ce qui a été observé, et dans quelles conditions"
    },
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
    AI,
    ADMIN,
    LISTS,
    SCIENCE,
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
