//! A loopback-only, authenticated Responses transport. No prompt or token logging.
use crate::accounts::{Credential, Store, atomic_write, private_dir};
use anyhow::{Context, Result, ensure};
use axum::{
    Router,
    body::{Body, Bytes},
    extract::{DefaultBodyLimit, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use fs2::FileExt;
use futures_util::StreamExt;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    fs::{File, OpenOptions},
    net::TcpListener,
    os::unix::fs::OpenOptionsExt,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
use subtle::ConstantTimeEq;
use tokio::sync::{Mutex, Notify, Semaphore};

const UPSTREAM: &str = "https://chatgpt.com/backend-api/codex";
#[derive(Serialize, Deserialize)]
pub struct Connection {
    pub port: u16,
    pub token: String,
}
#[derive(Default)]
pub struct Metrics {
    pub requests: AtomicUsize,
    pub failures: AtomicUsize,
    pub inflight: AtomicUsize,
}
#[derive(Clone)]
struct ProxyState {
    store: Store,
    token: String,
    client: reqwest::Client,
    refresh: Arc<Mutex<()>>,
    metrics: Arc<Metrics>,
    slots: Arc<Semaphore>,
    upstream: String,
    refresh_endpoint: String,
}
pub struct Proxy {
    pub port: u16,
    pub metrics: Arc<Metrics>,
    stop: Arc<Notify>,
    runtime: tokio::runtime::Handle,
    state: ProxyState,
    _lock: File,
}
impl Drop for Proxy {
    fn drop(&mut self) {
        self.stop.notify_one();
    }
}
impl Proxy {
    pub fn usage(
        &self,
        ids: Vec<String>,
    ) -> tokio::task::JoinHandle<Vec<(String, Result<crate::usage::Usage, String>)>> {
        let state = self.state.clone();
        self.runtime.spawn(async move {
            futures_util::stream::iter(ids.into_iter().map(|id| {
                let state = state.clone();
                async move {
                    let result =
                        fetch_usage(&state, &id, "https://chatgpt.com/backend-api/wham/usage")
                            .await;
                    (id, result.map_err(|e| e.to_string()))
                }
            }))
            .buffer_unordered(3)
            .collect()
            .await
        })
    }
    pub fn reset(
        &self,
        id: String,
        credit_id: Option<String>,
    ) -> tokio::task::JoinHandle<Result<crate::resets::Outcome>> {
        let state = self.state.clone();
        self.runtime.spawn(async move {
            consume_reset(
                &state,
                &id,
                "https://chatgpt.com/backend-api/wham/rate-limit-reset-credits/consume",
                credit_id,
            )
            .await
        })
    }
    pub fn start(store: Store) -> Result<Self> {
        private_dir(&store.root)?;
        let lock = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .mode(0o600)
            .open(store.root.join("proxy.lock"))?;
        lock.try_lock_exclusive()
            .context("Proxy jest już uruchomione w innym procesie.")?;
        let connection_path = store.root.join("connection.json");
        let saved = match std::fs::read(&connection_path) {
            Ok(bytes) => Some(
                serde_json::from_slice::<Connection>(&bytes)
                    .context("Niepoprawny plik połączenia proxy.")?,
            ),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => return Err(e.into()),
        };
        let port = saved.as_ref().map(|c| c.port).unwrap_or(0);
        let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, port)).context(
            "Port proxy jest zajęty. Zamknij poprzedni proces switchera i spróbuj ponownie.",
        )?;
        listener.set_nonblocking(true)?;
        let port = listener.local_addr()?.port();
        let token = if let Some(c) = saved {
            ensure!(c.token.len() == 64, "Niepoprawny klucz lokalnego proxy.");
            c.token
        } else {
            let mut random = [0u8; 32];
            rand::rngs::OsRng.fill_bytes(&mut random);
            random
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<String>()
        };
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(Duration::from_secs(15))
            .read_timeout(Duration::from_secs(300))
            .build()?;
        let metrics = Arc::new(Metrics::default());
        let state = ProxyState {
            store: store.clone(),
            token: token.clone(),
            client,
            refresh: Arc::new(Mutex::new(())),
            metrics: metrics.clone(),
            slots: Arc::new(Semaphore::new(32)),
            upstream: UPSTREAM.into(),
            refresh_endpoint: "https://auth.openai.com/oauth/token".into(),
        };
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()?;
        let handle = runtime.handle().clone();
        let usage_state = state.clone();
        let stop = Arc::new(Notify::new());
        let shutdown = stop.clone();
        std::thread::Builder::new()
            .name("codex-proxy".into())
            .spawn(move || {
                runtime.block_on(async move {
                    if let Ok(listener) = tokio::net::TcpListener::from_std(listener) {
                        let _ = axum::serve(listener, router(state))
                            .with_graceful_shutdown(async move { shutdown.notified().await })
                            .await;
                    }
                });
            })?;
        atomic_write(
            &store.root.join("connection.json"),
            &serde_json::to_vec(&Connection { port, token })?,
        )?;
        Ok(Self {
            port,
            metrics,
            stop,
            _lock: lock,
            runtime: handle,
            state: usage_state,
        })
    }
}
fn router(state: ProxyState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/v1/models", get(models))
        .route("/v1/responses", post(responses))
        .route("/v1/responses/compact", post(compact))
        .layer(DefaultBodyLimit::max(64 * 1024 * 1024))
        .with_state(state)
}
fn authorized(headers: &HeaderMap, token: &str) -> bool {
    if headers.contains_key("origin") {
        return false;
    }
    headers
        .get("authorization")
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
        .is_some_and(|s| bool::from(s.as_bytes().ct_eq(token.as_bytes())))
}
async fn health(State(state): State<ProxyState>, headers: HeaderMap) -> Response {
    if !authorized(&headers, &state.token) {
        return error(StatusCode::UNAUTHORIZED, "Nieautoryzowany klient proxy.");
    }
    axum::Json(json!({"service":"codex-sub-switcher","version":1})).into_response()
}
async fn models(
    State(state): State<ProxyState>,
    headers: HeaderMap,
    axum::extract::Query(query): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Response {
    if !authorized(&headers, &state.token) {
        return error(StatusCode::UNAUTHORIZED, "Nieautoryzowany klient proxy.");
    }
    let Ok(_permit) = state.slots.clone().try_acquire_owned() else {
        return error(StatusCode::TOO_MANY_REQUESTS, "Proxy jest zajęte.");
    };
    let store = state.store.clone();
    let credential = match tokio::task::spawn_blocking(move || store.active_credential()).await {
        Ok(Ok(c)) => c,
        _ => {
            return error(
                StatusCode::SERVICE_UNAVAILABLE,
                "Wybierz konto w switcherze.",
            );
        }
    };
    let credential = match refresh_if_needed(&state, credential).await {
        Ok(c) => c,
        Err(_) => {
            return error(
                StatusCode::UNAUTHORIZED,
                "Odśwież logowanie konta w switcherze.",
            );
        }
    };
    let version = query
        .get("client_version")
        .map(String::as_str)
        .unwrap_or("0.154.0");
    if version.len() > 80 {
        return error(StatusCode::BAD_REQUEST, "Niepoprawna wersja klienta.");
    }
    let result = state
        .client
        .get(format!("{}/models", state.upstream))
        .query(&[("client_version", version)])
        .bearer_auth(credential.access_token())
        .header("chatgpt-account-id", credential.workspace())
        .header("originator", "codex_cli_rs")
        .timeout(Duration::from_secs(30))
        .send()
        .await;
    match result {
        Ok(r) => {
            let status = r.status();
            match r.bytes().await {
                Ok(bytes) => {
                    (status, [("content-type", "application/json")], bytes).into_response()
                }
                Err(_) => error(StatusCode::BAD_GATEWAY, "Nie można odczytać listy modeli."),
            }
        }
        Err(_) => error(StatusCode::BAD_GATEWAY, "Nie można pobrać listy modeli."),
    }
}
async fn responses(State(s): State<ProxyState>, h: HeaderMap, b: Bytes) -> Response {
    forward(s, h, b, "/responses").await
}
async fn compact(State(s): State<ProxyState>, h: HeaderMap, b: Bytes) -> Response {
    forward(s, h, b, "/responses/compact").await
}
fn error(status: StatusCode, message: &str) -> Response {
    (
        status,
        axum::Json(json!({"error":{"message":message,"type":"switcher_error"}})),
    )
        .into_response()
}
struct Inflight(Arc<Metrics>);
impl Drop for Inflight {
    fn drop(&mut self) {
        self.0.inflight.fetch_sub(1, Ordering::Relaxed);
    }
}
async fn forward(state: ProxyState, headers: HeaderMap, body: Bytes, path: &str) -> Response {
    if !authorized(&headers, &state.token) {
        return error(StatusCode::UNAUTHORIZED, "Nieautoryzowany klient proxy.");
    }
    if headers.contains_key("content-encoding") {
        return error(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "Uruchom CLI przez polecenie switchera (bez kompresji żądań).",
        );
    }
    let Ok(permit) = state.slots.clone().try_acquire_owned() else {
        return error(
            StatusCode::TOO_MANY_REQUESTS,
            "Proxy obsługuje już 32 żądania. Spróbuj za chwilę.",
        );
    };
    let mut payload: Value = match serde_json::from_slice(&body) {
        Ok(Value::Object(v)) => Value::Object(v),
        _ => return error(StatusCode::BAD_REQUEST, "Niepoprawne żądanie JSON."),
    };
    // The CLI HTTP transport sends full conversation input. Server-side continuation
    // IDs belong to an upstream account and cannot safely cross identities.
    if payload
        .get("previous_response_id")
        .is_some_and(|v| !v.is_null())
    {
        return error(
            StatusCode::BAD_REQUEST,
            "Proxy wymaga pełnej historii HTTP, bez previous_response_id.",
        );
    }
    if path == "/responses" {
        payload["store"] = json!(false);
        payload["stream"] = json!(true);
    }
    let store = state.store.clone();
    // Capture the account once. Switching during auth refresh or streaming never
    // changes the identity of this request.
    let credential = match tokio::task::spawn_blocking(move || store.active_credential()).await {
        Ok(Ok(c)) => c,
        _ => {
            return error(
                StatusCode::SERVICE_UNAVAILABLE,
                "Wybierz zapisane konto w aplikacji switchera.",
            );
        }
    };
    let credential = match refresh_if_needed(&state, credential).await {
        Ok(c) => c,
        Err(_) => {
            return error(
                StatusCode::UNAUTHORIZED,
                "Nie można odświeżyć logowania wybranego konta. Dodaj je ponownie lub przełącz konto.",
            );
        }
    };
    state.metrics.requests.fetch_add(1, Ordering::Relaxed);
    state.metrics.inflight.fetch_add(1, Ordering::Relaxed);
    let inflight = Inflight(state.metrics.clone());
    let mut request = state
        .client
        .post(format!("{}{path}", state.upstream))
        .bearer_auth(credential.access_token())
        .header("chatgpt-account-id", credential.workspace())
        .header("originator", "codex_cli_rs")
        .header("content-type", "application/json")
        .header("accept", "text/event-stream");
    for name in [
        "user-agent",
        "openai-beta",
        "session_id",
        "conversation_id",
        "x-codex-turn-metadata",
        "x-codex-beta-features",
    ] {
        if let Some(value) = headers.get(name) {
            request = request.header(name, value);
        }
    }
    // Sticky routing, cookies, organization headers and caller Authorization are
    // deliberately not forwarded across accounts.
    let request = request.json(&payload);
    let mut upstream = match request
        .try_clone()
        .expect("JSON request is replayable")
        .send()
        .await
    {
        Ok(r) => r,
        Err(_) => {
            state.metrics.failures.fetch_add(1, Ordering::Relaxed);
            return error(
                StatusCode::BAD_GATEWAY,
                "Nie udało się połączyć z usługą Codex.",
            );
        }
    };
    if upstream.status() == StatusCode::UNAUTHORIZED {
        // Only retry a rejected request, never an already-started response stream.
        let refreshed = match refresh_credential(&state, credential, true).await {
            Ok(c) => c,
            Err(_) => {
                state.metrics.failures.fetch_add(1, Ordering::Relaxed);
                return error(
                    StatusCode::UNAUTHORIZED,
                    "Logowanie wygasło lub zostało cofnięte. Dodaj konto ponownie.",
                );
            }
        };
        let mut retry = request
            .build()
            .expect("the initial request was built successfully");
        let bearer = match reqwest::header::HeaderValue::from_str(&format!(
            "Bearer {}",
            refreshed.access_token()
        )) {
            Ok(value) => value,
            Err(_) => return error(StatusCode::BAD_GATEWAY, "Niepoprawna odpowiedź logowania."),
        };
        retry
            .headers_mut()
            .insert(reqwest::header::AUTHORIZATION, bearer);
        upstream = match state.client.execute(retry).await {
            Ok(r) => r,
            Err(_) => {
                state.metrics.failures.fetch_add(1, Ordering::Relaxed);
                return error(
                    StatusCode::BAD_GATEWAY,
                    "Nie udało się ponowić żądania po odświeżeniu logowania.",
                );
            }
        };
    }
    let status = upstream.status();
    if !status.is_success() {
        state.metrics.failures.fetch_add(1, Ordering::Relaxed);
    }
    let mut response = Response::builder().status(status);
    for name in ["content-type", "retry-after", "x-request-id"] {
        if let Some(value) = upstream.headers().get(name) {
            response = response.header(name, value);
        }
    }
    // Bounded streaming with backpressure; dropping downstream drops upstream and
    // releases its concurrency slot. No automatic retry of a partially sent turn.
    let stream = futures_util::stream::unfold(
        (upstream.bytes_stream(), permit, inflight),
        |(mut stream, permit, inflight)| async move {
            stream.next().await.map(|chunk| {
                (
                    chunk.map_err(|_| std::io::Error::other("Przerwano strumień Codex.")),
                    (stream, permit, inflight),
                )
            })
        },
    );
    response.body(Body::from_stream(stream)).unwrap()
}
async fn consume_reset(
    state: &ProxyState,
    id: &str,
    endpoint: &str,
    credit_id: Option<String>,
) -> Result<crate::resets::Outcome> {
    let store = state.store.clone();
    let account_id = id.to_owned();
    let (credential, key) = tokio::task::spawn_blocking(move || -> Result<_> {
        let credential = store.credential(&account_id)?;
        let key = crate::resets::request(&store, &account_id, credit_id.as_deref())?;
        Ok((credential, key))
    })
    .await??;
    let mut credential = refresh_if_needed(state, credential)
        .await
        .map_err(|_| anyhow::anyhow!("Odśwież logowanie konta."))?;
    for attempt in 0..2 {
        let response = state
            .client
            .post(endpoint)
            .bearer_auth(credential.access_token())
            .header("ChatGPT-Account-Id", credential.workspace())
            .json(&key)
            .timeout(Duration::from_secs(20))
            .send()
            .await
            .map_err(|_| {
                anyhow::anyhow!(
                    "Nie potwierdzono wyniku restartu. Ponów tę samą operację przyciskiem restartu."
                )
            })?;
        if response.status() == StatusCode::UNAUTHORIZED && attempt == 0 {
            credential = refresh_credential(state, credential, true)
                .await
                .map_err(|_| anyhow::anyhow!("Odśwież logowanie konta."))?;
            continue;
        }
        ensure!(
            response.status().is_success(),
            "Nie potwierdzono restartu (HTTP {}). Ponowienie zachowa identyfikator operacji.",
            response.status().as_u16()
        );
        let mut response = response;
        let mut bytes = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|_| anyhow::anyhow!("Nie potwierdzono odpowiedzi restartu. Ponów operację."))?
        {
            ensure!(
                bytes.len() + chunk.len() <= 1024 * 1024,
                "Niepoprawna odpowiedź restartu. Ponów operację."
            );
            bytes.extend_from_slice(&chunk);
        }
        let result: crate::resets::Response = serde_json::from_slice(&bytes).map_err(|_| {
            anyhow::anyhow!(
                "Nieznana odpowiedź restartu. Ponowienie zachowa identyfikator operacji."
            )
        })?;
        let store = state.store.clone();
        let id = id.to_owned();
        tokio::task::spawn_blocking(move || {
            crate::resets::finish(&store, &id, &key.redeem_request_id)
        })
        .await??;
        return Ok(result.code);
    }
    anyhow::bail!("Odśwież logowanie konta.")
}

