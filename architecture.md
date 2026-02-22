### Architecture
- No strategy so far. If I think it up, I will Write it later

### Specification
- Environment: Terminal Emulator(Ghostty)
- Language: Rust
- Mode: just a single player mode(maybe adding multiplayer mode in future)
- Level selects: Easy, Medium, Hard
- Objects: 
-- single player: a white Space craft
-- Opponents: 
--- Space craft with shocking green
--- Octopas (with Red) 
-- bullets: 
- Rules: if bullets by player hit on 
- Direction: 
-- A single player is located in a center buttom
-- Opponents come from up to down 
- Display total points on Left Upper Screen


### Programming Instruction
- Write programs in Rust in a Functional way(=as little as side effects for simpler maintainability), though Rust is a balanced System Programming Language that incorporates on Programming Language Designs like Immutability, Zero cost abstraction, and performance(=as fast as C language) 
- Write it and create directories in units of some modules with the responsibility, like display, calculate, lib
ex.) 
-- docs/
-- lib/
-- src/
    |- main.rs
    |_ display/
    |   |-
    |   |-
    |_ calculate/ or compute/
    |   |-
    |---|-
-- test/

### Tests Strategy
- Use cargo for library management, tests (needless to say...)
- I am not sure, but you can create usual Unit tests for functions, considering P

### Tests Strategy (Implemented)
- Runner: `cargo test` — Rust built-in test runner
- No external test crates or frameworks
- Test modules: `#[cfg(test)] mod tests` blocks inside each source file
  (binary crate constraint — no lib.rs, no tests/ directory)
- Files covered:
    src/entities/mod.rs   — derive-trait contract tests
    src/compute/mod.rs    — all pure game-logic functions (primary surface)
    src/main.rs           — HeldKey struct (private; must be in-file)
    src/display/mod.rs    — EXCLUDED (crossterm I/O side effects)
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


