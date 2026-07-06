-- Down: drop enum types for support module
DROP TYPE IF EXISTS warranty_status CASCADE;
DROP TYPE IF EXISTS issue_priority CASCADE;
DROP TYPE IF EXISTS agreement_status CASCADE;
DROP TYPE IF EXISTS issue_status CASCADE;
