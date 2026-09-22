//! A camada de idioma do Workspace.
//!
//! # Uma só via de resolução
//!
//! Todo o texto de produto passa por aqui: [`t`] para uma mensagem, [`tf`] para
//! uma com valores interpolados, [`tp`] para uma que conta. Não há `if locale ==
//! "fr"` espalhado pelos ecrãs, nem catálogos por página — há uma via, e é esta
//! (briefing i18n §5).
//!
//! # O idioma corrente é do pedido, não do parâmetro
//!
//! Fiar o idioma por centenas de funções de renderização seria reescrever a
//! assinatura de todo o produto. Em vez disso, o idioma vive num
//! [`tokio::task_local`]: um middleware resolve-o no início do pedido e corre o
//! resto do trabalho dentro de [`with_locale`]. Como a renderização SSR é
//! síncrona dentro da mesma tarefa, qualquer `t(...)` chamado ao construir ou ao
//! materializar a árvore lê o idioma certo — sem tocar numa única assinatura.
//!
//! # A queda é sempre para o canónico
//!
//! Falta uma tradução em `en` ou `fr`? A resposta é o português canónico, nunca
//! a chave crua (briefing i18n §7, §8). O portão de CI garante que, em produção,
//! não falta nenhuma; a queda existe para nunca mostrar uma chave, mesmo assim.

mod catalog;

#[cfg(test)]
mod completeness;

pub use ocinye_contracts::Locale;

tokio::task_local! {
    /// O idioma do pedido em curso. Só o middleware lhe entra; tudo o resto lê.
    static IDIOMA: Locale;
}

/// Corre `f` com `locale` como o idioma corrente.
///
/// O middleware embrulha o resto do pedido nisto. Fora de um escopo destes —
/// num teste, num trabalho de fundo — [`current`] devolve o canónico, e o texto
/// sai em português: a ausência de idioma é português, não um erro.
pub async fn with_locale<F, T>(locale: Locale, f: F) -> T
where
    F: std::future::Future<Output = T>,
{
    IDIOMA.scope(locale, f).await
}

/// O idioma corrente, ou o canónico fora de um pedido.
#[must_use]
pub fn current() -> Locale {
    IDIOMA
        .try_with(|l| *l)
        .unwrap_or(ocinye_contracts::locale::CANONICAL)
}

/// A mensagem de uma chave, no idioma corrente.
///
/// Devolve sempre uma string renderizável: a tradução pedida, ou o português
/// canónico se faltar, ou — só se a chave não existir de todo — a própria chave,
/// que é um erro de programação a apanhar em testes, não uma língua.
#[must_use]
pub fn t(key: &str) -> &'static str {
    resolve(current(), key)
}

/// A mensagem de uma chave num idioma explícito (para testes e pré-render).
#[must_use]
pub fn t_in(locale: Locale, key: &str) -> &'static str {
    resolve(locale, key)
}

/// Se uma chave existe no catálogo.
///
/// Para quem constrói a chave a partir de um valor de domínio (um estado, um
/// tipo) e precisa de saber se há tradução antes de a pedir, em vez de receber a
/// chave crua de volta.
#[must_use]
pub fn has(key: &str) -> bool {
    catalog::entry(key).is_some()
}

/// Uma mensagem com valores interpolados: `tf("greeting.evening", &[("name", n)])`.
///
/// A interpolação é textual e segura: os valores entram como texto, e a árvore
/// Leptos escapa o resultado ao renderizá-lo. Não se concatenam fragmentos de
/// frase no código dos ecrãs — a frase inteira é uma chave (briefing i18n §50).
#[must_use]
pub fn tf(key: &str, args: &[(&str, &str)]) -> String {
    interpolate(resolve(current(), key), args)
}

/// A categoria plural de `n` no idioma, pelas regras de cada língua.
///
/// Português e inglês tratam só o `1` como singular; o francês trata também o
/// `0` como singular («0 fichier»). Não se faz plural colando um «s» (briefing
/// i18n §49).
fn plural_category(locale: Locale, n: i64) -> &'static str {
    match locale {
        Locale::Fr => {
            if n.abs() <= 1 {
                "one"
            } else {
                "other"
            }
        }
        Locale::Pt | Locale::En => {
            if n == 1 {
                "one"
            } else {
                "other"
            }
        }
    }
}

/// Uma mensagem que conta: escolhe `<key>.one`/`<key>.other` e interpola `{count}`.
///
/// Ex.: `tp("files.count", 3)` com `files.count.other = "{count} ficheiros"`.
#[must_use]
pub fn tp(key: &str, count: i64) -> String {
    let locale = current();
    let categoria = plural_category(locale, count);
    let chave = format!("{key}.{categoria}");
    let contagem = count.to_string();
    interpolate(resolve(locale, &chave), &[("count", &contagem)])
}

/// A resolução crua: idioma pedido → português → a chave.
fn resolve(locale: Locale, key: &str) -> &'static str {
    match catalog::entry(key) {
        Some(entry) => entry.get(locale).unwrap_or(entry.pt),
        // A chave não existe no catálogo: é um erro de programação. Devolve-se a
        // própria chave para que salte à vista (e o teste de chaves cruas a
        // apanhe), em vez de um pânico que derrubava a página.
        None => leak_key(key),
    }
}

