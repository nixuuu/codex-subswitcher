//! File-backed Codex identities. Raw credentials never implement Debug or leave this module.
use anyhow::{Context, Result, ensure};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use fs2::FileExt;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
};

const MAX_AUTH: u64 = 1024 * 1024;
#[derive(Clone, Debug)]
pub struct Account {
    pub id: String,
    pub email: String,
    pub plan: String,
}
pub struct Credential {
    bytes: Vec<u8>,
    pub account: Account,
    workspace: String,
}
impl Credential {
    pub fn value(&self) -> Value {
        serde_json::from_slice(&self.bytes).expect("validated credential")
    }
    pub fn access_token(&self) -> String {
        self.value()["tokens"]["access_token"]
            .as_str()
            .unwrap()
            .to_owned()
    }
    pub fn workspace(&self) -> &str {
        &self.workspace
    }
    pub fn expires_soon(&self) -> bool {
        let token = self.access_token();
        let claims = token
            .split('.')
            .nth(1)
            .and_then(|s| URL_SAFE_NO_PAD.decode(s.trim_end_matches('=')).ok())
            .and_then(|b| serde_json::from_slice::<Value>(&b).ok());
        claims
            .and_then(|v| v["exp"].as_i64())
            .is_none_or(|exp| exp <= chrono::Utc::now().timestamp() + 300)
    }

    pub fn parse(bytes: Vec<u8>) -> Result<Self> {
        ensure!(
            bytes.len() <= MAX_AUTH as usize,
            "The credentials file is too large."
        );
        let value: Value = serde_json::from_slice(&bytes)
            .map_err(|_| anyhow::anyhow!("Invalid credentials file format."))?;
        ensure!(
            value
                .get("auth_mode")
                .and_then(Value::as_str)
                .is_none_or(|m| m == "chatgpt")
                && value.get("OPENAI_API_KEY").is_none_or(Value::is_null),
            "Only ChatGPT accounts are supported, not API keys."
        );
        let tokens = &value["tokens"];
        for key in ["id_token", "access_token", "refresh_token"] {
            ensure!(
                tokens[key].as_str().is_some_and(|v| !v.is_empty()),
                "Incomplete ChatGPT credentials. Sign in again."
            );
        }
        let payload = tokens["id_token"]
            .as_str()
            .unwrap()
            .split('.')
            .nth(1)
            .context("Invalid identity token.")?;
        let decoded = URL_SAFE_NO_PAD
            .decode(payload.trim_end_matches('='))
            .map_err(|_| anyhow::anyhow!("Invalid identity token."))?;
        let claims: Value = serde_json::from_slice(&decoded)
            .map_err(|_| anyhow::anyhow!("Invalid account metadata."))?;
        let auth = &claims["https://api.openai.com/auth"];
        let workspace = tokens["account_id"]
            .as_str()
            .or_else(|| auth["chatgpt_account_id"].as_str())
            .filter(|s| !s.is_empty())
            .context("Missing account ID.")?
            .to_owned();
        let subject = claims["sub"]
            .as_str()
            .or_else(|| auth["chatgpt_user_id"].as_str())
            .context("Missing user ID.")?;
        let id = format!("{:x}", Sha256::digest(format!("{workspace}\0{subject}")));
        // JWT claims are display metadata only; authentication is left to Codex.
        let email = claims["email"]
            .as_str()
            .unwrap_or("ChatGPT account")
            .to_owned();
        let plan = auth["chatgpt_plan_type"]
            .as_str()
            .unwrap_or("unknown plan")
            .to_owned();
        Ok(Self {
            bytes,
            account: Account { id, email, plan },
            workspace,
        })
    }
}

