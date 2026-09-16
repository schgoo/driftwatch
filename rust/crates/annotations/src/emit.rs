//! The feature-gated live auto-emit glue (OpenTelemetry `SimpleSpanProcessor` model).
//!
//! `runtime` cannot depend on `artifact` (the dependency points the other way),
//! so it exposes a root-close sink callback ([`runtime::set_sink`]) and this
//! glue — compiled only under the `driftwatch` feature — registers a sink that
//! renders each completed span tree to one CTSC OTLP JSONL line and **appends**
//! it to `trace.otlp.jsonl` in the resolved outdir. A process-global
//! `Mutex<File>` serializes concurrent appends: each thread renders a complete
//! line first, then performs a single locked `write_all`, so lines from
//! parallel test threads never interleave.
//!
//! There is no process-exit hook and no `unsafe`: the file stays current because
//! every root close writes, so nothing needs flushing at exit.

use std::env;
use std::fs::{self, File, OpenOptions};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock, PoisonError};

/// The resolved emit state, built once on the first root-close export.
struct EmitState {
    writer: Mutex<File>,
    resource: artifact::Resource,
}

/// Process-global emit state. `None` means "misconfigured" (e.g. a missing
/// `[target] name`); the one-time warning already fired during init, so
/// [`emit_capture`] then returns silently.
static STATE: OnceLock<Option<EmitState>> = OnceLock::new();

/// Register the Driftwatch trace emitter with the runtime.
///
/// Call once, early in a `cargo test --features driftwatch` run. It installs the
/// root-close sink so that whenever a thread's root span closes, the completed
/// span tree is appended as one OTLP JSONL line to `trace.otlp.jsonl` in the
/// resolved outdir. Idempotent — the runtime keeps the first registration.
pub fn install() {
    runtime::set_sink(emit_capture);
}

/// The root-close sink: render the span tree to one JSONL line and append it
/// under the global writer lock. Never panics (it runs inside `Drop`).
fn emit_capture(spans: &[runtime::Span]) {
    let Some(state) = STATE.get_or_init(init_state) else {
        return;
    };
    let capture = artifact::TraceCapture {
        resource: state.resource.clone(),
        spans: spans.to_vec(),
    };
    let line = capture.to_otlp(artifact::OtlpFormat::Jsonl);
    append_line(&state.writer, &line);
}

/// Append one complete pre-rendered line under the writer lock.
///
/// This runs from `SpanGuard::drop`, so it must never panic: a panic in `Drop`
/// during unwinding aborts the whole process (double-panic), which is a strictly
/// worse failure than a lost trace line. A poisoned lock only means another
/// thread panicked mid-emit; the underlying writer stays valid, so recovering
/// with `PoisonError::into_inner` and appending the next complete line is the
/// safe termination behavior at this Drop boundary. The original panic is
/// already surfaced by the panicking thread's own unwind.
///
/// Taking the whole line as one locked `write_all` serializes concurrent
/// threads: no partial line can interleave with another thread's line.
fn append_line<W: std::io::Write>(writer: &Mutex<W>, line: &str) {
    let mut w = writer.lock().unwrap_or_else(PoisonError::into_inner);
    // Best-effort append: a full disk / closed handle must not panic in Drop, so
    // the `io::Result` is intentionally dropped rather than unwrapped.
    let _ = w.write_all(line.as_bytes());
}

/// Resolve config, build the resource, and open the append target once. Returns
/// `None` (with a one-time warning) when the capture is misconfigured or the
/// output file cannot be opened, so emission is skipped rather than panicking.
fn init_state() -> Option<EmitState> {
    let dir = find_config_dir();
    let config = match artifact::CaptureConfig::load_dir(&dir) {
        Ok(config) => config,
        Err(err) => {
            eprintln!("driftwatch: {err}; using default config");
            artifact::CaptureConfig::default()
        }
    }
    .with_outdir_override(artifact::CaptureConfig::outdir_env_override());

    let Some(target_name) = config.target_name.clone() else {
        eprintln!(
            "driftwatch: [target] name is required to emit a capture; skipping trace emission"
        );
        return None;
    };

    // `CARGO_PKG_VERSION` here is the driftwatch tool version; the CTSC
    // `conformance.version` resource attribute is injected separately by
    // `artifact::Resource`.
    let resource =
        artifact::Resource::new("driftwatch", env!("CARGO_PKG_VERSION"), target_name, "rust");

    if let Err(err) = fs::create_dir_all(&config.outdir) {
        eprintln!(
            "driftwatch: cannot create outdir {}: {err}; skipping trace emission",
            config.outdir.display()
        );
        return None;
    }
    let path = config.outdir.join("trace.otlp.jsonl");
    if config.clean {
        // `clean` runs exactly once, here in the OnceLock init.
        let _ = fs::remove_file(&path);
    }
    match OpenOptions::new().create(true).append(true).open(&path) {
        Ok(file) => Some(EmitState {
            writer: Mutex::new(file),
            resource,
        }),
        Err(err) => {
            eprintln!(
                "driftwatch: cannot open {}: {err}; skipping trace emission",
                path.display()
            );
            None
        }
    }
}

/// Walk up from the current directory to the nearest `driftwatch.toml` (like
/// Cargo finds `Cargo.toml`), returning the directory that holds it. Falls back
/// to the starting directory when no config is found on any ancestor.
fn find_config_dir() -> PathBuf {
    let start = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let mut dir = start.clone();
    loop {
        if dir.join(artifact::CONFIG_FILE_NAME).exists() {
            return dir;
        }
        let Some(parent) = dir.parent().map(Path::to_path_buf) else {
            return start;
        };
        dir = parent;
    }
}

#[cfg(test)]
mod tests {
    use super::append_line;
    use std::sync::Mutex;

    /// Regression guard for the poisoned-writer bug: a poisoned lock must still
    /// be recovered and written to (via `PoisonError::into_inner`), not silently
    /// skipped as the old `if let Ok(..)` guard did. Fails (empty buffer) against
    /// the silent-skip version; passes with the recovery in `append_line`.
    #[test]
    fn poisoned_writer_still_appends() {
        let buf = Mutex::new(Vec::<u8>::new());

        // Poison the mutex by panicking while holding the lock.
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _g = buf.lock().unwrap();
            panic!("poison");
        }));
        assert!(
            buf.is_poisoned(),
            "expected the writer mutex to be poisoned"
        );

        append_line(&buf, "hello\n");

        let got = buf
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        assert_eq!(&*got, b"hello\n");
    }
}
