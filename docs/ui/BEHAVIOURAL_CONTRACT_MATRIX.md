# Matriz de contratos de comportamento (UI Reset)

> Cada teste que olhava para a apresentação legada, e o que lhe aconteceu. Um teste
> nunca desaparece em silêncio: ou passa a olhar para o comportamento por um
> marcador `data-oc`, ou fica aqui como **à espera da UI nova** — e o pacote do
> Claude Design que substituir o ecrã traz a medição de volta, sobre o desenho novo.

| Teste | O que media | Destino |
|---|---|---|
| `o_sino_abre_um_painel_que_diz_o_que_chegou` (browser) | abre sem navegar, desenha a notificação, fecha com `Escape` | **convertido** — `[data-oc="notificacao"]`, título e legenda |
| idem | superfície igual à do painel da conta; ícone por tipo | **à espera da UI nova** — acabamento da UI legada |
