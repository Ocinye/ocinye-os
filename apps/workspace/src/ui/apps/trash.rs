//! D007.1 · Lixo (`/trash`). DESIGN_LOCKED.
//!
//! O Lixo **pessoal** do membro: os ficheiros pessoais (`files.deleted_at`,
//! 0045) e as notas pessoais (`notes.deleted_at`, 0037) que ele próprio apagou.
//! Não é um índice institucional: apagar nunca alarga acesso, e o Lixo não
//! mostra nada de outro membro nem de um ambiente.
//!
//! Sem prazo de recuperação: o Core não tem retenção nem limpeza automática,
//! e a vista **não promete** «30 dias». Diz o que é verdade: fica no Lixo até ser
//! restaurado; um ficheiro no Lixo continua a contar para o armazenamento.
//!
//! «Restaurar» é um POST para a operação do domínio que já existe (o Core
//! reautoriza e decide o destino). «Eliminar definitivamente» segue o
//! precedente D004.1: o Core tem a operação, mas não há confirmação governada
//! de risco destrutivo (FG-D4.1-02); o botão está indisponível, com o motivo,
//! e nunca submete. «Esvaziar o Lixo» não existe pela mesma razão.

use leptos::prelude::*;

use super::res::{detail_or, kv, list, two_pane};
use super::{empty, frame, nav};
use crate::i18n::{t, tf};
use crate::ui::components::icon;
use crate::ui::view_models::{TrashItemVm, TrashKind, TrashNotice, TrashRefusal, TrashVm};

/// O ícone e o rótulo de um tipo no Lixo.
#[must_use]
pub const fn kind_meta(k: TrashKind) -> (&'static str, &'static str) {
    match k {
        TrashKind::File => ("files", "trash.kind.file"),
        TrashKind::Note => ("notes", "trash.kind.note"),
    }
}

fn detail(i: &TrashItemVm, back: String) -> impl IntoView {
    let (ic, k) = kind_meta(i.kind);
    view! {
        <article class="oc-res-doc" data-part="trash-item">
            <header class="oc-res-head">
                <a class="oc-app__icon oc-res-back" href=back aria-label=t("res.back")>{icon("chev-l")}</a>
                <span class="oc-trash-ic" aria-hidden="true">{icon(ic)}</span>
                <div class="oc-res-head__text"><p class="oc-res-head__code">{t(k)}</p><h2 class="oc-res-head__title" id="oc-res-title">{i.name.clone()}</h2></div>
            </header>
            <dl class="oc-res-meta">
                {kv("trash.deleted", Some(i.deleted.clone()))}
                {kv("trash.deleted_by", i.deleted_by.clone())}
                {kv("trash.origin", i.origin.clone())}
                {kv("trash.size", i.size.clone())}
            </dl>
            <p class="oc-app-note">{icon("clock")}<span>{t("trash.retention")}</span></p>
            {i.counts_storage.then(|| view! { <p class="oc-app-note" data-part="trash-storage">{icon("storage")}<span>{t("trash.counts_storage")}</span></p> })}
            <form class="oc-res-actions oc-trash-acts" method="post" action=i.restore_action.clone().unwrap_or_default()>
                <input type="hidden" name="id" value=i.id.clone() />
                {match i.restore_action {
                    Some(_) => view! { <button type="submit" class="oc-app-primary" data-part="trash-restore">{icon("restart")}<span>{t("trash.restore")}</span></button> }.into_any(),
                    None => view! { <button type="button" class="oc-app-btn" disabled="" aria-disabled="true" aria-describedby="oc-trash-restore-why">{icon("restart")}<span>{t("trash.restore")}</span></button><span id="oc-trash-restore-why" class="oc-files-sel__why">{t("trash.restore.unavailable")}</span> }.into_any(),
                }}
                <button type="button" class="oc-app-btn oc-app-btn--danger" disabled="" aria-disabled="true" aria-describedby="oc-trash-purge-why" data-part="trash-purge-unavailable">{icon("trash")}<span>{t("files.purge")}</span></button>
                <span id="oc-trash-purge-why" class="oc-files-sel__why">{t("files.purge.unavailable")}</span>
            </form>
        </article>
    }
}

