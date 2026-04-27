## Shooting game
- Written in Rust

## Indexes
- Motivation
- Gameplay
- Controls
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

### Debug mode (developer only)

| Key | Condition | Action |
|-----|-----------|--------|
| `` ` `` | Any | Toggle debug overlay (frame, position, entity counts, power-up, flags) |
| `G` | Debug on | Toggle god mode (invincibility) |
| `S` | Debug on | Toggle slow-motion (~7.5 FPS) |

Collision boxes are drawn around the player (cyan) and enemies (red) when the overlay is on.


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
sudo dnf install gcc
```

### Build & run

```bash
$ git clone https://github.com/clove335/ShootingGame.git
$ cd ShootingGame/

# Install Rust if not already present
$ curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
$ source ~/.bashrc

$ cargo build
$ cargo run
```

The game saves scores to `shooting_game.db` in the directory where you run it.


## License
MIT License

Copyright (c) 2026 clove335

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
