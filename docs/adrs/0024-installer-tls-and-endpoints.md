# ADR-0024 — TLS do Installer v1 e pontos de acesso na instalação

- **Estado:** Proposed
- **Domínio:** Foundation
- **Impacto:** MEDIUM
- **Depende de:** [ADR-0020](0020-access-endpoints.md) · [ADR-0021](0021-installer-consumes-typed-contracts.md) · [ADR-0022](0022-graphical-remote-installer.md)
- **Data:** 2026-10-04

Importada do pacote D011 do Claude Design, no formato desta biblioteca.

## Context

Uma Instância com várias Distribuições (ADR-0019) responde em vários anfitriões
(ADR-0020). O Installer tem de os semear, de servir TLS para todos, e de não mexer em DNS
que não é dele.

## Decision

1. Modos de TLS: `OPERATOR_SUPPLIED` (recomendado; validado localmente — leitura, chave
   correspondente, validade, SAN que cobre cada ponto de acesso, cadeia) e
   `SELF_SIGNED_TEST` (rotulado teste/desenvolvimento, não confiável para produção
   pública). ACME fica adiado e não é oferecido.
2. O ponto canónico genérico é a semente do Core a partir de
   `OCINYE_WORKSPACE_PUBLIC_URL`. Os pontos ligados a uma Distribuição semeiam-se na
   instalação pelo subcomando `endpoint-seed` no anfitrião (auditado como
   `access_endpoint_seeded`, idempotente, com sete recusas tipadas). O `server_name` do
   nginx e os SAN do certificado auto-assinado listam todos os pontos.
3. O Installer nunca muda DNS. Mostra os registos A/AAAA necessários e verifica-os a
   partir da máquina do operador; DNS por resolver não bloqueia a instalação mas desliga
   «Abrir Ocinye OS»; um registo que aponte para outro lado bloqueia.

## Alternatives

- **ACME na v1** — exige DNS e porta 80 públicos no momento da instalação; adiado.
- **Escrever na tabela de pontos de acesso directamente** — viola a ADR-0021.

## Consequences

Na instalação só há o genérico e os ligados a Distribuições; mudanças posteriores fazem-se
em Administração (D010). A verificação do lado do operador é a resposta do Workspace em
cada anfitrião com o certificado fixado; a ligação anfitrião→Distribuição verifica-se no
servidor (`verify-endpoints`), porque `/api/v1/access/resolve` é só do Core.
