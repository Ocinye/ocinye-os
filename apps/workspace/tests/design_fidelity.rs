//! Fidelidade ao dossier de design.
//!
//! O dossier em `design/` é a fonte de verdade. Estes testes lêem-no
//! directamente e comparam com o que está implementado, para que uma alteração
//! ao design que não chegue ao código falhe aqui em vez de passar despercebida.
//!
//! Não substituem a comparação visual com o protótipo; garantem o que uma
//! comparação visual não consegue garantir — que **todos** os valores foram
//! transpostos, e nenhum foi aproximado.

use std::collections::BTreeMap;

const TOKENS_MD: &str = include_str!("../../../design/DESIGN_TOKENS.md");
const OCINYE_CSS: &str = include_str!("../static/ocinye.css");
const ICONS_MD: &str = include_str!("../../../design/icons/ICONS.md");
const ICONS_SVG: &str = include_str!("../static/icons.svg");
const DESIGN_README: &str = include_str!("../../../design/README.md");

/// Extrai `--oc-nome: valor;` de um texto.
///
/// # Porque acumula em vez de ler linha a linha
///
/// Uma declaração pode ocupar várias linhas — `--oc-pop-shadow` tem três
/// camadas, uma por linha, porque cada uma faz uma coisa diferente e escrevê-las
/// seguidas esconde isso.
///
/// A versão anterior lia linha a linha. Nessa leitura, `--oc-pop-shadow:` era
/// um par com valor **vazio**, e vazio no dossier igualava vazio no CSS: a
/// paridade dava verde sem comparar coisa nenhuma. Verificou-se por reversão —
/// uma sombra deliberadamente errada no dossier passou o portão.
///
/// Por isso o texto entra num acumulador e só se fecha um par ao encontrar o
/// `;`. Uma declaração que nunca feche arrasta o que vem a seguir e aparece
/// como valor errado, que é a direcção certa para falhar: ruidosa, e não muda.
fn custom_properties(source: &str) -> BTreeMap<String, String> {
    /// Um nome de propriedade, e não uma frase que começa por `--oc-`.
    ///
    /// O dossier é markdown: tem tabelas e prosa onde os nomes aparecem em
    /// texto corrido. Sem isto, uma frase como «`--oc-interface-scale` é
    /// aplicada uma vez, na raiz:» seria lida como uma declaração aberta e
    /// arrastaria as secções seguintes para dentro do seu valor.
    fn e_um_nome(nome: &str) -> bool {
        nome.starts_with("--oc-")
            && nome
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    }

    let mut found = BTreeMap::new();
    let mut aberta: Option<String> = None;

    for line in source.lines() {
        let line = line.split("/*").next().unwrap_or(line).trim();

        // Uma declaração que ficou por fechar continua nesta linha.
        if let Some(mut acumulada) = aberta.take() {
            acumulada.push(' ');
            acumulada.push_str(line);
            match acumulada.find(';') {
                Some(fim) => registar(&mut found, &acumulada[..fim], e_um_nome),
                None => aberta = Some(acumulada),
            }
            continue;
        }

        let mut pedacos = line.split(';').peekable();
        while let Some(pedaco) = pedacos.next() {
            if pedacos.peek().is_some() {
                // Fechado por `;` nesta mesma linha.
                registar(&mut found, pedaco, e_um_nome);
            } else if pedaco.trim().starts_with("--oc-") && pedaco.contains(':') {
                let (nome, _) = pedaco.split_once(':').unwrap_or((pedaco, ""));
                if e_um_nome(nome.trim()) {
                    aberta = Some(pedaco.trim().to_owned());
                }
            }
        }
    }

    found
}

/// Guarda um par, se o que está antes de `:` for mesmo um nome de propriedade.
fn registar(found: &mut BTreeMap<String, String>, chunk: &str, e_um_nome: fn(&str) -> bool) {
    let chunk = chunk.trim();
    let Some((nome, valor)) = chunk.split_once(':') else {
        return;
    };
    let nome = nome.trim();
    if !e_um_nome(nome) {
        return;
    }
    found.insert(
        nome.to_owned(),
        valor.split_whitespace().collect::<Vec<_>>().join(" "),
    );
}

/// Todos os tokens do dossier existem no CSS, com o mesmo valor.
///
/// Este é o teste que impede "quase igual": um `#0B2D4B` em vez de `#0B2D4A`
/// passaria despercebido a olho e falha aqui.
#[test]
fn todos_os_tokens_do_dossier_estao_no_css_com_o_mesmo_valor() {
    let expected = custom_properties(TOKENS_MD);
    let actual = custom_properties(OCINYE_CSS);

    assert!(
        expected.len() > 60,
        "o dossier devia declarar dezenas de tokens; encontrados {}",
        expected.len()
    );

    let mut problems = Vec::new();
    for (name, value) in &expected {
        match actual.get(name) {
            None => problems.push(format!("{name}: em falta no CSS")),
            Some(found) if found != value => {
                problems.push(format!("{name}: dossier `{value}`, CSS `{found}`"));
            }
            Some(_) => {}
        }
    }

    assert!(
        problems.is_empty(),
        "tokens divergentes:\n  {}",
        problems.join("\n  ")
    );
}

/// As cores de marca, verificadas explicitamente.
///
/// Redundante com o teste acima por desenho: se alguém alterar o dossier por
/// engano, estas duas cores devem continuar a ser questionadas.
#[test]
fn as_cores_de_marca_sao_as_da_ocinye() {
    let tokens = custom_properties(OCINYE_CSS);
    assert_eq!(tokens.get("--oc-navy").map(String::as_str), Some("#0B2D4A"));
    assert_eq!(tokens.get("--oc-gold").map(String::as_str), Some("#E0A731"));
    assert_eq!(
        tokens.get("--oc-navy-deep").map(String::as_str),
        Some("#071E33")
    );
}

/// O dossier e o sprite descrevem o mesmo conjunto, nos dois sentidos.
///
/// A verificação nos dois sentidos é o que importa: uma só direcção deixaria
/// passar um símbolo acrescentado ao sprite e nunca registado no dossier, que
/// é como os dois se separam sem ninguém reparar.
#[test]
fn o_sprite_e_o_catalogo_de_icones_cobrem_se_mutuamente() {
    let declared: Vec<&str> = ICONS_MD
        .lines()
        .filter_map(|line| line.split('`').nth(1))
        .filter(|id| id.starts_with("oc-"))
        .collect();

    // Quarenta e nove desde 2026-08-29: `oc-files` e `oc-folder` entraram com o
    // ecrã de Ficheiros. Uma pasta e uma folha, e não um disco ou uma nuvem: o
    // que o ecrã mostra é arrumação institucional, não armazenamento — a pasta
    // não decide nada sobre quem lê o que está dentro dela.
    //
    // Quarenta e sete desde 2026-08-28: `oc-science` entrou com a cadeia
    // científica. Três nós ligados por duas arestas, e não um frasco de
    // laboratório: o que o ecrã mostra é proveniência, e o Ocinye OS não é um
    // caderno de laboratório electrónico (ADR-0412).
    //
    // Quarenta e seis desde 2026-08-27: `oc-close` entrou porque o conjunto não
    // tinha um glifo de fechar, e o compositor fechava com uma seta — que se
    // lê como «seguinte». O número muda **por decisão**, e é isso que este
    // teste protege: um ícone que desapareça do dossier tem de dar vermelho.
    assert_eq!(declared.len(), 49, "o dossier declara 49 ícones");

    for id in &declared {
        assert!(
            ICONS_SVG.contains(&format!("id=\"{id}\"")),
            "o ícone {id} está no dossier e não no sprite"
        );
    }

    let in_sprite: Vec<&str> = ICONS_SVG
        .split("id=\"")
        .skip(1)
        .filter_map(|part| part.split('"').next())
        .filter(|id| id.starts_with("oc-"))
        .collect();

    for id in &in_sprite {
        assert!(
            declared.contains(id),
            "o ícone {id} está no sprite e não foi registado no dossier"
        );
    }

    assert_eq!(in_sprite.len(), declared.len());
}