async fn fetch_usage(state: &ProxyState, id: &str, endpoint: &str) -> Result<crate::usage::Usage> {
    let bytes = fetch_account_bytes(state, id, endpoint).await?;
    let mut usage = crate::usage::Usage::parse(&bytes)
        .map_err(|_| anyhow::anyhow!("Niepoprawna odpowiedź usługi limitów."))?;
    let store = state.store.clone();
    let account_id = id.to_owned();
    usage.pending_reset =
        tokio::task::spawn_blocking(move || crate::resets::pending(&store, &account_id)).await??;
    let details_endpoint = format!(
        "{}/rate-limit-reset-credits",
        endpoint
            .rsplit_once('/')
            .context("Niepoprawny endpoint.")?
            .0
    );
    match fetch_account_bytes(state, id, &details_endpoint).await {
        Ok(bytes) => match crate::resets::Credits::parse(&bytes) {
            Ok(details) => usage.reset_details = Some(details),
            Err(_) => {
                usage.reset_details_error =
                    Some("Niepoprawne daty lub dane restartów. Odśwież listę.".into())
            }
        },
        Err(_) => {
            usage.reset_details_error =
                Some("Nie udało się pobrać dat ważności restartów. Odśwież listę.".into())
        }
    }
    Ok(usage)
}
async fn fetch_account_bytes(state: &ProxyState, id: &str, endpoint: &str) -> Result<Vec<u8>> {
    let store = state.store.clone();
    let id = id.to_owned();
    let credential = tokio::task::spawn_blocking(move || store.credential(&id)).await??;
    let mut credential = refresh_if_needed(state, credential)
        .await
        .map_err(|_| anyhow::anyhow!("Odśwież logowanie konta."))?;
    for attempt in 0..2 {
        let mut response = state
            .client
            .get(endpoint)
            .bearer_auth(credential.access_token())
            .header("ChatGPT-Account-Id", credential.workspace())
            .timeout(Duration::from_secs(20))
            .send()
            .await
            .map_err(|_| anyhow::anyhow!("Nie udało się połączyć z usługą limitów."))?;
        if response.status() == StatusCode::UNAUTHORIZED && attempt == 0 {
            credential = refresh_credential(state, credential, true)
                .await
                .map_err(|_| anyhow::anyhow!("Odśwież logowanie konta."))?;
            continue;
        }
        ensure!(
            response.status().is_success(),
            "Limity niedostępne (HTTP {}).",
            response.status().as_u16()
        );
        let mut bytes = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|_| anyhow::anyhow!("Przerwany odczyt limitów."))?
        {
            ensure!(
                bytes.len() + chunk.len() <= 1024 * 1024,
                "Odpowiedź limitów jest zbyt duża."
            );
            bytes.extend_from_slice(&chunk);
        }
        return Ok(bytes);
    }
    anyhow::bail!("Odśwież logowanie konta.")
}

