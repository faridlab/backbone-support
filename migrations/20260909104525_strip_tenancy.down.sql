-- Hand-authored (user-owned). Not regenerated.
--
-- Best-effort restore sketch for the tenancy strip (ADR-0029). This is a breaking module
-- release against dev-stage databases: the down re-adds the company_id column as nullable
-- with its plain indexes and the company isolation policy shape, but restores NO data —
-- rows written after the strip (or after the decorator re-keyed them) carry org_unit_id
-- only. The composing service's tenancy decorator remains the live fence; treat this
-- down as a schema-shape sketch for archaeology, not a usable rollback.

ALTER TABLE support.issues                   ADD COLUMN IF NOT EXISTS company_id uuid;
ALTER TABLE support.service_level_agreements ADD COLUMN IF NOT EXISTS company_id uuid;
ALTER TABLE support.service_level_priorities ADD COLUMN IF NOT EXISTS company_id uuid;
ALTER TABLE support.warranty_claims          ADD COLUMN IF NOT EXISTS company_id uuid;

-- The strip's restored tenant-free lookups go away again (the company-leading variants
-- would need company data this sketch does not restore).
DROP INDEX IF EXISTS support.idx_issues_status;
DROP INDEX IF EXISTS support.idx_issues_agreement_status;
DROP INDEX IF EXISTS support.idx_service_level_agreements_status;
DROP INDEX IF EXISTS support.idx_warranty_claims_status;

CREATE INDEX IF NOT EXISTS idx_issues_company_id_status
    ON support.issues (company_id);
CREATE INDEX IF NOT EXISTS idx_issues_company_id_agreement_status
    ON support.issues (company_id);
CREATE INDEX IF NOT EXISTS idx_service_level_agreements_company_id_status
    ON support.service_level_agreements (company_id);
CREATE INDEX IF NOT EXISTS idx_service_level_priorities_company_id
    ON support.service_level_priorities (company_id);
CREATE INDEX IF NOT EXISTS idx_warranty_claims_company_id_status
    ON support.warranty_claims (company_id);

-- The historical fence policies (issues_company_isolation and siblings on
-- service_level_agreements / service_level_priorities / warranty_claims) are NOT
-- recreated: the strip also removed the company_id data they fenced against, and a
-- USING clause on an all-NULL column would lock every row out of every session.
