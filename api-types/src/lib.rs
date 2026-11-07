//! JSON shapes shared by the server and the desktop client. The web frontend
//! mirrors these by hand in `frontend/src/api.ts`.

use jiff::Timestamp;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Week {
    pub id: i64,
    pub season: i64,
    pub week: i64,
    pub closes_at: Option<Timestamp>,
    pub is_closed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Me {
    pub id: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Team {
    pub id: i64,
    pub name: String,
    pub abbreviation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Quarterback {
    pub id: i64,
    pub name: String,
    pub team: Option<Team>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QuarterbackStats {
    pub season: i64,
    pub games: i64,
    pub dropbacks: i64,
    pub epa: Option<f64>,
    pub epa_per_dropback: Option<f64>,
    pub cpoe: Option<f64>,
    pub qbr: Option<f64>,
    // Ranks only cover QBs with enough dropbacks to qualify; null otherwise or
    // when the stat itself is missing.
    pub qualified: bool,
    pub qualified_count: i64,
    pub qbr_rank: Option<i64>,
    pub epa_rank: Option<i64>,
    pub cpoe_rank: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuarterbackWithStats {
    #[serde(flatten)]
    pub quarterback: Quarterback,
    /// Season-to-date including playoffs; null when nflverse has no line for this QB.
    pub stats: Option<QuarterbackStats>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BallotRequest {
    /// Ordered from #1 to #15; ranks are derived from list position.
    pub quarterback_ids: Vec<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BallotEntry {
    pub rank: i64,
    pub quarterback: Quarterback,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ballot {
    pub season: i64,
    pub week: i64,
    pub entries: Vec<BallotEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RankingEntry {
    pub rank: i64,
    pub points: i64,
    pub votes: i64,
    pub previous_rank: Option<i64>,
    pub quarterback: Quarterback,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rankings {
    pub season: i64,
    pub week: i64,
    pub ballots: i64,
    pub rankings: Vec<RankingEntry>,
}

/// Body of `POST /accounts/desktop/token/`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DesktopTokenRequest {
    pub code: String,
    pub code_verifier: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DesktopTokenResponse {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: i64,
}
