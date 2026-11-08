//! The signed-in screen, laid out like the web app: a header with the week
//! picker, a banner with this week's deadline, and Ballot/Results tabs.

mod ballot;
mod header;
mod logic;
mod pool;
mod results;

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use api_types::{
    BALLOT_SIZE, Ballot, Quarterback, QuarterbackStats, QuarterbackWithStats, Rankings, Week,
};
use iced::event::{self, Event};
use iced::keyboard::{self, Key, key::Named};
use iced::widget::{button, center, column, container, opaque, row, space, stack, text};
use iced::{Alignment, Element, Length, Subscription, Task, Theme};
use jiff::Timestamp;

pub use self::logic::Lens;
use super::Action;
use crate::session::{Client, Session};
use crate::store::{DraftKey, Store};
use crate::style;
use crate::task::{blocking, both};

/// The deadline countdown only shows minutes.
const TICK: Duration = Duration::from_secs(30);
const SEARCH_INPUT: &str = "pool-search";

pub struct Poll {
    client: Arc<Client>,
    token: String,
    user_id: i64,
    store: Store,
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
    /// A yes/no question shown over the page; edits wait until it's answered.
    confirm: Option<Confirm>,
}

#[derive(Debug, Clone, Copy)]
enum Confirm {
    ReplaceWithLastWeek,
    Add(i64),
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
    ConfirmYes,
    ConfirmNo,
    /// `captured` means a widget (the search box) already handled the key.
    Escape {
        captured: bool,
    },
    Tick,
    SignOut,
}

