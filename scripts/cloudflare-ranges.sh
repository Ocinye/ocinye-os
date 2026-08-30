#!/bin/sh
# Os ranges da Cloudflare, da fonte oficial, agora.
#
# # Porque isto não é uma lista no repositório
#
# Porque a Cloudflare acrescenta ranges. Um origin que confie numa lista velha
# ou rejeita tráfego legítimo, ou — pior — deixa de reconhecer como Cloudflare
# um peer que é, e passa a tratar o `CF-Connecting-IP` dele como não confiável.
#
# O sentido inverso é o perigoso: um range **a mais** na lista significaria
# confiar no cabeçalho de quem não é a Cloudflare, e nesse momento qualquer
# pessoa escolhe o seu próprio endereço.
#
# Por isso a lista busca-se no momento da instalação e regenera-se quando muda.
#
#     ./scripts/cloudflare-ranges.sh > /etc/ocinye/nginx/01-cloudflare-ranges.conf
set -eu

{
  printf '# Gerado de https://www.cloudflare.com/ips-v4 e ips-v6 em %s.\n' "$(date -u +%FT%TZ)"
  printf '# Regenerar com: scripts/cloudflare-ranges.sh\n'
  curl -fsS https://www.cloudflare.com/ips-v4 | sed 's/^/set_real_ip_from /; s/$/;/'
  curl -fsS https://www.cloudflare.com/ips-v6 | sed 's/^/set_real_ip_from /; s/$/;/'
}
