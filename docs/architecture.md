# Architecture

## Module map

```mermaid
graph TD
    main["main.rs\n─────────────────\nentry point\nmenu / game loop\nDB orchestration"]
    compute["compute.rs\n─────────────────\npure state transitions\ntick() pipeline\nplayer_shoot()"]
    display["display.rs\n─────────────────\nterminal rendering\ncrossterm I/O"]
    entities["entities.rs\n─────────────────\nall data types\nEntireGameStateInfo"]
    input["input_keyboard.rs\n─────────────────\nis_held() logic\nHOLD_WINDOW / GRACE"]
    db["db.rs\n─────────────────\nSQLite via rusqlite\ntop_scores / scores"]
    lib["lib.rs\n─────────────────\nre-exports for tests\npub mod compute\npub mod display\npub mod entities\npub mod input_keyboard"]

    main -->|"calls"| compute
    main -->|"calls"| display
    main -->|"calls"| db
    main -->|"calls"| input
    compute -->|"reads/returns"| entities
    display -->|"reads"| entities
    db -->|"reads"| entities
    lib --> compute
    lib --> display
    lib --> entities
    lib --> input
```

The design enforces a strict dependency direction: **entities** has no imports from the project; **compute** only imports from **entities**; **display** only imports from **entities**; **main** wires them together.

---

## Concurrency model

Two OS threads run for the lifetime of the program.

```mermaid
sequenceDiagram
    participant Main as Main thread
    participant Input as Input thread
    participant Term as Terminal

    Main->>Term: enable_raw_mode, EnterAlternateScreen
    Main->>Term: PushKeyboardEnhancementFlags (if supported)
    Main->>Input: spawn — loop { event::read() → tx.send() }
    loop every ~33 ms
        Main->>Main: rx.try_recv() drain all pending events
        Main->>Main: apply held-key movement (is_held)
        Main->>Main: tick(state, rng) → new state
        Main->>Term: render(state, full_redraw)
        Main->>Main: sleep(remaining frame budget)
    end
    Main->>Term: LeaveAlternateScreen, disable_raw_mode
```

`crossterm::event::read()` blocks in the input thread. The main thread uses `rx.try_recv()` (non-blocking) so it never stalls on I/O.

---

## Application flow

```mermaid
flowchart TD
    start([main]) --> setup["setup terminal\nenable raw mode\nspawn input thread"]
    setup --> run["run()"]
    run --> db_open["db::open()\nload best score"]
    db_open --> menu["show_menu()"]
    menu -->|"Q / Esc"| quit([exit])
    menu -->|"1-4 select level"| load_hs["load_top_score(level)"]
    load_hs --> init["init_state(level, w, h, difficulty_best)"]
    init --> game_loop["game_loop()\nreturns quit: bool"]
    game_loop --> save_scores["upsert_top_score() — always\ninsert_score() — only if GameOver"]
    save_scores --> check_quit{"quit == true?"}
    check_quit -->|"yes (Q pressed)"| quit2([exit])
    check_quit -->|"no (GameOver or R pressed)"| menu
```

`game_loop` returns `true` when Q is pressed (exit program) and `false` when R is pressed on GameOver (return to menu). Either way `upsert_top_score` is called unconditionally in `run()` after `game_loop` returns. `insert_score` is only called when `state.status == GameStatus::GameOver`.

---

## Game loop — one frame

```mermaid
flowchart LR
    A([frame start]) --> B["drain rx.try_recv()\nall pending KeyEvents"]
    B --> C{"key kind?"}
    C -->|"Press/Repeat"| D["update key_frame map\none-shot: Space → player_shoot\nquit: Q · return-to-menu: R (GameOver only)\ntoggle: \` (debug) · G (god) · S (slow-mo)"]
    C -->|"Release"| E["defer to deferred_releases\n(processed after all Press/Repeat)"]
    D --> F["apply deferred releases\nupdate release_frame map"]
    E --> F
    F --> G["is_held() × 2 directions\n→ move_player if cooldown=0\n(guard: status == Playing)"]
    G --> H{"status ==\nPlaying?"}
    H -->|"yes"| tick_node["tick(state, rng)\n→ new state"]
    H -->|"no (GameOver)"| I
    tick_node --> I["render(out, state, full_redraw)"]
    I --> J["sleep(33ms − elapsed)\nor 132ms if slow_mo"]
    J --> A
```

---

## State — data model

