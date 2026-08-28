-- Avatar do membro.
--
-- Três estados possíveis, e um deles é sempre válido: iniciais derivadas do
-- nome. Não há aqui um caminho em que a pessoa fique sem representação — as
-- iniciais não dependem de storage, de rede nem de escolha, e por isso são o
-- fundo de todos os outros.
--
--   initials  → nada guardado; o nome basta
--   preset    → um identificador do catálogo do produto, versionado com o código
--   custom    → um objecto em storage privado, carregado pelo próprio membro
--
-- Um preset **não** é um upload. Escolher um avatar do produto não copia
-- ficheiro nenhum para o bucket: guarda-se o identificador e o renderizador
-- resolve o asset. Tratá-lo como upload duplicaria doze ficheiros por cada
-- membro da instituição, para representar uma escolha que cabe numa palavra.

ALTER TABLE people
    ADD COLUMN avatar_kind      VARCHAR(16) NOT NULL DEFAULT 'initials',
    -- Identificador do catálogo, não caminho nem URL: o cliente escolhe de uma
    -- lista fechada, e nada do que ele escreva chega a um sistema de ficheiros.
    ADD COLUMN avatar_preset    VARCHAR(32),
    ADD COLUMN avatar_object_id UUID REFERENCES storage_objects (id) ON DELETE SET NULL;

ALTER TABLE people
    ADD CONSTRAINT ck_people_avatar_kind
        CHECK (avatar_kind IN ('initials', 'preset', 'custom')),
    -- O estado e os seus dados andam juntos. Sem isto, `custom` sem objecto
    -- seria uma pessoa cujo avatar existe e não está em lado nenhum, e a
    -- interface teria de adivinhar o que fazer com ela.
    ADD CONSTRAINT ck_people_avatar_coherent
        CHECK (
            (avatar_kind = 'initials' AND avatar_preset IS NULL AND avatar_object_id IS NULL)
            OR (avatar_kind = 'preset' AND avatar_preset IS NOT NULL AND avatar_object_id IS NULL)
            OR (avatar_kind = 'custom' AND avatar_preset IS NULL AND avatar_object_id IS NOT NULL)
        );

COMMENT ON COLUMN people.avatar_kind IS
    'initials | preset | custom. Apresentação, nunca prova de identidade nem de autorização.';
