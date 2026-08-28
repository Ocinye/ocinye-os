# Como acrescentar uma Capability ao Ocinye OS

Uma capability é uma **porta publicada sobre um serviço que já existe**. Não é
uma segunda forma de entrar com regras próprias.

Se estiver a escrever SQL num handler, ou a repetir uma verificação de
autorização, a capability está no sítio errado.

## Antes de escrever código

Responda a estas. Se alguma não tiver resposta, a capability ainda não está
pronta para existir.

**Que serviço de domínio detém a invariante?** Se nenhum, o trabalho é criar o
serviço primeiro. Um handler que chegue à base de dados directamente é uma regra
que só se aplica quando um agente pergunta.

**Que permissão nomeada a governa?** Tem de existir no catálogo `Permission` e
tem de ser a **mesma** que o serviço verifica. Um descriptor a declarar uma
permissão e um serviço a verificar outra é uma discrepância que ninguém nota até
ser explorada.

**É delegável a agentes?** `is_delegable_to_agents` recusa gestão de permissões,
papéis, membros, administração da plataforma, infraestrutura de IA, computação e
correio. **O registry não arranca** se violar isto.

**Qual é o risco, honestamente?**

| Nível | Quando |
|---|---|
| `ReadOnly` | Não altera nada |
| `LowImpact` | Alteração pequena e reversível |
| `MaterialMutation` | Alteração institucional relevante |
| `ExternalEffect` | Alguém fora da instituição vê o resultado |
| `Privileged` | Segurança, privilégio, ou irreversível |

Na dúvida, o mais alto. Um nível a mais custa uma confirmação; um a menos custa
uma acção que ninguém autorizou.

**É reversível?** `Reversible` só quando existe forma de desfazer **pela
interface**. Prometer Undo onde não há é pior do que não oferecer nada.

**Faz sentido simular?** `supports_dry_run` só quando descrever o efeito
acrescenta alguma coisa. Simular uma leitura é a leitura.

## O ficheiro

`crates/ocinye-core/src/modules/agentic/capabilities/<domínio>.rs`.

```rust
pub struct CreateThing;

#[async_trait]
impl CapabilityHandler for CreateThing {
    fn descriptor(&self) -> CapabilityDescriptor {
        CapabilityDescriptor {
            id: CapabilityId::new("domínio.coisa.create"),
            domain: "domínio".to_owned(),
            summary: "Uma frase que um membro leia.".to_owned(),
            permission: Permission::CoisasCreate,
            scope: Scope::Unit,
            risk: RiskLevel::LowImpact,
            approval: ApprovalRequirement::Never,
            max_autonomy: AutonomyLevel::Workflow,
            reversibility: Reversibility::Reversible,
            supports_dry_run: true,
            classification_ceiling: None,
            input_schema: serde_json::json!({
                "type": "object",
                "required": ["title"],
                "properties": { "title": {"type": "string"} }
            }),
        }
    }

    async fn execute(&self, ctx: &ExecutionContext<'_>) -> CoreResult<CapabilityResult> {
        let title = ctx.text("title")?;

        if ctx.dry_run {
            return Ok(/* … descreve, não faz … */);
        }

        // O serviço detém a invariante. O handler não a repete.
        let thing = dominio::create_thing(&mut tx, ctx.principal, ctx.ids, /* … */).await?;

        Ok(CapabilityResult { /* … com um `ResourceRef` do que criou … */ })
    }
}
```

Registe-a em `capabilities/mod.rs`.

## O identificador é permanente

`domínio.substantivo.verbo`, minúsculas, pontos. Aparece em linhas de auditoria,
em definições de agente e em aprovações. Renomear um é uma alteração
incompatível para tudo o que o referenciou.

## O que o handler nunca faz

- **Não autoriza.** Quando corre, o executor já decidiu.
- **Não valida contra o esquema.** Já foi validado.
- **Não escreve SQL.** Chama o serviço.
- **Não alcança infraestrutura.** Não há shell, ficheiros, rede nem segredos no
  `ExecutionContext`, e não os deve procurar.
- **Não devolve sucesso quando não houve.** Um `Err` do serviço torna-se o
  estado certo; inventar um `Succeeded` é a mentira que esta arquitectura existe
  para impedir.

## Testes obrigatórios

Os testes de registry aplicam-se automaticamente à capability nova: identificador
bem formado, domínio coerente, autoridade delegável, risco e confirmação de
acordo, `dry_run` só onde há o que simular, reversibilidade honesta.

Acrescente os seus, em `crates/ocinye-core/tests/agentic.rs`, para:

- **ALLOW** — quem deve conseguir, consegue;
- **DENY** — quem não deve, recebe `PermissionDenied`;
- **INVALID** — entrada malformada dá `ValidationFailed`;
- **UNAVAILABLE** — quando depende de infraestrutura ausente.

## Documentação

Actualize a tabela em [README.md](README.md) e, se a capability alterar o que o
sistema pode fazer, [docs/feature-status/](../feature-status/README.md).