async fn refresh_if_needed(state: &ProxyState, credential: Credential) -> Result<Credential> {
    refresh_credential(state, credential, false).await
}
async fn refresh_credential(
    state: &ProxyState,
    credential: Credential,
    force: bool,
) -> Result<Credential> {
    if !force && !credential.expires_soon() {
        return Ok(credential);
    }
    let _refresh = state.refresh.lock().await;
    let store = state.store.clone();
    let id = credential.account.id.clone();
    let current = tokio::task::spawn_blocking(move || store.credential(&id)).await??;
    if (!force && !current.expires_soon())
        || (force && current.access_token() != credential.access_token())
    {
        return Ok(current);
    }
    let mut value = current.value();
    let response = state.client.post(&state.refresh_endpoint)
        .timeout(Duration::from_secs(30))
        .json(&json!({"client_id":"app_EMoamEEZ73f0CkXaXp7hrann","grant_type":"refresh_token","refresh_token":value["tokens"]["refresh_token"]}))
        .send().await.context("Nie udało się odświeżyć logowania.")?;
    ensure!(
        response.status().is_success(),
        "Wymagane ponowne logowanie."
    );
    let refreshed: Value = response
        .json()
        .await
        .context("Niepoprawna odpowiedź logowania.")?;
    ensure!(
        refreshed["access_token"]
            .as_str()
            .is_some_and(|s| !s.is_empty()),
        "Brak nowego tokenu."
    );
    for key in ["access_token", "refresh_token", "id_token"] {
        if refreshed[key].as_str().is_some_and(|s| !s.is_empty()) {
            value["tokens"][key] = refreshed[key].clone();
        }
    }
    value["last_refresh"] = json!(chrono::Utc::now().to_rfc3339());
    let next = Credential::parse(serde_json::to_vec(&value)?)?;
    let store = state.store.clone();
    tokio::task::spawn_blocking(move || {
        store.persist_refresh(&current, &next)?;
        Ok(next)
    })
    .await?
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::accounts::tests::{add, store};
    use axum::http::header::{AUTHORIZATION, ORIGIN};
    #[test]
    fn local_auth_rejects_browser_origins_and_missing_or_wrong_keys() {
        let mut h = HeaderMap::new();
        assert!(!authorized(&h, "secret"));
        h.insert(AUTHORIZATION, "Bearer wrong".parse().unwrap());
        assert!(!authorized(&h, "secret"));
        h.insert(AUTHORIZATION, "Bearer secret".parse().unwrap());
        assert!(authorized(&h, "secret"));
        h.insert(ORIGIN, "https://example.test".parse().unwrap());
        assert!(!authorized(&h, "secret"));
    }
    async fn serve(router: Router) -> (String, tokio::task::JoinHandle<()>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let task = tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        });
        (format!("http://{addr}"), task)
    }
    #[tokio::test]
    async fn switches_next_request_while_existing_stream_keeps_original_account() {
        let (_temp, store) = store();
        let a = add(&store, "workspace-a", "alice");
        let b = add(&store, "workspace-b", "bob");
        store.switch(&a.id).unwrap();
        let (tx, mut rx) = tokio::sync::mpsc::channel::<(HeaderMap, Value)>(10);
        let finish = Arc::new(Notify::new());
        let gate = finish.clone();
        let upstream = Router::new().route(
            "/responses",
            post(
                move |headers: HeaderMap, axum::Json(body): axum::Json<Value>| {
                    let tx = tx.clone();
                    let gate = gate.clone();
                    async move {
                        let first = headers["chatgpt-account-id"] == "workspace-a";
                        tx.send((headers, body)).await.unwrap();
                        let initial = futures_util::stream::once(async {
                            Ok::<_, std::io::Error>(Bytes::from_static(b"data: first\n\n"))
                        });
                        let tail = futures_util::stream::once(async move {
                            if first {
                                gate.notified().await;
                            }
                            Ok::<_, std::io::Error>(Bytes::from_static(b"data: done\n\n"))
                        });
                        Response::builder()
                            .header("content-type", "text/event-stream")
                            .body(Body::from_stream(initial.chain(tail)))
                            .unwrap()
                    }
                },
            ),
        );
        let (upstream, up_task) = serve(upstream).await;
        let metrics = Arc::new(Metrics::default());
        let state = ProxyState {
            store: store.clone(),
            token: "test-local-secret".into(),
            client: reqwest::Client::new(),
            refresh: Arc::new(Mutex::new(())),
            metrics: metrics.clone(),
            slots: Arc::new(Semaphore::new(32)),
            upstream,
            refresh_endpoint: "https://auth.openai.com/oauth/token".into(),
        };
        let (base, proxy_task) = serve(router(state)).await;
        let client = reqwest::Client::new();
        let first = client
            .post(format!("{base}/v1/responses"))
            .bearer_auth("test-local-secret")
            .header("cookie", "not-forwarded")
            .header("x-codex-turn-state", "account-a-sticky")
            .json(&json!({"input":[],"store":true,"stream":true}))
            .send()
            .await
            .unwrap();
        let mut first_stream = first.bytes_stream();
        assert_eq!(
            first_stream.next().await.unwrap().unwrap(),
            "data: first\n\n"
        );
        let (ah, body) = rx.recv().await.unwrap();
        assert_eq!(ah["chatgpt-account-id"], "workspace-a");
        assert!(!ah.contains_key("cookie"));
        assert!(!ah.contains_key("x-codex-turn-state"));
        assert_eq!(body["store"], false);
        let old_auth = ah["authorization"].clone();
        store.switch(&b.id).unwrap();
        let second = client
            .post(format!("{base}/v1/responses"))
            .bearer_auth("test-local-secret")
            .json(&json!({"input":[]}))
            .send()
            .await
            .unwrap();
        assert_eq!(second.status(), StatusCode::OK);
        let (bh, _) = rx.recv().await.unwrap();
        assert_eq!(bh["chatgpt-account-id"], "workspace-b");
        assert_ne!(bh["authorization"], old_auth);
        assert_ne!(bh["authorization"], "Bearer test-local-secret");
        assert_eq!(
            second.text().await.unwrap(),
            "data: first\n\ndata: done\n\n"
        );
        finish.notify_one();
        assert_eq!(
            first_stream.next().await.unwrap().unwrap(),
            "data: done\n\n"
        );
        assert!(first_stream.next().await.is_none());
        assert_eq!(metrics.requests.load(Ordering::Relaxed), 2);
        let denied = client
            .post(format!("{base}/v1/responses"))
            .json(&json!({"input":[]}))
            .send()
            .await
            .unwrap();
        assert_eq!(denied.status(), StatusCode::UNAUTHORIZED);
        let continuation = client
            .post(format!("{base}/v1/responses"))
            .bearer_auth("test-local-secret")
            .json(&json!({"previous_response_id":"resp_a"}))
            .send()
            .await
            .unwrap();
        assert_eq!(continuation.status(), StatusCode::BAD_REQUEST);
        assert!(rx.try_recv().is_err());
        proxy_task.abort();
        up_task.abort();
    }
}

