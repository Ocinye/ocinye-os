//! Textos da D009 · Predefinições de Distribuição e primeiros passos (Claude Design).
//!
//! Os nomes (`dist.*`) e as descrições (`shell.dist.*`) das quatro
//! Distribuições já existem e **não mudam**: são canónicos desde o D001. Aqui
//! entram só os primeiros passos, a proveniência da predefinição, os quatro
//! fundos novos e a nota «predefinição ≠ autorização».
//!
//! Juntar a `GROUPS` em `catalog.rs` (depois de `UI_SYS`).

use crate::i18n::Entry;

/// D009 · Distribuições.
pub const UI_DIST: &[Entry] = crate::catalogo! {
    // ── Primeiros passos (painel da Distribuição e Ajuda › Começar) ─────────
    "dist.first.research.title": { pt: "Começar com Research", en: "Getting started with Research", fr: "Bien démarrer avec Research" },
    "dist.first.research.body": { pt: "Esta Instância começa preparada para investigação. As aplicações fixadas para começar estão na barra de aplicações.", en: "This Instance starts out set up for research. The applications pinned to get you started are in the application bar.", fr: "Cette instance est préparée pour la recherche. Les applications épinglées pour commencer sont dans la barre d’applications." },
    "dist.first.business.title": { pt: "Começar com Business", en: "Getting started with Business", fr: "Bien démarrer avec Business" },
    "dist.first.business.body": { pt: "Esta Instância começa preparada para o trabalho de uma organização: tarefas, agenda e comunicação.", en: "This Instance starts out set up for an organisation’s work: tasks, schedule and communication.", fr: "Cette instance est préparée pour le travail d’une organisation : tâches, agenda et communication." },
    "dist.first.personal.title": { pt: "Começar com Personal", en: "Getting started with Personal", fr: "Bien démarrer avec Personal" },
    "dist.first.personal.body": { pt: "Esta Instância começa simples: os seus ficheiros, notas e agenda. O que guarda fica no seu espaço pessoal.", en: "This Instance starts out simple: your files, notes and schedule. What you save stays in your personal space.", fr: "Cette instance commence simplement : vos fichiers, notes et agenda. Ce que vous enregistrez reste dans votre espace personnel." },
    "dist.first.education.title": { pt: "Começar com Education", en: "Getting started with Education", fr: "Bien démarrer avec Education" },
    "dist.first.education.body": { pt: "Esta Instância começa preparada para ensino e aprendizagem, com o conhecimento e a bibliografia à mão.", en: "This Instance starts out set up for teaching and learning, with knowledge and bibliography at hand.", fr: "Cette instance est préparée pour l’enseignement et l’apprentissage, avec la connaissance et la bibliographie à portée de main." },
    "dist.first.desk": { pt: "Para acrescentar ou retirar widgets, use o lápis no canto superior direito do Desktop.", en: "To add or remove widgets, use the pencil in the top-right corner of the Desktop.", fr: "Pour ajouter ou retirer des widgets, utilisez le crayon en haut à droite du Desktop." },
    "dist.first.nye": { pt: "Para perguntar ou pedir à Nye, use a barra de cima ou ⌘K.", en: "To ask Nye or give it a task, use the top bar or ⌘K.", fr: "Pour interroger Nye ou lui confier une tâche, utilisez la barre du haut ou ⌘K." },
    "dist.first.open_apps": { pt: "Abrir aplicações", en: "Open applications", fr: "Ouvrir les applications" },
    "dist.first.recommended": { pt: "Também disponíveis", en: "Also available", fr: "Également disponibles" },
    "dist.authority": { pt: "A Distribuição define o ponto de partida. O que cada pessoa pode abrir é decidido pela sua autorização.", en: "The Distribution sets the starting point. What each person can open is decided by their authorisation.", fr: "La distribution définit le point de départ. Ce que chaque personne peut ouvrir dépend de son autorisation." },
    "dist.pins": { pt: "Fixadas por omissão", en: "Pinned by default", fr: "Épinglées par défaut" },
    // ── Proveniência e «Repor predefinição» ─────────────────────────────────
    "desk.restore.origin.distribution": { pt: "PREDEFINIÇÃO DA DISTRIBUIÇÃO", en: "DISTRIBUTION DEFAULT", fr: "VALEUR PAR DÉFAUT DE LA DISTRIBUTION" },
    "desk.restore.distribution_name": { pt: "Predefinição {distribution} do Ocinye OS", en: "Ocinye OS {distribution} default", fr: "Valeur par défaut {distribution} d’Ocinye OS" },
    "desk.restore.distribution_meta": { pt: "Versão {version} · incluída no Ocinye OS", en: "Version {version} · shipped with Ocinye OS", fr: "Version {version} · fournie avec Ocinye OS" },
    "desk.restore.system_fallback": { pt: "Predefinição mínima do Ocinye OS", en: "Minimal Ocinye OS default", fr: "Valeur par défaut minimale d’Ocinye OS" },
    "desk.restore.system_fallback_meta": { pt: "Sem widgets · usada quando a Distribuição não é conhecida", en: "No widgets · used when the Distribution is not known", fr: "Aucun widget · utilisée lorsque la distribution n’est pas connue" },
    "desk.restore.pins_kept": { pt: "As aplicações fixadas não mudam.", en: "Pinned applications do not change.", fr: "Les applications épinglées ne changent pas." },
    "desk.prov.member": { pt: "Personalizado por si", en: "Customised by you", fr: "Personnalisé par vous" },
    "desk.prov.distribution": { pt: "Segue a predefinição {distribution} · versão {version}", en: "Follows the {distribution} default · version {version}", fr: "Suit la valeur par défaut {distribution} · version {version}" },
    "desk.prov.instance": { pt: "Segue a predefinição da Instância", en: "Follows the Instance default", fr: "Suit la valeur par défaut de l’instance" },
    "desk.prov.system": { pt: "Segue a predefinição mínima do Ocinye OS", en: "Follows the minimal Ocinye OS default", fr: "Suit la valeur par défaut minimale d’Ocinye OS" },
    // ── Fundos das Distribuições (lista fechada, D009) ─────────────────────
    "desk.wall.field": { pt: "Campo", en: "Field", fr: "Champ" },
    "desk.wall.module": { pt: "Módulo", en: "Module", fr: "Module" },
    "desk.wall.calm": { pt: "Calma", en: "Calm", fr: "Calme" },
    "desk.wall.lattice": { pt: "Trama", en: "Lattice", fr: "Trame" },
    "desk.wall.dist_default": { pt: "Predefinido da Distribuição", en: "Distribution default", fr: "Par défaut de la distribution" },
    // ── Gestor de Aplicações ────────────────────────────────────────────────
    "shell.launcher.dist": { pt: "Distribuição {distribution}", en: "{distribution} Distribution", fr: "Distribution {distribution}" },
};