/// O focus ring é o do dossier, é global, e vem de tokens.
///
/// Antes este teste afirmava a cadeia literal `outline: 2px solid
/// var(--oc-gold)`. Passou a afirmar os tokens, que é propriedade mais forte:
/// a cadeia literal só provava que **aquele** anel existia algures, e deixava
/// um módulo novo livre para escrever outro ao lado.
///
/// Há dois afastamentos, e os dois têm nome. O anel normal afasta-se 1px do
/// elemento que tem o foco; o `wrap` afasta-se 2px porque envolve um contentor
/// cujo filho é que está focado, e precisa de livrar a borda do contentor. Dois
/// casos, e não dois acidentes — que era o que estava antes.
#[test]
fn o_focus_ring_e_dourado_e_global() {
    assert!(DESIGN_README.contains("outline:2px solid #E0A731"));

    let tokens = custom_properties(OCINYE_CSS);
    for (name, value) in [
        ("--oc-focus-color", "var(--oc-gold)"),
        ("--oc-focus-width", "2px"),
        ("--oc-focus-offset", "1px"),
        ("--oc-focus-offset-wrap", "2px"),
    ] {
        assert_eq!(
            tokens.get(name).map(String::as_str),
            Some(value),
            "o tratamento de foco tem de vir de tokens; {name} não confere"
        );
    }

    assert!(
        OCINYE_CSS.contains(":focus-visible")
            && OCINYE_CSS.contains("outline: var(--oc-focus-width) solid var(--oc-focus-color)"),
        "o focus ring tem de estar definido globalmente, a partir dos tokens"
    );

    // Nenhum anel escrito à mão ao lado do canónico. É isto que impede o
    // Calendar de ter um outline, o Boot outro e as Definições um terceiro.
    let literais = OCINYE_CSS
        .lines()
        .filter(|linha| linha.contains("outline:") && linha.contains("solid"))
        .filter(|linha| !linha.contains("var(--oc-focus-"))
        .collect::<Vec<_>>();
    assert!(
        literais.is_empty(),
        "há anéis de foco escritos fora dos tokens: {literais:?}"
    );
}

/// O corpo do CSS, sem o bloco `:root`.
///
/// O bloco dos tokens é a fonte autorizada da linguagem visual: é suposto ter
/// cores, durações e camadas escritas por extenso. O que os guardas abaixo
/// vigiam é tudo o resto — o sítio onde um valor cru significa que alguém
/// escolheu sozinho.
fn corpo_do_css() -> &'static str {
    let fim = OCINYE_CSS
        .find("\n}")
        .expect("o bloco :root tem de existir e fechar");
    &OCINYE_CSS[fim..]
}

/// Nenhuma camada de sobreposição é escolhida à mão.
///
/// Um módulo não precisa de saber que `modal` é 200. Precisa de saber que um
/// popover não passa por cima de um modal — e é por isso que a hierarquia é
/// semântica e vive num sítio só. Sem isto, a resposta a «o meu menu ficou por
/// baixo» é sempre `z-index: 999999`, e a ordem passa a ser o que cada módulo
/// conseguiu gritar mais alto.
#[test]
fn nenhum_z_index_e_escolhido_a_mao() {
    let crus: Vec<&str> = corpo_do_css()
        .lines()
        .filter(|linha| linha.contains("z-index:") && !linha.contains("var(--oc-z-"))
        .collect();
    assert!(
        crus.is_empty(),
        "há camadas escolhidas à mão em vez de `var(--oc-z-…)`: {crus:?}"
    );
}

/// Nenhuma duração de transição é inventada.
///
/// Três durações e três curvas chegam para todo o sistema. O que este guarda
/// impede não é um valor feio: é o Boot animar a 200ms, o Centro Temporal a
/// 300ms e o Calendar a 120ms, e o produto passar a parecer três produtos.
#[test]
fn nenhuma_transicao_inventa_a_sua_duracao() {
    // O inventário do que ficou de fora da escala, com a razão de cada um.
    // Este conjunto só pode diminuir.
    const EXCEPCOES: [&str; 3] = [
        "transform .1s", // composição de um elemento, não a escala
        "ocFade .14s",   // 10ms fora da escala; snap é decisão à parte
        "ocPulse 2.6s",  // o ritmo do ponto de estado do Core
    ];

    let crus: Vec<&str> = corpo_do_css()
        .lines()
        .filter(|linha| linha.contains("transition:") || linha.contains("animation:"))
        .filter(|linha| {
            linha.split([';', '{']).any(|parte| {
                (parte.contains("transition:") || parte.contains("animation:"))
                    && parte.contains('s')
                    && parte.split_whitespace().any(|palavra| {
                        palavra.trim_end_matches(',').ends_with('s')
                            && palavra.starts_with(|c: char| c.is_ascii_digit() || c == '.')
                    })
            })
        })
        .filter(|linha| !EXCEPCOES.iter().any(|excepcao| linha.contains(excepcao)))
        .filter(|linha| !linha.contains(".01ms")) // válvula do movimento reduzido
        .collect();

    assert!(
        crus.is_empty(),
        "há durações fora da escala e fora do inventário: {crus:?}"
    );
}

/// Nenhuma cor nova é escrita à mão fora da fonte dos tokens.
///
/// O risco não é uma cor errada. É `#173A5E` — quase igual ao navy, escolhido
/// por alguém que não encontrou o token e achou que se via na mesma. Ao fim de
/// cinco módulos, a instituição tem cinco azuis.
#[test]
fn nenhuma_cor_nova_e_escrita_a_mao() {
    // O que já estava escrito quando este guarda entrou. Cada um destes é
    // candidato a token; nenhum é candidato a companhia. O conjunto só encolhe.
    const HERANCA: [&str; 8] = [
        "#CFD8E3", // polegar da barra de deslocamento
        "#4FA97B", // verde de presença
        "#F1F5F9", // fundo do badge navy
        "#DCE4EC", // borda do badge navy
        "#F5F7F9", // fundo do badge cinzento
        "#1C4B74", // topo do gradiente
        "#0E3454", // base do gradiente
        "#E0A731", // o dourado, no comentário que o nomeia
    ];

    let mut fora = Vec::new();
    for linha in corpo_do_css().lines() {
        let mut resto = linha;
        while let Some(inicio) = resto.find('#') {
            let cor: String = resto[inicio..]
                .chars()
                .take_while(|c| *c == '#' || c.is_ascii_hexdigit())
                .collect();
            resto = &resto[inicio + 1..];
            if cor.len() < 4 {
                continue;
            }
            let normalizada = cor.to_uppercase();
            if !HERANCA.contains(&normalizada.as_str()) {
                fora.push(format!("{normalizada} em `{}`", linha.trim()));
            }
        }
    }

    assert!(
        fora.is_empty(),
        "cores escritas à mão fora dos tokens e fora do inventário: {fora:#?}"
    );
}

