# D11 · Adaptação, modo escuro e idiomas

CSS: `static/ods-d11-adaptive.css` (carregar em último).

## Adaptação
| Largura | Modelo |
|---|---|
| ≥1100 | Desktop completo + janelas (quando G-05 existir) |
| 768–1099 | janelas simplificadas; data escondida na barra |
| <768 | uma app activa em ecrã inteiro; sem arrastar; popovers em folha inferior; Nye em cartão arredondado |

## Escuro
Os tokens de `ocinye-ds.css` já mudam com `data-theme`. Este ficheiro só corrige superfícies com gradiente claro fixo. Testar contraste em: menus de vidro sobre widgets, texto dourado (`--ods-gold-text` passa a `--ods-gold-300` no escuro se o contraste cair abaixo de 4,5:1).

## Idiomas
- pt canónico; en e fr completos em todos os `d*_catalog_entries.rs`. O portão de paridade (`i18n/completeness.rs`) fecha a CI se faltar uma chave.
- Verificar em fr (strings mais longas): «Rétablir la disposition», «Toutes les fenêtres», menus do Criar, cabeçalhos do Monitor.
- Conteúdo de membros não se traduz.

## Verificação final por ecrã (antes de pedir certificação)
1. `./scripts/verify.sh` passa (126 viagens).
2. Sem `style=` nem `<style>`/`<script>` inline (`grep` no HTML gerado).
3. Os marcadores `data-oc` de cada ecrã (listados em D2–D10) continuam no elemento com o mesmo papel.
4. Claro, escuro e sistema; 375, 768, 1280 e 1600 px; pt, en, fr.
