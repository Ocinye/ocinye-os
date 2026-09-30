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
    // D004 · Rótulos que as vistas das aplicações recebem já traduzidos
    // (`AppNavVm.label`, `MailVm.folder_label`, `AppNyeVm.label_key`): são do
    // Code, porque é o Code que sabe que secções o Core serve.
    "prod.files.mine": { pt: "Os meus ficheiros", en: "My files", fr: "Mes fichiers" },
    "prod.files.recent": { pt: "Recentes", en: "Recent", fr: "Récents" },
    "prod.files.favourites": { pt: "Favoritos", en: "Starred", fr: "Favoris" },
    "prod.files.folder": { pt: "Pasta", en: "Folder", fr: "Dossier" },
    "prod.res.role.lead": { pt: "Responsável", en: "Lead", fr: "Responsable" },
    "prod.res.role.member": { pt: "Membro", en: "Member", fr: "Membre" },
    "prod.res.role.viewer": { pt: "Leitura", en: "Read only", fr: "Lecture" },
    "prod.res.kind.hypothesis": { pt: "Hipótese", en: "Hypothesis", fr: "Hypothèse" },
    "prod.res.kind.methodology": { pt: "Metodologia", en: "Methodology", fr: "Méthodologie" },
    "prod.res.kind.study": { pt: "Estudo", en: "Study", fr: "Étude" },
    "prod.res.kind.result": { pt: "Resultado", en: "Result", fr: "Résultat" },
    "prod.know.type.article": { pt: "Artigo", en: "Article", fr: "Article" },
    "prod.know.type.book": { pt: "Livro", en: "Book", fr: "Livre" },
    "prod.know.type.book_chapter": { pt: "Capítulo de livro", en: "Book chapter", fr: "Chapitre d’ouvrage" },
    "prod.know.type.conference_paper": { pt: "Comunicação em conferência", en: "Conference paper", fr: "Communication de conférence" },
    "prod.know.type.thesis": { pt: "Tese", en: "Thesis", fr: "Thèse" },
    "prod.know.type.report": { pt: "Relatório", en: "Report", fr: "Rapport" },
    "prod.know.type.standard": { pt: "Norma", en: "Standard", fr: "Norme" },
    "prod.know.type.patent": { pt: "Patente", en: "Patent", fr: "Brevet" },
    "prod.know.type.dataset_reference": { pt: "Referência de dados", en: "Dataset reference", fr: "Référence de données" },
    "prod.know.type.software": { pt: "Software", en: "Software", fr: "Logiciel" },
    "prod.know.type.webpage": { pt: "Página web", en: "Web page", fr: "Page web" },
    "prod.know.type.preprint": { pt: "Pré-publicação", en: "Preprint", fr: "Prépublication" },
    "prod.know.type.other": { pt: "Outro", en: "Other", fr: "Autre" },
    "prod.know.kind.note_attachment": { pt: "Anexo de nota", en: "Note attachment", fr: "Pièce jointe de note" },
    "prod.know.kind.protocol": { pt: "Protocolo", en: "Protocol", fr: "Protocole" },
    "prod.know.kind.report": { pt: "Relatório", en: "Report", fr: "Rapport" },
    "prod.know.kind.presentation": { pt: "Apresentação", en: "Presentation", fr: "Présentation" },
    "prod.know.kind.figure": { pt: "Figura", en: "Figure", fr: "Figure" },
    "prod.know.kind.contract": { pt: "Contrato", en: "Contract", fr: "Contrat" },
    "prod.know.kind.source_full_text": { pt: "Texto integral de uma fonte", en: "Full text of a source", fr: "Texte intégral d’une source" },
    "prod.know.kind.other": { pt: "Outro", en: "Other", fr: "Autre" },
    "prod.data.totals.one": { pt: "{n} ficheiro · {size}", en: "{n} file · {size}", fr: "{n} fichier · {size}" },
    "prod.data.totals.other": { pt: "{n} ficheiros · {size}", en: "{n} files · {size}", fr: "{n} fichiers · {size}" },
    "prod.org.scope.institution": { pt: "Instituição", en: "Institution", fr: "Institution" },
    "prod.org.scope.unit": { pt: "Unidade", en: "Unit", fr: "Unité" },
    "prod.org.scope.research_workspace": { pt: "Ambiente de investigação", en: "Research environment", fr: "Environnement de recherche" },
    "prod.org.scope.resource": { pt: "Recurso", en: "Resource", fr: "Ressource" },
    "prod.org.locale.pt": { pt: "Português", en: "Portuguese", fr: "Portugais" },
    "prod.org.locale.en": { pt: "Inglês", en: "English", fr: "Anglais" },
    "prod.org.locale.fr": { pt: "Francês", en: "French", fr: "Français" },
    "prod.org.unit.none": { pt: "Sem unidade", en: "No unit", fr: "Aucune unité" },
    "prod.work.nobody": { pt: "Ninguém", en: "Nobody", fr: "Personne" },
    "prod.files.up.type": { pt: "Este tipo de ficheiro não é aceite.", en: "This file type is not accepted.", fr: "Ce type de fichier n’est pas accepté." },
    "prod.files.up.full": { pt: "Sem espaço para este ficheiro", en: "Not enough space for this file", fr: "Pas assez d’espace pour ce fichier" },
    "prod.files.err.download_many": { pt: "Descarregar vários ficheiros de uma vez ainda não está disponível. Descarregue um de cada vez.", en: "Downloading several files at once is not available yet. Download them one at a time.", fr: "Le téléchargement de plusieurs fichiers à la fois n’est pas encore disponible. Téléchargez-les un par un." },
    "prod.files.err.purge_confirmation": { pt: "Eliminar para sempre ainda não está disponível: falta a confirmação. Nada foi eliminado.", en: "Deleting forever is not available yet: the confirmation is missing. Nothing was deleted.", fr: "La suppression définitive n’est pas encore disponible : la confirmation manque. Rien n’a été supprimé." },
    "prod.files.err.move_target": { pt: "Para mover, arraste os ficheiros para uma pasta.", en: "To move files, drag them onto a folder.", fr: "Pour déplacer des fichiers, faites-les glisser sur un dossier." },
    "prod.files.trash": { pt: "Lixo", en: "Trash", fr: "Corbeille" },
    "prod.notes.mine": { pt: "As minhas notas", en: "My notes", fr: "Mes notes" },
    "prod.notes.shared": { pt: "Partilhadas comigo", en: "Shared with me", fr: "Partagées avec moi" },
    "prod.notes.trash": { pt: "Lixo", en: "Trash", fr: "Corbeille" },
    "prod.mail.inbox": { pt: "Entrada", en: "Inbox", fr: "Réception" },
    "prod.mail.starred": { pt: "Favoritos", en: "Starred", fr: "Favoris" },
    "prod.mail.drafts": { pt: "Rascunhos", en: "Drafts", fr: "Brouillons" },
    "prod.mail.sent": { pt: "Enviados", en: "Sent", fr: "Envoyés" },
    "prod.mail.archive": { pt: "Arquivo", en: "Archive", fr: "Archives" },
    "prod.mail.spam": { pt: "Spam", en: "Spam", fr: "Indésirables" },
    "prod.mail.trash": { pt: "Lixo", en: "Trash", fr: "Corbeille" },
    "prod.mail.err.move": { pt: "Arquivar e mover para o lixo ainda não estão disponíveis no Correio.", en: "Archiving and moving to trash are not available in Mail yet.", fr: "L’archivage et la mise à la corbeille ne sont pas encore disponibles dans le Courrier." },
    "prod.nye.about.project": { pt: "Sobre o projecto «{name}»: ", en: "About the project “{name}”: ", fr: "À propos du projet « {name} » : " },
    "prod.nye.about.task": { pt: "Sobre a tarefa «{name}»: ", en: "About the task “{name}”: ", fr: "À propos de la tâche « {name} » : " },
    "prod.nye.about.idea": { pt: "Sobre a ideia «{name}»: ", en: "About the idea “{name}”: ", fr: "À propos de l’idée « {name} » : " },
    "prod.nye.about.dataset": { pt: "Sobre o dataset «{name}»: ", en: "About the dataset “{name}”: ", fr: "À propos du jeu de données « {name} » : " },
    "prod.nye.about.source": { pt: "Sobre a referência «{name}»: ", en: "About the reference “{name}”: ", fr: "À propos de la référence « {name} » : " },
    "prod.nye.about.document": { pt: "Sobre o documento «{name}»: ", en: "About the document “{name}”: ", fr: "À propos du document « {name} » : " },
    "prod.nye.about.note": { pt: "Sobre a nota «{name}»: ", en: "About the note “{name}”: ", fr: "À propos de la note « {name} » : " },
    "prod.nye.about.event": { pt: "Sobre o evento «{name}»: ", en: "About the event “{name}”: ", fr: "À propos de l’événement « {name} » : " },
    "prod.nye.about.file": { pt: "Sobre o ficheiro «{name}»: ", en: "About the file “{name}”: ", fr: "À propos du fichier « {name} » : " },
    "prod.nye.about.message": { pt: "Sobre a mensagem «{name}»: ", en: "About the message “{name}”: ", fr: "À propos du message « {name} » : " },
    "prod.nye.file": { pt: "Perguntar à Nye sobre este ficheiro", en: "Ask Nye about this file", fr: "Demander à Nye à propos de ce fichier" },
    "prod.nye.note": { pt: "Perguntar à Nye sobre esta nota", en: "Ask Nye about this note", fr: "Demander à Nye à propos de cette note" },
    "prod.nye.event": { pt: "Perguntar à Nye sobre este evento", en: "Ask Nye about this event", fr: "Demander à Nye à propos de cet événement" },
    "prod.cal.scope.personal": { pt: "Pessoal", en: "Personal", fr: "Personnel" },
    "prod.cal.scope.unit": { pt: "Unidade", en: "Unit", fr: "Unité" },
    "prod.cal.scope.research_workspace": { pt: "Espaço de investigação", en: "Research workspace", fr: "Espace de recherche" },
    "prod.cal.scope.institution": { pt: "Instituição", en: "Institution", fr: "Institution" },
    "prod.cal.err.dates": { pt: "Indique o início e o fim.", en: "Enter the start and the end.", fr: "Indiquez le début et la fin." },
    "prod.cal.err.order": { pt: "O fim tem de ser depois do início.", en: "The end must be after the start.", fr: "La fin doit être après le début." },
    "prod.nye.message": { pt: "Perguntar à Nye sobre esta mensagem", en: "Ask Nye about this message", fr: "Demander à Nye à propos de ce message" },
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
    // D003: o nome visível é Nye; o identificador (`prompt`), a rota
    // (`/ai/prompt`) e o manifesto ficam estáveis (ADR-0619).
    "nav.prompt": { pt: "Nye", en: "Nye", fr: "Nye" },
    "nav.search": { pt: "Pesquisar", en: "Search", fr: "Rechercher" },
    "nav.ask": {
        pt: "Pesquisar, perguntar ou executar",
        en: "Search, ask or run",
        fr: "Rechercher, demander ou exécuter"
    },
    "nav.section.intelligence": { pt: "Inteligência", en: "Intelligence", fr: "Intelligence" },
    "nav.section.institutional": { pt: "Institucional", en: "Institutional", fr: "Institutionnel" },
    "nav.section.pinned": { pt: "Fixadas", en: "Pinned", fr: "Épinglées" },
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