/// A mesma regra, para as cores escritas em decimal.
///
/// # Como esta descobriu que faltava
///
/// O guarda acima procura `#`. Escrever `rgba(224, 167, 49, .3)` é escrever o
/// dourado à mão exactamente da mesma maneira, e passava-lhe ao lado — o
/// buraco tinha o tamanho de uma família inteira de sintaxe.
///
/// Descobri-o da pior forma: a folha de estilos do arranque tinha um comentário
/// a dizer que toda a cor vem dos tokens, e duas declarações abaixo escrevia o
/// dourado em decimal. A reversão que eu tinha corrido para provar que o
/// arranque não escapava aos guardas usou `#173A5E` e foi recusada; se tivesse
/// usado `rgba(...)`, teria sido aceite. O guarda dizia que sim a código que
/// não devia passar, e eu li isso como prova.
///
/// # Branco e preto não contam
///
/// `rgba(255,255,255,…)` e `rgba(0,0,0,…)` não são cores da instituição: são
/// véus e sombras sobre o que está por baixo. Não há um token errado a
/// escolher, porque não há marca nenhuma envolvida.
#[test]
fn nenhuma_cor_nova_e_escrita_em_decimal() {
    // O que já estava escrito quando este guarda entrou. O conjunto só encolhe.
    //
    // As cores estruturais entram por valor: são a paleta navy, e uma nova
    // opacidade sobre elas não é uma cor nova.
    const HERANCA: [&str; 6] = [
        "#0B2D4A", // navy
        "#071E33", // navy profundo
        "#0B1A2D", // navy de sombra
        "#154974", // navy do gradiente
        "#4FA97B", // verde de presença
        "#B3261E", // vermelho de erro
    ];

    // O dourado entra pelo literal exacto, e não pelo valor.
    //
    // É a cor da marca, e é a que alguém escreve à mão por não encontrar o
    // token — foi o que aconteceu no arranque. Inventariar o valor abriria a
    // porta a qualquer dourado novo; inventariar estes três literais deixa
    // passar o que já existe e recusa uma opacidade que ainda não existisse.
    const DOURADOS_HERDADOS: [&str; 3] = [
        "rgba(224,167,49,.13)", // brilho do ecrã de entrada
        "rgba(224,167,49,.14)", // anel do ecrã de entrada
        "rgba(224,167,49,.22)", // moldura do cartão em destaque
    ];

    let mut fora = Vec::new();
    for linha in corpo_do_css().lines() {
        for captura in linha.match_indices("rgb") {
            let resto = &linha[captura.0..];
            let Some(fim) = resto.find(')') else { continue };
            let dentro = &resto[..fim];
            let Some(abre) = dentro.find('(') else {
                continue;
            };
            let canais: Vec<u32> = dentro[abre + 1..]
                .split(',')
                .take(3)
                .filter_map(|c| c.trim().parse().ok())
                .collect();
            if canais.len() != 3 {
                continue;
            }
            // Véus e sombras: sem marca, sem token por escolher.
            if canais == [255, 255, 255] || canais == [0, 0, 0] {
                continue;
            }
            let hex = format!("#{:02X}{:02X}{:02X}", canais[0], canais[1], canais[2]);
            if HERANCA.contains(&hex.as_str()) {
                continue;
            }
            let literal: String = dentro
                .chars()
                .filter(|c| !c.is_whitespace())
                .chain(")".chars())
                .collect();
            if DOURADOS_HERDADOS.contains(&literal.as_str()) {
                continue;
            }
            fora.push(format!("{hex} escrito em decimal em `{}`", linha.trim()));
        }
    }

    assert!(
        fora.is_empty(),
        "cores escritas à mão em decimal, fora dos tokens e fora do inventário: {fora:#?}"
    );
}

/// As dimensões estruturais do dossier.
#[test]
fn as_dimensoes_da_shell_sao_as_do_dossier() {
    let tokens = custom_properties(OCINYE_CSS);
    for (name, value) in [
        ("--oc-sidebar-w", "224px"),
        ("--oc-sidebar-w-collapsed", "58px"),
        ("--oc-topbar-h", "52px"),
        ("--oc-row-h", "38px"),
        ("--oc-row-h-dense", "30px"),
    ] {
        assert_eq!(tokens.get(name).map(String::as_str), Some(value), "{name}");
    }
}

/// As animações do dossier existem, e não há outras decorativas.
#[test]
fn existem_as_duas_animacoes_do_dossier_e_nao_mais() {
    assert!(OCINYE_CSS.contains("@keyframes ocFade"));
    assert!(OCINYE_CSS.contains("@keyframes ocPulse"));
    assert_eq!(
        OCINYE_CSS.matches("@keyframes").count(),
        2,
        "o dossier define duas animações; não devem existir mais"
    );
}

/// O tipo de letra é o do dossier.
#[test]
fn a_tipografia_e_ibm_plex() {
    let tokens = custom_properties(OCINYE_CSS);
    let sans = tokens.get("--oc-font-sans").cloned().unwrap_or_default();
    let mono = tokens.get("--oc-font-mono").cloned().unwrap_or_default();

    assert!(sans.contains("IBM Plex Sans"), "sans: {sans}");
    assert!(mono.contains("IBM Plex Mono"), "mono: {mono}");
    // E há sempre uma alternativa do sistema, para o caso de a fonte não
    // carregar.
    assert!(sans.contains("system-ui"));
}

/// Movimento reduzido é respeitado.
///
/// O dossier não o especifica, mas exige acessibilidade; uma animação infinita
/// como o pulsar do estado do Core é exactamente o que incomoda quem pede
/// movimento reduzido.
#[test]
fn o_movimento_reduzido_e_respeitado() {
    assert!(OCINYE_CSS.contains("prefers-reduced-motion"));
}

// ---------------------------------------------------------------------------
// Comparação com o protótipo
// ---------------------------------------------------------------------------
//
// O protótipo é um `.dc.html` que precisa do runtime do Claude Design para
// renderizar, por isso não pode ser aberto num browser nem comparado por
// diferença de imagem. Mas os seus componentes trazem as medidas em `style=`
// inline — e essas são comparáveis exactamente, que é mais do que um olho
// consegue fazer.
//
// Cada assinatura abaixo foi extraída do protótipo. `proof` é o fragmento
// literal que a ancora: se o design mudar, o fragmento desaparece e o teste
// falha a dizer que a assinatura deixou de existir, em vez de continuar a
// validar uma medida obsoleta.

const PROTOTYPE: &str = include_str!("../../../design/prototype/Ocinye Workspace.dc.html");

/// Declarações de uma regra de `ocinye.css`, com `var(--oc-*)` já resolvido.
fn declarations(selector: &str) -> BTreeMap<String, String> {
    let tokens = custom_properties(OCINYE_CSS);

    let start = OCINYE_CSS
        .find(&format!("\n{selector} {{"))
        .unwrap_or_else(|| panic!("selector ausente de ocinye.css: {selector}"));
    let body_start = start + OCINYE_CSS[start..].find('{').unwrap() + 1;
    let body_end = body_start + OCINYE_CSS[body_start..].find('}').unwrap();

    let mut found = BTreeMap::new();
    for chunk in OCINYE_CSS[body_start..body_end].split(';') {
        let Some((property, value)) = chunk.split_once(':') else {
            continue;
        };

        // Resolve os tokens, para comparar com os valores literais do protótipo.
        let mut value = value.split_whitespace().collect::<Vec<_>>().join(" ");
        while let Some(open) = value.find("var(") {
            let close = open + value[open..].find(')').expect("var() por fechar");
            let name = value[open + 4..close].trim();
            let resolved = tokens
                .get(name)
                .unwrap_or_else(|| panic!("token indefinido: {name}"));
            value.replace_range(open..=close, resolved);
        }

        found.insert(property.trim().to_string(), value.trim().to_string());
    }
    found
}

