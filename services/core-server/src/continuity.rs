//! `ocinye-core-server snapshot` e `… verify-snapshot` — continuidade institucional.
//!
//! # O que estes comandos são, e o que não são
//!
//! São a descrição verificável do que esta instalação contém, e a comparação
//! dessa descrição com outra. **Não** são o transporte: copiar o `pg_dump` e os
//! objectos é trabalho de quem opera a máquina, e há ferramentas boas para isso.
//!
//! O que o Core acrescenta é a única parte que ele sabe fazer e mais ninguém:
//! dizer *o que tem de ir* e *se o que chegou é o mesmo*.
//!
//! > **A backup that has never been restored is not evidence of
//! > recoverability.**
//!
//! Por isso não existe aqui um comando que imprima «backup completed
//! successfully» porque o `pg_dump` terminou com zero. O que existe é um
//! manifesto que outra instalação tem de conseguir igualar.
//!
//! # O par
//!
//! ```text
//! servidor A   ocinye-core-server snapshot        > manifesto.json
//!                        ↓ transporte
//! servidor B   ocinye-core-server verify-snapshot < manifesto.json
//! ```
//!
//! O primeiro descreve; o segundo lê a base **local** e compara. Um estado que
//! não coincida sai não-zero e diz onde.

use std::io::Read;

use anyhow::{bail, Context};
use ocinye_core::config::CoreConfig;
use ocinye_core::continuity::{self, Classe};
use ocinye_core::db;

/// Descreve o que esta instalação contém.
///
/// Escreve o manifesto em JSON no `stdout`, para que possa seguir por um cano
/// para um ficheiro ou para outra máquina sem tocar em disco aqui.
///
/// # Errors
///
/// Devolve erro quando a configuração está incompleta ou a base não responde.
/// Um manifesto parcial seria pior do que nenhum: comparar-se-ia contra ele e a
/// diferença passaria por igualdade.
pub async fn snapshot() -> anyhow::Result<()> {
    let config = CoreConfig::from_env().context("configuration")?;
    let pool = db::connect(&config)
        .await
        .context("a base institucional não respondeu")?;

    let manifesto = continuity::descrever(&pool)
        .await
        .context("não foi possível descrever o estado institucional")?;

    // Para `stdout`, e o resumo para `stderr`: assim `… snapshot > m.json`
    // produz um ficheiro limpo e quem o corre continua a ver o que levou.
    eprintln!("Ocinye OS — snapshot institucional");
    eprintln!("──────────────────────────────────");
    eprintln!("  formato                {}", manifesto.formato);
    eprintln!("  nível de migrations    {}", manifesto.migrations);
    for familia in &manifesto.familias {
        if familia.quantos > 0 {
            eprintln!("  {:<22} {}", familia.tabela, familia.quantos);
        }
    }
    eprintln!("  objectos guardados     {}", manifesto.objectos.len());
    eprintln!("  arestas de proveniência {}", manifesto.proveniencia.len());
    eprintln!("  eventos de auditoria   {}", manifesto.auditoria.eventos);
    eprintln!();

    // ── O que este manifesto **não** transporta ─────────────────────────
    //
    // Dito aqui, e não só na documentação, porque é aqui que alguém está a
    // fazer a migração. Um snapshot que a pessoa julgue completo é a maneira de
    // chegar ao servidor novo com a base intacta e sem a chave que a abre.
    eprintln!("  Isto descreve. Não transporta:");
    for activo in continuity::inventario() {
        if activo.classe.viaja() {
            eprintln!("    · {} — {}", activo.nome, activo.classe.as_str());
        }
    }
    eprintln!();
    eprintln!(
        "  A chave de selagem viaja por um canal próprio. Sem ela, \
         `mailbox_credentials`"
    );
    eprintln!("  chega íntegra e ilegível.");

    println!(
        "{}",
        serde_json::to_string_pretty(&manifesto).context("serializar o manifesto")?
    );
    Ok(())
}

