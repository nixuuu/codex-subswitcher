# Limity kont — weryfikacja 2026-09-10

> Screenshot assets were refreshed on September 12, 2026. This report retains its original results; see [capture details](screenshots.md).

Zmiana: `src/usage.rs` (model, walidacja, etykiety), `src/proxy.rs` (odczyt kont na istniejącym runtime i wspólna blokada odświeżania OAuth), `src/main.rs` (karty, odświeżanie, stany i zwijana instrukcja), `README.md`.

## Wyniki

- `cargo fmt --check`: OK.
- `cargo test --locked`: 21 zaliczonych, 2 celowo pominięte (zainstalowany CLI i rzeczywista generacja).
- `cargo clippy --locked --all-targets -- -D warnings`: OK.
- `cargo build --locked`: OK.
- `scripts/bundle.sh release`: OK, zoptymalizowana paczka `dist/Codex Sub Switcher.app`, podpisana ad hoc.
- GUI uruchomione w osobnej paczce i katalogach `/tmp/switcher-usage-demo`, z syntetycznymi kontami i debugowym `--demo-usage`, który wyłącza pobieranie limitów z sieci.
- Sprawdzono jasny/ciemny motyw bez restartu, przewijanie, dwa okna oraz sam weekly. Rzeczywiste obrazy: `usage-light.png`, `usage-dark.png`. Drzewo dostępności zawiera nazwane wskaźniki postępu z wartościami 37, 64 i 92.
- Nie wykonano odczytu limitów rzeczywistych subskrypcji. Mock potwierdza wybór niewybranego konta przez ChatGPT-Account-Id, brak zmiany aktywnego profilu oraz sanitację błędów HTTP. Istniejące regresje OAuth i routingu nadal przechodzą.

## Kontrakt danych

Endpoint: `https://chatgpt.com/backend-api/wham/usage`. Źródło: kod upstream Codex, `codex-rs/backend-client/src/client/rate_limit_resets.rs` (`rate_limit_status_url`) oraz modele `RateLimitStatusDetails` i testy `app-server/tests/suite/v2/rate_limits.rs`. Jest to backend subskrypcji, nie publiczny kontrakt API.

Etykiety wynikają z `limit_window_seconds`, nie z pozycji primary/secondary ani nazwy planu. Brak okna nie tworzy paska 0%. Null `rate_limit` pokazuje brak udostępnionych okien, a brak całego pola lub niepoprawne liczby to błąd odpowiedzi. Terminy pochodzą z `reset_at`; po ich przekroczeniu widok czeka na dane, zamiast zerować limit. Pokazywane są główne okna konta; dodatkowe limity konkretnych modeli, kredyty i spend controls nie są częścią tego widoku.

## Checklista GPUI Kit

| Obszar | Wynik i dowód |
|---|---|
| Wersje / API | Tak: bez zmiany zależności; gpui-kit 0.6.0, component/base 0.6.1, gpui-pre 0.3.4; sprawdzone źródło Progress. |
| Stan / tożsamość | Tak: Task w Switcher, karty i paski mają ID profilu. |
| Render | Tak: bez I/O i uruchamiania zadań; formatowanie krótkich etykiet dat w widoku. |
| Async | Tak: jedna partia, numer generacji, anulowanie przy zamknięciu; wyniki usuniętych kont ignorowane. |
| Kolejki | Tak: do 3 równoległych odczytów; 20 s timeout żądania, 1 MiB odpowiedzi; cache tylko bieżących profili. |
| Dane | Tak dla małej listy zapisanych kont; bez wirtualizacji i benchmarku dużego zbioru. |
| Layout | Tak: skończony przewijany obszar, karty nie kurczą się; instrukcja zwijana. Minimalny rozmiar i powiększenie nieprzetestowane. |
| Motyw | Tak: oba motywy i przełączenie na działającym oknie. |
| Wejście | Częściowo: kliknięcie i przewijanie; pełny przebieg Tab/IME/długi tekst nieprzetestowany. |
| Dostępność | Częściowo: nazwane wskaźniki w AX; bez testu czytnika ekranu. |
| Błędy | Tak w kodzie/modelu: loading, brak okien, błąd, zachowany nieaktualny odczyt; nie wszystkie stany sprawdzone wizualnie. |
| Okna | Częściowo: własność zadań i abort przy drop; zamknięcie podczas rzeczywistego OAuth nieprzetestowane. |
| Pamięć | Cache czyszczony do zapisanych ID; bez pomiaru retencji. |
| Pomiary | Nie dotyczy: brak deklaracji FPS i optymalizacji wydajności. |
| Regresje | Tak: sam weekly, oba okna, brak/błędne dane, miniony reset, odczyt innego profilu, sanitacja błędu. |
| Wydanie | Uruchomiono osobną paczkę debug spoza repo; release zbudowany i podpisany przez scripts/bundle.sh. |
| Dowody | Wyniki lokalne i granice testu live jawnie rozdzielone. |
