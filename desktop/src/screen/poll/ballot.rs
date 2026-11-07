use api_types::Quarterback;
use iced::widget::{button, column, container, progress_bar, row, rule, scrollable, space, text};
use iced::{Alignment, Element, Length, Padding, Theme, padding};

use super::logic::{self, BALLOT_SIZE, Lens, Tone};
use super::{Message, Poll, pool};
use crate::style::{self, Chip};

/// Wide enough for "15 pts" so the column lines up.
const POINTS_WIDTH: f32 = 56.0;
const ROW_HEIGHT: f32 = 56.0;
const ROW_PADDING: Padding = Padding {
    top: 0.0,
    right: 16.0,
    bottom: 0.0,
    left: 24.0,
};

pub fn view(poll: &Poll) -> Element<'_, Message> {
    if poll.week.is_none() {
        return column![].into();
    }
    row![
        container(column![
            scrollable(ballot_panel(poll))
                .height(Length::Fill)
                .direction(style::scrollbar())
                .style(style::scroll),
            save_bar(poll),
        ])
        .width(Length::FillPortion(3))
        .height(Length::Fill)
        .style(style::panel),
        container(pool::view(poll))
            .width(Length::FillPortion(2))
            .height(Length::Fill)
            .style(style::panel),
    ]
    .spacing(24)
    .height(Length::Fill)
    .into()
}

fn ballot_panel(poll: &Poll) -> Element<'_, Message> {
    let closed = poll.closed();
    let subtitle = closed.then(|| {
        text("Voting is closed. Your saved ballot is read-only.")
            .size(13)
            .style(style::muted)
    });
    let copy = (poll.previous_week().is_some() && !closed).then(|| {
        button(text("Copy last week").font(style::BOLD).size(13))
            .padding([13, 14])
            .style(style::secondary)
            .on_press_maybe(poll.can_edit().then_some(Message::CopyPrevious))
    });
    let heading = row![
        column![text("YOUR TOP 15").font(style::DISPLAY).size(30)]
            .push(subtitle)
            .spacing(4),
        space::horizontal(),
    ]
    .push(copy)
    .align_y(Alignment::Center)
    .padding(padding::top(20).bottom(16).left(24).right(24));

    let lenses = Lens::ALL.map(|lens| {
        button(text(lens.label()).font(style::BOLD).size(13))
            .padding([9, 12])
            .style(style::segment(poll.lens == lens))
            .on_press(Message::SetLens(lens))
            .into()
    });
    let lens_switch = row![
        text("BADGE SHOWS")
            .font(style::BOLD)
            .size(11)
            .style(style::muted),
        space::horizontal(),
        container(row(lenses).spacing(2))
            .padding(3)
            .style(style::segmented),
    ]
    .align_y(Alignment::Center)
    .padding([0, 24]);

    let filled = poll
        .ballot
        .iter()
        .enumerate()
        .map(|(index, qb)| slot(poll, index, qb));
    let open = (poll.ballot.len()..BALLOT_SIZE).map(open_slot);
    let slots = filled.chain(open).fold(column![divider()], |slots, slot| {
        slots.push(slot).push(divider())
    });

    column![heading, lens_switch, space::vertical().height(12), slots]
        .push((poll.lens != Lens::Crowd).then(|| legend(poll)))
        .into()
}

fn divider<'a>() -> Element<'a, Message> {
    rule::horizontal(1).style(style::divider).into()
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
        container(text(badge.text).font(style::SEMIBOLD).size(12))
            .padding([4, 8])
            .style(style::chip(chip_kind(badge.tone)))
    });
    let name = row![text(&qb.name).font(style::BOLD).size(15)]
        .push(qb.team.as_ref().map(|team| {
            text(&team.abbreviation)
                .font(style::SEMIBOLD)
                .size(12)
                .style(style::subtle)
        }))
        .spacing(8)
        .align_y(Alignment::Center)
        .width(Length::Fill);

    let edit = poll.can_edit();
    let controls = (!poll.closed()).then(|| {
        let on = |enabled: bool, message| (edit && enabled).then_some(message);
        row![
            control("▲", style::icon, on(index > 0, Message::MoveUp(index))),
            control(
                "▼",
                style::icon,
                on(index + 1 < poll.ballot.len(), Message::MoveDown(index))
            ),
            control("✕", style::remove, on(true, Message::Remove(index))),
        ]
        .spacing(4)
    });

    slot_row(
        row![slot_number(index, false), name]
            .push(badge)
            .push(points(index))
            .push(controls),
    )
}

