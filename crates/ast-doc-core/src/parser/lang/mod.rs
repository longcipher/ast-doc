//! Per-language parser implementations.
//!
//! Each parser is feature-gated behind its corresponding `lang-*` feature.

#[cfg(feature = "lang-rust")]
pub mod rust_parser;

#[cfg(feature = "lang-python")]
pub mod python_parser;

#[cfg(feature = "lang-typescript")]
pub mod typescript_parser;

#[cfg(feature = "lang-go")]
pub mod go_parser;

#[cfg(feature = "lang-c")]
pub mod c_parser;
