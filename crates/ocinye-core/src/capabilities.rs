//! Onde o Core fala com o Capability Runtime.
//!
//! # A fronteira, num sítio só
//!
//! Este é o único módulo do Core que conhece `ocinye-capabilities`. Tudo o que
//! é Wasmtime, WASI, combustível, épocas e manifestos termina aqui; o que sai
//! daqui para o resto do Core são tipos do Ocinye OS e erros do Ocinye OS.
//!
//! A razão é a mesma pela qual os formatos de um fornecedor de inferência
//! terminam no adaptador: se metade do Core souber falar WebAssembly, trocar de
//! runtime deixa de ser uma decisão e passa a ser uma reescrita.
//!
//! # O Runtime executa. O Core decide.
//!
//! O que corre lá dentro é código isolado — sem rede, sem sistema de ficheiros,
//! sem ambiente, sem base de dados, com combustível e tempo contados. O que ele
//! devolve é **matéria-prima**: só depois de o Core a validar e a interpretar é
//! que aquilo significa alguma coisa para a instituição.
//!
//! # Qual componente corre, e quem escolhe
//!
//! O Core. Cada operação nomeia o seu componente por uma constante deste
//! módulo, e não há caminho por onde um pedido — de uma pessoa, de um agente ou
//! da API — escolha o que se executa. Um endpoint que aceitasse o nome de um
//! componente seria um executor de código arbitrário com outro nome.

use std::path::PathBuf;
use std::sync::Arc;

use ocinye_capabilities::{CapabilityError, CapabilityRuntime, Invocation, Manifest};

use crate::error::{CoreError, CoreResult};

/// Um componente que o Core sabe executar.
///
/// O identificador é o nome do ficheiro sem extensão, e é uma constante desta
/// enumeração fechada. Não existe construtor a partir de texto: é essa ausência
/// que impede um pedido de nomear o que quer correr.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Component {
    /// Lê bibliografia BibTeX e devolve os registos que conseguiu ler.
    BibtexImport,
}

impl Component {
    /// O nome do ficheiro do componente, sem extensão.
    #[must_use]
    const fn nome(self) -> &'static str {
        match self {
            Self::BibtexImport => "bibtex-import",
        }
    }

    /// O manifesto do componente, embebido no binário.
    ///
    /// # Porque não se lê de disco
    ///
    /// Duas razões, e a segunda é a que importa.
    ///
    /// A primeira é prática: um caminho relativo em código de biblioteca é uma
    /// dependência escondida do directório onde o processo arrancou. Escrevi-o
    /// assim primeiro, e os testes falharam todos — o binário de teste corre a
    /// partir do directório do crate, não da raiz do repositório.
    ///
    /// A segunda é a que fica: o manifesto **é** a política. Declara o
    /// combustível, o tempo, a memória, e que este componente não pede rede nem
    /// sistema de ficheiros. Um manifesto em disco é uma política que se edita
    /// num servidor; embebido, viaja com o Core que o aplica e não pode ser
    /// alargado sem recompilar.
    #[must_use]
    const fn manifesto(self) -> &'static str {
        match self {
            Self::BibtexImport => {
                include_str!("../../../wasm/capabilities/bibtex-import/manifest.json")
            }
        }
    }
}

/// O Capability Runtime, com os componentes já carregados.
///
/// # Porque se carrega uma vez
///
/// Ler e compilar um módulo WebAssembly custa; fazê-lo a cada pedido punha esse
/// custo no caminho de quem espera. E há uma razão melhor: um componente lido no
/// arranque é o mesmo durante toda a vida do processo. Se o ficheiro em disco
/// mudar por baixo, não é a meio de um pedido que se descobre.
pub struct Capabilities {
    runtime: Arc<CapabilityRuntime>,
    bibtex: Option<Arc<Carregado>>,
}

/// Um componente lido, com o seu manifesto.
struct Carregado {
    manifest: Manifest,
    bytes: Vec<u8>,
}

impl Capabilities {
    /// Lê os componentes que o Core sabe executar.
    ///
    /// # Um componente em falta não impede o arranque
    ///
    /// O Ocinye Core arranca com zero fornecedores de IA e zero nós de
    /// computação; arranca também sem os componentes construídos. A operação
    /// que precisar de um recusa com uma razão que se lê, em vez de o processo
    /// não subir — que é a diferença entre uma capacidade indisponível e uma
    /// instalação partida.
    ///
    /// # Errors
    ///
    /// Devolve erro apenas quando o próprio motor não pode ser construído.
    pub fn load(components_dir: &str) -> CoreResult<Self> {
        let runtime = CapabilityRuntime::new().map_err(traduzir)?;
        let bibtex = ler(components_dir, Component::BibtexImport);

        match &bibtex {
            Some(_) => tracing::info!(
                component = Component::BibtexImport.nome(),
                "capability component loaded"
            ),
            None => tracing::warn!(
                component = Component::BibtexImport.nome(),
                directory = components_dir,
                "capability component is not built; operations that need it will refuse"
            ),
        }

        Ok(Self {
            runtime: Arc::new(runtime),
            bibtex,
        })
    }

