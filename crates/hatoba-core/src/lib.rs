//! Crypto, vault, data model, local store and sync engine for Hatoba.

pub mod backup;
pub mod crypto;
pub mod error;
pub mod model;
pub mod platform;
pub mod recovery;
pub mod store;
pub mod sync;
pub mod vault;

pub use error::{Error, Result};
