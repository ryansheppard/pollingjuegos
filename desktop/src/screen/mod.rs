pub mod poll;
pub mod sign_in;

/// What a signed-in screen asks `App` to do after handling a message. Screens
/// report these instead of acting on them so the session lifecycle stays in
/// one place.
pub enum Action {
    None,
    SessionExpired,
    SignOut,
}
