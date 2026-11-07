//! JSON shapes shared by the server, the desktop client, and the web frontend,
//! which gets them as `frontend/src/api.gen.ts` via `src/bin/gen_ts.rs`.

use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct Week {
    pub id: i64,
    pub season: i64,
    pub week: i64,
    pub closes_at: Option<Timestamp>,
    pub is_closed: bool,
    /// The week clients open on: the oldest still taking votes, or the newest
    /// if every week is closed. Exactly one week in a list has it set.
    #[serde(default)]
    pub is_default: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct Me {
    pub id: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct Team {
    pub id: i64,
    pub name: String,
    pub abbreviation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct Quarterback {
    pub id: i64,
    pub name: String,
    pub team: Option<Team>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct QuarterbackStats {
    pub season: i64,
    pub games: i64,
    pub dropbacks: i64,
    pub epa: Option<f64>,
    pub epa_per_dropback: Option<f64>,
    pub cpoe: Option<f64>,
    pub qbr: Option<f64>,
    /// Ranks only cover QBs with enough dropbacks to qualify; null otherwise or
    /// when the stat itself is missing.
    pub qualified: bool,
    pub qualified_count: i64,
    pub qbr_rank: Option<i64>,
    pub epa_rank: Option<i64>,
    pub cpoe_rank: Option<i64>,
    /// Set exactly when the matching rank is.
    pub qbr_band: Option<RankBand>,
    pub epa_band: Option<RankBand>,
    pub cpoe_band: Option<RankBand>,
}

/// Which slice of the league a stat rank falls in; clients color badges by it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
pub enum RankBand {
    Top,
    Mid,
    Low,
}

/// /quarterbacks rows carry season-to-date stats; ballot and ranking payloads do not.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct QuarterbackWithStats {
    #[serde(flatten)]
    pub quarterback: Quarterback,
    /// Season-to-date including playoffs; null when nflverse has no line for this QB.
    pub stats: Option<QuarterbackStats>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct BallotRequest {
    /// Ordered from #1 to #15; ranks are derived from list position.
    pub quarterback_ids: Vec<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct BallotEntry {
    pub rank: i64,
    pub quarterback: Quarterback,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct Ballot {
    pub season: i64,
    pub week: i64,
    pub entries: Vec<BallotEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct RankingEntry {
    pub rank: i64,
    pub points: i64,
    pub votes: i64,
    pub previous_rank: Option<i64>,
    pub quarterback: Quarterback,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct Rankings {
    pub season: i64,
    pub week: i64,
    pub ballots: i64,
    pub rankings: Vec<RankingEntry>,
}

/// Body of `POST /accounts/desktop/token/`.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct DesktopTokenRequest {
    pub code: String,
    pub code_verifier: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct DesktopTokenResponse {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: i64,
}
