-- Identidade privilegiada ligada: administrar não é a identidade normal de
-- alguém.
--
-- # A propriedade
--
-- > **Uma identidade privilegiada ligada estabelece responsabilidade, e não
-- > herança de autoridade.**
--
-- Fidel Monteiro é uma pessoa. `Fidel Admin` é a identidade por onde ele exerce
-- autoridade administrativa. São linhas diferentes porque o modelo é hoje
-- 1 pessoa = 1 email = 1 credencial — e refazer essa fundação para acomodar isto
-- seria mexer em quatrocentos sítios com produção viva.
--
-- O que se acrescenta é a ligação que torna a relação explícita, e as
-- invariantes que impedem essa ligação de se transformar num atalho.
--
-- # O que isto **não** é
--
-- Não é `Person → N Principals`. O Ocinye OS não suporta N identidades
-- arbitrárias por pessoa, e documentá-lo como se suportasse seria descrever um
-- sistema que não existe. É uma coisa mais pequena e mais verdadeira: uma
-- identidade operacional privilegiada pode declarar a quem pertence.

-- O tipo de identidade operacional que a linha representa.
--
-- Não um booleano `is_admin`: a propriedade não é sobre autorização — é sobre
-- **o que a linha é**. Uma identidade privilegiada continua a passar pela mesma
-- política que todas as outras; o que a distingue é não ser uma pessoa.
ALTER TABLE people
    ADD COLUMN identity_kind TEXT NOT NULL DEFAULT 'human',
    ADD COLUMN belongs_to_person_id UUID REFERENCES people (id) ON DELETE RESTRICT;

-- Toda a gente que já existe é humana.
--
-- Não se infere `privileged` de ter `platform_admin`: administrar é uma coisa,
-- ser uma identidade privilegiada é outra, e adivinhar aqui transformaria contas
-- existentes em algo que ninguém decidiu.
UPDATE people SET identity_kind = 'human' WHERE identity_kind IS NULL;

ALTER TABLE people
    ADD CONSTRAINT ck_people_identity_kind
        CHECK (identity_kind IN ('human', 'privileged')),

    -- Uma pessoa não pertence a ninguém; uma identidade privilegiada pertence
    -- sempre a alguém. As duas metades, porque só a primeira deixaria passar
    -- uma identidade privilegiada órfã — que é uma conta com autoridade e sem
    -- ninguém responsável por ela.
    ADD CONSTRAINT ck_people_link_matches_kind
        CHECK (
            (identity_kind = 'human'      AND belongs_to_person_id IS NULL)
         OR (identity_kind = 'privileged' AND belongs_to_person_id IS NOT NULL)
        ),

    -- Ninguém pertence a si próprio. Uma auto-ligação satisfaria a regra acima
    -- e não diria nada.
    ADD CONSTRAINT ck_people_no_self_link
        CHECK (belongs_to_person_id IS NULL OR belongs_to_person_id <> id);

-- O pai tem de ser humano.
--
-- Um `CHECK` não consegue consultar outra linha, e por isso isto é um trigger.
-- Sem ele, `privilegiada → privilegiada → humana` passava: uma cadeia onde a
-- responsabilidade se dilui a cada salto, e onde a pergunta «quem responde por
-- esta conta?» deixa de ter uma resposta.
CREATE OR REPLACE FUNCTION people_link_must_be_human() RETURNS trigger AS $$
DECLARE
    tipo_do_pai TEXT;
BEGIN
    IF NEW.belongs_to_person_id IS NULL THEN
        RETURN NEW;
    END IF;

    SELECT identity_kind INTO tipo_do_pai
      FROM people WHERE id = NEW.belongs_to_person_id;

    IF tipo_do_pai IS DISTINCT FROM 'human' THEN
        RAISE EXCEPTION
            'uma identidade privilegiada pertence a uma pessoa, e não a outra identidade privilegiada'
            USING ERRCODE = 'check_violation';
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_people_link_must_be_human
    BEFORE INSERT OR UPDATE OF belongs_to_person_id, identity_kind ON people
    FOR EACH ROW EXECUTE FUNCTION people_link_must_be_human();

-- Quem pertence a quem, para a auditoria resolver a pessoa por trás da
-- identidade sem uma varredura.
CREATE INDEX ix_people_belongs_to ON people (belongs_to_person_id)
    WHERE belongs_to_person_id IS NOT NULL;