/// Um componente do protótipo e as medidas que o definem.
struct Signature {
    component: &'static str,
    selector: &'static str,
    /// Fragmento literal do protótipo que ancora esta assinatura.
    proof: &'static str,
    /// `(propriedade, valor)` — o valor implementado tem de começar por este,
    /// para que `font: 400 12px` case com a pilha de fontes completa.
    expected: &'static [(&'static str, &'static str)],
}

const SIGNATURES: &[Signature] = &[
    Signature {
        component: "pesquisa global da topbar",
        selector: ".oc-search",
        proof: "flex:0 1 420px;min-width:180px;height:33px",
        expected: &[
            ("height", "33px"),
            ("padding", "0 11px"),
            ("gap", "9px"),
            ("border-radius", "8px"),
            ("background", "#F3F6F9"),
            ("border", "1px solid #E4E9F0"),
        ],
    },
    Signature {
        component: "item de navegação da sidebar",
        selector: ".oc-nav",
        proof: "gap:10px;height:32px;padding:0 9px;border-radius:8px",
        expected: &[
            ("height", "32px"),
            ("padding", "0 9px"),
            ("gap", "10px"),
            ("border-radius", "8px"),
            ("font", "500 12.5px"),
        ],
    },
    Signature {
        component: "badge de estado",
        selector: ".oc-badge",
        proof: "letter-spacing:.04em;color:{{ c.tone.fg }};background:{{ c.tone.bg }};\
                border:1px solid {{ c.tone.bd }};border-radius:4px;padding:2.5px 6px",
        expected: &[
            ("gap", "5px"),
            ("padding", "2.5px 6px"),
            ("border-radius", "4px"),
            ("letter-spacing", ".04em"),
            ("font", "500 10px"),
        ],
    },
    Signature {
        component: "tab em pill",
        selector: ".oc-tab",
        proof: "height:27px;display:flex;align-items:center;padding:0 11px;border-radius:7px",
        expected: &[
            ("height", "27px"),
            ("padding", "0 11px"),
            ("border-radius", "7px"),
            ("font", "500 12px"),
        ],
    },
    Signature {
        component: "cabeçalho de secção",
        selector: ".oc-card__head",
        proof: "padding:13px 15px;border-bottom:1px solid #EEF2F6",
        expected: &[
            ("padding", "13px 15px"),
            ("border-bottom", "1px solid #EEF2F6"),
        ],
    },
    Signature {
        component: "header de colunas da tabela",
        selector: ".oc-table__head",
        proof: "gap:14px;padding:0 16px;height:34px;align-items:center;background:#FAFCFD",
        expected: &[
            ("height", "34px"),
            ("background", "#FAFCFD"),
            ("border-bottom", "1px solid #EEF2F6"),
        ],
    },
    Signature {
        component: "grelha de linha da tabela",
        selector: ".oc-table__head, .oc-table__row",
        proof: "gap:14px;padding:0 16px;min-height:{{ tbl.rowH }}",
        expected: &[("gap", "14px"), ("padding", "0 16px")],
    },
    Signature {
        component: "pesquisa dentro da tabela",
        selector: ".oc-table__search",
        proof: "gap:8px;height:29px;padding:0 10px;background:#F3F6F9",
        expected: &[
            ("width", "250px"),
            ("height", "29px"),
            ("padding", "0 10px"),
            ("gap", "8px"),
            ("border-radius", "7px"),
        ],
    },
    Signature {
        component: "botão de paginação",
        selector: ".oc-page-btn",
        proof: "border:1px solid #E4E9F0;border-radius:7px;cursor:pointer;\
                font:400 12px 'IBM Plex Mono',monospace;color:#8A98A6",
        expected: &[
            ("width", "27px"),
            ("height", "27px"),
            ("border-radius", "7px"),
            ("font", "400 12px"),
            ("color", "#8A98A6"),
        ],
    },
    Signature {
        component: "menu + Criar",
        selector: ".oc-create__menu",
        proof: "width:212px;background:#FFFFFF;border:1px solid #E4E9F0;border-radius:11px",
        expected: &[
            ("width", "212px"),
            ("padding", "6px"),
            ("border-radius", "11px"),
            ("box-shadow", "0 16px 40px"),
        ],
    },
    Signature {
        component: "painel da command palette",
        selector: ".oc-palette__panel",
        proof: "width:600px;max-width:92vw",
        expected: &[
            ("width", "600px"),
            ("max-width", "92vw"),
            ("border-radius", "14px"),
            ("box-shadow", "0 30px 80px"),
        ],
    },
    Signature {
        component: "tile de estado vazio",
        selector: ".oc-empty__tile",
        proof: "width:78px;height:78px;border:1px solid #E4E9F0;border-radius:20px;\
                display:flex;align-items:center;justify-content:center;background:#FAFCFD",
        expected: &[
            ("width", "78px"),
            ("height", "78px"),
            ("border-radius", "20px"),
            ("background", "#FAFCFD"),
        ],
    },
    Signature {
        component: "tile de acesso rápido",
        selector: ".oc-quick",
        proof: "gap:9px;height:36px;padding:0 11px;border:1px solid #E4E9F0;border-radius:8px",
        expected: &[
            ("height", "36px"),
            ("padding", "0 11px"),
            ("gap", "9px"),
            ("border-radius", "8px"),
            ("color", "#0B2D4A"),
            ("font", "500 12px"),
        ],
    },
    Signature {
        component: "barra de contexto do Prompt",
        selector: ".oc-prompt__bar",
        proof: "gap:12px;padding:0 24px;height:52px;border-bottom:1px solid #E4E9F0",
        expected: &[("height", "52px"), ("padding", "0 24px"), ("gap", "12px")],
    },
    Signature {
        component: "pill de contexto do Prompt",
        selector: ".oc-prompt__context",
        proof: "gap:7px;height:31px;padding:0 11px;background:#F3F6F9;border-radius:8px",
        expected: &[
            ("height", "31px"),
            ("padding", "0 11px"),
            ("gap", "7px"),
            ("background", "#F3F6F9"),
        ],
    },
    Signature {
        component: "chip de capacidade",
        selector: ".oc-cap",
        proof: "height:29px;display:flex;align-items:center;gap:6px;padding:0 11px",
        expected: &[
            ("height", "29px"),
            ("padding", "0 11px"),
            ("gap", "6px"),
            ("border-radius", "8px"),
            ("font", "500 11.5px"),
        ],
    },
    Signature {
        component: "área de conversa do Prompt",
        selector: ".oc-prompt__conv",
        proof: "justify-content:center;padding:48px 24px;gap:26px",
        expected: &[("padding", "48px 24px"), ("gap", "26px")],
    },
    Signature {
        component: "hero do Prompt",
        selector: ".oc-prompt__hero",
        proof: "align-items:center;gap:14px;max-width:560px;text-align:center",
        expected: &[("max-width", "560px"), ("gap", "14px")],
    },
    Signature {
        component: "sugestão do Prompt",
        selector: ".oc-suggestion",
        proof: "height:32px;display:flex;align-items:center;padding:0 12px;\
                border:1px solid #E4E9F0;border-radius:8px",
        expected: &[
            ("height", "32px"),
            ("padding", "0 12px"),
            ("border-radius", "8px"),
            ("font", "400 12px"),
            ("color", "#42546A"),
        ],
    },
    Signature {
        component: "caixa do Prompt",
        selector: ".oc-prompt__input",
        proof: "max-width:880px;margin:0 auto;border:1px solid #E4E9F0;border-radius:14px",
        expected: &[
            ("max-width", "880px"),
            ("border-radius", "14px"),
            ("padding", "14px 15px 11px"),
            ("box-shadow", "0 4px 18px"),
        ],
    },
    Signature {
        component: "acção do Prompt",
        selector: ".oc-chip",
        proof: "gap:7px;height:29px;padding:0 10px;border:1px solid #E4E9F0;border-radius:8px",
        expected: &[
            ("height", "29px"),
            ("padding", "0 10px"),
            ("gap", "7px"),
            ("font", "400 11.5px"),
            ("color", "#5F7183"),
        ],
    },
    Signature {
        component: "botão de envio do Prompt",
        selector: ".oc-prompt__send",
        proof: "width:34px;height:34px;border-radius:50%;background:#E0A731",
        expected: &[
            ("width", "34px"),
            ("height", "34px"),
            ("background", "#E0A731"),
        ],
    },
    Signature {
        component: "aviso do Prompt",
        selector: ".oc-prompt__note",
        proof: "max-width:880px;margin:10px auto 0;text-align:center;font:400 10.5px",
        expected: &[
            ("max-width", "880px"),
            ("margin", "10px auto 0"),
            ("font", "400 10.5px"),
        ],
    },
];

