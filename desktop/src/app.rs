use std::sync::Arc;

use iced::widget::{button, column, text};
use iced::{Element, Task};

use crate::screen::{Action, quarterbacks, sign_in};
use crate::session::{Client, Session};
use crate::task::blocking;

const DEFAULT_SERVER: &str = "https://pj.ryansheppard.xyz";

#[derive(Debug, Clone)]
pub enum Message {
    Restored(Result<Option<Session>, String>),
    SignIn(sign_in::Message),
    SignedIn(Result<Session, String>),
    SignOut,
    SignedOut(Result<(), String>),
    Quarterbacks(quarterbacks::Message),
}

enum Auth {
    SignedOut(sign_in::State),
    SignedIn {
        session: Session,
        quarterbacks: quarterbacks::State,
    },
}

pub struct App {
    client: Arc<Client>,
    auth: Auth,
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
            },
            restore,
        )
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Restored(Ok(Some(session))) | Message::SignedIn(Ok(session)) => {
                let (quarterbacks, load) =
                    quarterbacks::State::load(self.client.clone(), session.token.clone());
                self.auth = Auth::SignedIn {
                    session,
                    quarterbacks,
                };
                load.map(Message::Quarterbacks)
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
            Message::Quarterbacks(message) => {
                // The user may have signed out while a request was in flight.
                let Auth::SignedIn { quarterbacks, .. } = &mut self.auth else {
                    return Task::none();
                };
                match quarterbacks.update(message) {
                    Action::None => {}
                    Action::SessionExpired => self.session_expired(),
                }
                Task::none()
            }
        }
    }

    pub fn view(&self) -> Element<'_, Message> {
        match &self.auth {
            Auth::SignedOut(state) => state.view().map(Message::SignIn),
            Auth::SignedIn {
                session,
                quarterbacks,
            } => column![
                text(format!("Signed in as user #{}", session.user_id)),
                button("Sign out").on_press(Message::SignOut),
                quarterbacks.view().map(Message::Quarterbacks),
            ]
            .spacing(8)
            .into(),
        }
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
