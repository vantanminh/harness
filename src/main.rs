fn finish(result: harness::Result<()>) {
    if let Err(err) = result {
        let message = harness::error::redact_sensitive(&err.message);
        if std::env::var_os("HARNESS_JSON_ERRORS").is_some() {
            eprintln!(
                "{}",
                serde_json::json!({
                    "ok": false,
                    "code": err.code,
                    "message": message,
                    "exitCode": err.exit_code(),
                })
            );
        } else {
            eprintln!("{message}");
        }
        std::process::exit(err.exit_code());
    }
}

#[cfg(not(windows))]
fn main() {
    finish(harness::run());
}

// The Windows process entry stack is smaller than the Unix default.  The
// clap command tree is intentionally broad, so run the parser/dispatcher on a
// larger stack to keep `harness --version`, help, and spawned CLI calls from
// overflowing before they can do any work.
#[cfg(windows)]
fn main() {
    let result = std::thread::Builder::new()
        .name("harness-main".into())
        .stack_size(8 * 1024 * 1024)
        .spawn(harness::run)
        .map_err(|err| harness::Error::new(format!("failed to start CLI thread: {err}")))
        .and_then(|thread| {
            thread
                .join()
                .map_err(|_| harness::Error::new("CLI thread panicked"))?
        });
    finish(result);
}
