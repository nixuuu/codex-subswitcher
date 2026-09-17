use crate::accounts::{Store, atomic_write, private_dir};
use crate::proxy::Connection;
use anyhow::{Context, Result, bail, ensure};
use std::{
    io::Read,
    os::unix::{fs::PermissionsExt, process::CommandExt},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

pub fn codex_binary() -> Result<PathBuf> {
    if let Some(path) = std::env::var_os("CODEX_SWITCHER_CODEX") {
        let path = PathBuf::from(path);
        ensure!(
            path.is_absolute() && path.is_file(),
            "CODEX_SWITCHER_CODEX must be an absolute path to a file."
        );
        return Ok(native_if_packaged(path));
    }
    let mut dirs: Vec<PathBuf> =
        std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()).collect();
    if let Some(home) = std::env::var_os("HOME") {
        dirs.extend([
            PathBuf::from(&home).join(".bun/bin"),
            PathBuf::from(&home).join(".local/bin"),
        ]);
    }
    dirs.extend([
        PathBuf::from("/opt/homebrew/bin"),
        PathBuf::from("/usr/local/bin"),
    ]);
    dirs.into_iter()
        .map(|dir| dir.join("codex"))
        .find(|p| p.is_file())
        .map(native_if_packaged)
        .context("Codex CLI not found. Set CODEX_SWITCHER_CODEX to the path to codex.")
}
// The npm shim needs Node on PATH. Finder-launched apps often do not have it;
// use the native executable in the same official package when available.
fn native_if_packaged(path: PathBuf) -> PathBuf {
    let Ok(real) = std::fs::canonicalize(&path) else {
        return path;
    };
    if real.file_name().is_none_or(|n| n != "codex.js") {
        return path;
    }
    let Some(package) = real.parent().and_then(Path::parent) else {
        return path;
    };
    let (name, triple) = if cfg!(target_arch = "aarch64") {
        ("codex-darwin-arm64", "aarch64-apple-darwin")
    } else {
        ("codex-darwin-x64", "x86_64-apple-darwin")
    };
    let locations = [
        package.join("vendor"),
        package
            .parent()
            .unwrap_or(package)
            .join(name)
            .join("vendor"),
        package
            .join("node_modules/@openai")
            .join(name)
            .join("vendor"),
    ];
    locations
        .into_iter()
        .map(|dir| dir.join(triple).join("bin/codex"))
        .find(|p| p.is_file())
        .unwrap_or(path)
}
struct LoginChild(Child);
impl Drop for LoginChild {
    fn drop(&mut self) {
        // The npm launcher can own a native child. Terminate the group created
        // below so cancellation also releases the OAuth callback listener.
        if matches!(self.0.try_wait(), Ok(None)) {
            unsafe {
                libc::killpg(self.0.id() as i32, libc::SIGKILL);
            }
        }
        let _ = self.0.wait();
    }
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum LoginMode {
    Browser,
    Device,
}

#[derive(Clone)]
pub struct DeviceLogin {
    code: String,
}

impl DeviceLogin {
    pub fn url(&self) -> &'static str {
        "https://auth.openai.com/codex/device"
    }

    pub fn code(&self) -> &str {
        &self.code
    }

    pub fn copy_text(&self) -> String {
        format!("{}\nOne-time code: {}", self.url(), self.code)
    }

    // Codex prints a human-readable prompt, including ANSI color sequences.
    // Only accept the official device URL and the complete, labeled code line.
    fn parse(output: &str) -> Option<Self> {
        let mut plain = String::new();
        let mut chars = output.chars();
        while let Some(ch) = chars.next() {
            if ch == '\u{1b}' {
                if chars.next()? != '[' {
                    return None;
                }
                for ch in chars.by_ref() {
                    if ('@'..='~').contains(&ch) {
                        break;
                    }
                }
            } else {
                plain.push(ch);
            }
        }
        if !plain
            .lines()
            .any(|line| line.trim() == "https://auth.openai.com/codex/device")
        {
            return None;
        }
        let mut lines = plain.split_inclusive('\n');
        lines.find(|line| line.contains("Enter this one-time code"))?;
        let line = lines.find(|line| !line.trim().is_empty())?;
        if !line.ends_with('\n') {
            return None;
        }
        let code = line.trim();
        if !(6..=32).contains(&code.len())
            || !code
                .bytes()
                .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'-')
        {
            return None;
        }
        Some(Self { code: code.into() })
    }
}

