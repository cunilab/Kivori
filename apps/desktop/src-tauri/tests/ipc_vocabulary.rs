//! The closed token lists shared with the webview. `src/lib/ipc/__tests__/vocabulary.json` is the
//! contract: this test fails when Rust drifts from it, and the TS test fails when
//! `lib/ipc/types.ts` drifts from it. Regenerate it with `UPDATE_VOCABULARY=1 cargo test`.

use std::path::Path;

#[test]
fn rust_tokens_match_the_shared_vocabulary_file() {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../src/lib/ipc/__tests__/vocabulary.json");
    let actual = kivori_desktop::ipc::dto::vocabulary_json();
    if std::env::var_os("UPDATE_VOCABULARY").is_some() {
        let mut text = serde_json::to_string_pretty(&actual).unwrap();
        text.push('\n');
        std::fs::write(&path, text).unwrap();
    }
    let expected: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).expect("vocabulary.json")).unwrap();
    assert_eq!(
        actual, expected,
        "token drift: regenerate with UPDATE_VOCABULARY=1"
    );
}