    /// Um conjunto vazio, para contextos que não executam capacidades.
    ///
    /// # Errors
    ///
    /// Devolve erro quando o motor não pode ser construído.
    pub fn empty() -> CoreResult<Self> {
        Ok(Self {
            runtime: Arc::new(CapabilityRuntime::new().map_err(traduzir)?),
            bibtex: None,
        })
    }

    /// Se um componente está disponível.
    #[must_use]
    pub const fn has(&self, component: Component) -> bool {
        match component {
            Component::BibtexImport => self.bibtex.is_some(),
        }
    }

    /// Executa um componente sobre uma entrada.
    ///
    /// # O que quem chama recebe
    ///
    /// Os bytes que o componente escreveu, e nada mais. Nem diagnósticos do
    /// motor, nem caminhos, nem pilhas: um erro do Runtime chega traduzido para
    /// a linguagem do Core, e o detalhe fica no log de quem opera.
    ///
    /// # Errors
    ///
    /// - [`CoreError::CapabilityUnavailable`] quando o componente não está construído.
    /// - [`CoreError::Validation`] quando a entrada excede o que o componente
    ///   declara aceitar.
    /// - [`CoreError::Internal`] quando a execução falha, excede os limites ou
    ///   o componente não cumpre o seu contrato.
    pub async fn run(&self, component: Component, input: Vec<u8>) -> CoreResult<Vec<u8>> {
        let carregado = match component {
            Component::BibtexImport => self.bibtex.clone(),
        }
        .ok_or_else(|| {
            CoreError::CapabilityUnavailable(
                "Esta capacidade não está disponível nesta instalação.".to_owned(),
            )
        })?;

        // Numa thread de bloqueio, e não na que serve pedidos.
        //
        // Executar WebAssembly é trabalho de CPU síncrono, e o adaptador WASI
        // usa `block_on` por dentro: chamado a partir de uma tarefa assíncrona,
        // entra em pânico com «cannot start a runtime from within a runtime».
        // Descobri-o da melhor maneira possível — os testes de integração a
        // dizerem-no em voz alta antes de isto sair daqui.
        //
        // E há a razão que fica depois de o pânico desaparecer: uma capacidade
        // pode consumir o seu tempo todo, e uma execução longa numa thread de
        // trabalho do Tokio pára os outros pedidos que essa thread serve. O
        // combustível e a época limitam o que corre; isto limita a quem custa.
        let runtime = Arc::clone(&self.runtime);
        let resultado = tokio::task::spawn_blocking(move || {
            runtime.run(Invocation {
                manifest: &carregado.manifest,
                component: &carregado.bytes,
                input,
            })
        })
        .await
        .map_err(|_| CoreError::Internal("A capacidade não completou a execução.".to_owned()))?;

        match resultado {
            Ok(outcome) => {
                tracing::debug!(
                    component = component.nome(),
                    duration_ms = outcome.duration.as_millis(),
                    fuel_used = outcome.fuel_used,
                    output_bytes = outcome.output.len(),
                    "capability executed"
                );
                Ok(outcome.output)
            }
            Err(erro) => {
                // O detalhe fica aqui, para quem opera. Quem chamou recebe uma
                // categoria: dizer-lhe qual foi o trap é desenhar-lhe o mapa.
                tracing::warn!(
                    component = component.nome(),
                    error = %erro,
                    "capability execution failed"
                );
                Err(traduzir(erro))
            }
        }
    }
}

/// Lê o componente de disco e junta-lhe o manifesto embebido.
///
/// O `.wasm` vem de fora porque é grande e é construído à parte; a política vem
/// de dentro porque é uma decisão do Core.
fn ler(components_dir: &str, component: Component) -> Option<Arc<Carregado>> {
    let wasm = PathBuf::from(components_dir).join(format!("{}.wasm", component.nome()));
    let bytes = std::fs::read(&wasm).ok()?;

    // Um manifesto embebido que não fosse válido seria um defeito de compilação
    // à espera de acontecer em produção. Falha aqui, no arranque, e alto.
    let manifest =
        Manifest::parse(component.manifesto()).expect("o manifesto embebido tem de ser válido");

    Some(Arc::new(Carregado { manifest, bytes }))
}

