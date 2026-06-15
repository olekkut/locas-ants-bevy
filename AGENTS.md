# AGENTS.md — Specyfikacja Agentów w ECS

Dokument ten opisuje architekturę agentów (mrówek) w nowym, wydajnym porcie symulatora roju napisanym w języku **Rust** przy użyciu silnika **Bevy Engine**. Przejście z tradycyjnego paradygmatu obiektowego (LÖVE/Lua) na paradygmat **ECS (Entity Component System)** pozwala na optymalne wykorzystanie pamięci podręcznej CPU (cache locality) oraz pełną wielowątkowość.

---

## 1. Architektura mrówki jako "Entji" (Entity)

W paradygmacie ECS mrówka nie jest obiektem klasy. Jest to wyłącznie unikalny identyfikator (`Entity`), do którego przypięty jest zestaw płaskich struktur danych pozbawionych logiki — **Komponentów (Components)**. 

Rozbicie mrówki na niezależne komponenty pozwala systemom Bevy na wykonywanie zapytań (Queries) tylko do tych danych, które są w danym momencie niezbędne, znacznie redukując narzut transferu pamięci (bandwidth) i umożliwiając automatyczne zrównoleglenie przetwarzania (wielowątkowość) na procesorach wielordzeniowych.

---

## 2. Komponenty opisujące Mrówkę

Definiujemy następujące komponenty w pliku `components.rs` (przypisane do każdej encji mrówki):

| Komponent | Typ | Opis |
| :--- | :--- | :--- |
| `Position` | `struct Position(pub Vec2)` | Aktualne współrzędne 2D mrówki w świecie gry. |
| `Direction` | `struct Direction(pub Vec2)` | Znormalizowany wektor kierunku ruchu (długość = 1.0). |
| `Speed` | `struct Speed(pub f32)` | Bieżąca prędkość mrówki. |
| `AntState` | `struct AntState { pub looking_for: Target, pub last_time_seen_food: i64, pub last_time_seen_cave: i64, pub max_time_seen: i64 }` | Maszyna stanów mrówki: co aktualnie szuka (`Target::Food` lub `Target::Cave`) oraz metryki czasu (w klatkach) ostatniego kontaktu z celami. |
| `AntMemory` | `struct AntMemory { pub positions: [Vec2; 10], pub index: usize }` | Cykliczny bufor (Ring Buffer) przechowujący ostatnie N (10) pozycji mrówki w celu wyznaczania kierunku powrotnego feromonów. |
| `PheromoneCooldown` | `struct PheromoneCooldown { pub write_blocked_until: u64 }` | Licznik klatek (timestamp) określający, do kiedy mrówka ma zablokowany zapis feromonów (cooldown po znalezieniu celu). |

### Definicja struktur w Rust:
```rust
#[derive(Component)]
pub struct Position(pub Vec2);

#[derive(Component)]
pub struct Direction(pub Vec2);

#[derive(Component)]
pub struct Speed(pub f32);

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Target {
    Food,
    Cave,
}

#[derive(Component)]
pub struct AntState {
    pub looking_for: Target,
    pub last_time_seen_food: i64,
    pub last_time_seen_cave: i64,
    pub max_time_seen: i64,
}

#[derive(Component)]
pub struct AntMemory {
    pub positions: [Vec2; 10],
    pub index: usize,
}

#[derive(Component)]
pub struct PheromoneCooldown {
    pub write_blocked_until: u64,
}
```

---

## 3. Cykl Decyzyjny i Systemy Bevy

W Bevy logika zachowań realizowana jest przez **Systemy (Systems)** — zwykłe funkcje w języku Rust, uruchamiane w pętli aplikacji. Systemy działają na zapytaniach (`Query`), co pozwala Bevy na ich automatyczne i w pełni bezpieczne zrównoleglenie na wątkach roboczych (worker threads).

