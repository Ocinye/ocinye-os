# ocinye-system-service

O **Ocinye System Service**: a autoridade privilegiada, e única, sobre uma
máquina com o Ocinye OS instalado
([ADR-0031](../../docs/adrs/0031-rust-system-authority-and-native-system-surfaces.md)).

Dois binários:

- `ocinye-system-service serve [SOCKET]` — o daemon. Corre como root sob o
  systemd, escuta em `/run/ocinye/system.sock` e serve o contrato de
  [`ocinye-system-contracts`](../../crates/ocinye-system-contracts/README.md).
- `ocinye-system` — o cliente sem ecrã: um subcomando por capacidade, os mesmos
  códigos de recusa, a mesma autorização. Não é uma shell.

## O percurso de um pedido

```text
pedido ─▶ posição ─▶ autorização ─▶ prova fresca ─▶ validação ─▶ adaptador ─▶ auditoria ─▶ resultado
```

- **Posição** (`authority`): quem liga é o que o núcleo diz (credenciais do par
  do socket) — operador de recuperação (root), a conta da interface, a do Core,
  ou ninguém.
- **Autorização antes da validação**: quem não pode pedir não recebe a descrição
  de como pedir bem.
- **Adaptadores** (`linux`): `/proc` e `/sys` primeiro; uma ferramenta do sistema
  só através de `tool`, que fixa o caminho absoluto, limpa o ambiente, não usa
  shell, limita a saída e mata o processo no prazo.
- **Auditoria** (`audit`): linhas JSON em `/var/log/ocinye/system-audit.jsonl`,
  construídas de um resumo sem segredos.

## Estado

`IMPLEMENTED`, provado numa imagem `arm64` construída (`scripts/image-e2e.sh seat`). Fazem trabalho real: estado do sistema,
inventário, rede (leitura), energia, brilho, hora, idioma, teclado,
acessibilidade, privacidade (preferências), armazenamento (leitura), estado de
actualizações, diagnóstico, acesso remoto e recuperação (leitura). Wi-Fi,
Bluetooth, som, configuração de ecrãs e de rede, e a aplicação de actualizações
respondem `NOT_IMPLEMENTED`.

Ainda não existem o Session Service nem a ligação ao Core: o daemon trata todos
os clientes como presença, e o que precisa de membro, de operador ou de prova
fresca é recusado. O operador de recuperação (root) alcança tudo e fica
registado.

## Testes

```bash
cargo test -p ocinye-system-service
```

A matriz de recusas percorre o catálogo inteiro; os leitores correm contra uma
árvore `/sys` e `/proc` construída numa pasta temporária; o protocolo é exercido
por um socket Unix real. As acções que passam por uma ferramenta do sistema não
têm teste unitário: provam-se numa imagem a correr.
