# ADR-0409 — Duas credenciais de correio: a da instituição e a de cada membro

- **Estado:** Accepted
- **Domínio:** Mail
- **Impacto:** HIGH
- **Data:** 2026-08-26
- **Substitui parcialmente:** [ADR-0401](0401-mail-provider-abstraction.md), na parte da credencial
- **Complementa:** [ADR-0408](0408-imap-transport.md) · [ADR-0404](0404-mail-privacy-boundary.md)

## Context

Até aqui o Ocinye autenticava-se no serviço de correio com **uma credencial de
serviço**, lida da configuração da instalação — `OCINYE_MAIL_USERNAME` e
`OCINYE_MAIL_PASSWORD`. O Core resolvia o remetente de cada mensagem a partir da
caixa que a pessoa podia usar, e o adaptador nunca decidia identidade nenhuma.

Esse modelo funciona e tem uma propriedade desconfortável: **uma conta que pode
enviar em nome de qualquer endereço do domínio é uma capacidade de personificação
institucional, guardada numa variável de ambiente.** Quem lê essa variável passa
a poder escrever como qualquer pessoa da Ocinye. A concentração não é acidental —
é o preço de haver uma credencial só.

Havia também uma consequência prática: uma pessoa não podia ligar a sua própria
caixa. O correio de cada membro existia se, e só se, alguém com acesso ao
armazenamento de segredos o tivesse configurado.

## Decision

**Existem duas credenciais, com papéis distintos, e nenhuma substitui a outra.**

### 1. A credencial da instituição serve o que não tem pessoa

`OCINYE_MAIL_USERNAME` e `OCINYE_MAIL_PASSWORD`, no armazenamento de segredos da
instalação. É com ela que corre o que acontece **sem ninguém a olhar**: a
ingestão periódica do worker, e o que o plano agentic vier a fazer dentro da
política que o governa.

Este trabalho não tem membro a quem pedir credencial — um worker que dependesse
da senha de alguém pararia quando essa pessoa saísse da instituição.

### 2. A credencial da caixa é de quem a liga

`mailbox_credentials` guarda, por caixa: o nome de utilizador e a senha cifrada,
escritos pela pessoa a quem a caixa pertence. É com ela que essa pessoa lê e
envia o seu correio.

Uma credencial comprometida aqui alcança **uma** caixa, a de quem a ligou.

### 3. Qual delas é usada, e quando

    ingestão do worker, plano agentic    →  credencial da instituição
    ler e enviar como um membro          →  credencial dessa caixa

A regra é: **uma acção com pessoa usa a credencial dessa pessoa.** A credencial
da instituição nunca serve para agir em nome de alguém — se servisse, voltaríamos
à personificação de domínio que este ADR existe para separar.

Uma caixa sem credencial própria continua a ser indexada pelo worker, e a pessoa
vê o que lá está. O que ela não pode é **enviar** antes de ligar a sua.

### 4. A senha é cifrada com cifra autenticada, e a chave vive fora da base

`ChaCha20-Poly1305`, com uma chave de 256 bits lida de `OCINYE_MAIL_KEY`. Nonce
por registo, gerado do CSPRNG do sistema e guardado ao lado do criptograma.

Autenticada e não apenas cifrada: sem autenticação, alguém com escrita na base
pode alterar um criptograma e o sistema apresentaria o resultado como senha. Com
`Poly1305`, a decifra recusa.

A chave **não está na base de dados**. Quem obtiver um despejo da base obtém
criptogramas, e não senhas. Quem obtiver a chave sem a base não obtém nada.

### 5. O Core passa a ser guardião de credenciais reversíveis

Isto é o custo desta decisão, e regista-se como tal. Até aqui o Core guardava
apenas **verificadores** — Argon2id, de sentido único, que provam uma senha sem a
conter ([ADR-0104](0104-password-policy-and-hashing.md)). Uma senha de IMAP tem
de ser apresentada ao servidor, portanto tem de poder ser recuperada.

A superfície fica contida: a decifra acontece num sítio só, no momento de abrir
uma sessão, e o texto em claro nunca sai desse ponto — nem para registo, nem para
auditoria, nem para a API, nem para o browser depois de escrito.

### 6. A senha do correio não é a senha do Ocinye

Continuam a ser coisas distintas, e nenhuma serve para obter a outra. Ligar uma
caixa exige a senha dessa caixa, que a pessoa obtém do serviço de correio.

### 7. Quem liga uma caixa é quem a detém

Uma pessoa liga a sua própria caixa pessoal. Uma caixa partilhada é ligada por
quem tem autoridade sobre ela, pelas regras de partilha que já existem. Ninguém
liga a caixa de outra pessoa: fazê-lo seria escrever uma credencial em nome de
alguém que não a deu.

### 8. A ligação é verificada antes de ser guardada

Uma credencial que não abre sessão não se escreve. Guardar primeiro e falhar
depois deixaria caixas que dizem estar ligadas e não estão — e o membro
descobriria pela ausência de correio, que é indistinguível de não ter recebido
nada.

## Alternatives

**Só a credencial de serviço.** Mais simples, e foi o que existia. Rejeitada
como modelo único por concentrar a personificação do domínio inteiro e por
impedir uma pessoa de ligar a sua caixa. Mantida para o trabalho que não tem
pessoa, onde é a resposta certa.

**Só a credencial por membro.** Considerada, e rejeitada por deixar o worker sem
forma de indexar: a ingestão pararia com a saída de quem tivesse ligado a caixa,
e o correio institucional voltaria a aparecer apenas quando alguém está a olhar.

**OAuth delegado.** É o que se prefere onde o fornecedor o serve: o Core recebe um
token revogável e nunca vê a senha. O serviço em uso (`mail.ocinye.com`, LWS) não
o oferece. Fica registado como o caminho preferível quando existir, e a forma da
tabela não impede acrescentá-lo.

**Não guardar nada, pedir a senha em cada sessão.** Rejeitada: tornaria a
sincronização automática impossível, e o correio voltaria a aparecer só quando
alguém está a olhar.

## Consequences

**O que melhora.** Uma credencial de membro alcança uma caixa. Uma pessoa liga a
sua sem depender de quem administra. Revogar é apagar uma linha. E o trabalho
autónomo continua a correr, porque não depende de ninguém em particular.

**O que fica dividido de propósito.** Duas credenciais são duas coisas a rodar e
duas a comprometer. A troca é deliberada: a alternativa era uma só, e uma só
significa que quem a tem escreve como qualquer pessoa da Ocinye.

**O que piora.** O Core passa a deter segredos recuperáveis, e a perda da chave
de `OCINYE_MAIL_KEY` torna todas as credenciais ilegíveis — as caixas têm de ser
religadas. É recuperável, e é a troca deliberada: preferimos religar caixas a
guardar uma chave dentro da base que a protege.

**O que fica por fazer.** Rotação de chave sem religar as caixas exige decifrar e
recifrar em massa, e não está implementada. Quando for, é decisão própria.
