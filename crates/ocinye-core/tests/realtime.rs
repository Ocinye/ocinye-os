//! O plano realtime, contra um Redis a sério.
//!
//! # Porque estes testes precisam de Redis
//!
//! Porque o que se mede aqui é comportamento de TTL, de `pub/sub` e de duas
//! ligações a falarem uma com a outra. Um duplo em memória provaria que o meu
//! duplo funciona.
//!
//! Sem `OCINYE_TEST_REDIS_URL`, cada teste sai por onde entrou — e o contrato
//! de enumeração conta-os como ignorados em vez de os dar por passados.

use std::time::Duration;

use ocinye_core::realtime::events::{Channel, ServerEvent};
use ocinye_core::realtime::presence::{Presence, TYPING_TTL_SECONDS};
use ocinye_core::realtime::Realtime;
use uuid::Uuid;

/// Liga-se ao Redis de teste, ou desiste.
async fn plano() -> Option<Realtime> {
    let url = std::env::var("OCINYE_TEST_REDIS_URL").ok()?;
    let plano = Realtime::connect(&url).await;
    if !plano.saudavel() {
        // Distinguir «não configurado» de «configurado e em baixo». O segundo é
        // uma falha de infraestrutura, e um teste que a engolisse em silêncio
        // reportaria verde sem ter medido nada.
        panic!("OCINYE_TEST_REDIS_URL está definida mas o Redis não responde");
    }
    Some(plano)
}

