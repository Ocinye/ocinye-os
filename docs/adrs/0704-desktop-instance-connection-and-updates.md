# ADR-0704 — Ligação da casca a uma Instância, compatibilidade e actualizações

- **Estado:** Proposed
- **Domínio:** Operations
- **Impacto:** HIGH
- **Depende de:** [ADR-0702](0702-desktop-shell-technology.md) · [ADR-0703](0703-desktop-trust-boundary-and-native-bridge.md) · [ADR-0017](0017-instance-configuration-and-branding.md)
- **Data:** 2026-09-27

## Context

A casca e a Instância são versionadas separadamente: uma organização actualiza
o servidor quando quer, e cada pessoa actualiza a sua aplicação quando quer.
A casca também não pode aceitar como Instância qualquer página que devolva HTML.

## Decision

### Ligação

1. A pessoa escreve o endereço. Só `https` (excepção: `localhost` em builds de
   desenvolvimento).
2. TLS verificado pela pilha do sistema. Nenhum `accept_invalid_certs`; uma
   Instância com CA própria entra por confiança explícita, governada e
   registada, com a impressão digital à vista.
3. **Identidade, pelo host que a pessoa escreveu.** Numa Instância
   auto-instalada só o Workspace é público; o Core não (o nginx da Instância
   encaminha só o Workspace, e o Core não publica portas). Por isso a
   identidade vem do **Workspace**: `GET /.well-known/ocinye-instance`
   (público, sem sessão, pequeno) →
   `{ product: "ocinye-os", instance_name, os_version, api,
   minimum_desktop_version, capability_versions: [min, max] }`, composto pelo
   Workspace a partir de `GET /api/v1/instance/branding` e do contrato de
   prontidão do Core (a criar em R3). Sem este documento, com `product`
   diferente, ou numa origem diferente da escrita (redireccionamentos para
   outro host recusam-se), não é uma Instância Ocinye.
4. **Confiar:** cartão com nome, host, certificado e versão; a Instância
   confiada fica na lista local com a impressão digital. Uma mudança de
   certificado não esperada pede nova confiança.
5. Entrar é o ecrã de login da própria Instância (D3), no webview de confiança.

### Várias Instâncias

Lista local, cada uma com partição, confiança e preferências próprias. Nunca se
reutiliza sessão, cookie nem identidade entre Instâncias.

### Aperto de mão

A casca envia `runtime`, `shell_version` e `capability_version` — sem
impressão digital do anfitrião além de plataforma e arquitectura. Fora do
intervalo da Instância, a casca diz porquê e oferece o endereço Web.

### Actualizações da casca

- `tauri-plugin-updater` com pacotes **assinados**; a chave pública vai na
  build; o endpoint de actualização é fixo na build — nunca vindo da Instância.
- Uma Instância não serve executáveis à casca.
- Falha de actualização: a versão anterior continua; nada se perde da Instância
  (o estado é do servidor).
- Distribuição geral exige: Authenticode (Windows), Developer ID + notarização
  (macOS), pacotes Linux com verificação de assinatura. Uma build sem assinatura
  é de desenvolvimento, e diz-se.

### A Web

O Workspace já versiona o release (`X-Ocinye-Build` a acrescentar, D15 G-17) e
mostra «há uma nova versão» sem recarregar sozinho. Nenhum service worker
guarda páginas autenticadas (ADR-0617).

## Consequences

- Uma rota pública nova do Workspace (`/.well-known/ocinye-instance`) entra na
  lista das públicas com a razão, e diz só o que a página de entrada já diz
  (nome e versão) — nada de membros, políticas ou configuração.
