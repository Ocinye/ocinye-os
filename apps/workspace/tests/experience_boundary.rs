//! O que um ecrã do Ocinye Experience não pode saber.
//!
//! # A fronteira, em duas frases
//!
//!     O Core detém a verdade institucional.
//!     A Experience detém a sua apresentação.
//!
//! O grafo de dependências guarda a metade estrutural disto — está em
//! `scripts/architecture_boundaries.py`, e é lá que se prova que o Workspace
//! não liga o Core nem persistência. Estes testes guardam a outra metade, a que
//! um grafo não vê: um ecrã pode ter as dependências certas e ainda assim
//! escrever à mão o caminho de um endpoint, ou decidir sozinho quem pode o quê.
//!
//! # A distinção que interessa
//!
//! | | |
//! |---|---|
//! | **Permitido** | A interface mostra ou esconde uma affordance com base num estado que **já recebeu** do Core. |
//! | **Proibido** | A interface conclui que a operação é autorizada porque `role == PlatformAdmin`. |
//!
//! E mesmo no primeiro caso o Core reautoriza no efeito. Esconder um botão é
//! cortesia; não é segurança.
//!
//! # Porque é que a zona é delimitada
//!
//! Um grep sobre o Workspace inteiro daria alarmes falsos em cada comentário
//! que nomeia uma rota e em cada teste do cliente que verifica se chama o
//! endpoint certo. O que se vigia é `ui/screens/` e `ui/components/` — o sítio
//! onde uma pessoa escreve interface — e ignora-se comentários, porque um
//! comentário que explica de onde vem um dado é documentação, não acoplamento.

use std::path::{Path, PathBuf};

/// Os ficheiros que compõem interface: ecrãs e componentes.
///
/// A shell fica de fora de propósito: ela é a moldura da aplicação e conhece o
/// cliente por dever de ofício. O que aqui se vigia é o que cada módulo escreve.
fn ficheiros_de_interface() -> Vec<PathBuf> {
    let raiz = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/ui");
    let mut encontrados = Vec::new();
    for pasta in ["screens", "components"] {
        let caminho = raiz.join(pasta);
        let entradas = std::fs::read_dir(&caminho)
            .unwrap_or_else(|erro| panic!("{} não é legível: {erro}", caminho.display()));
        for entrada in entradas {
            let caminho = entrada.expect("entrada").path();
            if caminho.extension().is_some_and(|e| e == "rs") {
                encontrados.push(caminho);
            }
        }
    }
    assert!(
        encontrados.len() > 10,
        "esperavam-se dezenas de ficheiros de interface; encontrados {}",
        encontrados.len()
    );
    encontrados
}

/// O código de produção, sem comentários e sem os módulos de teste.
///
/// Duas exclusões, e as duas por causa de falsos positivos reais.
///
/// Um comentário que diz de onde vem um dado — «vem de `GET /api/v1/me`» — é
/// documentação útil e não acoplamento. Confundir os dois faria o guarda punir
/// justamente quem se deu ao trabalho de explicar.
///
/// E o `notice.rs` tem um teste cuja lista de palavras proibidas inclui
/// `sqlx`: uma guarda que impede que a palavra apareça a quem usa o produto.
/// Um guarda a acusar outro guarda de exactamente aquilo que ele previne seria
/// a leitura mais errada possível. A propriedade é sobre **código de
/// produção**, e é isso que fica.
fn codigo(fonte: &str) -> String {
    let fonte = &sem_modulos_de_teste(fonte);
    let mut limpo = String::with_capacity(fonte.len());
    let mut em_bloco = false;
    for linha in fonte.lines() {
        let cortada = linha.trim_start();
        if em_bloco {
            if let Some(resto) = cortada.find("*/") {
                em_bloco = false;
                limpo.push_str(&cortada[resto + 2..]);
            }
            limpo.push('\n');
            continue;
        }
        if cortada.starts_with("//") {
            limpo.push('\n');
            continue;
        }
        if cortada.starts_with("/*") {
            em_bloco = !cortada.contains("*/");
            limpo.push('\n');
            continue;
        }
        limpo.push_str(linha);
        limpo.push('\n');
    }
    limpo
}

