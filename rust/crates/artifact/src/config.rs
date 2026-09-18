//! The Driftwatch capture config (`driftwatch.toml`).
//!
//! A repo-root `driftwatch.toml` tells the emitter *where* and *how* to write a
//! capture and what target identity to stamp on it. It is intentionally tiny —
//! every key has a default, so a project with no config file still captures.
//! The `DRIFTWATCH_OUT_DIR` environment variable overrides the output directory
//! for one run without editing the file (precedence: env var > `outdir` key >
//! built-in default).

use std::path::{Path, PathBuf};
use std::{env, fmt, fs, io};

use serde::Deserialize;

use crate::capture::OtlpFormat;

/// The canonical config file name resolved inside a target directory.
pub const CONFIG_FILE_NAME: &str = "driftwatch.toml";

/// The environment variable that overrides [`CaptureConfig::outdir`].
pub const OUTDIR_ENV: &str = "DRIFTWATCH_OUT_DIR";

/// The environment variable that overrides the resolved registry `version`
/// (see [`CaptureConfig::resolve_registry_version`]).
pub const VERSION_ENV: &str = "DRIFTWATCH_VERSION";

/// The built-in output directory used when neither the env var nor the config
/// file specifies one.
const DEFAULT_OUTDIR: &str = "target/driftwatch";

/// The fallback registry `version` stamped when neither [`VERSION_ENV`] nor the
/// `[registry] version` config key supplies one.
const DEFAULT_VERSION: &str = "0.0.0-dev";

/// The resolved capture configuration: where to write artifacts, how to
/// serialize the trace, and the target identity to stamp on the capture.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaptureConfig {
    /// The directory the emitter writes the registry + trace artifacts into.
    /// Defaults to `target/driftwatch/`; overridden by [`OUTDIR_ENV`] via
    /// [`CaptureConfig::with_outdir_override`].
    pub outdir: PathBuf,
    /// The language-neutral `conformance.target.name` identity a compare pairs
    /// on ([`Resource`](crate::Resource)). `None` means "unset in config"; the
    /// emitter falls back to its own `CARGO_PKG_NAME`, which the caller knows
    /// but this crate cannot. Set it explicitly when a Rust and a non-Rust run
    /// of one component must declare the *same* target so they diff
    /// cross-language.
    pub target_name: Option<String>,
    /// The registry `version` (registry.md §2) from the `[registry] version`
    /// config key. `None` means "unset"; [`CaptureConfig::resolve_registry_version`]
    /// then falls back through [`VERSION_ENV`] to [`DEFAULT_VERSION`].
    pub registry_version: Option<String>,
    /// The trace serialization format. Defaults to [`OtlpFormat::Jsonl`].
    pub format: OtlpFormat,
    /// Whether to wipe [`CaptureConfig::outdir`] before a run so a prior run's
    /// artifacts cannot mix into this capture. Defaults to `false`.
    pub clean: bool,
}

impl Default for CaptureConfig {
    fn default() -> Self {
        CaptureConfig {
            outdir: PathBuf::from(DEFAULT_OUTDIR),
            target_name: None,
            registry_version: None,
            format: OtlpFormat::Jsonl,
            clean: false,
        }
    }
}

impl CaptureConfig {
    /// Parse a capture config from TOML source. Every key is optional; a missing
    /// key takes its default.
    ///
    /// # Errors
    /// Returns [`ConfigError::Parse`] on malformed TOML, an unknown key, or an
    /// unrecognized `format` value.
    pub fn parse(toml_src: &str) -> Result<Self, ConfigError> {
        let raw: RawConfig = toml::from_str(toml_src).map_err(ConfigError::Parse)?;
        Ok(Self::from_raw(raw))
    }

    /// Resolve the `driftwatch.toml` config inside `dir`, returning defaults
    /// when the file is absent (a project without config still captures).
    ///
    /// # Errors
    /// Returns [`ConfigError::Read`] when the file exists but cannot be read,
    /// plus any error from [`CaptureConfig::parse`].
    pub fn load_dir(dir: &Path) -> Result<Self, ConfigError> {
        Self::load_file(&dir.join(CONFIG_FILE_NAME))
    }

