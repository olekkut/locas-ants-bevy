====================================================
  Locas-Ants Bevy - Symulacja Roju Mrowek w Rust
  Wersja 0.3.0
====================================================

WYMAGANIA SYSTEMOWE
-------------------
- Windows 10 lub nowszy (64-bit)
- Karta graficzna z obsluga DirectX 12 (np. GTX 960+, RX 470+, Intel UHD 630+)
- Min. 4 GB RAM

JAK URUCHOMIC
-------------
1. Rozpakuj caly folder "locas-ants-bevy" w dowolne miejsce.
2. Kliknij dwukrotnie plik "run.bat".
3. Symulacja uruchomi sie automatycznie.

UWAGA: Nie przenosz pliku .exe poza ten folder!
Program wymaga obecnosci folderow "assets" i "maps" obok siebie.

STEROWANIE
----------
Pasek boczny (UI):
  - FPS & Ant Count  : Statystyki symulacji na zywo
  - zoom+ / zoom-    : Powieksz/pomniejsz widok (lub kolko myszy)
  - ants+ / ants-    : Dodaj/usun 1000 mrowek
  - speed+ / speed-  : Przyspiesz/zwolnij symulacje (1x, 2x, 4x, 8x, pauza)
  - phrms            : Pokaz/ukryj slady feromonow

Narzedzia edytora (kliknij na pasku, potem rysuj na siatce):
  - pan view  : Przeciagaj widok (PPM/srodkowy przycisk myszy tez dziala)
  - block     : Rysuj sciany (nieprzejezdne)
  - grass     : Rysuj trawe (spowalnia mrowki)
  - cave      : Postaw jaskinie (dom mrowek)
  - food      : Postaw zrodlo jedzenia
  - portal    : Kliknij raz = niebieski portal, drugi raz = pomaranczowy (polaczone)
  - remove    : Usun komorki (przywroc puste pole)

Klawiatura:
  - [Spacja]  : Przelacz widocznosc feromonow

MAPY
----
Folder "maps" zawiera gotowe mapy:
  - default.txt    : Standardowa mapa
  - maze.txt       : Labirynt
  - bottleneck.txt : Waskie gardlo
  - split.txt      : Rozdzielona arena

ROZWIAZYWANIE PROBLEMOW
------------------------
1. "Czarny ekran" -> Upewnij sie, ze folder "assets" jest obok pliku .exe
2. Blad grafiki    -> Sprawdz, czy masz aktualne sterowniki GPU (DirectX 12)
3. Niska wydajnosc -> Zamknij inne programy obciazajace GPU

INFORMACJE
----------
Silnik: Bevy Engine 0.14 (ECS, Rust)
Backend graficzny: WebGPU (wgpu) z DirectX 12
Architektura: Entity Component System z wielowatkowym przetwarzaniem
