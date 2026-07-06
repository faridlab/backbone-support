-- Down: drop support.service_level_agreements table
DROP TABLE IF EXISTS support.service_level_agreements CASCADE;
DROP FUNCTION IF EXISTS support.service_level_agreements_audit_timestamp() CASCADE;
