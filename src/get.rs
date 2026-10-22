//! A plain GET client for files this program downloads for itself, such as a theme.
//!
//! No cookie jar and no browser impersonation. Site sessions are not this client.

/// A client that sends no cookies.
pub fn client() -> anyhow::Result<reqwest::Client> {
    Ok(reqwest::Client::builder().build()?)
}
