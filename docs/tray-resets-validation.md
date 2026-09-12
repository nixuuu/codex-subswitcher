# Tray i ręczne restarty — 2026-09-10

> Screenshot assets were refreshed on September 12, 2026. This report retains its original results; see [capture details](screenshots.md).

## Zmiany

- `src/tray.rs`, `src/main.rs`: natywny status item ⇄, menu Pokaż/Schowaj/Zakończ, ⌘H/⌘Q, przechwycenie czerwonego przycisku zamknięcia, ponowne otwarcie przez lifecycle aplikacji. Ukrycie zachowuje encję okna, proxy oraz zadania limitów. Żółty przycisk pozostaje standardową minimalizacją macOS.
- `src/resets.rs`: parsowanie i sortowanie restartów, filtrowanie dostępności, lokalne daty ważności, trwały zapis idempotencji z konkretnym `credit_id`.
- `src/proxy.rs`, `src/usage.rs`: pobieranie dat i liczby restartów dla konkretnego profilu, uwierzytelniony reset na istniejącym runtime i wspólne odświeżanie OAuth.
- `src/main.rs`: daty ważności, oznaczenie następnego restartu, potwierdzenie z nazwą konta i terminem, retry operacji niepotwierdzonej, blokada nowego resetu przy braku wiarygodnych danych.
- `Cargo.toml` / `Cargo.lock`: macOS `tray-icon` 0.21.3, `muda` 0.17.2. GPUI Kit pozostaje 0.6.0, component/base 0.6.1, gpui-pre 0.3.4.

## Źródła kontraktu

Kod upstream Codex: `codex-rs/backend-client/src/client/rate_limit_resets.rs`, `rate_limit_resets_tests.rs`, `backend-client/src/types.rs` oraz `app-server/src/request_processors/account_processor/rate_limit_resets.rs`.

- GET `/backend-api/wham/rate-limit-reset-credits`: `credits`, `available_count`; rekord zawiera m.in. `id`, `reset_type`, `status`, `granted_at`, `expires_at`.
- POST `/backend-api/wham/rate-limit-reset-credits/consume`: `redeem_request_id` i jawne `credit_id`.
- Wyniki: `reset`, `nothing_to_reset`, `no_credit`, `already_redeemed`. Nieznany wynik, HTTP error lub przerwany odczyt nie usuwa zapisu idempotencji. Nie ma automatycznego powtórzenia po 500; po 401 jest najwyżej jedno odświeżenie uwierzytelnienia z tym samym żądaniem.

Wybór jest jawny: najwcześniejszy dokładny termin dostępnego `codex_rate_limits`, brak terminu na końcu, deterministyczne rozstrzygnięcie remisów. Termin miniony i nieznany typ nie kwalifikują się. UI wybiera z ostatniego odczytu; serwer ostatecznie rozstrzyga dostępność. Jeśli dane zmienią się podczas potwierdzenia, aplikacja nie podmienia wybranego ID na inne. Ponowienie zachowuje pierwotny restart również wtedy, gdy lista/aktywne konto uległy zmianie.

## Weryfikacja

- `cargo fmt --check`: OK.
- `cargo test --locked`: **27 zaliczonych, 2 pominięte** (integracja z CLI i live generation).
- `cargo clippy --locked --all-targets -- -D warnings`: OK.
- `cargo build --locked`: OK.
- `scripts/bundle.sh release`: OK, paczka release z podpisem ad hoc w `dist/Codex Sub Switcher.app`.
- Rzeczywisty GUI na syntetycznych kontach w `/tmp/switcher-tray-demo`, `--demo-usage`: jasny widok dat, ciemny dialog potwierdzenia, anulowanie. Ten tryb nie wysyła żądań restartu ani odczytów limitów.
- `docs/reset-expirations.png` i `docs/reset-confirmation.png`: rzeczywiste zrzuty działającej aplikacji.
- Po czerwonym zamknięciu i akcji Schowaj okno proces pozostawał aktywny, `/health` odpowiadał 200. Po ⌘Q proces zakończył się kodem 0, port proxy był zamknięty.
- Drzewo AX natywnego menu aplikacji potwierdziło Pokaż okno / Schowaj okno / Zakończ. Nie zweryfikowano bezpośredniego kliknięcia ikony status item ani otwierania przez Dock; SystemUIServer nie odpowiedział narzędziu GUI.
- Mock HTTP sprawdza pobranie dat dla właściwego konta, wskazanie `credit_id`, zachowanie request ID i credit ID po niejednoznacznym błędzie oraz po przełączeniu aktywnego profilu, brak replay po HTTP 500 i sanitację błędów.
- Regresje domenowe: kolejność wg czasu z uwzględnieniem stref, brak daty na końcu, pomijanie wygasłych/wykorzystanych/nieznanych, niepoprawna data, brak dostępnego kredytu, wymaganie ID nowej operacji, trwały retry i izolacja profili.
- **Nie zużyto rzeczywistego restartu.** Obsługa endpointów nie jest potwierdzona na koncie live.

## Checklista GPUI Kit

Wersje i nowe API sprawdzone w źródłach rozwiązanych zależności. Tray i jego task mają właściciela w Global; zadania UI w Switcher. Stałe ID kont i restartów. Sieć oraz zapis plików poza renderem i wątkiem UI. Jedna operacja resetu naraz, limity wstrzymane podczas resetu, trwały klucz przed POST. Odczyty ograniczone do 3 kont równolegle, 20 s/request, 1 MiB odpowiedzi i maks. 1000 rekordów dat. Sortowanie wykonywane podczas parsowania, nie w renderze. Widok ma ograniczony przewijany viewport. Brak danych i błąd szczegółów nie udają dostępnego restartu. Stare wyniki filtrowane według generacji i ID kont. Template tray nie wymaga pliku obrazka; paczka uruchomiona spoza repo.

Nie wykonano pomiarów FPS, retencji pamięci, pełnego testu dostępności, minimalnej geometrii, wielu okien ani zakończenia procesu w trakcie rzeczywistego resetu. Trwały klucz pozwala zachować identyfikację operacji po takim przerwaniu; gwarancja ponowienia po stronie serwera wynika z kontraktu idempotencji, nie testu live.
