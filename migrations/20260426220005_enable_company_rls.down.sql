-- Down: remove the company RLS fence for support module

-- Reverse the company RLS fence for support.issues
DROP POLICY IF EXISTS issues_company_isolation ON support.issues;
ALTER TABLE support.issues NO FORCE ROW LEVEL SECURITY;
ALTER TABLE support.issues DISABLE ROW LEVEL SECURITY;

-- Reverse the company RLS fence for support.service_level_agreements
DROP POLICY IF EXISTS service_level_agreements_company_isolation ON support.service_level_agreements;
ALTER TABLE support.service_level_agreements NO FORCE ROW LEVEL SECURITY;
ALTER TABLE support.service_level_agreements DISABLE ROW LEVEL SECURITY;

-- Reverse the company RLS fence for support.warranty_claims
DROP POLICY IF EXISTS warranty_claims_company_isolation ON support.warranty_claims;
ALTER TABLE support.warranty_claims NO FORCE ROW LEVEL SECURITY;
ALTER TABLE support.warranty_claims DISABLE ROW LEVEL SECURITY;