/// Retira os módulos de teste, cortando no primeiro `#[cfg(test)]`.
///
/// A primeira versão desta função contava chavetas, e foi um erro instrutivo.
/// Uma chaveta dentro de uma string do módulo de teste desequilibra a contagem,
/// o fecho nunca chega, e a função devolve silenciosamente só o princípio do
/// ficheiro — descartando exactamente o código que se queria vigiar. O guarda
/// ficava verde por não ter olhado, que é a pior maneira de ficar verde.
///
/// A regra que a substitui é a convenção do repositório, e
/// `os_modulos_de_teste_sao_o_ultimo_item` impõe-a em vez de a supor: o módulo
/// `#[cfg(test)]` é o último item do ficheiro, e não há código de produção
/// depois dele. Cortar aí é exacto, e não depende de analisar Rust.
fn sem_modulos_de_teste(fonte: &str) -> String {
    match fonte.find("#[cfg(test)]") {
        Some(inicio) => fonte[..inicio].to_owned(),
        None => fonte.to_owned(),
    }
}

/// A convenção em que o corte assenta, imposta e não suposta.
///
/// Se algum dia alguém puser código de produção depois do módulo de teste, este
/// teste falha — e falha **antes** de os outros passarem a olhar para menos
/// ficheiro do que julgam.
#[test]
fn os_modulos_de_teste_sao_o_ultimo_item() {
    let mut infractores = Vec::new();
    for caminho in ficheiros_de_interface() {
        let fonte = std::fs::read_to_string(&caminho).expect("ficheiro");
        let Some(inicio) = fonte.find("#[cfg(test)]") else {
            continue;
        };
        for (numero, linha) in fonte[inicio..].lines().enumerate().skip(1) {
            let inicio_da_linha = linha.chars().take_while(|c| *c == ' ').count();
            if inicio_da_linha > 0 || linha.trim().is_empty() {
                continue;
            }
            let cortada = linha.trim_start();
            let e_item = ["pub ", "fn ", "struct ", "enum ", "const ", "use ", "impl "]
                .iter()
                .any(|prefixo| cortada.starts_with(prefixo));
            if e_item {
                infractores.push(format!(
                    "{}: `{}` está depois do módulo de teste (linha {})",
                    caminho.file_name().unwrap().to_string_lossy(),
                    cortada,
                    numero + 1
                ));
            }
        }
    }
    assert!(
        infractores.is_empty(),
        "há código de produção depois de um módulo de teste, e os guardas desta \
         suite deixariam de o ver: {infractores:#?}"
    );
}

/// Nenhum ecrã conhece o caminho de um endpoint do Core.
///
/// As rotas do Core pertencem à camada de cliente. Um ecrã que as escreva passa
/// a depender da forma da API, e uma rota que mude deixa de ser uma alteração de
/// uma camada para passar a ser uma caçada por dezenas de ficheiros.
///
/// Os formulários são o caso a não confundir: num Workspace com renderização no
/// servidor, um `<form action="/…">` publica para uma rota **do Workspace**, e
/// isso é navegação do browser, não um cliente HTTP escondido.
#[test]
fn nenhum_ecra_escreve_o_caminho_de_um_endpoint_do_core() {
    let mut infractores = Vec::new();
    for caminho in ficheiros_de_interface() {
        let fonte = std::fs::read_to_string(&caminho).expect("ficheiro");
        for (numero, linha) in codigo(&fonte).lines().enumerate() {
            if linha.contains("/api/v1") {
                infractores.push(format!(
                    "{}:{} — {}",
                    caminho.file_name().unwrap().to_string_lossy(),
                    numero + 1,
                    linha.trim()
                ));
            }
        }
    }
    assert!(
        infractores.is_empty(),
        "um ecrã escreveu o caminho de um endpoint do Core; isso pertence à \
         camada de cliente (`src/api.rs`): {infractores:#?}"
    );
}

