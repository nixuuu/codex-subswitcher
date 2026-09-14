# Plan wdrożenia poprawek designu — instrukcja dla agenta

## Zadanie i rezultat

W repozytorium `/Users/nix/GIT/codex-subswitcher` wdroż poprawki z [review z 13 września 2026](design-review-2026-09-13.md), według poniższych pakietów. Dokument jest samodzielnym punktem wejścia do implementacji. Powstał na podstawie kodu `58ef555`; przed zmianami sprawdź bieżący stan, bo numery linii w review mogą być nieaktualne.

Rezultat: czytelny kompaktowy panel do porównania limitów i aktywowania konta, jednoznaczne potwierdzenia i blokady, skalowalne kontrolki GPUI, łatwo dostępne instrukcje połączenia oraz jawny wybór motywu. Zachowaj istniejący podział na panel menu bar i osobne Settings. UI pozostaje po angielsku.

Ten dokument nie jest dowodem wykonania zmian ani zgodą na E2E, operacje na rzeczywistych kontach, commit lub push. Implementację podejmij po przekazaniu przez użytkownika zadania jej wykonania.

## Instrukcje i skille

1. Przeczytaj obowiązujące `AGENTS.md` oraz `/Users/nix/.codex/RTK.md`. Sprawdź `rtk git status --short`; zachowaj cudze i niezacommitowane zmiany, w tym oba dokumenty tego przeglądu. Do edycji używaj `apply_patch`, do poleceń powłoki prefiksu `rtk`.
2. **Obowiązkowo `gpui-kit-design-guides`:** [SKILL.md](../.agents/skills/gpui-kit-design-guides/SKILL.md), następnie cały [Design Guides](../.agents/skills/gpui-kit-design-guides/references/design-guides.md). Praca obejmuje kilka powierzchni, więc nie wystarczy sekcja o przyciskach. Na zakończenie przejdź Design review checklist i Accessibility checklist.
3. **Obowiązkowo `gpui-kit`:** [SKILL.md](../.agents/skills/gpui-kit/SKILL.md), [Coding Guides](../.agents/skills/gpui-kit/references/coding-guides.md) oraz [conventions](../.agents/skills/gpui-kit/references/conventions.md). Szczególnie: theme/styling, rendering, state ownership, stable identity, focus/actions, layout/scrolling i testing. Przy ingerencji w konkretny mechanizm doczytaj wskazany w skillu plik referencyjny.
4. Sprawdź sygnatury używanych API w wersjach rozstrzygniętych przez `Cargo.lock`. Review odnotowało facade 0.6.0 i Component/Base 0.6.1; zweryfikuj to aktualnie. Dokumentacja online nie uprawnia do użycia metody nieobecnej w tej wersji. Nie aktualizuj zależności tylko po to, by dopasować przykład.
5. Inne skille dobieraj warunkowo: `gpui-kit-desktop`, jeśli potrzebne jest profilowanie/debugowanie mechaniki desktopowej. W tym zakresie nie ma potrzeby stosowania webowego `design`, `agent-browser`, `electron`, imagegen ani skilli CSS transitions. To natywne GPUI. Zachowaj istniejący motion; jeśli zakres zostanie rozszerzony o zmianę animacji, dobierz materiały do natywnej implementacji. Nie uruchamiaj subagentów wyłącznie dlatego, że dokument jest przeznaczony dla innego agenta.

## Mapa odpowiedzialności

| Plik | Zakres zmiany |
| --- | --- |
| `src/ui/components.rs` | Wspólne rozmiary i warianty akcji, styl potwierdzeń, kompozycje odstępów/paneli. |
| `src/ui/tokens.rs`, `src/palette.rs` | Skala zawartości, theme radius, uzasadnione wyjątki fizycznej geometrii; wybór motywu bez nowej palety. |
| `src/ui/account.rs` | Obiekt w dialogu, powody disabled, uproszczone szczegóły, stan Active, etykiety. |
| `src/ui/limits.rs` | Prosty miernik zwinięty i osobne objaśnienie budżetu w szczegółach. |
| `src/ui/chrome.rs`, `src/ui/connection.rs`, `src/ui.rs` | Toolbar, pusty stan, wejścia do importu/instrukcji, Appearance, kompozycja i skalowanie. |
| `src/windows.rs` | Tylko niezbędne dopasowanie rozmiaru do skali oraz otwarcie odpowiedniej sekcji Settings; chronić cykl życia okien. |
| `src/main.rs` | Tylko konieczne podłączenie istniejącego stanu/akcji/preferencji. Nie przenosić logiki do render. |
| `README.md`, `docs/design-system.md` | Aktualne etykiety i zachowanie po wdrożeniu. |

