//! File-resolution tests for [`extract::Binding`]: loading `driftwatch.toml`
//! from a directory and surfacing read failures.

use std::path::PathBuf;

use extract::{BINDING_FILE_NAME, Binding, BindingError};

fn scratch_dir(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create scratch dir");
    dir
}

#[test]
fn binding_file_in_dir_when_loaded_resolves() {
    let dir = scratch_dir("resolves");
    std::fs::write(
        dir.join(BINDING_FILE_NAME),
        "commands = [\"cargo test --features driftwatch\"]\n",
    )
    .expect("write binding");

    let binding = Binding::load_dir(&dir).expect("binding resolves");
    assert_eq!(binding.commands, vec!["cargo test --features driftwatch"]);
}

#[test]
fn absent_binding_file_when_loaded_is_read_error() {
    let dir = scratch_dir("absent");
    let err = Binding::load_dir(&dir).expect_err("missing binding fails");
    assert!(matches!(err, BindingError::Read { .. }));
}
