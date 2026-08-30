# ADR-0604 — Apresentação de acesso e autorização contextual no Workspace

- **Estado:** Accepted
- **Domínio:** Workspace
- **Impacto:** FOUNDATIONAL
- **Data:** 2026-08-30
- **Relaciona-se com:** [ADR-0602](0602-workspace-ssr-progressive-enhancement.md) ·
  [ADR-0100](0100-authorization-from-institutional-state.md) ·
  [ADR-0204](0204-institutional-files-and-folders.md)

## Context

A barra lateral do Workspace escondia **Ficheiros** a quem tinha todo o direito
de lá entrar. A causa não era um bug de renderização: era uma segunda política
de autorização, escrita na interface, a decidir por conta própria.

O filtro perguntava se a pessoa tinha uma permissão **em toda a organização**.
Mas o direito de ver ficheiros de investigação é **contextual** — nasce da
pertença a uma unidade ou a um ambiente. Quem tinha o direito exactamente onde
ele existe não o tinha em lado nenhum segundo a pergunta que a barra fazia, e o
módulo desaparecia.

Duas correcções erradas estiveram em cima da mesa e foram recusadas:

- **Alargar a permissão global.** Resolvia o sintoma criando autoridade que não
  devia existir.
- **Devolver `allowed: true` sem contexto.** Seria a mesma mentira com outro
  nome: um booleano sem `workspace_id` não responde «pode?», responde «talvez».

## Decision

> **O Core decide a autoridade. O Workspace apresenta-a. A interface nunca pode
> inventar uma segunda política de autorização.**

E, para que a navegação deixe de ser confundida com autoridade:

> **A relevância de um módulo responde se uma capacidade pertence ao trabalho
> institucional daquela pessoa. A autorização de um recurso responde o que essa
> pessoa pode de facto ver ou fazer. A relevância nunca concede autoridade.**

São **dois eixos**, e nunca uma tabela só:

| | pergunta | fonte | consequência |
|---|---|---|---|
| **Relevância** | «este módulo pertence ao trabalho desta pessoa?» | papel técnico | o módulo aparece na navegação |
| **Autorização** | «esta pessoa pode ver ou fazer isto, **aqui**?» | estado institucional no contexto | a operação é aceite ou recusada |

Uma tabela única onde «Ficheiros = true» seria lida como ACL daqui a seis meses.
É por isso que a matriz de personas é um artefacto executável com os dois eixos
separados, e que o teste que os separa escolhe deliberadamente uma persona para
quem a relevância é `false`: com uma persona já relevante, um colapso dos eixos
passa despercebido porque `true` continua `true`.

### Corolários

**A ausência de um controlo nunca é a defesa.** Um formulário que não aparece
poupa uma recusa a quem não podia; não impede nada a quem escrever o `POST` à
mão. Cada controlo condicionado tem uma viagem que exige que o Core recuse a
mesma operação por HTTP directo.

**A cortesia de renderização faz a mesma pergunta que a operação.** Quando o
ecrã decide mostrar um controlo, pergunta ao Core exactamente o que a operação
perguntará (`may_manage_members` nasce do mesmo `authorize(ManageMembers, …)`
que `add_workspace_member` corre). Duas perguntas diferentes divergiriam, e a
interface passaria a prometer o que o Core recusa — ou a esconder o que
concede.

**A autoridade lê-se do estado autoritativo a cada pedido.** Conceder e revogar
uma pertença vêem-se na **mesma sessão viva**, sem reentrada e sem reinício. Uma
sessão que guardasse a sua cópia da autoridade seria, outra vez, uma segunda
política.

**Criar um recurso estabelece a autoridade mínima para o continuar a operar** —
nunca auto-elevação arbitrária. Quem cria uma unidade fica seu gestor; quem cria
uma ideia fica líder do ambiente. E o inverso tem invariante: a última pessoa que
governa não pode ser removida sem que outra a substitua.

**Administrar a plataforma não é fazer investigação.** `PlatformAdmin` não torna
os módulos de investigação relevantes, nem dá leitura de material RESTRICTED.

## Consequences

- A navegação e a autorização passam a ter fontes distintas e testes distintos.
- `Me` expõe `modules: [{module, relevant, authorization_scope}]` — e
  deliberadamente **não** um campo `allowed`, que sem contexto não teria
  significado.
- Cada superfície com controlo condicionado ganha duas viagens: uma que exige o
  controlo a quem pode, outra que exige a recusa do Core a quem não pode.
- Uma conta suspensa perde autoridade a meio da sessão. A propriedade está
  defendida em quatro camadas independentes — revogação de sessões, releitura do
  estado em `CurrentSession`, `is_active` em `CurrentPrincipal`, e a política de
  domínio — e cada uma basta sozinha.

## Alternativas recusadas

**Uma lista de módulos permitidos servida pelo Core.** Parece o mesmo e não é:
seria uma ACL de navegação, e a primeira vez que divergisse da política real a
interface passaria a ser a autoridade.

**Calcular a relevância a partir das pertenças.** Colapsaria os dois eixos num
só. Quem faz investigação conhece o espaço onde ela acontece, tenha ou não
trabalho atribuído hoje; e quem tem uma pertença administrativa não passa por
isso a fazer investigação.
