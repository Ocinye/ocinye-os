# ADR-0013 — O Ocinye OS como ambiente operativo de uso geral: Instância e Nó

- **Estado:** Accepted
- **Domínio:** Foundation
- **Impacto:** FOUNDATIONAL
- **Depende de:** [ADR-0001](0001-ocinye-os-definition.md)
- **Data:** 2026-09-26

## Context

A [ADR-0001](0001-ocinye-os-definition.md) decidiu que o Ocinye OS é um sistema
operacional institucional e não uma colecção de aplicações. Decidiu-o **para a
Ocinye**: a definição, o `CLAUDE.md` e partes do código assumem uma só
instituição — uma variável de ambiente cujo valor por omissão era o slug
`ocinye`, o nome da organização igual ao slug, «Ocinye» escrito no emissor TOTP,
na assinatura de correio e na instrução de sistema da IA
([sistema actual](../architecture/CURRENT_SYSTEM.md#16-pressupostos-de-uma-só-organização)).

O programa de generalização muda o âmbito: o Ocinye OS passa a ser um ambiente
operativo auto-alojado para pessoas e organizações quaisquer — empresas,
universidades, institutos, instituições públicas, pessoas individuais. A
instalação da Ocinye passa a ser a primeira instância real desse sistema.

O esquema já tinha preparado isto sem o dizer: a tabela `organisations` existe
desde a migração 0001 («single-tenant today; modelled explicitly so scope is
never implicit»), todo o domínio se delimita por `organisation_id`, e a
autorização recusa atravessá-lo. Faltava declarar a que organização uma
instalação serve, e retirar as suposições de que é a Ocinye.

## Decision

**1. O Ocinye OS é um ambiente operativo auto-alojado de uso geral.** Unifica
identidade, aplicações, dados, armazenamento, computação, governação de
recursos, comunicação, colaboração, automação e IA sob um Core autoritativo, e
corre sobre um Linux mínimo que continua dono do kernel, dos drivers e das
primitivas. A Ocinye é a primeira instância, e o sistema não se define por ela.

**2. Uma Instância é um ambiente Ocinye OS governado de forma independente.**
É dona do seu reino de identidade, membros, configuração, aplicações, dados,
políticas, segredos, recursos, nós e fornecedores de IA. Na base de dados, a
Instância **é** a organização que a governa — a mesma linha de `organisations`
que já delimita tudo —, e a instalação regista qual é numa tabela singleton,
`instance_identity`, cujo `id` é a identidade durável da Instância: a mesma
quando ela muda de servidor.

**3. Uma instalação serve exactamente uma Instância.** Este é o modelo por
omissão e o único suportado. **Não é multi-tenancy**: nenhum código assume que
duas instâncias partilham uma base, e o isolamento que existe entre organizações
na mesma base (usado pelos testes) é defesa em profundidade, não um produto.

**4. O Core resolve a sua Instância assim, e nunca adivinha:**

```text
OCINYE_INSTANCE_SLUG definido (ou o nome antigo, OCINYE_ORGANISATION_SLUG)
  → adopta essa organização, ou cria-a se a base não tem Instância
senão, a Instância registada em instance_identity
senão, a única organização da base, se houver exactamente uma
senão → recusa arrancar, e diz o que definir
```

A primeira resolução fica registada; daí em diante o arranque é determinístico.
O slug é identidade técnica (prefixo das chaves de objecto), escolhido na
criação e não renomeado; o **nome** é o que as pessoas lêem, e é da Instância.

**5. Uma Instância usa um ou mais Nós.** Um Nó é uma máquina ou ambiente de
computação que contribui recursos — servidor físico, VM, máquina de cloud,
estação de trabalho, host de GPU. `Instância ≠ Nó`, e «servidor» não se usa como
sinónimo de nenhum dos dois. O modelo de Nó é da Parte 5 do programa; esta ADR
fixa a distinção.

**6. Recursos reportados por um nó pertencem à Instância do nó.** O inventário
de modelos (`ai_models`) não tem organização própria; o âmbito vem de
`compute_nodes.organisation_id`, e toda a leitura o aplica.

**7. A identidade da organização vem da Instância, não do código.** O nome da
Instância aparece onde antes estava «Ocinye» escrito: emissor TOTP, linha
institucional da assinatura de correio, instrução de sistema da IA, topo do
Workspace. «Ocinye OS» continua a nomear o **produto**; a marca de instância
(logótipo, cores) é da Parte 14.

## Alternatives

**Uma tabela `instances` nova, acima de `organisations`.** Separaria dois
conceitos que hoje coincidem um para um, e obrigaria a migrar setenta
referências de `organisation_id` para ganhar uma indirecção sem uso. Se um dia
uma Instância governar várias organizações, é aí que a tabela nasce, com a
necessidade real que hoje não existe.

**Multi-tenancy agora.** Recusado pelo programa e por esta ADR: o produto é
auto-alojado, e cada organização controla a sua infraestrutura. Uma instalação
partilhada seria outro produto, com outro modelo de ameaças.

**Manter `ocinye` como valor por omissão.** Custava zero hoje, e é exactamente o
pressuposto que torna o sistema um produto de uma só organização.

## Consequences

- Uma instalação nova já não arranca por omissão como «ocinye»: ou se diz qual é
  a Instância, ou o `bootstrap-admin` a cria com o nome que lhe é dado.
- A instalação existente mapeia sem perdas: a migração 0052 regista a única
  organização como a Instância, e corrige o nome das organizações cujo nome era o
  slug (o defeito do bootstrap).
- O isolamento do inventário de IA entre organizações passa a ser uma prova
  (`crates/ocinye-core/tests/instance_isolation.rs`), e não uma coincidência de
  haver uma só.
- A definição canónica em [`docs/architecture/`](../architecture/README.md) passa
  a ser a de uso geral, e o guarda `scripts/documentation-facts.py` protege a
  nova formulação.