### A. Fizyka Ruchu i Kolizji (`movement_system`)
Ten system działa w oparciu o zapytanie:
`Query<(&mut Position, &mut Direction, &mut Speed, &mut AntMemory)>`
Wykonuje on następujące operacje (wątki dzielą mrówki między siebie za pomocą Bevy `par_iter_mut()`):
1. **Zapis pozycji do pamięci podręcznej**: Aktualna pozycja mrówki jest zapisywana do bufora cyklicznego `AntMemory`.
2. **Aktualizacja prędkości**: Do prędkości dodawany jest wektor przyspieszenia (losowy dla każdej mrówki przy inicjalizacji) pomnożony przez tarcie (prędkość dąży do maksymalnej prędkości `antMaxSpeed`).
3. **Fluctuacje kierunku (Erratic Movement)**: Kierunek (`Direction`) ulega lekkiej losowej rotacji o kąt z zakresu `[-antErratic/2, antErratic/2]` w celu symulacji organicznego ruchu.
4. **Rozwiązanie kolizji**: Przed wykonaniem kroku system sprawdza komórkę siatki docelowej. Jeśli komórka jest nieprzejezdna (ściana lub granica mapy), wykonywany jest algorytm ślizgania się po ścianach:
   - Obliczany jest wektor relatywny względem środka zablokowanej komórki.
   - Kierunek mrówki jest korygowany do kierunku prostopadłego, co pozwala mrówce płynnie "ślizgać się" wzdłuż przeszkody zamiast blokowania się na niej.

### B. Maszyna Stanów i Interakcja ze Środowiskiem (`cell_interaction_system`)
Ten system sprawdza, w której komórce siatki znajduje się obecnie mrówka:
1. Jeżeli mrówka szuka **Jedzenia (`Target::Food`)** i wejdzie na komórkę zawierającą jedzenie:
   - Pobiera ładunek jedzenia (`cargo`).
   - Resetuje `max_time_seen` do `0`.
   - Odwraca kierunek ruchu o 180 stopni (`direction = -direction`).
   - Zeruje prędkość (`speed = 0.0`), zmuszając agenta do ponownego przyspieszenia w nowym kierunku.
   - Przełącza stan na poszukiwanie **Jaskini (`Target::Cave`)**.
   - Nakłada blokadę zapisu feromonów na kolejne 10 klatek (`PheromoneCooldown`), aby zapobiec natychmiastowemu zanieczyszczeniu śladu powrotnego w punkcie zwrotnym.
2. Zbieżna logika zachodzi, gdy mrówka szukająca **Jaskini (`Target::Cave`)** dotrze do komórki typu Cave (oddaje cargo, przełącza stan na Food, obraca się o 180 stopni).

### C. Sensoryka i Unikanie Przeszkód (`object_avoidance_system`)
Mrówki posiadają dynamiczny mechanizm unikania przeszkód na podstawie pola widzenia (Field of View):
*   **Sensor Przedni**: Mrówka rzutuje wektor wzroku przed siebie o długości `antSightDistance`. Sprawdza w siatce przejezdność komórki docelowej.
*   **FOVs (Sensory boczne)**: Jeśli sensor przedni wykryje przeszkodę, mrówka wykonuje "skanowanie" lewego i prawego wektora, obróconych o kąt `antObjectAvoidance_FOV` (w oryginale `30 stopni` lub `pi/6` radianów).
*   **Decyzja o skręcie**: 
  - Jeśli strona lewa jest wolna, a prawa zablokowana -> mrówka skręca w lewo (`direction = direction.rotate(-FOV)`).
  - Jeśli strona prawa jest wolna, a lewa zablokowana -> mrówka skręca w prawo (`direction = direction.rotate(FOV)`).
  - Jeśli obie strony są zablokowane -> mrówka odwraca się lub szuka szerszego kąta obrotu.

Dzięki rozdzieleniu logiki sensorycznej, ruchowej i stanowej na osobne systemy ECS, kod pozostaje czysty, modularny i maksymalnie zoptymalizowany pod współczesne procesory wielordzeniowe.
