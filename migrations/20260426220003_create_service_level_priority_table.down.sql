-- Down: drop support.service_level_priorities table
DROP TABLE IF EXISTS support.service_level_priorities CASCADE;
DROP FUNCTION IF EXISTS support.service_level_priorities_audit_timestamp() CASCADE;
