# 01 — System Truth Map (Pré-IA)

Verdade do terreno, derivada por ordem de autoridade: **produção deployada → `main`
→ testes/contratos executáveis → migrações/esquema → ADRs → docs → notas**. Não
parte de resumos.

**Data:** 2026-09-16. **`main`:** `99cb02a`. **Produção:** release `99cb02a99a58`.

## Concordância entre camadas

| Facto | `main` (repo) | Produção | Concorda? |
|---|---|---|---|
| Release SHA | `99cb02a` | `99cb02a99a58` (symlink `current`) | **sim** — produção corre o `main` certificado |
| Migrações | 47 na árvore (head `0047`) | 47/47 aplicadas com sucesso na BD | **sim** |
| Serviços a correr | 4 (core, worker, node-agent, conversion-runner) | core, worker, conversion-runner + proxy, postgres, redis, object-store — todos *healthy*/up | **sim** (node-agent não corre: não há nó) |
| Portas públicas | só o proxy (80/443) | só o proxy publica no host; DB/Redis/object-store/runner são internos | **sim** |
| Conversão | poppler + LibreOffice + ffmpeg na imagem do conversor | `pdftoppm`, `soffice`, `ffmpeg` presentes na imagem `ocinye-converter:99cb02a99a58` | **sim** |
| Endpoints públicos | — | `os.ocinye.com`→303, `/entrar`→303, `api…/raw` não-auth→401 | **sim** |

Nenhuma discordância entre `main` e produção no momento da auditoria.

## Contagens derivadas (`scripts/repository-facts.sh`)

| | valor |
|---|---|
| caminhos Core (`/api/v1`) | 200 |
| operações Core | 237 |
| ecrãs Workspace | 86 |
| migrações | 47 |
| tabelas | 84 |
| permissões | 76 |
| funções de teste na árvore | 1625 |
| testes que exigem BD | 598 |
| ADRs | 71 |
| runbooks | 12 |
| READMEs | 67 |

Os números não se escrevem à mão: saem do script, validados por
`scripts/section-one-contract.py`.

## Estado de IA (esperado e verdadeiro)

`OCINYE_AI_RUNTIME_READY = falso`: 0 fornecedores, 0 nós, 0 modelos, 0 GPU. Em
produção, a conclusão de um pedido de inferência é sempre `SYSTEM`/`DEGRADED`. O
plano de controlo (`OCINYE_AI_CONTROL_PLANE_READY`) está declarado; o runtime não,
e é a **única** camada que espera pela primeira GPU. Ver
[12-ai-runtime-gap.md](12-ai-runtime-gap.md).

## O que EU consigo provar, e o que precisa do humano

- **Provado por mim (read-only):** o SHA de produção, a saúde dos contentores, o
  head de migração na BD, as portas expostas, o comportamento dos endpoints
  **não-autenticados**, e a conversão de conteúdo (POST directo ao runner de
  PDF/RTF/mp4, todos devolveram PNG — ver [11-production-verification.md](11-production-verification.md)).
- **Provado na CI (base efémera):** a suite completa — 1 suite de browser E2E real
  (`apps/workspace/tests/browser.rs`, 106 viagens) + integração Core/HTTP. Ver
  [04-e2e-matrix.md](04-e2e-matrix.md).
- **Precisa do humano:** o **render autenticado em produção**. Não me autentico —
  não tenho credenciais e o login é-me proibido. As jornadas autenticadas provam-se
  na CI; a aceitação visual autenticada em produção é o Fidel que confirma. Este é
  um limite honesto da auditoria, não uma lacuna do produto.

## Fontes desta auditoria

- Inventário da superfície Workspace, do Core API + autorização, e dos testes/E2E
  — três varreduras de código dedicadas (2026-09-16).
- Evidência de produção read-only via SSH (só leitura + POST de conversão a um
  serviço interno, sem tocar em dados de utilizador).
- `repository-facts.sh`, `section-one-contract.py`, e os guardas de `verify.sh`.