```mermaid
classDiagram
    class EntireGameStateInfo {
        +Player player
        +Vec~Enemy~ enemies
        +Vec~Bullet~ bullets
        +Vec~FlameBullet~ flame_bullets
        +Vec~FirebombProj~ firebombs
        +Vec~Explosion~ explosions
        +Vec~BonusItem~ bonus_items
        +Option~BonusKind_u32~ active_power_up
        +u32 score
        +u32 high_score
        +Level level
        +GameStatus status
        +u64 frame
        +u16 width
        +u16 height
        +bool debug_mode
        +bool god_mode
        +bool slow_mo
        +u32 muzzle_flash
        +Option~String_u32~ cheer_msg
    }
    class Player {
        +i32 x
        +i32 y
        +u32 lives
    }
    class Enemy {
        +i32 x
        +i32 y
        +EnemyKind kind
    }
    class Bullet {
        +i32 x
        +i32 y
        +BulletOwner owner
    }
    class FlameBullet {
        +f32 x
        +f32 y
        +f32 vx
    }
    class FirebombProj {
        +i32 x
        +i32 y
        +u32 fuse
    }
    class Explosion {
        +i32 x
        +i32 y
        +u32 frames
    }
    class BonusItem {
        +i32 x
        +i32 y
        +BonusKind kind
    }
    class EnemyKind {
        <<enumeration>>
        Spacecraft
        Octopus
    }
    class BonusKind {
        <<enumeration>>
        SpreadShot
        ExtraLife
        RapidFire
        FlameBurst
        Firebomb
    }
    class BulletOwner {
        <<enumeration>>
        Player
        Enemy
    }
    class Level {
        <<enumeration>>
        Easy
        Medium
        Hard
        Extreme
    }
    class GameStatus {
        <<enumeration>>
        Playing
        GameOver
    }

    EntireGameStateInfo *-- Player
    EntireGameStateInfo *-- Enemy
    EntireGameStateInfo *-- Bullet
    EntireGameStateInfo *-- FlameBullet
    EntireGameStateInfo *-- FirebombProj
    EntireGameStateInfo *-- Explosion
    EntireGameStateInfo *-- BonusItem
    Enemy --> EnemyKind
    Bullet --> BulletOwner
    BonusItem --> BonusKind
    EntireGameStateInfo --> Level
    EntireGameStateInfo --> GameStatus
```

`EntireGameStateInfo` is a plain `Clone`-able struct with no methods. Every compute function takes `&EntireGameStateInfo` and returns a new `EntireGameStateInfo` via struct-update syntax (`..state.clone()`). Nothing is mutated in place inside `compute.rs`.

---

## tick() pipeline — 13 steps per frame

```mermaid
flowchart TD
    s0(["state (frame N)"])
    s0 --> s1["1 · Move standard bullets\nplayer: y−1 · enemy: y+1\ndiscard out-of-bounds"]
    s1 --> s2["2 · Move flame bullets\nx += vx · y −= 1.0 (float)\ndiscard out-of-bounds"]
    s2 --> s3["3 · Move enemies down\nevery move_interval frames\nspawn new enemy every spawn_rate frames"]
    s3 --> s4["4 · Enemies randomly shoot\n1/220 chance per enemy per frame"]
    s4 --> s5["5 · Collide: player bullets ↔ enemies\n3-wide × 2-tall AABB\nscore += 100 (Spacecraft) / 150 (Octopus)"]
    s5 --> s6["6 · Collide: flame bullets ↔ enemies\nsame AABB · float rounded to int"]
    s6 --> s7["7 · Collide: enemy bullets ↔ player\n3-wide × 2-tall AABB\nenemy reaching player row also counts\ndetection always runs; damage skipped when god_mode = true"]
    s7 --> s8["8 · Move firebombs\ny−1 every FIREBOMB_MOVE_INTERVAL=4 frames\nfuse−=1 each frame\ndetonate on: fuse=0 · y≤2 · dist²≤4 from enemy"]
    s8 --> s9["9 · Tick explosions\nframes−=1 · remove at 0\nadd new Explosion per detonation point"]
    s9 --> s10["10 · Move bonus items\ny+1 every BONUS_MOVE_INTERVAL=10 frames\ndiscard at bottom"]
    s10 --> s11["11 · Spawn bonus item\nevery BONUS_SPAWN_INTERVAL=150 frames\nrandom kind: SpreadShot/ExtraLife/RapidFire/FlameBurst/Firebomb"]
    s11 --> s12["12 · Tick active power-up\nframes−=1 · remove at 0"]
    s12 --> s13["13 · Player catches bonus items\n3-wide × 2-tall AABB\nExtraLife: +1 life (max 5)\nothers: set active_power_up = (kind, 300)"]
    s13 --> s14["Update player · score · status\nmuzzle_flash−=1 · cheer_msg logic"]
    s14 --> sN(["state (frame N+1)"])
```

