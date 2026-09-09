-- Hand-authored (user-owned). Not regenerated.
--
-- Strip every company-fence artifact from the support tables (ADR-0029): the module is
-- tenant-agnostic; org scoping is installed by the COMPOSING service's tenancy decorator,
-- never by the module. Dropped here, per table: the company-leading indexes, the
-- <table>_company_isolation RLS policy, and the company_id column itself.
--
-- The module-local outbox mirror (support.outbox_events) is NOT touched: its company_id
-- column and fence stay, matching the framework outbox table it mirrors.
--
-- Ordering guard (the decorator must run FIRST on any database with data): the module
-- never moves tenancy data. A table is safe to strip when EITHER
--   a) it carries org_unit_id with no NULLs — the decorator backfilled it from company_id —
--      or b) it is empty (a fresh database: the earlier chain files created it empty).
-- Otherwise the strip RAISEs, naming the decorator step, rather than dropping a column
-- that still holds the only tenancy key. The file is re-runnable (every drop is IF EXISTS
-- and the tracker has no checksums), so a failed run retries cleanly after the decorator
-- lands.
--
-- RLS enable/force flags are deliberately NOT touched: the decorator owns those now.

DO $$
DECLARE
    t text;
    has_org boolean;
    org_nulls bigint;
    total bigint;
    offenders text := '';
BEGIN
    FOREACH t IN ARRAY ARRAY['issues', 'service_level_agreements', 'service_level_priorities', 'warranty_claims']
    LOOP
        IF to_regclass(format('support.%I', t)) IS NULL THEN
            CONTINUE; -- chain not fully applied on this database; nothing to strip
        END IF;

        SELECT EXISTS (
                   SELECT 1 FROM information_schema.columns
                   WHERE table_schema = 'support' AND table_name = t AND column_name = 'org_unit_id'
               )
        INTO has_org;

        EXECUTE format('SELECT count(*) FROM support.%I', t) INTO total;

        IF has_org THEN
            EXECUTE format(
                'SELECT count(*) FROM support.%I WHERE org_unit_id IS NULL', t)
            INTO org_nulls;
        ELSE
            org_nulls := total; -- no org column: every row's only tenancy key is company_id
        END IF;

        IF has_org AND org_nulls = 0 THEN
            CONTINUE; -- decorator backfilled: safe
        END IF;
        IF total = 0 THEN
            CONTINUE; -- empty table (fresh database): safe
        END IF;
        offenders := offenders || format(' support.%s (%s rows, %s rows not covered by org_unit_id);', t, total, org_nulls);
    END LOOP;

    IF offenders <> '' THEN
        RAISE EXCEPTION 'refusing to strip company_id — these tables are not yet covered by the tenancy decorator:%. Apply the composing service''s tenancy decorator (it backfills org_unit_id from company_id) and re-run; it is the only step that moves tenancy data.', offenders;
    END IF;
END $$;

-- ── issues ─────────────────────────────────────────────────────────────────────
DROP INDEX IF EXISTS support.idx_issues_company_id_status;
DROP INDEX IF EXISTS support.idx_issues_company_id_agreement_status;
DROP POLICY IF EXISTS issues_company_isolation ON support.issues;
ALTER TABLE support.issues DROP COLUMN IF EXISTS company_id;

-- ── service_level_agreements ───────────────────────────────────────────────────
DROP INDEX IF EXISTS support.idx_service_level_agreements_company_id_status;
DROP INDEX IF EXISTS support.idx_service_level_agreements_company_id_is_active;
DROP POLICY IF EXISTS service_level_agreements_company_isolation ON support.service_level_agreements;
ALTER TABLE support.service_level_agreements DROP COLUMN IF EXISTS company_id;

-- ── service_level_priorities ───────────────────────────────────────────────────
DROP INDEX IF EXISTS support.idx_service_level_priorities_company_id;
DROP POLICY IF EXISTS service_level_priorities_company_isolation ON support.service_level_priorities;
ALTER TABLE support.service_level_priorities DROP COLUMN IF EXISTS company_id;

-- ── warranty_claims ────────────────────────────────────────────────────────────
DROP INDEX IF EXISTS support.idx_warranty_claims_company_id_status;
DROP POLICY IF EXISTS warranty_claims_company_isolation ON support.warranty_claims;
ALTER TABLE support.warranty_claims DROP COLUMN IF EXISTS company_id;

-- ── Restore the tenant-free lookups ────────────────────────────────────────────
-- The schema's tenant-free status/agreement-status lookups are installed idempotently so a
-- database stripped in place regains the indexes the schema declares. The per-unit scoping
-- indexes are POSTURE and are owned by the composing service's tenancy decorator — they are
-- intentionally NOT restored here (the pre-fence global forms would collapse every unit's rows
-- into one namespace).
CREATE INDEX IF NOT EXISTS idx_issues_status
    ON support.issues (status);
CREATE INDEX IF NOT EXISTS idx_issues_agreement_status
    ON support.issues (agreement_status);
CREATE INDEX IF NOT EXISTS idx_service_level_agreements_status
    ON support.service_level_agreements (status);
CREATE INDEX IF NOT EXISTS idx_warranty_claims_status
    ON support.warranty_claims (status);
