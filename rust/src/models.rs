use jiff::Timestamp;
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
pub struct QuarterbackOut {
    pub id: i64,
    pub name: String,
    pub team_id: Option<i64>,
}

#[derive(Serialize)]
pub struct WeekOut {
    pub id: i64,
    pub season: i64,
    pub week: i64,
    pub closes_at: Option<Timestamp>,
    pub is_closed: bool,
}

#[derive(Serialize)]
pub struct Me {
    pub id: i64,
}

#[derive(Serialize)]
pub struct TeamOut {
    pub id: i64,
    pub name: String,
    pub abbreviation: String,
}

#[derive(Serialize)]
pub struct QuarterbackWithTeam {
    pub id: i64,
    pub name: String,
    pub team: Option<TeamOut>,
}

#[derive(Deserialize)]
pub struct BallotIn {
    /// Ordered from #1 to #15; ranks are derived from list position.
    pub quarterback_ids: Vec<i64>,
}

#[derive(Serialize)]
pub struct BallotEntry {
    pub rank: i64,
    pub quarterback: QuarterbackWithTeam,
}

#[derive(Serialize)]
pub struct BallotOut {
    pub season: i64,
    pub week: i64,
    pub entries: Vec<BallotEntry>,
}

#[derive(Serialize)]
pub struct RankingEntry {
    pub rank: i64,
    pub points: i64,
    pub votes: i64,
    pub previous_rank: Option<i64>,
    pub quarterback: QuarterbackWithTeam,
}

#[derive(Serialize)]
pub struct RankingsOut {
    pub season: i64,
    pub week: i64,
    pub ballots: i64,
    pub rankings: Vec<RankingEntry>,
}
