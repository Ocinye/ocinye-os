# 11 — Production Verification (Pré-IA)

Evidência recolhida contra produção a 2026-09-16. **Toda read-only**, excepto um
POST de conversão a um serviço interno (sem tocar em dados de utilizador). Sem
segredos.

## Release e saúde

| Item | Observado |
|---|---|
| Release corrente (`readlink current`) | `99cb02a99a58` (== `main @ 99cb02a`) |
| Contentores | core *healthy*, workspace *healthy*, worker up, conversion-runner up, proxy/postgres/redis/object-store *healthy* |
| Migração head na BD | `SELECT max(version), count(*) FROM _sqlx_migrations WHERE success` → `47 | 47` |
| Portas publicadas no host | só o proxy (`80`, `443`); DB/Redis/object-store/runner **não** publicam |

## Endpoints públicos (não-autenticados)

| Pedido | Código | Esperado |
|---|---|---|
| `GET https://os.ocinye.com/` | 303 | redirecção ao login sem sessão |
| `GET https://os.ocinye.com/entrar` | 303 | — |
| `GET https://api.ocinye.com/api/v1/me/files/{uuid}/raw` (sem sessão) | 401 | recusa sem autenticação |

## Conversão de conteúdo (ADR-0609) — ponta a ponta no host

Cada perfil testado com um POST directo ao Conversion Runner na rede interna
(a partir de um contentor com `curl`), com um artefacto sintético:

| Perfil | Entrada | Resultado |
|---|---|---|
| `pdf-thumbnail` | PDF mínimo | HTTP 200, PNG (`89504e47…`) |
| `office-thumbnail` | RTF trivial (LibreOffice `--headless`) | HTTP 200, PNG, 5004 bytes |
| `video-thumbnail` | mp4 de teste (ffmpeg) | HTTP 200, PNG, 13794 bytes |

Endurecimento confirmado no host: o contentor descartável corre `pdftoppm`,
`soffice` e `ffmpeg` com `--network=none`, `--read-only`, `--cap-drop=ALL`,
`--security-opt no-new-privileges`, `--user 65534`, tmpfs `/tmp`; rede ausente
(`getent` falha); rootfs bloqueia escrita fora de `/tmp`; contentores `--rm`, spool
limpo, sem órfãos. O `bubblewrap` está bloqueado pelo perfil do contentor (daí o
contentor descartável, não seccomp/bwrap em-processo).

## O que falta desta verificação (precisa do humano)

O **render autenticado**: Home, Files, Notes, Mail, Calendar, Prompt, etc. vistos
com uma sessão real em produção. Não me autentico. A prova autenticada é a
aceitação visual do Fidel; as jornadas autenticadas provam-se na CI contra base
efémera ([04-e2e-matrix.md](04-e2e-matrix.md)).
