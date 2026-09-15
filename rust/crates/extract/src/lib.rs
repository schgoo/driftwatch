//! The Driftwatch extraction driver.
//!
//! Resolves a target's [`Binding`] (the capture recipe), generates a runner
//! that drives annotated operations, builds and runs it, collects keyed traces,
//! and discovers the contract from the link-time registry.
//!
//! ```
//! let binding = extract::Binding::parse(r#"commands = ["cargo test"]"#)
//!     .expect("valid binding");
//! assert_eq!(binding.commands, ["cargo test"]);
//! ```

mod binding;

pub use binding::{BINDING_FILE_NAME, Binding, BindingError};
