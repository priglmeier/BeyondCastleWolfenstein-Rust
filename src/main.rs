// Beyond Castle Wolfenstein for Windows and Linux.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

use bcw::game::{Game, Options};
use bcw::testhost::TestHost;

const USAGE: &str = "Beyond Castle Wolfenstein (Rust port)

usage: bcw [options]
  -nosb          PC-speaker sound only (no sampled voices)
  -quiet         no sound
  --dir <path>   folder for savegame.bcw / optiona.dat / optionc.dat
                 (default: the folder the program is in)
  --help         this text

Keys (as in the original):
  q w e / a d / z x c   move (s = stop)       arrow keys also move
  i o p / k ; / , . /   aim the gun in that direction
  l  fire   h  holster   space  search / open / drag a body
  u  use    :  switch gun/dagger   b  bomb   r  reset bomb timer
  f  first aid   1-5  show a pass   m  offer money   0-9  lock combination
  Return  inventory   Esc  save game and quit to the title
";

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut opts = Options {
        no_sb: false,
        quiet: false,
    };
    let mut script: Option<String> = None;
    let mut log = "port_trace.txt".to_string();
    let mut dir: Option<String> = None;
    let mut max_polls = u64::MAX;
    let mut dump_at = None;
    let mut dump = "port_dump.bin".to_string();
    let mut i = 1;
    while i < args.len() {
        let next = |i: &mut usize| -> String {
            *i += 1;
            args.get(*i).cloned().unwrap_or_default()
        };
        match args[i].to_ascii_lowercase().as_str() {
            "-nosb" | "--nosb" | "/nosb" => opts.no_sb = true,
            "-quiet" | "--quiet" | "/quiet" => opts.quiet = true,
            "--dir" => dir = Some(next(&mut i)),
            "--test-script" => script = Some(next(&mut i)),
            "--log" => log = next(&mut i),
            "--max-polls" => max_polls = next(&mut i).parse().unwrap_or(u64::MAX),
            "--dump-at" => dump_at = next(&mut i).parse().ok(),
            "--dump" => dump = next(&mut i),
            "-h" | "--help" | "/?" => {
                println!("{}", USAGE);
                return;
            }
            a => eprintln!("bcw: ignoring unknown option {}", a),
        }
        i += 1;
    }

    if let Some(s) = script {
        // headless, deterministic mode used for verification
        let d = dir.unwrap_or_else(|| ".".into());
        let host = TestHost::new(&s, &log, &d, max_polls, dump_at, &dump);
        let mut g = Game::new(host, &opts);
        g.run();
        eprintln!("done: polls={}", g.host.polls);
        return;
    }

    let dir = match dir {
        Some(d) => std::path::PathBuf::from(d),
        None => std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.to_path_buf()))
            .unwrap_or_else(|| ".".into()),
    };
    if let Err(e) = run_gui(opts, dir) {
        report_error(&e);
        std::process::exit(1);
    }
}

#[cfg(all(unix, not(target_os = "macos")))]
fn run_gui(opts: Options, dir: std::path::PathBuf) -> Result<(), String> {
    use bcw::platform::*;
    let sh = Shared::new();
    if let Ok(p) = std::env::var("BCW_WAV") {
        start_wav_recorder(sh.clone(), p);
    } else if !opts.quiet {
        alsa::start(sh.clone());
    }
    let game = spawn_game(sh.clone(), opts, dir);
    let r = x11::run(sh.clone(), "Beyond Castle Wolfenstein");
    sh.request_quit();
    let _ = game.join();
    r
}

#[cfg(windows)]
fn run_gui(opts: Options, dir: std::path::PathBuf) -> Result<(), String> {
    use bcw::platform::*;
    let sh = Shared::new();
    if !opts.quiet {
        win32::start_audio(sh.clone());
    }
    let game = spawn_game(sh.clone(), opts, dir);
    let r = win32::run(sh.clone(), "Beyond Castle Wolfenstein");
    sh.request_quit();
    let _ = game.join();
    r
}

#[cfg(not(any(windows, all(unix, not(target_os = "macos")))))]
fn run_gui(_opts: Options, _dir: std::path::PathBuf) -> Result<(), String> {
    Err("no window back end for this platform".into())
}

fn report_error(e: &str) {
    eprintln!("bcw: {}", e);
    #[cfg(windows)]
    bcw::platform::win32::error_box(e);
}
