-- Favoritos: a marca de um membro sobre um dos seus ficheiros.
--
-- > **Um membro pode marcar um ficheiro seu como favorito; a marca é dele, não
-- > do ficheiro, e não muda quem o vê nem onde ele vive.**
--
-- É preferência, não autoridade. Marcar não concede acesso nem classificação —
-- só junta o ficheiro à vista «Favoritos» de quem o marcou. Por isso a chave é o
-- par (pessoa, ficheiro), e a marca desaparece com qualquer um dos dois.
CREATE TABLE file_favourites (
    person_id  UUID NOT NULL REFERENCES people (id) ON DELETE CASCADE,
    file_id    UUID NOT NULL REFERENCES files (id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),

    PRIMARY KEY (person_id, file_id)
);

-- A vista «Favoritos» de uma pessoa lê-se pela pessoa, mais recente primeiro.
CREATE INDEX ix_file_favourites_pessoa
    ON file_favourites (person_id, created_at DESC);
