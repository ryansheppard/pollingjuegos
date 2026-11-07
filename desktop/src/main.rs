mod loopback;
mod session;

use std::sync::Arc;

use iced::futures::channel::oneshot;
use iced::widget::{button, column, text};
use iced::{Element, Task};

use session::{Client, Session};

const DEFAULT_SERVER: &str = "https://pj.ryansheppard.xyz";

#[derive(Debug, Clone)]
enum Message {
    Restored(Result<Option<Session>, String>),
    SignIn,
    SignedIn(Result<Session, String>),
    SignOut,
    SignedOut(Result<(), String>),
}

enum Auth {
    Restoring,
    SignedOut { error: Option<String> },
    SigningIn,
    SignedIn(Session),
}

struct App {
    client: Arc<Client>,
    auth: Auth,
}

/// Runs blocking work on its own thread and resolves when it finishes, so
/// iced's executor never blocks and no async runtime is needed.
fn blocking<T, M>(
    work: impl FnOnce() -> anyhow::Result<T> + Send + 'static,
    to_message: impl FnOnce(Result<T, String>) -> M + Send + 'static,
) -> Task<M>
where
    T: Send + 'static,
    M: Send + 'static,
{
    let (tx, rx) = oneshot::channel();
    std::thread::spawn(move || {
        // `{:#}` keeps anyhow's context chain, e.g. "exchanging login code: 400".
        let _ = tx.send(work().map_err(|e| format!("{e:#}")));
    });
    Task::perform(
        async move {
            rx.await
                .unwrap_or_else(|_| Err("background thread panicked".to_owned()))
        },
        to_message,
    )
}

impl App {
    fn new() -> (Self, Task<Message>) {
        let server = std::env::var("PJ_SERVER").unwrap_or_else(|_| DEFAULT_SERVER.to_owned());
        let client = Arc::new(Client::new(server.trim_end_matches('/').to_owned()));
        let restore = {
            let client = client.clone();
            blocking(move || client.restore(), Message::Restored)
        };
        (
            Self {
                client,
                auth: Auth::Restoring,
            },
            restore,
        )
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Restored(Ok(Some(session))) | Message::SignedIn(Ok(session)) => {
                self.auth = Auth::SignedIn(session);
                Task::none()
            }
            Message::Restored(Ok(None)) => {
                self.auth = Auth::SignedOut { error: None };
                Task::none()
            }
            Message::Restored(Err(error)) | Message::SignedIn(Err(error)) => {
                self.auth = Auth::SignedOut { error: Some(error) };
                Task::none()
            }
            Message::SignIn => {
                self.auth = Auth::SigningIn;
                let client = self.client.clone();
                blocking(move || client.sign_in(), Message::SignedIn)
            }
            Message::SignOut => {
                let Auth::SignedIn(session) = std::mem::replace(&mut self.auth, Auth::Restoring)
                else {
                    return Task::none();
                };
                let client = self.client.clone();
                blocking(move || client.sign_out(&session.token), Message::SignedOut)
            }
            Message::SignedOut(result) => {
                self.auth = Auth::SignedOut {
                    error: result.err(),
                };
                Task::none()
            }
        }
    }

    fn view(&self) -> Element<'_, Message> {
        match &self.auth {
            Auth::Restoring => text("Loading…").into(),
            Auth::SignedOut { error } => column![
                button("Sign in with Discord").on_press(Message::SignIn),
                text(error.as_deref().unwrap_or_default()),
            ]
            .spacing(8)
            .into(),
            Auth::SigningIn => text("Finish signing in in your browser…").into(),
            Auth::SignedIn(session) => column![
                text(format!("Signed in as user #{}", session.user_id)),
                button("Sign out").on_press(Message::SignOut),
            ]
            .spacing(8)
            .into(),
        }
    }
}

fn main() -> iced::Result {
    iced::application(App::new, App::update, App::view)
        .title("pollingjuegos")
        .run()
}
