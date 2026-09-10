use crate::accounts::{Store, atomic_write, private_dir};
use crate::proxy::Connection;
use anyhow::{Context, Result, bail, ensure};
use std::{
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
            "CODEX_SWITCHER_CODEX musi wskazywać plik bezwzględną ścieżką."
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
        .context("Nie znaleziono Codex CLI. Ustaw CODEX_SWITCHER_CODEX na ścieżkę do codex.")
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
pub fn login(store: &Store, cancel: Arc<AtomicBool>) -> Result<crate::accounts::Snapshot> {
    private_dir(&store.root)?;
    let home = tempfile::Builder::new()
        .prefix("login-")
        .tempdir_in(&store.root)?;
    private_dir(home.path())?;
    let mut child = LoginChild(
        Command::new(codex_binary()?)
            .process_group(0)
            .env("CODEX_HOME", home.path())
            .env_remove("OPENAI_API_KEY")
            .env_remove("CODEX_API_KEY")
            .env_remove("CODEX_ACCESS_TOKEN")
            .args(["-c", "cli_auth_credentials_store=\"file\"", "login"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .context("Nie udało się rozpocząć logowania.")?,
    );
    let start = Instant::now();
    loop {
        if cancel.load(Ordering::Relaxed) {
            bail!("Logowanie anulowane.");
        }
        if start.elapsed() > Duration::from_secs(300) {
            bail!("Upłynął czas logowania. Spróbuj ponownie.");
        }
        if let Some(status) = child.0.try_wait()? {
            ensure!(
                status.success(),
                "Logowanie nie powiodło się. Sprawdź, czy inny proces logowania nie używa portu 1455."
            );
            return store.import_from(&home.path().join("auth.json"));
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
            .context("Najpierw uruchom aplikację Codex Sub Switcher.")?,
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
            .context("Proxy jest wyłączone. Uruchom aplikację switchera.")?;
        ensure!(
            response.status().is_success(),
            "Uruchom ponownie aplikację switchera."
        );
        let value: serde_json::Value = response.json().await?;
        ensure!(
            value["service"] == "codex-sub-switcher",
            "Pod podanym portem nie działa switcher."
        );
        anyhow::Ok(())
    })?;
    drop(runtime);
    ensure!(
        store.active_id()?.is_some(),
        "Najpierw wybierz konto w aplikacji."
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