/// O Gestor de Aplicações — o lançador, os filtros de categoria e a descrição
/// curta de cada aplicação. A identidade é semântica (`files`); só o texto muda
/// de língua (i18n §7). Cada descrição é uma frase, não um parágrafo.
const APPS: &[Entry] = catalogo! {
    "apps.title": { pt: "Aplicações", en: "Applications", fr: "Applications" },
    "apps.open": { pt: "Abrir Aplicações", en: "Open Applications", fr: "Ouvrir les applications" },
    "apps.search_placeholder": {
        pt: "Pesquisar aplicações do Ocinye…",
        en: "Search Ocinye applications…",
        fr: "Rechercher des applications Ocinye…"
    },
    "apps.category.all": { pt: "Todos", en: "All", fr: "Tous" },
    "apps.category.productivity": { pt: "Produtividade", en: "Productivity", fr: "Productivité" },
    "apps.category.research": { pt: "Investigação", en: "Research", fr: "Recherche" },
    "apps.category.knowledge": { pt: "Conhecimento", en: "Knowledge", fr: "Connaissance" },
    "apps.category.communication": { pt: "Comunicação", en: "Communication", fr: "Communication" },
    "apps.category.administration": { pt: "Administração", en: "Administration", fr: "Administration" },
    "apps.empty": { pt: "Nenhuma aplicação encontrada", en: "No applications found", fr: "Aucune application trouvée" },
    "apps.empty.hint": {
        pt: "Experimente outro termo de pesquisa.",
        en: "Try another search term.",
        fr: "Essayez un autre terme de recherche."
    },
    "apps.close": { pt: "Fechar o lançador", en: "Close the launcher", fr: "Fermer le lanceur" },
    "apps.esc_hint": { pt: "ESC", en: "ESC", fr: "ÉCHAP" },
    "apps.pin": {
        pt: "Fixar na barra lateral",
        en: "Pin to sidebar",
        fr: "Épingler à la barre latérale"
    },
    "apps.unpin": {
        pt: "Remover da barra lateral",
        en: "Remove from sidebar",
        fr: "Retirer de la barre latérale"
    },
    // As descrições, uma por aplicação. Uma frase que diz o que a aplicação faz.
    "apps.desc.home": {
        pt: "O painel de início do seu trabalho.",
        en: "The starting panel for your work.",
        fr: "Le tableau de bord de votre travail."
    },
    "apps.desc.work": {
        pt: "As tarefas e o trabalho que lhe estão atribuídos.",
        en: "The tasks and work assigned to you.",
        fr: "Les tâches et le travail qui vous sont attribués."
    },
    "apps.desc.notes": {
        pt: "Capture e organize as suas notas.",
        en: "Capture and organise your notes.",
        fr: "Capturez et organisez vos notes."
    },
    "apps.desc.calendar": {
        pt: "Planeie reuniões e eventos.",
        en: "Plan meetings and events.",
        fr: "Planifiez réunions et événements."
    },
    "apps.desc.mail": {
        pt: "Gestão do correio institucional.",
        en: "Institutional mail management.",
        fr: "Gestion du courrier institutionnel."
    },
    "apps.desc.messages": {
        pt: "Comunicação com a sua equipa.",
        en: "Communication with your team.",
        fr: "Communication avec votre équipe."
    },
    "apps.desc.files": {
        pt: "Aceda e gira os seus ficheiros.",
        en: "Access and manage your files.",
        fr: "Accédez à vos fichiers et gérez-les."
    },
    "apps.desc.knowledge": {
        pt: "Aceda à base de conhecimento.",
        en: "Access the knowledge base.",
        fr: "Accédez à la base de connaissances."
    },
    "apps.desc.bibliography": {
        pt: "Gira referências bibliográficas.",
        en: "Manage bibliographic references.",
        fr: "Gérez les références bibliographiques."
    },
    "apps.desc.units": {
        pt: "Explore as suas unidades de investigação.",
        en: "Explore your research units.",
        fr: "Explorez vos unités de recherche."
    },
    "apps.desc.ideas": {
        pt: "Registe e desenvolva novas ideias.",
        en: "Record and develop new ideas.",
        fr: "Enregistrez et développez de nouvelles idées."
    },
    "apps.desc.projects": {
        pt: "Gira projectos de investigação.",
        en: "Manage research projects.",
        fr: "Gérez les projets de recherche."
    },
    "apps.desc.datasets": {
        pt: "Explore e gira datasets.",
        en: "Explore and manage datasets.",
        fr: "Explorez et gérez les jeux de données."
    },
    "apps.desc.prompt": {
        pt: "Pesquise, pergunte e aja no Ocinye OS.",
        en: "Search, ask and act across Ocinye OS.",
        fr: "Recherchez, demandez et agissez dans Ocinye OS."
    },
    "apps.desc.ai": {
        pt: "O estado da inteligência do Ocinye OS.",
        en: "The state of Ocinye OS intelligence.",
        fr: "L'état de l'intelligence d'Ocinye OS."
    },
    "apps.desc.agents": {
        pt: "Defina e gira agentes de IA.",
        en: "Define and manage AI agents.",
        fr: "Définissez et gérez des agents IA."
    },
    "apps.desc.compute": {
        pt: "Os nós de computação da plataforma.",
        en: "The platform's compute nodes.",
        fr: "Les nœuds de calcul de la plateforme."
    },
    "apps.desc.resources": {
        pt: "Consulte os recursos que lhe estão atribuídos.",
        en: "Review the resources assigned to you.",
        fr: "Consultez les ressources qui vous sont attribuées."
    },
    "apps.desc.activity": {
        pt: "O registo de actividade da instituição.",
        en: "The institution's activity feed.",
        fr: "Le journal d'activité de l'institution."
    },
    "apps.desc.administration": {
        pt: "A consola de administração da plataforma.",
        en: "The platform administration console.",
        fr: "La console d'administration de la plateforme."
    },
    "apps.desc.audit": {
        pt: "O registo de auditoria da plataforma.",
        en: "The platform audit log.",
        fr: "Le journal d'audit de la plateforme."
    },
    "apps.desc.settings": {
        pt: "Personalize o seu ambiente.",
        en: "Personalise your environment.",
        fr: "Personnalisez votre environnement."
    },
    "apps.desc.help": {
        pt: "Consulte ajuda e suporte do Ocinye OS.",
        en: "Get Ocinye OS help and support.",
        fr: "Obtenez aide et assistance pour Ocinye OS."
    },
};

