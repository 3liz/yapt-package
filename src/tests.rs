//
// Tests
//
use std::sync::Once;
use std::{env, path};

static INIT: Once = Once::new();

pub(crate) fn setup() {
    INIT.call_once(|| {
        env_logger::init();
    });
}

pub(crate) fn rootdir() -> path::PathBuf {
    path::Path::new(&env::var("CARGO_MANIFEST_DIR").unwrap()).join("tests")
}

pub(crate) fn fixtures() -> path::PathBuf {
    rootdir().join("fixtures")
}