impl Poll {
    pub fn new(client: Arc<Client>, session: &Session) -> (Self, Task<Message>) {
        let token = session.token.clone();
        let store = Store::load();
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
            user_id: session.user_id,
            lens: store.lens().unwrap_or(Lens::Epa),
            store,
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
            removed: None,
            saving: false,
            confirm: None,
        };
        (poll, load)
    }

    pub fn update(&mut self, message: Message) -> (Task<Message>, Action) {
        let task = match message {
            Message::Loaded(Ok(Some((weeks, catalog)))) => {
                self.ready = true;
                self.weeks = weeks;
                self.catalog = catalog;
                match self.weeks.iter().position(|week| week.is_default) {
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
            Message::FocusSearch => {
                if self.confirm.is_some() {
                    return (Task::none(), Action::None);
                }
                self.tab = Tab::Ballot;
                iced::widget::operation::focus(SEARCH_INPUT)
            }
            Message::AddTopMatch => {
                if !self.search.trim().is_empty()
                    && let Some(top) = self.available().first()
                {
                    self.request_add(top.quarterback.id);
                }
                Task::none()
            }
            Message::Add(id) => {
                self.request_add(id);
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
                    self.persist_draft();
                }
                Task::none()
            }
            Message::Undo => {
                if self.can_undo()
                    && let Some(Removed { quarterback, index }) = self.removed.take()
                {
                    self.ballot
                        .insert(index.min(self.ballot.len()), quarterback);
                    self.persist_draft();
                }
                Task::none()
            }
            Message::SetLens(lens) => {
                self.lens = lens;
                self.store.set_lens(lens);
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
            Message::CopyPrevious => {
                if self.dirty() && self.can_edit() {
                    self.confirm = Some(Confirm::ReplaceWithLastWeek);
                    Task::none()
                } else {
                    self.copy_previous()
                }
            }
            Message::CopiedPrevious { generation, result } => {
                if generation == self.generation {
                    self.saving = false;
                    self.copied_previous(result);
                }
                Task::none()
            }
            Message::ConfirmYes => match self.confirm.take() {
                Some(Confirm::ReplaceWithLastWeek) => self.copy_previous(),
                Some(Confirm::Add(id)) => {
                    self.add(id);
                    Task::none()
                }
                None => Task::none(),
            },
            Message::ConfirmNo => {
                self.confirm = None;
                Task::none()
            }
            Message::Escape { captured } => {
                if self.confirm.take().is_some() || self.search.is_empty() {
                    return (Task::none(), Action::None);
                }
                // Like the web app, the first Esc clears the search; the search
                // box drops focus on Esc, so give it back.
                self.search.clear();
                if captured {
                    iced::widget::operation::focus(SEARCH_INPUT)
                } else {
                    Task::none()
                }
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
        let keys = event::listen_with(|event, status, _window| {
            let Event::Keyboard(keyboard::Event::KeyPressed {
                modified_key,
                modifiers,
                ..
            }) = event
            else {
                return None;
            };
            match modified_key.as_ref() {
                Key::Named(Named::Escape) => Some(Message::Escape {
                    captured: status == event::Status::Captured,
                }),
                // Only when nothing captured it, so typing "/" in a text box
                // still types a slash.
                Key::Character("/")
                    if status == event::Status::Ignored
                        && !(modifiers.control() || modifiers.alt() || modifiers.logo()) =>
                {
                    Some(Message::FocusSearch)
                }
                _ => None,
            }
        });
        Subscription::batch([iced::time::every(TICK).map(|_| Message::Tick), keys])
    }

    pub fn view(&self) -> Element<'_, Message> {
        let content: Element<'_, Message> = if !self.ready {
            text("Loading the field…").style(style::muted).into()
        } else if self.weeks.is_empty() {
            text("No weeks have been created yet.")
                .style(style::muted)
                .into()
        } else {
            match self.tab {
                Tab::Ballot => ballot::view(self),
                Tab::Results => results::view(self),
            }
        };
        let mut main = column![].spacing(16);
        if let Some(error) = &self.error {
            main = main.push(notice(text(error), style::notice_error));
        }
        if let Some(message) = &self.notice {
            main = main.push(notice(text(message), style::notice_success));
        }
        if let (Some(removed), Tab::Ballot) = (&self.removed, self.tab) {
            let undo = (self.can_undo() && !self.saving).then_some(Message::Undo);
            main = main.push(notice(
                row![
                    text(format!("Removed {}.", removed.quarterback.name)),
                    space::horizontal(),
                    button(text("Undo").font(style::BOLD).size(13))
                        .padding([8, 14])
                        .style(style::secondary)
                        .on_press_maybe(undo),
                ]
                .align_y(Alignment::Center),
                style::notice_undo,
            ));
        }
        let main = container(main.push(content))
            .padding([28, 24])
            .height(Length::Fill);
        let page = container(column![header::view(self), main])
            .width(Length::Fill)
            .height(Length::Fill)
            .style(style::page);
        match self.confirm {
            Some(confirm) => stack![page, confirm_dialog(confirm)].into(),
            None => page.into(),
        }
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
                self.restore_draft();
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
        self.persist_draft();
    }

    fn request_add(&mut self, id: i64) {
        let is_cam_ward = self.catalog.iter().any(|qb| {
            qb.quarterback.id == id && qb.quarterback.name.trim().eq_ignore_ascii_case("cam ward")
        });
        if is_cam_ward && self.can_edit() {
            self.confirm = Some(Confirm::Add(id));
        } else {
            self.add(id);
        }
    }

    fn swap(&mut self, a: usize, b: usize) {
        if self.can_edit() && a < self.ballot.len() && b < self.ballot.len() {
            self.ballot.swap(a, b);
            self.notice = None;
            self.removed = None;
            self.persist_draft();
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
                self.persist_draft();
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
                self.persist_draft();
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
        self.week.is_some() && !self.closed() && !self.busy() && self.confirm.is_none()
    }

    fn draft_key(&self) -> Option<DraftKey> {
        let week = self.current_week()?;
        Some(DraftKey::new(
            self.client.server(),
            self.user_id,
            week.season,
            week.week,
        ))
    }

    /// Keeps unsaved edits on this device until they're saved, and drops the
    /// draft once the ballot matches what the server has.
    fn persist_draft(&mut self) {
        if self.week.is_none() || self.closed() {
            return;
        }
        let Some(key) = self.draft_key() else {
            return;
        };
        let ids = self
            .dirty()
            .then(|| self.ballot.iter().map(|qb| qb.id).collect());
        self.store.set_draft(&key, ids);
    }

    fn restore_draft(&mut self) {
        if self.week.is_none() || self.closed() {
            return;
        }
        let Some(key) = self.draft_key() else {
            return;
        };
        let Some(ids) = self.store.draft(&key).map(<[i64]>::to_vec) else {
            return;
        };
        match self.draft_quarterbacks(&ids) {
            Some(ballot) => {
                self.ballot = ballot;
                if self.dirty() {
                    self.notice =
                        Some("Your unfinished draft was restored on this device.".to_owned());
                }
            }
            None => self.store.set_draft(&key, None),
        }
    }

    /// `None` if the stored draft can't be a valid ballot, e.g. a quarterback
    /// was removed from the server since it was saved.
    fn draft_quarterbacks(&self, ids: &[i64]) -> Option<Vec<Quarterback>> {
        let unique: HashSet<_> = ids.iter().collect();
        if ids.len() > BALLOT_SIZE || unique.len() != ids.len() {
            return None;
        }
        ids.iter()
            .map(|&id| {
                self.catalog
                    .iter()
                    .find(|qb| qb.quarterback.id == id)
                    .map(|qb| qb.quarterback.clone())
            })
            .collect()
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

fn notice<'a>(
    content: impl Into<Element<'a, Message>>,
    style: fn(&Theme) -> container::Style,
) -> Element<'a, Message> {
    container(content)
        .padding([14, 18])
        .width(Length::Fill)
        .style(style)
        .into()
}

fn confirm_dialog<'a>(confirm: Confirm) -> Element<'a, Message> {
    let (question, yes) = match confirm {
        Confirm::ReplaceWithLastWeek => (
            "Replace your unfinished changes with last week’s ballot?",
            "Replace",
        ),
        Confirm::Add(_) => ("Are you sure?", "Yes"),
    };
    let action = |label, style: fn(&Theme, button::Status) -> button::Style, message| {
        button(text(label).font(style::BOLD).size(14))
            .padding([12, 18])
            .style(style)
            .on_press(message)
    };
    let dialog = container(
        column![
            text(question).font(style::SEMIBOLD).size(15),
            row![
                action("Cancel", style::secondary, Message::ConfirmNo),
                action(yes, style::primary, Message::ConfirmYes),
            ]
            .spacing(8),
        ]
        .spacing(20)
        .align_x(Alignment::End),
    )
    .padding(24)
    .max_width(420)
    .style(style::panel);
    // Dims the page and swallows clicks so nothing behind the dialog changes.
    opaque(center(dialog).style(style::scrim))
}