// D14 · Ocinye Terminal (ocsh). Juntar a catalog.rs e acrescentar DS_TERMINAL a GROUPS.
// As descrições de comandos vêm localizadas do registo (G-10); até lá usar ocsh.cmd.*.
const DS_TERMINAL: &[Entry] = catalogo! {
    "terminal.app": { pt: "Terminal", en: "Terminal", fr: "Terminal" },
    "terminal.app.desc": { pt: "Ocinye Terminal — a shell ocsh para operar o sistema.", en: "Ocinye Terminal — the ocsh shell to operate the system.", fr: "Ocinye Terminal — le shell ocsh pour opérer le système." },
    "terminal.category.system": { pt: "Sistema", en: "System", fr: "Système" },
    "terminal.shortcut": { pt: "Abrir Terminal: Ctrl+Shift+`", en: "Open Terminal: Ctrl+Shift+`", fr: "Ouvrir le Terminal : Ctrl+Maj+`" },
    "terminal.welcome.title": { pt: "Ocinye Terminal", en: "Ocinye Terminal", fr: "Ocinye Terminal" },
    "terminal.welcome.powered": { pt: "Powered by ocsh {version} · instância {instance} · contexto {context}", en: "Powered by ocsh {version} · {instance} instance · context {context}", fr: "Propulsé par ocsh {version} · instance {instance} · contexte {context}" },
    "terminal.welcome.start": { pt: "Escreva help para começar.", en: "Type help to get started.", fr: "Tapez help pour commencer." },
    "terminal.context.personal": { pt: "pessoal", en: "personal", fr: "personnel" },
    "terminal.tab.default": { pt: "Terminal {n}", en: "Terminal {n}", fr: "Terminal {n}" },
    "terminal.tab.new": { pt: "Novo separador", en: "New tab", fr: "Nouvel onglet" },
    "terminal.tab.close": { pt: "Fechar separador", en: "Close tab", fr: "Fermer l’onglet" },
    "terminal.tab.rename": { pt: "Renomear", en: "Rename", fr: "Renommer" },
    "terminal.tab.duplicate": { pt: "Duplicar sessão", en: "Duplicate session", fr: "Dupliquer la session" },
    "terminal.tab.split_right": { pt: "Dividir ao lado", en: "Split right", fr: "Diviser à droite" },
    "terminal.tab.menu_note": { pt: "Duplicar mantém contexto e pasta; a sessão de administração nunca é duplicada.", en: "Duplicate keeps context and folder; an administration session is never duplicated.", fr: "Dupliquer garde le contexte et le dossier ; une session d’administration n’est jamais dupliquée." },
    "terminal.split.v": { pt: "Dividir na vertical", en: "Split vertically", fr: "Diviser verticalement" },
    "terminal.split.h": { pt: "Dividir na horizontal", en: "Split horizontally", fr: "Diviser horizontalement" },
    "terminal.pane.close": { pt: "Fechar painel", en: "Close pane", fr: "Fermer le panneau" },
    "terminal.output.aria": { pt: "Saída do terminal", en: "Terminal output", fr: "Sortie du terminal" },
    "terminal.line.aria": { pt: "Linha de comando", en: "Command line", fr: "Ligne de commande" },
    "terminal.running": { pt: "a executar", en: "running", fr: "en cours" },
    "terminal.running.meta": { pt: "a executar · ⌃C cancela", en: "running · ⌃C cancels", fr: "en cours · ⌃C annule" },
    "terminal.running.placeholder": { pt: "⌃C para cancelar · pode abrir outro separador", en: "⌃C to cancel · you can open another tab", fr: "⌃C pour annuler · vous pouvez ouvrir un autre onglet" },
    "terminal.mode.search": { pt: "(pesquisa)", en: "(search)", fr: "(recherche)" },
    "terminal.mode.type_word": { pt: "Escreva {word}", en: "Type {word}", fr: "Tapez {word}" },
    "terminal.mode.yn": { pt: "Continuar? [s/N]", en: "Continue? [y/N]", fr: "Continuer ? [o/N]" },
    "terminal.mode.mfa": { pt: "Código MFA", en: "MFA code", fr: "Code MFA" },
    "terminal.mode.offline": { pt: "sem ligação", en: "offline", fr: "hors ligne" },
    "terminal.ac.hint": { pt: "Tab completa · ↑↓ navega · → aceita sugestão · Esc fecha", en: "Tab completes · ↑↓ navigate · → accepts suggestion · Esc closes", fr: "Tab complète · ↑↓ naviguer · → accepte · Échap ferme" },
    "terminal.ac.kind.cmd": { pt: "cmd", en: "cmd", fr: "cmd" },
    "terminal.ac.kind.sub": { pt: "sub", en: "sub", fr: "sub" },
    "terminal.ac.kind.opt": { pt: "opt", en: "opt", fr: "opt" },
    "terminal.ac.kind.res": { pt: "recurso", en: "resource", fr: "ressource" },
    "terminal.rs.title": { pt: "PESQUISA NO HISTÓRICO", en: "HISTORY SEARCH", fr: "RECHERCHE DANS L’HISTORIQUE" },
    "terminal.rs.none": { pt: "Sem correspondências.", en: "No matches.", fr: "Aucune correspondance." },
    "terminal.rs.hint": { pt: "Enter coloca na linha · ⌃R seguinte · Esc cancela", en: "Enter puts it on the line · ⌃R next · Esc cancels", fr: "Entrée place sur la ligne · ⌃R suivant · Échap annule" },
    "terminal.paste.title": { pt: "Colou {n} linhas", en: "You pasted {n} lines", fr: "Vous avez collé {n} lignes" },
    "terminal.paste.detail": { pt: "Serão executadas uma a uma, por esta ordem.", en: "They will run one by one, in this order.", fr: "Elles seront exécutées une à une, dans cet ordre." },
    "terminal.paste.risky": { pt: "Inclui {n} comando de alto impacto — cada um pedirá confirmação escrita.", en: "Includes {n} high-impact command — each one will ask for typed confirmation.", fr: "Inclut {n} commande à fort impact — chacune demandera une confirmation saisie." },
    "terminal.paste.run": { pt: "Executar {n} linhas", en: "Run {n} lines", fr: "Exécuter {n} lignes" },
    "terminal.paste.review": { pt: "Rever antes", en: "Review first", fr: "Relire d’abord" },
    "terminal.cancel": { pt: "Cancelar", en: "Cancel", fr: "Annuler" },
    "terminal.copy": { pt: "Copiar", en: "Copy", fr: "Copier" },
    "terminal.copied": { pt: "Copiado", en: "Copied", fr: "Copié" },
    "terminal.elev.band": { pt: "SESSÃO DE ADMINISTRAÇÃO", en: "ADMINISTRATION SESSION", fr: "SESSION D’ADMINISTRATION" },
    "terminal.elev.expires": { pt: "expira em {mmss}", en: "expires in {mmss}", fr: "expire dans {mmss}" },
    "terminal.elev.end": { pt: "Terminar", en: "End", fr: "Terminer" },
    "terminal.offline.title": { pt: "Sem ligação ao Core", en: "No connection to Core", fr: "Pas de connexion au Core" },
    "terminal.offline.detail": { pt: "a religar · tentativa {n} · a saída anterior mantém-se visível", en: "reconnecting · attempt {n} · previous output stays visible", fr: "reconnexion · tentative {n} · la sortie précédente reste visible" },
    "terminal.offline.placeholder": { pt: "Sem ligação ao Core — os comandos não são aceites", en: "No connection to Core — commands are not accepted", fr: "Pas de connexion au Core — les commandes ne sont pas acceptées" },
    "terminal.offline.retry": { pt: "Tentar agora", en: "Retry now", fr: "Réessayer" },
    "terminal.reconnecting": { pt: "A religar ao Core…", en: "Reconnecting to Core…", fr: "Reconnexion au Core…" },
    "terminal.reconnected": { pt: "Ligação restabelecida · sessão retomada", en: "Connection restored · session resumed", fr: "Connexion rétablie · session reprise" },
    "terminal.status.connected": { pt: "Ligado ao Core", en: "Connected to Core", fr: "Connecté au Core" },
    "terminal.status.disconnected": { pt: "Desligado", en: "Disconnected", fr: "Déconnecté" },
    "terminal.status.ai_local": { pt: "IA: local", en: "AI: local", fr: "IA : locale" },
    "terminal.status.ai_none": { pt: "IA: sem recurso", en: "AI: no resource", fr: "IA : aucune ressource" },
    "terminal.inspector": { pt: "Inspector de comandos", en: "Command Inspector", fr: "Inspecteur de commandes" },
    "terminal.inspector.empty": { pt: "Execute um comando para ver a capacidade, o âmbito e a autoridade usados.", en: "Run a command to see the capability, scope and authority used.", fr: "Exécutez une commande pour voir la capacité, la portée et l’autorité utilisées." },
    "terminal.inspector.note": { pt: "O Terminal, o Nye e a interface chamam as mesmas capacidades do Core. A decisão é sempre do Core.", en: "Terminal, Nye and the UI call the same Core capabilities. Core always decides.", fr: "Le Terminal, Nye et l’interface appellent les mêmes capacités du Core. Le Core décide toujours." },
    "terminal.inspector.command": { pt: "COMANDO", en: "COMMAND", fr: "COMMANDE" },
    "terminal.inspector.capability": { pt: "CAPACIDADE", en: "CAPABILITY", fr: "CAPACITÉ" },
    "terminal.inspector.scope": { pt: "ÂMBITO", en: "SCOPE", fr: "PORTÉE" },
    "terminal.inspector.authority": { pt: "AUTORIDADE", en: "AUTHORITY", fr: "AUTORITÉ" },
    "terminal.inspector.policy": { pt: "POLÍTICA", en: "POLICY", fr: "POLITIQUE" },
    "terminal.inspector.result": { pt: "RESULTADO", en: "RESULT", fr: "RÉSULTAT" },
    "terminal.inspector.duration": { pt: "DURAÇÃO", en: "DURATION", fr: "DURÉE" },
    "terminal.inspector.audit": { pt: "AUDITORIA", en: "AUDIT", fr: "AUDIT" },
    "terminal.prefs.title": { pt: "Preferências do Terminal", en: "Terminal preferences", fr: "Préférences du Terminal" },
    "terminal.prefs.sub": { pt: "Aplicam-se a todas as sessões deste membro. Não alteram permissões.", en: "Apply to all of this member’s sessions. They don’t change permissions.", fr: "S’appliquent à toutes les sessions de ce membre. Elles ne changent pas les permissions." },
    "terminal.prefs.appearance": { pt: "APARÊNCIA", en: "APPEARANCE", fr: "APPARENCE" },
    "terminal.prefs.theme": { pt: "Tema", en: "Theme", fr: "Thème" },
    "terminal.prefs.theme.desc": { pt: "O Terminal usa uma superfície escura por omissão, mesmo com o sistema em modo claro.", en: "Terminal uses a dark surface by default, even with the system in light mode.", fr: "Le Terminal utilise une surface sombre par défaut, même en mode clair." },
    "terminal.prefs.theme.dark": { pt: "Escuro", en: "Dark", fr: "Sombre" },
    "terminal.prefs.theme.light": { pt: "Claro", en: "Light", fr: "Clair" },
    "terminal.prefs.font_size": { pt: "Tamanho da letra", en: "Font size", fr: "Taille de police" },
    "terminal.prefs.cursor": { pt: "Cursor", en: "Cursor", fr: "Curseur" },
    "terminal.prefs.cursor.block": { pt: "Bloco", en: "Block", fr: "Bloc" },
    "terminal.prefs.cursor.bar": { pt: "Barra", en: "Bar", fr: "Barre" },
    "terminal.prefs.cursor.under": { pt: "Sublinhado", en: "Underline", fr: "Souligné" },
    "terminal.prefs.density": { pt: "Densidade", en: "Density", fr: "Densité" },
    "terminal.prefs.density.compact": { pt: "Compacta", en: "Compact", fr: "Compacte" },
    "terminal.prefs.density.normal": { pt: "Normal", en: "Normal", fr: "Normale" },
    "terminal.prefs.density.comfy": { pt: "Ampla", en: "Roomy", fr: "Aérée" },
    "terminal.prefs.session": { pt: "SESSÃO", en: "SESSION", fr: "SESSION" },
    "terminal.prefs.default_context": { pt: "Contexto predefinido", en: "Default context", fr: "Contexte par défaut" },
    "terminal.prefs.default_context.desc": { pt: "Usado ao abrir um novo separador.", en: "Used when opening a new tab.", fr: "Utilisé à l’ouverture d’un onglet." },
    "terminal.prefs.startup": { pt: "Ao abrir", en: "On open", fr: "À l’ouverture" },
    "terminal.prefs.startup.welcome": { pt: "Boas-vindas", en: "Welcome", fr: "Accueil" },
    "terminal.prefs.startup.blank": { pt: "Vazio", en: "Blank", fr: "Vide" },
    "terminal.prefs.scrollback": { pt: "Scrollback", en: "Scrollback", fr: "Historique d’affichage" },
    "terminal.prefs.scrollback.desc": { pt: "Linhas mantidas por sessão.", en: "Lines kept per session.", fr: "Lignes conservées par session." },
    "terminal.prefs.history": { pt: "HISTÓRICO", en: "HISTORY", fr: "HISTORIQUE" },
    "terminal.prefs.history.save": { pt: "Guardar histórico", en: "Save history", fr: "Enregistrer l’historique" },
    "terminal.prefs.history.save.desc": { pt: "Local a este membro, segundo a política da instância.", en: "Local to this member, per instance policy.", fr: "Local à ce membre, selon la politique de l’instance." },
    "terminal.prefs.history.secrets": { pt: "Omitir segredos", en: "Omit secrets", fr: "Omettre les secrets" },
    "terminal.prefs.history.secrets.desc": { pt: "Valores de --password, --token e --secret nunca são guardados.", en: "Values of --password, --token and --secret are never stored.", fr: "Les valeurs de --password, --token et --secret ne sont jamais enregistrées." },
    "terminal.prefs.history.clear": { pt: "Apagar histórico", en: "Clear history", fr: "Effacer l’historique" },
    "terminal.prefs.output": { pt: "SAÍDA", en: "OUTPUT", fr: "SORTIE" },
    "terminal.prefs.show_duration": { pt: "Mostrar duração", en: "Show duration", fr: "Afficher la durée" },
    "terminal.prefs.show_exit": { pt: "Mostrar código de saída", en: "Show exit status", fr: "Afficher le code de sortie" },
    "terminal.prefs.show_exit.desc": { pt: "Por omissão, só as falhas mostram o código.", en: "By default, only failures show the code.", fr: "Par défaut, seuls les échecs affichent le code." },
    "terminal.prefs.copy_select": { pt: "Copiar ao seleccionar", en: "Copy on select", fr: "Copier à la sélection" },
    "terminal.prefs.debug": { pt: "Modo de depuração", en: "Debug mode", fr: "Mode débogage" },
    "terminal.prefs.debug.desc": { pt: "Mostra o Inspector de comandos a qualquer função.", en: "Shows the Command Inspector to any role.", fr: "Affiche l’inspecteur de commandes pour tout rôle." },
    "terminal.prefs.security": { pt: "SEGURANÇA", en: "SECURITY", fr: "SÉCURITÉ" },
    "terminal.prefs.paste_warn": { pt: "Avisar ao colar várias linhas", en: "Warn on multi-line paste", fr: "Avertir au collage multiligne" },
    "terminal.prefs.paste_warn.desc": { pt: "Mostra os comandos antes de os executar.", en: "Shows the commands before running them.", fr: "Affiche les commandes avant de les exécuter." },
    "terminal.prefs.confirmations": { pt: "Confirmações de alto impacto", en: "High-impact confirmations", fr: "Confirmations à fort impact" },
    "terminal.prefs.confirmations.desc": { pt: "Definidas pela política de risco do Core. Não podem ser desactivadas no Terminal.", en: "Set by the Core risk policy. They cannot be turned off in Terminal.", fr: "Définies par la politique de risque du Core. Non désactivables dans le Terminal." },
    "terminal.keys.aria": { pt: "Teclas acessórias", en: "Accessory keys", fr: "Touches accessoires" },
    "ocsh.help.tagline": { pt: "a shell do Ocinye OS. Os mesmos comandos em pt, en e fr.", en: "the Ocinye OS shell. The same commands in pt, en and fr.", fr: "le shell d’Ocinye OS. Les mêmes commandes en pt, en et fr." },
    "ocsh.help.footer": { pt: "help <comando> · <comando> --help · ? <pergunta> pergunta ao Nye", en: "help <command> · <command> --help · ? <question> asks Nye", fr: "help <commande> · <commande> --help · ? <question> demande à Nye" },
    "ocsh.help.usage": { pt: "UTILIZAÇÃO", en: "USAGE", fr: "UTILISATION" },
    "ocsh.help.subcommands": { pt: "SUBCOMANDOS", en: "SUBCOMMANDS", fr: "SOUS-COMMANDES" },
    "ocsh.help.options": { pt: "OPÇÕES", en: "OPTIONS", fr: "OPTIONS" },
    "ocsh.help.example": { pt: "EXEMPLO", en: "EXAMPLE", fr: "EXEMPLE" },
    "ocsh.help.details": { pt: "{family} <subcomando> --help para detalhes e opções", en: "{family} <subcommand> --help for details and options", fr: "{family} <sous-commande> --help pour détails et options" },
    "ocsh.help.meta": { pt: "capacidade {cap} · risco {risk}", en: "capability {cap} · risk {risk}", fr: "capacité {cap} · risque {risk}" },
    "ocsh.group.workspace": { pt: "Espaço de trabalho", en: "Workspace", fr: "Espace de travail" },
    "ocsh.group.files": { pt: "Ficheiros", en: "Files", fr: "Fichiers" },
    "ocsh.group.work": { pt: "Trabalho", en: "Work", fr: "Travail" },
    "ocsh.group.ai": { pt: "IA", en: "AI", fr: "IA" },
    "ocsh.group.admin": { pt: "Administração", en: "Administration", fr: "Administration" },
    "ocsh.group.system": { pt: "Sistema", en: "System", fr: "Système" },
    "ocsh.group.shell": { pt: "Shell", en: "Shell", fr: "Shell" },
    "ocsh.risk.low": { pt: "baixo", en: "low", fr: "faible" },
    "ocsh.risk.med": { pt: "médio", en: "medium", fr: "moyen" },
    "ocsh.risk.high": { pt: "alto", en: "high", fr: "élevé" },
    "ocsh.err.not_found": { pt: "Comando não encontrado: {cmd}", en: "Command not found: {cmd}", fr: "Commande introuvable : {cmd}" },
    "ocsh.err.did_you_mean": { pt: "Quis dizer {cmd}?", en: "Did you mean {cmd}?", fr: "Vouliez-vous dire {cmd} ?" },
    "ocsh.err.posix": { pt: "No ocsh, os ficheiros são operados pela família files.", en: "In ocsh, files are handled by the files family.", fr: "Dans ocsh, les fichiers passent par la famille files." },
    "ocsh.err.missing_sub": { pt: "Falta o subcomando.", en: "Missing subcommand.", fr: "Sous-commande manquante." },
    "ocsh.err.bad_sub": { pt: "Subcomando inválido: {cmd}", en: "Invalid subcommand: {cmd}", fr: "Sous-commande invalide : {cmd}" },
    "ocsh.err.bad_opt": { pt: "Opção inválida: {opt}", en: "Invalid option: {opt}", fr: "Option invalide : {opt}" },
    "ocsh.err.no_opts": { pt: "Este subcomando não aceita opções.", en: "This subcommand takes no options.", fr: "Cette sous-commande n’accepte pas d’options." },
    "ocsh.err.missing_arg": { pt: "Falta o argumento.", en: "Missing argument.", fr: "Argument manquant." },
    "ocsh.err.resource_not_found": { pt: "{kind} não encontrado: {q}", en: "{kind} not found: {q}", fr: "{kind} introuvable : {q}" },
    "ocsh.err.tab_hint": { pt: "Use Tab para ver as opções disponíveis.", en: "Press Tab to see available options.", fr: "Appuyez sur Tab pour voir les options." },
    "ocsh.err.host_path": { pt: "Caminho fora do espaço Ocinye: {path}", en: "Path outside the Ocinye namespace: {path}", fr: "Chemin hors de l’espace Ocinye : {path}" },
    "ocsh.err.host_path.detail": { pt: "O ocsh opera o armazenamento governado (~/files, ~/projects, ~/notes). Caminhos do anfitrião como /etc não existem aqui.", en: "ocsh operates governed storage (~/files, ~/projects, ~/notes). Host paths such as /etc do not exist here.", fr: "ocsh opère le stockage gouverné (~/files, ~/projects, ~/notes). Les chemins de l’hôte comme /etc n’existent pas ici." },
    "ocsh.err.pipe": { pt: "Operação de pipeline desconhecida: {op}", en: "Unknown pipeline operation: {op}", fr: "Opération de pipeline inconnue : {op}" },
    "ocsh.err.pipe.detail": { pt: "Disponíveis: filter, sort, head, count, export json", en: "Available: filter, sort, head, count, export json", fr: "Disponibles : filter, sort, head, count, export json" },
    "ocsh.err.network": { pt: "Erro de rede. O comando não foi enviado.", en: "Network error. The command was not sent.", fr: "Erreur réseau. La commande n’a pas été envoyée." },
    "ocsh.err.policy": { pt: "Bloqueado pela política da instância.", en: "Blocked by instance policy.", fr: "Bloqué par la politique de l’instance." },
    "ocsh.err.unavailable": { pt: "Capacidade indisponível neste momento.", en: "Capability currently unavailable.", fr: "Capacité indisponible pour le moment." },
    "ocsh.denied": { pt: "Permissão negada", en: "Permission denied", fr: "Permission refusée" },
    "ocsh.denied.detail": { pt: "Não tem autorização para executar este comando neste contexto.", en: "You are not authorised to run this command in this context.", fr: "Vous n’êtes pas autorisé à exécuter cette commande dans ce contexte." },
    "ocsh.denied.hint": { pt: "Se precisa deste acesso, peça-o a um administrador da instância.", en: "If you need this access, ask an instance administrator.", fr: "Si vous avez besoin de cet accès, demandez-le à un administrateur de l’instance." },
    "ocsh.elev.required": { pt: "Requer sessão de administração", en: "Requires an administration session", fr: "Nécessite une session d’administration" },
    "ocsh.elev.required.detail": { pt: "Tem esta função, mas as capacidades de administração só ficam activas numa sessão explícita, com MFA e prazo.", en: "You hold this role, but admin capabilities are only active in an explicit, time-bound MFA session.", fr: "Vous avez ce rôle, mais les capacités d’administration ne sont actives que dans une session explicite, limitée et avec MFA." },
    "ocsh.elev.title": { pt: "ELEVAÇÃO DE SESSÃO", en: "SESSION ELEVATION", fr: "ÉLÉVATION DE SESSION" },
    "ocsh.elev.mechanism": { pt: "mecanismo de segurança do Ocinye", en: "Ocinye security mechanism", fr: "mécanisme de sécurité Ocinye" },
    "ocsh.elev.active": { pt: "Sessão de administração activa", en: "Administration session active", fr: "Session d’administration active" },
    "ocsh.elev.active.detail": { pt: "Expira às {time} · session drop termina antes", en: "Expires at {time} · session drop ends it early", fr: "Expire à {time} · session drop la termine avant" },
    "ocsh.elev.ended": { pt: "Sessão de administração terminada.", en: "Administration session ended.", fr: "Session d’administration terminée." },
    "ocsh.host.title": { pt: "O ocsh não é uma shell do anfitrião", en: "ocsh is not a host shell", fr: "ocsh n’est pas un shell de l’hôte" },
    "ocsh.host.detail": { pt: "Os comandos operam capacidades do Ocinye. O acesso ao sistema anfitrião é uma superfície separada — a Consola do Anfitrião — restrita a operadores e auditada.", en: "Commands operate Ocinye capabilities. Host access is a separate surface — the Host Console — restricted to operators and audited.", fr: "Les commandes opèrent des capacités Ocinye. L’accès à l’hôte est une surface distincte — la Console de l’hôte — réservée aux opérateurs et auditée." },
    "ocsh.sudo.title": { pt: "O ocsh não tem sudo", en: "ocsh has no sudo", fr: "ocsh n’a pas de sudo" },
    "ocsh.sudo.detail": { pt: "A autoridade vem das suas capacidades no Core. Para acções de administração, active uma sessão explícita e auditada.", en: "Authority comes from your capabilities in Core. For administration, start an explicit, audited session.", fr: "L’autorité vient de vos capacités dans Core. Pour l’administration, ouvrez une session explicite et auditée." },
    "ocsh.confirm.high": { pt: "ACÇÃO DE ALTO IMPACTO", en: "HIGH-IMPACT ACTION", fr: "ACTION À FORT IMPACT" },
    "ocsh.confirm.med": { pt: "CONFIRMAÇÃO", en: "CONFIRMATION", fr: "CONFIRMATION" },
    "ocsh.confirm.word.revoke": { pt: "REVOGAR", en: "REVOKE", fr: "RÉVOQUER" },
    "ocsh.confirm.wait_word": { pt: "Escreva {word} para confirmar · Esc cancela", en: "Type {word} to confirm · Esc cancels", fr: "Tapez {word} pour confirmer · Échap annule" },
    "ocsh.confirm.wait": { pt: "A aguardar confirmação · Esc cancela", en: "Awaiting confirmation · Esc cancels", fr: "En attente de confirmation · Échap annule" },
    "ocsh.confirm.ok": { pt: "Confirmado e executado", en: "Confirmed and executed", fr: "Confirmé et exécuté" },
    "ocsh.confirm.aborted": { pt: "Cancelado. Nada foi alterado.", en: "Cancelled. Nothing was changed.", fr: "Annulé. Rien n’a été modifié." },
    "ocsh.confirm.mismatch": { pt: "Confirmação não corresponde. Cancelado — nada foi alterado.", en: "Confirmation did not match. Cancelled — nothing was changed.", fr: "La confirmation ne correspond pas. Annulé — rien n’a été modifié." },
    "ocsh.confirm.k.action": { pt: "Acção", en: "Action", fr: "Action" },
    "ocsh.confirm.k.target": { pt: "Alvo", en: "Target", fr: "Cible" },
    "ocsh.confirm.k.effects": { pt: "Efeitos", en: "Effects", fr: "Effets" },
    "ocsh.confirm.k.preserves": { pt: "Preserva", en: "Preserves", fr: "Préserve" },
    "ocsh.confirm.k.reversible": { pt: "Reversível", en: "Reversible", fr: "Réversible" },
    "ocsh.confirm.k.capability": { pt: "Capacidade", en: "Capability", fr: "Capacité" },
    "ocsh.confirm.k.audit": { pt: "Auditoria", en: "Audit", fr: "Audit" },
    "ocsh.receipt": { pt: "recibo", en: "receipt", fr: "reçu" },
    "ocsh.cancelled_at": { pt: "Cancelado aos {pct} %", en: "Cancelled at {pct} %", fr: "Annulé à {pct} %" },
    "ocsh.follow_ended": { pt: "Seguimento terminado.", en: "Follow ended.", fr: "Suivi terminé." },
    "ocsh.logs.following": { pt: "a seguir", en: "following", fr: "en suivi" },
    "ocsh.logs.paused": { pt: "em pausa", en: "paused", fr: "en pause" },
    "ocsh.logs.ended": { pt: "terminado", en: "ended", fr: "terminé" },
    "ocsh.logs.filter": { pt: "filtrar", en: "filter", fr: "filtrer" },
    "ocsh.logs.pause": { pt: "Pausa", en: "Pause", fr: "Pause" },
    "ocsh.logs.resume": { pt: "Retomar", en: "Resume", fr: "Reprendre" },
    "ocsh.no_results": { pt: "Sem resultados.", en: "No results.", fr: "Aucun résultat." },
    "ocsh.empty.projects": { pt: "Sem projectos neste contexto.", en: "No projects in this context.", fr: "Aucun projet dans ce contexte." },
    "ocsh.empty.folder": { pt: "Pasta vazia.", en: "Empty folder.", fr: "Dossier vide." },
    "ocsh.nye.who": { pt: "o Nye", en: "Nye", fr: "Nye" },
    "ocsh.nye.thinking": { pt: "o Nye está a pensar…", en: "Nye is thinking…", fr: "Nye réfléchit…" },
    "ocsh.nye.route": { pt: "rota: {route}", en: "route: {route}", fr: "route : {route}" },
    "ocsh.nye.sources": { pt: "fontes: {caps}", en: "sources: {caps}", fr: "sources : {caps}" },
    "ocsh.nye.unavailable": { pt: "O Nye não está disponível — sem recurso de inferência", en: "Nye is unavailable — no inference resource", fr: "Nye est indisponible — aucune ressource d’inférence" },
    "ocsh.nye.unavailable.detail": { pt: "Os comandos determinísticos continuam a funcionar sem IA.", en: "Deterministic commands keep working without AI.", fr: "Les commandes déterministes fonctionnent sans IA." },
    "ocsh.nye.deterministic": { pt: "Equivalente determinístico:", en: "Deterministic equivalent:", fr: "Équivalent déterministe :" },
    "ocsh.nye.propose": { pt: "Proponho executar este comando determinístico:", en: "I propose running this deterministic command:", fr: "Je propose cette commande déterministe :" },
    "ocsh.nye.proposed": { pt: "ACÇÃO PROPOSTA PELO NYE", en: "ACTION PROPOSED BY NYE", fr: "ACTION PROPOSÉE PAR NYE" },
    "ocsh.nye.authority": { pt: "a sua — o Nye não tem mais permissões do que você", en: "yours — Nye holds no more permissions than you", fr: "la vôtre — Nye n’a pas plus de permissions que vous" },
    "ocsh.state.pending_exec": { pt: "Este comando ainda não está ligado ao Core nesta versão.", en: "This command is not connected to Core in this version yet.", fr: "Cette commande n’est pas encore reliée au Core dans cette version." },
    "ocsh.history.hint": { pt: "⌃R pesquisa · history clear apaga · comandos com segredos não são guardados", en: "⌃R search · history clear deletes · commands with secrets are not stored", fr: "⌃R rechercher · history clear efface · les commandes avec secrets ne sont pas enregistrées" },
    "ocsh.status.healthy": { pt: "saudável", en: "healthy", fr: "sain" },
    "ocsh.status.degraded": { pt: "degradado", en: "degraded", fr: "dégradé" },
    "ocsh.status.no_resource": { pt: "sem recurso", en: "no resource", fr: "aucune ressource" },
    "ocsh.status.active": { pt: "Activo", en: "Active", fr: "Actif" },
    "ocsh.status.paused": { pt: "Pausado", en: "Paused", fr: "En pause" },
    "ocsh.status.done": { pt: "Concluído", en: "Done", fr: "Terminé" },
};

