# AGENT-MEMORY.md — Pamięć Indywidualna i Rojowa

Wydajna symulacja roju mrówek opiera się na dwóch typach pamięci: **indywidualnej pamięci krótkotrwałej każdego agenta** oraz **współdzielonej pamięci środowiskowej (rojowej)**, pełniącej rolę cyfrowego odpowiednika śladów feromonowych.

---

## 1. Pamięć Indywidualna (Internal Short-term Memory)

Każda mrówka posiada komponent `AntMemory`, implementujący **bufor cykliczny (Ring Buffer)** o stałym rozmiarze $N = 10$.

### Struktura i mechanizm działania:
Komponent przechowuje tablicę 10 ostatnich pozycji 2D agenta oraz indeks wskazujący najstarszy wpis:
```rust
pub struct AntMemory {
    pub positions: [Vec2; 10], // Stały rozmiar kolejki FIFO
    pub index: usize,          // Wskaźnik na najstarszy element (nadchodzący do nadpisania)
}
```

### Aktualizacja pamięci w każdym kroku symulacji:
Przed wykonaniem ruchu, aktualna pozycja mrówki $P_{nowa}$ jest zapisywana na pozycji `index` w buforze:
1. Odczytujemy najstarszą zapisaną pozycję: $P_{oldest} = positions[index]$. Ta pozycja staje się naszym **punktem odniesienia kierunku powrotnego** (`oldestPositionRemembered`).
2. Nadpisujemy najstarszą pozycję nowym wpisem: $positions[index] = P_{nowa}$.
3. Inkrementujemy indeks kołowy: `index = (index + 1) % 10`.

### Rola w wytyczaniu kierunku powrotnego:
Gdy mrówka porusza się w środowisku, pozostawia za sobą ślad feromonowy wskazujący kierunek, z którego przybyła. Zamiast wskazywać bezpośrednio sąsiednią klatkę (co powodowałoby lokalne pętle ruchowe), mrówka zapisuje w komórce siatki pozycję $P_{oldest}$ (sprzed 10 klatek).
Kiedy inna mrówka idzie po tym śladzie feromonowym, kieruje się w stronę $P_{oldest}$, co gwarantuje płynne podążanie wzdłuż makroskopowej ścieżki i eliminuje lokalne fluktuacje kierunku.

---

## 2. Pamięć Rojowa (Shared Swarm/Grid Memory)

Środowisko jest podzielone na dwuwymiarową siatkę komórek `GridCell`. Zamiast ciągłych wartości feromonów, które parują i dyfundują (co jest kosztowne obliczeniowo), zastosowano system **feromonów czasowych**.

Każda komórka siatki przechowuje informacje o tym, kiedy ostatnio widziano w jej pobliżu jedzenie (`Food`) lub jaskinię (`Cave`):
*   `last_time_seen` (timestamp) — numer klatki symulacji, w której przechodząca mrówka widziała dany cel.
*   `where_pos` (Vec2) — współrzędne najstarszej zapamiętanej pozycji tej mrówki ($P_{oldest}$), wskazujące kierunek powrotny do źródła (jaskini lub jedzenia).

Gdy inna mrówka przechodzi obok, skanuje sąsiednie komórki (obszar 3x3) i szuka najwyższej (najnowszej) wartości `time` dla celu, który aktualnie próbuje zlokalizować. Jeśli znajdzie świeższy ślad niż jej własny `max_time_seen`, kieruje się w stronę zapisanego w komórce wektora `where_pos`.

---

## 3. Wzorce Współbieżności: Lock-Free Atomic Grid

W symulacji z udziałem **100 000 agentów** uruchamianej na wielu rdzeniach procesora, jednoczesny odczyt i zapis do tych samych komórek siatki powoduje kolizje i wyścigi danych (data races). Tradycyjne blokady (np. `Mutex` czy `RwLock` dla każdej komórki) generują ogromny narzut (contention) i rujnują wydajność (powodując blokowanie wątków).

### Rozwiązanie: Bit-Packing i Instrukcje Atomowe (Lock-Free)

Aby zapewnić stuprocentowe bezpieczeństwo wątkowe przy zerowym narzucie synchronizacji, dane feromonowe dla każdego celu są upakowane w jedną liczbę **64-bitową (`u64`)** i zarządzane za pomocą atomowego typu **`AtomicU64`**.

```rust
// Upakowana struktura feromonu w AtomicU64:
// [ 16 bitów: Y ] [ 16 bitów: X ] [ 32 bity: Czas (frame_number) ]
// 63           48 47           32 31                             0
```

Dzięki temu odczyt i aktualizacja feromonu odbywa się w pojedynczej, niepodzielnej operacji na poziomie procesora:

#### 1. Odczyt (Lock-free Read):
Każdy wątek skanujący siatkę odczytuje wartość za pomocą operacji `load(Ordering::Relaxed)`:
```rust
let packed = cell.food_pheromone.load(Ordering::Relaxed);
let time = (packed & 0xFFFF_FFFF) as u32;
let x = ((packed >> 32) & 0xFFFF) as u16;
let y = ((packed >> 48) & 0xFFFF) as u16;
```
Następnie współrzędne `x` i `y` są dekodowane z powrotem na pozycję świata `Vec2`.

#### 2. Bezpieczny Zapis (Lock-free Write-Conditional):
Gdy mrówka chce zapisać swój ślad feromonowy, nie może po prostu nadpisać wartości, ponieważ inny wątek mógł w tej samej mikrosekundzie zapisać świeższy ślad. Używamy pętli **CAS (Compare-And-Swap)** zaimplementowanej w metodzie `fetch_update`:
```rust
let new_time: u32 = current_frame;
let (new_x, new_y) = pack_coords(ant.oldest_position_remembered);
let new_val = (new_time as u64) | ((new_x as u64) << 32) | ((new_y as u64) << 48);

cell.food_pheromone.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
    let old_time = (current & 0xFFFF_FFFF) as u32;
    // Aktualizujemy tylko wtedy, gdy nasz ślad jest młodszy (bardziej aktualny)
    if new_time > old_time {
        Some(new_val)
    } else {
        None
    }
});
```

### Korzyści z podejścia Lock-Free:
1. **Brak zakleszczeń (Deadlocks)**: Wątki nigdy nie czekają na zwolnienie blokady.
2. **Ekstremalna skalowalność**: 100 000 agentów może jednocześnie modyfikować siatkę bez wywoływania wątkowych wąskich gardeł (thread bottlenecks).
3. **Optymalizacja pamięci podręcznej**: Dwie liczby `AtomicU64` zajmują zaledwie 16 bajtów na komórkę siatki, co sprawia, że cała siatka feromonów łatwo mieści się w pamięci cache L2/L3 procesora.
