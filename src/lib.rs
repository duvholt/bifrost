#![warn(
    clippy::all,
    clippy::correctness,
    clippy::pedantic,
    clippy::cargo,
    clippy::nursery,
    clippy::perf,
    clippy::style
)]
#![allow(
    clippy::multiple_crate_versions,
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::module_name_repetitions
)]

pub mod backend;
pub mod config;
pub mod error;
pub mod hue;
pub mod mdns;
pub mod model;
pub mod resource;
pub mod routes;
pub mod server;
pub mod z2m;
