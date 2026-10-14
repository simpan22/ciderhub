-- Remove the spine rows too, so no 'comparison' events are left
-- behind without their typed row.
DELETE FROM events WHERE event_type = 'comparison';
DROP TABLE comparison_events;
