# Matriz de contratos de comportamento (UI Reset)

> Cada teste que olhava para a apresentação legada, e o que lhe aconteceu. Um teste
> nunca desaparece em silêncio: ou passa a olhar para o comportamento por um
> marcador `data-oc`, ou fica aqui como **à espera da UI nova** — e o pacote do
> Claude Design que substituir o ecrã traz a medição de volta, sobre o desenho novo.

| Teste | O que media | Destino |
|---|---|---|
| `o_sino_abre_um_painel_com_o_que_chegou` (browser) | abre sem navegar, desenha a notificação, fecha com `Escape` | **convertido** — `[data-oc="notificacao"]`, título e legenda |
| idem | superfície igual à do painel da conta; ícone por tipo | **à espera da UI nova** — acabamento da UI legada |
| `design_fidelity.rs` — 24 testes: tokens do dossier no CSS, cores de marca, sprite ↔ catálogo de ícones, anel de foco, z-index, durações, cores escritas à mão, dimensões da shell, animações, tipografia IBM Plex, movimento reduzido, medidas dos componentes, autofill, escala, margem de página, barra estreita, rodapé vs popover, classe do avatar, avatar fora da topbar, mensagens próprias à direita, painéis iguais ao da conta, regras fora de media query, superfície de painel | a folha `ocinye.css` e o sprite `icons.svg` legados | **à espera da UI nova** — cada pacote D1–D11 traz as suas medições sobre `ocinye-ds.css` |
| `design_fidelity.rs` — 5 testes | CSP sem `style` inline; CSP sem `unsafe-inline`; catálogo de avatares ↔ ficheiros; arranque do `app.js` sem chamadas partidas; nenhum ecrã pede nome de utilizador | **preservados** em `tests/ui_contract.rs` (portão `UI Contract`) |
| `scripts/rendered_value_equivalence.py` (portão `Rendered-Value Equivalence`) | tokens expandidos devolvem o CSS anterior | **à espera da UI nova** — media a consolidação da folha legada |
| `a_consolidacao_nao_mudou_o_que_a_pessoa_ve` (browser) | estilo computado igual ao da folha anterior à consolidação | **à espera da UI nova** |
| `capturas_do_calendario`, `capturas_dos_paineis_da_barra`, `capturas_do_correio`, `capturas_da_ciencia` (browser, `#[ignore]`) e `scripts/capturas.sh` | PNGs para revisão visual humana, e o acabamento igual dos três painéis da barra | **à espera da UI nova** — a revisão visual é do Claude Design |
| `despejar_lancador_para_verificacao_visual` (unitário, `#[ignore]`) | o lançador aberto com a folha legada, para inspecção | **à espera da UI nova** |
| `a_largura_da_barra_e_declaravel_para_qualquer_valor` (unitário) | as 101 regras `[data-pct]` na folha legada | **à espera da UI nova** — a barra continua a levar `data-pct` e `aria-valuenow` |
| `o_subtitulo_concorda_em_numero`, `o_home_nao_oferece_criar_ideia_a_quem_nao_pode`, `com_a_permissao_o_home_leva_ao_formulario`, `cada_accao_indisponivel_diz_a_sua_propria_razao` (unitários, Home) | o resumo por baixo da saudação e o botão «Nova Ideia» da Home legada | **retirados com o D4** — o Desktop não tem saudação nem botões de criar; criar é o «+ Criar» da barra (D2), que não oferece o que a Instância desactivou |

## Viagens de geometria: vermelhas sem folha de estilo, critério de aceitação da UI nova

Sem CSS nenhum, cinco viagens falham porque medem geometria que só existe com
apresentação. O comportamento por trás delas continua lá (o teclado move o
separador, o botão alterna o estado expandido, a lista tem o seu contentor). Não
se apagam nem se saltam: voltam a verde com o pacote que as cobre, e são o
critério de aceitação desse pacote.

| Viagem | Mede | Volta com |
|---|---|---|
| `a_pessoa_arruma_o_correio_e_nao_o_parte` | as setas movem o separador da disposição | D8 → Q-23 (o D12 não traz a regra das colunas) |
| `o_compositor_obedece_e_guarda_o_que_se_escreveu` | expandir alarga o compositor | D8 → Q-24 |
| `o_correio_rola_por_dentro_e_nao_por_fora` | a lista rola dentro do seu contentor | D8 → Q-23 |
| `o_arranque_cabe_num_ecra_pequeno` | nada transborda a 390 px | **verde** com o D3 |
| `o_separador_de_navegacao_activo_e_azul_branco_sem_dourado` | activo = superfície azul, texto branco | voltou a passar: a superfície do D1 é um gradiente navy e a medida lê o `background-image`; o scrollspy passou a mover também o `aria-selected` |
| `o_detalhe_do_membro_marca_a_seccao_e_atribui_uma_unidade` | o separador clicado fica activo; o scrollspy decide pela geometria das secções, que sem estilo ficam todas na mesma banda | voltou a passar com as primitivas D1 |
