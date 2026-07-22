-- ADR-0011: fence support.outbox_events by company_id. The outbox carries tenant event data
-- (IssueEscalated etc.) in its payload; extract company_id to a real column and fence it so a
-- non-super role can't read another tenant's events. inbox_consumed stays infra (no PII).
ALTER TABLE support.outbox_events ADD COLUMN IF NOT EXISTS company_id UUID;

UPDATE support.outbox_events
   SET company_id = (payload ->> 'company_id')::uuid
 WHERE company_id IS NULL;

ALTER TABLE support.outbox_events ALTER COLUMN company_id SET NOT NULL;
CREATE INDEX IF NOT EXISTS idx_support_outbox_company_id ON support.outbox_events (company_id);

ALTER TABLE support.outbox_events ENABLE ROW LEVEL SECURITY;
ALTER TABLE support.outbox_events FORCE  ROW LEVEL SECURITY;
DROP POLICY IF EXISTS outbox_events_company_isolation ON support.outbox_events;
CREATE POLICY outbox_events_company_isolation ON support.outbox_events
    FOR ALL
    USING      (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid)
    WITH CHECK (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid);
