# ADR-0203 — Artefactos de modelo como memória institucional

- **Estado:** Accepted
- **Domínio:** Data
- **Impacto:** FOUNDATIONAL
- **Data:** 2026-08-29
- **Relaciona-se com:** [ADR-0700](0700-institutional-continuity-and-portability.md) ·
  [ADR-0200](0200-object-storage.md) · [ADR-0300](0300-ai-gateway.md) ·
  [ADR-0304](0304-canonical-inference-contract.md) ·
  [ADR-0412](0412-scientific-lifecycle-and-provenance.md) ·
  [ADR-0500](0500-compute-registry-node-agent.md)

## Context

Até aqui, a memória institucional da Ocinye era **explícita**: documentos,
datasets, resultados, proveniência. O modelo era runtime — substituível, e a
[ADR-0700](0700-institutional-continuity-and-portability.md) classificou os
pesos como `EXTERNAL` precisamente por isso.

Isso é verdade enquanto a IA for consumida. **Deixa de ser verdade no dia em
que a Ocinye treinar.**

Com RAG, o conhecimento fica nos dados e o modelo é intercambiável. Com
*fine-tuning*, *continued pretraining* ou destilação, parte da capacidade
institucional passa a existir **parametricamente**, nos pesos, e não se
reconstrói a olhar para o PostgreSQL. Se a Ocinye passar meses a ensinar um
modelo e perder o servidor GPU, volta ao ponto zero.

### O que a auditoria encontrou

`ai_models` **não é um registo de artefactos**. É um inventário reportado:

- `replace_reported_models` faz `DELETE FROM ai_models WHERE node_id = $1` e
  volta a inserir a cada relatório do nó, pelo que **os identificadores são
  novos de cada vez**;
- `node_id … ON DELETE CASCADE` faz a linha desaparecer com o nó;
- não há soma, nem modelo base, nem corrida de treino, nem versões de dataset,
  nem licença, nem referência a bytes.

Ou seja: **hoje o nó de computação detém o modelo.** Um modelo treinado pela
Ocinye, registado ali, desapareceria com o nó que o treinou.

A auditoria encontrou ainda que a lista de tipos aceites em
`storage::ALLOWED_CONTENT_TYPES` **recusa** `application/octet-stream`. Está
certa a recusar: é uma lista de permissões para artefactos documentais
(`CLAUDE.md` §40). Mas significa que **um artefacto de modelo não tem hoje por
onde entrar na instituição**, e alargar aquela lista seria a correcção errada —
transformaria a fronteira de uploads documentais num canal para binários
arbitrários.

## Decision

### 1. Um modelo treinado pela Ocinye é um activo institucional durável

> **A trained model is an institutional artefact when its acquired capability
> cannot be reconstructed solely from externally recoverable dependencies.**

Duas classes novas na classificação de continuidade:

| Classe | Viaja | Quando se aplica |
|---|---|---|
| `DURABLE_MODEL_ARTIFACT` | **sim** | pesos, adaptadores, tokenizer e configuração que a instituição produziu. Ninguém fora da Ocinye os tem. |
| `EXTERNAL_REACQUIRABLE` | não | um modelo base publicado — **e só** se a versão exacta estiver identificada, houver soma para a confrontar e a licença permitir voltar a obtê-la. |

As três condições da segunda linha não são cerimónia. `Qwen X revisão A` e
`revisão B` podem não ser cientificamente equivalentes, e um adaptador LoRA sem
o modelo base **exacto** é ruído com a forma certa.

### 2. Computação produz artefactos institucionais; não os detém

> **Compute produces institutional artifacts; compute does not own them.**

O nó de treino é computação. Depois da promoção, o artefacto vive no Object
Storage institucional e é registado no Core, e o nó pode ser destruído sem
perda. É a inversão exacta do que `ai_models` faz hoje.

### 3. Pesos sem linhagem são um ficheiro

> **Weights without lineage are an artefact. Weights with lineage are
> institutional capability.**

Daqui a quatro anos, `ocinye-qwen-17.safetensors` sem saber de que base veio,
com que dados, com que receita e com que avaliação **não é** memória
institucional. A linhagem tem de responder a: que dados ensinaram esta versão?
De que base deriva? Que versão substituiu? Que versão estava operacional quando
o resultado científico X foi produzido? Consegue-se restaurar exactamente? E
continuar o treino?

Os quinze verbos de proveniência que já existem
([ADR-0412](0412-scientific-lifecycle-and-provenance.md)) chegam para isto —
`DerivedFrom`, `Uses`, `InputTo`, `Produces`, `ProducedBy`, `ExecutedOn`,
`Validates`, `Supersedes`. O que falta são os **tipos de recurso** e as tabelas,
não o vocabulário.

### 4. A retenção distingue o que é institucional do que é descartável

Um treino grande produz centenas de gigabytes. Guardar tudo para sempre não é
política, é ausência dela:

| | Retenção |
|---|---|
| versão promovida | durável, institucional; nunca eliminada sem ciclo de vida explícito |
| checkpoints seleccionados | duráveis se forem precisos para retomar treino, investigar ou auditar |
| checkpoints intermédios | reconstruíveis; elimináveis por política |
| estado do optimizador | só se se quiser **continuar exactamente** o treino; para inferência, não |

