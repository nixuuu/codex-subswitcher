# Review designu — 13 września 2026

Zakres: panel kont, Settings, limity, szczegóły konta i dialogi. Podstawa: kod z `58ef555`, dokumentacja produktu oraz cały [GPUI Kit Design Guides](../.agents/skills/gpui-kit-design-guides/references/design-guides.md). To review i plan, bez implementacji.

Szczegółowa instrukcja wdrożenia dla kolejnego agenta: [plan poprawek, wymagane skille i kryteria odbioru](design-fixes-handoff.md).

## Wniosek

Zachować podział na kompaktowy panel kont i osobne Settings. Główne zadanie to porównanie dostępnych limitów i wybór konta dla następnego żądania. To uzasadnia widoczne Activate, stan Active i limity bez otwierania szczegółów. Potrzebne są celowane poprawki komunikacji, skalowania i hierarchii, a nie nowy układ całej aplikacji.

Przegląd opiera się na źródłach. Inwentaryzacja Computer Use nie pokazała uruchomionej aplikacji. Nie uruchamiano jej ani scenariuszy E2E. README wyraźnie oznacza zrzuty z 12 września jako starszy układ; nie są dowodem jakości aktualnego panelu. Kontrast szkła, dokładność wyrównań, focus i zachowanie przy powiększeniu pozostają do sprawdzenia w aktualnym oknie.

## Ustalenia

### 1. P1 — Usunięcie konta nie identyfikuje obiektu w potwierdzeniu

Dowód: `src/ui/account.rs:122–129`. Dialog „Remove saved account?” pokazuje skutek, lecz ani tytuł, ani treść nie wskazują konta. Po otwarciu warstwa decyzyjna traci kontekst wiersza.

Poprawka: tytuł `Remove “Account a1b2c3d4”?`, zachowanie informacji o subskrypcji i ponownym logowaniu, akcje Cancel / Remove. Używać prywatnej etykiety konta, nie ujawniać emaila. Wspólny helper dialogów nie określa destrukcyjnego wariantu; jawnie dobrać go do Remove i nieodwracalnego zużycia kredytu, zamiast polegać na domyślnym wyglądzie biblioteki.

Odbiór: dla dwóch różnych kont każde potwierdzenie samodzielnie identyfikuje właściwy obiekt; Cancel nie zmienia danych. Reset nadal pokazuje konto, termin i koszt jednego kredytu. Sprawdzenie nie może zużyć rzeczywistego kredytu.

### 2. P1 — Zablokowane akcje nie podają konkretnego powodu

Dowód: `src/ui/account.rs:23–24,122,200–205`. Use reset łączy brak świeżych danych, brak kredytu i trwającą operację w jeden disabled. Remove account jest zablokowane dla aktywnego konta bez wyjaśnienia. Tooltip Activate obiecuje następne żądanie również wtedy, gdy brak proxy uniemożliwia działanie.

Poprawka: przy akcji pokazać przyczynę i możliwy następny krok, np. `Activate another account before removing this one`, `Refresh limits to use a reset`, `No eligible reset credits`. W czasie pracy użyć widocznego stanu postępu. Nie wymagać hover nad disabled, aby poznać powód. Zachować wszystkie istniejące blokady domenowe.

Odbiór: każdy powód blokady ma odpowiadającą mu treść; świeże dane, brak kredytu, aktywne konto, niedostępne proxy i operacja w toku są rozróżnialne. Powód resetu pozostaje przy przycisku w szczegółach.

### 3. P1 — Własne rozmiary w px omijają skalowanie GPUI

Dowód: `src/ui/tokens.rs:5–19`, `src/ui/components.rs:5–39`, `src/ui.rs:30`, `src/ui/limits.rs`. Stałe są scentralizowane, ale tekst, padding, wysokości i promienie są ponownie zamieniane na px. `compact_action` dziedziczy Size 32 i zmienia zewnętrzną wysokość na 24, zamiast wybrać spójny mały rozmiar komponentu.

Poprawka: standardowe i kompaktowe kontrolki oprzeć na semantycznych rozmiarach biblioteki; tekst i odstępy na rem/helperach skali, promienie na aktualnym theme. Oddzielić udokumentowaną fizyczną geometrię okna/AppKit i hairlines od geometrii zawartości. Uwzględnić skalę zawartości przy wyznaczaniu szerokości panelu.

Odbiór: przy bazowej czcionce 14, 16 i 18 tekst, ikony i hit targets skalują się razem, bez obcinania etykiet i focus ringów. Przegląd wystąpień px pozostawia tylko uzasadnione wyjątki. To potwierdzona niezgodność z zasadą rem; konkretne wizualne uszkodzenie przy zoomie nie zostało jeszcze zaobserwowane.

### 4. P2 — Tygodniowy miernik miesza pojemność z osią czasu

Dowód: `src/ui/limits.rs:83–158`. Na pasku pozostałej pojemności umieszczono dni w kolejności wstecz od Reset oraz kreskę budżetu czasowego. „Fast usage”, „pp below budget” i „Now: …% budget” wymagają poznania modelu obliczeń, którego panel nie wyjaśnia. „Fast usage” może sugerować analizę tempa, choć to tylko porównanie do liniowego budżetu.