    /// Load a config from an explicit file path, returning defaults when the
    /// file does not exist.
    ///
    /// # Errors
    /// Returns [`ConfigError::Read`] when the file exists but cannot be read,
    /// plus any error from [`CaptureConfig::parse`].
    pub fn load_file(path: &Path) -> Result<Self, ConfigError> {
        match fs::read_to_string(path) {
            Ok(src) => Self::parse(&src),
            Err(source) if source.kind() == io::ErrorKind::NotFound => Ok(Self::default()),
            Err(source) => Err(ConfigError::Read {
                path: path.to_path_buf(),
                source,
            }),
        }
    }

    /// The [`OUTDIR_ENV`] override, if the variable is set. Read once and passed
    /// to [`CaptureConfig::with_outdir_override`] so the env read stays out of
    /// the pure parse/resolve path.
    #[must_use]
    pub fn outdir_env_override() -> Option<PathBuf> {
        env::var_os(OUTDIR_ENV).map(PathBuf::from)
    }

    /// The [`VERSION_ENV`] override, if the variable is set and non-empty. Read
    /// here rather than in the pure `from_raw` path (mirroring
    /// [`CaptureConfig::outdir_env_override`]) so parsing stays deterministic.
    #[must_use]
    pub fn version_env_override() -> Option<String> {
        env::var(VERSION_ENV).ok().filter(|v| !v.is_empty())
    }

    /// Resolve the registry `version` to stamp on a derived registry document,
    /// with precedence [`VERSION_ENV`] > `[registry] version` > [`DEFAULT_VERSION`].
    #[must_use]
    pub fn resolve_registry_version(&self) -> String {
        resolve_version(Self::version_env_override(), self.registry_version.clone())
    }

    /// Apply an output-directory override (typically
    /// [`CaptureConfig::outdir_env_override`]), giving it precedence over the
    /// config file. A `None` override leaves [`CaptureConfig::outdir`] untouched.
    #[must_use]
    pub fn with_outdir_override(mut self, override_dir: Option<PathBuf>) -> Self {
        if let Some(dir) = override_dir {
            self.outdir = dir;
        }
        self
    }

    fn from_raw(raw: RawConfig) -> Self {
        let defaults = Self::default();
        CaptureConfig {
            outdir: raw.outdir.map_or(defaults.outdir, PathBuf::from),
            target_name: raw.target.and_then(|target| target.name),
            registry_version: raw.registry.and_then(|registry| registry.version),
            format: raw.format.map_or(defaults.format, RawFormat::into_otlp),
            clean: raw.clean.unwrap_or(defaults.clean),
        }
    }
}

/// Apply the registry-version precedence: env override > config key > default.
/// Kept as a free function (env-free) so the precedence is unit-testable without
/// mutating the process environment (which is `unsafe` under edition 2024).
fn resolve_version(env_override: Option<String>, config_version: Option<String>) -> String {
    env_override
        .or(config_version)
        .unwrap_or_else(|| DEFAULT_VERSION.to_string())
}

/// The raw TOML shape, before defaults are applied.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawConfig {
    outdir: Option<String>,
    format: Option<RawFormat>,
    clean: Option<bool>,
    target: Option<RawTarget>,
    registry: Option<RawRegistry>,
}

/// The `[target]` table.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawTarget {
    name: Option<String>,
}

/// The `[registry]` table.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawRegistry {
    version: Option<String>,
}

/// The `format` key's accepted values, mapped onto [`OtlpFormat`].
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
enum RawFormat {
    Json,
    Jsonl,
}

impl RawFormat {
    fn into_otlp(self) -> OtlpFormat {
        match self {
            RawFormat::Json => OtlpFormat::Json,
            RawFormat::Jsonl => OtlpFormat::Jsonl,
        }
    }
}

/// Why a `driftwatch.toml` could not be resolved into a [`CaptureConfig`].
#[derive(Debug)]
pub enum ConfigError {
    /// The config file exists but could not be read from disk.
    Read {
        /// The path that failed to read.
        path: PathBuf,
        /// The underlying I/O error.
        source: io::Error,
    },
    /// The TOML was malformed, held an unknown key, or an invalid value.
    Parse(toml::de::Error),
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigError::Read { path, source } => {
                write!(f, "cannot read config {}: {source}", path.display())
            }
            ConfigError::Parse(source) => write!(f, "invalid config: {source}"),
        }
    }
}

