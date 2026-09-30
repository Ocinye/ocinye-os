# ADR-0624 — Predefinições de Distribuição: configuração de produto tipada, versionada, abaixo da Instância e sem autoridade

- **Estado:** Accepted
- **Domínio:** Workspace
- **Impacto:** HIGH
- **Depende de:** [ADR-0014](0014-instance-profiles-and-application-activation.md) · [ADR-0016](0016-application-manifest-contract.md) · [ADR-0017](0017-instance-configuration-and-branding.md)
- **Data:** 2026-09-30

## Context

Até à D008, a experiência inicial de uma Instância era a mesma em qualquer
Distribuição, com duas excepções escritas em sítios diferentes: as fixações do
manifesto mais «O Meu Trabalho» só em Research, e uma disposição do Desktop
por Distribuição dentro da vista, com os avisos institucionais obrigatórios —
um widget sem fonte no Core (FG-013), portanto um cartão sempre indisponível
em todos os Desktops. Uma Distribuição desconhecida caía em Research.

A D009 (Claude Design) dá a cada uma das quatro Distribuições — Research,
Business, Personal, Education — um ponto de partida próprio: fixações, widgets,
fundo e primeiros passos. A questão arquitectural é onde isso vive, como se
relaciona com a Instância e com o membro, e o que não pode fazer.

## Decision

1. **Configuração de produto, tipada.** As predefinições de cada
   Distribuição vivem em `experience::distribution::DEFAULTS`: ids de
   aplicação e tipos de widget tipados, versionados por
   `DISTRIBUTION_DEFAULTS_VERSION`, validados por testes (existem, são
   fixáveis, estão activos na Distribuição, cabem, passam a validação do
   Core). Nunca vêm de ficheiro, caminho ou JSON.
2. **Hierarquia: membro ?? Instância ?? Distribuição ?? sistema.** A
   disposição gravada pelo membro ganha a tudo. A predefinição da Instância
   ganharia à da Distribuição — **mas ainda não existe** (FG-014,
   `INSTANCE_DEFAULT = NOT_IMPLEMENTED`); o ponto de extensão é
   `distribution::restore_target`, e não há uma Instância vazia a fingir. «Repor
   predefinição» apaga a linha do membro e volta à predefinição efectiva, hoje a
   da Distribuição, e a folha diz de onde vem.
3. **Uma Distribuição desconhecida cai na predefinição mínima do sistema** —
   sem widgets, fundo `ocinye`, as fixações do manifesto — **nunca em
   Research**. Um Desktop sem widgets fica vazio: sem cartão de boas-vindas.
4. **Uma actualização do produto não reescreve nada gravado.** Quem
   personalizou fica como está; quem nunca personalizou segue a versão nova.
5. **Nenhum widget é obrigatório** enquanto não houver fonte de dados para ele:
   os avisos institucionais saem de todas as predefinições e da biblioteca; quem
   já os tinha pode retirá-los.
6. **Nada disto é autorização.** `DEFAULT ≠ AUTHORIZED`, `NOT DEFAULT ≠
   FORBIDDEN`, `PINNED ≠ AUTHORIZED`, `WIDGET PRESENT ≠ AUTHORIZED`. As
   fixações passam pelo mesmo filtro de visibilidade que o lançador (uma que
   o membro não pode abrir sai da barra e a seguinte ocupa o lugar; nada se
   grava em nome dele); um widget cuja aplicação o membro não vê, ou que o
   Core recusa, **esconde-se mas fica na disposição**, para que gravar não o
   apague. A activação das aplicações continua a do perfil (ADR-0014),
   inalterada.
7. **A barra de aplicações segue a ordem das fixações** (do membro, ou da
   Distribuição), já filtrada; o lançador continua pela ordem do registo.
8. **Os primeiros passos são a pedido**, no painel do distintivo da
   Distribuição: sem assistente, sem abrir sozinho, sem estado «visto» — não há
   contrato que o grave, e o navegador não é autoridade de produto.

## Emenda R2 (D009 R2, 2026-09-30)

O pacote D009 R2 do Claude Design corrige o modelo: **uma instalação →
Instância → Distribuições activadas [1..4] → Distribuição activa → contextos
dentro do Desktop**. A D009 não implementa esse modelo; fixa o que o código de
hoje não pode fingir nem aprofundar.

- **R2.1** Uma Instância activa uma ou mais Distribuições. As predefinições
  são indexadas pelo **tipo** de Distribuição, e todo o código novo recebe a
  Distribuição activa como argumento — nenhum assume uma por Instância.
- **R2.2** Âmbito-alvo da disposição, do fundo e das fixações do membro:
  **membro + Instância + Distribuição**. O âmbito actual é `person_id`
  (`member_desktop_layouts`, `member_app_pins`) e só é válido com uma
  Distribuição activa (G9-30/31, bloqueante da D010).
- **R2.3** A predefinição da Instância, quando existir, é por **Instância +
  Distribuição**, nunca uma só para todas. «Repor» nunca devolve a de outra
  Distribuição.
- **R2.4** Pontos de acesso (genérico e fixo), domínio próprio, TLS, escolha
  e mudança de Distribuição e contexto activo são D010, com ADR própria.
- **R2.5 (verdade do repositório, Code).** `organisations.profile` guarda um
  valor: activar uma segunda Distribuição é **estruturalmente impossível** hoje
  (`SECOND_DISTRIBUTION_ENABLEMENT = BLOCKED`). Existe
  `PUT /instance/profile` no Core (`organisation.manage`, auditado), que
  **substitui** a Distribuição — não acrescenta — e que a Workspace não expõe;
  a disposição gravada do membro atravessa essa troca, como antes da D009.

## Alternatives

| Alternativa | Porque não |
|---|---|
| Predefinições por Distribuição em JSON configurável | Um blob sem tipos aceita ids que não existem e aplicações inactivas; a validação passaria a ser em tempo de execução, na casa de quem instala. |
| Uma predefinição da Instância vazia, para já, a preencher a hierarquia | Seria uma camada que diz existir sem existir; «Repor» afirmaria voltar a algo que ninguém publicou. |
| Distribuição desconhecida → Research | Transforma um erro de dados na experiência de outra instituição, com os seus widgets de investigação. |
| Retirar da disposição um widget que o membro deixou de poder ver | A autoridade muda e volta; apagar na gravação perderia a escolha do membro por causa de um estado temporário. |
| Mostrar a fixação desactivada | Um ícone morto na barra; e revela uma aplicação que o membro não abre. |

## Consequences

- `ocinye_contracts::desktop::WIDGET_KINDS`: `notice` deixa de ser obrigatório;
  `WALLPAPERS` ganha `field`, `module`, `calm`, `lattice` (um por Distribuição).
- `apps::default_pins_for` delega em `distribution::default_pins`; o caso
  especial «research + work» passou a dado.
- `ShellVm.pin_order`; `controllers::desktop` esconde os widgets de aplicações
  invisíveis ao membro; a biblioteca só oferece o que o membro vê.
- Provas: `apps/workspace/tests/d009_journeys.rs` (Instâncias novas nas quatro
  Distribuições, pelo caminho de `resolve_instance`) e `d009_contracts.rs`.
- Em aberto: a predefinição da Instância (FG-014) e a proveniência gravada da
  disposição do membro (G9-04); mudar a Distribuição depois de criada
  (G9-21, fica só leitura).