Korekta po informacji użytkownika: w zwiniętym wierszu pozostawić limit, procent dostępny i termin resetu oraz znaczniki dni z etykietami, marker bieżącego budżetu i odchylenie. Wcześniejsze zalecenie przeniesienia skali wyłącznie do disclosure zostało wycofane. Podstawowy pasek ma od razu pokazywać, czy zużycie jest szybsze, zgodne czy wolniejsze względem równomiernego budżetu tygodnia. Objaśnienie modelu może pozostać w szczegółach. Nie zmieniać obliczeń ani znaczenia % left. Nie kodować ostrzeżenia wyłącznie kolorem.

Odbiór: podstawowy wiersz pozwala porównać dostępność bez interpretacji osi czasu. Szczegóły jasno odróżniają pojemność od czasu; daty i dni nie nakładają się przy większym tekście. Liczby porównywane między kontami mają stałą kolumnę i wyrównanie do prawej.

### 5. P2 — Hierarchia promuje dodawanie zamiast bieżącego wyboru

Dowód: `src/ui/chrome.rs:194–195` nadaje Add account primary; `src/ui/account.rs:196–205` pokazuje Activate jako zwykły przycisk. Guide wprost odradza primary dla zwykłego Add w toolbarze. Szczegóły (`account.rs:41–56`) tworzą kolejną zaokrągloną powierzchnię wewnątrz karty i powtarzają jej etykietę.

Poprawka: Add account jako zwykła kompaktowa akcja; Activate nadal widoczne bez mnożenia primary na wszystkich kontach. Zachować tekstowe Active i subtelne zaznaczenie. Szczegóły rozdzielić separatorem i odstępem zamiast kolejną kartą; usunąć powtórzenie nazwy konta tam, gdzie kontekst jest widoczny.

Odbiór: po pominięciu koloru nadal widać tożsamość, aktualne konto, dostępność i akcję. Rozwinięcie nie tworzy konkurencyjnego centrum uwagi.

### 6. P2 — Pierwsze użycie kieruje do niewidocznej ścieżki importu

Dowód: `src/ui/chrome.rs:71–80` proponuje import CLI, ale akcja znajduje się dopiero w Settings (`42–68`). Panel nie wyjaśnia też, jak po aktywacji konta połączyć terminal; instrukcje są schowane w osobnym oknie i disclosure (`src/ui/connection.rs`).

Poprawka: w pustym stanie pokazać Add account… oraz bezpośrednie przejście do importu w Settings. Dodać dyskretny, jawny punkt wejścia do instrukcji terminala. Wyjaśnić, że przełączanie dotyczy wszystkich terminali korzystających z proxy i dopiero kolejnego żądania. Nie twierdzić, że terminal jest połączony wyłącznie na podstawie uruchomionego proxy.

Odbiór: nowy użytkownik odnajduje dodanie/import i połączenie terminala bez zgadywania znaczenia koła zębatego. Obecne konta nadal mają pierwszeństwo przestrzeni.

### 7. P2 — Wybór motywu nie pokazuje dostępnych stanów

Dowód: `src/ui/chrome.rs:119–126`. Toggle theme wykonuje przełączenie light/dark, lecz nie pokazuje wyboru System ani jawnie wybranego trybu.

Poprawka: kontrolka wyboru Appearance z Light / Dark, a System po zapewnieniu rzeczywistego śledzenia ustawienia systemowego. Nie dodawać opcji, która jedynie jednorazowo odczyta wygląd macOS.

Odbiór: wybrany tryb jest widoczny bez wykonania akcji; wszystkie oferowane tryby mają jednoznaczne działanie.

### 8. P3 — Etykiety nie sygnalizują kolejnego etapu

Dowód: `src/ui/account.rs:23,122`, `src/ui/connection.rs:19`, `src/ui/chrome.rs:53,194`. Akcje otwierające dialog/okno lub wymagające dalszych danych nie mają wymaganego przez guide wielokropka.

Poprawka: m.in. `Use reset…`, `Retry reset…`, `Remove account…`, `Enable in config.toml…`, `Restore config.toml…`, `Open accounts…`, `Add account…`. Akcje wykonujące polecenie od razu, np. Activate i Copy command, pozostają bez wielokropka. Ujednolicić tooltips i dokumentację.

Odbiór: etykieta odróżnia otwarcie decyzji od wykonania polecenia; wszędzie używany jest znak ….

## Co zachować

- Panel utility i osobne Settings, stały nagłówek/stopkę oraz przewijanie samej listy kont.
- Standardowe Button, ikony z tooltipami i nazwami dostępności, semantyczne kolory z theme.
- Prywatne etykiety kont, jawne Active i procenty tekstowe niezależne od koloru.
- Zachowanie ostatnich limitów przy błędzie oraz informację o nieaktualności danych.
- Warstwy Root, istniejące skróty macOS i obsługę Reduce Motion; ich obecność w kodzie nie zastępuje testu zachowania.

## Plan realizacji

