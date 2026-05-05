## Shooting game
- Written in Rust

## Indexes
- Motivation
- Gameplay
- Controls
- Autonomous Play Mode
- Installation
- License

## Motivation
- Want to create some games executed on TUI with Rust
- Want to use Claude Code for building some programs


## Gameplay

### Difficulty levels

| Level | Description |
|-------|-------------|
| `1` Easy | Very slow enemies, relaxed pace |
| `2` Medium | Balanced challenge |
| `3` Hard | Fast and relentless |
| `4` Extreme | Unforgiving — good luck |

### Player

```
  ▲      ← tip
 /█\     ← fuselage + wings
```

- Hitbox: 3-wide × 2-tall (centre ± 1 column, both rows)
- Starts with **3 lives** (max 5)

### Enemies

| Sprite | Color | Points |
|--------|-------|--------|
| `«▼» / ╚═╝` Spacecraft | Bright green | 100 pts |
| `(◎) / ╰─╯` Octopus | Red | 150 pts |

Enemies spawn from the top and move downward. Reaching the player's row costs 1 life.

### Bullets

| Bullet | Color | Direction |
|--------|-------|-----------|
| `║` Player bullet | Cyan | Upward |
| `↓` Enemy bullet | Magenta | Downward |

Up to 3 player bullets on screen at once (6 with RapidFire).

### Power-ups (catch falling items)

| Symbol | Color | Effect |
|--------|-------|--------|
| `★` SpreadShot | Yellow | 3-way spread fire for ~10 seconds |
| `♥` ExtraLife | Magenta | Instantly adds 1 life (max 5) |
| `!` RapidFire | Cyan | Raises bullet cap to 6 for ~10 seconds |

### Score persistence

Scores are saved automatically to `shooting_game.db` (SQLite, in the working directory).
The in-game HUD shows the top score for the current difficulty.


## Controls

| Key | Action |
|-----|--------|
| `←` / `A` | Move left |
| `→` / `D` | Move right |
| `Space` | Shoot |
| `Q` / `Esc` | Quit |
| `R` | Restart (Game Over screen) |

### Movement feel
- **Single tap** — moves exactly 1 step; press fires immediately, then stops
- **Hold** — 1 step on press, ~167 ms pause, then continuous movement at ~10 cols/sec


## Autonomous Play Mode

For testing and demonstration purposes, the game includes an "Autonomous Play" mode.

To run the game with auto-play enabled:
```bash
cargo run -- --auto-play
```

The bot uses simple heuristics to:
- **Targeting**: Align itself with the lowest enemy or falling power-up on screen.
- **Dodging**: Automatically move to avoid incoming enemy bullets if they are directly above.
- **Aggression**: Fire weapons continuously and prioritize targets directly in front.
- **Persistence**: Automatically restarts the game on "Hard" difficulty after a short delay upon Game Over.


## Installation

### Prerequisites

`rusqlite` bundles SQLite and compiles it from source, so a **C compiler** is required:

```bash
# Ubuntu / Debian
sudo apt install build-essential

# macOS (Xcode command line tools)
xcode-select --install

# Arch Linux
sudo pacman -S base-devel

# Fedora / RHEL
sudo dnf groupinstall "Development Tools"
```

### Build & run

```bash
$ git clone https://github.com/clove335/ShootingGame.git
$ cd ShootingGame/

# Install Rust if not already present
$ curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
$ source "$HOME/.cargo/env"

$ cargo build

# Normal play
$ cargo run

# Autonomous Play (Testing mode)
$ cargo run -- --auto-play
```

The game saves scores to `shooting_game.db` in the directory where you run it.


## License

MIT — see [LICENSE.txt](LICENSE.txt)
