# ADR-0031 — Autoridade do sistema em Rust e superfícies nativas que só apresentam

- **Estado:** Proposed
- **Domínio:** Foundation
- **Impacto:** FOUNDATIONAL
- **Depende de:** ADR-0004 · ADR-0013 · ADR-0030
- **Data:** 2026-10-11

## Context

Uma máquina com o Ocinye OS instalado precisa de quem mude a rede, o som, o
brilho, a hora, a energia e as actualizações — e de quem decida se isso pode ser
pedido. Até aqui só existia o Core, que governa a instituição, e o Workspace, que
é um cliente do Core. Nenhum dos dois é autoridade sobre a máquina, e nenhum
deve passar a ser.

O risco conhecido de uma interface gráfica de sistema é ela tornar-se a
autoridade por acidente: um botão desactivado a fazer de permissão, um
`exec("nmcli …")` a fazer de API, um processo gráfico a correr como root porque
duas definições precisam de privilégio.

## Decision

> **Rust controla o sistema; as interfaces nativas do sistema usam TypeScript,
> HTML e CSS para apresentar o sistema. As aplicações podem usar o seu framework
> aprovado, mas nunca obtêm autoridade directa sobre o sistema.**

1. **Um serviço privilegiado, o Ocinye System Service**, em Rust, com módulos
   tipados, é a única autoridade sobre a máquina instalada. Não se cria um
   daemon por subsistema.
2. **Um catálogo fechado de capacidades** (`ocinye-system-contracts`) é a única
   definição do que se pode pedir: cada capacidade tem pedido e resposta
   tipados, classe de autorização, prova fresca exigida, classe de auditoria,
   dependência de hardware, e se é oferecida com a sessão bloqueada, pelo
   cliente sem ecrã e como proposta do Nye. **Nenhuma capacidade recebe um
   comando, um executável, um ambiente ou um caminho a abrir**, e um teste mede
   isso.
3. **IPC local por socket Unix**, tramas com comprimento limitado, JSON com
   esquemas fechados, e um aperto de mão com a versão do contrato: um cliente de
   outro contrato é recusado e nada é invocado.
4. **Quem pede é o que o núcleo diz que é** (credenciais do par do socket), e
   não o que o cliente afirma. Três posições: presença na máquina, membro com
   sessão desbloqueada, e operador de recuperação (root na consola — identidade
   Linux, não um membro). O que precisa de permissão institucional pergunta ao
   Core; se o Core não responde, a resposta é `AUTHORITY_UNAVAILABLE`, nunca um
   palpite.
5. **A autorização vem antes da validação**, e nada chega a um adaptador antes
   de passar as quatro portas: posição, autorização, prova fresca, validação.
6. **Prova fresca por capacidade** (`NONE`, `CONFIRM`, `LOCAL_REAUTH`,
   `CORE_STEP_UP`), ligada a um pedido exacto e de uso único. Não existe
   palavra-passe de administrador.
7. **Adaptadores por ordem de preferência**: interfaces do núcleo, D-Bus,
   biblioteca nativa, e só então um subprocesso — com caminho absoluto de uma
   tabela fechada, argumentos construídos em Rust, sem shell, ambiente limpo,
   saída limitada, prazo, e o texto da ferramenta nunca como mensagem.
8. **As ligações TypeScript são geradas dos tipos Rust**, versionadas, e um
   teste regenera-as e falha com qualquer diferença.
9. **Todos os clientes usam a mesma autoridade**: a interface nativa (Tauri), o
   cliente sem ecrã `ocinye-system` e as propostas que o Core encaminha. O
   Workspace continua Rust/Leptos, não é reescrito e não recebe ponte nativa.
10. **Auditoria na autoridade**, em Rust, construída de um resumo sem segredos
    que cada pedido oferece de si próprio — nunca serializando o pedido.

## Alternatives

- **A interface chama ferramentas** (`nmcli`, `pactl`, `systemctl`) — é a
  arquitectura que esta decisão proíbe: a segurança passa a depender de cada
  linha de TypeScript.
- **Um serviço D-Bus próprio com polkit** — convencional, mas põe a política em
  ficheiros de regras fora dos tipos e dos testes do repositório.
- **Um daemon por subsistema** — mais processos privilegiados, mais superfície,
  sem isolamento que a v1 justifique.
- **O Core como autoridade da máquina** — misturaria a autoridade institucional
  com a de um computador, e uma máquina com o Core em baixo deixava de se poder
  ligar à rede ou reiniciar.

## Consequences

O que existe com esta decisão é o primeiro bloco: os contratos, o cliente, o
serviço com as portas e a matriz de recusas, os leitores Linux e as acções de
energia, hora, idioma, teclado, brilho e acesso remoto. Wi-Fi, Bluetooth, som e
configuração de ecrãs estão no catálogo e respondem `NOT_IMPLEMENTED`; o
inventário di-lo. O Session Service (sessão local, bloqueio, desbloqueio sem o
Core) e a ligação ao Core ainda não existem: até lá, o serviço trata todos como
presença, e o que precisa de membro, operador ou prova fresca é recusado. O
compositor, a interface gráfica e a integração na imagem são passos seguintes.
Nada disto correu ainda numa imagem do Ocinye OS: os adaptadores que mudam o
sistema têm testes de protocolo e de leitura, não prova numa máquina.

**Actualização, 2026-10-12.** O segundo bloco de adaptadores existe: Wi-Fi
(iwd) e Bluetooth (BlueZ) por D-Bus, com o `zbus`; som por ALSA; ecrãs pelos
conectores DRM; endereçamento por netplan; microfone e câmara. Continuam
`NOT_IMPLEMENTED` a escolha de entrada de som e a configuração de ecrãs, pelas
razões no [README do serviço](../../services/system-service/README.md). Estão
testados contra duplos e não provados numa máquina.
