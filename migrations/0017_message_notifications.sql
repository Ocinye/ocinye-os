-- O sino passa a saber das Mensagens.
--
-- O sino já existe (`0014_calendar.sql`) e as menções já eram uma razão para
-- tocar (`0016_messaging.sql`). Falta a razão mais comum: alguém escreveu.
--
-- # Porque um tipo novo e não uma menção
--
-- Porque são duas coisas diferentes para quem lê: «o Fidel escreveu» e «o Fidel
-- chamou por ti». Um sino que as diga da mesma maneira obriga a abrir para
-- saber qual foi.

ALTER TABLE notifications DROP CONSTRAINT ck_notifications_kind;
ALTER TABLE notifications ADD CONSTRAINT ck_notifications_kind
    CHECK (kind IN (
        'reminder',
        'event_cancelled',
        'event_invited',
        'message_received',
        'message_mention'
    ));

-- Uma notificação por conversa e por pessoa, enquanto estiver por ler.
--
-- # Porque não uma por mensagem
--
-- Porque uma conversa activa encheria o sino com quarenta linhas iguais, e
-- quarenta linhas iguais são zero informação. A que existe actualiza-se: o
-- sino diz «há coisas por ler nesta conversa», que é o que uma pessoa precisa
-- de saber para decidir abri-la.
--
-- O índice é parcial porque a regra só vale para o que está por ler: o
-- histórico de notificações lidas pode ter tantas quantas houve.
CREATE UNIQUE INDEX uq_notifications_conversa_por_ler
    ON notifications (recipient_id, resource_id, kind)
    WHERE read_at IS NULL AND resource_type = 'conversation';