/// Cada componente implementado tem as medidas exactas do protótipo.
#[test]
fn os_componentes_tem_as_medidas_do_prototipo() {
    // O protótipo traz o CSS inline sem espaços; normalizamos para comparar.
    let prototype = PROTOTYPE.replace("&quot;", "\"").replace("; ", ";");

    for signature in SIGNATURES {
        let Signature {
            component,
            selector,
            proof,
            expected,
        } = signature;

        assert!(
            prototype.contains(proof),
            "a assinatura de «{component}» já não existe no protótipo; \
             o design mudou e a medida validada aqui está obsoleta.\n\
             procurado: {proof}"
        );

        let implemented = declarations(selector);
        for (property, value) in *expected {
            let actual = implemented
                .get(*property)
                .unwrap_or_else(|| panic!("«{component}» ({selector}) não declara `{property}`"));
            assert!(
                actual.starts_with(value),
                "«{component}» ({selector}) diverge do protótipo em `{property}`:\n  \
                 protótipo: {value}\n  implementado: {actual}"
            );
        }
    }
}

/// O autofill do browser não pode repintar os campos de autenticação.
///
/// O `<input>` é transparente por desenho, sobre o vidro do cartão. Ao preencher
/// a palavra-passe, o WebKit pinta um fundo claro ao nível do user-agent que
/// `background` não anula — e o campo ficava branco exactamente até onde o
/// `<input>` acaba, ao lado do botão «Mostrar».
///
/// Os dois selectores têm de existir **separados**: um browser que desconheça um
/// deles descarta a regra inteira onde ele aparece, pelo que agrupá-los faria o
/// desconhecido levar o conhecido consigo.
#[test]
fn o_autofill_nao_repinta_os_campos_de_autenticacao() {
    assert!(
        OCINYE_CSS.contains(".oc-login__field input:-webkit-autofill"),
        "o realce de autofill do WebKit tem de ser neutralizado nos campos de entrada"
    );
    assert!(
        OCINYE_CSS.contains("background-clip: text"),
        "o fundo do autofill é recortado aos glifos, para manter a transparência do cartão"
    );
    assert!(
        OCINYE_CSS.contains("-webkit-text-fill-color: var(--oc-on-navy)"),
        "o texto preenchido tem de ficar na cor clara, e não na do user-agent"
    );
    assert!(
        OCINYE_CSS.contains(".oc-login__field input:autofill"),
        "o Firefox aplica o realce com um filtro, e precisa da sua própria regra"
    );
}

/// Nenhum ecrã pode depender de um atributo `style`.
///
/// A Content-Security-Policy do Workspace declara `style-src 'self'` sem
/// `'unsafe-inline'` — decisão registada no threat model e no baseline de
/// segurança, onde um botão de correio chegou a ser retirado por causa dela em
/// vez de a política ser alargada.
///
/// O browser descarta esses atributos antes de pintar. Um `style` inline chega
/// portanto **correcto no HTML** e sem efeito nenhum no ecrã, que é a razão de
/// os testes de marcação nunca terem apanhado isto: as tabelas tinham as suas
/// colunas no atributo, `display: grid` caía para uma coluna só, e o cabeçalho
/// empilhava-se por cima das linhas em todas as listas da aplicação.
///
/// A regra é procurada no código-fonte e não no HTML renderizado: assim cobre
/// também os ecrãs que nenhum teste chega a renderizar.
#[test]
fn nenhum_ecra_depende_de_um_atributo_style_que_a_csp_descarta() {
    let mut culpados = Vec::new();
    let raiz = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");

    fn varrer(dir: &std::path::Path, culpados: &mut Vec<String>) {
        for entrada in std::fs::read_dir(dir).expect("ler directório") {
            let caminho = entrada.expect("entrada").path();
            if caminho.is_dir() {
                varrer(&caminho, culpados);
            } else if caminho.extension().is_some_and(|e| e == "rs") {
                let fonte = std::fs::read_to_string(&caminho).expect("ler ficheiro");
                for (n, linha) in fonte.lines().enumerate() {
                    let corte = linha.trim_start();
                    if corte.starts_with("//") {
                        continue;
                    }
                    if corte.contains("style=") {
                        culpados.push(format!("{}:{}", caminho.display(), n + 1));
                    }
                }
            }
        }
    }

    varrer(&raiz, &mut culpados);

    assert!(
        culpados.is_empty(),
        "a CSP descarta `style` inline; estes seriam ignorados pelo browser:\n  {}",
        culpados.join("\n  ")
    );
}

/// A política que torna o teste acima necessário continua a existir.
///
/// Se `'unsafe-inline'` alguma vez voltar, isto tem de ser uma decisão visível
/// e não um efeito colateral de alguém a tentar fazer uma tabela funcionar.
#[test]
fn a_csp_do_workspace_nao_admite_estilos_inline() {
    let rotas = include_str!("../src/routes.rs");
    assert!(rotas.contains("style-src 'self'"));
    assert!(
        !rotas.contains("unsafe-inline"),
        "a CSP foi alargada a estilos inline"
    );
}

/// A escala é uma decisão declarada, e não medidas reescritas.
///
/// O risco que este teste fecha é concreto: alguém quer a interface maior,
/// aumenta as medidas do CSS uma a uma, e depois actualiza as assinaturas deste
/// ficheiro para voltarem ao verde. O CSS fica maior, os testes ficam verdes, e
/// a fidelidade ao protótipo deixa de significar seja o que for — porque a
/// expectativa passou a seguir a implementação em vez de a verificar.
///
/// A escala vive num sítio só e não toca em medida nenhuma. Se voltar a `1`, a
/// interface volta exactamente ao tamanho do protótipo.
#[test]
fn a_interface_e_ampliada_por_escala_e_nao_por_medidas_reescritas() {
    let tokens = custom_properties(TOKENS_MD);
    let escala = tokens
        .get("--oc-interface-scale")
        .expect("o dossier tem de declarar a escala de apresentação");

    assert!(
        OCINYE_CSS.contains("zoom: var(--oc-interface-scale)"),
        "a escala tem de ser aplicada na raiz, e não repetida por componente"
    );
    assert_eq!(
        OCINYE_CSS.matches("zoom:").count(),
        1,
        "a escala é aplicada num sítio só; uma segunda aplicação compõe-se com a primeira"
    );
    assert!(
        escala
            .parse::<f32>()
            .is_ok_and(|v| (1.0..=2.0).contains(&v)),
        "escala fora do razoável: {escala}"
    );
}

