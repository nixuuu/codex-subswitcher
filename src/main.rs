mod accounts;
mod config;
mod dock;
mod launcher;
mod palette;
mod proxy;
mod resets;
mod tray;
mod ui;
mod usage;

use accounts::{Snapshot, Store};
use gpui_kit::component::{
    ActiveTheme, Disableable, Root, ThemeMode, WindowExt,
    button::{Button, ButtonVariants},
    dialog::DialogButtonProps,
    progress::Progress,
};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

#[derive(Default)]
struct UsageView {
    data: Option<usage::Usage>,
    checked: Option<chrono::DateTime<chrono::Utc>>,
    error: Option<String>,
}
struct Switcher {
    store: Store,
    snapshot: Snapshot,
    status: SharedString,
    error: bool,
    busy: bool,
    task: Option<Task<()>>,
    poll: Option<Task<()>>,
    cancel: Arc<AtomicBool>,
    login_pending: bool,
    proxy: Option<proxy::Proxy>,
    counters: (usize, usize, usize),
    command: SharedString,
    usage: std::collections::HashMap<String, UsageView>,
    usage_task: Option<Task<()>>,
    usage_loading: bool,
    show_connection: bool,
    usage_generation: u64,
    usage_next: std::time::Instant,
    now: chrono::DateTime<chrono::Utc>,
    page_scroll: ScrollHandle,
    expanded_accounts: std::collections::HashSet<String>,
}
impl Drop for Switcher {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}
impl Switcher {
    fn new(
        store: Store,
        snapshot: anyhow::Result<Snapshot>,
        proxy: anyhow::Result<proxy::Proxy>,
        command: String,
        cx: &mut Context<Self>,
    ) -> Self {
        let (snapshot, status, error) = match (snapshot, &proxy) {
            (Ok(s), Ok(_)) => (s, "".to_owned(), false),
            (Err(e), _) => (Snapshot::default(), e.to_string(), true),
            (Ok(s), Err(e)) => (s, e.to_string(), true),
        };
        let mut this = Self {
            store,
            snapshot,
            status: status.into(),
            error,
            busy: false,
            task: None,
            poll: None,
            cancel: Arc::new(AtomicBool::new(false)),
            login_pending: false,
            proxy: proxy.ok(),
            counters: (0, 0, 0),
            command: command.into(),
            usage: Default::default(),
            usage_task: None,
            usage_loading: false,
            show_connection: false,
            usage_generation: 0,
            usage_next: std::time::Instant::now(),
            now: chrono::Utc::now(),
            page_scroll: ScrollHandle::new(),
            expanded_accounts: Default::default(),
        };
        this.poll = Some(cx.spawn(async move |entity, cx| {
            loop {
                cx.background_executor()
                    .timer(std::time::Duration::from_secs(1))
                    .await;
                if entity
                    .update(cx, |this, cx| {
                        this.sync_tray(cx);
                        let now = chrono::Utc::now();
                        if now.timestamp() / 60 != this.now.timestamp() / 60 {
                            this.now = now;
                            cx.notify();
                        }
                        if std::time::Instant::now() >= this.usage_next {
                            this.refresh_usage(cx);
                        }
                        if let Some(proxy) = &this.proxy {
                            let m = &proxy.metrics;
                            let counters = (
                                m.requests.load(Ordering::Relaxed),
                                m.inflight.load(Ordering::Relaxed),
                                m.failures.load(Ordering::Relaxed),
                            );
                            if counters != this.counters {
                                this.counters = counters;
                                cx.notify();
                            }
                        }
                    })
                    .is_err()
                {
                    break;
                }
            }
        }));
        #[cfg(debug_assertions)]
        if std::env::args().any(|a| a == "--demo-usage") {
            for (i, account) in this.snapshot.accounts.iter().enumerate() {
                let window = |seconds, used, reset| serde_json::json!({"limit_window_seconds":seconds,"used_percent":used,"reset_at":chrono::Utc::now().timestamp()+reset});
                let data = serde_json::json!({"rate_limit_reset_credits":{"available_count":2},"rate_limit":{"primary_window":window(if i % 3 == 0 {18000} else {604800}, [37,18,61,82,6,96][i % 6], 7200), "secondary_window":if i % 3 == 0 {window(604800,64,172800)} else {serde_json::Value::Null}}});
                let mut demo_usage =
                    usage::Usage::parse(&serde_json::to_vec(&data).unwrap()).unwrap();
                let demo_credit = |id, expires| serde_json::json!({"id":id,"reset_type":"codex_rate_limits","status":"available","granted_at":"2026-09-01T00:00:00Z","expires_at":expires});
                let details = serde_json::json!({"available_count":2,"credits":[demo_credit("later",None::<String>),demo_credit("soon",Some((chrono::Utc::now()+chrono::Duration::days(3)).to_rfc3339()))]});
                demo_usage.reset_details =
                    Some(resets::Credits::parse(&serde_json::to_vec(&details).unwrap()).unwrap());
                this.usage.insert(
                    account.id.clone(),
                    UsageView {
                        data: Some(demo_usage),
                        checked: Some(chrono::Utc::now()),
                        error: None,
                    },
                );
            }
        }
        this.refresh_usage(cx);
        this.sync_tray(cx);
        this
    }
    fn sync_tray(&self, cx: &mut App) {
        let account = self.snapshot.current.as_ref();
        let view = account.and_then(|a| self.usage.get(&a.id));
        let mut status = tray::status_for(account, view, self.usage_loading, self.now);
        if self.proxy.is_none() {
            status.lines.push("Proxy niedostępne".into());
        }
        tray::update(status, cx);
    }
    fn refresh_usage(&mut self, cx: &mut Context<Self>) {
        #[cfg(debug_assertions)]
        if std::env::args().any(|a| a == "--demo-usage") {
            return;
        }
        if self.usage_loading || self.busy {
            return;
        }
        self.usage_next = std::time::Instant::now() + std::time::Duration::from_secs(60);
        let Some(proxy) = &self.proxy else {
            return;
        };
        let ids = self
            .snapshot
            .accounts
            .iter()
            .map(|a| a.id.clone())
            .collect::<Vec<_>>();
        self.usage.retain(|id, _| ids.contains(id));
        if ids.is_empty() {
            return;
        }
        self.usage_loading = true;
        self.usage_generation += 1;
        let generation = self.usage_generation;
        let job = proxy.usage(ids);
        let abort = job.abort_handle();
        // Aborting the Tokio future also stops pending network reads when the view closes.
        struct AbortOnDrop(tokio::task::AbortHandle);
        impl Drop for AbortOnDrop {
            fn drop(&mut self) {
                self.0.abort();
            }
        }
        let guard = AbortOnDrop(abort);
        self.usage_task = Some(cx.spawn(async move |entity, cx| {
            let _guard = guard;
            let result = job.await;
            let _ = entity.update(cx, |this, cx| {
                if generation != this.usage_generation {
                    return;
                }
                this.usage_loading = false;
                this.usage_next = std::time::Instant::now() + std::time::Duration::from_secs(60);
                match result {
                    Ok(results) => {
                        for (id, result) in results {
                            if !this.snapshot.accounts.iter().any(|a| a.id == id) {
                                continue;
                            }
                            let view = this.usage.entry(id).or_default();
                            match result {
                                Ok(data) => {
                                    view.data = Some(data);
                                    view.checked = Some(chrono::Utc::now());
                                    view.error = None;
                                }
                                Err(error) => view.error = Some(error),
                            }
                        }
                    }
                    Err(_) => {
                        for account in &this.snapshot.accounts {
                            this.usage.entry(account.id.clone()).or_default().error =
                                Some("Odczyt limitów został przerwany.".into());
                        }
                    }
                }
                cx.notify();
            });
        }));
        cx.notify();
    }
    fn reset_account(&mut self, id: String, credit_id: Option<String>, cx: &mut Context<Self>) {
        if self.busy || self.usage_loading {
            return;
        }
        let Some(proxy) = &self.proxy else {
            return;
        };
        #[cfg(debug_assertions)]
        if std::env::args().any(|a| a == "--demo-usage") {
            self.status = "Tryb demonstracyjny: żaden restart nie został wykorzystany.".into();
            cx.notify();
            return;
        }
        let data = self.usage.get(&id).and_then(|v| v.data.as_ref());
        if !data.is_some_and(|d| d.pending_reset)
            && data
                .and_then(|d| d.reset_details.as_ref())
                .and_then(|d| d.next(chrono::Utc::now()))
                .is_none_or(|c| Some(&c.id) != credit_id.as_ref())
        {
            self.status = "Lista restartów zmieniła się lub restart wygasł. Odśwież dane i potwierdź ponownie.".into();
            self.error = true;
            self.refresh_usage(cx);
            cx.notify();
            return;
        }
        self.busy = true;
        self.error = false;
        self.status = "Wykorzystywanie restartu…".into();
        if let Some(data) = self.usage.get_mut(&id).and_then(|v| v.data.as_mut()) {
            data.pending_reset = true;
        }
        let job = proxy.reset(id.clone(), credit_id);
        self.task = Some(cx.spawn(async move |entity, cx| {
            let result = job.await;
            let _ = entity.update(cx, |this, cx| {
                this.busy = false;
                match result {
                    Ok(Ok(outcome)) => {
                        this.status = outcome.message().into();
                        this.error = false;
                        // A reset invalidates percentages; never display a fabricated 0%.
                        this.usage.remove(&id);
                    }
                    _ => {
                        this.status = match result {
                            Ok(Err(e)) => format!("Restart niepotwierdzony: {e}").into(),
                            _ => "Restart niepotwierdzony. Ponowienie zachowa identyfikator operacji.".into(),
                        };
                        this.error = true;
                        if let Some(view) = this.usage.get_mut(&id) {
                            view.error = Some("Wynik restartu wymaga sprawdzenia.".into());
                        }
                    }
                }
                this.refresh_usage(cx);
                cx.notify();
            });
        }));
        cx.notify();
    }
    fn work(
        &mut self,
        message: &'static str,
        success: &'static str,
        operation: impl FnOnce(Store) -> anyhow::Result<Snapshot> + Send + 'static,
        cx: &mut Context<Self>,
    ) {
        if self.busy {
            return;
        }
        self.busy = true;
        self.error = false;
        self.status = message.into();
        let store = self.store.clone();
        let background = cx
            .background_executor()
            .spawn(async move { operation(store) });
        self.task = Some(cx.spawn(async move |entity, cx| {
            let result = background.await;
            let _ = entity.update(cx, |this, cx| {
                this.busy = false;
                this.login_pending = false;
                match result {
                    Ok(snapshot) => {
                        this.expanded_accounts
                            .retain(|id| snapshot.accounts.iter().any(|a| &a.id == id));
                        this.snapshot = snapshot;
                        this.sync_tray(cx);
                        this.refresh_usage(cx);
                        this.status = success.into();
                        this.error = false;
                    }
                    Err(e) => {
                        this.status = e.to_string().into();
                        this.error = true;
                    }
                }
                cx.notify();
            });
        }));
        cx.notify();
    }
}
fn main() {
    let store = match Store::discover() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1)
        }
    };
    let mut args = std::env::args_os().skip(1);
    let first = args.next();
    if first.as_ref().is_some_and(|arg| arg == "--proxy-token") {
        let result = (|| -> anyhow::Result<()> {
            let path = args
                .next()
                .map(std::path::PathBuf::from)
                .ok_or_else(|| anyhow::anyhow!("Brak ścieżki połączenia."))?;
            let c: proxy::Connection = serde_json::from_slice(&std::fs::read(path)?)?;
            println!("{}", c.token);
            Ok(())
        })();
        if result.is_err() {
            eprintln!("Nie można odczytać połączenia proxy.");
            std::process::exit(1)
        }
        return;
    }
    if first.as_ref().is_some_and(|arg| arg == "--cli") {
        if let Err(e) = launcher::run_cli(&store, args) {
            eprintln!("{e}");
            std::process::exit(1)
        }
        return;
    }
    let mut snapshot = store.snapshot();
    let proxy = proxy::Proxy::start(store.clone());
    let command = if proxy.is_ok() {
        match launcher::install_launcher(&store) {
            Ok(path) => launcher::shell_quote(&path.to_string_lossy()),
            Err(e) => {
                snapshot = Err(e);
                String::new()
            }
        }
    } else {
        String::new()
    };
    let app = gpui_kit::application().with_assets(gpui_kit::assets::Assets);
    app.on_reopen(tray::show);
    app.run(move |cx| {
        gpui_kit::init(cx);
        let mode = if matches!(
            cx.window_appearance(),
            WindowAppearance::Dark | WindowAppearance::VibrantDark
        ) {
            ThemeMode::Dark
        } else {
            ThemeMode::Light
        };
        palette::change(mode, None, cx);
        cx.on_window_closed(|cx, _| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
        let tray_available = match tray::install(cx) {
            Ok(()) => true,
            Err(error) => {
                eprintln!("Nie udało się utworzyć ikony paska menu: {error}");
                false
            }
        };
        let bounds = Bounds::centered(None, size(px(940.), px(780.)), cx);
        cx.spawn(async move |cx| {
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                window_min_size: Some(size(px(800.), px(640.))),
                titlebar: Some(TitlebarOptions {
                    title: Some("Codex Sub Switcher".into()),
                    ..Default::default()
                }),
                ..Default::default()
            };
            if let Err(e) = cx.open_window(options, |window, cx| {
                if tray_available {
                    window.on_window_should_close(cx, |_, cx| {
                        tray::hide(cx);
                        false
                    });
                }
                let view = cx.new(|cx| Switcher::new(store, snapshot, proxy, command, cx));
                cx.new(|cx| Root::new(view, window, cx))
            }) {
                eprintln!("Nie udało się otworzyć okna: {e}");
            }
        })
        .detach();
    });
}