#[derive(Clone)]
pub struct Store {
    pub codex_home: PathBuf,
    pub root: PathBuf,
}
#[derive(Default)]
pub struct Snapshot {
    pub accounts: Vec<Account>,
    pub current: Option<Account>,
    pub config_enabled: bool,
    pub config_backup: bool,
}
impl Store {
    pub fn discover() -> Result<Self> {
        let user_home = std::env::var_os("HOME").context("User home directory not found.")?;
        let codex_home = std::env::var_os("CODEX_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(&user_home).join(".codex"));
        let root = std::env::var_os("CODEX_SWITCHER_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                PathBuf::from(user_home).join("Library/Application Support/Codex Sub Switcher")
            });
        Ok(Self { codex_home, root })
    }
    pub fn lock(&self) -> Result<File> {
        private_dir(&self.root)?;
        let path = self.root.join("operation.lock");
        reject_symlink(&path)?;
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .mode(0o600)
            .open(path)?;
        file.try_lock_exclusive()
            .context("Another switcher operation is in progress.")?;
        Ok(file)
    }
    fn check_config(&self, credential: Option<&Credential>) -> Result<()> {
        let path = self.codex_home.join("config.toml");
        let config: toml::Value = match fs::read_to_string(&path) {
            Ok(s) => {
                toml::from_str(&s).map_err(|_| anyhow::anyhow!("Could not read config.toml."))?
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                toml::Value::Table(Default::default())
            }
            Err(e) => return Err(e.into()),
        };
        ensure!(
            config
                .get("cli_auth_credentials_store")
                .and_then(toml::Value::as_str)
                .unwrap_or("file")
                == "file",
            "Only file credential storage is supported. A different cli_auth_credentials_store was detected; nothing was changed."
        );
        ensure!(
            config
                .get("forced_login_method")
                .and_then(toml::Value::as_str)
                .is_none_or(|m| m == "chatgpt"),
            "The configuration requires a different sign-in method."
        );
        if let (Some(expected), Some(credential)) =
            (config.get("forced_chatgpt_workspace_id"), credential)
        {
            let matches = expected.as_str().is_some_and(|s| s == credential.workspace)
                || expected
                    .as_array()
                    .is_some_and(|a| a.iter().any(|v| v.as_str() == Some(&credential.workspace)));
            ensure!(
                matches,
                "The account does not match the workspace required by the configuration."
            );
        }
        Ok(())
    }
    fn account_path(&self, id: &str) -> Result<PathBuf> {
        ensure!(
            id.len() == 64 && id.bytes().all(|b| b.is_ascii_hexdigit()),
            "Invalid profile ID."
        );
        Ok(self.root.join("accounts").join(format!("{id}.json")))
    }
    fn save(&self, credential: &Credential) -> Result<()> {
        private_dir(&self.root.join("accounts"))?;
        atomic_write(
            &self.account_path(&credential.account.id)?,
            &credential.bytes,
        )
    }
    pub fn snapshot(&self) -> Result<Snapshot> {
        let current = self
            .active_id()?
            .map(|id| self.credential(&id).map(|c| c.account))
            .transpose()?;
        let mut accounts = Vec::new();
        let dir = self.root.join("accounts");
        if dir.exists() {
            reject_symlink(&dir)?;
            for entry in fs::read_dir(dir)? {
                let path = entry?.path();
                if path.extension().is_some_and(|e| e == "json") {
                    let c = read_credential(&path)?
                        .context("The profile disappeared while being read.")?;
                    ensure!(
                        path.file_stem().and_then(|s| s.to_str()) == Some(&c.account.id),
                        "The profile identity does not match the file."
                    );
                    accounts.push(c.account);
                }
            }
        }
        accounts.sort_by(|a, b| a.email.cmp(&b.email).then(a.id.cmp(&b.id)));
        Ok(Snapshot {
            accounts,
            current,
            config_enabled: crate::config::enabled(self),
            config_backup: self.root.join("config-patch.json").exists(),
        })
    }
    pub fn import_current(&self) -> Result<Snapshot> {
        let _lock = self.lock()?;
        self.check_config(None)?;
        let c = read_credential(&self.codex_home.join("auth.json"))?
            .context("No saved CLI credentials. Choose Add account.")?;
        self.check_config(Some(&c))?;
        ensure!(
            !self.account_path(&c.account.id)?.exists(),
            "This account is already saved. Use Add account if you need to sign in again."
        );
        self.save(&c)?;
        self.snapshot()
    }
    pub fn import_from(&self, path: &Path) -> Result<Snapshot> {
        let _lock = self.lock()?;
        let c = read_credential(path)?.context("Sign-in did not save an account.")?;
        self.check_config(Some(&c))?;
        self.save(&c)?;
        self.snapshot()
    }
    pub fn switch(&self, id: &str) -> Result<Snapshot> {
        let _lock = self.lock()?;
        let target = read_credential(&self.account_path(id)?)?.context("Account not found.")?;
        ensure!(
            target.account.id == id,
            "The profile identity does not match the selection."
        );
        atomic_write(&self.root.join("active"), id.as_bytes())?;
        self.snapshot()
    }
    pub fn active_id(&self) -> Result<Option<String>> {
        match fs::read_to_string(self.root.join("active")) {
            Ok(id) => {
                self.account_path(&id)?;
                Ok(Some(id))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.into()),
        }
    }
    pub fn active_credential(&self) -> Result<Credential> {
        let id = self
            .active_id()?
            .context("Select an account in the switcher.")?;
        self.credential(&id)
    }
    pub fn credential(&self, id: &str) -> Result<Credential> {
        let c = read_credential(&self.account_path(id)?)?.context("Profile not found.")?;
        ensure!(
            c.account.id == id,
            "The profile identity does not match the selection."
        );
        Ok(c)
    }
    pub fn persist_refresh(&self, previous: &Credential, refreshed: &Credential) -> Result<()> {
        let _lock = self.lock()?;
        ensure!(
            previous.account.id == refreshed.account.id,
            "Refreshing credentials changed the account identity."
        );
        let stored = self.credential(&previous.account.id)?;
        ensure!(
            stored.bytes == previous.bytes,
            "The profile changed while refreshing; retry the request."
        );
        self.save(refreshed)
    }
    pub fn remove(&self, id: &str) -> Result<Snapshot> {
        let _lock = self.lock()?;
        let current = self.snapshot()?.current;
        ensure!(
            current.is_none_or(|c| c.id != id),
            "Switch to another account first."
        );
        let path = self.account_path(id)?;
        reject_symlink(&path)?;
        fs::remove_file(path)?;
        self.snapshot()
    }
}
pub fn private_dir(path: &Path) -> Result<()> {
    reject_symlink(path)?;
    fs::create_dir_all(path)?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    Ok(())
}
fn reject_symlink(path: &Path) -> Result<()> {
    match fs::symlink_metadata(path) {
        Ok(m) => ensure!(
            !m.file_type().is_symlink(),
            "Symbolic links are not allowed in credential storage."
        ),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(e.into()),
    }
    Ok(())
}
pub fn read_credential(path: &Path) -> Result<Option<Credential>> {
    reject_symlink(path)?;
    let f = match File::open(path) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.into()),
    };
    ensure!(
        f.metadata()?.is_file(),
        "Credentials must be stored in a regular file."
    );
    let mut bytes = Vec::new();
    f.take(MAX_AUTH + 1).read_to_end(&mut bytes)?;
    Credential::parse(bytes).map(Some)
}
pub fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    reject_symlink(path)?;
    let parent = path.parent().context("Destination directory not found.")?;
    ensure!(parent.is_dir(), "Destination directory not found.");
    let mut temp = tempfile::NamedTempFile::new_in(parent)?;
    temp.as_file()
        .set_permissions(fs::Permissions::from_mode(0o600))?;
    temp.write_all(bytes)?;
    temp.as_file().sync_all()?;
    temp.persist(path).map_err(|e| e.error)?;
    File::open(parent)?.sync_all()?;
    Ok(())
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use serde_json::json;
    pub fn fixture(workspace: &str, user: &str, marker: &str) -> Vec<u8> {
        let jwt = |v: Value| {
            format!(
                "e30.{}.sig",
                URL_SAFE_NO_PAD.encode(serde_json::to_vec(&v).unwrap())
            )
        };
        serde_json::to_vec(&json!({"auth_mode":"chatgpt","OPENAI_API_KEY":null,"tokens":{
            "id_token":jwt(json!({"sub":user,"email":format!("{user}@example.test"),"https://api.openai.com/auth":{"chatgpt_account_id":workspace,"chatgpt_plan_type":"plus"}})),
            "access_token":jwt(json!({"exp":chrono::Utc::now().timestamp()+3600,"marker":marker})),"refresh_token":format!("fake-refresh-{marker}"),"account_id":workspace
        },"last_refresh":"2026-09-10T00:00:00Z"})).unwrap()
    }
    pub fn store() -> (tempfile::TempDir, Store) {
        let temp = tempfile::tempdir().unwrap();
        let store = Store {
            root: temp.path().join("store"),
            codex_home: temp.path().join("codex"),
        };
        private_dir(&store.root).unwrap();
        private_dir(&store.codex_home).unwrap();
        (temp, store)
    }
    pub fn add(store: &Store, workspace: &str, user: &str) -> Account {
        let c = Credential::parse(fixture(workspace, user, user)).unwrap();
        store.save(&c).unwrap();
        c.account
    }
    #[test]
    fn switching_preserves_cli_and_targets_correct_identity() {
        let (_temp, s) = store();
        let a = add(&s, "a", "user-a");
        let b = add(&s, "b", "user-b");
        let original = fixture("cli", "cli-user", "cli");
        atomic_write(&s.codex_home.join("auth.json"), &original).unwrap();
        s.switch(&a.id).unwrap();
        s.switch(&b.id).unwrap();
        assert_eq!(s.active_credential().unwrap().account.id, b.id);
        assert_eq!(fs::read(s.codex_home.join("auth.json")).unwrap(), original);
        let mode = fs::metadata(s.account_path(&b.id).unwrap())
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o600);
        assert_eq!(
            fs::metadata(&s.root).unwrap().permissions().mode() & 0o777,
            0o700
        );
    }
    #[test]
    fn users_in_same_workspace_have_distinct_profiles() {
        let (_t, s) = store();
        assert_ne!(add(&s, "team", "alice").id, add(&s, "team", "bob").id);
    }
    #[test]
    fn invalid_target_and_active_removal_leave_selection_intact() {
        let (_t, s) = store();
        let a = add(&s, "a", "a");
        s.switch(&a.id).unwrap();
        assert!(s.switch(&"f".repeat(64)).is_err());
        assert!(s.switch("../../auth").is_err());
        assert!(s.remove(&a.id).is_err());
        assert_eq!(s.active_id().unwrap().unwrap(), a.id);
    }
    #[test]
    fn malformed_auth_never_exposes_raw_secrets() {
        let err = Credential::parse(br#"{"OPENAI_API_KEY":"sk-do-not-print"}"#.to_vec())
            .err()
            .unwrap();
        assert!(!err.to_string().contains("sk-do-not-print"));
        assert!(Credential::parse(vec![0; MAX_AUTH as usize + 1]).is_err());
    }
    #[test]
    fn symlink_and_second_mutator_are_rejected() {
        let (_t, s) = store();
        let a = add(&s, "a", "a");
        let _lock = s.lock().unwrap();
        assert!(s.switch(&a.id).is_err());
        drop(_lock);
        let outside = s.root.join("outside");
        atomic_write(&outside, b"original").unwrap();
        std::os::unix::fs::symlink(&outside, s.root.join("active")).unwrap();
        assert!(s.switch(&a.id).is_err());
        assert_eq!(fs::read(outside).unwrap(), b"original");
    }
    #[test]
    fn refresh_never_overwrites_newer_login() {
        let (_t, s) = store();
        let a = add(&s, "a", "a");
        let before = s.credential(&a.id).unwrap();
        let newer = Credential::parse(fixture("a", "a", "new-login")).unwrap();
        s.save(&newer).unwrap();
        assert!(s.persist_refresh(&before, &before).is_err());
        assert_eq!(s.credential(&a.id).unwrap().bytes, newer.bytes);
    }
    #[test]
    fn file_import_rejects_keyring_and_duplicate_stale_auth() {
        let (_t, s) = store();
        let bytes = fixture("a", "a", "a");
        atomic_write(&s.codex_home.join("auth.json"), &bytes).unwrap();
        atomic_write(
            &s.codex_home.join("config.toml"),
            b"cli_auth_credentials_store = 'keyring'\n",
        )
        .unwrap();
        assert!(s.import_current().is_err());
        atomic_write(&s.codex_home.join("config.toml"), b"").unwrap();
        s.import_current().unwrap();
        assert!(s.import_current().is_err());
    }
}
