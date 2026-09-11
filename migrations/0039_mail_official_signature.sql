-- Assinatura institucional oficial no correio.
--
-- O correio da Ocinye passa a acrescentar uma assinatura institucional às
-- mensagens enviadas (ADR-0414). Esta preferência diz se o membro a quer — por
-- omissão, sim: é a identidade da instituição, e o padrão institucional é
-- assiná-la.
--
-- A coluna `signature` que já existe **não** se toca: continua a guardar a
-- linha pessoal do membro, acrescentada por cima da assinatura oficial. Uma
-- assinatura guardada não se perde nem muda de significado.

ALTER TABLE mail_preferences
    ADD COLUMN official_signature BOOLEAN NOT NULL DEFAULT true;

COMMENT ON COLUMN mail_preferences.official_signature IS
    'Se o membro acrescenta a assinatura institucional oficial ao enviar (ADR-0414).';

COMMENT ON COLUMN mail_preferences.signature IS
    'Linha pessoal opcional do membro, acrescentada acima da assinatura oficial. Texto simples; escapado na projecção HTML.';
