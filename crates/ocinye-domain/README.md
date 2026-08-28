# `ocinye-domain`

Invariantes institucionais do Ocinye OS: workflows e política de autorização.

## Finalidade

As regras que têm de valer independentemente de transporte, persistência ou
apresentação: quem pode fazer o quê, e que transições de estado são legítimas.

## Porque é puro

Nada aqui faz I/O. Uma política que só se consegue exercitar com uma base de
dados é uma política que não será testada exaustivamente — e a autorização é
precisamente onde a exaustividade compensa.

Por ser puro, `policy::tests` consegue **enumerar** todas as combinações de
classificação, papel e membership em vez de amostrar algumas.

## Responsabilidades

| Módulo | O quê |
|---|---|
| `policy` | RBAC + regras contextuais, *fail closed*. A função de decisão. |
| `policy::visibility` | O espelho da regra de leitura para listagens e pesquisa. |
| `workflow::idea` | Ciclo de vida da ideia, incluindo as saídas honestas. |
| `workflow::project` | Ciclo de vida do projecto. |
| `workflow::task` | Ciclo de vida da tarefa. |
| `identifiers` | Validação de códigos institucionais e identificadores de nó. |
| `principal` | Quem age, e em que contexto institucional. |

## O invariante mais importante

A regra de leitura existe **duas vezes**: como decisão sobre um recurso já
carregado, e como conjunto de predicados sobre linhas. Duas implementações que
têm de concordar são um risco permanente.

Este crate elimina o risco dando ao filtro semântica executável
(`VisibilityFilter::permits`) e afirmando a equivalência contra a política em
`visibility_filter_agrees_with_the_read_policy_exhaustively`. Altere um dos lados
sozinho e o teste falha.

## Limites

**O que não pertence aqui:** SQL, HTTP, configuração, segredos. O Core traduz
`VisibilityFilter` em SQL; este crate define o que esse filtro *significa*.

`Principal` **não** transporta posição institucional. A posição não concede nada,
logo a política não tem nada que a ver.

## Dependências

`ocinye-contracts`, `serde`, `uuid`, `thiserror`. Sem runtime assíncrono, sem
base de dados, sem rede.

## Execução e testes

```bash
cargo test -p ocinye-domain
```

33 testes. Entre eles:

- nenhum papel técnico isolado abre `RESTRICTED`;
- um membro inactivo é negado em todas as acções e classificações;
- uma leitura negada esconde a existência; uma escrita negada sobre recurso
  legível não;
- exportar `RESTRICTED` é mais restrito do que o ler;
- promoção não é alcançável como transição ordinária;
- fechar uma ideia exige um motivo registado.

## Segurança relevante

Este crate **é** a superfície de segurança do sistema. Qualquer alteração a
`policy` exige revisão explícita e ADR se mudar a regra.
