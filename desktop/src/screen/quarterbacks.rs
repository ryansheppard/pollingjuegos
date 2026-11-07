use std::sync::Arc;

use api_types::QuarterbackWithStats;
use iced::widget::{column, scrollable, text};
use iced::{Element, Task};

use super::Action;
use crate::session::Client;
use crate::task::blocking;

pub enum State {
    Loading,
    Loaded(Vec<QuarterbackWithStats>),
    Failed(String),
}

#[derive(Debug, Clone)]
pub enum Message {
    Fetched(Result<Option<Vec<QuarterbackWithStats>>, String>),
}

impl State {
    pub fn load(client: Arc<Client>, token: String) -> (Self, Task<Message>) {
        let fetch = blocking(move || client.quarterbacks(&token), Message::Fetched);
        (Self::Loading, fetch)
    }

    pub fn update(&mut self, message: Message) -> Action {
        match message {
            Message::Fetched(Ok(Some(list))) => *self = Self::Loaded(list),
            Message::Fetched(Ok(None)) => return Action::SessionExpired,
            Message::Fetched(Err(error)) => *self = Self::Failed(error),
        }
        Action::None
    }

    pub fn view(&self) -> Element<'_, Message> {
        match self {
            Self::Loading => text("Loading quarterbacks…").into(),
            Self::Failed(error) => text(error).into(),
            Self::Loaded(list) => scrollable(
                column(list.iter().map(|qb| text(&qb.quarterback.name).into())).spacing(4),
            )
            .into(),
        }
    }
}