#[cfg(test)]
mod refresh_tests {
    use super::*;
    use crate::accounts::tests::{add, fixture, store};
    async fn mock(router: Router) -> (String, tokio::task::JoinHandle<()>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let task = tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        });
        (format!("http://{addr}"), task)
    }
    fn state(store: Store, url: String) -> ProxyState {
        ProxyState {
            store,
            token: "local-test-key".into(),
            client: reqwest::Client::new(),
            refresh: Arc::new(Mutex::new(())),
            metrics: Arc::new(Metrics::default()),
            slots: Arc::new(Semaphore::new(32)),
            upstream: url.clone(),
            refresh_endpoint: format!("{url}/refresh"),
        }
    }
    #[tokio::test]
    async fn usage_reads_requested_account_without_switching_and_sanitizes_errors() {
        let (_t, store) = store();
        let a = add(&store, "a", "alice");
        let b = add(&store, "b", "bob");
        store.switch(&a.id).unwrap();
        let app = Router::new().route("/usage", get(|headers: HeaderMap| async move {
            assert_eq!(headers["chatgpt-account-id"], "b");
            assert!(headers["authorization"].to_str().unwrap().starts_with("Bearer "));
            axum::Json(json!({"rate_limit":{"primary_window":{"used_percent":37,"limit_window_seconds":604800,"reset_at":2000000000}}}))
        })).route("/error", get(|| async { (StatusCode::TOO_MANY_REQUESTS, "private-response-must-not-appear") }));
        let (url, server) = mock(app).await;
        let state = state(store.clone(), url.clone());
        let usage = fetch_usage(&state, &b.id, &format!("{url}/usage"))
            .await
            .unwrap();
        assert_eq!(usage.windows().next().unwrap().used_percent, 37.);
        assert_eq!(store.active_id().unwrap(), Some(a.id));
        let error = fetch_usage(&state, &b.id, &format!("{url}/error"))
            .await
            .err()
            .unwrap()
            .to_string();
        assert!(error.contains("429"));
        assert!(!error.contains("private-response"));
        server.abort();
    }
    #[tokio::test]
    async fn usage_fetches_credit_expirations_for_the_same_account() {
        let (_t, store) = store();
        let a = add(&store, "a", "alice");
        let app = Router::new()
            .route("/usage", get(|| async { axum::Json(json!({"rate_limit":null,"rate_limit_reset_credits":{"available_count":1}})) }))
            .route("/rate-limit-reset-credits", get(|headers: HeaderMap| async move {
                assert_eq!(headers["chatgpt-account-id"], "a");
                axum::Json(json!({"available_count":1,"credits":[{"id":"expiring","reset_type":"codex_rate_limits","status":"available","granted_at":"2026-01-01T00:00:00Z","expires_at":"2099-01-01T00:00:00Z"}]}))
            }));
        let (url, server) = mock(app).await;
        let state = state(store.clone(), url.clone());
        let usage = fetch_usage(&state, &a.id, &format!("{url}/usage"))
            .await
            .unwrap();
        assert!(usage.reset_details_error.is_none());
        assert_eq!(
            usage
                .reset_details
                .unwrap()
                .next(chrono::Utc::now())
                .unwrap()
                .id,
            "expiring"
        );
        assert!(store.active_id().unwrap().is_none());
        server.abort();
    }
    #[tokio::test]
    async fn reset_retry_preserves_key_and_account_after_ambiguous_response() {
        let (_t, store) = store();
        let a = add(&store, "a", "alice");
        let b = add(&store, "b", "bob");
        store.switch(&a.id).unwrap();
        let seen = Arc::new(Mutex::new(Vec::<Value>::new()));
        let requests = seen.clone();
        let app = Router::new().route(
            "/consume",
            post(
                move |headers: HeaderMap, axum::Json(body): axum::Json<Value>| {
                    let requests = requests.clone();
                    async move {
                        assert_eq!(headers["chatgpt-account-id"], "a");
                        assert_eq!(body["credit_id"], "credit-earliest");
                        let mut requests = requests.lock().await;
                        requests.push(body);
                        if requests.len() == 1 {
                            (StatusCode::INTERNAL_SERVER_ERROR, "private response").into_response()
                        } else {
                            axum::Json(json!({"code":"already_redeemed","windows_reset":2}))
                                .into_response()
                        }
                    }
                },
            ),
        );
        let (url, server) = mock(app).await;
        let state = state(store.clone(), url.clone());
        let error = consume_reset(
            &state,
            &a.id,
            &format!("{url}/consume"),
            Some("credit-earliest".into()),
        )
        .await
        .unwrap_err()
        .to_string();
        assert!(!error.contains("private response"));
        assert_eq!(
            seen.lock().await.len(),
            1,
            "No automatic replay after HTTP 500"
        );
        assert!(crate::resets::pending(&store, &a.id).unwrap());
        store.switch(&b.id).unwrap();
        let outcome = consume_reset(
            &state,
            &a.id,
            &format!("{url}/consume"),
            Some("credit-different".into()),
        )
        .await
        .unwrap();
        assert_eq!(outcome, crate::resets::Outcome::AlreadyRedeemed);
        let requests = seen.lock().await;
        assert_eq!(requests[0], requests[1]);
        assert!(!crate::resets::pending(&store, &a.id).unwrap());
        assert_eq!(store.active_id().unwrap(), Some(b.id));
        server.abort();
    }
    #[tokio::test]
    async fn reset_outcomes_are_explicit_and_unknown_response_keeps_pending_key() {
        for code in ["reset", "nothing_to_reset", "no_credit", "future_unknown"] {
            let (_t, store) = store();
            let a = add(&store, "a", "alice");
            let app = Router::new().route(
                "/consume",
                post(move || async move { axum::Json(json!({"code":code})) }),
            );
            let (url, server) = mock(app).await;
            let state = state(store.clone(), url.clone());
            let outcome = consume_reset(
                &state,
                &a.id,
                &format!("{url}/consume"),
                Some("credit-earliest".into()),
            )
            .await;
            assert_eq!(outcome.is_err(), code == "future_unknown");
            assert_eq!(
                crate::resets::pending(&store, &a.id).unwrap(),
                code == "future_unknown"
            );
            server.abort();
        }
    }
    #[tokio::test]
    async fn concurrent_expiry_rotates_refresh_token_only_once() {
        let (_t, s) = store();
        let a = add(&s, "a", "alice");
        let before = s.credential(&a.id).unwrap();
        let mut expired = before.value();
        expired["tokens"]["access_token"] = json!("e30.eyJleHAiOjB9.sig");
        let expired = Credential::parse(serde_json::to_vec(&expired).unwrap()).unwrap();
        s.persist_refresh(&before, &expired).unwrap();
        let count = Arc::new(AtomicUsize::new(0));
        let calls = count.clone();
        let rotated = Credential::parse(fixture("a", "alice", "rotated"))
            .unwrap()
            .value()["tokens"]
            .clone();
        let response = rotated.clone();
        let router = Router::new().route(
            "/refresh",
            post(move |axum::Json(request): axum::Json<Value>| {
                let calls = calls.clone();
                let response = response.clone();
                async move {
                    assert_eq!(request["grant_type"], "refresh_token");
                    calls.fetch_add(1, Ordering::SeqCst);
                    axum::Json(response)
                }
            }),
        );
        let (url, task) = mock(router).await;
        let state = state(s.clone(), url);
        let (one, two) = tokio::join!(
            refresh_if_needed(&state, s.credential(&a.id).unwrap()),
            refresh_if_needed(&state, s.credential(&a.id).unwrap())
        );
        assert_eq!(one.unwrap().access_token(), two.unwrap().access_token());
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(
            s.credential(&a.id).unwrap().value()["tokens"]["refresh_token"],
            rotated["refresh_token"]
        );
        task.abort();
    }
    #[tokio::test]
    async fn unauthorized_retry_remains_on_captured_account_after_switch() {
        let (_t, s) = store();
        let a = add(&s, "a", "alice");
        let b = add(&s, "b", "bob");
        s.switch(&a.id).unwrap();
        let old = format!("Bearer {}", s.credential(&a.id).unwrap().access_token());
        let rotated = Credential::parse(fixture("a", "alice", "rotated"))
            .unwrap()
            .value()["tokens"]
            .clone();
        let started = Arc::new(Notify::new());
        let proceed = Arc::new(Notify::new());
        let start = started.clone();
        let gate = proceed.clone();
        let accounts = Arc::new(Mutex::new(Vec::<String>::new()));
        let captured = accounts.clone();
        let router = Router::new()
            .route(
                "/refresh",
                post(move || {
                    let start = start.clone();
                    let gate = gate.clone();
                    let tokens = rotated.clone();
                    async move {
                        start.notify_one();
                        gate.notified().await;
                        axum::Json(tokens)
                    }
                }),
            )
            .route(
                "/responses",
                post(move |h: HeaderMap| {
                    let old = old.clone();
                    let captured = captured.clone();
                    async move {
                        captured
                            .lock()
                            .await
                            .push(h["chatgpt-account-id"].to_str().unwrap().into());
                        if h["authorization"] == old {
                            StatusCode::UNAUTHORIZED.into_response()
                        } else {
                            "ok".into_response()
                        }
                    }
                }),
            );
        let (url, up) = mock(router).await;
        let state = state(s.clone(), url);
        let (url, proxy) = mock(super::router(state)).await;
        let response = tokio::spawn(async move {
            reqwest::Client::new()
                .post(format!("{url}/v1/responses"))
                .bearer_auth("local-test-key")
                .json(&json!({"input":[]}))
                .send()
                .await
                .unwrap()
                .text()
                .await
                .unwrap()
        });
        tokio::time::timeout(Duration::from_secs(5), started.notified())
            .await
            .unwrap();
        s.switch(&b.id).unwrap();
        proceed.notify_one();
        assert_eq!(response.await.unwrap(), "ok");
        assert_eq!(*accounts.lock().await, vec!["a", "a"]);
        assert_eq!(s.active_id().unwrap().unwrap(), b.id);
        proxy.abort();
        up.abort();
    }
    #[tokio::test]
    async fn quota_errors_keep_status_and_retry_after_without_switching() {
        let (_t, s) = store();
        let a = add(&s, "a", "alice");
        s.switch(&a.id).unwrap();
        let count = Arc::new(AtomicUsize::new(0));
        let calls = count.clone();
        let router = Router::new().route(
            "/responses",
            post(move || {
                let calls = calls.clone();
                async move {
                    calls.fetch_add(1, Ordering::SeqCst);
                    (
                        StatusCode::TOO_MANY_REQUESTS,
                        [("retry-after", "60")],
                        "quota",
                    )
                }
            }),
        );
        let (url, up) = mock(router).await;
        let (url, proxy) = mock(super::router(state(s.clone(), url))).await;
        let r = reqwest::Client::new()
            .post(format!("{url}/v1/responses"))
            .bearer_auth("local-test-key")
            .json(&json!({"input":[]}))
            .send()
            .await
            .unwrap();
        assert_eq!(r.status(), StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(r.headers()["retry-after"], "60");
        assert_eq!(r.text().await.unwrap(), "quota");
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(s.active_id().unwrap().unwrap(), a.id);
        proxy.abort();
        up.abort();
    }
}

