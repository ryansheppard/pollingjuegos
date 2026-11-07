//! The signed-in screen, laid out like the web app: a header with the week
//! picker, a banner with this week's deadline, and Ballot/Results tabs.

mod ballot;
mod header;
mod logic;
mod results;

use std::sync::Arc;
use std::time::Duration;

use api_types::{Ballot, QuarterbackWithStats, Rankings, Week};
use iced::widget::{column, container, text};
use iced::{Element, Subscription, Task};
use jiff::Timestamp;

use super::Action;
use crate::session::Client;
use crate::task::{blocking, both};

/// The deadline countdown only shows minutes.
const TICK: Duration = Duration::from_secs(30);

pub struct Poll {
    client: Arc<Client>,
    token: String,
    /// False until the week list and quarterbacks have loaded.
    ready: bool,
    /// Newest first, as the server sends them.
    weeks: Vec<Week>,
    catalog: Vec<QuarterbackWithStats>,
    selected: Option<usize>,
    /// The last week that loaded. It stays on screen while another week loads
    /// so the layout doesn't collapse.
    week: Option<WeekData>,
    loading_week: bool,
    /// Bumped on every week load so a slow response for a week the user has
    /// already left is dropped.
    generation: u64,
    tab: Tab,
    error: Option<String>,
    /// Confirmation of the last action, e.g. copying results.
    notice: Option<String>,
    now: Timestamp,
}

#[derive(Debug, Clone)]
pub struct WeekData {
    pub rankings: Rankings,
    pub saved: Ballot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Ballot,
    Results,
}

type Fetched<T> = Result<Option<T>, String>;

#[derive(Debug, Clone)]
pub enum Message {
    Loaded(Fetched<(Vec<Week>, Vec<QuarterbackWithStats>)>),
    SelectWeek(usize),
    WeekLoaded {
        generation: u64,
        result: Fetched<WeekData>,
    },
    SelectTab(Tab),
    CopyResults,
    Tick,
    SignOut,
}

impl Poll {
    pub fn new(client: Arc<Client>, token: String) -> (Self, Task<Message>) {
        let load = {
            let (client, token) = (client.clone(), token.clone());
            blocking(
                move || {
                    let (weeks, catalog) =
                        both(|| client.weeks(&token), || client.quarterbacks(&token));
                    Ok(weeks?.zip(catalog?))
                },
                Message::Loaded,
            )
        };
        let poll = Self {
            client,
            token,
            ready: false,
            weeks: Vec::new(),
            catalog: Vec::new(),
            selected: None,
            week: None,
            loading_week: false,
            generation: 0,
            tab: Tab::Ballot,
            error: None,
            notice: None,
            now: Timestamp::now(),
        };
        (poll, load)
    }

    pub fn update(&mut self, message: Message) -> (Task<Message>, Action) {
        match message {
            Message::Loaded(Ok(Some((weeks, catalog)))) => {
                self.ready = true;
                self.weeks = weeks;
                self.catalog = catalog;
                match logic::initial_week(&self.weeks, self.now) {
                    Some(index) => (self.load_week(index), Action::None),
                    None => (Task::none(), Action::None),
                }
            }
            Message::Loaded(Ok(None))
            | Message::WeekLoaded {
                result: Ok(None), ..
            } => (Task::none(), Action::SessionExpired),
            Message::Loaded(Err(error)) => {
                self.ready = true;
                self.error = Some(error);
                (Task::none(), Action::None)
            }
            Message::SelectWeek(index) => {
                if self.loading_week || index >= self.weeks.len() || self.selected == Some(index) {
                    return (Task::none(), Action::None);
                }
                (self.load_week(index), Action::None)
            }
            Message::WeekLoaded { generation, result } => {
                if generation != self.generation {
                    return (Task::none(), Action::None);
                }
                self.loading_week = false;
                // A closed week has nothing left to vote on, so lead with its results.
                self.tab = if self.closed() {
                    Tab::Results
                } else {
                    Tab::Ballot
                };
                match result {
                    Ok(data) => self.week = data,
                    Err(error) => {
                        // Don't leave the previous week's data under this week's header.
                        self.week = None;
                        self.error = Some(error);
                    }
                }
                (Task::none(), Action::None)
            }
            Message::SelectTab(tab) => {
                self.tab = tab;
                (Task::none(), Action::None)
            }
            Message::CopyResults => {
                // While another week loads, `self.week` still holds the old one.
                let (Some(week), Some(data)) = (self.current_week(), &self.week) else {
                    return (Task::none(), Action::None);
                };
                if self.loading_week {
                    return (Task::none(), Action::None);
                }
                let copy = iced::clipboard::write(logic::results_text(week, &data.rankings));
                self.notice = Some("Copied the top 10 to the clipboard.".to_owned());
                (copy, Action::None)
            }
            Message::Tick => {
                self.now = Timestamp::now();
                (Task::none(), Action::None)
            }
            Message::SignOut => (Task::none(), Action::SignOut),
        }
    }

    pub fn subscription(&self) -> Subscription<Message> {
        iced::time::every(TICK).map(|_| Message::Tick)
    }

    pub fn view(&self) -> Element<'_, Message> {
        let content: Element<'_, Message> = if !self.ready {
            text("Loading the field…").into()
        } else if self.weeks.is_empty() {
            text("No weeks have been created yet.").into()
        } else {
            match self.tab {
                Tab::Ballot => ballot::view(self),
                Tab::Results => results::view(self),
            }
        };
        let mut page = column![header::view(self)].spacing(16);
        if let Some(error) = &self.error {
            page = page.push(text(error).style(text::danger));
        }
        if let Some(notice) = &self.notice {
            page = page.push(text(notice).style(text::success));
        }
        container(page.push(content)).padding(16).into()
    }

    fn load_week(&mut self, index: usize) -> Task<Message> {
        let Week { season, week, .. } = self.weeks[index];
        self.selected = Some(index);
        self.loading_week = true;
        self.error = None;
        self.notice = None;
        self.generation += 1;
        let generation = self.generation;
        let (client, token) = (self.client.clone(), self.token.clone());
        blocking(
            move || {
                let (rankings, saved) = both(
                    || client.rankings(&token, season, week),
                    || client.ballot(&token, season, week),
                );
                Ok(rankings?
                    .zip(saved?)
                    .map(|(rankings, saved)| WeekData { rankings, saved }))
            },
            move |result| Message::WeekLoaded { generation, result },
        )
    }

    fn current_week(&self) -> Option<&Week> {
        self.selected.map(|index| &self.weeks[index])
    }

    fn closed(&self) -> bool {
        self.current_week()
            .is_some_and(|week| logic::is_closed(week, self.now))
    }
}
