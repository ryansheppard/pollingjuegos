use iced::Element;
use iced::widget::{column, scrollable, text};

use super::{Message, Poll, logic};

pub fn view(poll: &Poll) -> Element<'_, Message> {
    let Some(data) = &poll.week else {
        return column![].into();
    };
    if data.rankings.rankings.is_empty() {
        return text("No votes yet. Be the first to make your picks.").into();
    }
    let rows = data.rankings.rankings.iter().map(|entry| {
        text(format!(
            "{:>2}. {} · {} pts · {}/{} ballots",
            entry.rank,
            logic::qb_label(&entry.quarterback),
            entry.points,
            entry.votes,
            data.rankings.ballots,
        ))
        .into()
    });
    scrollable(column(rows).spacing(4)).into()
}
