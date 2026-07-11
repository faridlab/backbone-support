-- Durable staging for support's IssueEscalated event (outbox rollout plan, P2). backbone-project SUBSCRIBES
-- to it to open a delivery project for the escalated ticket; a crash between the escalation CAS and the
-- in-proc publish would drop it (no project ever opens). Staging in the same tx as the CAS makes it survive.
CREATE TABLE IF NOT EXISTS support.outbox_events (
  id uuid PRIMARY KEY, event_type text NOT NULL, aggregate_type text NOT NULL, aggregate_id text NOT NULL,
  payload jsonb NOT NULL, occurred_at timestamptz NOT NULL, correlation_id text, causation_id text,
  version int NOT NULL DEFAULT 1, created_at timestamptz NOT NULL DEFAULT now(), published_at timestamptz );
CREATE INDEX IF NOT EXISTS idx_support_outbox_unpublished ON support.outbox_events (occurred_at) WHERE published_at IS NULL;
CREATE TABLE IF NOT EXISTS support.inbox_consumed (
  consumer text NOT NULL, event_id uuid NOT NULL, consumed_at timestamptz NOT NULL DEFAULT now(), PRIMARY KEY (consumer, event_id) );