/// Nenhum ecrã fala HTTP por sua conta.
#[test]
fn nenhum_ecra_constroi_o_seu_proprio_cliente() {
    let mut infractores = Vec::new();
    for caminho in ficheiros_de_interface() {
        let fonte = std::fs::read_to_string(&caminho).expect("ficheiro");
        let limpo = codigo(&fonte);
        for agulha in ["reqwest", "sqlx", "PgPool", "repository::"] {
            if limpo.contains(agulha) {
                infractores.push(format!(
                    "{} contém `{agulha}`",
                    caminho.file_name().unwrap().to_string_lossy()
                ));
            }
        }
    }
    assert!(
        infractores.is_empty(),
        "um ecrã alcançou transporte ou persistência directamente: {infractores:#?}"
    );
}

/// Nenhum ecrã decide autorização.
///
/// Esta é a mais delicada de guardar, porque a fronteira não está na palavra e
/// sim na conclusão. Um ecrã pode receber do Core «esta pessoa administra a
/// plataforma» e usar isso para não mostrar um botão inútil. O que não pode é
/// **concluir** a partir de um papel que a operação está autorizada — porque
/// então passam a existir duas respostas para a mesma pergunta, e um dia
/// discordam.
///
/// Por isso o guarda não procura menções a papéis: procura os motores que
/// decidem. Um ecrã que importe o avaliador de políticas está a decidir, seja
/// qual for o nome que dê à variável.
#[test]
fn nenhum_ecra_importa_um_avaliador_de_politicas() {
    let mut infractores = Vec::new();
    for caminho in ficheiros_de_interface() {
        let fonte = std::fs::read_to_string(&caminho).expect("ficheiro");
        let limpo = codigo(&fonte);
        for agulha in [
            "ocinye_domain::policy",
            "ocinye_core::authz",
            "ocinye_core::modules",
            "VisibilityFilter",
            "CurrentAuthority",
            "authority::resolve",
        ] {
            if limpo.contains(agulha) {
                infractores.push(format!(
                    "{} importa `{agulha}`",
                    caminho.file_name().unwrap().to_string_lossy()
                ));
            }
        }
    }
    assert!(
        infractores.is_empty(),
        "um ecrã alcançou a maquinaria de autorização do Core. A interface \
         apresenta autorização; não a estabelece: {infractores:#?}"
    );
}

/// A prontidão não é inferida de um pedido de domínio.
///
/// # A regressão que isto guarda
///
/// Houve uma altura em que a topbar dizia `CORE OK` porque a consulta de
/// organização tinha respondido: `let core_ok = !organisation.is_null()`.
///
/// Um pedido de domínio responde por razões suas, e uma delas não é a prontidão
/// institucional. A base podia estar de pé com a compatibilidade quebrada, ou
/// com uma dependência crítica em baixo, e a interface afirmava que estava tudo
/// bem — porque tinha perguntado a coisa errada e acreditado na resposta.
///
/// # Porque é que isto é estrutural e não uma revisão de código
///
/// O padrão é fácil de reintroduzir sem má intenção: há um valor à mão, ele
/// costuma vir preenchido, e concluir dele que o Core está bem parece
/// razoável. Um teste que o recuse é mais fiável do que a memória de quem
/// revê — e foi assim que este voltou uma vez.
#[test]
fn a_prontidao_nao_e_inferida_de_um_pedido_de_dominio() {
    let rotas = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/routes.rs"),
    )
    .expect("routes.rs");

    let producao = codigo(&rotas);

    // Cada linha que decide o estado do Core tem de o obter do arranque, que é
    // quem fala com o `/ready`.
    let mut infractores = Vec::new();
    for (numero, linha) in producao.lines().enumerate() {
        if !linha.contains("core_status") || !linha.contains('=') {
            continue;
        }
        // A atribuição da estrutura não decide nada; passa adiante o que já foi
        // decidido.
        if linha.trim() == "core_status," {
            continue;
        }
        if linha.contains("boot::probe") || linha.contains("boot::BootState") {
            continue;
        }
        // Um bloco que abre é seguido pela decisão; o que interessa é que a
        // sonda esteja lá dentro.
        if linha.trim_end().ends_with('{') {
            continue;
        }
        infractores.push(format!("routes.rs:{} — {}", numero + 1, linha.trim()));
    }

    assert!(
        infractores.is_empty(),
        "o estado do Core foi decidido sem passar pela sonda de prontidão: \
         {infractores:#?}"
    );

    // E nenhum pedido de domínio serve de sonda.
    for agulha in [
        "!organisation.is_null()",
        "organisation.is_null()",
        "core_ready(&state).await.is_ok()",
    ] {
        assert!(
            !producao.contains(agulha),
            "«{agulha}» voltou: um pedido de domínio não é uma sonda de prontidão"
        );
    }
}

