use iced::Element;
use iced::widget::{column, scrollable, text};

use super::{Message, Poll, logic};

pub fn view(poll: &Poll) -> Element<'_, Message> {
    let Some(data) = &poll.week else {
        return column![].into();
    };
    if data.saved.entries.is_empty() {
        return text("You haven’t submitted a ballot for this week.").into();
    }
    let picks = data.saved.entries.iter().map(|entry| {
        text(format!(
            "{:02}  {}",
            entry.rank,
            logic::qb_label(&entry.quarterback)
        ))
        .into()
    });
    scrollable(column(picks).spacing(4)).into()
}
