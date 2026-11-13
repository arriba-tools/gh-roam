//! Communication with GitHub: the authenticated REST client and the gists it reads and writes.

mod client;
pub mod gists;

pub use client::Client;
