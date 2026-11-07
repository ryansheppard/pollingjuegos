use api_types::QuarterbackWithStats;
use iced::widget::{button, column, row, scrollable, space, text, text_input};
use iced::{Alignment, Element, Length};

use super::logic::{self, BALLOT_SIZE};
use super::{Message, Poll, SEARCH_INPUT};

pub fn view(poll: &Poll) -> Element<'_, Message> {
    let available = poll.available();
    let heading = row![
        text("The pool").size(24),
        space::horizontal(),
        text(format!("{} available", available.len())).size(13),
    ]
    .align_y(Alignment::Center);
    let search = text_input("Allen, BUF, Bills…", &poll.search)
        .id(SEARCH_INPUT)
        .on_input(Message::Search)
        .on_submit(Message::AddTopMatch);

    let can_add = poll.can_edit() && poll.ballot.len() < BALLOT_SIZE;
    let list: Element<'_, Message> = if available.is_empty() {
        text("No match. Don't see your quarterback? Too bad.").into()
    } else {
        let rows = available.into_iter().map(|qb| entry(qb, can_add));
        scrollable(column(rows).spacing(8).padding([0, 16]))
            .height(Length::Fill)
            .into()
    };

    column![
        heading,
        text("Search by name or team").size(13),
        search,
        list
    ]
    .spacing(8)
    .into()
}

fn entry(qb: &QuarterbackWithStats, can_add: bool) -> Element<'_, Message> {
    let team = qb
        .quarterback
        .team
        .as_ref()
        .map_or("FA", |team| team.abbreviation.as_str());
    let stats = qb.stats.as_ref().map(|stats| {
        let line = logic::stat_line(stats)
            .map(|stat| format!("{} {}", stat.lens.label(), stat.value))
            .join("   ");
        text(line).size(12).style(text::secondary)
    });
    row![
        text(team).size(13).width(40),
        column![text(&qb.quarterback.name)]
            .push(stats)
            .width(Length::Fill),
        button("+ Add")
            .style(button::secondary)
            .on_press_maybe(can_add.then_some(Message::Add(qb.quarterback.id))),
    ]
    .spacing(8)
    .align_y(Alignment::Center)
    .into()
}
