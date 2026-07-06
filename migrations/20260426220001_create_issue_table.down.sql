-- Down: drop support.issues table
DROP TABLE IF EXISTS support.issues CASCADE;
DROP FUNCTION IF EXISTS support.issues_audit_timestamp() CASCADE;
