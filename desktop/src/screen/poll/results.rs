use api_types::{RankingEntry, Rankings};
use iced::widget::{button, column, container, progress_bar, row, scrollable, space, table, text};
use iced::{Alignment, Element, Length, Theme, padding};

use super::{Message, Poll};
use crate::style;

const SIDEBAR_WIDTH: f32 = 320.0;

pub fn view(poll: &Poll) -> Element<'_, Message> {
    let Some(data) = &poll.week else {
        return column![].into();
    };
    row![
        container(top_ten(poll, &data.rankings))
            .width(Length::Fill)
            .style(style::panel),
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
        button(text("Copy results").font(style::BOLD).size(13))
            .padding([13, 14])
            .style(style::secondary)
            .on_press_maybe((!poll.loading_week).then_some(Message::CopyResults))
    });
    let heading = row![
        column![
            text("THE TOP 10").font(style::DISPLAY).size(30),
            text(format!("{ballots} · {status}"))
                .size(13)
                .style(style::muted),
        ]
        .spacing(4),
        space::horizontal(),
    ]
    .push(copy)
    .align_y(Alignment::Center)
    .padding(padding::top(20).bottom(16).left(24).right(24));

    let body: Element<'_, Message> = if rankings.rankings.is_empty() {
        container(
            text("No votes yet. Be the first to make your picks.")
                .size(14)
                .style(style::muted),
        )
        .padding(padding::bottom(24).left(24).right(24))
        .into()
    } else {
        scrollable(
            container(rankings_table(poll, rankings))
                .padding(padding::bottom(12).left(12).right(12)),
        )
        .direction(style::scrollbar())
        .style(style::scroll)
        .into()
    };
    column![heading, body].into()
}

fn header<'a>(label: &'a str) -> Element<'a, Message> {
    text(label)
        .font(style::BOLD)
        .size(11)
        .style(style::muted)
        .into()
}

fn rankings_table<'a>(poll: &'a Poll, rankings: &'a Rankings) -> Element<'a, Message> {
    // Bars are scaled to the leader so the gap between places is visible.
    let top = rankings.rankings.first().map_or(1, |row| row.points.max(1)) as f32;
    // Weeks are newest first; with no earlier week there's nothing to compare to.
    let has_previous = poll
        .selected
        .is_some_and(|index| index + 1 < poll.weeks.len());

    let mut columns = vec![table::column(header("RK"), |entry: &RankingEntry| {
        let rank_style: fn(&Theme) -> text::Style = if entry.rank == 1 {
            style::accent_text
        } else {
            text::base
        };
        text(entry.rank)
            .font(style::NUMBER)
            .size(30)
            .line_height(1.0)
            .style(rank_style)
    })];
    if has_previous {
        columns.push(table::column(header("CHG"), |entry: &RankingEntry| {
            movement(entry)
        }));
    }
    columns.extend([
        table::column(header("QUARTERBACK"), |entry: &RankingEntry| {
            let team = entry.quarterback.team.as_ref();
            row![text(&entry.quarterback.name).font(style::BOLD).size(15)]
                .push(team.map(|team| {
                    text(&team.abbreviation)
                        .font(style::SEMIBOLD)
                        .size(12)
                        .style(style::subtle)
                }))
                .spacing(8)
                .align_y(Alignment::Center)
        })
        .width(Length::Fill),
        table::column(header("POINTS"), move |entry: &RankingEntry| {
            row![
                progress_bar(0.0..=top, entry.points as f32)
                    .length(160)
                    .girth(10)
                    .style(style::score(entry.rank == 1)),
                text(entry.points).font(style::BOLD).size(15),
            ]
            .spacing(12)
            .align_y(Alignment::Center)
        }),
        table::column(header("BALLOTS"), |entry: &RankingEntry| {
            text(format!("{}/{}", entry.votes, rankings.ballots))
                .size(14)
                .style(style::muted)
        }),
    ]);
    table(columns, &rankings.rankings)
        .padding_x(12)
        .padding_y(14)
        .separator_x(0)
        .into()
}

fn movement(entry: &RankingEntry) -> Element<'_, Message> {
    let (label, tone): (String, fn(&Theme) -> text::Style) = match entry.previous_rank {
        None => ("NEW".to_owned(), style::new),
        Some(previous) if previous > entry.rank => {
            (format!("▲ {}", previous - entry.rank), style::up)
        }
        Some(previous) if previous < entry.rank => {
            (format!("▼ {}", entry.rank - previous), style::down)
        }
        Some(_) => ("—".to_owned(), style::subtle),
    };
    text(label).font(style::BOLD).size(13).style(tone).into()
}

fn sidebar(poll: &Poll) -> Element<'_, Message> {
    let versus = poll.week.as_ref().and_then(|data| {
        let rows = &data.saved.disagreements;
        (!rows.is_empty()).then(|| {
            let rows = rows.iter().map(|row| {
                let crowd = row
                    .crowd
                    .map_or_else(|| "—".to_owned(), |rank| format!("#{rank}"));
                container(
                    row![
                        text(&row.quarterback.name)
                            .font(style::BOLD)
                            .size(14)
                            .width(Length::Fill),
                        text(format!("You #{} · Crowd {crowd}", row.mine))
                            .size(13)
                            .style(style::ink_2),
                    ]
                    .spacing(12)
                    .align_y(Alignment::Center),
                )
                .padding([12, 14])
                .width(Length::Fill)
                .style(style::inset)
                .into()
            });
            card(
                "You vs. the crowd",
                column![
                    text("Your saved picks compared with the live top 10.")
                        .size(13)
                        .style(style::muted)
                ]
                .extend(rows)
                .spacing(10),
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
        .size(13)
        .line_height(1.6)
        .style(style::muted),
    );
    column![]
        .push(versus)
        .push(scoring)
        .spacing(16)
        .width(SIDEBAR_WIDTH)
        .into()
}

fn card<'a>(title: &'a str, body: impl Into<Element<'a, Message>>) -> Element<'a, Message> {
    container(
        column![
            text(title.to_uppercase()).font(style::DISPLAY).size(22),
            body.into()
        ]
        .spacing(10),
    )
    .padding(20)
    .width(Length::Fill)
    .style(style::panel)
    .into()
}
