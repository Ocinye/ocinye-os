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
    // D007 · Mensagens: sair e retirar pela confirmação partilhada da D006.
    "org.act.msg_leave.label": { pt: "Sair do grupo", en: "Leave the group", fr: "Quitter le groupe" },
    "org.act.msg_leave.title": { pt: "Sair de «{name}»", en: "Leave “{name}”", fr: "Quitter « {name} »" },
    "org.act.msg_leave.body": { pt: "Deixa de ver esta conversa e de receber as mensagens dela. Para voltar, alguém que a gere tem de o acrescentar outra vez.", en: "You stop seeing this conversation and receiving its messages. To come back, someone who manages it must add you again.", fr: "Vous ne verrez plus cette conversation et n’en recevrez plus les messages. Pour revenir, quelqu’un qui la gère doit vous ajouter à nouveau." },
    "org.act.msg_leave.do": { pt: "Sair do grupo", en: "Leave the group", fr: "Quitter le groupe" },
    "org.act.msg_remove.label": { pt: "Retirar", en: "Remove", fr: "Retirer" },
    "org.act.msg_remove.title": { pt: "Retirar {name} da conversa", en: "Remove {name} from the conversation", fr: "Retirer {name} de la conversation" },
    "org.act.msg_remove.body": { pt: "A pessoa deixa de ver esta conversa e de receber as mensagens dela. O que já escreveu fica na conversa.", en: "The person stops seeing this conversation and receiving its messages. What they already wrote stays in the conversation.", fr: "La personne ne verra plus cette conversation et n’en recevra plus les messages. Ce qu’elle a déjà écrit reste dans la conversation." },
    "org.act.msg_remove.do": { pt: "Retirar da conversa", en: "Remove from the conversation", fr: "Retirer de la conversation" },
    "prod.msg.role.owner": { pt: "Dono", en: "Owner", fr: "Propriétaire" },
    "prod.msg.role.administrator": { pt: "Administrador", en: "Administrator", fr: "Administrateur" },
    "prod.msg.role.member": { pt: "Membro", en: "Member", fr: "Membre" },
    // D007 · IA.
    "prod.ai.model.available": { pt: "Disponível", en: "Available", fr: "Disponible" },
    "prod.ai.model.unavailable": { pt: "Indisponível", en: "Unavailable", fr: "Indisponible" },
    "prod.ai.model.disabled": { pt: "Desactivado", en: "Disabled", fr: "Désactivé" },
    "prod.ai.provider.local": { pt: "Fornecedor local", en: "Local provider", fr: "Fournisseur local" },
    "prod.ai.provider.external": { pt: "Fornecedor externo", en: "External provider", fr: "Fournisseur externe" },
    "prod.agents.target.none": { pt: "Sem alvo (pessoal ou institucional)", en: "No target (personal or institutional)", fr: "Sans cible (personnel ou institutionnel)" },
    "prod.nye.about.agent": { pt: "Com o agente «{name}»: ", en: "With the agent “{name}”: ", fr: "Avec l’agent « {name} » : " },
    // D007 · Computação.
    "prod.compute.kind.gpu": { pt: "GPU", en: "GPU", fr: "GPU" },
    "prod.compute.kind.cpu": { pt: "CPU", en: "CPU", fr: "CPU" },
    "prod.compute.kind.hpc": { pt: "Computação de alto desempenho", en: "High-performance computing", fr: "Calcul haute performance" },
    "prod.compute.kind.storage": { pt: "Armazenamento", en: "Storage", fr: "Stockage" },
    "prod.compute.control.ocinye": { pt: "Controlado pelo Ocinye", en: "Controlled by Ocinye", fr: "Contrôlé par Ocinye" },
    "prod.compute.control.external": { pt: "Controlo externo", en: "External control", fr: "Contrôle externe" },
    "prod.compute.residency.undeclared": { pt: "Não declarada", en: "Not declared", fr: "Non déclarée" },
    "prod.compute.residency.cloud": { pt: "Nuvem de terceiros", en: "Third-party cloud", fr: "Cloud tiers" },
    "prod.compute.residency.camama": { pt: "Ocinye Camama", en: "Ocinye Camama", fr: "Ocinye Camama" },
    "prod.compute.residency.colocation": { pt: "Ocinye em colocation", en: "Ocinye colocation", fr: "Ocinye en colocation" },
    // D007 · Actividade: o que se diz de um evento cujo alvo já não se lê.
    "prod.activity.kind.created": { pt: "criou um item", en: "created an item", fr: "a créé un élément" },
    "prod.activity.kind.updated": { pt: "alterou um item", en: "changed an item", fr: "a modifié un élément" },
    "prod.activity.kind.state_changed": { pt: "mudou o estado de um item", en: "changed the state of an item", fr: "a changé l’état d’un élément" },
    "prod.activity.kind.commented": { pt: "comentou um item", en: "commented on an item", fr: "a commenté un élément" },
    "prod.activity.kind.member_added": { pt: "acrescentou um membro", en: "added a member", fr: "a ajouté un membre" },
    "prod.activity.kind.attached": { pt: "anexou a um item", en: "attached to an item", fr: "a joint à un élément" },
    "prod.activity.kind.published": { pt: "publicou um item", en: "published an item", fr: "a publié un élément" },
    "prod.activity.kind.shared": { pt: "partilhou um item", en: "shared an item", fr: "a partagé un élément" },
    "prod.activity.kind.revoked": { pt: "retirou um acesso", en: "revoked an access", fr: "a retiré un accès" },
    "prod.activity.kind.deleted": { pt: "apagou um item", en: "deleted an item", fr: "a supprimé un élément" },
    "prod.activity.kind.restored": { pt: "repôs um item", en: "restored an item", fr: "a restauré un élément" },
    "prod.activity.kind.other": { pt: "fez uma alteração", en: "made a change", fr: "a fait une modification" },
    // D007 · Definições e Ajuda.
    "prod.settings.session.unknown": { pt: "Navegador não identificado", en: "Unidentified browser", fr: "Navigateur non identifié" },
    "prod.help.k.launcher": { pt: "Abrir o lançador de aplicações", en: "Open the application launcher", fr: "Ouvrir le lanceur d’applications" },
    "prod.help.k.switcher": { pt: "Alternar entre janelas abertas", en: "Switch between open windows", fr: "Basculer entre les fenêtres ouvertes" },
    "prod.help.k.send": { pt: "Enviar a mensagem (Mensagens)", en: "Send the message (Messages)", fr: "Envoyer le message (Messages)" },
    // D007.1 · Resultados.
    "prod.results.execution_n": { pt: "Execução n.º {n}", en: "Execution no. {n}", fr: "Exécution n° {n}" },
    "prod.nye.about.result": { pt: "Sobre o resultado «{name}»: ", en: "About the result “{name}”: ", fr: "À propos du résultat « {name} » : " },
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
    "nav.audit": { pt: "Registo de auditoria", en: "Audit log", fr: "Journal d’audit" },
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
    "terminal.context.personal": { pt: "pessoal", en: "personal", fr: "personnel" },
    "terminal.running": { pt: "a executar", en: "running", fr: "en cours" },
    "terminal.paste.title": { pt: "Colou {n} linhas", en: "You pasted {n} lines", fr: "Vous avez collé {n} lignes" },
    "terminal.status.connected": { pt: "Ligado ao Core", en: "Connected to Core", fr: "Connecté au Core" },
    "terminal.status.disconnected": { pt: "Desligado", en: "Disconnected", fr: "Déconnecté" },
    "ocsh.help.footer": { pt: "help <comando> · <comando> --help · ? <pergunta> pergunta ao Nye", en: "help <command> · <command> --help · ? <question> asks Nye", fr: "help <commande> · <commande> --help · ? <question> demande à Nye" },
    "ocsh.group.workspace": { pt: "Espaço de trabalho", en: "Workspace", fr: "Espace de travail" },
    "ocsh.group.work": { pt: "Trabalho", en: "Work", fr: "Travail" },
    "ocsh.group.ai": { pt: "IA", en: "AI", fr: "IA" },
    "ocsh.group.admin": { pt: "Administração", en: "Administration", fr: "Administration" },
    "ocsh.group.system": { pt: "Sistema", en: "System", fr: "Système" },
    "ocsh.group.shell": { pt: "Shell", en: "Shell", fr: "Shell" },
    "ocsh.err.not_found": { pt: "Comando não encontrado: {cmd}", en: "Command not found: {cmd}", fr: "Commande introuvable : {cmd}" },
    "ocsh.err.did_you_mean": { pt: "Quis dizer {cmd}?", en: "Did you mean {cmd}?", fr: "Vouliez-vous dire {cmd} ?" },
    "ocsh.err.posix": { pt: "O ocsh não é uma shell POSIX: não há comandos do anfitrião. Escreva help para ver os comandos que pode usar.", en: "ocsh is not a POSIX shell: there are no host commands. Type help to see the commands you can use.", fr: "ocsh n’est pas un shell POSIX : il n’y a pas de commandes de l’hôte. Tapez help pour voir les commandes disponibles." },
    "ocsh.err.missing_sub": { pt: "Falta o subcomando.", en: "Missing subcommand.", fr: "Sous-commande manquante." },
    "ocsh.err.bad_sub": { pt: "Subcomando inválido: {cmd}", en: "Invalid subcommand: {cmd}", fr: "Sous-commande invalide : {cmd}" },
    "ocsh.err.bad_opt": { pt: "Opção inválida: {opt}", en: "Invalid option: {opt}", fr: "Option invalide : {opt}" },
    "ocsh.err.missing_arg": { pt: "Falta o argumento.", en: "Missing argument.", fr: "Argument manquant." },
    "ocsh.err.pipe": { pt: "Operação de pipeline desconhecida: {op}", en: "Unknown pipeline operation: {op}", fr: "Opération de pipeline inconnue : {op}" },
    "ocsh.err.pipe.detail": { pt: "Disponíveis: filter, sort, head, count, export json", en: "Available: filter, sort, head, count, export json", fr: "Disponibles : filter, sort, head, count, export json" },
    "ocsh.err.network": { pt: "Erro de rede. O comando não foi enviado.", en: "Network error. The command was not sent.", fr: "Erreur réseau. La commande n’a pas été envoyée." },
    "ocsh.err.unavailable": { pt: "Capacidade indisponível neste momento.", en: "Capability currently unavailable.", fr: "Capacité indisponible pour le moment." },
    "ocsh.denied": { pt: "Permissão negada", en: "Permission denied", fr: "Permission refusée" },
    "ocsh.denied.detail": { pt: "Não tem autorização para executar este comando neste contexto.", en: "You are not authorised to run this command in this context.", fr: "Vous n’êtes pas autorisé à exécuter cette commande dans ce contexte." },
    "ocsh.denied.hint": { pt: "Se precisa deste acesso, peça-o a um administrador da instância.", en: "If you need this access, ask an instance administrator.", fr: "Si vous avez besoin de cet accès, demandez-le à un administrateur de l’instance." },
    "ocsh.host.title": { pt: "O ocsh não é uma shell do anfitrião", en: "ocsh is not a host shell", fr: "ocsh n’est pas un shell de l’hôte" },
    "ocsh.host.detail": { pt: "Os comandos operam capacidades do Ocinye através do Core. O acesso ao sistema anfitrião não existe no ocsh: se um dia existir, será outro subsistema, que o ocsh nunca alcança.", en: "Commands operate Ocinye capabilities through the Core. ocsh has no access to the host system: if that ever exists, it will be a separate subsystem that ocsh never reaches.", fr: "Les commandes opèrent les capacités d’Ocinye via le Core. ocsh n’a aucun accès au système hôte : si cela existe un jour, ce sera un autre sous-système, qu’ocsh n’atteint jamais." },
    "ocsh.sudo.title": { pt: "O ocsh não tem sudo", en: "ocsh has no sudo", fr: "ocsh n’a pas de sudo" },
    "ocsh.sudo.detail": { pt: "A autoridade vem das suas capacidades no Core, e nenhum prefixo a aumenta. O que não pode fazer, o Core recusa.", en: "Authority comes from your capabilities in the Core, and no prefix raises it. What you may not do, the Core refuses.", fr: "L’autorité vient de vos capacités dans le Core, et aucun préfixe ne l’élève. Ce que vous ne pouvez pas faire, le Core le refuse." },
    "ocsh.no_results": { pt: "Sem resultados.", en: "No results.", fr: "Aucun résultat." },
};

