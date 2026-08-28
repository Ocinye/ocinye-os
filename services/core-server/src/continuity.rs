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
use ocinye_core::continuity::{self, Classe, Legibilidade, Veredicto};
use ocinye_core::db;
use ocinye_core::password::sealed::{self, Sealed};
use ocinye_core::storage::ObjectStore;
use uuid::Uuid;

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

/// Verifica os bytes, e não a linha que aponta para eles.
///
/// # Porque isto é um comando separado
///
/// Porque `verify-snapshot` compara o **registo** dos objectos: identidades,
/// chaves e as somas que a base guarda. Provar que esses números coincidem dos
/// dois lados prova que o `pg_dump` chegou inteiro — não prova que existe um
/// único byte no Object Storage do servidor novo.
///
/// > **Uma soma que nunca foi recalculada não é evidência de integridade.**
///
/// Aqui cada objecto é lido do bucket e a sua soma recalculada. É caro de
/// propósito: é a única forma de a resposta significar alguma coisa.
///
/// # Custo
///
/// Lê todos os bytes do Object Storage. Não há amostragem: uma amostra que
/// passasse diria «verificado» sobre o que não se leu, e é precisamente o
/// objecto não lido que costuma faltar.
///
/// # Errors
///
/// Devolve erro quando a base não responde, quando o Object Storage não está
/// configurado — verificar sem alvo não é passar, é não ter corrido — ou
/// quando algum objecto falta, não se lê, ou chega com outro conteúdo.
pub async fn verify_objects() -> anyhow::Result<()> {
    let config = CoreConfig::from_env().context("configuration")?;
    let pool = db::connect(&config)
        .await
        .context("a base institucional não respondeu")?;

    // Sem loja configurada isto é `NOT_RUN`, e nunca `PASS`. Um verificador que
    // não encontrou o que devia observar não teve sucesso: observou zero.
    let Some(loja) = ObjectStore::new(config.storage.clone()) else {
        bail!(
            "o Object Storage não está configurado nesta instalação.\n\
             Não há nada para verificar, e isso não é o mesmo que estar certo:\n\
             metade do estado autoritativo não foi observada."
        );
    };

    // ── Antes de concluir seja o que for, provar que há quem responda ──
    //
    // `get` devolve o mesmo erro para «o objecto não existe» e para «o serviço
    // não atende». Sem esta sonda, um MinIO em baixo produzia o relatório mais
    // alarmante que este comando sabe escrever — trezentos objectos em falta —
    // quando o que houve foi um verificador que não observou nada.
    //
    // `INVALID` não é `FAIL`. Um verificador que não encontrou o que devia
    // observar não teve sucesso nem descobriu um problema: não correu.
    let saude = loja.health().await;
    if saude.status != "ok" {
        bail!(
            "o Object Storage respondeu «{}».\n\
             Nada foi verificado. Isto não é a mesma coisa que os objectos \
             faltarem:\n\
             é não se ter conseguido olhar para eles.",
            saude.status
        );
    }

    let objectos: Vec<(Uuid, String, String, i64)> = sqlx::query_as(
        "SELECT id, object_key, checksum_sha256, size_bytes
           FROM storage_objects ORDER BY id",
    )
    .fetch_all(&pool)
    .await
    .context("ler os objectos registados")?;

    println!("Ocinye OS — verificação dos bytes");
    println!("─────────────────────────────────");
    println!("  bucket        {}", loja.bucket());
    println!("  objectos      {}", objectos.len());
    println!();

    if objectos.is_empty() {
        println!("  Não há objectos registados. Nada foi verificado, e é isso");
        println!("  que esta linha diz — não que esteja tudo bem.");
        return Ok(());
    }

    let mut faltam = Vec::new();
    let mut diferem = Vec::new();
    let mut bytes_lidos: u64 = 0;
    // Contado entre os que **chegaram a ser lidos**, e não sobre a tabela: um
    // objecto em falta não é um objecto sem soma, e juntá-los faria a linha do
    // resumo dizer que se leram trezentos e três coisas que não existem.
    let mut sem_soma: usize = 0;

    for (id, chave, soma_registada, tamanho) in &objectos {
        match loja.get(chave).await {
            Err(_) => faltam.push((*id, chave.clone())),
            Ok(conteudo) => {
                bytes_lidos += conteudo.len() as u64;
                match continuity::conferir(soma_registada, *tamanho, &conteudo) {
                    Veredicto::Igual => {}
                    Veredicto::SemSoma => sem_soma += 1,
                    Veredicto::OutroConteudo { esperada, obtida } => {
                        diferem.push((*id, chave.clone(), esperada, obtida));
                    }
                    Veredicto::OutroTamanho { esperados, obtidos } => diferem.push((
                        *id,
                        chave.clone(),
                        format!("{esperados} bytes"),
                        format!("{obtidos} bytes"),
                    )),
                }
            }
        }
    }

    let lidos = objectos.len() - faltam.len();
    println!("  lidos         {lidos} objecto(s), {bytes_lidos} bytes");
    if sem_soma > 0 {
        println!(
            "  sem soma      {sem_soma} desses foram lidos e não puderam ser \
             comparados:"
        );
        println!("                a base não guardou soma para eles.");
    }
    println!();

    // ── O corte é dito, e não silencioso ────────────────────────────────
    //
    // Uma lista de milhares de linhas não se lê, e uma lista cortada sem o
    // dizer faz o problema parecer menor do que é.
    const MOSTRA: usize = 20;
    for (id, chave) in faltam.iter().take(MOSTRA) {
        eprintln!("  em falta      {id}  «{chave}»");
    }
    if faltam.len() > MOSTRA {
        eprintln!(
            "  em falta      … e mais {}, não listados aqui",
            faltam.len() - MOSTRA
        );
    }
    for (id, chave, esperada, obtida) in diferem.iter().take(MOSTRA) {
        eprintln!("  outro conteúdo {id}  «{chave}»");
        eprintln!(
            "                esperava {}…, leu {}…",
            &esperada[..esperada.len().min(12)],
            &obtida[..obtida.len().min(12)]
        );
    }
    if diferem.len() > MOSTRA {
        eprintln!(
            "  outro conteúdo … e mais {}, não listados aqui",
            diferem.len() - MOSTRA
        );
    }

    if faltam.is_empty() && diferem.is_empty() {
        if sem_soma == objectos.len() {
            println!("  Nenhuma soma pôde ser comparada. Os objectos existem no");
            println!("  bucket; que sejam os certos não foi verificado por nada.");
            return Ok(());
        }
        println!(
            "  {} objecto(s) foram lidos do bucket e as suas somas recalculadas.",
            objectos.len() - sem_soma
        );
        println!("  Os bytes que a instituição cita são os bytes que ela guardou.");
        return Ok(());
    }

    eprintln!();
    bail!(
        "{} objecto(s) em falta e {} com outro conteúdo. A base viajou; \
         os bytes não.",
        faltam.len(),
        diferem.len()
    );
}

