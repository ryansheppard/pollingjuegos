//! Prints TypeScript declarations for every API type to stdout. Bazel checks
//! the output against `frontend/src/api.gen.ts`; refresh it with
//! `bazel run //frontend:update_api_types`.

use api_types::{
    Ballot, BallotEntry, BallotRequest, DesktopTokenRequest, DesktopTokenResponse, Me, Quarterback,
    QuarterbackStats, QuarterbackWithStats, RankBand, RankingEntry, Rankings, Team, Week,
};
use ts_rs::{Config, TS};

fn export<T: TS>(cfg: &Config) -> String {
    // `decl` covers field docs but not the type's own doc comment.
    format!("{}export {}", T::docs().unwrap_or_default(), T::decl(cfg))
}

fn main() {
    // serde_json writes i64 as a plain JSON number and JSON.parse yields a
    // `number`, so ts-rs's default `bigint` would be wrong. Every id and count
    // here is far below 2^53.
    let cfg = Config::new().with_large_int("number");
    let decls = [
        export::<Week>(&cfg),
        export::<Me>(&cfg),
        export::<Team>(&cfg),
        export::<Quarterback>(&cfg),
        export::<QuarterbackStats>(&cfg),
        export::<RankBand>(&cfg),
        export::<QuarterbackWithStats>(&cfg),
        export::<BallotRequest>(&cfg),
        export::<BallotEntry>(&cfg),
        export::<Ballot>(&cfg),
        export::<RankingEntry>(&cfg),
        export::<Rankings>(&cfg),
        export::<DesktopTokenRequest>(&cfg),
        export::<DesktopTokenResponse>(&cfg),
    ];
    println!(
        "// Generated from api-types by `bazel run //frontend:update_api_types`. Do not edit."
    );
    for decl in decls {
        println!("\n{decl}");
    }
}
