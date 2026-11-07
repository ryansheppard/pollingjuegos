pub mod quarterbacks;
pub mod sign_in;

/// What a signed-in screen asks `App` to do after handling a message. Screens
/// report a rejected token instead of signing out themselves so the session
/// lifecycle stays in one place.
pub enum Action {
    None,
    SessionExpired,
}