`accounts.rs`, `usage.rs`, `resets.rs`, `proxy.rs`, `config.rs`, `launcher.rs`, `notifications.rs` czytaj tam, gdzie trzeba zrozumieć warunek lub konsekwencję. Nie zmieniaj ich algorytmów, protokołów ani formatu danych w ramach kosmetycznej poprawki. Jeśli nowy wymóg rzeczywiście wymaga takiej zmiany, nazwij ją i uzasadnij osobno.

## Pakiet 1 — Potwierdzenia i przyczyny blokad (P1, P3)

1. **Zidentyfikuj obiekt w dialogach.** Remove ma tytuł `Remove “Account a1b2c3d4”?`, tekst o ponownym logowaniu i braku anulowania subskrypcji oraz Cancel / Remove. Używaj `display_label()`, nigdy emaila. Reset zachowuje konto, konkretny kredyt, datę wygaśnięcia i informację o nieodwracalnym koszcie. Dobierz jawny wariant destrukcyjnego zatwierdzenia przez rzeczywiste API `DialogButtonProps`; nie zmieniaj wszystkich dialogów na czerwone. Enable proxy i Hide email nie są destrukcyjne.
2. **Powody disabled wyprowadź z tych samych warunków co dostępność.** Najpierw poznaj aktualną kolejność blokad, w tym wyjątek dla pending reset. Jeśli wydzielisz funkcję wyznaczającą stan, użyj jej zarówno do disabled, jak i komunikatu, aby nie utrzymywać dwóch rozjeżdżających się interpretacji. Nie odblokowuj akcji, aby uprościć UI. Zamknięte disclosure jest technicznym warunkiem dostępności, nie komunikatem wymagającym wyświetlenia.
3. **Pokaż konkretną przyczynę blisko akcji.** Użyj poniższej macierzy; wspólne blokady mogą mieć jeden komunikat dla regionu zamiast duplikatu pod każdym przyciskiem. Nie polegaj na tooltipie disabled. Stan trwającej pracy ma tekst i standardowy wskaźnik postępu tam, gdzie pomaga zrozumieć oczekiwanie.
4. **Ujednolić nazwy etapów.** Dodaj … do Add account, Use reset, Retry reset, Remove account, Enable/Restore config.toml i otwierania oddzielnych okien. Zachowaj Activate, Copy command i końcowe zatwierdzenia bez …. Sprawdź przy okazji tooltips i etykiety dostępności; skrótów nie wymyślaj.

| Sytuacja | Oczekiwany przekaz |
| --- | --- |
| Operacja w toku / odświeżanie | Nazwać rzeczywistą operację; bez równoległego uruchomienia tej samej czynności. |
| Brak proxy | Activate zablokowane; wyjaśnić niedostępność proxy i dać wejście do informacji w Settings. Nie obiecywać naprawy samym otwarciem Settings. |
| Konto aktywne | Active pozostaje widoczne. Przy Remove: `Activate another account before removing this one`. |
| Dane resetów nieaktualne lub brak terminów | Wyjaśnić brak danych; wskazać Refresh, jeżeli to rzeczywiście właściwa ścieżka odzyskania. |
| Brak kwalifikującego się kredytu / pending reset | Rozróżnić `No eligible reset credits` od ponowienia istniejącej operacji. Retry nie może wybrać nowego kredytu. |

Odbiór: dialog sam identyfikuje obiekt i skutek; powód każdej blokady zgadza się z istniejącymi warunkami, także przy kilku równoczesnych ograniczeniach. Nie używać surowych identyfikatorów operacji w zwykłym komunikacie, jeżeli nie pomagają użytkownikowi zdecydować.