impl std::error::Error for ConfigError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ConfigError::Read { source, .. } => Some(source),
            ConfigError::Parse(source) => Some(source),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{CaptureConfig, ConfigError, OtlpFormat};

    #[test]
    fn empty_source_when_parsed_is_all_defaults() {
        let config = CaptureConfig::parse("").expect("empty config parses");
        assert_eq!(config, CaptureConfig::default());
    }

    #[test]
    fn outdir_when_set_overrides_default() {
        let config = CaptureConfig::parse("outdir = \"artifacts/dw\"").expect("valid config");
        assert_eq!(config.outdir, PathBuf::from("artifacts/dw"));
    }

    #[test]
    fn format_when_json_maps_to_json() {
        let config = CaptureConfig::parse("format = \"json\"").expect("valid config");
        assert_eq!(config.format, OtlpFormat::Json);
    }

    #[test]
    fn format_when_jsonl_maps_to_jsonl() {
        let config = CaptureConfig::parse("format = \"jsonl\"").expect("valid config");
        assert_eq!(config.format, OtlpFormat::Jsonl);
    }

    #[test]
    fn target_name_when_set_is_carried() {
        let config =
            CaptureConfig::parse("[target]\nname = \"order-pricing\"").expect("valid config");
        assert_eq!(config.target_name.as_deref(), Some("order-pricing"));
    }

    #[test]
    fn clean_when_true_is_carried() {
        let config = CaptureConfig::parse("clean = true").expect("valid config");
        assert!(config.clean);
    }

    #[test]
    fn unknown_key_when_parsed_is_parse_error() {
        let err =
            CaptureConfig::parse("commands = [\"cargo test\"]").expect_err("unknown key rejected");
        assert!(matches!(err, ConfigError::Parse(_)));
    }

    #[test]
    fn unknown_target_key_when_parsed_is_parse_error() {
        let err = CaptureConfig::parse("[target]\nversion = \"1\"")
            .expect_err("unknown target key rejected");
        assert!(matches!(err, ConfigError::Parse(_)));
    }

    #[test]
    fn registry_version_when_set_is_carried() {
        let config = CaptureConfig::parse("[registry]\nversion = \"2.1.0\"").expect("valid config");
        assert_eq!(config.registry_version.as_deref(), Some("2.1.0"));
    }

    #[test]
    fn unknown_registry_key_when_parsed_is_parse_error() {
        let err = CaptureConfig::parse("[registry]\nid = \"x\"")
            .expect_err("unknown registry key rejected");
        assert!(matches!(err, ConfigError::Parse(_)));
    }

    #[test]
    fn registry_version_when_unset_resolves_to_default() {
        let version = super::resolve_version(None, None);
        assert_eq!(version, "0.0.0-dev");
    }

    #[test]
    fn registry_version_config_wins_over_default() {
        let version = super::resolve_version(None, Some("1.2.3".to_string()));
        assert_eq!(version, "1.2.3");
    }

    #[test]
    fn registry_version_env_wins_over_config() {
        let version = super::resolve_version(Some("9.9.9".to_string()), Some("1.2.3".to_string()));
        assert_eq!(version, "9.9.9");
    }

    #[test]
    fn invalid_format_when_parsed_is_parse_error() {
        let err = CaptureConfig::parse("format = \"yaml\"").expect_err("bad format rejected");
        assert!(matches!(err, ConfigError::Parse(_)));
    }

    #[test]
    fn outdir_override_when_present_wins_over_config() {
        let config = CaptureConfig::parse("outdir = \"from-file\"")
            .expect("valid config")
            .with_outdir_override(Some(PathBuf::from("from-env")));
        assert_eq!(config.outdir, PathBuf::from("from-env"));
    }

    #[test]
    fn outdir_override_when_absent_keeps_config() {
        let config = CaptureConfig::parse("outdir = \"from-file\"")
            .expect("valid config")
            .with_outdir_override(None);
        assert_eq!(config.outdir, PathBuf::from("from-file"));
    }
}
