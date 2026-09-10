//! Durable idempotency keys prevent a retry after an ambiguous response using another reset.
use crate::accounts::{Store, atomic_write};
use anyhow::{Result, ensure};
use chrono::{DateTime, Local, Utc};
use rand::RngCore;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    Reset,
    NothingToReset,
    NoCredit,
    AlreadyRedeemed,
}
#[derive(Deserialize)]
pub struct Response {
    pub code: Outcome,
}
impl Outcome {
    pub fn message(&self) -> &'static str {
        match self {
            Self::Reset => "Restart wykorzystany. Pobieram aktualne limity.",
            Self::NothingToReset => "Limity nie wymagają teraz resetu.",
            Self::NoCredit => "Brak dostępnego restartu na tym koncie.",
            Self::AlreadyRedeemed => {
                "Ten restart został już wykorzystany. Pobieram aktualne limity."
            }
        }
    }
}
#[derive(Clone)]
pub struct Credit {
    pub id: String,
    pub expires_at: Option<DateTime<Utc>>,
    pub granted_at: DateTime<Utc>,
    pub available: bool,
}
impl Credit {
    pub fn eligible(&self, now: DateTime<Utc>) -> bool {
        self.available && self.expires_at.is_none_or(|at| at > now)
    }
    pub fn expiry_label(&self) -> String {
        self.expires_at
            .map(|at| {
                format!(
                    "Wygasa {}",
                    at.with_timezone(&Local).format("%d.%m.%Y %H:%M %Z")
                )
            })
            .unwrap_or("Bez daty wygaśnięcia".into())
    }
}
#[derive(Clone)]
pub struct Credits {
    pub credits: Vec<Credit>,
    pub available_count: u32,
}
impl Credits {
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        #[derive(Deserialize)]
        struct RawCredit {
            id: String,
            reset_type: String,
            status: String,
            granted_at: String,
            expires_at: Option<String>,
        }
        #[derive(Deserialize)]
        struct Raw {
            credits: Vec<RawCredit>,
            available_count: u32,
        }
        let raw: Raw = serde_json::from_slice(bytes)?;
        ensure!(raw.credits.len() <= 1000, "Zbyt długa lista restartów.");
        let mut credits = Vec::new();
        let mut ids = std::collections::HashSet::new();
        for c in raw.credits {
            ensure!(
                !c.id.is_empty() && ids.insert(c.id.clone()),
                "Niepoprawne identyfikatory restartów."
            );
            credits.push(Credit {
                id: c.id,
                expires_at: c
                    .expires_at
                    .map(|s| DateTime::parse_from_rfc3339(&s).map(|at| at.with_timezone(&Utc)))
                    .transpose()?,
                granted_at: DateTime::parse_from_rfc3339(&c.granted_at)?.with_timezone(&Utc),
                available: c.status == "available" && c.reset_type == "codex_rate_limits",
            });
        }
        credits.sort_by(|a, b| {
            a.expires_at
                .unwrap_or(DateTime::<Utc>::MAX_UTC)
                .cmp(&b.expires_at.unwrap_or(DateTime::<Utc>::MAX_UTC))
                .then(a.granted_at.cmp(&b.granted_at))
                .then(a.id.cmp(&b.id))
        });
        Ok(Self {
            credits,
            available_count: raw.available_count,
        })
    }
    pub fn next(&self, now: DateTime<Utc>) -> Option<&Credit> {
        if self.available_count == 0 {
            return None;
        }
        self.credits.iter().find(|c| c.eligible(now))
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Request {
    pub redeem_request_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub credit_id: Option<String>,
}
fn read_request(bytes: &[u8]) -> Result<Request> {
    // Preserve an uncertain operation saved by an earlier version verbatim.
    let request = if let Ok(key) = serde_json::from_slice::<String>(bytes) {
        Request {
            redeem_request_id: key,
            credit_id: None,
        }
    } else {
        serde_json::from_slice::<Request>(bytes)?
    };
    ensure!(
        request.redeem_request_id.len() == 64
            && request
                .redeem_request_id
                .bytes()
                .all(|b| b.is_ascii_hexdigit()),
        "Niepoprawny zapis restartu."
    );
    ensure!(
        request.credit_id.as_ref().is_none_or(|id| !id.is_empty()),
        "Niepoprawny zapis restartu."
    );
    Ok(request)
}
fn path(store: &Store, id: &str) -> Result<std::path::PathBuf> {
    ensure!(
        id.len() == 64 && id.bytes().all(|b| b.is_ascii_hexdigit()),
        "Niepoprawny profil."
    );
    Ok(store.root.join(format!("reset-{id}.json")))
}
pub fn pending(store: &Store, id: &str) -> Result<bool> {
    Ok(path(store, id)?.try_exists()?)
}
pub fn request(store: &Store, id: &str, credit_id: Option<&str>) -> Result<Request> {
    let _lock = store.lock()?;
    let path = path(store, id)?;
    match std::fs::read(&path) {
        Ok(bytes) => read_request(&bytes),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            let credit_id = credit_id
                .filter(|s| !s.is_empty())
                .ok_or_else(|| anyhow::anyhow!("Odśwież listę restartów przed nową operacją."))?;
            let mut bytes = [0u8; 32];
            rand::rngs::OsRng.fill_bytes(&mut bytes);
            let request = Request {
                redeem_request_id: bytes.iter().map(|b| format!("{b:02x}")).collect(),
                credit_id: Some(credit_id.into()),
            };
            atomic_write(&path, &serde_json::to_vec(&request)?)?;
            Ok(request)
        }
        Err(e) => Err(e.into()),
    }
}
pub fn finish(store: &Store, id: &str, key: &str) -> Result<()> {
    let _lock = store.lock()?;
    let path = path(store, id)?;
    let saved = read_request(&std::fs::read(&path)?)?.redeem_request_id;
    ensure!(saved == key, "Zapis restartu zmienił się podczas operacji.");
    std::fs::remove_file(path)?;
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn selects_earliest_expiry_by_instant_and_skips_ineligible_credits() {
        let credit = |id, expires, status, kind| serde_json::json!({"id":id,"reset_type":kind,"status":status,"granted_at":"2026-09-01T00:00:00Z","expires_at":expires});
        let value = serde_json::json!({"available_count":3,"credits":[
            credit("no-expiry",None,"available","codex_rate_limits"),
            credit("later",Some("2026-09-12T10:00:00Z"),"available","codex_rate_limits"),
            credit("earliest",Some("2026-09-12T11:00:00+02:00"),"available","codex_rate_limits"),
            credit("expired",Some("2026-09-09T10:00:00Z"),"available","codex_rate_limits"),
            credit("used",Some("2026-09-11T10:00:00Z"),"redeemed","codex_rate_limits"),
            credit("unknown",Some("2026-09-11T10:00:00Z"),"available","future_type")
        ]});
        let mut details = Credits::parse(&serde_json::to_vec(&value).unwrap()).unwrap();
        let now = DateTime::parse_from_rfc3339("2026-09-10T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        assert_eq!(details.next(now).unwrap().id, "earliest");
        assert_eq!(
            details.next(now + chrono::Duration::days(3)).unwrap().id,
            "no-expiry"
        );
        details.available_count = 0;
        assert!(details.next(now).is_none());
        let mut invalid = value;
        invalid["credits"][0]["expires_at"] = serde_json::json!("invalid");
        assert!(Credits::parse(&serde_json::to_vec(&invalid).unwrap()).is_err());
    }
    #[test]
    fn a_new_reset_requires_an_explicit_credit() {
        let (_temp, store) = crate::accounts::tests::store();
        let id = "a".repeat(64);
        assert!(request(&store, &id, None).is_err());
        assert!(!pending(&store, &id).unwrap());
    }
    #[test]
    fn ambiguous_request_reuses_key_across_store_instances_and_accounts_are_isolated() {
        let (_temp, store) = crate::accounts::tests::store();
        let a = "a".repeat(64);
        let b = "b".repeat(64);
        let key = request(&store, &a, Some("credit-a")).unwrap();
        assert_eq!(
            request(&store.clone(), &a, Some("different-credit")).unwrap(),
            key
        );
        assert_ne!(request(&store, &b, Some("credit-b")).unwrap(), key);
        assert!(finish(&store, &a, "wrong").is_err());
        assert_eq!(request(&store, &a, Some("credit-a")).unwrap(), key);
        finish(&store, &a, &key.redeem_request_id).unwrap();
        assert_ne!(request(&store, &a, Some("credit-a")).unwrap(), key);
    }
}