## Pakiet 2 — Skala i geometria (P1)

1. Zastąp własne `Size::Size(px(...))` i nadpisywanie wysokości standardowymi rozmiarami komponentów. `compact_action` ma dostać pełny kompaktowy Size, a nie odziedziczone 32 i zmienioną ramkę 24. Zachowaj małą gęstość toolbarów i zwykły rozmiar decyzji w dialogach.
2. Przenieś typografię i odstępy zawartości na rem/helpery skali lub spójny produktowy zestaw tokenów względnych. Promienie pobieraj z aktywnego theme. Nie zamieniaj px mechanicznie na rem bez sprawdzenia relacji z bazową czcionką Root. Nie zakładaj, że własna skala spacing zapisuje się przez `Theme::apply_semantic_tokens` — guide opisuje ograniczenie.
3. W `windows.rs` rozdziel skalowany rozmiar treści od fizycznego kotwiczenia w AppKit. Uwzględnij szerokość potrzebną przy większym tekście, ograniczenia ekranu i wysokość mierzoną z zawartości. Zachowaj ograniczenie listy i nieruchomy header/footer. Nie zmieniaj panelu w zwykłe resizable window.
4. W mierniku usuń zależność etykiet od magicznych przesunięć takich jak `ml(px(-11.))`. Powtarzalne kolumny, odstępy i ticki mają wspólnych właścicieli geometrii. Hairlines i raster/platform boundaries mogą pozostać fizyczne z komentarzem uzasadniającym wyjątek.

Odbiór: brak mieszania pełnego Size z przypadkową wysokością, brak produktowych px bez uzasadnienia, spójna hierarchia przy font size 14/16/18. Nie dodawaj użytkownikowi suwaka zoom tylko na potrzeby tej naprawy. Rzeczywistą ocenę clippingu i focus ringów zaznacz jako niewykonaną, dopóki nie zostanie sprawdzona w oknie.

## Pakiet 3 — Miernik, hierarchia i pierwsze użycie (P2)

1. **Zwarty wiersz konta:** tożsamość + Eye, widoczne Activate/Active i disclosure; niżej plan/liczba resetów, następnie raportowane limity. Dla każdego limitu nazwa, termin resetu, wyrównane do prawej `% left` i pasek dostępnej pojemności. Zachowaj 100% jako pełną dostępność, 0% jako pusty limit i istniejący próg ostrzeżenia. Nie dodawaj okna 5h lub weekly na podstawie planu subskrypcji.
2. **Budżet tygodniowy — korekta według użytkownika:** znaczniki dni z etykietami, marker bieżącego budżetu, jego wartość i odchylenie muszą być widoczne na podstawowym pasku zwiniętego konta. Nie przenoś ich wyłącznie do rozwinięcia. Pokaż `Faster than weekly budget`, `On pace with weekly budget` lub `Slower than weekly budget`, zgodnie z tolerancją ±1 pp. Etykiety dni centruj na pozycjach ticków. W szczegółach wyjaśnij `Assumes even usage across the week; not a usage forecast.` Nie zmieniaj `weekly_budget`, zaokrągleń, strefy czasowej ani semantyki resetów. Brak daty lub budżetu nie ma produkować fikcyjnych danych.
3. **Szczegóły:** sekcje budżetu i Limit resets oddzielone rytmem/separatorem; bez kolejnej karty w karcie i bez powtórzenia nazwy konta. Zachowaj listę terminów, Next to use, powód blokady i Remove na końcu. Nie ukrywaj istotnego ostrzeżenia o danych wyłącznie w disclosure.
4. **Toolbar i pusty stan:** Add account jako zwykły kompaktowy przycisk, nie primary. W pustym stanie widoczne Add account… i wejście do importu w Settings. Uniknij dwóch identycznych Add obok siebie: przy pustej liście pomiń jego kopię w headerze. Przejście do importu ma otworzyć/pokazać sekcję, a nie od razu importować poświadczenia. Wykorzystaj istniejącą akcję importu, nie twórz drugiej implementacji.
5. **Instrukcje terminala:** dodaj dyskretny `Connect terminal…` w obszarze statusu panelu, otwierający Settings z rozwiniętymi instrukcjami. Sprawdź zarówno pierwsze otwarcie, jak i już istniejące okno. Copy command i Enable config pozostają oddzielnymi decyzjami. Wyjaśnij scope wszystkich terminali używających proxy i zastosowanie od kolejnego żądania; `Proxy active` nie oznacza wykrytego połączenia terminala.