/// Todo o ecrã que tem cabeçalho tem também o contentor de página.
///
/// `.oc-page` é quem traz `--oc-page-pad`. Sem ele o título encosta à barra
/// lateral enquanto o cartão por baixo respeita a margem — que foi o que
/// aconteceu nos quatro ecrãs do Correio e no da Universal Command Surface.
///
/// O defeito é fácil de reintroduzir porque cada ecrã monta a sua própria
/// árvore: nada obriga o `oc-head` a estar dentro de `oc-page`, e o ecrã parece
/// correcto até ser visto ao lado de outro.
#[test]
fn todo_o_ecra_com_cabecalho_respeita_a_margem_de_pagina() {
    let raiz = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/ui/screens");
    let mut culpados = Vec::new();

    for entrada in std::fs::read_dir(&raiz).expect("ler ecrãs") {
        let caminho = entrada.expect("entrada").path();
        if caminho.extension().is_none_or(|e| e != "rs") {
            continue;
        }
        let fonte = std::fs::read_to_string(&caminho).expect("ler");
        let cabecalhos = fonte.matches(r#"class="oc-head""#).count();
        if cabecalhos == 0 {
            continue;
        }
        let paginas = fonte.matches(r#"class="oc-page"#).count();
        if paginas < cabecalhos {
            culpados.push(format!(
                "{}: {cabecalhos} cabeçalho(s) para {paginas} contentor(es) de página",
                caminho.file_name().unwrap().to_string_lossy()
            ));
        }
    }

    assert!(
        culpados.is_empty(),
        "estes ecrãs têm cabeçalho sem margem de página:\n  {}",
        culpados.join("\n  ")
    );
}

/// O que a barra lateral estreita faz está declarado uma vez só.
///
/// Ela estreita por duas vias independentes: o membro recolhe-a
/// (`data-side="collapsed"`) ou a janela desce abaixo dos 1024px. As regras
/// estavam duplicadas, uma cópia por via, e uma correcção aplicada a uma não
/// chegava à outra — o cabeçalho ficou corrigido no modo recolhido e partido em
/// janela estreita, o que só se via redimensionando.
///
/// A container query pergunta se a barra é estreita em vez de perguntar porquê,
/// e serve as duas. Este teste existe para que a segunda cópia não regresse.
#[test]
fn a_barra_estreita_tem_uma_declaracao_so() {
    assert!(
        OCINYE_CSS.contains("@container (max-width: 120px)"),
        "as regras da barra estreita têm de viver numa container query"
    );
    assert!(
        OCINYE_CSS.contains("container-type: inline-size"),
        "a barra tem de ser um contentor de consulta"
    );

    // O bloco de 1024px define a largura, e nada do que vai por dentro.
    let inicio = OCINYE_CSS
        .find("@media (max-width: 1024px)")
        .expect("o ponto de quebra da barra tem de existir");
    let bloco = &OCINYE_CSS[inicio
        ..inicio
            + OCINYE_CSS[inicio..].find('\n').unwrap_or(0).max(1)
            + OCINYE_CSS[inicio..].find("\n}").unwrap_or(0)];

    for duplicado in [".oc-side__head", ".oc-side__names", ".oc-nav {"] {
        assert!(
            !bloco.contains(duplicado),
            "`{duplicado}` está declarado outra vez no ponto de quebra; \
             uma correcção feita numa das vias não chegaria à outra"
        );
    }
}

/// O rodapé da barra não estende as suas regras ao popover de conta.
///
/// `.oc-side__foot a` foi escrito quando o rodapé tinha três ligações em lista.
/// Depois passou a conter também a superfície de conta, e o seletor descendente
/// foi buscá-la: 30px de altura e o cinzento do navy aplicados a ligações de
/// duas linhas sobre fundo branco.
///
/// Nenhum teste de comportamento apanha isto — o HTML está certo, as rotas
/// resolvem, os botões submetem. Só se vê a olho, e é por isso que fica escrito
/// aqui: um seletor de descendência num contentor que cresceu volta a colidir
/// com o que lá for posto a seguir.
#[test]
fn o_rodape_da_barra_nao_alcanca_o_popover_de_conta() {
    // O alvo é a regra de disposição — altura, tipo e cor de lista. A
    // `.oc-side__foot a:hover { color: inherit }` fica: só anula o dourado
    // global dos links, e é isso que se quer nos dois sítios.
    for seletor in [".oc-side__foot a,", ".oc-side__foot a {"] {
        assert!(
            !OCINYE_CSS.contains(seletor),
            "`{seletor}` dá ao popover de conta a disposição da lista do rodapé; \
             usar `>` para ficar nos filhos directos"
        );
    }
}

/// Cada avatar do catálogo tem um ficheiro que o Workspace serve.
///
/// # A classe de defeito
///
/// O catálogo vive em `ocinye-contracts` e o ficheiro em `static/avatars/`.
/// São dois sítios, e nada os obrigava a concordar: acrescentar um
/// identificador sem entregar a imagem dá uma grelha com um buraco, e entregar
/// uma imagem sem a registar dá um ficheiro que ninguém alcança.
///
/// Nenhuma das duas falha a compilar, e nenhuma falha em tempo de execução —
/// o `<img>` simplesmente não pinta, e o componente cai nas iniciais, que é o
/// comportamento certo para uma imagem partida e o comportamento errado para
/// uma imagem que devia existir.
#[test]
fn cada_avatar_do_catalogo_tem_ficheiro() {
    let pasta = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("static/avatars");

    let mut declarados = std::collections::BTreeSet::new();
    for (id, file) in ocinye_contracts::AVATAR_PRESETS {
        let caminho = pasta.join(file);
        assert!(
            caminho.is_file(),
            "o avatar «{id}» está no catálogo e o ficheiro {file} não existe"
        );
        declarados.insert((*file).to_owned());
    }

    // E o inverso: um ficheiro na pasta que ninguém registou nunca aparece.
    for entrada in std::fs::read_dir(&pasta).expect("a pasta dos avatares") {
        let nome = entrada.expect("entrada").file_name();
        let nome = nome.to_string_lossy().to_string();
        assert!(
            declarados.contains(&nome),
            "o ficheiro {nome} está na pasta e não no catálogo: ninguém o alcança"
        );
    }
}

/// `.oc-avatar` é definido uma só vez.
///
/// # O que se via
///
/// Havia três definições da mesma classe: a do componente e duas anteriores,
/// escritas quando o avatar era só um círculo com iniciais. As antigas vinham
/// depois na folha e ganhavam à do componente — o círculo da topbar continuava
/// a receber o fundo e o tamanho delas, e a fotografia do membro não aparecia
/// lá.
///
/// A mesma pessoa aparecia de duas maneiras no mesmo ecrã: com a fotografia na
/// barra lateral e com as iniciais em cima. Nenhum teste de comportamento
/// apanha isto — o HTML estava certo dos dois lados.
#[test]
fn a_classe_do_avatar_e_definida_uma_so_vez() {
    // O selector de cada regra é o texto imediatamente antes de `{`, e o que
    // interessa é a **última linha** desse texto: antes dela vem o corpo da
    // regra anterior e, muitas vezes, um comentário inteiro.
    let selectores: Vec<String> = OCINYE_CSS
        .split('{')
        .filter_map(|antes| antes.lines().last())
        .map(|linha| linha.trim().to_owned())
        .collect();

    for classe in [".oc-avatar", ".oc-avatar--sm"] {
        // Igualdade exacta, e não «começa por»: `.oc-avatar img` e
        // `.oc-avatar--sm` também começam por `.oc-avatar`, e a primeira versão
        // deste teste contava-os como definições da classe. Um teste que conta
        // mal falha por um motivo que não é o que procura.
        let quantas = selectores
            .iter()
            .filter(|selector| selector.split(',').any(|parte| parte.trim() == classe))
            .count();
        assert_eq!(
            quantas, 1,
            "`{classe}` tem {quantas} definições; a última na folha ganha às outras"
        );
    }
}

/// A topbar não mostra o avatar do membro.
///
/// O dossier põe ali um círculo de 27px com as iniciais, e durante algum tempo
/// foi isso que estava. Era repetição: a identidade vive no rodapé da barra
/// lateral, com o nome e o menu de conta, e a topbar mostrava a mesma pessoa
/// outra vez sem acrescentar nada.
///
/// No lugar dela está o relógio, e a hierarquia fica limpa: a topbar responde
/// «como está o sistema e que horas são», o rodapé responde «quem sou eu aqui».
///
/// Este teste existe para a decisão não se desfazer por distracção — voltar a
/// pôr o avatar ali é reintroduzir a repetição, e deve ser deliberado.
#[test]
fn a_topbar_nao_mostra_o_avatar_do_membro() {
    assert!(
        !OCINYE_CSS.contains(".oc-avatar--bar"),
        "a medida do avatar da topbar voltou; ela já não mostra avatar"
    );
}

/// As mensagens próprias ficam de um lado, e as das outras pessoas do outro.
///
/// # Porque isto é um portão da folha de estilo
///
/// Porque a marca `--minha` está no HTML e é o CSS que decide o lado. Um teste
/// sobre a marcação passaria com a folha a alinhar tudo à esquerda — que é
/// exactamente o estado de que este portão nos tirou.
#[test]
fn as_mensagens_proprias_ficam_do_outro_lado() {
    let css = include_str!("../static/ocinye.css");

    // A **regra**, e não a primeira ocorrência do nome: ele aparece antes
    // dentro de um `:not(...)`, e um `split` posicional lia o bloco errado.
    let inicio = css
        .find(".oc-msg__mensagem--minha {")
        .expect("a regra das mensagens próprias tem de existir");
    let bloco = &css[inicio..css[inicio..].find('}').map_or(css.len(), |f| inicio + f)];

    assert!(
        bloco.contains("justify-items: end"),
        "as mensagens próprias não estão alinhadas ao lado oposto"
    );

    // E não só por cor: a superfície e a largura máxima também as distinguem.
    let regras: String = css
        .match_indices(".oc-msg__mensagem--minha")
        .map(|(i, _)| &css[i..(i + 400).min(css.len())])
        .collect();
    assert!(
        regras.contains("max-width"),
        "uma linha que atravessa o ecrã inteiro deixa de se ler"
    );
    assert!(
        regras.contains("background"),
        "as mensagens próprias precisam de uma superfície própria"
    );
}

/// Nenhuma função chamada no arranque deixa de existir.
///
/// # O defeito que isto guarda
///
/// `start()` chama uma função por ecrã. Uma chamada a uma função que não foi
/// escrita é um `ReferenceError` — e o que ele parte não é só essa: parte
/// **tudo o que vem depois** dela na mesma sequência.
///
/// Aconteceu: uma edição inseriu `initSino()` na lista e falhou a escrever a
/// função, porque a âncora de texto não coincidiu. O JavaScript continuou
/// sintacticamente válido, a página carregou, e a barra lateral, a paleta e o
/// centro temporal deixaram de responder — sem uma linha vermelha em lado
/// nenhum.
#[test]
fn tudo_o_que_o_arranque_chama_existe() {
    let js = include_str!("../static/app.js");

    let inicio = js
        .find("const start = () => {")
        .expect("o arranque tem de existir");
    let fim = inicio
        + js[inicio..]
            .find("\n  };")
            .expect("o arranque tem de fechar");
    let corpo = &js[inicio..fim];

    let chamadas: Vec<&str> = corpo
        .lines()
        .filter_map(|linha| linha.trim().strip_suffix("();"))
        .filter(|nome| nome.starts_with("init"))
        .collect();

    assert!(
        chamadas.len() > 5,
        "o arranque devia chamar vários inicializadores, e encontrou {}",
        chamadas.len()
    );

    let em_falta: Vec<&str> = chamadas
        .iter()
        .filter(|nome| !js.contains(&format!("function {nome}(")))
        .copied()
        .collect();

    assert!(
        em_falta.is_empty(),
        "o arranque chama funções que não existem: {em_falta:?}\n\nUm `ReferenceError` \
         aqui não parte só esta — parte tudo o que vem depois dela."
    );
}

/// Os painéis novos igualam o acabamento do painel da conta.
///
/// # A direcção importa
///
/// O painel da conta é a **referência**, e não um participante. Foi ele que
/// definiu o acabamento — translucidez ligeira, três camadas de sombra, o fio de
/// luz na aresta — e é ele que os outros têm de igualar.
///
/// Este portão nasceu virado ao contrário: mediu que os três partilhavam uma
/// classe, e para isso a primeira escrita reescreveu o painel da conta. Igualar
/// não é mexer em quem já estava certo.
///
/// Por isso a asserção não é sobre classes partilhadas: é sobre os **valores**
/// do painel da conta aparecerem nos outros. Quem mudar a referência tem de
/// mudar os que a seguem, e não o contrário.
#[test]
fn os_paineis_novos_igualam_o_painel_da_conta() {
    let css = include_str!("../static/ocinye.css");

    /// O bloco de uma regra, e não a primeira ocorrência do seu nome.
    ///
    /// O mesmo selector aparece duas vezes: uma regra de uma linha dentro de
    /// uma media query, e a regra a sério. Corta-se em `}` — a chaveta que
    /// fecha a regra — e fica-se com a maior das duas, que é a que declara o
    /// acabamento.
    ///
    /// Cortar em `\n}` seria mais tolerante e é o que estava aqui: a regra de
    /// uma linha estendia-se então até ao fim da media query inteira, ficava a
    /// maior, e o portão passou a ler o bloco errado no dia em que a regra a
    /// sério encurtou. Um portão que lê o sítio errado é um portão que dá
    /// verde por acidente.
    fn bloco<'a>(css: &'a str, seletor: &str) -> &'a str {
        css.match_indices(seletor)
            .filter_map(|(inicio, _)| {
                css[inicio..]
                    .find('}')
                    .map(|fim| &css[inicio..inicio + fim])
            })
            .max_by_key(|bloco| bloco.len())
            .unwrap_or_else(|| panic!("a regra «{seletor}» tem de existir"))
    }

    let referencia = bloco(css, ".oc-account__menu {");
    let partilhada = bloco(css, ".oc-pop {");

    // O acabamento saiu das duas regras e passou a viver nos tokens. Cada uma
    // continua a tê-lo — pelo nome, e não pelo valor.
    for token in [
        "background: var(--oc-pop-surface)",
        "backdrop-filter: var(--oc-pop-blur)",
        "border: 1px solid var(--oc-pop-line)",
        "border-radius: var(--oc-pop-radius)",
        "box-shadow: var(--oc-pop-shadow)",
    ] {
        assert!(
            referencia.contains(token),
            "a referência mudou e este portão ficou obsoleto: «{token}» já não \
             está no painel da conta"
        );
        assert!(
            partilhada.contains(token),
            "os painéis novos não igualam a referência: falta «{token}»"
        );
    }

    // E os tokens continuam a valer o que o painel da conta escreveu. Sem
    // isto, os dois lados podiam concordar num acabamento errado.
    let raiz = bloco(css, ":root {");
    for valor in [
        "--oc-pop-surface: color-mix(in srgb, var(--oc-surface) 88%, transparent)",
        "--oc-pop-blur:    blur(18px) saturate(140%)",
        "--oc-pop-line:    color-mix(in srgb, var(--oc-border) 70%, transparent)",
        "--oc-pop-radius:  13px",
        "0 16px 40px rgba(11,26,45,.20)",
        "0 2px 8px rgba(11,26,45,.10)",
        "inset 0 1px 0 rgba(255,255,255,.60)",
    ] {
        assert!(
            raiz.contains(valor),
            "o acabamento mudou de valor: «{valor}» já não está nos tokens"
        );
    }

    // E os painéis novos usam-na. O da conta **não** — continua com a sua
    // regra, intocada.
    let shell = include_str!("../src/ui/shell.rs");
    let calendario = include_str!("../src/ui/screens/calendar.rs");
    assert!(shell.contains("oc-pop oc-sino__painel"), "o sino não a usa");
    assert!(
        calendario.contains("oc-pop oc-datepop"),
        "o calendário não a usa"
    );
    assert!(
        !shell.contains("oc-pop oc-account__menu"),
        "o painel da conta foi alterado; ele é a referência, e não um participante"
    );
}

