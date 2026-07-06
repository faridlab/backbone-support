-- Down: drop support.warranty_claims table
DROP TABLE IF EXISTS support.warranty_claims CASCADE;
DROP FUNCTION IF EXISTS support.warranty_claims_audit_timestamp() CASCADE;
