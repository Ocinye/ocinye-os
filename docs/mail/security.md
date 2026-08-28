# Segurança do Ocinye Mail

Complementa [docs/security/](../security/README.md) e
[docs/threat-model/](../threat-model/README.md).

## Porque o correio é a superfície mais exposta do Ocinye OS

Todas as outras entradas do sistema exigem uma conta. O correio não: qualquer
pessoa no mundo pode enviar uma mensagem para um endereço da Ocinye, com HTML,
anexos e texto arbitrários, e essa mensagem será renderizada a um membro
autenticado.

Três ameaças concretizam-se aqui.

## 1. XSS pelo corpo da mensagem

**Mitigação:** higienização por lista de permissões com `ammonia`, antes de
qualquer renderização ([ADR-0402](../adrs/0402-mail-html-sanitisation.md)).

Removidos sem excepção: `<script>`, `<iframe>`, `<object>`, `<embed>`, `<form>`,
`<input>`, `<style>`, todos os atributos `on*`, e qualquer URL fora de
`http`/`https`/`mailto`/`tel`/`cid`.

O corpo é higienizado no Core e não é escrito em lado nenhum. O
`inner_html` da interface é único e está documentado no ficheiro onde vive.

## 2. Rastreio por conteúdo remoto

Uma imagem remota num email é um pedido HTTP disparado no momento em que a
mensagem é aberta. Diz a quem enviou que foi lida, quando, e de que rede.

**Mitigação:** bloqueado. `src` é reescrito para `data-oc-remote` e contado.

O Core sabe servir o corpo com o conteúdo remoto a pedido explícito por
mensagem, e regista-o no audit trail — o que serve um cliente de API que o
queira, e mantém o registo se a interface um dia o oferecer.

**O Ocinye Workspace não o carrega.** A sua Content Security Policy declara
`img-src 'self' data:`, por isso um `src` de terceiros seria recusado pelo
browser de qualquer modo. Houve um botão «Carregar mesmo assim» que fazia
exactamente isso: recarregava a página, o aviso desaparecia — porque já nada
estava por carregar — e as imagens continuavam ausentes, deixando ao membro a
impressão de que o pedido tinha sido atendido. O botão saiu; o estado ficou,
dito por inteiro ([Security Baseline v1](../security/2026-08-23-security-baseline-v1.md), F-12).

Alargar a CSP a origens de terceiros seria desmontar a última barreira contra o
rastreio para repor um botão. Servir o conteúdo remoto através do Ocinye, sem
contactar o remetente a partir do browser do membro, é funcionalidade por
construir — não correcção.

A preferência `remote_content_policy` tem `block` por omissão, e
`RemoteContentPolicy::parse` devolve `Block` para qualquer valor irreconhecível
— uma preferência corrompida não pode voltar a ligar o rastreio.

## 3. Prompt injection

Ver [ADR-0405](../adrs/0405-mail-prompt-injection.md) e [ai.md](ai.md).

Resumo: conjunto fechado de acções, blocos de dados delimitados, e — a garantia
que não depende do modelo — **a assistência não tem nenhuma acção com efeito ao
seu alcance**. Devolve texto.

## Privacidade entre membros

Ver [ADR-0404](../adrs/0404-mail-privacy-boundary.md).

Nenhum papel administrativo lê uma caixa pessoal alheia. A garantia está na
cláusula `WHERE`, e não existe consulta de correio que consulte um papel.

Uma caixa que não é do actor lê-se como **inexistente**, não como recusada.

## Saída de material classificado

Ver [ADR-0403](../adrs/0403-mail-send-policy.md).

`RESTRICTED` não sai para destinatários externos, e confirmar não desfaz a
recusa. Um domínio semelhante — `ocinye.com.atacante.net` — conta como externo.

## Nomes de ficheiro

`safe_filename` retira tudo o que se pareça com caminho antes de um nome
escolhido por um remetente ser escrito num cabeçalho `Content-Disposition` ou no
disco de alguém. `../../.bashrc` não sobrevive à viagem.

A base de dados reforça-o: `ck_mail_attachment_filename_is_safe`.

## O que nunca é registado

Por §57 do briefing e `CLAUDE.md` §62, nenhum destes entra em log, audit ou
telemetria:

- palavras-passe ou credenciais de caixa;
- tokens OAuth ou credenciais SMTP;
- corpos de mensagem, integrais ou parciais;
- anexos;
- prompts que contenham uma mensagem inteira.

`MailConfig` e `ImapSmtpConfig` implementam `Debug` à mão para redigir a
password, porque `CoreConfig` deriva `Debug` e é registado no arranque.
Existe um teste que verifica isso.

`ProviderHealth::endpoints` mostra anfitriões e portos — nunca credenciais, e
não por disciplina: a estrutura não as contém.

## Transporte

A cifra não tem interruptor. `OCINYE_MAIL_IMAP_TLS` e `OCINYE_MAIL_SMTP_TLS`
aceitam `tls` ou `starttls`; `false`, `none`, `off` e `plain` fazem o Core
recusar arrancar ([ADR-0408](../adrs/0408-imap-transport.md)).

O certificado é sempre verificado. O `rustls` é construído com a raiz da Mozilla
compilada e sem opção de a ignorar — não existe *flag* de desenvolvimento que
desligue a verificação, porque uma que exista acaba ligada em produção.

Ler uma mensagem usa `BODY.PEEK`, nunca `BODY`: abrir algo no Ocinye Workspace
não altera o estado da mensagem no servidor como efeito secundário.

## Onde as credenciais vivem

Em `OCINYE_MAIL_PASSWORD`, lida do ambiente no arranque. **Não** em
`mail_provider_settings` — essa tabela não tem colunas de credenciais, por
desenho. **Não** em `.env.example`, que documenta a variável sem valor.

Configuração parcial faz o Core recusar arrancar: um correio meio configurado
parece disponível e falha no momento em que alguém carrega em «Enviar».

`ocinye-core-server mail-check` prova uma configuração sem arrancar o Core.
Imprime anfitriões, portos, segurança de transporte, nomes de pasta e contagens
— nunca a password, a resposta de autenticação, assuntos, remetentes ou corpos.
E não envia nada: diagnosticar um caminho de saída enviando correio a alguém é
como mensagens de teste chegam a pessoas reais.

### Uma credencial hoje, muitas depois

A instalação usa uma credencial única. A versão multiutilizador precisará de
credenciais por caixa, e essas não podem ficar em claro na base de dados — nem
podem ser apenas um digest, porque o Ocinye precisa da credencial original para
se autenticar no IMAP. A camada de cifra em repouso é `PLANNED` e exige ADR
próprio.
