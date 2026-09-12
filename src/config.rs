//! Reversible, narrowly scoped config edits. Never serializes an OAuth token.
use crate::accounts::{Store, atomic_write};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};
use toml_edit::{DocumentMut, Item, Table, value};
const PROVIDER: &str = "subscription_switcher";
#[derive(Serialize, Deserialize)]
struct Patch {
    config: PathBuf,
    backup: PathBuf,
    provider: String,
}
fn parse(text: &str) -> Result<DocumentMut> {
    text.parse()
        .map_err(|_| anyhow::anyhow!("Invalid config.toml. The file was not changed."))
}
fn read(path: &Path) -> Result<String> {
    match fs::read_to_string(path) {
        Ok(v) => Ok(v),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(e) => Err(e.into()),
    }
}
fn provider(port: u16, binary: &Path, connection: &Path) -> Table {
    let mut t = Table::new();
    t["name"] = value("Subscription Switcher");
    t["base_url"] = value(format!("http://127.0.0.1:{port}/v1"));
    t["wire_api"] = value("responses");
    t["requires_openai_auth"] = value(false);
    t["supports_websockets"] = value(false);
    let mut auth = Table::new();
    auth["command"] = value(binary.to_string_lossy().into_owned());
    let mut args = toml_edit::Array::new();
    args.push("--proxy-token");
    args.push(connection.to_string_lossy().into_owned());
    auth["args"] = value(args);
    auth["timeout_ms"] = value(5000);
    auth["refresh_interval_ms"] = value(60000);
    t["auth"] = Item::Table(auth);
    t
}
pub fn enabled(store: &Store) -> bool {
    read(&store.codex_home.join("config.toml"))
        .ok()
        .and_then(|s| parse(&s).ok())
        .is_some_and(|d| d.get("model_provider").and_then(Item::as_str) == Some(PROVIDER))
}
pub fn enable(store: &Store, port: u16, binary: &Path) -> Result<()> {
    let _lock = store.lock()?;
    let path = store.codex_home.join("config.toml");
    ensure!(store.codex_home.is_dir(), "CODEX_HOME directory not found.");
    let original = read(&path)?;
    let mut doc = parse(&original)?;
    let record_path = store.root.join("config-patch.json");
    ensure!(
        !record_path.exists(),
        "A previous configuration change exists. Choose Restore config.toml first."
    );
    ensure!(
        doc.get("model_providers")
            .is_none_or(|p| p.get(PROVIDER).is_none()),
        "The name subscription_switcher is already in use in config.toml."
    );
    ensure!(
        doc.get("model_provider").and_then(Item::as_str) != Some(PROVIDER),
        "The configuration already selects subscription_switcher; restore the previous setting first."
    );
    let p = provider(port, binary, &store.root.join("connection.json"));
    let expected = p.to_string();
    if doc.get("model_providers").is_none() {
        doc["model_providers"] = Item::Table(Table::new());
    }
    ensure!(
        doc["model_providers"].is_table(),
        "model_providers uses an unsupported inline format. Nothing was changed."
    );
    doc["model_providers"][PROVIDER] = Item::Table(p);
    doc["model_provider"] = value(PROVIDER);
    let next = doc.to_string();
    let _: toml::Value = toml::from_str(&next).context("Could not prepare the configuration.")?;
    let backup = store.root.join(format!(
        "config-before-proxy-{}.toml",
        chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()
    ));
    atomic_write(&backup, original.as_bytes())?;
    atomic_write(
        &record_path,
        &serde_json::to_vec(&Patch {
            config: path.clone(),
            backup,
            provider: expected,
        })?,
    )?;
    ensure!(
        read(&path)? == original,
        "The configuration changed during the operation. Try restoring it before enabling again."
    );
    atomic_write(&path, next.as_bytes())
}
pub fn restore(store: &Store) -> Result<()> {
    let _lock = store.lock()?;
    let record_path = store.root.join("config-patch.json");
    let record: Patch = serde_json::from_slice(
        &fs::read(&record_path).context("No saved switcher configuration change to restore.")?,
    )?;
    ensure!(
        record.config == store.codex_home.join("config.toml"),
        "The configuration backup belongs to a different CODEX_HOME."
    );
    let current = read(&record.config)?;
    let mut doc = parse(&current)?;
    let backup = parse(&read(&record.backup)?)?;
    if let Some(p) = doc.get("model_providers").and_then(|p| p.get(PROVIDER)) {
        let actual: toml::Value =
            toml::from_str(&p.to_string()).context("Could not compare the proxy configuration.")?;
        let expected: toml::Value = toml::from_str(&record.provider)?;
        ensure!(
            actual == expected,
            "The proxy provider settings were changed manually. The configuration and its backup have been preserved."
        );
    }
    if doc.get("model_provider").and_then(Item::as_str) == Some(PROVIDER) {
        if let Some(old) = backup.get("model_provider") {
            doc["model_provider"] = old.clone();
        } else {
            doc.remove("model_provider");
        }
    }
    if let Some(providers) = doc.get_mut("model_providers").and_then(Item::as_table_mut) {
        providers.remove(PROVIDER);
        if providers.is_empty() && backup.get("model_providers").is_none() {
            doc.remove("model_providers");
        }
    }
    ensure!(
        read(&record.config)? == current,
        "The configuration changed during the operation. Try again."
    );
    atomic_write(&record.config, doc.to_string().as_bytes())?;
    fs::remove_file(record_path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::accounts::tests::store;
    #[test]
    fn patch_and_restore_preserve_comments_and_unrelated_later_edits() {
        let (_t, s) = store();
        let path = s.codex_home.join("config.toml");
        let original = "# keep my comment\nmodel = 'gpt-5.4'\nmodel_provider = 'openai' # original\n\n[projects.'/tmp/example']\ntrust_level = 'trusted'\n";
        atomic_write(&path, original.as_bytes()).unwrap();
        enable(
            &s,
            12345,
            Path::new("/Applications/Example App/bin/switcher"),
        )
        .unwrap();
        let patched = read(&path).unwrap();
        assert!(patched.contains("# keep my comment"));
        assert!(enabled(&s));
        let doc: toml::Value = toml::from_str(&patched).unwrap();
        assert_eq!(
            doc["model_providers"][PROVIDER]["auth"]["command"].as_str(),
            Some("/Applications/Example App/bin/switcher")
        );
        assert!(!patched.contains("Bearer"));
        assert!(!patched.contains("refresh_token"));
        atomic_write(
            &path,
            format!("{patched}\n[another]\nvalue = 42\n").as_bytes(),
        )
        .unwrap();
        restore(&s).unwrap();
        let restored = read(&path).unwrap();
        assert!(restored.contains("# keep my comment"));
        assert!(restored.contains("# original"));
        let doc: toml::Value = toml::from_str(&restored).unwrap();
        assert_eq!(doc["model_provider"].as_str(), Some("openai"));
        assert_eq!(doc["another"]["value"].as_integer(), Some(42));
        assert!(doc.get("model_providers").is_none());
    }
    #[test]
    fn restore_does_not_overwrite_manually_changed_provider() {
        let (_t, s) = store();
        enable(&s, 12345, Path::new("/switcher")).unwrap();
        let path = s.codex_home.join("config.toml");
        let mut doc = parse(&read(&path).unwrap()).unwrap();
        doc["model_providers"][PROVIDER]["base_url"] = value("https://example.test");
        atomic_write(&path, doc.to_string().as_bytes()).unwrap();
        let before = read(&path).unwrap();
        assert!(restore(&s).is_err());
        assert_eq!(read(&path).unwrap(), before);
    }
    #[test]
    fn existing_provider_and_malformed_config_are_not_overwritten() {
        let (_t, s) = store();
        let p = s.codex_home.join("config.toml");
        for text in [
            "[model_providers.subscription_switcher]\nname='Mine'\n",
            "not valid toml!",
        ] {
            atomic_write(&p, text.as_bytes()).unwrap();
            assert!(enable(&s, 12345, Path::new("/switcher")).is_err());
            assert_eq!(read(&p).unwrap(), text);
        }
    }
    #[test]
    fn empty_config_and_second_enable_restore_cleanly() {
        let (_t, s) = store();
        enable(&s, 12345, Path::new("/switcher")).unwrap();
        assert!(enable(&s, 12345, Path::new("/switcher")).is_err());
        restore(&s).unwrap();
        assert_eq!(read(&s.codex_home.join("config.toml")).unwrap().trim(), "");
    }
}

#[cfg(test)]
mod cli_tests {
    use super::*;
    use crate::accounts::tests::store;
    use axum::{Router, http::HeaderMap, response::IntoResponse, routing::post};
    use serde_json::json;
    use std::{os::unix::fs::PermissionsExt, sync::Arc, time::Duration};
    /// Optional because CI may not have Codex installed. It never uses real auth.
    #[tokio::test]
    #[ignore = "requires installed codex CLI; uses only a local mock and synthetic credentials"]
    async fn installed_codex_accepts_patched_config_and_command_auth() {
        let (_temp, s) = store();
        let helper = s.root.join("fake-token-helper");
        atomic_write(&helper, b"#!/bin/sh\nprintf '%s' 'synthetic-proxy-key'\n").unwrap();
        fs::set_permissions(&helper, fs::Permissions::from_mode(0o700)).unwrap();
        let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let count = calls.clone();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let router=Router::new().route("/v1/responses",post(move |headers:HeaderMap,axum::Json(body):axum::Json<serde_json::Value>|{
            let count=count.clone();async move {
                assert_eq!(headers["authorization"],"Bearer synthetic-proxy-key");assert!(body["input"].is_array());count.fetch_add(1,std::sync::atomic::Ordering::SeqCst);
                let item=json!({"id":"msg_1","type":"message","role":"assistant","status":"completed","content":[{"type":"output_text","text":"switcher-ok","annotations":[]}]});
                let events=[json!({"type":"response.created","response":{"id":"resp_test"}}),json!({"type":"response.output_item.added","output_index":0,"item":{"id":"msg_1","type":"message","role":"assistant","content":[]}}),json!({"type":"response.output_text.delta","item_id":"msg_1","output_index":0,"content_index":0,"delta":"switcher-ok"}),json!({"type":"response.output_item.done","output_index":0,"item":item}),json!({"type":"response.completed","response":{"id":"resp_test","status":"completed","output":[item],"usage":{"input_tokens":1,"output_tokens":1,"total_tokens":2}}})];
                let text=events.iter().map(|v|format!("data: {v}\n\n")).collect::<String>();
                ([("content-type","text/event-stream")],text).into_response()
            }
        }));
        let server = tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        });
        enable(&s, port, &helper).unwrap();
        let output = tokio::time::timeout(
            Duration::from_secs(40),
            tokio::process::Command::new(crate::launcher::codex_binary().unwrap())
                .env("CODEX_HOME", &s.codex_home)
                .env_remove("OPENAI_API_KEY")
                .env_remove("CODEX_API_KEY")
                .env_remove("CODEX_ACCESS_TOKEN")
                .env_remove("CODEX_SWITCHER_TOKEN")
                .current_dir(&s.root)
                .args([
                    "exec",
                    "--skip-git-repo-check",
                    "--sandbox",
                    "read-only",
                    "--json",
                    "-m",
                    "gpt-5.4",
                    "Reply switcher-ok without using tools.",
                ])
                .kill_on_drop(true)
                .output(),
        )
        .await
        .unwrap()
        .unwrap();
        server.abort();
        assert!(
            output.status.success(),
            "CLI rejected isolated config: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(String::from_utf8_lossy(&output.stdout).contains("switcher-ok"));
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 1);
    }
}