/// Compara um manifesto com o que esta instalação contém.
///
/// Lê o manifesto do `stdin`.
///
/// # Errors
///
/// Devolve erro quando o manifesto não se lê, quando a base não responde, ou
/// quando o estado diverge. A divergência é um erro e não um aviso: um restore
/// que não trouxe o que se pensava ter trazido não está terminado.
pub async fn verify_snapshot() -> anyhow::Result<()> {
    let mut cru = String::new();
    std::io::stdin()
        .read_to_string(&mut cru)
        .context("ler o manifesto do stdin")?;
    if cru.trim().is_empty() {
        bail!(
            "não veio manifesto nenhum pelo stdin.\n\
             Uso: ocinye-core-server verify-snapshot < manifesto.json\n\
             Sem manifesto isto não é uma comparação: é uma leitura."
        );
    }

    let esperado: continuity::Manifesto =
        serde_json::from_str(&cru).context("o manifesto não se lê")?;

    let config = CoreConfig::from_env().context("configuration")?;
    let pool = db::connect(&config)
        .await
        .context("a base institucional não respondeu")?;
    let encontrado = continuity::descrever(&pool)
        .await
        .context("não foi possível descrever o estado institucional")?;

    let divergencias = continuity::comparar(&esperado, &encontrado);

    println!("Ocinye OS — verificação de continuidade");
    println!("───────────────────────────────────────");
    println!("  esperado: nível {}", esperado.migrations);
    println!("  aqui:     nível {}", encontrado.migrations);
    println!();

    if divergencias.is_empty() {
        let recursos: usize = encontrado.familias.iter().map(|f| f.quantos).sum();
        println!(
            "  {recursos} recursos institucionais, {} objectos e {} arestas de \
             proveniência",
            encontrado.objectos.len(),
            encontrado.proveniencia.len()
        );
        println!("  chegaram com as mesmas identidades.");
        println!();
        match encontrado.auditoria.primeiro.as_deref() {
            Some(instante) => {
                println!("  A auditoria mantém o seu primeiro evento em {instante}.");
            }
            None => println!("  Não há auditoria nenhuma para manter."),
        }
        return Ok(());
    }

    for divergencia in &divergencias {
        eprintln!("  {:<22} {}", divergencia.onde, divergencia.o_que);
    }
    eprintln!();
    bail!(
        "{} divergência(s). O estado institucional que chegou não é o que saiu.",
        divergencias.len()
    );
}

/// Escreve o inventário de continuidade.
///
/// Existe para que a pergunta «o que é preciso levar?» tenha uma resposta que
/// se pode correr, e não uma que se procura na documentação no dia em que o
/// servidor já ardeu.
///
/// # Errors
///
/// Não devolve erro: lê apenas a classificação, que é código.
pub fn inventory() -> anyhow::Result<()> {
    println!("Ocinye OS — inventário de continuidade");
    println!("──────────────────────────────────────");
    println!();

    let mut por_classe: std::collections::BTreeMap<Classe, Vec<_>> =
        std::collections::BTreeMap::new();
    for activo in continuity::inventario() {
        por_classe.entry(activo.classe).or_default().push(activo);
    }

    for (classe, activos) in por_classe {
        let marca = if classe.viaja() { "VIAJA" } else { "fica" };
        println!("{}  [{marca}]", classe.as_str());
        for activo in activos {
            println!("  · {} — {}", activo.nome, activo.onde);
            for linha in textwrap(activo.porque, 68) {
                println!("      {linha}");
            }
        }
        println!();
    }
    Ok(())
}

/// Quebra um parágrafo em linhas, sem cortar palavras.
fn textwrap(texto: &str, largura: usize) -> Vec<String> {
    let mut linhas = Vec::new();
    let mut actual = String::new();
    for palavra in texto.split_whitespace() {
        if !actual.is_empty() && actual.len() + 1 + palavra.len() > largura {
            linhas.push(std::mem::take(&mut actual));
        }
        if !actual.is_empty() {
            actual.push(' ');
        }
        actual.push_str(palavra);
    }
    if !actual.is_empty() {
        linhas.push(actual);
    }
    linhas
}
