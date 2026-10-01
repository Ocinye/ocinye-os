-- ---------------------------------------------------------------------------
-- D010 · Code: a Distribuição activa e o contexto activo da sessão
-- ---------------------------------------------------------------------------
--
-- ADR-0019 §3: uma Distribuição activa de cada vez, por sessão, revalidada a
-- cada pedido (activada ∧ acesso do membro). ADR-0625 §2: o contexto vive
-- dentro dela e é reposto quando ela muda. Os dois são da sessão do Core —
-- não do browser — para que uma revogação ou uma desactivação se veja no
-- pedido seguinte, venha ele de onde vier.
--
-- Uma sessão sem Distribuição activa (um cliente da API que nunca entrou numa)
-- não é afectada: a Distribuição decide a experiência, não a autoridade.

ALTER TABLE sessions
    ADD COLUMN active_distribution VARCHAR(16)
        CHECK (active_distribution IN ('research', 'business', 'personal', 'education')),
    ADD COLUMN active_context_kind VARCHAR(16)
        CHECK (active_context_kind IN ('organisation', 'unit', 'project', 'personal')),
    ADD COLUMN active_context_id UUID,
    ADD CONSTRAINT ck_sessions_context_needs_distribution
        CHECK (active_context_kind IS NULL OR active_distribution IS NOT NULL),
    ADD CONSTRAINT ck_sessions_context_id
        CHECK ((active_context_kind IN ('unit', 'project')) = (active_context_id IS NOT NULL));
