# Runbook — Ligar o primeiro nó Compute/Intelligence

> **Estado: `PLANNED`.** Nenhum nó existe (`CLAUDE.md` §7). Este runbook é o
> ponto de partida da milestone `CONNECT OCINYE AI / COMPUTE NODE`. O **contrato**
> está fechado ([ADR-0502](../adrs/0502-compute-intelligence-connection-contract.md),
> [ADR-0500](../adrs/0500-compute-registry-node-agent.md),
> [`docs/node-protocol/`](../node-protocol/README.md)); o que falta é executá-lo
> contra um nó real. Nada aqui liga um fornecedor externo para simular IA (§41).

## Antes de começar

- Um servidor com GPU, provisionado e actualizado. Não aceita tráfego público de
  aplicação (ADR-0500).
- Acesso de administração ao Ocinye OS como identidade privilegiada (MFA).
- A raiz de selagem e as credenciais de produção **não** vão para o nó.

## Passos

1. **Provisionar o nó.** Sistema base, relógio sincronizado, disco. Nenhum porto
   de aplicação exposto à Internet.

2. **Estabelecer a confiança de rede (WireGuard).** Túnel `VPS ↔ nó`, o nó a
   ligar **para fora**. Fora do túnel o nó é inalcançável. (mTLS só se o
   transporte deixar de ser WireGuard — ADR-0502 §1.)

3. **Registar o nó** no Compute Registry:

   ```text
   POST /api/v1/compute/nodes   {identifier, location, …}
   → {node_id, enrollment_token}      # o token é devolvido UMA vez
   ```

   Estado inicial: `pending_enrollment`. Só o digest do token é guardado.

4. **Instalar o Node Agent e enrolar.** O agente troca o token de uso único por
   uma credencial própria:

   ```text
   POST /api/v1/compute/enroll   {enrollment_token}
   → {agent_token}
   ```

   A credencial é rotacionável e revogável (ADR-0502 §2). Nunca reutiliza
   credenciais humanas.

5. **Heartbeat.** O agente reporta saúde, recursos e capacidades:

   ```text
   POST /api/v1/compute/heartbeat   (x-ocinye-node-token)
   ```

   `online`/`offline` derivam de `last_seen_at` — ninguém os declara.

6. **Activar a capacidade.** O relato do nó é dado não confiável até ser provado
   (ADR-0502 §3):
   - **Intelligence** → a Provider Conformance Suite (ADR-0305) tem de passar
     antes de a capacidade servir tráfego pelo AI Gateway (ADR-0300).
   - **Compute** → um job de verificação controlado.
   - Só ao passar é que `Compute`/`Intelligence` deixam de ser
     `EXPECTED_PENDING_AI` e ficam `AVAILABLE`.

7. **Teste controlado.** Uma inferência/job real, pelo contrato canónico
   (ADR-0304), com timeout do Core e falha tipada. Sem expor conteúdo nem
   segredos.

8. **Confirmar o estado honesto.** A Experience deixa de mostrar «IA e computação
   aguardam a ligação do primeiro nó» e passa a mostrar a capacidade disponível,
   com a sua proveniência.

## Reversão / retirada

`draining` → `retired` pela administração; a credencial revoga-se; a liveness
trata o nó como `offline` sem flag. Retirar um nó não é um rewrite — é uma linha
de estado.

## O que este runbook não faz

Não integra modelos, não liga fornecedores externos, não gera credenciais
definitivas de um nó que ainda não existe. Fá-lo-á, passo a passo, no dia da
ligação.
