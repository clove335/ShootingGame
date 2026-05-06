## Shooting game
- Written in Rust

## Indexes
- Motivation
- Gameplay
- Controls
- Demo Mode
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


## Demo Mode

Demo Mode runs the game autonomously — useful for watching gameplay, testing, or leaving the game running as an attract screen.

```bash
cargo run -- --auto-play
```

The game starts immediately on **Hard** difficulty and restarts automatically after each Game Over with no human input required.

### How the bot works

The bot is a rule-based heuristic, not a learning AI. Every frame it evaluates the current game state and picks one action:

**Targeting**
Scans all enemies and falling power-ups, picks the one closest to the bottom of the screen (highest threat / highest value), and steers toward its x-position. Power-ups are prioritised over enemies at equal depth.

**Dodging**
Before moving toward a target, checks whether any enemy bullet is falling in the same column (within ±1) and within a few rows above the player. If a bullet is detected, the bot sidesteps away from it. Dodge takes priority over targeting.

**Shooting**
Fires every frame an enemy is within ±2 columns of the player's x-position, and otherwise fires every 5 frames unconditionally.

**Limitations**
- No look-ahead: the bot reacts to the current frame only, it cannot predict enemy trajectories.
- Single-threat dodge: only the nearest bullet is considered; flanking bullets from multiple angles can still hit.
- No power-up timing: activates whatever power-up is caught without strategic planning.


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

# Demo Mode
$ cargo run -- --auto-play
```

The game saves scores to `shooting_game.db` in the directory where you run it.


## License

MIT — see [LICENSE.txt](LICENSE.txt)