pub fn login(
    store: &Store,
    cancel: Arc<AtomicBool>,
    mode: LoginMode,
    progress: std::sync::mpsc::Sender<DeviceLogin>,
) -> Result<crate::accounts::Snapshot> {
    private_dir(&store.root)?;
    let home = tempfile::Builder::new()
        .prefix("login-")
        .tempdir_in(&store.root)?;
    private_dir(home.path())?;
    // Keep CLI output private and temporary; never publish arbitrary log content.
    let output = tempfile::NamedTempFile::new_in(home.path())?;
    let mut command = Command::new(codex_binary()?);
    command.args(["-c", "cli_auth_credentials_store=\"file\"", "login"]);
    if mode == LoginMode::Device {
        command.arg("--device-auth");
    }
    let mut child = LoginChild(
        command
            .process_group(0)
            .env("CODEX_HOME", home.path())
            .env_remove("OPENAI_API_KEY")
            .env_remove("CODEX_API_KEY")
            .env_remove("CODEX_ACCESS_TOKEN")
            .stdin(Stdio::null())
            .stdout(Stdio::from(output.reopen()?))
            .stderr(Stdio::null())
            .spawn()
            .context("Could not start sign-in.")?,
    );
    let start = Instant::now();
    let timeout = match mode {
        LoginMode::Browser => Duration::from_secs(300),
        LoginMode::Device => Duration::from_secs(15 * 60),
    };
    let mut prompt_sent = false;
    loop {
        if cancel.load(Ordering::Relaxed) {
            bail!("Sign-in canceled.");
        }
        if start.elapsed() > timeout {
            bail!("Sign-in timed out. Try again.");
        }
        if let Some(status) = child.0.try_wait()? {
            ensure!(
                status.success(),
                "{}",
                match mode {
                    LoginMode::Browser =>
                        "Sign-in failed. Check whether another sign-in process is using port 1455.",
                    LoginMode::Device =>
                        "Device sign-in failed or expired. Enable device code login in ChatGPT security settings (or ask your workspace admin), check your connection, and try again with an up-to-date Codex CLI.",
                }
            );
            return store.import_from(&home.path().join("auth.json"));
        }
        if mode == LoginMode::Device && !prompt_sent {
            let mut text = String::new();
            output.reopen()?.take(64 * 1024).read_to_string(&mut text)?;
            if let Some(details) = DeviceLogin::parse(&text) {
                let _ = progress.send(details);
                prompt_sent = true;
            } else if start.elapsed() > Duration::from_secs(60) {
                bail!(
                    "Could not read the device sign-in link and code. Check your connection and update Codex CLI, then try again."
                );
            }
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}
pub fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}
pub fn install_launcher(store: &Store) -> Result<PathBuf> {
    let bin = store.root.join("bin");
    private_dir(&bin)?;
    let path = bin.join("codex-switch");
    let exe = std::env::current_exe()?;
    let text = format!(
        "#!/bin/sh\nexec /usr/bin/env CODEX_SWITCHER_HOME={} CODEX_HOME={} {} --cli \"$@\"\n",
        shell_quote(&store.root.to_string_lossy()),
        shell_quote(&store.codex_home.to_string_lossy()),
        shell_quote(&exe.to_string_lossy())
    );
    atomic_write(&path, text.as_bytes())?;
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700))?;
    Ok(path)
}
pub fn provider_args(port: u16, binary: &Path, connection: &Path) -> Vec<String> {
    let mut auth_args = toml_edit::Array::new();
    auth_args.push("--proxy-token");
    auth_args.push(connection.to_string_lossy().into_owned());
    [
        "model_provider=\"subscription_switcher\"".to_owned(),
        "model_providers.subscription_switcher.name=\"Subscription Switcher\"".into(),
        format!("model_providers.subscription_switcher.base_url=\"http://127.0.0.1:{port}/v1\""),
        format!(
            "model_providers.subscription_switcher.auth.command={}",
            toml_edit::Value::from(binary.to_string_lossy().into_owned())
        ),
        format!("model_providers.subscription_switcher.auth.args={auth_args}"),
        "model_providers.subscription_switcher.wire_api=\"responses\"".into(),
        "model_providers.subscription_switcher.requires_openai_auth=false".into(),
        "model_providers.subscription_switcher.supports_websockets=false".into(),
        "features.responses_websockets=false".into(),
        "features.responses_websockets_v2=false".into(),
        "features.enable_request_compression=false".into(),
    ]
    .into_iter()
    .flat_map(|v| ["-c".into(), v])
    .collect()
}
pub fn run_cli(store: &Store, args: impl Iterator<Item = std::ffi::OsString>) -> Result<()> {
    let connection: Connection = serde_json::from_slice(
        &std::fs::read(store.root.join("connection.json"))
            .context("Start Codex Sub Switcher first.")?,
    )?;
    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(async {
        let client = reqwest::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_secs(3))
            .build()?;
        let response = client
            .get(format!("http://127.0.0.1:{}/health", connection.port))
            .bearer_auth(&connection.token)
            .send()
            .await
            .context("The proxy is offline. Start the switcher app.")?;
        ensure!(response.status().is_success(), "Restart the switcher app.");
        let value: serde_json::Value = response.json().await?;
        ensure!(
            value["service"] == "codex-sub-switcher",
            "No switcher is running on the specified port."
        );
        anyhow::Ok(())
    })?;
    drop(runtime);
    ensure!(
        store.active_id()?.is_some(),
        "Select an account in the app first."
    );
    let error = Command::new(codex_binary()?)
        .args(provider_args(
            connection.port,
            &std::env::current_exe()?,
            &store.root.join("connection.json"),
        ))
        .args(args)
        .exec();
    Err(error.into())
}

