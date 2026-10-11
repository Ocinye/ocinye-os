# A D013 sobre a D011 — dependências provisórias

Do pacote D013 do Claude Design (`D013_D011_PROVISIONAL_DEPENDENCIES.md`),
traduzido para o estado do código. A D013 fase A de código assenta em
`feat/design-d011 @ 3c1d4fa` (a D011 ainda **não certificada**); o Design
assentava em `baf5031`. Entre os dois só entraram o endurecimento pré-cloud
(digests das imagens de terceiros, sudo sem palavra-passe provado) e
documentação.

Cada tipo ou função da D013 que se apoia num contrato da D011 está marcado
`[P, PD-nn]` no código. Na fase B (sincronização com a D011 certificada) cada
linha volta a ser verificada; nenhuma passa a «estável» por omissão.

## Mudanças observadas na fase A de código

| ID | Observação |
|---|---|
| PD-03 | **SATISFEITA** em `e855fee`: cada imagem de terceiros por `repo:tag@sha256` (F-01). |
| PD-18 | **PROVADA** a 2026-10-05: instalação completa num `operador` com sudo NOPASSWD sintético, pelo caminho real (`sudo -n`), V01–V16 PASS (F-07). |
| PD-20 | D011 instala o Docker sem versões fixadas; a imagem fixa-as no construtor e segura-as (`apt-mark hold`) — reprovar a detecção na fase B. |
| PD-25 | amd64: a certificação cloud da D011 continua `NOT_RUN`. |

## Tabela (do Design)

