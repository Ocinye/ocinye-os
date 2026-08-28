//! Liveness e prontidão institucional.
//!
//! `/health` responde se o processo está vivo. `/ready` responde se o Ocinye OS
//! pode ser entregue, e é público — a página de arranque pergunta-o antes de
//! haver sessão nenhuma.
//!
//! E nenhum dos dois pode ficar sem resposta: um probe que não devolve não é
//! prudência, é a avaria a alastrar da dependência para quem a observa.
//!
//! # Porque `/ready` devolve uma projecção e não o catálogo
//!
//! Porque isto é lido por quem ainda não se identificou. A versão anterior
//! devolvia o código do backend de armazenamento, a residência declarada e a
//! latência da base: seguro para um operador, e topologia da instalação para um
//! desconhecido. **Seguro para um membro não é o mesmo que seguro antes de
//! autenticar** (ADR-0603).

use axum::extract::{Query, State};
use axum::http::{header, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use ocinye_contracts::readiness::{
    reasons, ReadinessComponentId, ReadinessOverall, CONTRACT_VERSION,
};
use ocinye_contracts::system_capability::SystemCapabilityState;
use ocinye_core::modules::platform;
use ocinye_core::readiness;
use serde::{Deserialize, Serialize};

use crate::state::AppState;

/// Liveness response.
#[derive(Serialize)]
struct Health {
    status: &'static str,
    service: &'static str,
    version: &'static str,
}

/// A versão de contrato que quem pergunta fala.
///
/// Opcional: uma sonda de operação — `curl`, um verificador de infraestrutura —
/// não fala contrato nenhum, e não deve receber `Blocked` por isso. Quando vem,
/// o Core compara, e é **o Core** que decide se são compatíveis. O Workspace
/// não reclassifica nada: só a sua própria versão é que ele conhece, e é essa
/// que envia.
#[derive(Deserialize)]
struct ReadinessQuery {
    contract: Option<u32>,
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/health", get(health))
        .route("/ready", get(ready))
}

async fn health() -> Json<Health> {
    Json(Health {
        status: "ok",
        service: "ocinye-core",
        version: env!("CARGO_PKG_VERSION"),
    })
}

/// `GET /ready`
///
/// Público, barato, sem escrita, e sem `cache`.
///
/// # Porque `no-store`
///
/// Porque prontidão é informação operacional que muda. Um intermediário que
/// guardasse um `READY` de há cinco minutos entregaria o sistema com o núcleo em
/// baixo, e a pessoa descobri-lo-ia depois de escrever a palavra-passe.
async fn ready(State(state): State<AppState>, Query(query): Query<ReadinessQuery>) -> Response {
    // O catálogo canónico. Sem ele — porque a base não responde — os módulos
    // opcionais ficam indisponíveis, e não disponíveis por omissão.
    let capabilities = platform::system_capabilities(
        &state.pool,
        &state.config,
        state.store.is_some(),
        state.mail_registry.reachability().await,
    )
    .await
    .ok();

    let mut snapshot =
        readiness::public_snapshot(&state.pool, capabilities.as_ref(), Some(&state.realtime)).await;

    // Compatibilidade: um Core saudável que fala outro contrato não é um sistema
    // pronto. Dizê-lo aqui evita rebentar mais tarde num erro de desserialização
    // que ninguém consegue ler.
    if let Some(pedido) = query.contract {
        if pedido != CONTRACT_VERSION {
            for componente in &mut snapshot.components {
                if componente.component == ReadinessComponentId::Compatibility {
                    componente.state = SystemCapabilityState::Unavailable;
                    componente.reason = reasons::INCOMPATIBLE.to_owned();
                }
            }
            snapshot.overall = ReadinessOverall::Blocked;
        }
    }

    // `Blocked` responde 503: o cliente tem de conseguir distinguir «o Core
    // disse-me que não está pronto» de «não consegui falar com o Core». Ambos
    // impedem o arranque, e são diagnósticos diferentes.
    let status = if snapshot.overall.may_proceed() {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };

    let mut resposta = (status, Json(snapshot)).into_response();
    resposta.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("no-store, no-cache, must-revalidate"),
    );
    resposta
}

/// Todos os campos que a projecção pública pode conter.
///
/// Serve de lista de autorização executável: um teste percorre o JSON emitido e
/// recusa qualquer chave que não esteja aqui. Acrescentar um campo ao que é
/// público passa a exigir mexer nesta lista.
#[cfg(test)]
pub(crate) const PUBLIC_FIELDS: &[&str] = &[
    "overall",
    "contract_version",
    "components",
    "component",
    "state",
    "criticality",
    "reason",
];

#[cfg(test)]
mod tests {
    use super::*;
    use ocinye_contracts::readiness::{Criticality, PublicReadiness, ReadinessComponent};

    /// Nada fora da lista sai para o mundo.
    ///
    /// # Porque isto percorre o JSON e não os tipos
    ///
    /// Porque é o JSON que atravessa a rede. Um campo acrescentado a uma
    /// estrutura aninhada aparece aqui mesmo que ninguém se lembre dele, e é
    /// esse esquecimento que este teste existe para apanhar.
    #[test]
    fn a_projeccao_publica_nao_tem_campos_fora_da_lista() {
        let snapshot = PublicReadiness {
            overall: ReadinessOverall::Degraded,
            contract_version: CONTRACT_VERSION,
            components: vec![ReadinessComponent {
                component: ReadinessComponentId::Mail,
                state: SystemCapabilityState::NotConfigured,
                criticality: Criticality::Optional,
                reason: reasons::NOT_CONFIGURED.to_owned(),
            }],
        };

        fn chaves(valor: &serde_json::Value, encontradas: &mut Vec<String>) {
            match valor {
                serde_json::Value::Object(mapa) => {
                    for (chave, dentro) in mapa {
                        encontradas.push(chave.clone());
                        chaves(dentro, encontradas);
                    }
                }
                serde_json::Value::Array(itens) => {
                    for item in itens {
                        chaves(item, encontradas);
                    }
                }
                _ => {}
            }
        }

        let json = serde_json::to_value(&snapshot).expect("serializa");
        let mut encontradas = Vec::new();
        chaves(&json, &mut encontradas);

        assert!(!encontradas.is_empty(), "não se leu campo nenhum");
        for chave in encontradas {
            assert!(
                PUBLIC_FIELDS.contains(&chave.as_str()),
                "«{chave}» chegou à projecção pública sem estar na lista de autorização"
            );
        }
    }

    /// Nenhum valor emitido revela infraestrutura.
    #[test]
    fn nenhum_valor_publico_nomeia_infraestrutura() {
        let proibidos = [
            "postgres",
            "minio",
            "localhost",
            "127.0.0.1",
            "s3",
            "smtp",
            "imap",
            ":5432",
            ":9000",
            "local-minio",
            "/Users/",
            "internal",
            "amazonaws",
        ];

        for razao in reasons::all() {
            let baixo = razao.to_lowercase();
            for proibido in proibidos {
                assert!(
                    !baixo.contains(&proibido.to_lowercase()),
                    "a frase pública «{razao}» contém «{proibido}»"
                );
            }
        }
    }
}
