-- A batida do coração da ingestão automática de correio.
--
-- # O que faltava
--
-- O worker já percorria as caixas ligadas a cada `INGESTION_INTERVAL` — a
-- ingestão automática existe. O que não existia era **prova de que corre**. Com
-- zero caixas ligadas não há sequer um `last_synced_at` por caixa para observar:
-- uma passagem vazia não deixa rasto, e «o worker está a sincronizar» ficava
-- indistinguível de «alguém desligou o worker e ninguém reparou».
--
-- Sem essa prova, a capacidade `MailSync` não se pode declarar disponível sem
-- mentir: um health check nunca reporta saudável o que não verificou
-- (`CLAUDE.md` §62). Esta tabela é a coisa que se verifica.
--
-- # Uma passagem, não uma caixa
--
-- Cada passagem de `ingest_all` escreve aqui no fim — com sucesso ou com caixas
-- falhadas —, e é a passagem inteira que fica registada: quando terminou, quantas
-- caixas visitou, quantos cabeçalhos indexou, quantas caixas não conseguiu
-- actualizar. A razão de cada falha continua a viver **na caixa** que falhou
-- (`mailboxes.last_sync_error`); aqui fica só a contagem, que é o que a
-- capacidade lê para decidir entre disponível e degradado.
--
-- # Porque uma única linha
--
-- Porque não há duas ingestões automáticas. Há uma, e o que importa dela é o
-- último estado que deixou. O identificador é fixo e o `CHECK` impede uma
-- segunda linha: quem escreve faz `ON CONFLICT` sobre a que existe.
--
-- # Continuidade
--
-- Estado operacional efémero, classificado como tal em
-- `continuity::classification`. Viaja no dump por estar na base, e não faz
-- falta: num servidor restaurado a batida chega velha, o que lê correctamente
-- como «o worker ainda não correu aqui» até correr de facto.
CREATE TABLE mail_ingestion_heartbeat (
    id boolean PRIMARY KEY DEFAULT true CONSTRAINT mail_ingestion_heartbeat_singleton CHECK (id),
    last_swept_at timestamptz NOT NULL,
    mailboxes integer NOT NULL DEFAULT 0 CHECK (mailboxes >= 0),
    indexed integer NOT NULL DEFAULT 0 CHECK (indexed >= 0),
    failed integer NOT NULL DEFAULT 0 CHECK (failed >= 0)
);
