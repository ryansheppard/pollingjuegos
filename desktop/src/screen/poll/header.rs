use std::fmt;

use iced::widget::{button, column, pick_list, row, space, text};
use iced::{Alignment, Element};

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

pub fn view(poll: &Poll) -> Element<'_, Message> {
    column![bar(poll), banner(poll), tabs(poll)]
        .spacing(16)
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

    let stepper = row![
        button("◀").on_press_maybe(previous),
        pick_list(choices, selected, |choice: WeekChoice| {
            Message::SelectWeek(choice.index)
        })
        .placeholder("No weeks"),
        button("▶").on_press_maybe(next),
    ]
    .spacing(4)
    .align_y(Alignment::Center);

    row![
        text("PollingJuegos").size(22),
        space::horizontal(),
        stepper,
        space::horizontal(),
        button("Sign out")
            .style(button::secondary)
            .on_press(Message::SignOut),
    ]
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

    let title = column![
        text("The smoljuegos QB poll").size(13),
        text("Who’s the GOAT?").size(34),
    ];
    let stats = row![
        stat(
            if closed {
                "Voting closed"
            } else {
                "Voting closes"
            },
            closes
        ),
        stat("Time left", left),
        stat("Ballots in", ballots),
    ]
    .spacing(32);

    row![title, space::horizontal(), stats]
        .align_y(Alignment::End)
        .into()
}

fn stat<'a>(label: &'a str, value: String) -> Element<'a, Message> {
    column![text(label).size(13), text(value).size(20)]
        .spacing(2)
        .into()
}

fn tabs(poll: &Poll) -> Element<'_, Message> {
    let picks = poll.ballot.len();
    let enabled = !poll.weeks.is_empty();
    let tab = |label: String, tab: Tab| {
        button(text(label))
            .style(if poll.tab == tab {
                button::primary
            } else {
                button::text
            })
            .on_press_maybe(enabled.then_some(Message::SelectTab(tab)))
    };
    row![
        tab(format!("Your ballot {picks}/15"), Tab::Ballot),
        tab("Results · Top 10".to_owned(), Tab::Results),
    ]
    .spacing(4)
    .into()
}
