# ADR-0110 — A Autoridade de Segredos da Instância

- **Estado:** Accepted
- **Domínio:** Security
- **Impacto:** HIGH
- **Depende de:** [ADR-0107](0107-mandatory-mfa-sessions-and-recovery.md) · [ADR-0013](0013-general-purpose-os-instance-and-node.md)
- **Data:** 2026-09-26

## Context

Uma Instância vai querer ligar fornecedores: uma chave de API da OpenAI, da
Anthropic, da Google ou da Mistral; o token de um endpoint compatível com OpenAI;
as credenciais de um conector. Até aqui não havia onde as guardar. A raiz de
selagem já existia, com subchaves HKDF por domínio para o correio e para o TOTP
([ADR-0107](0107-mandatory-mfa-sessions-and-recovery.md)); faltava um armazém de
credenciais de fornecedor, e a regra de quem as pode ver.

## Decision

**1. O Core é a Autoridade de Segredos.** Guarda, sela, roda, revoga e abre as
credenciais de fornecedores e integrações de uma Instância, na tabela
`instance_secrets` (migração 0055).

**2. Selagem num domínio próprio.** `ocinye/sealing/instance-secrets/v1`: uma
subchave HKDF distinta da mesma raiz, ChaCha20-Poly1305 com o esquema versionado
que já existia. Um criptograma de correio não abre com ela, nem o contrário.

**3. O valor nunca sai.** Não há rota que o devolva — nem agora, nem depois. A
API mostra metadados: tipo, nome, âmbito, os últimos quatro caracteres (só quando
o segredo tem pelo menos dezasseis), versão, estado, e quando foi criado, rodado,
revogado e usado.

**4. Uso por âmbito, só dentro do Core.** Cada segredo nomeia, ao nascer, o
serviço do Core que o pode abrir (`ai_gateway`, `mail`, `connector`).
`secrets::use_secret` é uma função do Core, e não uma rota: abre o segredo para
esse serviço, no momento do uso, regista o uso na auditoria sem o valor, e
devolve-o num `Secret` que não se imprime. As aplicações recebem o resultado da
chamada ao fornecedor, nunca a credencial.

**5. Rodar substitui; revogar apaga.** Rodar sela o valor novo por cima do antigo
e incrementa a versão — o antigo deixa de existir. Revogar apaga o criptograma e
o nonce; a linha fica como registo, e nada a volta a abrir.

**6. É da administração da plataforma.** `platform.administer`, que é
privilegiada e exige o segundo factor.

**7. O que não entra aqui.** Os segredos da própria plataforma — o URL da base, as
chaves do object store, a raiz de selagem — continuam no ambiente do operador. São
do operador, e não do administrador da Instância, e a raiz não se pode selar a si
mesma.

## Alternatives

**Um cofre externo (Vault, KMS de cloud).** Seria uma dependência estratégica de
um fornecedor para uma Instância que tem de funcionar sozinha; e a raiz continuaria
a ter de viver algures. A selagem existente é madura e está provada por restauro.

**Guardar só o digest.** Serve para verificar tokens que nós emitimos; não serve
para credenciais que temos de apresentar a um terceiro.

## Consequences

- Um despejo da base tem criptogramas, e não chaves de API; um teste procura os
  valores em claro em todas as tabelas e não os encontra.
- A continuidade conta e abre os segredos da Instância em `verify-keys`, como já
  fazia com as caixas de correio e o TOTP.
- A `Debug` das configurações do Core e do armazenamento deixou de imprimir
  credenciais.
- A superfície de administração dos fornecedores de IA (Parte 7) usa isto; não há
  interface de segredos avulsa antes dela.
