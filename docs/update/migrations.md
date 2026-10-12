# Migrações e reversão

Um release pode mudar o esquema da Instância. A janela de reversão (o release
anterior é guardado até ao seguinte) só é segura se o release anterior ainda
funcionar sobre o esquema novo. Daí a regra:

> **Expandir → migrar → contrair.** Um release de funcionalidade só
> **acrescenta** esquema compatível com o anterior. O que retira declara-se, e
> só chega a máquinas que já não dependem do que ele retira.

| `--migrations` | Significa | Reversão |
|---|---|---|
| `none` | não toca no esquema | trivial |
| `expand` | só acrescenta (tabelas, colunas com omissão, índices) | o checkpoint da base é reposto: o Core anterior não arranca sobre um esquema mais novo |
| `contract:N` | retira o que só releases anteriores a `N` usavam | só oferecido a máquinas com número ≥ `N`; a reversão é a reposição do checkpoint, e diz-se assim |

Um `contract:N` não é oferecido a uma máquina abaixo de `N`, e a pré-verificação recusa-o outra vez antes de instalar.

## O que a reversão faz

A Instância volta pelo `install/ocinye` do próprio release:

1. antes da troca, checkpoint da base (`pg_dump`);
2. troca, migração, portão de saúde;
3. se o portão falha, ou se o motor reverte depois da activação: o release
   anterior volta e, quando o esquema já tinha mudado, a base é reposta a
   partir do checkpoint.

**Limite honesto:** a reversão da Instância é a reposição do checkpoint
tirado antes da actualização. Numa reversão automática a janela é a do portão
de saúde (minutos, sem sessões, porque os serviços estão a reiniciar). Numa
reversão **pedida** depois de um compromisso — nas Definições, pelo menu de
arranque ou pela CLI — **tudo o que a Instância escreveu desde a actualização
perde-se**, porque o Core anterior recusa arrancar sobre um esquema mais novo.
É uma ferramenta de recuperação, não um «desfazer»; quem a pede é avisado
disso.

## Provas

- `ocinye upgrade` com uma segunda migração que rebenta: recusado e revertido
  sem restos no esquema (`scripts/upgrade-e2e.sh`, D011).
- Cenário `update` do E2E das imagens: o release sintético doente traz uma
  migração `expand`; depois da reversão automática a tabela não existe; o N+1
  traz a mesma e ela fica.
