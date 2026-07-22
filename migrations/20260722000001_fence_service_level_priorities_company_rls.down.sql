DROP POLICY IF EXISTS service_level_priorities_company_isolation ON support.service_level_priorities;
ALTER TABLE support.service_level_priorities NO FORCE ROW LEVEL SECURITY;
ALTER TABLE support.service_level_priorities DISABLE ROW LEVEL SECURITY;
DROP INDEX IF EXISTS support.idx_service_level_priorities_company_id;
ALTER TABLE support.service_level_priorities DROP COLUMN IF EXISTS company_id;
