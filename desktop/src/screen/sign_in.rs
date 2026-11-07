use iced::widget::{button, center, column, container, row, text};
use iced::{Alignment, Element, Length};

use crate::style;

pub enum State {
    /// Checking a saved session at startup, or ending one on sign-out.
    Restoring,
    SignedOut {
        error: Option<String>,
    },
    SigningIn,
}

#[derive(Debug, Clone)]
pub enum Message {
    SignIn,
}

impl State {
    pub fn view(&self) -> Element<'_, Message> {
        let body: Element<'_, Message> = match self {
            Self::Restoring => text("Loading…").style(style::muted).into(),
            Self::SignedOut { error } => column![
                text("Rank the week’s quarterbacks with the group.")
                    .size(14)
                    .style(style::muted),
                button(text("Sign in with Discord").font(style::BOLD).size(15))
                    .padding([14, 18])
                    .width(Length::Fill)
                    .style(style::primary)
                    .on_press(Message::SignIn),
            ]
            .push(error.as_deref().map(|error| {
                container(text(error).font(style::SEMIBOLD).size(14))
                    .padding([14, 18])
                    .width(Length::Fill)
                    .style(style::notice_error)
            }))
            .spacing(16)
            .into(),
            Self::SigningIn => text("Finish signing in in your browser…")
                .style(style::muted)
                .into(),
        };
        let title = row![
            text("POLLING").font(style::DISPLAY).size(40),
            text("JUEGOS")
                .font(style::DISPLAY)
                .size(40)
                .style(style::accent_text),
        ];
        let card = container(column![title, body].spacing(16).align_x(Alignment::Start))
            .padding(32)
            .width(400)
            .style(style::panel);
        center(card).style(style::page).into()
    }
}
