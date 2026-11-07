use iced::Element;
use iced::widget::{button, column, text};

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
        match self {
            Self::Restoring => text("Loading…").into(),
            Self::SignedOut { error } => column![
                button("Sign in with Discord").on_press(Message::SignIn),
                text(error.as_deref().unwrap_or_default()),
            ]
            .spacing(8)
            .into(),
            Self::SigningIn => text("Finish signing in in your browser…").into(),
        }
    }
}
