# Língua canónica

```
OCINYE CANONICAL PRODUCT LANGUAGE = pt

`pt` significa Português de Portugal.

LOCALES DE PRODUTO SUPORTADOS =
    pt
    en
    fr

O IDIOMA DO UTILIZADOR É UMA PREFERÊNCIA DE APRESENTAÇÃO.

O IDIOMA NUNCA MUDA:
    O ESTADO DE DOMÍNIO
    A AUTORIDADE
    AS PERMISSÕES
    A IDENTIDADE DE RECURSO
    OS IDS DE RECURSO
    O CONTEÚDO ESCRITO POR MEMBROS
```

## Regras permanentes

1. Cada mensagem de produto tem **uma** origem canónica em português.
2. `pt` é Português de Portugal; `fr` é Français de France; `en` é inglês
   profissional de convenção única e consistente.
3. `en` e `fr` preservam a **mesma semântica** de produto que o `pt`.
4. A tradução nunca muda permissões, significados de domínio, semântica de ciclo
   de vida, terminologia institucional, significado de segurança nem identidade
   de recurso.
5. Falta de tradução em tempo de execução → queda para `pt`. **Nunca** se mostra
   uma chave crua.
6. Os identificadores internos são exactamente `pt`, `en`, `fr` — nunca `pt-PT`,
   `en-US`, `fr-FR`. Valores externos normalizam-se à entrada.
7. Para certificação de produção, `en` e `fr` alcançam paridade total de
   catálogo (portão de CI: `pt/en/fr` em falta = 0).

## Convenção do inglês

O inglês do Ocinye usa a **grafia britânica/internacional** (`organisation`,
`favourite`, `centre`), coerente com o português europeu e o francês do produto.
A convenção é única e mantém-se; o identificador é `en` de qualquer forma.
