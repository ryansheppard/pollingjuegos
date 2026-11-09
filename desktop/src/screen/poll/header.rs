use std::fmt;

use api_types::BALLOT_SIZE;
use iced::widget::{button, column, container, pick_list, row, rule, space, text};
use iced::{Alignment, Element, Length, Theme, padding};

use crate::style;

use super::{Message, Poll, Tab, logic};

/// `pick_list` needs `Display` options; the index maps the choice back to
/// `Poll::weeks`.
#[derive(Debug, Clone, PartialEq)]
struct WeekChoice {
    index: usize,
    label: String,
}

impl fmt::Display for WeekChoice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.label)
    }
}

const GUTTER: f32 = 24.0;

pub fn view(poll: &Poll) -> Element<'_, Message> {
    column![
        container(bar(poll))
            .padding([10.0, GUTTER])
            .width(Length::Fill)
            .style(style::site_header),
        rule::horizontal(1).style(style::band_divider),
        container(column![banner(poll), tabs(poll)].spacing(24))
            .padding(padding::top(28).left(GUTTER).right(GUTTER))
            .width(Length::Fill)
            .style(style::band),
    ]
    .into()
}

fn bar(poll: &Poll) -> Element<'_, Message> {
    let choices: Vec<WeekChoice> = poll
        .weeks
        .iter()
        .enumerate()
        .map(|(index, week)| WeekChoice {
            index,
            label: logic::week_label(week),
        })
        .collect();
    let selected = poll.selected.map(|index| choices[index].clone());
    let busy = poll.busy();
    // Weeks are newest first, so "previous" is the next index.
    let step = |target: Option<usize>| {
        target
            .filter(|&index| !busy && index < poll.weeks.len())
            .map(Message::SelectWeek)
    };
    let previous = step(poll.selected.map(|index| index + 1));
    let next = step(poll.selected.and_then(|index| index.checked_sub(1)));
    let step_button = |glyph, message| {
        button(
            text(glyph)
                .size(12)
                .center()
                .width(Length::Fill)
                .height(Length::Fill),
        )
        .width(44)
        .height(40)
        .style(style::header_step)
        .on_press_maybe(message)
    };

    let stepper = container(
        row![
            step_button("◀", previous),
            pick_list(choices, selected, |choice: WeekChoice| {
                Message::SelectWeek(choice.index)
            })
            .placeholder("No weeks")
            .font(style::BOLD)
            .text_size(14)
            .padding([10, 10])
            .style(style::header_pick_list)
            .menu_style(style::menu),
            step_button("▶", next),
        ]
        .spacing(2)
        .align_y(Alignment::Center),
    )
    .padding(2)
    .style(style::week_stepper);

    row![
        brand(),
        space::horizontal(),
        stepper,
        space::horizontal(),
        button(text("Sign out").font(style::BOLD).size(13))
            .padding([12, 8])
            .style(style::header_link)
            .on_press(Message::SignOut),
    ]
    .align_y(Alignment::Center)
    .into()
}

fn brand<'a>() -> Element<'a, Message> {
    let mark = container(text("PJ").font(style::DISPLAY).size(19))
        .center(34)
        .style(style::brand_mark);
    row![
        mark,
        row![
            text("POLLING").font(style::DISPLAY).size(26),
            text("JUEGOS")
                .font(style::DISPLAY)
                .size(26)
                .style(style::accent_bright),
        ],
    ]
    .spacing(10)
    .align_y(Alignment::Center)
    .into()
}

fn banner(poll: &Poll) -> Element<'_, Message> {
    let week = poll.current_week();
    let closed = poll.closed();
    let closes = week.map_or_else(|| "—".to_owned(), logic::closes_label);
    let left = week.map_or_else(|| "—".to_owned(), |w| logic::time_left(w, poll.now));
    let ballots = match &poll.week {
        Some(data) if !poll.loading_week => data.rankings.ballots.to_string(),
        _ => "—".to_owned(),
    };

    let headline = |value| text(value).font(style::DISPLAY).size(54).line_height(1.0);
    let title = column![
        text("THE SMOLJUEGOS QB POLL")
            .font(style::BOLD)
            .size(11.5)
            .style(style::accent_bright),
        row![
            headline("WHO’S THE GOAT"),
            headline("?").style(style::accent_bright)
        ],
    ]
    .spacing(8);

    let divider = || rule::vertical(1).style(style::band_divider);
    let stats = container(
        row![
            stat(
                if closed {
                    "VOTING CLOSED"
                } else {
                    "VOTING CLOSES"
                },
                closes,
                false,
            ),
            divider(),
            stat("TIME LEFT", left, true),
            divider(),
            stat("BALLOTS IN", ballots, false),
        ]
        // The dividers fill vertically, which would otherwise stretch the band to the window.
        .height(Length::Shrink),
    )
    .style(style::stats);

    row![title, space::horizontal(), stats]
        .align_y(Alignment::End)
        .into()
}

fn stat<'a>(label: &'a str, value: String, accent: bool) -> Element<'a, Message> {
    let value_style: fn(&Theme) -> text::Style = if accent {
        style::accent_bright
    } else {
        style::on_dark
    };
    column![
        text(label)
            .font(style::BOLD)
            .size(11)
            .style(style::on_dark_muted),
        text(value)
            .font(style::NUMBER)
            .size(26)
            .line_height(1.0)
            .style(value_style),
    ]
    .spacing(6)
    .padding([12, 20])
    .into()
}

fn tabs(poll: &Poll) -> Element<'_, Message> {
    let picks = poll.ballot.len();
    let enabled = !poll.weeks.is_empty();
    let tab = |label: &'static str, chip: Option<String>, tab: Tab| {
        let selected = poll.tab == tab;
        let chip = chip.map(|chip| {
            container(text(chip).font(style::BOLD).size(12))
                .padding([3, 8])
                .style(style::tab_chip(selected))
        });
        button(
            row![text(label).font(style::BOLD).size(15)]
                .push(chip)
                .spacing(10)
                .align_y(Alignment::Center),
        )
        .padding([14, 18])
        .style(style::tab(selected))
        .on_press_maybe(enabled.then_some(Message::SelectTab(tab)))
    };
    row![
        tab(
            "Your ballot",
            Some(format!("{picks}/{BALLOT_SIZE}")),
            Tab::Ballot
        ),
        tab("Results · Top 10", None, Tab::Results),
    ]
    .spacing(4)
    .into()
}