---

## Weapon firing — player_shoot()

```mermaid
flowchart TD
    shoot(["player_shoot(state)"])
    shoot --> pu{"active_power_up?"}

    pu -->|"FlameBurst"| flame["push 4 FlameBullets\nvx ∈ {−1.3764, −0.3249, +0.3249, +1.3764}\n(±54° and ±18° from vertical)\nmuzzle_flash = 4"]

    pu -->|"Firebomb"| cap{"firebombs.len ≥ 2?"}
    cap -->|"yes"| noop["return state unchanged\n(no flash)"]
    cap -->|"no"| bomb["push FirebombProj\nx=player.x · y=player.y−1 · fuse=90\nmuzzle_flash = 4"]

    pu -->|"SpreadShot"| any_bullet{"any player bullet\nalready live?"}
    any_bullet -->|"yes"| noop2["return state unchanged"]
    any_bullet -->|"no"| spread["push 3 Bullets\nat x−2, x, x+2\nmuzzle_flash = 4"]

    pu -->|"RapidFire"| rapid_cap{"player bullets ≥ 6?"}
    rapid_cap -->|"yes"| noop3["return state unchanged"]
    rapid_cap -->|"no"| rapid["push 1 Bullet at x\nmuzzle_flash = 4"]

    pu -->|"None / ExtraLife"| normal_cap{"player bullets ≥ 3?"}
    normal_cap -->|"yes"| noop4["return state unchanged"]
    normal_cap -->|"no"| normal["push 1 Bullet at x\nmuzzle_flash = 4"]
```

---

## Rendering pipeline

```mermaid
flowchart TD
    render(["render(out, state, full_redraw)"])
    render --> check{"full_redraw?"}
    check -->|"true (first frame)"| full["Clear(All)\ndraw_border\ndraw_controls_hint"]
    check -->|"false (subsequent)"| partial["erase row 0 (HUD)\nfor each play-area row:\n  draw │ · blank · │\n(prevents ghost sprites\nwithout full clear)"]
    full --> dynamic
    partial --> dynamic["Always repaint dynamic content"]
    dynamic --> hud["draw_hud\nscore · hi-score · level\npower-up tag · bullet slots · lives"]
    hud --> enemies["draw_enemy × N"]
    enemies --> bonus["draw_bonus_item × N"]
    bonus --> expl["draw_explosion × N"]
    expl --> flame["draw_flame_bullet × N"]
    flame --> bombs["draw_firebomb × N"]
    bombs --> bullets["draw_bullet × N"]
    bullets --> player["draw_player\n▲ tip (yellow during muzzle flash)\n/█\\ fuselage + wings"]
    player --> cheer{"cheer_msg?"}
    cheer -->|"yes"| draw_cheer["draw_cheer centred banner"]
    cheer -->|"no"| go
    draw_cheer --> go{"GameOver?"}
    go -->|"yes"| gameover["draw_game_over overlay"]
    go -->|"no"| dbg
    gameover --> dbg{"debug_mode?"}
    dbg -->|"yes"| overlay["draw_debug_overlay\nframe · pos · counts · PU · GOD · SLOW\nhitbox dots (cyan player, red enemies)"]
    dbg -->|"no"| flush["ResetColor · MoveTo(0,h−1) · flush"]
    overlay --> flush
```

Draw order matters: explosions are painted before flame bullets, which are before standard bullets, which are before the player. This means the player sprite is never occluded by its own projectiles.

---

## Input — key-held detection

```mermaid
stateDiagram-v2
    [*] --> Idle : key never seen

    Idle --> Active : Press event\nkey_frame[key] = frame

    Active --> Active : Repeat event\nkey_frame[key] = frame\n(refreshes timestamp)

    Active --> GracePeriod : Release event\nrelease_frame[key] = frame\n(deferred to end of drain)

    GracePeriod --> Active : Press or Repeat arrives\nbefore GRACE_PERIOD (1 frame) elapses

    GracePeriod --> Expired : GRACE_PERIOD elapsed\nno new Press/Repeat

    Active --> Expired : HOLD_WINDOW (5 frames)\nwithout any Press/Repeat

    Expired --> Active : new Press event
    Expired --> Idle : (conceptually reset)
```

