-- Fecha o `TRUNCATE` na trilha de auditoria.
--
-- A migration 0001 instalou dois triggers `BEFORE UPDATE` e `BEFORE DELETE`
-- `FOR EACH ROW` sobre `audit_events`, e a documentação passou a descrever a
-- tabela como append-only.
--
-- `TRUNCATE` não é `DELETE`. Não percorre linhas, por isso um trigger de linha
-- nunca chega a correr, e a tabela ficava vazia sem que nada objectasse. Quem
-- pudesse escrever na base podia apagar a evidência de o ter feito — que é
-- exactamente a manipulação de auditoria que o modelo de ameaças enumera
-- (`CLAUDE.md` §32, §37).
--
-- Um trigger `BEFORE TRUNCATE` é ao nível do comando e é a única forma de o
-- recusar. Como os outros dois, esta é uma barreira contra a aplicação — e não
-- contra um superutilizador da base de dados, que pode sempre remover o
-- trigger. Trabalho deliberado de retenção continua a ser uma migration
-- privilegiada que o remove e o repõe.

CREATE TRIGGER trg_audit_events_no_truncate
    BEFORE TRUNCATE ON audit_events
    FOR EACH STATEMENT EXECUTE FUNCTION audit_events_are_append_only();

COMMENT ON TRIGGER trg_audit_events_no_truncate ON audit_events IS
    'A trilha de auditoria não é esvaziável pela aplicação, nem linha a linha nem de uma vez.';