#[cfg(test)]
mod live_tests {
    use super::*;
    /// Opt-in probe: real subscription, isolated CODEX_HOME, one tiny generation.
    #[tokio::test]
    #[ignore = "real subscription; requires CODEX_SWITCHER_LIVE_AUTH and a built debug binary"]
    async fn live_subscription_smoke() {
        let source = std::path::PathBuf::from(
            std::env::var_os("CODEX_SWITCHER_LIVE_AUTH").expect("explicit auth path required"),
        );
        let original = std::fs::read(&source).unwrap();
        let credential = Credential::parse(original.clone()).unwrap();
        assert!(
            !credential.expires_soon(),
            "Live smoke must not refresh shared credentials"
        );
        let (_temp, store) = crate::accounts::tests::store();
        atomic_write(&store.codex_home.join("auth.json"), &original).unwrap();
        store.import_current().unwrap();
        store.switch(&credential.account.id).unwrap();
        let proxy = Proxy::start(store.clone()).unwrap();
        let binary = std::env::current_dir()
            .unwrap()
            .join("target/debug/codex-sub-switcher");
        crate::config::enable(&store, proxy.port, &binary).unwrap();
        let output = tokio::time::timeout(
            Duration::from_secs(90),
            tokio::process::Command::new(crate::launcher::codex_binary().unwrap())
                .env("CODEX_HOME", &store.codex_home)
                .env("CODEX_SWITCHER_HOME", &store.root)
                .env_remove("OPENAI_API_KEY")
                .env_remove("CODEX_API_KEY")
                .env_remove("CODEX_ACCESS_TOKEN")
                .env_remove("CODEX_SWITCHER_TOKEN")
                .current_dir(&store.root)
                .args([
                    "exec",
                    "--ephemeral",
                    "--skip-git-repo-check",
                    "--sandbox",
                    "read-only",
                    "--json",
                    "-m",
                    "gpt-6-astra",
                    "-c",
                    "model_reasoning_effort=\"low\"",
                    "Reply exactly SWITCHER_OK. Do not use tools.",
                ])
                .kill_on_drop(true)
                .output(),
        )
        .await
        .unwrap()
        .unwrap();
        assert_eq!(
            std::fs::read(&source).unwrap(),
            original,
            "Source credentials must remain unchanged"
        );
        assert!(
            output.status.success(),
            "Live CLI failed: {}\n{}",
            String::from_utf8_lossy(&output.stderr),
            String::from_utf8_lossy(&output.stdout)
        );
        assert!(String::from_utf8_lossy(&output.stdout).contains("SWITCHER_OK"));
        assert!(proxy.metrics.requests.load(Ordering::Relaxed) >= 1);
    }
}
