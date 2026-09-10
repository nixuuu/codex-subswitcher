# Kompaktowy widok, paleta i status tray — 2026-09-10

## Wynik

- Jeden przewijany obszar całej strony (`page`, trwały `ScrollHandle`). Nagłówek, konta i połączenie terminala przewijają się razem.
- Zwarte wiersze, subtelne separatory zamiast dużych kart; w standardowym oknie 940 × 780 mieści się sześć syntetycznych kont, bez rozwijania szczegółów.
- Przy szerokości poniżej 900 jednostek układ przechodzi do dwóch rzędów na konto. Przetestowano przeciągnięcie okna do minimalnej szerokości 800, przewijanie oraz rozwinięte szczegóły.
- Restarty: liczba, najbliższy termin, rozwijana lista wszystkich dostępnych terminów z oznaczeniem następnego oraz przyciskiem użycia. Usuwanie konta jest w szczegółach. Pełny adres konta pozostaje dostępny w szczegółach, gdy nie mieści się w wierszu.
- Czas odświeżania pokazany raz nad listą; błędy i brak aktualnych danych nadal mają komunikaty przy odpowiednich kontach.
- Tray: nazwa i plan aktywnego konta, rzeczywiście zwrócone okna limitów, procenty, terminy resetów, liczba restartów i czas odczytu. Stan pochodzi z istniejącego cache, aktualizowany także przy ukrytym oknie. Menu jest podmieniane tylko, gdy jego treść się zmieni.
- Paleta w `src/palette.rs`: jasny motyw z ciepłym szarym tłem, ciemny grafitowy, turkusowy akcent i koralowe ostrzeżenia. Tokeny GPUI Component i Base synchronizowane; start według wyglądu systemu, ręczny przełącznik zachowany.

## Sprawdzenia

- `cargo fmt --check`, `cargo test --locked`, `cargo clippy --locked --all-targets -- -D warnings`, `cargo build --locked`: OK.
- `scripts/bundle.sh release`: OK; paczka release przebudowana i podpisana ad hoc.
- Testy: **28 zaliczonych, 2 pominięte**. Nowa regresja sprawdza treść tray’a: konto, sam weekly bez wymyślonego 5h, restarty, oznaczenie nieaktualności i brak wycieku poprzednich limitów przy braku aktywnego konta.
- GUI na sześciu fikcyjnych kontach, osobna paczka i dane `/tmp/switcher-design-demo`; `--demo-usage`, bez wywołań usługi limitów i bez zużywania restartów.
- Rzeczywiste zrzuty: `compact-light.png`, `compact-dark.png`, `compact-details-dark.png`, `compact-narrow.png`.
- Sprawdzono przełączenie motywu, rozwijanie szczegółów, długi adres, przewijanie całej strony i zmianę szerokości. Nie wykonano pełnego testu klawiatury/czytnika ekranu, pomiarów FPS ani pomiaru retencji pamięci.
- Dane menu tray’a sprawdzone testem; bez bezpośredniego zrzutu zaktualizowanego natywnego status menu. Integracja nadal korzysta z `tray-icon` 0.21.3 / `muda` 0.17.2.
- Obliczony kontrast sRGB: główny tekst 10,92:1 / 12,43:1, tekst pomocniczy 4,93:1 / 7,66:1, tekst przycisku głównego 5,07:1 / 7,00:1 (jasny / ciemny). To sprawdzenie tych par, nie deklaracja pełnej zgodności dostępności.

## Zakres i checklisty

Zmiany: `src/ui.rs` (widok), `src/palette.rs` (paleta), `src/main.rs` (stan rozwinięcia i przewijania, synchronizacja tray’a), `src/tray.rs` (status i regresja), README i obrazy. Sieć, zapis restartów i wybór najwcześniejszego terminu niezmienione.

GPUI Kit 0.6.0, component/base 0.6.1 i gpui-pre 0.3.4 pozostają zgodne z lockfile. Nowe API (ScrollHandle, viewport_size, Sizable, Theme::global_mut/sync_base, TrayIcon::set_menu) sprawdzone w rozwiązanych źródłach. Render nie wykonuje I/O. Stan przewijania i rozwiniętych kont ma właściciela; ID kont nie zależą od kolejności. Nakładki Root poza scrollowaniem, semantyczne kolory bez surowych wartości w widokach, niezmienione zabezpieczenia potwierdzenia restartu i usunięcia.

Zastosowane materiały skilla design: design-guidelines oraz badges, border-radius, buttons, colors, copywriting, dark-mode, description-lists, dashboards, flexbox-layout, general, headers, icons, interactivity, navigation, responsive-design, section-layout, shadows, surfaces, tables, typography, custom-fonts. Reguły CSS/HTML/Tailwind i breakpointy mobilnej strony zastosowano tylko koncepcyjnie: aplikacja pozostaje natywna na macOS, z istniejącym fontem systemowym i komponentami GPUI. Mobilna platforma nie jest celem tego projektu.