// Chaves de engenharia do Terminal que o D14 não traz: descrições do registo
// (`ocsh.family.*`, `ocsh.cmd.*`), rótulos de colunas (`ocsh.col.*`) e erros
// do parser e do executor. Texto, não desenho.
const DS_TERMINAL_ENGINE: &[Entry] = catalogo! {
    "ocsh.family.help": { pt: "Ajuda do ocsh", en: "ocsh help", fr: "Aide d’ocsh" },
    "ocsh.family.clear": { pt: "Limpar o ecrã", en: "Clear the screen", fr: "Effacer l’écran" },
    "ocsh.family.history": { pt: "Histórico desta sessão", en: "History of this session", fr: "Historique de cette session" },
    "ocsh.family.exit": { pt: "Fechar o Terminal", en: "Close the Terminal", fr: "Fermer le Terminal" },
    "ocsh.family.whoami": { pt: "Quem sou e onde estou", en: "Who I am and where I am", fr: "Qui je suis et où je suis" },
    "ocsh.family.context": { pt: "Contexto da sessão", en: "Session context", fr: "Contexte de la session" },
    "ocsh.family.tasks": { pt: "Tarefas", en: "Tasks", fr: "Tâches" },
    "ocsh.family.nodes": { pt: "Nós de computação", en: "Compute nodes", fr: "Nœuds de calcul" },
    "ocsh.family.nye": { pt: "Perguntar ao Nye", en: "Ask Nye", fr: "Demander à Nye" },
    "ocsh.cmd.help": { pt: "Mostra os comandos que pode usar", en: "Shows the commands you can use", fr: "Affiche les commandes que vous pouvez utiliser" },
    "ocsh.cmd.clear": { pt: "Limpa o ecrã; o histórico fica", en: "Clears the screen; history is kept", fr: "Efface l’écran ; l’historique reste" },
    "ocsh.cmd.history": { pt: "Mostra as linhas desta sessão, sem segredos", en: "Shows this session’s lines, without secrets", fr: "Affiche les lignes de cette session, sans secrets" },
    "ocsh.cmd.exit": { pt: "Fecha o Terminal", en: "Closes the Terminal", fr: "Ferme le Terminal" },
    "ocsh.cmd.whoami": { pt: "Membro, instância, papéis e contexto", en: "Member, instance, roles and context", fr: "Membre, instance, rôles et contexte" },
    "ocsh.cmd.context.show": { pt: "Mostra o contexto activo", en: "Shows the active context", fr: "Affiche le contexte actif" },
    "ocsh.cmd.context.list": { pt: "Lista os ambientes que pode usar", en: "Lists the workspaces you can use", fr: "Liste les espaces que vous pouvez utiliser" },
    "ocsh.cmd.context.use": { pt: "Muda o contexto desta sessão", en: "Changes this session’s context", fr: "Change le contexte de cette session" },
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
    "ocsh.err.context_unreachable": { pt: "O contexto desta sessão deixou de estar acessível. Voltou ao pessoal.", en: "This session’s context is no longer accessible. It is back to personal.", fr: "Le contexte de cette session n’est plus accessible. Retour au personnel." },
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
    super::ui_ops::UI_OPS,
    super::ui_reg::UI_REG,
    super::ui_sys::UI_SYS,
    super::ui_dist::UI_DIST,
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
