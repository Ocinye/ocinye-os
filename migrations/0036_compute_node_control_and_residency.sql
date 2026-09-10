-- Institutional control and physical residency of a compute node.
--
-- # A distinção que faltava
--
-- Um `compute_node` tinha só `location_label` — texto livre. Isso não distingue
-- **quem controla o software e os dados** de **onde o hardware está fisicamente**.
-- O primeiro nó GPU real da Ocinye será uma máquina numa cloud de terceiros
-- (OVHcloud) a correr software da Ocinye: controlo institucional é da Ocinye, a
-- residência física é de terceiros. Alugar hardware não cede controlo, e o
-- modelo tem de o poder dizer sem inferir um do outro (ADR-0503).
--
-- Dois eixos, aditivos e independentes, reutilizando o vocabulário de residência
-- que já existe para o armazenamento (ADR-0201), sem criar uma taxonomia
-- paralela:
--
--   institutional_control  OCINYE | EXTERNAL           (quem controla)
--   physical_residency     UNDECLARED | THIRD_PARTY_CLOUD
--                          | OCINYE_CAMAMA | OCINYE_COLOCATION   (onde está)
--
-- `location_label` continua a ser a etiqueta legível («OVHcloud Europe»); estes
-- dois campos são a representação tipada. Nenhum relaxamento de política nasce
-- daqui: são descritivos, e a autorização continua onde estava.
--
-- Defaults honestos: um nó enrolado corre o Node Agent da Ocinye sob a nossa
-- credencial, por isso `OCINYE` é o controlo por omissão; a residência começa
-- `UNDECLARED` — não se afirma uma residência que não foi declarada.

ALTER TABLE compute_nodes
    ADD COLUMN institutional_control VARCHAR(16) NOT NULL DEFAULT 'OCINYE',
    ADD COLUMN physical_residency    VARCHAR(24) NOT NULL DEFAULT 'UNDECLARED';

ALTER TABLE compute_nodes
    ADD CONSTRAINT ck_compute_nodes_institutional_control
        CHECK (institutional_control IN ('OCINYE', 'EXTERNAL')),
    ADD CONSTRAINT ck_compute_nodes_physical_residency
        CHECK (physical_residency IN (
            'UNDECLARED', 'THIRD_PARTY_CLOUD', 'OCINYE_CAMAMA', 'OCINYE_COLOCATION'
        ));