/// Espera até que uma condição se verifique, ou desiste.
///
/// Existe para não pôr `sleep`s fixos nos testes: um `sleep` longo torna a
/// suite lenta, e um curto torna-a intermitente.
async fn ate<F, Fut>(limite: Duration, mut condicao: F) -> bool
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = bool>,
{
    let fim = std::time::Instant::now() + limite;
    loop {
        if condicao().await {
            return true;
        }
        if std::time::Instant::now() >= fim {
            return false;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
}

#[tokio::test]
async fn uma_mensagem_publicada_chega_a_quem_escuta() {
    let Some(plano) = plano().await else { return };
    let Some(mut escuta) = plano.escutar().await else {
        panic!("o Redis respondeu mas não abriu uma escuta");
    };

    let canal = Channel::Conversation { id: Uuid::new_v4() };
    assert!(escuta.subscrever(canal).await);

    // O `subscribe` do Redis é assíncrono: publicar já a seguir corre o risco
    // de o servidor ainda não ter registado a inscrição. Publica-se até chegar.
    let publicador = async {
        for _ in 0..40 {
            plano
                .publish(
                    canal,
                    &ServerEvent::ConversationUpdated {
                        conversation_id: match canal {
                            Channel::Conversation { id } => id,
                            Channel::Person { id } => id,
                        },
                    },
                )
                .await;
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    };

    let recebido = tokio::select! {
        () = publicador => None,
        entrega = escuta.proxima() => entrega,
    };

    let (recebido_canal, carga) = recebido.expect("nada chegou pelo plano realtime");
    assert_eq!(recebido_canal, canal);
    assert!(carga.contains("conversation_updated"), "carga: {carga}");
}

#[tokio::test]
async fn quem_nao_subscreveu_nao_recebe() {
    let Some(plano) = plano().await else { return };
    let Some(mut escuta) = plano.escutar().await else {
        panic!("o Redis respondeu mas não abriu uma escuta");
    };

    let meu = Channel::Conversation { id: Uuid::new_v4() };
    let alheio = Channel::Conversation { id: Uuid::new_v4() };
    assert!(escuta.subscrever(meu).await);

    for _ in 0..10 {
        plano
            .publish(alheio, &ServerEvent::RealtimeDegraded { activo: true })
            .await;
    }

    // Nada deve chegar. Um limite curto porque a asserção é a **ausência**, e
    // esperar mais só torna a suite lenta sem a tornar mais forte.
    let entrega = tokio::time::timeout(Duration::from_millis(300), escuta.proxima()).await;
    assert!(
        entrega.is_err(),
        "chegou um evento de um canal que esta escuta nunca pediu: {entrega:?}"
    );
}

#[tokio::test]
async fn dois_planos_falam_um_com_o_outro() {
    // É esta a propriedade que o Redis existe para dar, e a razão de não ter
    // ficado por um `broadcast` em processo: dois Cores, e uma mensagem
    // enviada num tem de chegar ao outro (ADR-0012).
    let Some(a) = plano().await else { return };
    let Some(b) = plano().await else { return };

    let Some(mut escuta) = b.escutar().await else {
        panic!("o segundo plano não abriu escuta");
    };
    let canal = Channel::Person { id: Uuid::new_v4() };
    assert!(escuta.subscrever(canal).await);

    let publicador = async {
        for _ in 0..40 {
            a.publish(canal, &ServerEvent::RealtimeDegraded { activo: true })
                .await;
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    };

    let recebido = tokio::select! {
        () = publicador => None,
        entrega = escuta.proxima() => entrega,
    };
    assert!(
        recebido.is_some(),
        "o que um plano publicou não chegou ao outro"
    );
}

#[tokio::test]
async fn tres_ligacoes_da_mesma_pessoa_sao_uma_presenca() {
    let Some(plano) = plano().await else { return };
    let pessoa = Uuid::new_v4();

    let ligacoes: Vec<Uuid> = (0..3).map(|_| Uuid::new_v4()).collect();
    for ligacao in &ligacoes {
        plano.batimento(pessoa, *ligacao, true).await;
    }

    assert!(plano.sinais(pessoa).await.ligado);

    // Fechar dois separadores não apaga a pessoa: o terceiro continua aberto.
    plano.largar(pessoa, ligacoes[0]).await;
    plano.largar(pessoa, ligacoes[1]).await;
    assert!(
        plano.sinais(pessoa).await.ligado,
        "fechar dois separadores de três pôs a pessoa offline"
    );

    plano.largar(pessoa, ligacoes[2]).await;
    assert!(
        !plano.sinais(pessoa).await.ligado,
        "fechar o último separador devia pôr a pessoa offline"
    );
}

#[tokio::test]
async fn o_typing_expira_sozinho() {
    // Um browser que fecha, uma rede que cai ou um portátil que adormece não
    // mandam aviso nenhum. É o TTL que faz o `typing` desaparecer, e não um
    // adeus educado (ADR-0012 §7).
    let Some(plano) = plano().await else { return };
    let conversa = Uuid::new_v4();
    let pessoa = Uuid::new_v4();

    plano.a_escrever(conversa, pessoa, true).await;
    assert!(
        plano.quem_escreve(conversa).await.contains(&pessoa),
        "quem começou a escrever não apareceu"
    );

    // Ninguém diz que parou. Só se espera.
    let limite = Duration::from_secs(TYPING_TTL_SECONDS + 3);
    let expirou = ate(limite, || async {
        !plano.quem_escreve(conversa).await.contains(&pessoa)
    })
    .await;

    assert!(
        expirou,
        "o `typing` continuou {}s depois do TTL de {TYPING_TTL_SECONDS}s",
        limite.as_secs()
    );
}

#[tokio::test]
async fn parar_de_escrever_e_imediato() {
    let Some(plano) = plano().await else { return };
    let conversa = Uuid::new_v4();
    let pessoa = Uuid::new_v4();

    plano.a_escrever(conversa, pessoa, true).await;
    plano.a_escrever(conversa, pessoa, false).await;

    assert!(
        !plano.quem_escreve(conversa).await.contains(&pessoa),
        "quem parou de escrever teve de esperar pelo TTL"
    );
}

#[tokio::test]
async fn uma_declaracao_nao_expira_com_a_presenca() {
    // Quem se pôs em «Não incomodar» pediu uma coisa. Voltar a «Disponível»
    // porque passaram quarenta e cinco segundos seria ignorar o pedido.
    let Some(plano) = plano().await else { return };
    let pessoa = Uuid::new_v4();

    plano.declarar(pessoa, Some(Presence::NaoIncomodar)).await;
    let sinais = plano.sinais(pessoa).await;
    assert_eq!(sinais.declarado, Some(Presence::NaoIncomodar));

    plano.declarar(pessoa, None).await;
    assert_eq!(plano.sinais(pessoa).await.declarado, None);
}

#[tokio::test]
async fn o_typing_de_uma_conversa_nao_aparece_noutra() {
    let Some(plano) = plano().await else { return };
    let aqui = Uuid::new_v4();
    let ali = Uuid::new_v4();
    let pessoa = Uuid::new_v4();

    plano.a_escrever(aqui, pessoa, true).await;

    assert!(plano.quem_escreve(aqui).await.contains(&pessoa));
    assert!(
        plano.quem_escreve(ali).await.is_empty(),
        "o `typing` atravessou para outra conversa"
    );
}
