# ADR-0029 — Identidades: imagem, máquina, instalação, Instância, organização, utilizador

- **Estado:** Proposed
- **Domínio:** Foundation
- **Impacto:** MEDIUM
- **Depende de:** ADR-0027 · ADR-0028 · [ADR-0013](0013-general-purpose-os-instance-and-node.md)
- **Data:** 2026-10-05

Importada do pacote D013 do Claude Design (fase A, provisória sobre a D011), no formato desta biblioteca.

## Decision

| Identidade | Nasce | Igual em todos os clones? | Onde vive |
|---|---|---|---|
| Release Ocinye | build do release | sim | `MANIFEST.json` |
| Imagem Ocinye (`ocinye-os-<release>-r<N>`) | build da imagem | sim | `OcinyeImageManifest`, `/etc/ocinye/image.json` |
| Base Ubuntu (série + SHA-256) | upstream | sim | manifesto da imagem |
| Máquina (machine-id, chaves de anfitrião) | primeiro arranque | **não** | `/etc/machine-id`, `/etc/ssh` |
| Arranque (`ocb-…`) | primeiro arranque | **não** | `/var/lib/ocinye-firstboot` |
| Reclamação (`claim_id`, chave dona) | reclamação | **não** | estado do firstboot |
| Instalação (`inst-…`) | D011 | **não** | diário da D011 |
| Instância, organização, membro | D011 + Core | **não** | base de dados |

Regras: nenhuma identidade de baixo deriva de outra de cima; uma imagem contém só identidades de software; nenhuma identidade de máquina, reclamação, instalação, Instância, organização ou membro existe numa imagem; metadados cloud dão acesso à máquina, nunca identidade de organização; o recibo da instalação regista a origem (imagem, digest, base, método de reclamação) sem segredos.

## Implementação — D013 fase A de código (2026-10-05)

`PROVISIONAL_PENDING_D011_CERTIFICATION`. O construtor limpa offline as
identidades de máquina (`machine-id` vazio, chaves de anfitrião, estado do
cloud-init, sementes, diários, utilizador de construção) e a inspecção falha a
construção se alguma ficar; o primeiro arranque gera-as e o `ImageSourceReceipt`
(`ocinye-image-contracts::receipt`) regista a origem sem segredos. A integração
no recibo da D011 (`image_source`, esquema 2) é da D011 e fica por fazer.