`is_held(key_frame, release_frame, key, frame)` returns `true` when:
- `key_frame[key]` exists AND `frame − last_press ≤ HOLD_WINDOW (5)`, AND
- either no release was recorded, OR `last_press ≥ last_release` (re-pressed after release), OR `frame − last_release ≤ GRACE_PERIOD (1)`.

The GRACE_PERIOD works around a Ghostty/Kitty-protocol quirk: pressing Space while holding a direction fires a spurious Release event for the direction key.

---

## Database schema

```mermaid
erDiagram
    top_scores {
        INTEGER id PK
        TEXT    username
        TEXT    difficulty
        INTEGER points
        TEXT    created_at
        TEXT    updated_at
        TEXT    deleted_at
    }
    scores {
        INTEGER id PK
        TEXT    username
        TEXT    difficulty
        INTEGER points
        TEXT    created_at
        TEXT    deleted_at
    }
```

`top_scores` has `UNIQUE(username, difficulty)`. The upsert uses `ON CONFLICT DO UPDATE SET points = MAX(points, excluded.points)` so it is safe to call unconditionally after every game — SQL handles the "only update if higher" logic.

`scores` is append-only history; one row per completed game regardless of rank.

`difficulty` is stored as a lowercase string (`easy` / `medium` / `hard` / `extreme`) so the DB is readable without the Rust source.

SQLite is compiled from source via `rusqlite` with the `bundled` feature — no system SQLite or C library installation is required beyond a C compiler toolchain.

---

## Difficulty parameters

| Level   | Enemy move interval (frames) | Enemy spawn rate (frames) | Effective speed at 30 FPS |
|---------|------------------------------|---------------------------|---------------------------|
| Easy    | 22                           | 130                       | ~1.4 rows/sec             |
| Medium  | 14                           | 90                        | ~2.1 rows/sec             |
| Hard    | 8                            | 55                        | ~3.8 rows/sec             |
| Extreme | 4                            | 28                        | ~7.5 rows/sec             |

Power-up duration is fixed at 300 frames (≈10 s) for all timed power-ups across all difficulties.

---

## Sprite layout and hitboxes

```
Row y:    ▲        ← player tip       (◎)   «▼»   ← enemy row 0
Row y+1: /█\       ← player fuselage  ╰─╯   ╚═╝   ← enemy row 1

Hitbox for both player and every enemy: 3 wide × 2 tall
  centre x ± 1  ×  row y and row y+1
```

Collision in `tick()` uses integer AABB: `|bx − ex| ≤ 1 && (by == ey || by == ey+1)`.

`FlameBullet` positions are `f32`; they are rounded to `i32` before the AABB check so the same integer arithmetic applies.

---

## Key constants (compute.rs)

| Constant | Value | Meaning |
|---|---|---|
| `FRAME` | 33 ms | Target frame duration (≈30 FPS) |
| `MOVE_COOLDOWN` | 0.1 | Cooldown set after each held move; decremented by 1.0/frame so it reaches 0 the same frame it's set — effectively no cooldown (moves every frame) |
| `POWER_UP_DURATION` | 300 frames | ≈10 s for all timed power-ups |
| `BONUS_SPAWN_INTERVAL` | 150 frames | ≈5 s between bonus drops |
| `BONUS_MOVE_INTERVAL` | 10 frames | Bonus falls 1 row every 10 frames |
| `MAX_LIVES` | 5 | Player lives cap |
| `MUZZLE_FLASH_DURATION` | 4 frames | ≈132 ms yellow burst at player tip |
| `CHEER_DURATION` | 90 frames | ≈3 s score-milestone banner |
| `FLAME_VX_NEAR` | 0.3249 | tan(18°) — inner FlameBurst angle |
| `FLAME_VX_FAR` | 1.3764 | tan(54°) — outer FlameBurst angle |
| `FIREBOMB_MOVE_INTERVAL` | 4 frames | Firebomb rises 1 row every 4 frames |
| `FIREBOMB_FUSE` | 90 frames | ≈3 s before auto-detonation |
| `FIREBOMB_CAP` | 2 | Max simultaneous firebombs |
| `EXPLOSION_TRIGGER_RADIUS_SQ` | 4 | r=2 — proximity auto-detonation radius² |
| `EXPLOSION_KILL_RADIUS_SQ` | 16 | r=4 — blast kill radius² |
| `EXPLOSION_DISPLAY_FRAMES` | 10 frames | ≈333 ms explosion visual |
| `HOLD_WINDOW` | 5 frames | `is_held` expiry window |
| `GRACE_PERIOD` | 1 frame | False-release suppression window |
