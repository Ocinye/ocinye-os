-- Mail drafts gain an authored rich-text body.
--
-- A draft's `body` stays the plain-text projection (the editor's innerText),
-- canonical and always present. `body_html` holds the member's sanitised rich
-- HTML when they formatted the message, and is NULL for a plain-text draft. The
-- HTML that lands here has already crossed the outbound sanitisation boundary
-- (ADR-0415); it is never raw member markup.
--
-- Additive and reversible from the previous state: existing drafts keep their
-- text body and a NULL html body, which reads exactly as "plain text".

ALTER TABLE mail_drafts
    ADD COLUMN body_html TEXT;

COMMENT ON COLUMN mail_drafts.body_html IS
    'Sanitised authored rich-text HTML (ADR-0415); NULL for a plain-text draft. '
    'The plain-text projection lives in body.';
