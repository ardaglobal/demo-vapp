#![allow(
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::wildcard_imports
)]

pub mod client;
pub mod config;
pub mod contracts;
pub mod error;
pub mod event_manager;
pub mod types;

pub use client::{ArithmeticEvent, EthereumClient, EventCallback, EventFilter, SubscriptionId};
pub use config::{Config, NetworkConfig};
pub use error::{EthereumError, Result};
pub use event_manager::{EventFilterBuilder, EventHandler, EventManager, VAppEventHandler};
pub use types::*;

#[cfg(feature = "database")]
pub mod cache;

#[cfg(feature = "database")]
pub use cache::EthereumCache;
