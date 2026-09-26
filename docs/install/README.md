# Instalar o Ocinye OS

Uma Instância do Ocinye OS instala-se num anfitrião Linux a partir de um **pacote
de release** ([ADR-0701](../adrs/0701-release-bundle-and-host-installer.md)). O
resultado é o mesmo layout, o mesmo Compose e o mesmo proxy que a produção da
Ocinye usa.

## O que o anfitrião precisa

| | |
|---|---|
| Sistema | Linux, 64 bits (a arquitectura do pacote: `amd64` ou `arm64`) |
| Docker | Engine 24 ou mais recente, com o plugin `docker compose` (v2) |
| Memória | 3,5 GB, verificados pelo instalador (o mínimo medido é trabalho da Parte 15) |
| Disco | 15 GB livres em `/srv` |
| Rede | portas 80 e 443 livres; acesso aos registries das imagens de terceiros |
| Ferramentas | `bash`, `curl`, coreutils |

**Não precisa** de GPU, de chave de fornecedor de IA, de fornecedor externo nem de
correio configurado. Uma Instância nova arranca com a IA indisponível — que é o
estado certo até alguém ligar um nó ou registar um fornecedor.

## Instalar

```bash
tar -xf ocinye-os-<sha>.tar && cd ocinye-os-<sha>
```

```bash
sudo ./install/ocinye install \
  --domain os.exemplo.org \
  --instance-name "Exemplo" \
  --profile business \
  --name "Pessoa Responsável" --email pessoa@exemplo.org \
  --admin-name "Pessoa Responsável (Admin)" --admin-email admin@exemplo.org \
  --tls provided --cert /caminho/cert.pem --key /caminho/key.pem
```

- `--profile` escolhe as aplicações com que a Instância nasce: `research`,
  `business`, `education` ou `personal`
  ([ADR-0014](../adrs/0014-instance-profiles-and-application-activation.md)).
  Não há perfil por omissão.
- `--tls self-signed` gera um certificado para laboratório; em uso real, forneça
  o seu.
- `--public-url` só é preciso quando o endereço público não é
  `https://<domínio>` (outra porta, por exemplo).

O instalador, por esta ordem: verifica o anfitrião, confere as somas do pacote,
instala o release em `/srv/ocinye`, gera a configuração e os segredos em
`/etc/ocinye` (0600), carrega as imagens, levanta a base de dados e o
armazenamento, cria a Instância e o primeiro administrador, levanta os serviços e
confirma de fora que o Workspace responde.

No fim imprime a **credencial temporária** do administrador, uma vez. No primeiro
acesso define-se a palavra-passe e enrola-se o segundo factor, que é obrigatório
para identidades privilegiadas.

## Depois de instalar

- **Guarde a raiz de selagem** (`OCINYE_SEALING_KEY` em `/etc/ocinye/core.env`)
  fora do anfitrião. Sem ela, um backup não se lê
  ([continuidade](../adrs/0700-institutional-continuity-and-portability.md)).
- `sudo ./install/ocinye status` mostra o release e o estado dos serviços.
- Com systemd, `ocinye.service` levanta a Instância depois de um reboot.

## Desinstalar

`sudo ./install/ocinye uninstall --purge` pára tudo e apaga dados, configuração e
releases. **Não há volta.** Serve anfitriões descartáveis e a prova de instalação.

## A prova

`scripts/install-e2e.sh <pacote>` instala num anfitrião Linux limpo e descartável,
entra por um browser, trabalha, destrói, e repete de raiz com outro perfil. É a
evidência de que esta página descreve o que acontece.

## O que ainda não existe

- Actualizar uma Instância instalada (Parte 10) e backup/restauro com o
  instalador (Parte 11).
- Instalação sem rede (imagens de terceiros dentro do pacote).
- Pacote assinado (Parte 17): hoje as somas provam integridade, não origem.