Odbiór: procenty i aktualne konto czytelne bez rozwijania; użytkownik znajdzie import/połączenie bez szukania w kołach zębatych; żadna nawigacja nie wykonuje importu ani zmiany konfiguracji. Dane błędne lub nieaktualne nadal zachowują ostatni wynik wraz z ostrzeżeniem.

## Pakiet 4 — Appearance (P2)

Zastąp Toggle theme standardową kontrolką wyboru z widocznym zaznaczeniem Light / Dark, podpisaną Appearance. Dla krótkiej listy preferuj RadioGroup albo właściwy natywny komponent segmentowy obsługiwany przez zainstalowaną wersję. Nie buduj pseudo-selecta z divów. Zmiana ma od razu obejmować panel, Settings i nakładki, również po ich ponownym otwarciu.

System jest wariantem warunkowym: oferuj go tylko wtedy, gdy istniejąca integracja zapewni obserwowanie zmian wyglądu macOS, a nie wyłącznie odczyt przy uruchomieniu. Jeśli wymagałby nowego mechanizmu poza małym zakresem, ukończ Light / Dark i opisz brak System. Nie rozszerzaj zadania o nowy format preferencji lub zapamiętywanie motywu między uruchomieniami, jeśli produkt dotychczas tego nie robił.

Odbiór: wybrany tryb jest jednoznaczny bez klikania; wyświetlana opcja zgadza się ze stanem i nie rozjeżdża między oknami. Nie zmieniaj palety kolorów bez konkretnego powodu wynikającego z kontrastu lub semantyki.

## Granice, których nie naruszać

- Prywatność: nazwy `Account …` pozostają domyślne. Eye jest świadomym ujawnieniem; zamknięcie ukrywa email. Nie przenoś adresów do logów, toastów, screenshotów ani tray.
- Resety: zachowaj dokładny `credit_id`, termin, sprawdzenie aktualności przy zatwierdzaniu i ten sam operation ID przy retry. Nie resetuj liczników lokalnie po czasie ani optymistycznie po kliknięciu. Demo też sprawdź przed użyciem — nie zakładaj, że każdy przycisk ma bezpieczny stub.
- Proxy i logowanie: nie zmieniaj routingu, credential refresh, globalnego zakresu aktywnego konta ani zachowania trwającego żądania. Bez realnego sign-in, importu, aktywacji, usunięcia lub zużycia kredytu podczas walidacji.
- Okna: zachowaj wspólny model także po zamknięciu okien, panel blur, zakotwiczenie, ograniczenie do widocznego ekranu, znikanie przy focus loss i kolejność Escape. Samo zamknięcie Settings nie może zatrzymywać proxy. Nie ujawniaj emaila podczas fade-out.
- Framework: Root pozostaje właścicielem warstw, ważne akcje mają ścieżkę klawiaturową, stabilne ID pochodzą od obiektu. Bez I/O w render, arbitralnych kolorów w widokach i odtwarzania bibliotecznych zachowań ręcznie.

## Weryfikacja i zakończenie

