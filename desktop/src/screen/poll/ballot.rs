use api_types::Quarterback;
use iced::widget::{button, column, container, progress_bar, row, scrollable, space, text};
use iced::{Alignment, Element, Length, Theme};

use super::logic::{self, BALLOT_SIZE, Lens, Tone};
use super::{Message, Poll, pool};

/// Wide enough for "15 pts" so the column lines up.
const POINTS_WIDTH: f32 = 56.0;

pub fn view(poll: &Poll) -> Element<'_, Message> {
    if poll.week.is_none() {
        return column![].into();
    }
    row![
        container(scrollable(ballot_panel(poll)).height(Length::Fill))
            .width(Length::FillPortion(3)),
        container(pool::view(poll)).width(Length::FillPortion(2)),
    ]
    .spacing(24)
    .height(Length::Fill)
    .into()
}

fn ballot_panel(poll: &Poll) -> Element<'_, Message> {
    let closed = poll.closed();
    let subtitle =
        closed.then(|| text("Voting is closed. Your saved ballot is read-only.").size(13));
    let copy = (poll.previous_week().is_some() && !closed).then(|| {
        button("Copy last week")
            .style(button::secondary)
            .on_press_maybe(poll.can_edit().then_some(Message::CopyPrevious))
    });
    let heading = row![
        column![text("Your top 15").size(24)].push(subtitle),
        space::horizontal(),
    ]
    .push(copy)
    .align_y(Alignment::Center);

    let lenses = Lens::ALL.map(|lens| {
        button(text(lens.label()).size(13))
            .style(if poll.lens == lens {
                button::primary
            } else {
                button::text
            })
            .on_press(Message::SetLens(lens))
            .into()
    });
    let lens_switch = row![text("Badge shows").size(13)]
        .extend(lenses)
        .spacing(4)
        .align_y(Alignment::Center);

    let filled = poll
        .ballot
        .iter()
        .enumerate()
        .map(|(index, qb)| slot(poll, index, qb));
    let open = (poll.ballot.len()..BALLOT_SIZE).map(open_slot);
    let slots = column(filled.chain(open)).spacing(6);

    column![heading, lens_switch, slots]
        .push((poll.lens != Lens::Crowd).then(|| legend(poll)))
        .push(save_bar(poll))
        .spacing(16)
        .padding([0, 16])
        .into()
}

fn slot<'a>(poll: &'a Poll, index: usize, qb: &'a Quarterback) -> Element<'a, Message> {
    // Before anyone has voted, every crowd badge would just say "Unranked".
    let show_badge = poll.lens != Lens::Crowd
        || poll
            .week
            .as_ref()
            .is_some_and(|data| data.rankings.ballots > 0);
    let badge = show_badge.then(|| {
        let badge = logic::badge(poll.lens, poll.crowd_rank(qb.id), poll.stats_for(qb.id));
        text(badge.text).size(13).style(tone_style(badge.tone))
    });
    let name = row![text(&qb.name)]
        .push(
            qb.team
                .as_ref()
                .map(|team| text(&team.abbreviation).size(12)),
        )
        .spacing(6)
        .align_y(Alignment::Center)
        .width(Length::Fill);

    let edit = poll.can_edit();
    let controls = (!poll.closed()).then(|| {
        let on = |enabled: bool, message| (edit && enabled).then_some(message);
        row![
            button("▲").on_press_maybe(on(index > 0, Message::MoveUp(index))),
            button("▼").on_press_maybe(on(index + 1 < poll.ballot.len(), Message::MoveDown(index))),
            button("✕")
                .style(button::danger)
                .on_press_maybe(on(true, Message::Remove(index))),
        ]
        .spacing(4)
    });

    row![slot_number(index), name]
        .push(badge)
        .push(points(index))
        .push(controls)
        .spacing(12)
        .align_y(Alignment::Center)
        .into()
}

fn open_slot<'a>(index: usize) -> Element<'a, Message> {
    row![
        slot_number(index),
        text("Open slot — add a quarterback from the pool")
            .style(text::secondary)
            .width(Length::Fill),
        points(index),
    ]
    .spacing(12)
    .align_y(Alignment::Center)
    .into()
}

fn slot_number<'a>(index: usize) -> Element<'a, Message> {
    text(format!("{:02}", index + 1)).size(18).width(32).into()
}

fn points<'a>(index: usize) -> Element<'a, Message> {
    text(format!("{} pts", BALLOT_SIZE - index))
        .size(13)
        .width(POINTS_WIDTH)
        .into()
}

fn tone_style(tone: Tone) -> fn(&Theme) -> text::Style {
    match tone {
        Tone::Plain => text::base,
        Tone::Unranked => text::secondary,
        Tone::Top => text::success,
        Tone::Mid => text::warning,
        Tone::Low => text::danger,
    }
}

fn legend(poll: &Poll) -> Element<'_, Message> {
    // Every QB with stats carries the same league-wide count.
    let qualified = poll
        .catalog
        .iter()
        .find_map(|qb| qb.stats.as_ref())
        .map_or(0, |stats| stats.qualified_count);
    let bottom = if qualified > 0 {
        format!("21+ of {qualified}")
    } else {
        "21+".to_owned()
    };
    row![
        text("League top 8").size(13).style(text::success),
        text("9–20").size(13).style(text::warning),
        text(bottom).size(13).style(text::danger),
    ]
    .spacing(16)
    .into()
}

fn save_bar(poll: &Poll) -> Element<'_, Message> {
    let count = poll.ballot.len();
    let closed = poll.closed();
    let dirty = poll.dirty();
    let label = if poll.saving {
        "Saving…"
    } else if closed {
        "Voting closed"
    } else if count == BALLOT_SIZE && !dirty {
        "Saved"
    } else {
        "Save ballot"
    };
    let can_save = count == BALLOT_SIZE && dirty && poll.can_edit();
    let add = (!closed && count < BALLOT_SIZE).then(|| {
        button("+ Add QB")
            .style(button::secondary)
            .on_press(Message::FocusSearch)
    });

    let actions = row![text(logic::save_hint(count, dirty, poll.has_saved())).width(Length::Fill),]
        .push(add)
        .push(button(label).on_press_maybe(can_save.then_some(Message::Save)))
        .spacing(8)
        .align_y(Alignment::Center);

    column![
        progress_bar(0.0..=BALLOT_SIZE as f32, count as f32).girth(6),
        actions,
    ]
    .spacing(8)
    .into()
}
