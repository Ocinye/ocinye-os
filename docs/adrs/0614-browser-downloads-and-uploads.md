# ADR-0614 — Transferências e carregamentos do Browser pelo caminho governado dos Ficheiros

- **Estado:** Proposed
- **Domínio:** Workspace
- **Impacto:** HIGH
- **Depende de:** [ADR-0612](0612-browser-manager.md) · [ADR-0703](0703-desktop-trust-boundary-and-native-bridge.md) · [ADR-0108](0108-resource-governance-and-compute-control-plane.md) · [ADR-0607](0607-files-as-a-content-browser.md)
- **Data:** 2026-09-27

## Context

Um ficheiro que um site entrega pode ir para o computador ou para Ocinye Files.
Um site pode pedir um ficheiro. Nos dois sentidos, o site é hostil e o nome, o
tipo e o tamanho que declara são dados, não factos.

## Decision

### Transferir para Ocinye Files (Desktop)

```text
resposta do site ─► casca: stream para ficheiro temporário (0600, nome aleatório, limite de disco)
                 ─► sessão de carregamento existente dos Ficheiros pessoais (por partes)
                 ─► Core: quota, RBAC, tipo validado pelos bytes, chave opaca
                 ─► armazenamento de objectos
```

- O Browser Manager **não** escreve no armazenamento de objectos; entrega ao
  mesmo caminho de carregamento que a interface usa.
- Memória limitada a um pedaço; progresso e cancelamento; o cancelamento aborta
  a sessão de carregamento e apaga o temporário — sem ficheiro fantasma.
- A quota é perguntada antes (preflight) e decidida pelo Core no fim.
- Proveniência: `source_url` (sem query string) na actividade do ficheiro.
- Auditoria: «guardar ficheiro externo no Ocinye» é uma acção governada, e
  audita-se; a visita à página não.

### Transferir para o computador

- Desktop: diálogo nativo de guardar; o site não escolhe caminho. O nome
  sugerido é saneado (sem separadores, sem `..`, sem nomes reservados do
  Windows, sem caracteres de controlo, Unicode normalizado NFC, comprimento
  limitado).
- Web: a transferência é do navegador; o Ocinye Web não acede ao disco e di-lo.

### Carregar de Ocinye Files para um site (Desktop)

A pessoa escolhe **um** ficheiro no seletor do Ocinye (só leitura); o Core
autoriza a leitura; a casca materializa-o num temporário restrito e entrega-o
ao `<input type=file>` do webview; o temporário é apagado a seguir. Regista-se
`file.shared_external {host}`. Sem acesso a directórios, sem acesso repetido.

### Na Web

«Guardar no Ocinye Files» só onde a web o permite (o ficheiro já está no
computador → Carregar). Nunca por proxy.

## Alternatives

- **Casca escreve directamente no Garage.** Recusado: contorna quota, RBAC,
  validação e auditoria.
- **Descarregar tudo para memória e enviar.** Recusado: ficheiros de vários GB.

## Consequences

- Viagens obrigatórias: transferência pequena, grande com memória medida,
  cancelamento limpo, nomes hostis, ficheiro não autorizado não seleccionável.