#[cfg(test)]
mod login_tests {
    use super::DeviceLogin;

    const PROMPT: &str = "Welcome to Codex [v0.114.0]\n\
        1. Open this link in your browser and sign in to your account\n\
           \x1b[94mhttps://auth.openai.com/codex/device\x1b[0m\n\
        2. Enter this one-time code \x1b[90m(expires in 15 minutes)\x1b[0m\n\
           \x1b[94mABCD-12345\x1b[0m\n\
        Continue only if you started this login in Codex.\n";

    #[test]
    fn extracts_colored_device_prompt_and_copies_only_link_and_code() {
        let details = DeviceLogin::parse(PROMPT).unwrap();
        assert_eq!(details.code(), "ABCD-12345");
        assert_eq!(
            details.copy_text(),
            "https://auth.openai.com/codex/device\nOne-time code: ABCD-12345"
        );
    }

    #[test]
    fn waits_for_the_complete_code_line() {
        let end = PROMPT.find("ABCD-12345").unwrap();
        for offset in 0.."ABCD-12345\x1b[0m".len() {
            assert!(DeviceLogin::parse(&PROMPT[..end + offset]).is_none());
        }
        assert!(DeviceLogin::parse(&PROMPT[..end + "ABCD-12345\x1b[0m\n".len()]).is_some());
    }

    #[test]
    fn supports_plain_output_and_crlf() {
        let plain = "https://auth.openai.com/codex/device\r\n\r\n2. Enter this one-time code\r\n\r\nABCD-12345\r\n";
        assert_eq!(DeviceLogin::parse(plain).unwrap().code(), "ABCD-12345");
    }

    #[test]
    fn rejects_other_urls_and_error_output() {
        assert!(DeviceLogin::parse(&PROMPT.replace("auth.openai.com", "example.com")).is_none());
        assert!(
            DeviceLogin::parse(&PROMPT.replace("ABCD-12345", "Error: access denied")).is_none()
        );
        assert!(
            DeviceLogin::parse("https://auth.openai.com/oauth/authorize?state=secret\n").is_none()
        );
        assert!(DeviceLogin::parse("Device sign-in failed\n").is_none());
    }
}
