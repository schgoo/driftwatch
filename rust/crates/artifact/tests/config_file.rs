//! Integration coverage for resolving `driftwatch.toml` from disk.

use std::fs;
use std::path::PathBuf;

use artifact::{CaptureConfig, OtlpFormat};

fn temp_dir(tag: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("config-{tag}"));
    fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

#[test]
fn load_dir_when_file_present_resolves_keys() {
    let dir = temp_dir("present");
    fs::write(
        dir.join("driftwatch.toml"),
        "outdir = \"out\"\nformat = \"json\"\nclean = true\n[target]\nname = \"widget\"\n",
    )
    .expect("write config");

    let config = CaptureConfig::load_dir(&dir).expect("config resolves");

    assert_eq!(config.outdir, PathBuf::from("out"));
    assert_eq!(config.format, OtlpFormat::Json);
    assert!(config.clean);
    assert_eq!(config.target_name.as_deref(), Some("widget"));
}

#[test]
fn load_dir_when_file_absent_is_defaults() {
    let dir = temp_dir("absent");
    let _ = fs::remove_file(dir.join("driftwatch.toml"));

    let config = CaptureConfig::load_dir(&dir).expect("absent file is not an error");

    assert_eq!(config, CaptureConfig::default());
}
