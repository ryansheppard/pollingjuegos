use jiff::Timestamp;
use serde::Serialize;

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
