# A Autoridade de Segredos

Como o Ocinye OS guarda as credenciais que uma Instância configura para os seus
fornecedores e integrações. Decisão:
[ADR-0110](../adrs/0110-instance-secrets-authority.md).

```text
administrador ──► Core (Autoridade de Segredos) ──► selado em repouso
                                                     (instance-secrets/v1)
                         serviço do Core no âmbito ──► usa no momento
                                   aplicação ◄── recebe o resultado, nunca o segredo
```

| Propriedade | Como |
|---|---|
| Cifrado em repouso | ChaCha20-Poly1305, subchave HKDF própria da raiz `OCINYE_SEALING_KEY` |
| Cifrado em trânsito | HTTPS até ao proxy; rede interna até ao Core |
| Nunca devolvido | nenhuma rota de leitura; a API mostra metadados e os últimos quatro caracteres |
| Âmbito | cada segredo nomeia o serviço do Core que o pode abrir |
| Rotação | o valor novo sobrepõe o antigo; a versão sobe |
| Revogação | o criptograma é apagado |
| Auditoria | criar, rodar, revogar e usar, sempre sem o valor |
| Registos | nenhum `Debug` de configuração imprime credenciais |
| Browser | nenhum segredo é persistido no browser, nem em `localStorage` |
| Quem | `platform.administer`, privilegiado, com segundo factor |

## Rotas

`GET`/`POST /api/v1/instance/secrets`,
`POST /api/v1/instance/secrets/{id}/rotate`,
`POST /api/v1/instance/secrets/{id}/revoke`. Não existe `GET …/{id}`.

## Provas

`services/core-server/tests/secrets_http.rs`: criar, não conseguir ler, usar no
âmbito e não fora dele nem noutra Instância, recusar quem não administra, rodar,
revogar — e procurar os dois valores em claro em **todas** as linhas de **todas**
as tabelas, incluindo a auditoria.
