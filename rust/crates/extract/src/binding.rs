//! The Driftwatch binding file (`driftwatch.toml`): a capture recipe.
//!
//! A binding is the *how* half of a capture — the shell commands that build and
//! run one target's annotated code so the runtime emits traces the harness then
//! collects. The *what* half — which working tree, git ref, directory, or
//! already-captured artifact a capture runs against — is the CLI target
//! resolver's concern, not the binding's. Each codebase therefore carries its
//! own single-recipe binding; cross-version and cross-codebase comparison is
//! composed by pointing the CLI at two targets, each resolving to its own
//! binding.

use std::fmt;
use std::path::{Path, PathBuf};

use serde::Deserialize;

/// The canonical binding file name resolved inside a target directory.
pub const BINDING_FILE_NAME: &str = "driftwatch.toml";

/// A capture recipe: the ordered shell commands that drive a target's annotated
/// code so it emits traces.
///
/// Multi-language captures need no special support — they are simply more
/// commands (e.g. `cargo test …` then `dotnet test …`), all collected into one
/// capture.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Binding {
    /// The shell commands run, in order, to build and exercise the annotated
    /// code. At least one is required and none may be blank.
    pub commands: Vec<String>,
}

impl Binding {
    /// Parse and validate a binding from TOML source.
    ///
    /// # Errors
    /// Returns [`BindingError::Parse`] on malformed or shape-invalid TOML,
    /// [`BindingError::NoCommands`] when `commands` is empty, and
    /// [`BindingError::BlankCommand`] when a command is empty or whitespace.
    pub fn parse(toml_src: &str) -> Result<Self, BindingError> {
        let binding: Self = toml::from_str(toml_src).map_err(BindingError::Parse)?;
        binding.validate()?;
        Ok(binding)
    }

    /// Resolve the `driftwatch.toml` binding inside `dir`.
    ///
    /// # Errors
    /// Returns [`BindingError::Read`] when the file cannot be read, plus any
    /// error from [`Binding::parse`].
    pub fn load_dir(dir: &Path) -> Result<Self, BindingError> {
        Self::load_file(&dir.join(BINDING_FILE_NAME))
    }

    /// Load and validate a binding from an explicit file path.
    ///
    /// # Errors
    /// Returns [`BindingError::Read`] when the file cannot be read, plus any
    /// error from [`Binding::parse`].
    pub fn load_file(path: &Path) -> Result<Self, BindingError> {
        let src = std::fs::read_to_string(path).map_err(|source| BindingError::Read {
            path: path.to_path_buf(),
            source,
        })?;
        Self::parse(&src)
    }

    fn validate(&self) -> Result<(), BindingError> {
        if self.commands.is_empty() {
            return Err(BindingError::NoCommands);
        }
        for (index, command) in self.commands.iter().enumerate() {
            if command.trim().is_empty() {
                return Err(BindingError::BlankCommand { index });
            }
        }
        Ok(())
    }
}

/// Why a binding file could not be resolved into a valid [`Binding`].
#[derive(Debug)]
pub enum BindingError {
    /// The binding file could not be read from disk.
    Read {
        /// The path that failed to read.
        path: PathBuf,
        /// The underlying I/O error.
        source: std::io::Error,
    },
    /// The TOML was malformed or contained unexpected fields.
    Parse(toml::de::Error),
    /// `commands` was present but empty.
    NoCommands,
    /// A command entry was empty or whitespace-only.
    BlankCommand {
        /// The zero-based index of the blank command.
        index: usize,
    },
}

impl fmt::Display for BindingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BindingError::Read { path, source } => {
                write!(f, "cannot read binding {}: {source}", path.display())
            }
            BindingError::Parse(source) => write!(f, "invalid binding: {source}"),
            BindingError::NoCommands => write!(f, "binding declares no commands"),
            BindingError::BlankCommand { index } => {
                write!(f, "binding command {index} is blank")
            }
        }
    }
}

impl std::error::Error for BindingError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            BindingError::Read { source, .. } => Some(source),
            BindingError::Parse(source) => Some(source),
            BindingError::NoCommands | BindingError::BlankCommand { .. } => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Binding, BindingError};

    #[test]
    fn single_command_binding_when_parsed_resolves() {
        let binding = Binding::parse("commands = [\"cargo test --features driftwatch\"]")
            .expect("valid binding parses");
        assert_eq!(binding.commands, vec!["cargo test --features driftwatch"]);
    }

    #[test]
    fn multi_language_binding_when_parsed_keeps_command_order() {
        let binding = Binding::parse("commands = [\"cargo test\", \"dotnet test App\"]")
            .expect("valid binding parses");
        assert_eq!(binding.commands, vec!["cargo test", "dotnet test App"]);
    }

    #[test]
    fn empty_commands_when_parsed_is_no_commands() {
        let err = Binding::parse("commands = []").expect_err("empty commands rejected");
        assert!(matches!(err, BindingError::NoCommands));
    }

    #[test]
    fn blank_command_when_parsed_is_blank_command() {
        let err =
            Binding::parse("commands = [\"cargo test\", \"   \"]").expect_err("blank rejected");
        assert!(matches!(err, BindingError::BlankCommand { index: 1 }));
    }

    #[test]
    fn unknown_field_when_parsed_is_parse_error() {
        let err = Binding::parse("commands = [\"cargo test\"]\noutputs = [\"x\"]")
            .expect_err("unknown field rejected");
        assert!(matches!(err, BindingError::Parse(_)));
    }

    #[test]
    fn missing_commands_when_parsed_is_parse_error() {
        let err = Binding::parse("").expect_err("missing commands rejected");
        assert!(matches!(err, BindingError::Parse(_)));
    }
}