/// Prova que a chave que chegou abre o que chegou.
///
/// # Porque isto é uma terceira pergunta
///
/// `verify-snapshot` prova que as **linhas** chegaram. `verify-objects` prova
/// que os **bytes** chegaram. Nenhum dos dois prova que o que chegou é
/// **legível**.
///
/// `mailbox_credentials` é o caso: as linhas viajam no dump, íntegras, com o
/// nonce e o criptograma certos, e um restore sem a chave de selagem passa nos
/// dois verificadores anteriores. A instituição só descobre a falta quando um
/// membro tenta ver o correio, semanas depois, e a mensagem que recebe fala de
/// uma caixa que não liga.
///
/// > **Um restore que passa estruturalmente e entrega estado ilegível é a pior
/// > forma de falhar, porque parece sucesso.**
///
/// # O que este comando não faz
///
/// Não imprime nenhuma senha, nem parte de nenhuma. Abrir e descartar é o
/// suficiente: o que se quer saber é se a etiqueta de autenticação valida, e
/// isso é um sim ou um não.
///
/// # Errors
///
/// Devolve erro quando a base não responde, quando há estado selado e a chave
/// não está configurada, ou quando alguma linha se recusa a abrir.
pub async fn verify_keys() -> anyhow::Result<()> {
    let config = CoreConfig::from_env().context("configuration")?;
    let pool = db::connect(&config)
        .await
        .context("a base institucional não respondeu")?;

    println!("Ocinye OS — verificação do material criptográfico");
    println!("─────────────────────────────────────────────────");
    println!();
    for material in continuity::viaja_por_canal_proprio() {
        println!("  {}  [{}]", material.variavel, material.destino.as_str());
        if let Some(estado) = material.interpreta {
            println!("      interpreta  {estado}");
        }
    }
    println!();

    let seladas: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM mailbox_credentials")
        .fetch_one(&pool)
        .await
        .context("contar as credenciais seladas")?;

    // A leitura das linhas só acontece quando há chave; sem ela não há nada a
    // tentar, e a decisão é a mesma.
    let mut recusadas: Vec<Uuid> = Vec::new();
    if let Some(chave) = config.mail.sealing_key.as_ref() {
        let linhas: Vec<(Uuid, Vec<u8>, Vec<u8>)> =
            sqlx::query_as("SELECT mailbox_id, nonce, ciphertext FROM mailbox_credentials")
                .fetch_all(&pool)
                .await
                .context("ler as credenciais seladas")?;

        // Todas, e não uma amostra. Uma amostra que abrisse diria «legível»
        // sobre o que não se leu, e a linha que não se leu é a que costuma ter
        // sido selada com a chave anterior.
        for (caixa, nonce, ciphertext) in &linhas {
            let fechado = Sealed {
                nonce: nonce.clone(),
                ciphertext: ciphertext.clone(),
            };
            // O texto em claro é descartado imediatamente. O que se guarda é o
            // veredicto: nenhuma senha, nem parte de nenhuma, chega ao ecrã.
            if sealed::open(chave, &fechado).is_err() {
                recusadas.push(*caixa);
            }
        }
    }

    match continuity::legibilidade(config.mail.sealing_key.is_some(), seladas, recusadas.len()) {
        Legibilidade::NadaParaLer => {
            println!("  Não há estado selado, e não há chave. Nada foi verificado,");
            println!("  e é isso que esta linha diz — não que esteja tudo bem.");
            Ok(())
        }
        Legibilidade::ChaveSemEstado => {
            println!("  Há chave configurada e nenhum estado selado para abrir.");
            println!("  Nada foi verificado. A chave continua a ser precisa quando houver.");
            Ok(())
        }
        Legibilidade::IlegivelSemChave { seladas } => bail!(
            "há {seladas} credencial(is) selada(s) nesta base e nenhuma \
             `OCINYE_MAIL_KEY` configurada.\n\
             O estado chegou íntegro e ilegível: o dump trouxe o nonce e o \
             criptograma,\n\
             e a chave que os interpreta não estava lá dentro — nem devia \
             estar.\n\
             Nenhuma chave nova reconstrói isto. Recupere a original."
        ),
        Legibilidade::Legivel { abriram } => {
            println!("  {abriram} credencial(is) selada(s) abriram com a chave desta instalação.");
            println!("  O que chegou não é apenas íntegro: é interpretável.");
            Ok(())
        }
        Legibilidade::Ilegivel {
            recusadas: n,
            total,
        } => {
            const MOSTRA: usize = 20;
            for caixa in recusadas.iter().take(MOSTRA) {
                eprintln!("  não abre     caixa {caixa}");
            }
            if n > MOSTRA {
                eprintln!("  não abre     … e mais {}, não listadas aqui", n - MOSTRA);
            }
            eprintln!();
            bail!(
                "{n} de {total} credencial(is) não abriram. A chave desta \
                 instalação não é a que as selou."
            )
        }
    }
}
