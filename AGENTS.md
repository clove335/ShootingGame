### Architecture
- No strategy so far. If I think it up, for example OOP, DDD, TDD, or whatsoever, I will Write it later

### Specification
- Environment: Terminal Emulator(Ghostty)
- Language: Rust
- Mode: just a single player mode(maybe adding multiplayer mode in future)
- Level selects: Easy, Medium, Hard, Extreme
- Objects: 
-- single player: a white Space craft
-- Opponents: 
--- Space craft with shocking green
--- Octopas (with Red) 
-- bullets:
--- player bullet: cyan ║, fires upward from the player tip
--- enemy bullet: magenta ↓, fires downward from opponents
-- power-ups (falling items, catch to activate):
--- ★ SpreadShot (yellow)  — 3-way spread fire for ~10 seconds
--- ♥ ExtraLife  (magenta) — instantly adds 1 life (max 5)
--- ! RapidFire  (cyan)    — raises on-screen bullet cap to 6 for ~10 seconds
-- player sprite: 2-row, 3-col
---   ▲      ← tip (row y)
---  /█\     ← fuselage + wings (row y+1)
--- hitbox: 3-wide × 2-tall (x±1, rows y and y+1)
- Rules:
-- if player bullets hit an opponent, the opponent disappears and the player earns points (Spacecraft: 100pts, Octopus: 150pts)
-- if enemy bullets hit the player, or an opponent reaches the player's row, the player loses 1 life
-- player loses all lives → Game Over
- Direction: 
-- A single player is located in a center buttom
-- Opponents come from up to down 
- Display total points on Left Upper Screen

### Programming Instruction
- Write programs in Rust in a Functional way(=as little as side effects for simpler maintainability), though Rust is a balanced System Programming Language that incorporates on Programming Language Designs like Immutability, Zero cost abstraction, and performance(=as fast as C language) 
- Write it and create directories in units of some modules with the responsibility, like display, calculate, lib
ex.) 
```
-- docs/
-- lib/
-- src/
    |- main.rs
    |- display.rs
    |- compute.rs
    |
... # if modules get bigger(ex.) over 10000 lines), create directories like below and divide big files into multiple files with some responsibilities
    |
    |_ display/
    |   |-
    |   |-
    |_ calculate/ or compute/
    |   |-
    |---|-
-- tests/
```
- Before add new features, make sure the below lists and add the features
    - Pull from remote repositories into main to make sure the local branches are updated
    - After pulling, merge updated diffs of accepted pull requests into develop
    - Write codes and execute tests on develop branch(updated)
    - Old rules: After you confirm that branches are latest, create branches with the unit of features
- After add new features
    - use git add(ex. git add -u, git add .) git diff, cargo test
    - make sure execute `cargo fmt --all ` locally to pass the CIs on pull requests

### Tests Strategy
- Use cargo for library management, tests (needless to say...)
- I am not sure, but you can create usual Unit tests for functions, considering

### Tests Strategy (Implemented)
- Runner: `cargo test` — Rust built-in test runner
- No external test crates or frameworks
    - use GitHub Actions for CI/CD
- Test modules: create test codes under tests/ directory
- Files covered:
    src/entities.rs   — derive-trait contract tests
    src/compute.rs    — all pure game-logic functions (primary surface)
    src/main.rs       — HeldKey struct (private; must be in-file)
    src/display.rs    — EXCLUDED (crossterm I/O side effects)
- Seeded RNG for tick() tests:
    use rand::rngs::StdRng;
    use rand::SeedableRng;
    let mut rng = StdRng::seed_from_u64(42);
  StdRng is in the default std_rng feature — no Cargo.toml change needed
- Backward-compatibility lock:
    Tests explicitly verify that move_player_left / move_player_right are the
    single source of truth for ← / A and → / D movement, and that the held-key
    refactor (HeldKey + input thread) did not alter movement semantics
- Categories:
    1. Entity contract    — Clone/PartialEq/deep-copy correctness
    2. Initialization     — init_state field values
    3. Movement           — step size (2), boundary clamps, immutability
    4. Shooting           — bullet spawn position, 3-bullet cap, mixed-owner cap
    5. Simulation (tick)  — frame counter, bullet travel, enemy interval/purge,
                            collision bounding box (3-wide × 2-tall), score, lives,
                            game-over trigger, u32 saturation
    6. HeldKey            — press/release/grace-period/key-repeat/edge cases

