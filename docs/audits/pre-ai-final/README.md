# Auditoria Pré-IA — pacote de evidência

A última varredura ampla da baseline **determinista** do Ocinye OS antes de entrar
o primeiro runtime de IA/GPU. Evidência permanente, versionada, sem segredos.

**`main`:** `99cb02a` · **Produção:** `99cb02a99a58` · **Data:** 2026-09-16 ·
**Portão agregado:** [gate.md](gate.md) — **NÃO declarado READY** (fail-closed).

## Documentos

- [00 — Sumário executivo](00-executive-summary.md)
- [01 — System Truth Map](01-system-truth-map.md)
- [02 — Inventário de funcionalidades](02-feature-inventory.md)
- [04 — Matriz E2E e cobertura](04-e2e-matrix.md)
- [05 — Revisão de segurança](05-security-review.md)
- [11 — Verificação de produção](11-production-verification.md)
- [12 — Lista de bloqueios de runtime de IA](12-ai-runtime-gap.md)
- [findings.md — registo de defeitos](findings.md)
- [gate.md — portão agregado](gate.md)
- Handoff de IA: [`docs/operations/AI_RUNTIME_HANDOFF.md`](../../operations/AI_RUNTIME_HANDOFF.md)

## Estado

Esta é uma auditoria **faseada e em curso**. O que está provado, o que precisa do
humano (aceitação autenticada em produção) e o que está bloqueado só no runtime de
IA está separado de forma explícita, em cada documento. Nada aqui declara pronto o
que a evidência não sustenta.

## Documentos ainda por escrever nesta pass

`03-requirements-traceability.md`, `06-ui-ux-review.md`, `07-data-integrity.md`,
`08-performance.md`, `09-backup-restore.md`, `10-deploy-rollback.md`,
`13-known-limitations.md`, `evidence-manifest.json` — à medida que as fatias
correspondentes forem exercitadas. A numeração segue a estrutura da missão.
