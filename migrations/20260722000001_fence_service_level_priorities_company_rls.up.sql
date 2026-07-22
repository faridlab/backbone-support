-- ADR-0010 Decision A: direct company_id + FORCE RLS on support.service_level_priorities.
-- Parent (service_level_agreements) is already fenced; the child was not.
-- Backfill is deterministic (parent carries non-null company_id).
ALTER TABLE support.service_level_priorities ADD COLUMN IF NOT EXISTS company_id UUID;

UPDATE support.service_level_priorities AS p
   SET company_id = s.company_id
  FROM support.service_level_agreements AS s
 WHERE p.sla_id = s.id
   AND p.company_id IS NULL;

ALTER TABLE support.service_level_priorities ALTER COLUMN company_id SET NOT NULL;
CREATE INDEX IF NOT EXISTS idx_service_level_priorities_company_id ON support.service_level_priorities (company_id);

ALTER TABLE support.service_level_priorities ENABLE ROW LEVEL SECURITY;
ALTER TABLE support.service_level_priorities FORCE  ROW LEVEL SECURITY;
DROP POLICY IF EXISTS service_level_priorities_company_isolation ON support.service_level_priorities;
CREATE POLICY service_level_priorities_company_isolation ON support.service_level_priorities
    FOR ALL
    USING      (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid)
    WITH CHECK (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid);