fn notice(n: TrashNotice, name: &str) -> impl IntoView {
    let k = match n {
        TrashNotice::Restored => "trash.done.restored",
    };
    view! { <p class="oc-app-note oc-org-done" role="status" data-notice=k>{icon("check")}<span>{tf(k, &[("name", name)])}</span></p> }
}

fn refusal(r: TrashRefusal) -> impl IntoView {
    let k = match r {
        TrashRefusal::Gone => "trash.refusal.gone",
        TrashRefusal::Denied => "trash.refusal.denied",
        TrashRefusal::Conflict => "trash.refusal.conflict",
    };
    view! { <p class="oc-app-note oc-org-refusal" data-tone="warn" data-refusal=k role="alert">{icon("shield")}<span><strong>{t(&format!("{k}.title"))}</strong>{" "}{t(&format!("{k}.body"))}</span></p> }
}

/// A aplicação Lixo.
pub fn app(vm: &TrashVm) -> AnyView {
    let toolbar = view! { <h2 class="oc-app__title">{t("nav.trash")}</h2> }.into_any();
    let l = view! {
        {vm.notice.map(|n| notice(n, vm.notice_name.as_deref().unwrap_or_default()))}
        {vm.refusal.map(refusal)}
        {list(&vm.list, "trash.list", empty("trash", "trash.empty", Some("trash.empty.body")).into_any())}
    }
    .into_any();
    let d = match &vm.item {
        Some(i) => detail(i, vm.list_href.clone()).into_any(),
        None => detail_or(
            vm.item_error,
            empty("trash", "trash.none_open", Some("trash.scope")).into_any(),
        ),
    };
    frame(
        "trash",
        t("nav.trash").to_owned(),
        toolbar,
        Some(nav(&vm.nav).into_any()),
        two_pane(vm.pane, l, d),
        None,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::testing::assert_contracts;
    use crate::ui::view_models::{AppLoad, AppPageVm, ResListVm, ResPane};

    fn item(restore: bool) -> TrashItemVm {
        TrashItemVm {
            kind: TrashKind::File,
            id: "f1".into(),
            name: "<script>x</script>.pdf".into(),
            deleted: "hoje".into(),
            deleted_by: None,
            origin: None,
            size: Some("2 MB".into()),
            counts_storage: true,
            restore_action: restore.then(|| "/me/files/restore".into()),
        }
    }

    fn vm(i: Option<TrashItemVm>) -> TrashVm {
        TrashVm {
            nav: vec![],
            list: ResListVm {
                columns: vec![],
                items: vec![],
                load: AppLoad::Ready,
                page: AppPageVm::default(),
            },
            pane: ResPane::Detail,
            list_href: "/trash".into(),
            item: i,
            item_error: None,
            notice: None,
            notice_name: None,
            refusal: None,
        }
    }

    #[test]
    fn eliminar_definitivamente_nunca_submete() {
        let html = app(&vm(Some(item(true)))).to_html();
        assert_contracts(&html);
        assert!(
            html.contains(r#"data-part="trash-purge-unavailable""#)
                && !html.contains("/purge")
                && !html.contains(r#"value="purge""#)
        );
        assert!(html.contains("&lt;script&gt;") && html.contains(t("trash.retention")));
    }

    #[test]
    fn sem_prazo_inventado() {
        let html = app(&vm(Some(item(true)))).to_html();
        assert!(!html.contains("30 dias") && !html.contains("30 days"));
    }

    #[test]
    fn restaurar_indisponivel_nao_submete() {
        let html = app(&vm(Some(item(false)))).to_html();
        assert!(
            !html.contains(r#"data-part="trash-restore""#)
                && html.contains(t("trash.restore.unavailable"))
        );
    }
}