/// Substitui `{nome}` pelos valores dados. O que não casar fica como está.
fn interpolate(template: &str, args: &[(&str, &str)]) -> String {
    if args.is_empty() || !template.contains('{') {
        return template.to_owned();
    }
    let mut out = String::with_capacity(template.len() + 16);
    let mut resto = template;
    while let Some(inicio) = resto.find('{') {
        out.push_str(&resto[..inicio]);
        if let Some(fim) = resto[inicio..].find('}') {
            let nome = &resto[inicio + 1..inicio + fim];
            match args.iter().find(|(chave, _)| *chave == nome) {
                Some((_, valor)) => out.push_str(valor),
                // Um marcador sem valor fica textual: é visível, e melhor visível
                // que engolido em silêncio.
                None => out.push_str(&resto[inicio..inicio + fim + 1]),
            }
            resto = &resto[inicio + fim + 1..];
        } else {
            out.push_str(&resto[inicio..]);
            resto = "";
        }
    }
    out.push_str(resto);
    out
}

/// Uma chave em falta sai como texto estático, sem pânico.
///
/// Interna e rara: só acontece quando o código pede uma chave que o catálogo não
/// tem. `Box::leak` dá o `'static` que a assinatura promete; o custo é uma fuga
/// minúscula por chave inexistente distinta, e o teste de existência garante que
/// isto nunca acontece com uma chave real em produção.
fn leak_key(key: &str) -> &'static str {
    Box::leak(key.to_owned().into_boxed_str())
}

/// Uma entrada do catálogo: a chave e as três línguas (o `pt` obrigatório).
pub struct Entry {
    /// O identificador semântico estável (`nav.home`, `projects.create`).
    pub key: &'static str,
    /// O português canónico — sempre presente.
    pub pt: &'static str,
    /// O inglês, quando traduzido.
    pub en: Option<&'static str>,
    /// O francês, quando traduzido.
    pub fr: Option<&'static str>,
}

impl Entry {
    /// A tradução para um idioma, se existir (o `pt` existe sempre).
    #[must_use]
    pub const fn get(&self, locale: Locale) -> Option<&'static str> {
        match locale {
            Locale::Pt => Some(self.pt),
            Locale::En => self.en,
            Locale::Fr => self.fr,
        }
    }
}

/// Constrói o catálogo de forma declarativa, uma linha por chave.
///
/// Uma entrada tem o `pt` obrigatório e o `en`/`fr` opcionais — o que deixa
/// provar a queda com uma chave sem tradução, enquanto o portão de CI exige que
/// as chaves reais tenham as três (briefing i18n §52, §77).
#[macro_export]
macro_rules! catalogo {
    ( $( $key:literal : { pt: $pt:literal $(, en: $en:literal)? $(, fr: $fr:literal)? } ),* $(,)? ) => {
        &[
            $(
                $crate::i18n::Entry {
                    key: $key,
                    pt: $pt,
                    en: $crate::catalogo!(@opt $($en)?),
                    fr: $crate::catalogo!(@opt $($fr)?),
                },
            )*
        ]
    };
    (@opt) => { None };
    (@opt $v:literal) => { Some($v) };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cai_no_portugues_quando_falta_a_traducao() {
        // `test.fallback.only_pt` existe só em pt no catálogo de teste.
        assert_eq!(t_in(Locale::Pt, "test.fallback.only_pt"), "Só português");
        assert_eq!(
            t_in(Locale::Fr, "test.fallback.only_pt"),
            "Só português",
            "sem fr, cai no canónico — nunca a chave crua"
        );
        assert_eq!(t_in(Locale::En, "test.fallback.only_pt"), "Só português");
    }

    #[test]
    fn uma_chave_inexistente_nao_entra_em_panico() {
        assert_eq!(t_in(Locale::Pt, "nao.existe.de.todo"), "nao.existe.de.todo");
    }

    #[test]
    fn interpola_por_nome() {
        assert_eq!(
            interpolate("Bonsoir, {name}", &[("name", "Fidel")]),
            "Bonsoir, Fidel"
        );
        // Um marcador sem valor fica visível.
        assert_eq!(interpolate("Olá, {name}", &[]), "Olá, {name}");
        // Texto sem marcadores não é tocado.
        assert_eq!(interpolate("Sem nada", &[("x", "y")]), "Sem nada");
    }

    #[test]
    fn o_frances_conta_o_zero_como_singular() {
        assert_eq!(plural_category(Locale::Fr, 0), "one");
        assert_eq!(plural_category(Locale::Fr, 1), "one");
        assert_eq!(plural_category(Locale::Fr, 2), "other");
        assert_eq!(plural_category(Locale::Pt, 0), "other");
        assert_eq!(plural_category(Locale::En, 0), "other");
        assert_eq!(plural_category(Locale::Pt, 1), "one");
    }
}
