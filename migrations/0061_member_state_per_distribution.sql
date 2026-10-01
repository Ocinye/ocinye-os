-- ---------------------------------------------------------------------------
-- D010 · Claude Design; integrado e corrigido pela Code (ver os blocos «Code»).
-- 0061 · Disposição do Desktop e fixações por membro + Distribuição
-- ---------------------------------------------------------------------------
--
-- Âmbito-alvo (ADR-0624 R2.2): membro + Instância + Distribuição. Um membro pertence a uma
-- organização (people.organisation_id NOT NULL, 0001) e person_id é global, por isso a chave
-- (person_id, distribution) já determina a Instância. Fase B: a Distribuição de cada linha vem
-- da organização DO MEMBRO, por junção — não de «a» organização.
--
-- Migração: a linha existente de cada membro passa a ser da Distribuição que a Instância tinha.
-- Não se copia para as outras três: quem entrar numa Distribuição nova segue a predefinição dela.
-- O fundo vive dentro de `layout` e passa, com ele, a ser por Distribuição (decisão D010).

ALTER TABLE member_desktop_layouts ADD COLUMN distribution VARCHAR(16);
UPDATE member_desktop_layouts m SET distribution = o.profile
  FROM people p JOIN organisations o ON o.id = p.organisation_id WHERE p.id = m.person_id;
ALTER TABLE member_desktop_layouts
    ALTER COLUMN distribution SET NOT NULL,
    ADD CONSTRAINT ck_member_desktop_layouts_distribution
        CHECK (distribution IN ('research', 'business', 'personal', 'education')),
    DROP CONSTRAINT member_desktop_layouts_pkey,
    ADD PRIMARY KEY (person_id, distribution);

ALTER TABLE member_app_pins ADD COLUMN distribution VARCHAR(16);
UPDATE member_app_pins m SET distribution = o.profile
  FROM people p JOIN organisations o ON o.id = p.organisation_id WHERE p.id = m.person_id;
ALTER TABLE member_app_pins
    ALTER COLUMN distribution SET NOT NULL,
    ADD CONSTRAINT ck_member_app_pins_distribution
        CHECK (distribution IN ('research', 'business', 'personal', 'education')),
    DROP CONSTRAINT member_app_pins_pkey,
    ADD PRIMARY KEY (person_id, distribution);

-- «Repor» continua a apagar a linha — agora só a da Distribuição activa:
--   DELETE FROM member_desktop_layouts WHERE person_id = $1 AND distribution = $2;
-- Sem linha = segue a predefinição efectiva DESSA Distribuição (nunca a de outra).
--
-- Compatibilidade: um binário anterior lê por person_id e encontraria várias linhas depois de
-- uma segunda Distribuição. Por isso a ordem é: 0060 → binário novo → 0061 → activar segunda.
-- Nenhuma segunda Distribuição é activada antes de 0061 (portão K4).