/// Conteúdo não confiável nunca vira estrutura no documento.
///
/// # A propriedade
///
/// > **Untrusted content must not create document structure.**
///
/// Títulos de actividades, nomes de pessoas, assuntos de correio, referências
/// bibliográficas — tudo o que vem do domínio é **texto**. O Leptos escapa por
/// omissão, e a maneira de sair dessa omissão chama-se `inner_html`.
///
/// # Porque isto é um teste e não o comentário que já lá estava
///
/// `mail.rs` dizia, em prosa, «o único `inner_html` do Ocinye Workspace». Era
/// verdade e não era verificada. Escrevi um segundo, no título dos eventos do
/// Calendário: compilou, passou os 23 portões do sistema de desenho, passou as
/// fronteiras arquitecturais e passou as viagens de browser. Um `<script>` no
/// título de uma reunião teria chegado ao documento de quem abrisse o mês.
///
/// # A excepção, e porque é uma só
///
/// O corpo de uma mensagem de correio é HTML por natureza — mostrá-lo como
/// texto seria mostrar as etiquetas a quem quer ler a carta. Ele atravessa a
/// sanitização do Core (`ocinye_core::modules::mail::sanitize`, sobre `ammonia`)
/// antes de chegar aqui, e é essa passagem que o autoriza. A lista abaixo é
/// para encolher: um sítio novo é uma decisão de segurança, não uma
/// conveniência de apresentação.
#[test]
fn conteudo_do_dominio_nunca_vira_marcacao() {
    /// Onde `inner_html` é legítimo, e porquê.
    const AUTORIZADOS: [(&str, &str); 1] = [(
        "mail.rs",
        "o corpo da mensagem, já sanitizado pelo Core antes de sair de lá",
    )];

    let ficheiros = ficheiros_de_interface();
    let mut lidos = 0usize;
    let mut infractores = Vec::new();

    for caminho in &ficheiros {
        let texto = std::fs::read_to_string(caminho)
            .unwrap_or_else(|erro| panic!("{} não é legível: {erro}", caminho.display()));
        lidos += 1;

        let nome = caminho
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default();

        for (numero, linha) in texto.lines().enumerate() {
            // Um comentário que fala de `inner_html` não é um `inner_html`. É
            // precisamente onde a razão da excepção está escrita.
            let cru = linha.trim_start();
            if cru.starts_with("//") || cru.starts_with("///") || cru.starts_with("*") {
                continue;
            }
            if !linha.contains("inner_html") {
                continue;
            }
            if AUTORIZADOS.iter().any(|(f, _)| *f == nome) {
                continue;
            }
            infractores.push(format!("{}:{}", caminho.display(), numero + 1));
        }
    }

    // Um universo vazio aprova tudo. Se não se leu nada, não se mediu nada.
    assert!(
        lidos > 10,
        "só {lidos} ficheiros de interface lidos: pequeno de mais para provar seja o que for"
    );

    assert!(
        infractores.is_empty(),
        "conteúdo do domínio a virar marcação, fora dos sítios autorizados: {infractores:#?}\n\
         Se é mesmo preciso, o conteúdo tem de passar por sanitização antes de aqui \
         chegar, e o sítio entra em AUTORIZADOS com a razão."
    );

    // E a excepção continua a ser o que diz ser: se `mail.rs` deixar de usar
    // `inner_html`, esta lista está a autorizar um sítio que já não existe, e a
    // próxima pessoa herda uma autorização sem dono.
    for (nome, razao) in AUTORIZADOS {
        let caminho = ficheiros
            .iter()
            .find(|c| c.file_name().and_then(|n| n.to_str()) == Some(nome))
            .unwrap_or_else(|| panic!("`{nome}` está autorizado e não existe"));
        let texto = std::fs::read_to_string(caminho).expect("legível");
        assert!(
            texto.lines().any(|l| {
                let cru = l.trim_start();
                !cru.starts_with("//") && !cru.starts_with("*") && l.contains("inner_html")
            }),
            "`{nome}` está autorizado a usar `inner_html` — {razao} — e já não o usa"
        );
    }
}

