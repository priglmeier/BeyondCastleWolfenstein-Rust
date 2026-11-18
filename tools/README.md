# Verification tools

These tools check the Rust port against the original program.

## `reference/`

This folder holds the original source, assembled.

* **`BCW.ASM`**: the v0.2 source with three small fixes so that JWasm
  accepts it:
  * `v1967:` / `v1da9:` labels that preceded `dw` became plain data labels;
  * an undefined `exit` label in `sbDetect` became `sbDetExit`.
* **`CONSTANT.INC`**: re-created, because the original include file was
  not in the source. It holds `BIT0`–`BIT7`, `CR`, `LF`, `ESCAPE`, and so on.
* **`bcw.exe`**: built with:

  ```
  jwasm -mz -Fl=bcw.lst -Fo=bcw.exe BCW.ASM
  ```

  JWasm is at <https://github.com/Baron-von-Riedesel/JWasm>.

`scripts/extract.py` creates `../assets/*.bin` (the segment images the port
embeds) and `symbols.txt` from that `.exe` and the listing.

## `harness/`

The harness is a small 8086 interpreter with just enough PC around it:

* BIOS video/keyboard;
* DOS files and clock;
* CGA;
* PC speaker;
* a Sound Blaster (DSP, DMA, IRQ 5).

It runs `bcw.exe`, and at every keyboard poll it logs a hash of the whole
game state.

```
cd harness && cargo build --release
./target/release/harness --script keys.txt --log h.txt --dir <folder> [--nosb] [--max-polls N]
../../target/release/bcw  --test-script keys.txt --log p.txt --dir <folder2> [--nosb] [--max-polls N]
cmp h.txt p.txt
```

* **Key scripts.** A key script has one line per key: `<polls to wait>
  <BIOS key code in hex>`. For example, `0 000d` is Return.
* **Dumping state.** `--dump-at N --dump file` writes the full state at poll
  N on either side.
* **Comparing dumps.** `scripts/sdiff.py a b` lists the bytes that differ,
  by variable name.

`scripts/fuzz.py` generates random games and compares the two programs.
`scripts/covreport.py` shows which original instructions those games
exercised (harness `--cov`).