/// Traduz uma falha do Runtime para a linguagem do Core.
///
/// # Categorias, e não mensagens
///
/// Quem chama precisa de distinguir «esta capacidade não está aqui» de «isto
/// correu e rebentou». Não precisa de saber qual instrução fez trap, nem em que
/// caminho estava o módulo, e dar-lhe isso é oferecer reconhecimento a quem
/// esteja a sondar a fronteira.
fn traduzir(erro: CapabilityError) -> CoreError {
    match erro {
        CapabilityError::InvalidManifest(_) | CapabilityError::Load(_) => {
            CoreError::CapabilityUnavailable(
                "Esta capacidade não está disponível nesta instalação.".to_owned(),
            )
        }
        CapabilityError::ResourceExhausted(_) => CoreError::Validation(
            "O conteúdo é demasiado extenso ou complexo para ser processado.".to_owned(),
        ),
        CapabilityError::PermissionDenied(_)
        | CapabilityError::Execution(_)
        | CapabilityError::Contract(_) => {
            CoreError::Internal("A capacidade não completou a execução.".to_owned())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Um componente que não existe não impede o Core de arrancar.
    #[test]
    fn um_componente_em_falta_nao_impede_o_arranque() {
        let capacidades =
            Capabilities::load("/um/directorio/que/nao/existe").expect("o motor constrói-se");
        assert!(!capacidades.has(Component::BibtexImport));
    }

    /// E a operação que precisa dele recusa com uma razão que se lê.
    #[tokio::test]
    async fn sem_componente_a_execucao_recusa_em_vez_de_rebentar() {
        let capacidades = Capabilities::empty().expect("o motor constrói-se");
        let erro = capacidades
            .run(
                Component::BibtexImport,
                b"@article{k, title = {T}}".to_vec(),
            )
            .await
            .expect_err("sem componente não há execução");

        assert!(
            matches!(erro, CoreError::CapabilityUnavailable(_)),
            "esperava-se indisponível, veio {erro:?}"
        );
    }

    /// A tradução dá a categoria certa, e nunca a mensagem do motor.
    ///
    /// # O que este teste aprendeu a medir
    ///
    /// Media a coisa errada. Comparava `to_string()` do erro público com o do
    /// erro interno, e nunca podia falhar: `CoreError::Internal` tem
    /// `#[error("internal error")]` e **não mostra o que lhe puseram dentro**.
    /// Escrevi uma reversão a fazer o `traduzir` reencaminhar a mensagem crua,
    /// e o teste passou na mesma — porque a mensagem crua ia para um sítio que
    /// ninguém imprime.
    ///
    /// O que se mede agora são as duas coisas verdadeiras: a categoria, que é o
    /// que quem chama precisa de distinguir; e o texto das categorias que
    /// **são** mostradas, que tem de ser uma frase institucional e não o que o
    /// motor disse.
    #[test]
    fn a_traducao_da_a_categoria_certa_e_nunca_a_mensagem_do_motor() {
        let motor = "wasm trap: unreachable at 0x4f2 in /var/folders/xy/bibtex.wasm";

        // Indisponível: o componente não abre.
        let publico = traduzir(CapabilityError::Load(motor.to_owned()));
        assert!(matches!(publico, CoreError::CapabilityUnavailable(_)));
        sem_vestigios(&publico.to_string(), motor);

        // Recusa: pediu-se demais.
        let publico = traduzir(CapabilityError::ResourceExhausted(motor.to_owned()));
        assert!(matches!(publico, CoreError::Validation(_)));
        sem_vestigios(&publico.to_string(), motor);

        // Falhou a correr: interno, e sem detalhe.
        for interno in [
            CapabilityError::Execution(motor.to_owned()),
            CapabilityError::Contract(motor.to_owned()),
            CapabilityError::PermissionDenied(motor.to_owned()),
        ] {
            let publico = traduzir(interno);
            assert!(matches!(publico, CoreError::Internal(_)));
            sem_vestigios(&publico.to_string(), motor);
        }
    }

    /// Nenhum fragmento do que o motor disse sobrevive no texto público.
    fn sem_vestigios(publico: &str, motor: &str) {
        assert_ne!(publico, motor, "a mensagem passou tal e qual");
        for fragmento in ["wasm", "trap", "0x", "/var/", "unreachable"] {
            assert!(
                !publico.to_lowercase().contains(fragmento),
                "«{fragmento}» chegou a quem chamou: {publico}"
            );
        }
    }

    /// Um erro interno não mostra o que lhe puseram dentro.
    ///
    /// # Porque isto é um teste à parte
    ///
    /// Porque é **este** o facto que protege tudo o resto. `CoreError::Internal`
    /// carrega texto para quem opera ler no log, e não o imprime para quem
    /// chamou. Se alguém mudar o formato para `{0}` — uma alteração de uma
    /// linha, com boa intenção, para «melhorar o diagnóstico» — cada mensagem
    /// interna do sistema inteiro passa a sair pela API.
    #[test]
    fn um_erro_interno_nao_mostra_o_que_lhe_puseram_dentro() {
        let dentro = "connection to /var/run/postgres failed: password authentication";
        let publico = CoreError::Internal(dentro.to_owned()).to_string();

        assert!(
            !publico.contains("postgres") && !publico.contains("/var/"),
            "o interior de um erro interno chegou ao exterior: {publico}"
        );
    }
}
