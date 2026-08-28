# O contrato de inferência do Ocinye

Decisões: [ADR-0304](../adrs/0304-canonical-inference-contract.md) ·
[ADR-0305](../adrs/0305-provider-conformance.md).

> **O contrato de inferência pertence ao Ocinye, não a um fornecedor de
> modelos.**

Qwen adapta-se ao contrato. DeepSeek adapta-se ao contrato. Uma futura L40S
adapta-se ao contrato. Nunca o contrário.

## A fronteira

```
Agent Runtime
     ↓
Contrato canónico do Ocinye        ← termina aqui o que é do Ocinye
     ↓
InferenceProvider
     ↓
Provider Adapter                   ← começa aqui o que é do fornecedor
     ↓
Qwen · DeepSeek · modelo futuro
```

**Formatos específicos de fornecedor terminam no adapter.** Se um modelo exigir
um *chat template* próprio, um modo JSON próprio, ou devolver raciocínio à
parte, isso é trabalho do adapter e fica no adapter.

## Confiança

| Campo | Origem | Confiança |
|---|---|---|
| `system` | O Ocinye Core, e só ele | Controlado |
| `data` | Documentos, email, datasets, web | **Potencialmente hostil** |
| `instruction` | O actor | Input de utilizador |

**Os três são campos distintos, e não podem ser fundidos antes do adapter.** Um
contrato que aceitasse uma só string opaca teria já misturado política de
sistema com conteúdo recuperado, e é nessa separação que a defesa contra
injecção assenta ([ADR-0405](../adrs/0405-mail-prompt-injection.md)).

O adapter é livre de os renderizar como o seu modelo espera. Não é livre de os
receber já fundidos.

## O pedido

```rust
InferenceRequest {
    contract,           // ContractVersion — a versão que o Core fala
    capability,         // GENERAL · CODING · REASONING · EMBEDDING
    system,             // política do Ocinye
    data,               // Vec<DataBlock> — material a processar
    instruction,        // o pedido do membro
    schema,             // a forma que a resposta tem de ter, ou nada
    max_output_tokens,
    deadline,           // quanto tempo o Core espera
}
```

## A resposta

```rust
InferenceResponse {
    contract,   // a versão em que o adapter respondeu
    text,       // prosa, quando não se pediu forma
    value,      // o valor estruturado, quando se pediu
    model,      // ModelIdentity — para PROVENIÊNCIA, não para routing
    usage,      // tokens, quando reportados
}
```

`model` existe porque *output* institucional tem de ser atribuível a um modelo e
a uma versão. **Nada a montante ramifica sobre ele**, e é normalizado no guarda
porque é texto controlado pelo fornecedor que aterra em logs.

## O que o Core aplica, sempre

`infer_within_deadline` é o único caminho para um provider. Aplica três coisas
do lado do Core, porque **um provider é entrada não confiável como qualquer
outra**:

1. **o prazo** — um provider que fica pendurado não prende o pedido;
2. **a versão do contrato** — uma resposta cujo significado não se garante é
   recusada, não interpretada com esperança;
3. **o tamanho** — um provider não pode esgotar o processo com um payload.

E normaliza a `ModelIdentity` à saída.

## Versionamento

`ContractVersion` tem uma variante, `V1`, e isso é deliberado.

> **Mudanças incompatíveis no contrato do Ocinye são explícitas.**

Uma segunda variante aparece quando o Core genuinamente precisar de falar duas —
e esse é o momento em que a decisão se toma, em vez de se descobrir.

Uma versão que o Core não conhece é **recusada**. Falha fechada: não sabemos ler
o significado.

**Não existe `QwenV1` nem `DeepSeekV1`.** Versionar por modelo seria o contrato
pertencer aos modelos.

## Erros

Fechado, estruturado, seguro para logging, e **sem eco do fornecedor**:

| Variante | Quando |
|---|---|
| `NoProvider` | Nada serve esta capacidade |
| `Unavailable` | Existe e não respondeu |
| `Refused` | Recusou o pedido |
| `ContextExceeded` | O pedido excede o contexto |
| `MalformedResponse` | Respondeu, e a resposta não serve |
| `Timeout` | Não respondeu a tempo |
| `ResponseTooLarge` | Respondeu com mais do que o Core lê |
| `UnsupportedContractVersion` | Respondeu num contrato desconhecido |

Nenhuma variante tem campo onde texto do fornecedor caiba. O texto de erro de um
modelo pode citar o prompt de volta, e o prompt pode conter correspondência de
um membro (briefing §18).

`is_transient()` diz que uma repetição não é obviamente fútil. **Não diz que é
segura**: um timeout não significa que o efeito não aconteceu, e uma capability
com efeito externo nunca deve ser repetida por causa dele.

## Saída estruturada

O Runtime precisa de um **plano**, não de prosa. Arrancar forma a um modelo
concreto é trabalho de adapter.

O Core valida **na mesma**. Um provider a afirmar conformidade não é
conformidade.

Política do que um modelo erra:

| Caso | Política |
|---|---|
| Campo desconhecido | Ignorado — é ruído |
| Campo obrigatório em falta | **Recusado** |
| `null` num campo obrigatório | **Recusado** — `null` é ausência por extenso |
| Tipo errado | **Recusado** — coerção é adivinhação |
| Enum desconhecido | **Recusado** |
| Versão desconhecida | **Recusado** no guarda |

Tudo com impacto de segurança **falha fechado**.

## Como acrescentar um fornecedor

1. **Implementar `InferenceProvider`.** Três métodos.
2. **Nenhuma política do Core no adapter.** Não decide permissões, risco,
   aprovação, que capabilities existem, nem se algo é estado do sistema.
3. **Traduzir o pedido canónico** para o que o modelo espera.
4. **Traduzir a resposta** para a forma canónica, incluindo a saída
   estruturada.
5. **Normalizar erros** para `InferenceError`. Nunca reencaminhar o texto do
   fornecedor.
6. **Sem fuga de segredos.** A credencial do adapter não entra no pedido, nem
   em erros, nem em logs.
7. **Passar a Conformance Suite:**
   ```rust
   let report = conformance::certify(&adapter, ProviderKind::Serving).await;
   assert!(report.passed(), "{}", report.summary());
   ```
8. **Registar no Model Registry** — e só então.
9. **Documentar os requisitos de execução**: onde corre, que recursos usa, que
   residência de dados implica.

## O adapter oficial também não é confiável

Um adapter escrito pela Ocinye é uma fronteira não confiável do ponto de vista
do Core determinístico, exactamente como qualquer outro. O guarda aplica-se-lhe
igual, e o executor valida tudo o que dele vier.

## A L40S é hardware

Vale a pena separar quatro coisas que é fácil confundir:

```text
NVIDIA L40S 48 GB          hardware. Um recurso.
        ↓
Servidor de inferência     software que serve o modelo
        ↓
Qwen · DeepSeek · outro    o modelo
        ↓
Ocinye Provider Adapter    software nosso. Implementa o contrato.
        ↓
InferenceProvider          o contrato canónico do Ocinye
```

**A GPU não implementa `InferenceProvider`.** Nem o modelo, nem o servidor de
inferência. Quem o implementa é um Ocinye Provider Adapter.

Quando a L40S for provisionada, o serviço de inferência executado nesse nó será
integrado através de um adapter que implementa o contrato canónico, passa a
Conformance Suite, e só então é registado no Model Registry. Nessa ordem.

O mesmo contrato serve, sem alteração, uma L40S em cloud, um futuro nó físico da
Ocinye, outra GPU, inferência em CPU, ou outro servidor de inferência. **É um
alvo de integração, não uma dependência arquitectural.**
