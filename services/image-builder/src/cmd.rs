//! Running the build tools: fixed programs, argument vectors, never a shell
//! string assembled from input. Every invocation is logged with its step.

use std::path::Path;
use std::process::{Command, Stdio};

use ocinye_image_contracts::build::ImageBuildError;

/// Log one line to stderr (the build record keeps the structured version).
pub fn log(step: &str, msg: &str) {
    eprintln!("[{step}] {msg}");
}

fn describe(program: &str, args: &[&str]) -> String {
    let mut s = program.to_owned();
    for a in args {
        s.push(' ');
        s.push_str(a);
    }
    s
}

/// Run to completion, inheriting stderr; fail with `on_fail`.
pub fn run(
    step: &str,
    program: &str,
    args: &[&str],
    on_fail: impl FnOnce() -> ImageBuildError,
) -> Result<(), ImageBuildError> {
    log(step, &describe(program, args));
    let st = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .status();
    match st {
        Ok(s) if s.success() => Ok(()),
        _ => Err(on_fail()),
    }
}

/// Run and capture stdout.
pub fn output(
    step: &str,
    program: &str,
    args: &[&str],
    on_fail: impl FnOnce() -> ImageBuildError,
) -> Result<Vec<u8>, ImageBuildError> {
    log(step, &describe(program, args));
    match Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stderr(Stdio::inherit())
        .output()
    {
        Ok(o) if o.status.success() => Ok(o.stdout),
        _ => Err(on_fail()),
    }
}

/// SHA-256 and size of a file.
pub fn sha256_file(p: &Path) -> std::io::Result<(String, u64)> {
    use sha2::Digest as _;
    use std::io::Read as _;
    let mut f = std::fs::File::open(p)?;
    let mut h = sha2::Sha256::new();
    let mut buf = vec![0u8; 1 << 20];
    let mut n = 0u64;
    loop {
        let r = f.read(&mut buf)?;
        if r == 0 {
            break;
        }
        n += r as u64;
        h.update(&buf[..r]);
    }
    Ok((hex::encode(h.finalize()), n))
}

pub fn p(path: &Path) -> &str {
    path.to_str().unwrap_or_default()
}