### 5. A classificação de um modelo depende do que o treinou

Não se assume que um modelo treinado sobre `RESTRICTED` seja `INTERNAL`. O
*fine-tuning* pode memorizar partes do material de treino, e por isso a
classificação de um modelo derivado é uma **decisão de política**, não uma
herança automática nem um valor por omissão. Isto merece entrada própria no
modelo de ameaças: um modelo treinado sobre dados sensíveis não pode tornar-se
um contorno de autorização.

E, pela mesma razão: **apagar o dataset depois do treino não apaga o que os
pesos aprenderam.** O ciclo de vida de dados terá de considerar remoção,
memorização, retreino e retirada de modelo.

### 6. O modelo não substitui a evidência de que nasceu

> **Trained models may embody institutional capability, but they do not replace
> the institutional evidence and provenance from which that capability was
> derived.**

Se um modelo foi treinado com `DatasetVersion 7`, **preserva-se a
`DatasetVersion 7`**. Não se diz «já está nos pesos». Os pesos não substituem
evidência, proveniência, direito de auditoria, reprodutibilidade nem
interpretação humana.

### 7. Restaurar não exige GPU. Provar que corre, exige

Um restauro verifica bytes, somas, metadados, linhagem e controlo de acesso —
tudo sem placa gráfica. A inferência fica `NO_RESOURCE` e o restauro continua
correcto.

O que **não** fica provado assim é que o artefacto é utilizável: um ficheiro
com a soma certa pode ser inútil se faltar o tokenizer ou a configuração. Isso
exige um ensaio próprio — carregar o artefacto num runtime compatível e correr
uma avaliação determinística — e é uma verificação separada, com o seu próprio
recurso.

## Alternatives

**Deixar os pesos como `EXTERNAL`.** É o estado actual, e é correcto enquanto
não houver treino. Rejeitado como posição permanente: torna-se falso
silenciosamente, no dia em que alguém afina o primeiro modelo, e o sintoma é
perder o trabalho.

**Guardar tudo, incluindo o modelo base e todos os checkpoints.** Rejeitado:
enche cada cópia com dezenas de gigabytes que o publicador já guarda, e a
inflação faz com que se deixe de fazer cópias.

**Alargar `ALLOWED_CONTENT_TYPES` para aceitar `application/octet-stream`.**
Rejeitado com firmeza. Transformaria a fronteira de uploads documentais num
canal para binários arbitrários (`CLAUDE.md` §40). Um artefacto de modelo
precisa do seu próprio caminho tipado, com a sua própria política.

**Confiar em RAG para tudo.** Rejeitado como doutrina única. A Ocinye usará
três mecanismos com propriedades diferentes: conhecimento explícito e citável,
recuperação, e capacidade paramétrica. Escolher um só deles perde as
propriedades dos outros.

## Consequences

**O que muda já.** A classificação de continuidade distingue o modelo base
readquirível do artefacto treinado, e o segundo viaja. `ai_models` sai da
comparação de identidades — com a razão escrita e um teste que lê o código que
a justifica, para que a decisão não sobreviva à sua própria causa.

**O que fica `NOT IMPLEMENTED`, e é dito.** Não existe `Model`, `ModelVersion`,
`ModelArtifact`, `TrainingRun` nem `EvaluationRun`. Não existe caminho para
carregar pesos. Não existe promoção, nem política de retenção aplicada, nem
registo de licença do modelo base. **Nada disto foi construído aqui, e não deve
ser apresentado como existente.**

**Porque não foi construído agora.** Porque não há nó de computação, não há
treino, e não há um único artefacto de modelo para preservar. Construir o
registo antes de existir o primeiro modelo seria desenhar contra imaginação em
vez de contra um caso real — e a forma correcta das tabelas depende de coisas
que só se sabem ao afinar o primeiro modelo: que técnica, que artefactos
acompanham, que avaliação sustenta a promoção.

O que esta ADR fixa é a **decisão** e as classes. A implementação vem com o
primeiro treino.

### O portão de entrada

A dívida não fica à espera de que alguém se lembre dela:

> **No first institutional model without continuity.**

As onze perguntas vivem em `continuity::models`, tipadas, cada uma com a razão
por que é obrigatória. Dez estão `PorResponder`; uma está `Provada`, com a
evidência a apontar para o ensaio que a sustenta. Um teste percorre as
migrations e **falha no dia em que o esquema ganhar** `model_artifacts`,
`model_versions`, `training_runs`, `evaluation_runs` ou `model_checkpoints` com
perguntas por responder — nomeando-as uma a uma.

Isto inverte a ordem em que estas coisas costumam correr. Normalmente o registo
aparece primeiro, os modelos entram, e a continuidade descobre-se em falta
quando um deles se perde. Aqui a continuidade é a condição de entrada.

E uma segunda resposta é aceite: responder à pergunta com evidência que se
possa ir verificar. Um teste recusa `Provada("sim")` — uma resposta sem
evidência é uma opinião com aspecto de facto.