1. Przeczytaj bieżącą konfigurację testów przed uruchomieniem. Dozwolone są jednostkowe, lint i formatowanie. Użyj `rtk cargo fmt --check` oraz `rtk cargo clippy --locked --all-targets -- -D warnings`; dla jednostkowych dobierz filtry po sprawdzeniu klasyfikacji testów. Nie uruchamiaj bezrefleksyjnie całego `cargo test`, jeśli obejmuje integracyjne scenariusze. Nie uruchamiaj build/bundle, testów ignored/live, GUI integration ani E2E bez odpowiedniej zgody zgodnie z aktualnymi instrukcjami użytkownika.
2. Dodaj jednostkowe tylko dla rzeczywistych reguł zmienionych/wydzielonych w tym zadaniu: spójność przyczyny blokady i dostępności, obiekt potwierdzenia przy zmianie konta, brzegowe klasyfikacje budżetu jeżeli zmieniono ich prezentacyjne mapowanie. Nie pisz testów kopiujących buildery, konkretne odstępy lub całą treść widoku. Popraw błędy wynikające ze zmian i powtórz tylko potrzebne sprawdzenia.
3. Najpierw ukończ kod, dokumentację i dozwolone sprawdzenia. Dopiero potem, jeśli nie ma uprzedniej zgody, zapytaj o wykonanie przygotowanej walidacji E2E na izolowanych danych demo. Źródłem tej granicy są instrukcje użytkownika: „Testy E2E wymagają uprzedniego zapytania użytkownika i otrzymania zgody”. Brak zgody nie blokuje pozostałego zakresu; oznacz walidację okna jako oczekującą, nie jako zaliczoną.
4. Po zgodzie i sprawdzeniu dostępnych narzędzi użyj natywnego Computer Use; nie zakładaj dostępności na podstawie starej sesji. Sprawdź macierz poniżej. Generowanie demo wykonuj istniejącym `scripts/demo-data.py` z osobnymi `CODEX_SWITCHER_HOME` i `CODEX_HOME`, po przeczytaniu jego opcji; nie twórz własnego zamiennika. Upewnij się, że brak realnej sieci i zużycia kredytów dla sprawdzanych akcji. Jeśli brak bezpiecznej obsługi mutacji w demo, nie klikaj finalnych potwierdzeń.
5. Zaktualizuj README i design-system zgodnie z faktycznym rezultatem. Historycznych raportów nie nadpisuj nową datą; dodaj osobny raport walidacji z datą, stanem kodu, poleceniami, wynikami i ograniczeniami. Odśwież screenshots tylko jeśli wykonano aktualny capture. Przejdź obie checklisty designu i checklistę implementacji GPUI. W zakończeniu podaj zmiany, wyniki dozwolonych sprawdzeń, niewykonane punkty i ewentualną decyzję o System. Bez commit/push, jeśli użytkownik ich nie zlecił.

| Obszar walidacji okna | Przypadki i oczekiwany wynik |
| --- | --- |
| Dane i stany | 0/1/6 kont; jedno/dwa/brak okien limitów; brak proxy, loading, błąd, stare dane; brak/expired/pending reset. Brak utraty kontekstu i nieuzasadnionego odblokowania. |
| Układ | Panel, rozwinięcia, Settings przy minimalnym rozmiarze; font 14/16/18; długie etykiety/komunikaty. Bez nakładania, bez clippingu, stabilne kolumny, scrollbar na krawędzi swojego regionu. |
| Klawiatura | Tab/Shift-Tab, standardowa aktywacja kontrolki, Cmd+,, Escape w dialogu i potem panelu. Focus widoczny, wraca do triggera/logicznego miejsca; zamknięte szczegóły nie przyjmują focusu. |
| Wygląd i ruch | Light/Dark na jasnym/ciemnym tle pulpitu; kontrast szkła, selected/hover/disabled/focus; Reduce Motion; przerwanie/rewers istniejącej animacji. |
| Cykl życia | Otwarcie istniejącego/nowego Settings na żądanej sekcji; panel znika przy utracie focusu, model nadal działa. Pomiar wspólnych krawędzi w reprezentatywnych skalach ekranu, nie ocena „na oko”. |

## Gotowy prompt przekazania

> Wdróż poprawki designu w `/Users/nix/GIT/codex-subswitcher` według `docs/design-fixes-handoff.md` i ustaleń `docs/design-review-2026-09-13.md`. Użyj obowiązkowo skilli `gpui-kit-design-guides` oraz `gpui-kit`, czytając wskazane przewodniki. Zachowaj granice funkcjonalne i prywatność, ukończ pakiety 1–4, zaktualizuj dokumentację i wykonaj dozwolone testy jednostkowe, lint oraz formatowanie. Nie uruchamiaj E2E bez mojej osobnej zgody; nie wykonuj operacji na prawdziwych kontach, commit ani push. Raportuj rzeczywiste wyniki i pozostałe ograniczenia.