/// As regras que a interface toda usa vivem ao nível de topo.
///
/// # O defeito que isto guarda
///
/// Uma regra escrita **dentro** de uma media query só se aplica quando a
/// condição dela é verdadeira. A `.oc-pop` foi lá parar por engano — a inserção
/// procurou o primeiro `.oc-account__menu {` do ficheiro, e o primeiro está
/// dentro de um `@media` para ecrãs estreitos.
///
/// O resultado: a folha de estilo válida, os portões todos verdes, e os painéis
/// sem superfície nenhuma num ecrã normal. Só uma captura o mostrou.
#[test]
fn as_regras_partilhadas_nao_estao_dentro_de_uma_media_query() {
    let css = include_str!("../static/ocinye.css");

    /// A que profundidade de chavetas está a primeira ocorrência.
    fn profundidade(css: &str, seletor: &str) -> i32 {
        let inicio = css
            .find(seletor)
            .unwrap_or_else(|| panic!("a regra «{seletor}» tem de existir"));
        let antes = &css[..inicio];
        i32::try_from(antes.matches('{').count()).unwrap_or(i32::MAX)
            - i32::try_from(antes.matches('}').count()).unwrap_or(0)
    }

    for seletor in [".oc-pop {", ".oc-pop__item {", ".oc-pop__head {"] {
        assert_eq!(
            profundidade(css, seletor),
            0,
            "«{seletor}» está dentro de outro bloco — provavelmente uma media \
             query — e por isso não se aplica no ecrã normal"
        );
    }
}

