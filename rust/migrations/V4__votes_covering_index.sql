-- Covers rankings aggregation so scanning a week's votes never reads the
-- table for `rank`; the UNIQUE (ballot_id, quarterback_id) index lacks it.
CREATE INDEX votes_ballot_cover ON votes (ballot_id, quarterback_id, rank);
