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


