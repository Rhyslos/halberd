//! A stand-in for vbsp, vvis and vrad, for Halberd's own tests (copied
//! into a made-up Garry's Mod folder under those names). It acts on the
//! map file's text: "LEAK" makes vbsp report a leak, "CRASH" makes it
//! fail, "SLOW" makes vrad take a long time. Not part of the editor.

use std::io::Write;
use std::path::PathBuf;

fn main() {
    let tool = std::env::current_exe()
        .ok()
        .and_then(|p| p.file_stem().map(|s| s.to_string_lossy().into_owned()))
        .unwrap_or_default();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(map) = args.last().map(PathBuf::from) else {
        writeln!(std::io::stderr(), "Error: no map given").ok();
        std::process::exit(2);
    };
    let text = std::fs::read_to_string(map.with_extension("vmf")).unwrap_or_default();
    let bsp = map.with_extension("bsp");
    let mut out = std::io::stdout();
    writeln!(out, "Valve Software - {tool}.exe (fake) {}", args.join(" ")).ok();
    match tool.as_str() {
        "vbsp" => {
            if text.contains("CRASH") {
                writeln!(std::io::stderr(), "Error: brush 3 has no sides").ok();
                std::process::exit(3);
            }
            if text.contains("LEAK") {
                writeln!(out, "**** leaked ****").ok();
                std::fs::write(map.with_extension("lin"), "0 0 0\n64 0 0\n").ok();
                std::process::exit(0);
            }
            writeln!(out, "WARNING: a brush has a texture missing").ok();
            std::fs::write(&bsp, b"VBSP").ok();
        }
        "vvis" => {
            writeln!(out, "Building visibility clusters...\rdone").ok();
        }
        "vrad" => {
            if text.contains("SLOW") {
                std::thread::sleep(std::time::Duration::from_secs(30));
            }
            writeln!(out, "Lighting done").ok();
        }
        _ => {}
    }
}
