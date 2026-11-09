// Generated from api-types by `bazel run //frontend:update_api_types`. Do not edit.

export const BALLOT_SIZE = 15

export const RANKINGS_SIZE = 10

export type Week = { id: number, season: number, week: number, closes_at: string | null, is_closed: boolean, 
/**
 * The week clients open on: the oldest still taking votes, or the newest
 * if every week is closed. Exactly one week in a list has it set.
 */
is_default: boolean, };

export type Me = { id: number, };

export type Team = { id: number, name: string, abbreviation: string, };

export type Quarterback = { id: number, name: string, team: Team | null, };

export type QuarterbackStats = { season: number, games: number, dropbacks: number, epa: number | null, epa_per_dropback: number | null, cpoe: number | null, qbr: number | null, 
/**
 * Ranks only cover QBs with enough dropbacks to qualify; null otherwise or
 * when the stat itself is missing.
 */
qualified: boolean, qualified_count: number, qbr_rank: number | null, epa_rank: number | null, cpoe_rank: number | null, 
/**
 * Set exactly when the matching rank is.
 */
qbr_band: RankBand | null, epa_band: RankBand | null, cpoe_band: RankBand | null, };

/**
 * Which slice of the league a stat rank falls in; clients color badges by it.
 */
export type RankBand = "top" | "mid" | "low";

/**
 * /quarterbacks rows carry season-to-date stats; ballot and ranking payloads do not.
 */
export type QuarterbackWithStats = { 
/**
 * Season-to-date including playoffs; null when nflverse has no line for this QB.
 */
stats: QuarterbackStats | null, id: number, name: string, team: Team | null, };

export type BallotRequest = { 
/**
 * Ordered from #1 to #15; ranks are derived from list position.
 */
quarterback_ids: Array<number>, };

export type BallotEntry = { rank: number, quarterback: Quarterback, };

export type Ballot = { season: number, week: number, entries: Array<BallotEntry>, };

export type RankingEntry = { rank: number, points: number, votes: number, previous_rank: number | null, quarterback: Quarterback, };

export type Rankings = { season: number, week: number, ballots: number, rankings: Array<RankingEntry>, };

/**
 * Body of `POST /accounts/desktop/token/`.
 */
export type DesktopTokenRequest = { code: string, code_verifier: string, };

export type DesktopTokenResponse = { access_token: string, token_type: string, expires_in: number, };
