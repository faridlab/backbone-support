-- Migration: replace the SLA active boolean with a status enum
-- support.service_level_agreements carried `is_active BOOLEAN NOT NULL DEFAULT TRUE`; the
-- tree-wide convention is one `status` enum field per lifecycle (see docs/refactoring-schema in
-- the serpa workspace). FALSE rows are written to 'inactive'; TRUE rows ride the new column's
-- DEFAULT 'active' (no UPDATE needed). The enum type is created unqualified so it lands beside
-- the module's other enum types (public), where the generated sqlx type_name resolves.

DO $$ BEGIN
    CREATE TYPE service_level_agreement_status AS ENUM ('active', 'inactive');
EXCEPTION WHEN duplicate_object THEN NULL; END $$;

ALTER TABLE support.service_level_agreements ADD COLUMN status service_level_agreement_status NOT NULL DEFAULT 'active';
UPDATE support.service_level_agreements SET status = 'inactive' WHERE NOT is_active;
ALTER TABLE support.service_level_agreements DROP COLUMN is_active;

DROP INDEX IF EXISTS support.idx_service_level_agreements_company_id_is_active;
CREATE INDEX IF NOT EXISTS idx_service_level_agreements_company_id_status ON support.service_level_agreements (company_id, status);
