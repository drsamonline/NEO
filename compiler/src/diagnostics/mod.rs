//! Diagnostics re-exports
//!
//! The diagnostic types live in `nova-common` so they can be shared by the
//! compiler, LSP, and other tooling. They are re-exported here for
//! convenience.

pub use nova_common::diagnostics::{Diagnostic, DiagnosticLevel, Diagnostics};