// Chaves de engenharia do Terminal que o D14 não traz: descrições do registo
// (`ocsh.family.*`, `ocsh.cmd.*`), rótulos de colunas (`ocsh.col.*`) e erros
// do parser e do executor. Texto, não desenho.
const DS_TERMINAL_ENGINE: &[Entry] = catalogo! {
    "ocsh.family.help": { pt: "Ajuda do ocsh", en: "ocsh help", fr: "Aide d’ocsh" },
    "ocsh.family.clear": { pt: "Limpar o ecrã", en: "Clear the screen", fr: "Effacer l’écran" },
    "ocsh.family.history": { pt: "Histórico desta sessão", en: "History of this session", fr: "Historique de cette session" },
    "ocsh.family.exit": { pt: "Fechar o separador", en: "Close the tab", fr: "Fermer l’onglet" },
    "ocsh.family.whoami": { pt: "Quem sou e onde estou", en: "Who I am and where I am", fr: "Qui je suis et où je suis" },
    "ocsh.family.context": { pt: "Contexto do separador", en: "Tab context", fr: "Contexte de l’onglet" },
    "ocsh.family.tasks": { pt: "Tarefas", en: "Tasks", fr: "Tâches" },
    "ocsh.family.nodes": { pt: "Nós de computação", en: "Compute nodes", fr: "Nœuds de calcul" },
    "ocsh.family.nye": { pt: "Perguntar ao Nye", en: "Ask Nye", fr: "Demander à Nye" },
    "ocsh.cmd.help": { pt: "Mostra os comandos que pode usar", en: "Shows the commands you can use", fr: "Affiche les commandes que vous pouvez utiliser" },
    "ocsh.cmd.clear": { pt: "Limpa o ecrã; o histórico fica", en: "Clears the screen; history is kept", fr: "Efface l’écran ; l’historique reste" },
    "ocsh.cmd.history": { pt: "Mostra as linhas desta sessão, sem segredos", en: "Shows this session’s lines, without secrets", fr: "Affiche les lignes de cette session, sans secrets" },
    "ocsh.cmd.exit": { pt: "Fecha este separador", en: "Closes this tab", fr: "Ferme cet onglet" },
    "ocsh.cmd.whoami": { pt: "Membro, instância, papéis e contexto", en: "Member, instance, roles and context", fr: "Membre, instance, rôles et contexte" },
    "ocsh.cmd.context.show": { pt: "Mostra o contexto activo", en: "Shows the active context", fr: "Affiche le contexte actif" },
    "ocsh.cmd.context.list": { pt: "Lista os ambientes que pode usar", en: "Lists the workspaces you can use", fr: "Liste les espaces que vous pouvez utiliser" },
    "ocsh.cmd.context.use": { pt: "Muda o contexto deste separador", en: "Changes this tab’s context", fr: "Change le contexte de cet onglet" },
    "ocsh.cmd.tasks.list": { pt: "Lista as tarefas do contexto", en: "Lists the context’s tasks", fr: "Liste les tâches du contexte" },
    "ocsh.cmd.nodes.list": { pt: "Nós de computação registados e em linha", en: "Registered and online compute nodes", fr: "Nœuds de calcul enregistrés et en ligne" },
    "ocsh.cmd.nye.ask": { pt: "Pergunta ao Nye, que só propõe", en: "Asks Nye, who only proposes", fr: "Demande à Nye, qui ne fait que proposer" },
    "ocsh.col.display_name": { pt: "Nome", en: "Name", fr: "Nom" },
    "ocsh.col.instance": { pt: "Instância", en: "Instance", fr: "Instance" },
    "ocsh.col.roles": { pt: "Papéis", en: "Roles", fr: "Rôles" },
    "ocsh.col.context": { pt: "Contexto", en: "Context", fr: "Contexte" },
    "ocsh.col.title": { pt: "Título", en: "Title", fr: "Titre" },
    "ocsh.col.kind": { pt: "Tipo", en: "Kind", fr: "Type" },
    "ocsh.col.code": { pt: "Código", en: "Code", fr: "Code" },
    "ocsh.col.state": { pt: "Estado", en: "State", fr: "État" },
    "ocsh.col.due_on": { pt: "Prazo", en: "Due", fr: "Échéance" },
    "ocsh.col.registered": { pt: "Registados", en: "Registered", fr: "Enregistrés" },
    "ocsh.col.online": { pt: "Em linha", en: "Online", fr: "En ligne" },
    "ocsh.err.bad_value": { pt: "Valor inválido para {opt}.", en: "Invalid value for {opt}.", fr: "Valeur invalide pour {opt}." },
    "ocsh.err.missing_value": { pt: "Falta o valor de {opt}.", en: "Missing value for {opt}.", fr: "Valeur manquante pour {opt}." },
    "ocsh.err.unexpected_arg": { pt: "Argumento a mais: {cmd}", en: "Unexpected argument: {cmd}", fr: "Argument en trop : {cmd}" },
    "ocsh.err.host_syntax": { pt: "{op} não existe no ocsh", en: "{op} does not exist in ocsh", fr: "{op} n’existe pas dans ocsh" },
    "ocsh.err.host_syntax.detail": { pt: "O ocsh não encadeia processos, não redirecciona nem expande variáveis. Para compor, use o pipeline tipado: | filter, sort, head, count, export json.", en: "ocsh does not chain processes, redirect or expand variables. To compose, use the typed pipeline: | filter, sort, head, count, export json.", fr: "ocsh n’enchaîne pas de processus, ne redirige pas et n’étend pas de variables. Pour composer, utilisez le pipeline typé : | filter, sort, head, count, export json." },
    "ocsh.err.syntax": { pt: "Linha inválida: {detail}", en: "Invalid line: {detail}", fr: "Ligne invalide : {detail}" },
    "ocsh.err.no_json": { pt: "Este comando não tem saída JSON.", en: "This command has no JSON output.", fr: "Cette commande n’a pas de sortie JSON." },
    "ocsh.err.not_pipeable": { pt: "Esta saída não é uma tabela; não entra num pipeline.", en: "This output is not a table; it cannot enter a pipeline.", fr: "Cette sortie n’est pas un tableau ; elle ne peut pas entrer dans un pipeline." },
    "ocsh.err.unknown_column": { pt: "Coluna desconhecida: {column}", en: "Unknown column: {column}", fr: "Colonne inconnue : {column}" },
    "ocsh.err.invalid": { pt: "O Core recusou os valores do comando.", en: "Core rejected the command’s values.", fr: "Le Core a refusé les valeurs de la commande." },
    "ocsh.err.failed": { pt: "O comando falhou.", en: "The command failed.", fr: "La commande a échoué." },
    "ocsh.err.resource_missing": { pt: "Não encontrado, ou não acessível a si.", en: "Not found, or not accessible to you.", fr: "Introuvable, ou inaccessible pour vous." },
    "ocsh.err.needs_confirmation": { pt: "Este comando precisa de confirmação, que o Terminal ainda não oferece.", en: "This command needs confirmation, which the Terminal does not offer yet.", fr: "Cette commande nécessite une confirmation, que le Terminal ne propose pas encore." },
    "ocsh.err.needs_workspace": { pt: "Este comando precisa de um ambiente. Escolha um com context use.", en: "This command needs a workspace. Pick one with context use.", fr: "Cette commande nécessite un espace. Choisissez-en un avec context use." },
    "ocsh.err.context_not_found": { pt: "Ambiente não encontrado entre os seus: {target}", en: "Workspace not found among yours: {target}", fr: "Espace introuvable parmi les vôtres : {target}" },
    "ocsh.err.context_unreachable": { pt: "O contexto deste separador deixou de estar acessível. Voltou ao pessoal.", en: "This tab’s context is no longer accessible. It is back to personal.", fr: "Le contexte de cet onglet n’est plus accessible. Retour au personnel." },
    "ocsh.ok.context_personal": { pt: "Contexto: pessoal", en: "Context: personal", fr: "Contexte : personnel" },
    "ocsh.ok.context_workspace": { pt: "Contexto: {code}", en: "Context: {code}", fr: "Contexte : {code}" },
};