1. **Doprecyzować decyzje i blokady — ustalenia 1–2, 8.** Zmienić treści przy akcjach i dialogach w account/connection/chrome, jawnie dobrać warianty potwierdzeń. Kryteria odbioru: właściwy obiekt, koszt, powód blokady i następny krok. Jednostkowo sprawdzić mapowanie powodów, jeśli zostanie wydzielone; lint i formatowanie.
2. **Naprawić skalowanie komponentów — ustalenie 3.** Zmienić wspólne helpery i tokeny, potem ich użycia oraz zależne ograniczenia panelu. Przed implementacją przeczytać Coding Guides i dokumentację API wersji z Cargo.lock. Kryteria: spójny Size, theme radius, rem oraz jawna lista wyjątków px.
3. **Uprościć czytanie panelu — ustalenia 4–6.** Uporządkować miernik, disclosure, hierarchię toolbaru i wejście do instrukcji. Kryteria: najpierw konto i dostępność, potem akcja, szczegóły na żądanie; zachowany poziom prywatności.
4. **Doprecyzować Appearance — ustalenie 7.** Zastąpić Toggle theme kontrolką z widocznym wyborem; podjąć decyzję o System na podstawie istniejącej obsługi motywu. Zaktualizować `docs/design-system.md` oraz odpowiednie opisy w README.
5. **Zweryfikować aktualne okna.** Po osobnej zgodzie na E2E użyć izolowanych danych demo: pusta lista, jedno i sześć kont, limity nieznane/nieaktualne/wyczerpane, reset niedostępny i oczekujący, długi tekst. Sprawdzić oba motywy, jasne/ciemne tło pulpitu, skalowanie, minimalne Settings, Tab/Shift-Tab, aktywację klawiaturą, Escape i powrót focusu, przewijanie oraz Reduce Motion. Zmierzyć powtarzalne krawędzie przy reprezentatywnych skalach ekranu; odświeżyć zrzuty i raport walidacji.

## Design review checklist

| Pytanie z guide | Ocena tego review |
| --- | --- |
| 1. Czy zadanie jest jasne? | Częściowo: wybór konta tak, pierwszy import i połączenie terminala wymagają lepszego wejścia — ustalenie 6. |
| 2. Czy akcje dotrzymują obietnicy? | Wymaga poprawy: obiekt usunięcia, przyczyny blokad i semantyka budżetu — 1, 2, 4, 8. |
| 3. Czy hierarchia jest zdecydowana i oszczędna? | Kierunek właściwy, primary Add i złożoność tygodniowego wiersza do korekty — 4, 5. |
| 4. Czy można zrobić mniej, lepiej? | Tak: prostszy miernik i mniej zagnieżdżonych powierzchni — 4, 5. |
| 5. Czy struktura jest dokładna? | Wspólne helpery i scrollbar poza paddingiem są dobrym punktem wyjścia. Brak aktualnych pomiarów; nie zatwierdzono zgodności pikselowej. |
| 6. Czy używa systemu komponentów? | Częściowo: natywne Button/Progress/Root i theme tak; własne rozmiary px i radius do poprawy — 3. |
| 7. Czy działa we wszystkich stanach i ograniczeniach? | Kod obsługuje wiele stanów, ale nie wyjaśnia wszystkich blokad. Zoom, focus, kontrast szkła i długie teksty nieweryfikowane w oknie. |
| 8. Czy zadanie wykonano w rzeczywistym oknie? | Nie w tym review. Starsze raporty nie zamykają tego punktu dla aktualnego panelu. |

## Accessibility checklist

| Kryterium | Dowód lub ograniczenie |
| --- | --- |
| Każda akcja dostępna klawiaturą | Standardowe Button; istnieją Cmd+, / Cmd+Q / Cmd+H. Pełnej ścieżki nie sprawdzono. |
| Kolejność focusu zgodna z zadaniem | Do sprawdzenia w aktualnym panelu i Settings. |
| Widoczny focus i przywracanie po nakładkach | Root i obsługa Escape obecne; zachowania i clippingu nie zatwierdzono. |
| Nazwy kontrolek i tooltips ikon | Wspólny icon_action nadaje oba; Eye doprecyzowuje konto. |
| Kontrast tekstu i istotnych granic | Tokeny istnieją; dawnych pomiarów nie przenosimy na szkło z obecnego kodu. |
| Status nie tylko kolorem | Active, % left i komunikaty błędów są tekstowe; zachować. |
| Disabled i read-only rozróżnialne | Biblioteczny disabled obecny, wyjaśnienia niepełne — ustalenie 2. Brak osobnego trybu read-only w ocenianych widokach. |
| Opisy i błędy przy kontrolkach | Błędy limitów lokalne, ogólny status operacji wspólny; powody blokad do dodania lokalnie. |
| Dłuższy tekst i większa czcionka | Stałe px i pozycjonowane etykiety tygodnia wymagają poprawy i sprawdzenia — 3, 4. |
| Wygodne cele wskaźnika | Kod deklaruje 24/32 px; mieszany Size kompaktowych przycisków do korekty, rzeczywistych bounds nie zmierzono. |