fn control<'a>(
    glyph: &'a str,
    style: fn(&Theme, button::Status) -> button::Style,
    message: Option<Message>,
) -> Element<'a, Message> {
    button(
        text(glyph)
            .size(13)
            .center()
            .width(Length::Fill)
            .height(Length::Fill),
    )
    .width(44)
    .height(44)
    .padding(0)
    .style(style)
    .on_press_maybe(message)
    .into()
}

fn open_slot<'a>(index: usize) -> Element<'a, Message> {
    slot_row(row![
        slot_number(index, true),
        container(text("Open slot — add a quarterback from the pool").size(13))
            .padding([0, 14])
            .height(40)
            .width(Length::Fill)
            .align_y(Alignment::Center)
            .style(style::open_slot),
        points(index),
    ])
}

fn slot_row<'a>(content: iced::widget::Row<'a, Message>) -> Element<'a, Message> {
    container(content.spacing(14).align_y(Alignment::Center))
        .padding(ROW_PADDING)
        .height(ROW_HEIGHT)
        .align_y(Alignment::Center)
        .into()
}

fn slot_number<'a>(index: usize, empty: bool) -> Element<'a, Message> {
    let number_style: fn(&Theme) -> text::Style = if empty { style::faint } else { text::base };
    text(format!("{:02}", index + 1))
        .font(style::NUMBER)
        .size(26)
        .line_height(1.0)
        .width(38)
        .style(number_style)
        .into()
}

fn points<'a>(index: usize) -> Element<'a, Message> {
    text(format!("{} pts", BALLOT_SIZE - index))
        .font(style::SEMIBOLD)
        .size(12)
        .style(style::subtle)
        .width(POINTS_WIDTH)
        .align_x(Alignment::End)
        .into()
}

fn chip_kind(tone: Tone) -> Chip {
    match tone {
        Tone::Plain => Chip::Plain,
        Tone::Unranked => Chip::Unranked,
        Tone::Top => Chip::Top,
        Tone::Mid => Chip::Mid,
        Tone::Low => Chip::Low,
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
        legend_key(Chip::Top, "League top 8".to_owned()),
        legend_key(Chip::Mid, "9–20".to_owned()),
        legend_key(Chip::Low, bottom),
    ]
    .spacing(18)
    .padding([14, 24])
    .into()
}

fn legend_key<'a>(kind: Chip, label: String) -> Element<'a, Message> {
    row![
        container(space::horizontal())
            .width(12)
            .height(12)
            .style(style::swatch(kind)),
        text(label).size(12).style(style::muted),
    ]
    .spacing(6)
    .align_y(Alignment::Center)
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
        button(text("+ Add QB").font(style::BOLD).size(13))
            .padding([13, 14])
            .style(style::add)
            .on_press(Message::FocusSearch)
    });
    let save = button(text(label).font(style::BOLD).size(15))
        .padding([14, 18])
        .width(220)
        .style(style::primary)
        .on_press_maybe(can_save.then_some(Message::Save));

    let hint = text(logic::save_hint(count, dirty, poll.has_saved()))
        .font(style::SEMIBOLD)
        .size(14)
        .style(style::ink_2)
        .width(Length::Fill);
    let actions = row![hint]
        .push(add)
        .push(save)
        .spacing(8)
        .align_y(Alignment::Center);

    container(
        column![
            progress_bar(0.0..=BALLOT_SIZE as f32, count as f32)
                .girth(6)
                .style(style::progress),
            actions,
        ]
        .spacing(14),
    )
    .padding(padding::top(20).bottom(24).left(24).right(24))
    .width(Length::Fill)
    .style(style::save_bar)
    .into()
}