| ID | D011 source contract | D011 file/module @ baf5031 | D013 consumer | D013 impact | Security | Phase B action | Change tolerance | Status |
|---|---|---|---|---|---|---|---|---|
| PD-01 | Release manifest `ReleaseManifest` schema 1 | `crates/ocinye-installer-contracts/src/manifest.rs` | B02, ImageContentManifest, descriptor | release identity, artifacts, images, bootstrap hash | yes | diff struct; update `release` block of image manifest | additive fields OK; renamed/removed fields → contract update | PROVISIONAL_REVERIFY |
| PD-02 | Canonical JSON | `contracts/src/canonical.rs` | both D013 manifests, claim state | hashes and signatures | yes | diff; rerun R1 | none (any change re-hashes) | PROVISIONAL_SECURITY_CRITICAL |
| PD-03 | Third-party images (reference only, no digest) | `manifest.rs` `ThirdPartyImage`; `infra/compose/docker-compose.production.yml` | B08, offline install, SBOM | digest identity required | yes | confirm F-01 fixed | must gain digests | PROVISIONAL_EXPECTED_TO_CHANGE |
| PD-04 | Docker prerequisites (key fingerprint, 4 packages) | `manifest.rs` `Prerequisites`, `DOCKER_PACKAGES`; `openpgp.rs` | B07 | pre-baked runtime trust | yes | diff; confirm versions policy (F-03) | fingerprint change → rebuild | PROVISIONAL_SECURITY_CRITICAL |
| PD-05 | Bootstrap protocol v1 (`Command`, `Event`, `ServerFacts`) | `contracts/src/protocol.rs`; `lib.rs` `BOOTSTRAP_PROTOCOL` | E-02, E-08 | image facts, execute guard | yes | diff enum; add E-02/E-08 | additive | PROVISIONAL_EXPECTED_TO_CHANGE |
| PD-06 | Installation plan + phases P01–P16 + safety classes | `contracts/src/plan.rs` | E-04 | prebaked P02/P09 | yes | add `PrebakedVerify` | additive | PROVISIONAL_EXPECTED_TO_CHANGE |
| PD-07 | Installation journal | `contracts/src/journal.rs`; `bootstrap/src/state.rs` | claim release guard (< P06) | knowable state | medium | confirm journal path/format | path stable | PROVISIONAL_REVERIFY |
| PD-08 | Installation receipt schema 1 | `contracts/src/receipt.rs` | E-05 `image_source` | provenance in receipt | low | add field, schema 2 | additive | PROVISIONAL_EXPECTED_TO_CHANGE |
| PD-09 | First-admin bootstrap + one-time credential | `protocol.rs` `ClaimSecret`; core `bootstrap-admin` | none directly (after CLAIMED) | must remain post-claim only | yes | confirm unchanged | — | PROVISIONAL_STABLE |
| PD-10 | Instance provisioning (P11) | `bootstrap/src/phases.rs`, `install/ocinye` | CL-7, journeys | no Instance before CLAIMED | yes | E-08 guard | — | PROVISIONAL_SECURITY_CRITICAL |
| PD-11 | Distribution provisioning | `install/ocinye --distribution` | journeys only | none | no | confirm | — | PROVISIONAL_STABLE |
| PD-12 | Access Endpoint integration (`endpoint-seed`, P12) | core `endpoint-seed`; `phases.rs` | journeys only | none | no | confirm | — | PROVISIONAL_STABLE |
| PD-13 | TLS integration (operator / self-signed) | `controller/src/opverify.rs`; ADR-0024 | journey P | none | yes | confirm | — | PROVISIONAL_STABLE |
| PD-14 | Hardware discovery | `contracts/src/hardware.rs`; `bootstrap/src/hardware.rs`; ADR-0025 | D012 boundary, OIE scope | OIE does not duplicate | no | confirm `ProviderMode::Off` only value | — | PROVISIONAL_STABLE |
| PD-15 | Lifecycle states | `contracts/src/verification.rs` `LifecycleState` | state machine §5, console C28 | display only | low | diff names | rename → reference update | PROVISIONAL_REVERIFY |
| PD-16 | Read-only verify capabilities (`verify-schema/-instance/-endpoints/-admin-bootstrap`) | core-server | journeys | none | no | confirm | — | PROVISIONAL_STABLE |
| PD-17 | Host-key pinning (Ed25519/ECDSA only, explicit trust, mismatch stop) | `controller/src/hostkeys.rs`, `ssh.rs` | claim anti-MITM, sshd host key types | core claim security | yes | re-prove CL-5 | none | PROVISIONAL_SECURITY_CRITICAL |
| PD-18 | Privilege `SudoNoPassword` | `contracts/src/preflight.rs` | `ocinye` account | operator account model | yes | prove a D011 run with NOPASSWD sudo (F-07) | — | PROVISIONAL_REVERIFY |
| PD-19 | Preflight: Ubuntu 24.04 by os-release, firewall classification (ufw active + 22 allowed → add 80/443) | `preflight.rs`; `bootstrap/src/preflight.rs` | image keeps Ubuntu os-release; ufw pre-active | must not classify image as foreign/external firewall | medium | re-run preflight on a D013 image | — | PROVISIONAL_REVERIFY |
| PD-20 | Runtime detection: compatible Docker already present → not reinstalled | `preflight.rs` `judge_runtime` | pre-baked Docker | P04 skipped | medium | re-prove with image-owned held packages | — | PROVISIONAL_REVERIFY |
| PD-21 | Minimum resources `MIN_RAM_MB 3500`, `MIN_DISK_GB 15`, `MIN_CPU 2` | `preflight.rs` | QCOW2 virtual size 24 GiB, OIE disk min 20 GB | sizing | no | update if changed | numbers may change | PROVISIONAL_REVERIFY |
| PD-22 | Fixed paths (`/srv/ocinye`, `/etc/ocinye`, `/var/lib/ocinye-installer`, `/tmp/ocinye-bootstrap-*`) | `contracts/src/lib.rs` `paths` | clone-safety rows, image layout | absent in image | yes | diff | rename → matrix update | PROVISIONAL_STABLE |
| PD-23 | Release bundle layout (`scripts/release-bundle.sh`) | `scripts/release-bundle.sh`, `services/release-tool` | B02, B08, payload layout | which files are kept | low | diff | additive | PROVISIONAL_REVERIFY |
| PD-24 | Installer UI frame, classes, strings | `apps/installer/ui/{oc-installer.css,oc-base.css,strings.js}` | `reference/d013/installer.*` | references reuse | no | re-render & revalidate | any | PROVISIONAL_REVERIFY |
| PD-25 | amd64 runtime certification | `docs/install/installer.md` ("Cloud amd64 — por fazer") | primary arch | `PRIMARY_ARCH_CERTIFICATION` | — | read D011 amd64 evidence; update RESOURCE_PROFILE | — | PROVISIONAL_EXPECTED_TO_CHANGE |
| PD-26 | ADR-0022…0025 (Proposed) | `docs/adrs/0022–0025` | D013 ADR references | decisions referenced | — | confirm Accepted | — | PROVISIONAL_REVERIFY |

Contagem: 26 dependências — `PROVISIONAL_STABLE` 7 · `PROVISIONAL_REVERIFY` 10 ·
`PROVISIONAL_SECURITY_CRITICAL` 4 · `PROVISIONAL_EXPECTED_TO_CHANGE` 5 (antes das
observações acima; PD-03 e PD-18 ficam resolvidas pela D011 endurecida, mas só
saem da tabela na fase B).
