# ADR-0026 — Assinatura dos releases e das imagens do Ocinye OS

- **Estado:** Proposed
- **Domínio:** Foundation
- **Impacto:** HIGH
- **Depende de:** [ADR-0701](0701-release-bundle-and-host-installer.md) · [ADR-0022](0022-graphical-remote-installer.md) (Proposed)
- **Data:** 2026-10-05

Importada do pacote D013 do Claude Design (fase A, provisória sobre a D011), no formato desta biblioteca.

## Context

A D011 instala um pacote verificado por somas (`RELEASE_SIGNING = NOT_IMPLEMENTED`): as somas provam que o pacote chegou como saiu, não quem o fez. A D013 publica imagens arrancáveis para pessoas que não as construíram. Sem assinatura, uma imagem num espelho alterado é indistinguível da oficial.

## Decision

1. Formato **minisign (Ed25519)**, assinatura destacada sobre `SHA256SUMS` (ordenado, LF, cada ficheiro publicado, incluindo `MANIFEST.json` e `IMAGE_MANIFEST.json` em forma canónica). O comentário confiável assinado diz o canal: `ocinye-os <id>-r<N> <arch> channel=<stable|candidate|development> sha256sums=<…> ts=<…>`.
2. Três papéis: **raiz** offline (assina `trusted-keys.json` e `revocations.json`), **release** anual em token de hardware no ambiente de release, fora do CI (assina `candidate` e `stable`), **desenvolvimento** anual (só `development`, nunca confiável como estável).
3. A chave pública da raiz vai compilada no Installer e publicada no repositório; as listas assinadas viajam com o Installer ou num ficheiro do operador. O Installer nunca as vai buscar sozinho.
4. `PUBLIC_IMAGE_RELEASE` exige assinatura de produção verificada. Sem ela: `DEVELOPMENT`/`INTERNAL`.
5. Promoção `candidate → stable` reassina os mesmos bytes. Bytes novos = revisão nova.
6. Nenhuma chave privada no Git, na imagem, no CI em claro, no pacote de Design, em imagens de contentores ou em registos.

## Consequences

- Operadores verificam com o Installer ou com `minisign -Vm` + `sha256sum -c`.
- Revogação e rotação dependem de cerimónias registadas e de listas assinadas pela raiz.
- O comprometimento da raiz exige uma actualização do Installer com nova raiz.

## Alternatives

OpenPGP (modelo de confiança e formato maiores do que o necessário); Sigstore/cosign keyless (dependência de serviços online e de identidade OIDC para verificar); só somas (não prova origem).

## Implementação — D013 fase A de código (2026-10-05)

`PROVISIONAL_PENDING_D011_CERTIFICATION`. Implementado em
`ocinye-image-contracts::signing`: a gramática fechada do comentário confiável
(`SignedStatement::parse` recusa campos a mais, ordem, grafia e números não
canónicos), a regra de papel e canal (uma chave `development` nunca assina
`stable`, mesmo que a lista a autorize), revogação de chave (com
`compromised_at`) e de artefacto, validade, e `verify_release`, que verifica a
assinatura Ed25519 pré-hashed com o verificador de referência
[`minisign-verify`](https://crates.io/crates/minisign-verify) e só depois
confronta o comentário com os ficheiros. `SHA256SUMS` aceita caminhos relativos
seguros (o Design dizia «ficheiros publicados»; a saída tem `sbom/`,
`inventory/`, `provenance/`). Só há chave de **desenvolvimento**, gerada uma
vez em `~/.cache/ocinye-image-builder/dev-signing/`, nunca no Git nem numa
imagem. Raiz, release, `trusted-keys.json` assinado e cerimónias: `NOT_IMPLEMENTED`.
