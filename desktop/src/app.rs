use std::sync::Arc;

use iced::theme::Mode;
use iced::{Element, Subscription, Task, Theme};

use crate::screen::{Action, poll, sign_in};
use crate::session::{Client, Session};
use crate::style;
use crate::task::blocking;

const DEFAULT_SERVER: &str = "https://pj.ryansheppard.xyz";

#[derive(Debug, Clone)]
pub enum Message {
    Restored(Result<Option<Session>, String>),
    SignIn(sign_in::Message),
    SignedIn(Result<Session, String>),
    SignOut,
    SignedOut(Result<(), String>),
    Poll(poll::Message),
    ThemeChanged(Mode),
}

enum Auth {
    SignedOut(sign_in::State),
    SignedIn {
        session: Session,
        poll: Box<poll::Poll>,
    },
}

pub struct App {
    client: Arc<Client>,
    auth: Auth,
    mode: Mode,
}

impl App {
    pub fn new() -> (Self, Task<Message>) {
        let server = std::env::var("PJ_SERVER").unwrap_or_else(|_| DEFAULT_SERVER.to_owned());
        let client = Arc::new(Client::new(server.trim_end_matches('/').to_owned()));
        let restore = {
            let client = client.clone();
            blocking(move || client.restore(), Message::Restored)
        };
        (
            Self {
                client,
                auth: Auth::SignedOut(sign_in::State::Restoring),
                mode: Mode::None,
            },
            Task::batch([restore, iced::system::theme().map(Message::ThemeChanged)]),
        )
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Restored(Ok(Some(session))) | Message::SignedIn(Ok(session)) => {
                let (poll, load) = poll::Poll::new(self.client.clone(), &session);
                self.auth = Auth::SignedIn {
                    session,
                    poll: Box::new(poll),
                };
                load.map(Message::Poll)
            }
            Message::Restored(Ok(None)) => {
                self.sign_out_with(None);
                Task::none()
            }
            Message::Restored(Err(error)) | Message::SignedIn(Err(error)) => {
                self.sign_out_with(Some(error));
                Task::none()
            }
            Message::SignIn(sign_in::Message::SignIn) => {
                self.auth = Auth::SignedOut(sign_in::State::SigningIn);
                let client = self.client.clone();
                blocking(move || client.sign_in(), Message::SignedIn)
            }
            Message::SignOut => {
                let Auth::SignedIn { session, .. } =
                    std::mem::replace(&mut self.auth, Auth::SignedOut(sign_in::State::Restoring))
                else {
                    return Task::none();
                };
                let client = self.client.clone();
                blocking(move || client.sign_out(&session.token), Message::SignedOut)
            }
            Message::SignedOut(result) => {
                self.sign_out_with(result.err());
                Task::none()
            }
            Message::ThemeChanged(mode) => {
                self.mode = mode;
                Task::none()
            }
            Message::Poll(message) => {
                // The user may have signed out while a request was in flight.
                let Auth::SignedIn { poll, .. } = &mut self.auth else {
                    return Task::none();
                };
                let (task, action) = poll.update(message);
                match action {
                    Action::None => task.map(Message::Poll),
                    Action::SessionExpired => {
                        self.session_expired();
                        Task::none()
                    }
                    Action::SignOut => self.update(Message::SignOut),
                }
            }
        }
    }

    pub fn view(&self) -> Element<'_, Message> {
        match &self.auth {
            Auth::SignedOut(state) => state.view().map(Message::SignIn),
            Auth::SignedIn { poll, .. } => poll.view().map(Message::Poll),
        }
    }

    pub fn theme(&self) -> Theme {
        style::theme(self.mode)
    }

    pub fn subscription(&self) -> Subscription<Message> {
        let screen = match &self.auth {
            Auth::SignedOut(_) => Subscription::none(),
            Auth::SignedIn { poll, .. } => poll.subscription().map(Message::Poll),
        };
        Subscription::batch([
            screen,
            iced::system::theme_changes().map(Message::ThemeChanged),
        ])
    }

    fn sign_out_with(&mut self, error: Option<String>) {
        self.auth = Auth::SignedOut(sign_in::State::SignedOut { error });
    }

    /// The client has already dropped the token from the keyring by the time a
    /// screen reports this.
    fn session_expired(&mut self) {
        self.sign_out_with(Some("Your session expired. Sign in again.".to_owned()));
    }
}
