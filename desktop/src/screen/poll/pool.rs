use api_types::{BALLOT_SIZE, QuarterbackWithStats};
use iced::widget::{button, column, container, row, rule, scrollable, space, text, text_input};
use iced::{Alignment, Element, Length, padding};

use super::logic;
use super::{Message, Poll, SEARCH_INPUT};
use crate::style;

pub fn view(poll: &Poll) -> Element<'_, Message> {
    let available = poll.available();
    let title = row![
        text("THE POOL").font(style::DISPLAY).size(30),
        space::horizontal(),
        text(format!("{} available", available.len()))
            .font(style::SEMIBOLD)
            .size(12)
            .style(style::subtle),
    ]
    .align_y(Alignment::Center);
    let search = text_input("Allen, BUF, Bills…", &poll.search)
        .id(SEARCH_INPUT)
        .on_input(Message::Search)
        .on_submit(Message::AddTopMatch)
        .padding([13, 14])
        .size(15)
        .style(style::search);
    let head = column![
        title,
        text("SEARCH BY NAME OR TEAM")
            .font(style::BOLD)
            .size(11)
            .style(style::muted),
        search,
    ]
    .spacing(12)
    .padding(padding::top(20).bottom(16).left(20).right(20));

    let can_add = poll.can_edit() && poll.ballot.len() < BALLOT_SIZE;
    let list: Element<'_, Message> = if available.is_empty() {
        container(
            text("No match. Don't see your quarterback? Too bad.")
                .size(14)
                .style(style::muted),
        )
        .padding(24)
        .into()
    } else {
        let rows = available.into_iter().fold(column![], |rows, qb| {
            rows.push(entry(qb, can_add)).push(divider())
        });
        scrollable(rows)
            .height(Length::Fill)
            .direction(style::scrollbar())
            .style(style::scroll)
            .into()
    };

    column![head, divider(), list].into()
}

fn divider<'a>() -> Element<'a, Message> {
    rule::horizontal(1).style(style::divider).into()
}

fn entry(qb: &QuarterbackWithStats, can_add: bool) -> Element<'_, Message> {
    let team = qb
        .quarterback
        .team
        .as_ref()
        .map_or("FA", |team| team.abbreviation.as_str());
    let badge = container(text(team).font(style::BOLD).size(11))
        .center_x(40)
        .center_y(28)
        .style(style::team_badge);
    let stats = qb.stats.as_ref().map(|stats| {
        let line = logic::stat_line(stats)
            .map(|stat| format!("{} {}", stat.lens.label(), stat.value))
            .join("   ");
        text(line)
            .font(style::SEMIBOLD)
            .size(12)
            .style(style::ink_2)
    });
    row![
        badge,
        column![text(&qb.quarterback.name).font(style::SEMIBOLD).size(15)]
            .push(stats)
            .spacing(3)
            .width(Length::Fill),
        button(text("+ Add").font(style::BOLD).size(13))
            .padding([10, 14])
            .style(style::add)
            .on_press_maybe(can_add.then_some(Message::Add(qb.quarterback.id))),
    ]
    .spacing(12)
    .align_y(Alignment::Center)
    .padding(padding::top(8).bottom(8).left(20).right(16))
    .into()
}
