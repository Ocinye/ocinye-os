# ADR-0625 — Distribuição antes do contexto; mudar de Distribuição ≠ mudar de contexto

- **Estado:** Accepted
- **Domínio:** Workspace
- **Impacto:** HIGH
- **Depende de:** [ADR-0019](0019-multi-distribution-instance.md) · [ADR-0020](0020-access-endpoints.md) · [ADR-0618](0618-window-manager.md) · [ADR-0613](0613-ocinye-deep-link-protocol.md)
- **Data:** 2026-09-30

## Decision

1. **Ordem:** ponto de acesso → autenticação → Distribuição (0 / 1 / várias, ou fixa pelo ponto) →
   Desktop → contexto. A entrada nunca escolhe Organização, Unidade, Projecto nem Espaço pessoal.
2. **Contexto vive dentro da Distribuição activa** e escolhe-se no Desktop (chip de contexto). Só se
   mostram contextos que o Core autoriza e que o domínio tem (Organização, Unidade, Projecto, Espaço
   pessoal). Sem contexto necessário, não se fabrica um. Equipa, Turma e Departamento não existem.
3. **Dois controlos distintos:** distintivo da Distribuição (ícone D009; painel com primeiros passos e
   mudança de Distribuição) e chip de contexto (tipo + nome). Nunca o mesmo menu.
4. **Mudar de Distribuição** só aparece com mais de uma acessível. Num ponto genérico, fica no mesmo
   anfitrião e o Core reautoriza. Num ponto fixo, não muda localmente: oferece o ponto configurado da
   outra Distribuição (ou o genérico) ou diz que não é possível daqui.
5. **Confirmação normal** (não privilegiada): diz a Distribuição de destino, quantas janelas fecham e
   quais têm alterações por gravar. Sem palavra escrita.
6. **Janelas:** a mudança fecha as janelas da Distribuição anterior pelo Window Manager existente
   (ADR-0618). Cada janela suja passa pelo `dirty_close` do D002 (ALLOW / REQUIRE_CONFIRMATION); se
   o membro cancelar uma, **a mudança aborta** e nada fecha.
7. **Depois da mudança:** contexto resolvido de novo na Distribuição destino (nunca transportado);
   dados em cache, listas e resultados descartados; o histórico do browser recebe uma entrada nova de
   Desktop, e «atrás» para uma rota da anterior é revalidado no servidor (recusa → Desktop actual).
8. **Ligação profunda** para um recurso de outra Distribuição: o Core resolve o recurso e a
   Distribuição de destino; com acesso → pede mudança (S34); sem acesso ou inexistente → recusa sem
   revelar (mesma mensagem nos dois casos). Um id válido no URL nunca expõe dados.
9. **Última Distribuição/contexto:** fora do âmbito até haver contrato de preferência; se vier, é só
   preferência, revalidada em cada sessão.