/// Os componentes do arranque (`/ready`), pelo nome que a pessoa lê. O Design
/// (D001) desenha a linha de cada componente mas não traz os nomes; são do
/// sistema, e por isso vivem aqui e não em `ui_auth`.
const BOOT_COMPONENTS: &[Entry] = catalogo! {
    "boot.component.core": { pt: "Núcleo institucional", en: "Institutional core", fr: "Noyau institutionnel" },
    "boot.component.persistence": { pt: "Persistência", en: "Persistence", fr: "Persistance" },
    "boot.component.identity": { pt: "Identidade", en: "Identity", fr: "Identité" },
    "boot.component.compatibility": { pt: "Compatibilidade", en: "Compatibility", fr: "Compatibilité" },
    "boot.component.storage": { pt: "Armazenamento", en: "Storage", fr: "Stockage" },
    "boot.component.mail": { pt: "Correio", en: "Mail", fr: "Courrier" },
    "boot.component.intelligence": { pt: "Inteligência", en: "Intelligence", fr: "Intelligence" },
    "boot.component.compute": { pt: "Computação", en: "Compute", fr: "Calcul" },
    "boot.component.calendar": { pt: "Calendário", en: "Calendar", fr: "Calendrier" },
    "boot.component.realtime": { pt: "Tempo real", en: "Real time", fr: "Temps réel" },
    // A recusa do início de sessão do Core (`SIGN_IN_REFUSED`), a mesma para
    // todas as falhas de credencial. O Core escreve-a só em português; a porta
    // fala a língua escolhida.
    "auth.refused.sign_in": { pt: "Endereço ou palavra-passe inválidos.", en: "Invalid address or password.", fr: "Adresse ou mot de passe invalide." },
};

pub const GROUPS: &[&[Entry]] = &[
    DS_TERMINAL,
    DS_TERMINAL_ENGINE,
    NAV,
    APPS,
    BOOT_COMPONENTS,
    super::ui_auth::UI_AUTH,
    super::ui_base::UI_BASE,
    super::ui_shell::UI_SHELL,
    super::ui_nye::UI_NYE,
    super::ui_apps::UI_APPS,
    super::ui_research::UI_RESEARCH,
    super::ui_org::UI_ORG,
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
