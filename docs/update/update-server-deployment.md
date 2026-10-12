# Servidor de actualizações — runbook

> **Estado: `NOT_RUN`.** A máquina não existe. Este runbook é o que se fará
> quando existir; nada aqui foi executado contra um servidor.

O servidor **serve ficheiros estáticos** e mais nada. Não tem chaves, não
assina, não decide: a confiança vem dos metadados TUF, assinados na máquina de
publicação ([publicar](publishing.md)). Um servidor comprometido pode, no
máximo, deixar de servir — o motor recusa metadados alterados, antigos ou
congelados.

## Requisitos

- Ubuntu Server 24.04 LTS, 2 vCPU, 2 GiB, disco para os releases guardados
  (cada um tem o tamanho do pacote da D011 mais a camada nativa).
- Um nome (`updates.<domínio>.com`) com TLS. O TLS é defesa em profundidade e
  privacidade; a integridade é do TUF.
- Ou, em vez de servidor: um bucket de armazenamento de objectos com leitura
  pública e um CDN à frente. O formato é o mesmo.

## 1. Preparar a máquina

```bash
sudo apt-get update && sudo apt-get install -y nginx
sudo install -d -o root -g root -m 0755 /srv/ocinye-updates
```

Nenhuma conta da máquina de publicação; nenhuma chave privada copiada para cá.

## 2. Servir

`/etc/nginx/sites-available/ocinye-updates`:

```nginx
server {
    listen 443 ssl http2;
    server_name updates.empresa.com;
    ssl_certificate     /etc/ssl/ocinye-updates/fullchain.pem;
    ssl_certificate_key /etc/ssl/ocinye-updates/privkey.pem;
    root /srv/ocinye-updates;
    autoindex off;
    location ~ /metadata/timestamp\.json$ { add_header Cache-Control "no-cache"; }
    location ~ /metadata/ { add_header Cache-Control "max-age=60"; }
    location ~ /targets/  { add_header Cache-Control "public, max-age=31536000, immutable"; }
}
```

Os alvos são endereçados pelo conteúdo (`<sha256>.<nome>`): podem ficar em
cache para sempre. O `timestamp.json` nunca.

## 3. Publicar

Na máquina de publicação, depois de `ocinye-release publish` e `verify`:

```bash
rsync -a --delay-updates --exclude '*.tmp' REPO/stable/ servidor:/srv/ocinye-updates/stable/
```

Ordem: primeiro os alvos, depois `targets`/`snapshot`, por fim
`timestamp.json` — `--delay-updates` aproxima isso; uma máquina que leia a meio
vê o estado anterior, coerente.

## 4. Manter

- `ocinye-release refresh` agendado antes de o `timestamp` expirar (canal
  `stable`: um dia), seguido do `rsync`.
- Monitorizar a idade do `timestamp.json` servido; alarme a metade da validade.
- Os releases antigos ficam: uma máquina atrasada precisa do seu caminho.

## 5. Aceitação

De uma máquina instalada com uma imagem desse canal:

```bash
sudo ocinye-system update repository https://updates.empresa.com/stable/metadata/ https://updates.empresa.com/stable/targets/
sudo ocinye-system update check
sudo ocinye-system --json update status
```

Aceite quando: a verificação passa, o release oferecido é o publicado, e uma
cópia do `timestamp.json` alterada num byte, servida no lugar da verdadeira, é
recusada.
