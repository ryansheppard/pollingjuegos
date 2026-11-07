//! The signed-in screen, laid out like the web app: a header with the week
//! picker, a banner with this week's deadline, and Ballot/Results tabs.

mod ballot;
mod header;
mod logic;
mod pool;
mod results;

use std::sync::Arc;
use std::time::Duration;

use api_types::{Ballot, Quarterback, QuarterbackStats, QuarterbackWithStats, Rankings, Week};
use iced::widget::{button, column, container, row, text};
use iced::{Alignment, Element, Length, Subscription, Task};
use jiff::Timestamp;

use self::logic::{BALLOT_SIZE, Lens};
use super::Action;
use crate::session::Client;
use crate::task::{blocking, both};

/// The deadline countdown only shows minutes.
const TICK: Duration = Duration::from_secs(30);
const SEARCH_INPUT: &str = "pool-search";

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
    /// The ballot being edited; compared with `week.saved` to find unsaved changes.
    ballot: Vec<Quarterback>,
    search: String,
    lens: Lens,
    removed: Option<Removed>,
    /// A save or "copy last week" request is in flight.
    saving: bool,
}

#[derive(Debug, Clone)]
pub struct WeekData {
    pub rankings: Rankings,
    pub saved: Ballot,
}

struct Removed {
    quarterback: Quarterback,
    index: usize,
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
    Search(String),
    FocusSearch,
    AddTopMatch,
    Add(i64),
    MoveUp(usize),
    MoveDown(usize),
    Remove(usize),
    Undo,
    SetLens(Lens),
    Save,
    Saved {
        generation: u64,
        result: Fetched<Ballot>,
    },
    RankingsRefreshed {
        generation: u64,
        result: Fetched<Rankings>,
    },
    CopyPrevious,
    CopiedPrevious {
        generation: u64,
        result: Fetched<Ballot>,
    },
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
            ballot: Vec::new(),
            search: String::new(),
            lens: Lens::Epa,
            removed: None,
            saving: false,
        };
        (poll, load)
    }

    pub fn update(&mut self, message: Message) -> (Task<Message>, Action) {
        let task = match message {
            Message::Loaded(Ok(Some((weeks, catalog)))) => {
                self.ready = true;
                self.weeks = weeks;
                self.catalog = catalog;
                match logic::initial_week(&self.weeks, self.now) {
                    Some(index) => self.load_week(index),
                    None => Task::none(),
                }
            }
            Message::Loaded(Ok(None))
            | Message::WeekLoaded {
                result: Ok(None), ..
            }
            | Message::Saved {
                result: Ok(None), ..
            }
            | Message::RankingsRefreshed {
                result: Ok(None), ..
            }
            | Message::CopiedPrevious {
                result: Ok(None), ..
            } => return (Task::none(), Action::SessionExpired),
            Message::Loaded(Err(error)) => {
                self.ready = true;
                self.error = Some(error);
                Task::none()
            }
            Message::SelectWeek(index) => {
                if self.busy() || index >= self.weeks.len() || self.selected == Some(index) {
                    return (Task::none(), Action::None);
                }
                self.load_week(index)
            }
            Message::WeekLoaded { generation, result } => {
                if generation == self.generation {
                    self.week_loaded(result);
                }
                Task::none()
            }
            Message::SelectTab(tab) => {
                self.tab = tab;
                Task::none()
            }
            Message::CopyResults => self.copy_results(),
            Message::Search(search) => {
                self.search = search;
                Task::none()
            }
            Message::FocusSearch => iced::widget::operation::focus(SEARCH_INPUT),
            Message::AddTopMatch => {
                if !self.search.trim().is_empty()
                    && let Some(top) = self.available().first()
                {
                    self.add(top.quarterback.id);
                }
                Task::none()
            }
            Message::Add(id) => {
                self.add(id);
                Task::none()
            }
            Message::MoveUp(index) => {
                if index > 0 {
                    self.swap(index, index - 1);
                }
                Task::none()
            }
            Message::MoveDown(index) => {
                self.swap(index, index + 1);
                Task::none()
            }
            Message::Remove(index) => {
                if self.can_edit() && index < self.ballot.len() {
                    let quarterback = self.ballot.remove(index);
                    self.notice = None;
                    self.removed = Some(Removed { quarterback, index });
                }
                Task::none()
            }
            Message::Undo => {
                if self.can_undo()
                    && let Some(Removed { quarterback, index }) = self.removed.take()
                {
                    self.ballot
                        .insert(index.min(self.ballot.len()), quarterback);
                }
                Task::none()
            }
            Message::SetLens(lens) => {
                self.lens = lens;
                Task::none()
            }
            Message::Save => self.save(),
            Message::Saved { generation, result } => {
                if generation != self.generation {
                    return (Task::none(), Action::None);
                }
                self.saving = false;
                self.saved(result)
            }
            Message::RankingsRefreshed { generation, result } => {
                if generation == self.generation {
                    match result {
                        Ok(rankings) => {
                            if let (Some(data), Some(rankings)) = (&mut self.week, rankings) {
                                data.rankings = rankings;
                            }
                        }
                        Err(_) => {
                            self.error = Some(
                                "Ballot saved, but rankings could not be refreshed. \
                                 Switch weeks to try again."
                                    .to_owned(),
                            );
                        }
                    }
                }
                Task::none()
            }
            Message::CopyPrevious => self.copy_previous(),
            Message::CopiedPrevious { generation, result } => {
                if generation == self.generation {
                    self.saving = false;
                    self.copied_previous(result);
                }
                Task::none()
            }
            Message::Tick => {
                self.now = Timestamp::now();
                Task::none()
            }
            Message::SignOut => return (Task::none(), Action::SignOut),
        };
        (task, Action::None)
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
        if let (Some(removed), Tab::Ballot) = (&self.removed, self.tab) {
            let undo = (self.can_undo() && !self.saving).then_some(Message::Undo);
            page = page.push(
                row![
                    text(format!("Removed {}.", removed.quarterback.name)),
                    button("Undo").style(button::text).on_press_maybe(undo),
                ]
                .spacing(8)
                .align_y(Alignment::Center),
            );
        }
        container(page.push(content))
            .padding(16)
            .height(Length::Fill)
            .into()
    }

    fn load_week(&mut self, index: usize) -> Task<Message> {
        let Week { season, week, .. } = self.weeks[index];
        self.selected = Some(index);
        self.loading_week = true;
        self.error = None;
        self.notice = None;
        self.removed = None;
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

    fn week_loaded(&mut self, result: Fetched<WeekData>) {
        self.loading_week = false;
        // A closed week has nothing left to vote on, so lead with its results.
        self.tab = if self.closed() {
            Tab::Results
        } else {
            Tab::Ballot
        };
        match result {
            Ok(data) => {
                self.ballot = data
                    .iter()
                    .flat_map(|data| &data.saved.entries)
                    .map(|entry| entry.quarterback.clone())
                    .collect();
                self.week = data;
            }
            Err(error) => {
                // Don't leave the previous week's picks under this week's header.
                self.week = None;
                self.ballot.clear();
                self.error = Some(error);
            }
        }
    }

    fn copy_results(&mut self) -> Task<Message> {
        // While another week loads, `self.week` still holds the old one.
        let (Some(week), Some(data)) = (self.current_week(), &self.week) else {
            return Task::none();
        };
        if self.loading_week {
            return Task::none();
        }
        let copy = iced::clipboard::write(logic::results_text(week, &data.rankings));
        self.notice = Some("Copied the top 10 to the clipboard.".to_owned());
        copy
    }

    fn add(&mut self, id: i64) {
        if !self.can_edit()
            || self.ballot.len() >= BALLOT_SIZE
            || self.ballot.iter().any(|qb| qb.id == id)
        {
            return;
        }
        let Some(qb) = self.catalog.iter().find(|qb| qb.quarterback.id == id) else {
            return;
        };
        self.ballot.push(qb.quarterback.clone());
        self.search.clear();
        self.notice = None;
        self.removed = None;
    }

    fn swap(&mut self, a: usize, b: usize) {
        if self.can_edit() && a < self.ballot.len() && b < self.ballot.len() {
            self.ballot.swap(a, b);
            self.notice = None;
            self.removed = None;
        }
    }

    fn save(&mut self) -> Task<Message> {
        let Some(&Week { season, week, .. }) = self.current_week() else {
            return Task::none();
        };
        if !self.can_edit() || self.ballot.len() != BALLOT_SIZE || !self.dirty() {
            return Task::none();
        }
        self.saving = true;
        self.error = None;
        self.notice = None;
        self.removed = None;
        let ids = self.ballot.iter().map(|qb| qb.id).collect();
        let (client, token, generation) =
            (self.client.clone(), self.token.clone(), self.generation);
        blocking(
            move || client.save_ballot(&token, season, week, ids),
            move |result| Message::Saved { generation, result },
        )
    }

    fn saved(&mut self, result: Fetched<Ballot>) -> Task<Message> {
        match result {
            Ok(Some(saved)) => {
                self.ballot = saved
                    .entries
                    .iter()
                    .map(|entry| entry.quarterback.clone())
                    .collect();
                if let Some(data) = &mut self.week {
                    data.saved = saved;
                }
                self.notice = Some("Ballot saved. Horrible picks.".to_owned());
                self.refresh_rankings()
            }
            Ok(None) => Task::none(),
            Err(error) => {
                // The deadline passed since the week list was fetched; lock the
                // ballot instead of letting the user retry.
                if error.contains("closed")
                    && let Some(index) = self.selected
                {
                    self.weeks[index].is_closed = true;
                    self.now = Timestamp::now();
                }
                self.error = Some(error);
                Task::none()
            }
        }
    }

    fn refresh_rankings(&self) -> Task<Message> {
        let Some(&Week { season, week, .. }) = self.current_week() else {
            return Task::none();
        };
        let (client, token, generation) =
            (self.client.clone(), self.token.clone(), self.generation);
        blocking(
            move || client.rankings(&token, season, week),
            move |result| Message::RankingsRefreshed { generation, result },
        )
    }

    fn copy_previous(&mut self) -> Task<Message> {
        let Some(&Week { season, week, .. }) = self.previous_week() else {
            return Task::none();
        };
        if !self.can_edit() {
            return Task::none();
        }
        self.saving = true;
        self.error = None;
        let (client, token, generation) =
            (self.client.clone(), self.token.clone(), self.generation);
        blocking(
            move || client.ballot(&token, season, week),
            move |result| Message::CopiedPrevious { generation, result },
        )
    }

    fn copied_previous(&mut self, result: Fetched<Ballot>) {
        match result {
            Ok(Some(last)) if last.entries.is_empty() => {
                self.notice = Some("You did not submit a ballot last week.".to_owned());
            }
            Ok(Some(last)) => {
                self.ballot = last
                    .entries
                    .into_iter()
                    .map(|entry| entry.quarterback)
                    .collect();
                self.removed = None;
                self.notice = Some(
                    "Copied last week’s picks. Review and save to submit this week.".to_owned(),
                );
            }
            Ok(None) => {}
            Err(error) => self.error = Some(error),
        }
    }

    fn current_week(&self) -> Option<&Week> {
        self.selected.map(|index| &self.weeks[index])
    }

    /// Weeks are newest first, so the previous week is the next index.
    fn previous_week(&self) -> Option<&Week> {
        self.selected.and_then(|index| self.weeks.get(index + 1))
    }

    fn closed(&self) -> bool {
        self.current_week()
            .is_some_and(|week| logic::is_closed(week, self.now))
    }

    /// Week switching and edits wait for in-flight requests so their results
    /// land on the week they were made for.
    fn busy(&self) -> bool {
        !self.ready || self.loading_week || self.saving
    }

    fn can_edit(&self) -> bool {
        self.week.is_some() && !self.closed() && !self.busy()
    }

    fn can_undo(&self) -> bool {
        self.removed.as_ref().is_some_and(|removed| {
            !self.closed()
                && self.ballot.len() < BALLOT_SIZE
                && !self.ballot.iter().any(|qb| qb.id == removed.quarterback.id)
        })
    }

    fn dirty(&self) -> bool {
        let saved = self.week.iter().flat_map(|data| &data.saved.entries);
        !self
            .ballot
            .iter()
            .map(|qb| qb.id)
            .eq(saved.map(|entry| entry.quarterback.id))
    }

    fn has_saved(&self) -> bool {
        self.week
            .as_ref()
            .is_some_and(|data| !data.saved.entries.is_empty())
    }

    /// Ballot and ranking payloads don't carry stats, so they're joined from
    /// the full quarterback list.
    fn stats_for(&self, id: i64) -> Option<&QuarterbackStats> {
        self.catalog
            .iter()
            .find(|qb| qb.quarterback.id == id)
            .and_then(|qb| qb.stats.as_ref())
    }

    fn crowd_rank(&self, id: i64) -> Option<i64> {
        self.week.as_ref().and_then(|data| {
            data.rankings
                .rankings
                .iter()
                .find(|row| row.quarterback.id == id)
                .map(|row| row.rank)
        })
    }

    /// The pool: quarterbacks not on the ballot that match the search.
    fn available(&self) -> Vec<&QuarterbackWithStats> {
        self.catalog
            .iter()
            .filter(|qb| {
                !self
                    .ballot
                    .iter()
                    .any(|picked| picked.id == qb.quarterback.id)
                    && logic::matches_search(&qb.quarterback, &self.search)
            })
            .collect()
    }
}
