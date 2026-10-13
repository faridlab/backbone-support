-- Down: drop the declared foreign keys for support module
ALTER TABLE IF EXISTS support.service_level_priorities DROP CONSTRAINT IF EXISTS fk_service_level_priorities_sla_id;