/// Nenhuma vista decide um dia civil em Greenwich.
///
/// # A invariante
///
/// «Em que dia isto cai» tem **uma** resposta no Ocinye, e ela vive em
/// `ui::tempo`. Quando cada vista a calculava por si, todas escreviam
/// `date_naive()` sobre um instante em UTC — e o Calendário mostrava um
/// compromisso das 00:30 em Lisboa no dia anterior, às 23:30.
///
/// O defeito não estava numa vista. Estava em não haver sítio nenhum onde a
/// pergunta se fizesse uma vez. Este portão existe para que a próxima vista não
/// possa voltar a decidi-lo sozinha.
#[test]
fn nenhuma_vista_decide_um_dia_civil_em_greenwich() {
    let raiz = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/ui");

    /// Onde a conversão pode viver, e porquê.
    ///
    /// Uma lista de excepções declaradas, e não uma ausência de portão: uma
    /// excepção nova tem de ser escrita aqui por alguém, com a razão.
    const AUTORIZADOS: [(&str, &str); 1] = [(
        "tempo.rs",
        "é a primitiva. É aqui que a conversão acontece, e é por isso que ela \
         existe num sítio só.",
    )];

    let mut ficheiros = Vec::new();
    let mut por_visitar = vec![raiz.clone()];
    while let Some(dir) = por_visitar.pop() {
        for entrada in std::fs::read_dir(&dir).expect("ler a pasta da interface") {
            let caminho = entrada.expect("entrada").path();
            if caminho.is_dir() {
                por_visitar.push(caminho);
            } else if caminho.extension().is_some_and(|e| e == "rs") {
                ficheiros.push(caminho);
            }
        }
    }

    assert!(
        ficheiros.len() > 5,
        "o portão não encontrou a interface: {} ficheiros",
        ficheiros.len()
    );

    let mut infractores = Vec::new();
    for caminho in &ficheiros {
        let nome = caminho
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default();
        if AUTORIZADOS.iter().any(|(f, _)| *f == nome) {
            continue;
        }
        let texto = std::fs::read_to_string(caminho).expect("ler");
        for (n, linha) in texto.lines().enumerate() {
            let codigo = linha.split("//").next().unwrap_or_default();
            if codigo.contains("date_naive()") || codigo.contains("naive_local()") {
                infractores.push(format!("{nome}:{}: {}", n + 1, linha.trim()));
            }
        }
    }

    assert!(
        infractores.is_empty(),
        "estas linhas decidem um dia civil fora da primitiva, e portanto em \
         Greenwich:\n  {}\n\nUse `ui::tempo::dia_civil` ou `hora_civil`, que \
         exigem a zona de quem olha.",
        infractores.join("\n  ")
    );

    // Uma excepção que ficou sem dono é uma excepção que ninguém volta a
    // questionar.
    for (ficheiro, _) in AUTORIZADOS {
        assert!(
            ficheiros
                .iter()
                .any(|c| c.file_name().and_then(|n| n.to_str()) == Some(ficheiro)),
            "«{ficheiro}» está autorizado e já não existe"
        );
    }
}
