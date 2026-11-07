use api_types::{RankingEntry, Rankings};
use iced::widget::{button, column, container, progress_bar, row, scrollable, space, table, text};
use iced::{Alignment, Element, Length};

use super::{Message, Poll, logic};

const SIDEBAR_WIDTH: f32 = 300.0;

pub fn view(poll: &Poll) -> Element<'_, Message> {
    let Some(data) = &poll.week else {
        return column![].into();
    };
    row![
        container(top_ten(poll, &data.rankings)).width(Length::Fill),
        sidebar(poll),
    ]
    .spacing(24)
    .into()
}

fn top_ten<'a>(poll: &'a Poll, rankings: &'a Rankings) -> Element<'a, Message> {
    let ballots = match rankings.ballots {
        1 => "1 ballot".to_owned(),
        n => format!("{n} ballots"),
    };
    let status = if poll.closed() {
        "final"
    } else {
        "live until voting closes"
    };
    let copy = (!rankings.rankings.is_empty()).then(|| {
        button("Copy results")
            .style(button::secondary)
            .on_press_maybe((!poll.loading_week).then_some(Message::CopyResults))
    });
    let heading = row![
        column![
            text("The top 10").size(24),
            text(format!("{ballots} · {status}")).size(13),
        ],
        space::horizontal(),
    ]
    .push(copy)
    .align_y(Alignment::Center);

    let body: Element<'_, Message> = if rankings.rankings.is_empty() {
        text("No votes yet. Be the first to make your picks.").into()
    } else {
        scrollable(rankings_table(poll, rankings)).into()
    };
    column![heading, body].spacing(16).into()
}

fn rankings_table<'a>(poll: &'a Poll, rankings: &'a Rankings) -> Element<'a, Message> {
    // Bars are scaled to the leader so the gap between places is visible.
    let top = rankings.rankings.first().map_or(1, |row| row.points.max(1)) as f32;
    // Weeks are newest first; with no earlier week there's nothing to compare to.
    let has_previous = poll
        .selected
        .is_some_and(|index| index + 1 < poll.weeks.len());

    let mut columns = vec![table::column(text("Rk"), |entry: &RankingEntry| {
        text(entry.rank)
    })];
    if has_previous {
        columns.push(table::column(text("Chg"), |entry: &RankingEntry| {
            movement(entry)
        }));
    }
    columns.extend([
        table::column(text("Quarterback"), |entry: &RankingEntry| {
            let team = entry.quarterback.team.as_ref();
            row![text(&entry.quarterback.name)]
                .push(team.map(|team| text(&team.abbreviation).size(12)))
                .spacing(6)
                .align_y(Alignment::Center)
        })
        .width(Length::Fill),
        table::column(text("Points"), move |entry: &RankingEntry| {
            row![
                progress_bar(0.0..=top, entry.points as f32)
                    .length(120)
                    .girth(8),
                text(entry.points),
            ]
            .spacing(8)
            .align_y(Alignment::Center)
        }),
        table::column(text("Ballots"), |entry: &RankingEntry| {
            text(format!("{}/{}", entry.votes, rankings.ballots))
        }),
    ]);
    table(columns, &rankings.rankings).into()
}

fn movement(entry: &RankingEntry) -> Element<'_, Message> {
    match entry.previous_rank {
        None => text("NEW").style(text::primary).into(),
        Some(previous) if previous > entry.rank => text(format!("▲ {}", previous - entry.rank))
            .style(text::success)
            .into(),
        Some(previous) if previous < entry.rank => text(format!("▼ {}", entry.rank - previous))
            .style(text::danger)
            .into(),
        Some(_) => text("—").into(),
    }
}

fn sidebar(poll: &Poll) -> Element<'_, Message> {
    let versus = poll.week.as_ref().and_then(|data| {
        let rows = logic::disagreements(&data.saved, &data.rankings);
        (!rows.is_empty() && !data.rankings.rankings.is_empty()).then(|| {
            let rows = rows.into_iter().map(|row| {
                let crowd = row
                    .crowd
                    .map_or_else(|| "—".to_owned(), |rank| format!("#{rank}"));
                column![
                    text(&row.quarterback.name),
                    text(format!("You #{} · Crowd {crowd}", row.mine)).size(13),
                ]
                .into()
            });
            card(
                "You vs. the crowd",
                column![text("Your saved picks compared with the live top 10.").size(13)]
                    .extend(rows)
                    .spacing(8),
            )
        })
    });
    let scoring = card(
        "How scoring works",
        text(
            "Each ballot ranks 15 quarterbacks. A No. 1 pick earns 15 points, a No. 2 \
             earns 14, and so on down to 1 point for No. 15. The table shows the top 10 \
             by total points; tied scores are ordered by ballot appearances, then name.",
        )
        .size(14),
    );
    column![]
        .push(versus)
        .push(scoring)
        .spacing(16)
        .width(SIDEBAR_WIDTH)
        .into()
}

fn card<'a>(title: &'a str, body: impl Into<Element<'a, Message>>) -> Element<'a, Message> {
    container(column![text(title).size(18), body.into()].spacing(8))
        .padding(16)
        .width(Length::Fill)
        .style(container::bordered_box)
        .into()
}