/// A superfície de um painel escreve-se uma vez, e herda-se sempre.
///
/// # O que este portão resolve
///
/// Nada disto tem valor enquanto a alternativa continuar disponível. Três
/// painéis chegaram ao mesmo acabamento nesta interface, e nas três vezes o
/// caminho foi o mesmo: copiar seis declarações de um sítio para outro. Nas
/// três vezes correu mal de maneira diferente — uma cópia foi parar dentro de
/// uma `@media`, outra reescreveu a referência em vez de a seguir, e a
/// terceira ficou com a superfície certa e o conteúdo por acabar.
///
/// Por isso os valores vivem agora em `--oc-pop-*`, e este portão fecha a
/// porta a quem os volte a escrever à mão: **fora do bloco de tokens, os
/// literais do acabamento não podem aparecer.** Um painel novo escreve
/// `class="oc-pop …"` e fica acabado sem decidir nada.
///
/// # Porque não basta procurar a classe
///
/// Porque um portão que exigisse `.oc-pop` em cada painel seria satisfeito por
/// quem acrescentasse a classe e depois a sobrepusesse. O que se mede é o que
/// falha: o valor repetido.
#[test]
fn a_superficie_de_um_painel_nao_se_reescreve() {
    let css = include_str!("../static/ocinye.css");

    // O bloco de tokens é onde os literais **devem** estar. Tudo o resto da
    // folha é território onde não podem aparecer.
    let fim_da_raiz = css
        .find(":root {")
        .and_then(|inicio| css[inicio..].find("\n}").map(|fim| inicio + fim))
        .expect("o bloco de tokens tem de existir");
    let resto = &css[fim_da_raiz..];

    for literal in [
        "blur(18px) saturate(140%)",
        "0 16px 40px rgba(11,26,45,.20)",
        "inset 0 1px 0 rgba(255,255,255,.60)",
        "border-radius: 13px",
    ] {
        assert!(
            !resto.contains(literal),
            "«{literal}» foi escrito à mão fora dos tokens. A superfície de um \
             painel não se copia: usa-se `.oc-pop`, ou o token `--oc-pop-*` que \
             a declara"
        );
    }
}

/// Nenhum ecrã corrente pede um nome de utilizador.
///
/// # Porque isto não é um grep sobre o repositório
///
/// Porque `username` continua a ser legítimo em dois sítios, e um portão que
/// os apanhasse seria desligado na primeira vez que incomodasse:
///
/// - `autocomplete="username"` é o nome que os gestores de palavras-passe do
///   browser esperam no campo da conta. É convenção do HTML, não conceito do
///   Ocinye, e o ecrã de entrada usa-o **no campo do endereço**.
/// - `preferred_username` é um *claim* do OIDC, guardado para uma federação
///   futura ([ADR-0103](../../docs/adrs/0103-core-owned-authentication.md)).
///
/// Por isso o que se mede é a **superfície**: o texto que uma pessoa lê e os
/// campos que ela preenche. É aí que o conceito reaparece, e foi aí que ele
/// esteve escondido — um campo com a etiqueta «Nome de utilizador», o `pattern`
/// do username e o `name="email"`, ao lado do campo do endereço verdadeiro.
///
/// # O que esse campo fazia
///
/// O `pattern` não admitia `@`. Nenhum endereço válido passava a validação do
/// browser, e **ninguém conseguia criar um membro** por aquele ecrã. Não deu
/// erro em lado nenhum: o botão simplesmente não submetia.
#[test]
fn nenhum_ecra_pede_um_nome_de_utilizador() {
    let mut problemas = Vec::new();

    for caminho in std::fs::read_dir("src/ui/screens").expect("ecrãs") {
        let caminho = caminho.expect("entrada").path();
        if caminho.extension().and_then(|e| e.to_str()) != Some("rs") {
            continue;
        }
        let fonte = std::fs::read_to_string(&caminho).expect("ler");
        let ecra = caminho
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("?")
            .to_owned();

        for (numero, linha) in fonte.lines().enumerate() {
            let limpa = linha.trim();
            // Comentários e documentação falam do passado, e devem poder
            // continuar a falar dele.
            if limpa.starts_with("//") {
                continue;
            }
            let baixa = limpa.to_lowercase();

            if baixa.contains("nome de utilizador") {
                problemas.push(format!(
                    "{ecra}:{} pede «nome de utilizador» a quem lê",
                    numero + 1
                ));
            }
            // Um campo chamado `username` num formulário. O atributo
            // `autocomplete` tem o mesmo texto e é outra coisa.
            if baixa.contains("name=\"username\"") {
                problemas.push(format!(
                    "{ecra}:{} tem um campo `username` no formulário",
                    numero + 1
                ));
            }
        }
    }

    assert!(
        problemas.is_empty(),
        "o nome de utilizador voltou à superfície:\n  {}",
        problemas.join("\n  ")
    );
}
