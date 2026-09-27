# Resultados da certificação de hardware

Gerado por `scripts/hardware-certification.sh` a 2026-09-27, release
`e62a1e5dc795` (proof (sem LTO; nunca para produção)),
num anfitrião arm64 com 8 núcleos.
Sem GPU, sem fornecedor de IA. Limiares: instalação ≤ 900s,
p95 da página de entrada ≤ 800 ms, cada passo da viagem ≤ 5000 ms.

| Classe | Resultado | Instalação | Serviços em repouso | Anfitrião inteiro em repouso | Entrada p50 / p95 (ms) | Passos da viagem (ms) | Razão |
|---|---|---|---|---|---|---|---|
| 2 vCPU · 4g | PASS | 76s | 78 MiB | 295.4MiB | 4 / 6 | entrar 1153 abrir_ficheiros 198 criar_e_guardar_nota 2306 prompt_responde 540  | — |
| 4 vCPU · 8g | PASS | 99s | 81 MiB | 295MiB | 4 / 5 | entrar 1236 abrir_ficheiros 201 criar_e_guardar_nota 1741 prompt_responde 361  | — |
