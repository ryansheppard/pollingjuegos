-- Per-week rankings totals kept current by triggers, so rankings reads ~one row
-- per quarterback instead of aggregating every vote on each request. Triggers
-- rather than application code so cascades and hand-written SQL stay in sync.
-- Points are AP-style: rank 1 earns 15 (ballots::BALLOT_SIZE) down to 1.
CREATE TABLE week_scores (
    poll_week_id INTEGER NOT NULL REFERENCES poll_weeks (id) ON DELETE CASCADE,
    quarterback_id INTEGER NOT NULL REFERENCES quarterbacks (id) ON DELETE RESTRICT,
    points INTEGER NOT NULL,
    votes INTEGER NOT NULL CHECK (votes > 0),
    PRIMARY KEY (poll_week_id, quarterback_id)
) WITHOUT ROWID, STRICT;

CREATE TRIGGER votes_score_insert AFTER INSERT ON votes BEGIN
    INSERT INTO week_scores (poll_week_id, quarterback_id, points, votes)
    SELECT b.poll_week_id, NEW.quarterback_id, 16 - NEW.rank, 1
    FROM ballots b WHERE b.id = NEW.ballot_id
    ON CONFLICT DO UPDATE SET points = points + excluded.points, votes = votes + 1;
END;

-- The DELETE must run first: decrementing first would turn a 2 into a 1 that
-- the DELETE then wrongly removes.
CREATE TRIGGER votes_score_delete AFTER DELETE ON votes BEGIN
    DELETE FROM week_scores
    WHERE poll_week_id = (SELECT poll_week_id FROM ballots WHERE id = OLD.ballot_id)
      AND quarterback_id = OLD.quarterback_id
      AND votes = 1;
    UPDATE week_scores SET points = points - (16 - OLD.rank), votes = votes - 1
    WHERE poll_week_id = (SELECT poll_week_id FROM ballots WHERE id = OLD.ballot_id)
      AND quarterback_id = OLD.quarterback_id;
END;

-- ON DELETE CASCADE removes the ballot before its votes, leaving
-- votes_score_delete unable to find the week; deleting the votes first avoids that.
CREATE TRIGGER ballots_score_delete BEFORE DELETE ON ballots BEGIN
    DELETE FROM votes WHERE ballot_id = OLD.id;
END;

-- Rewriting a vote or moving a ballot between weeks would bypass the totals;
-- submit_ballot replaces votes by delete + insert instead.
CREATE TRIGGER votes_no_update BEFORE UPDATE ON votes BEGIN
    SELECT RAISE(ABORT, 'votes are immutable; delete and reinsert to keep week_scores in sync');
END;

CREATE TRIGGER ballots_no_week_change BEFORE UPDATE OF poll_week_id ON ballots
WHEN NEW.poll_week_id IS NOT OLD.poll_week_id BEGIN
    SELECT RAISE(ABORT, 'ballots cannot move weeks; week_scores would drift');
END;

INSERT INTO week_scores (poll_week_id, quarterback_id, points, votes)
SELECT b.poll_week_id, v.quarterback_id, sum(16 - v.rank), count(*)
FROM ballots b
JOIN votes v ON v.ballot_id = b.id
GROUP BY b.poll_week_id, v.quarterback_id;
