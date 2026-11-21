# Beyond Castle Wolfenstein — Rust port

A native Windows and Linux version of *Beyond Castle Wolfenstein* (Muse
Software, 1985). It is translated from the public-domain PC source,
`BCW.ASM` (v0.2, July 2004), which jeff leyda disassembled and commented.

> Original game concept and programming by Silas Warner.
> Donated by Kari Ann Owen to the public domain, in loving memory of her
> husband, Silas Warner.

This port follows the source's request: keep it open and free.

## What it is

* **The game logic is a routine-by-routine translation of the 8086 code.**
  The original keeps its state in a few hundred bytes inside its code
  segment, passes values in registers and flags, and jumps into the middle
  of other routines. The port keeps that model on purpose, so it behaves
  exactly like the original, bugs included:
  * the original memory images and variable offsets (`src/game/sym.rs`);
  * a small register file;
  * one Rust function per routine, named after its label in `BCW.ASM`.
* **Everything that touched hardware was rewritten:**
  * CGA video, scaled to a resizable 4:3 window;
  * BIOS text output, using the IBM 8×8 font;
  * keyboard;
  * PC speaker, timed like the original's port-I/O delay loops;
  * Sound Blaster voice samples;
  * DOS clock (18.2 Hz) and files.
* **There are no external crates.** On Linux it uses Xlib and ALSA (ALSA is
  loaded at run time, so it still starts without sound). On Windows it uses
  Win32/GDI and waveOut. `cargo build` works offline.

## How it was checked

The original source was assembled with JWasm. A small 8086 interpreter
(`tools/harness`) runs that binary with emulated BIOS, DOS, CGA, speaker and
Sound Blaster. The port and the original then get the same scripted
keystrokes. At every keyboard poll, both hash the complete game state: all
variables, all 64 rooms, video memory and the cursor.

Results:

* **Demo mode:** identical for 100,000 polls with Sound Blaster and 300,000
  polls with PC speaker.
* **Randomised games:** dozens of games with random input (moving, shooting,
  searching, opening closets, bribing, saving and resuming) gave identical
  state traces.

If you change game logic, you can re-run the same checks (see `tools/`).

## Building

```
cargo build --release
```

* **Linux:** needs `libX11` (the `libx11-dev` / `libx11` package). On
  CachyOS/Arch: `sudo pacman -S libx11 alsa-lib`.
* **Windows:** a normal Rust install with the MSVC toolchain. It also builds
  with the GNU toolchain.

The binary is `target/release/bcw` (`bcw.exe` on Windows).

## Data files

The program has the title screen, graphics, sounds and the demo built in.
To **play**, it also needs **`optiona.dat`**, the 16 KB bunker template
from the original BCW DOS release (<http://www.oldskool.org/pc/BCW>). The
game generates a fresh random bunker from it for every new game, just like
the original.

Put `optiona.dat` in the same folder as the program, or point to a folder
with `--dir <path>`. That folder also holds `savegame.bcw`, which Esc
writes, and `optionc.dat`, which menu option C reads.

If that folder also has **`optionz.dat`**, a custom bunker exported from
the Beyond Castle Wolfenstein Editor, the menu adds option **E) Start a new
game with your custom bunker**, which plays it exactly as designed. The game
never writes `optionz.dat`, so options A, B and C can't erase it. Without
the file, the menu is exactly the original's.

Without `optiona.dat`, the title screen and demo still run. Choosing A, B
or C shows the original's "file is missing" message.

## Running

```
bcw [-nosb] [-quiet] [--dir <folder>]
```

* `-nosb`: PC-speaker sound only, including the 1-bit speaker voices,
  like a PC without a Sound Blaster.
* `-quiet`: no sound.

### Keys

| Key | Action |
| --- | --- |
| `q w e` / `a d` / `z x c` | move (as on a numeric keypad, centred on `s`) |
| `s` | stop moving |
| arrow keys | move (added for convenience) |
| `i o p` / `k ;` / `, . /` | aim the gun in that direction |
| `l` | fire |
| `h` | holster the gun |
| Space | search a body / drag it / open a closet |
| `u` | use what is in a closet |
| `:` | switch between gun and dagger |
| `b` | drop / pick up / place the bomb |
| `r` | reset the bomb timer |
| `f` | first aid |
| `1`–`5` | show a pass |
| `m` | offer money |
| `0`–`9` | dial a lock combination |
| Ctrl-T | use the tool kit |
| Ctrl-K | use keys |
| Return | inventory |
| Esc | save the game and return to the title |

On the menu, Ctrl-C switches between the two CGA palettes (the original's
"monitor type").

## Notes on faithfulness

* **Random numbers.** The original reads its random numbers from the PC's
  BIOS ROM (segment F800h), which differs between machines. The port uses a
  fixed pseudo-random 64 KB image instead, with the real 8×8 font at its
  real place. The demo has its own recorded random numbers and plays exactly
  as it did.
* **Hardware probes.** No joystick is detected (the joystick code runs as
  it would on a PC without a game port). The CPU is detected as "fast", so
  the 8088/PCjr timing paths are not used.
* **Bugs kept.** Original bugs are preserved. Examples:
  * menu option C's `optionc.dat` is read but never written;
  * a few table lookups read past their tables.

## Source layout

```
src/game/      translated game logic
  core.rs        start-up, title, keyboard, random numbers, text, files
  main_loop.rs   main loop, player commands, bomb, changing rooms
  draw.rs        rooms, sprites, glyphs, status line
  guards.rs      guard AI, path finding, line of sight, bribes, shots
  items.rs       searching, closets, locks, alarm box, inventory
  sound.rs       PC speaker, Sound Blaster, voices, alarm
  menu.rs        menu, resume, random bunker generator
  sym.rs         offsets of every label/variable (from the listing)
src/pc/bios.rs CGA mode 4, BIOS teletype, ROM stand-in
src/platform/  window, keyboard, audio (x11.rs, alsa.rs, win32.rs)
assets/        segment images extracted from the assembled original
tools/         reference harness and test scripts
```
