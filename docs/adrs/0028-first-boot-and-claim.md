# ADR-0028 — Primeiro arranque e reclamação (UNCLAIMED → CLAIMED)

- **Estado:** Proposed
- **Domínio:** Foundation
- **Impacto:** HIGH
- **Depende de:** ADR-0027 · ADR-0022 (Proposed) · ADR-0023 (Proposed)
- **Data:** 2026-10-05

Importada do pacote D013 do Claude Design (fase A, provisória sobre a D011), no formato desta biblioteca.

## Context

Uma imagem reutilizável não pode trazer identidade, palavra-passe nem chave partilhada, e não pode ser «de alguém» antes de alguém provar que a controla. Um endereço IP não identifica uma máquina.

## Decision

1. O primeiro arranque gera a identidade da máquina (machine-id, chaves de anfitrião SSH Ed25519/ECDSA, identificador de arranque), confere o release incluído e entra em **UNCLAIMED**. Sem identidade ou com integridade falhada não há UNCLAIMED.
2. A reclamação reutiliza o SSH da D011: conta restrita `ocinye-claim` (qualquer chave oferecida, comando forçado, sem tty nem reencaminhamento, activa só em UNCLAIMED) que fala um protocolo tipado fechado.
3. Prova: um **código de emparelhamento de 125 bits**, de uso único, válido 15 minutos, 5 tentativas, mostrado só a pedido na consola da máquina; ou uma chave pública já colocada pela plataforma cloud ou por pen no OIE.
4. Antes de qualquer segredo, o operador compara a impressão digital da chave de anfitrião em grupos com a consola; o Installer fixa-a como na D011.
5. Duas fases: `Enroll` (código consumido, chave do operador autorizada, `CLAIM_IN_PROGRESS` com prazo de 10 minutos) e `Confirm` pela conta `ocinye` com o `claim_id`. Sem confirmação, desfaz-se e volta UNCLAIMED com código novo. Transições sob `flock` com compare-and-swap.
6. Só depois de CLAIMED a D011 provisiona; o bootstrap recusa executar antes.
7. Sem palavras-passe: root bloqueado, `ocinye` só com chaves, `ocinye-claim` sem palavra-passe.

## Consequences

Nenhum serviço de rede novo; reclamação concorrente impossível; código inútil depois de usado ou expirado. Quem tem a consola física tem controlo total (sem cifra de disco v1), o que fica escrito.

## Alternatives

Serviço HTTP de reclamação próprio (mais superfície, segundo protocolo); código de seis dígitos (entropia insuficiente como único segredo); palavra-passe inicial (proibida).

## Implementação — D013 fase A de código (2026-10-05)

`PROVISIONAL_PENDING_D011_CERTIFICATION`. As regras são funções puras em
`ocinye-image-contracts::claim` (código Crockford 5×5, 15 min, 5 tentativas,
três códigos esgotados bloqueiam 15 min a duplicar até 4 h, prazo de
confirmação de 10 min, `claim_id` de uso único); `services/firstboot` aplica-as
sob um `flock`, com escrita atómica. A chave a inscrever é a da sessão SSH: o
`AuthorizedKeysCommand` (utilizador sem privilégios) guarda a chave oferecida e
devolve um comando forçado com a impressão digital; `claim-serve` corre como
root por **uma** regra sudoers e confere que a chave guardada é essa. O Docker
só é activado em CLAIMED. A recusa de execução do bootstrap antes de CLAIMED
(extensão E-08 da D011) **não** está feita: é uma alteração da D011 que espera
pela certificação dela.
