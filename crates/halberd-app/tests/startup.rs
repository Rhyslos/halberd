//! End-to-end tests: run the real `halberd` program against fake Steam and
//! GMod folders, the way a user would, and check what it reports and saves.

// This whole file is test code: a failed setup step should stop the test loudly.
#![allow(clippy::unwrap_used)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

struct Run {
    stdout: String,
    success: bool,
}

fn halberd(args: &[&std::ffi::OsStr]) -> Run {
    // --report-only: print the startup report and exit, without opening the
    // editor window (CI machines have no screen).
    let output = Command::new(env!("CARGO_BIN_EXE_halberd"))
        .arg("--report-only")
        .args(args)
        .stdin(Stdio::null())
        .output()
        .unwrap();
    Run {
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        success: output.status.success(),
    }
}

/// Builds a fake Steam folder with GMod installed in its main library and
/// returns (steam folder, gmod folder).
fn fake_steam_with_gmod(base: &Path) -> (PathBuf, PathBuf) {
    let steam = base.join("Steam");
    let steamapps = steam.join("steamapps");
    fs::create_dir_all(&steamapps).unwrap();
    let path = steam.display().to_string().replace('\\', "\\\\");
    fs::write(
        steamapps.join("libraryfolders.vdf"),
        format!("\"libraryfolders\"\n{{\n\t\"0\"\n\t{{\n\t\t\"path\"\t\t\"{path}\"\n\t}}\n}}\n"),
    )
    .unwrap();
    fs::write(
        steamapps.join("appmanifest_4000.acf"),
        "\"AppState\"\n{\n\t\"appid\"\t\t\"4000\"\n\t\"installdir\"\t\t\"GarrysMod\"\n}\n",
    )
    .unwrap();
    let gmod = steamapps.join("common").join("GarrysMod");
    fs::create_dir_all(gmod.join("garrysmod")).unwrap();
    fs::write(
        gmod.join("garrysmod").join("gameinfo.txt"),
        "\"GameInfo\" {}",
    )
    .unwrap();
    let bin = gmod.join("bin").join("win64");
    fs::create_dir_all(&bin).unwrap();
    for tool in ["vbsp", "vvis", "vrad", "bspzip"] {
        fs::write(bin.join(format!("{tool}.exe")), b"MZ").unwrap();
    }
    (steam, gmod)
}

#[test]
fn first_launch_finds_gmod_and_remembers_it() {
    let dir = tempfile::tempdir().unwrap();
    let (steam, gmod) = fake_steam_with_gmod(dir.path());
    let settings = dir.path().join("cfg").join("settings.toml");

    let run = halberd(&[
        "--settings".as_ref(),
        settings.as_os_str(),
        "--steam-dir".as_ref(),
        steam.as_os_str(),
    ]);
    assert!(run.success, "{}", run.stdout);
    assert!(
        run.stdout.contains("first launch, using defaults"),
        "{}",
        run.stdout
    );
    assert!(
        run.stdout.contains("Garry's Mod: found through Steam"),
        "{}",
        run.stdout
    );
    assert!(
        run.stdout
            .contains("vbsp found, vvis found, vrad found, bspzip found")
    );
    assert!(run.stdout.contains("Settings saved."));

    // Read the file back the way Halberd does, rather than matching text:
    // how a path is written (quotes, backslashes) varies by system.
    let saved = halberd_config::SettingsStore::new(&settings).load();
    assert_eq!(saved.status, halberd_config::LoadStatus::Loaded);
    assert_eq!(saved.settings.game.gmod_dir, Some(gmod));
}

#[test]
fn second_launch_uses_the_saved_folder() {
    let dir = tempfile::tempdir().unwrap();
    let (steam, _gmod) = fake_steam_with_gmod(dir.path());
    let settings = dir.path().join("settings.toml");
    let args = [
        "--settings".as_ref(),
        settings.as_os_str(),
        "--steam-dir".as_ref(),
        steam.as_os_str(),
    ];
    halberd(&args);
    let run = halberd(&args);
    assert!(run.stdout.contains("(loaded)"), "{}", run.stdout);
    assert!(
        run.stdout.contains("found from saved settings"),
        "{}",
        run.stdout
    );
    assert!(
        !run.stdout.contains("Settings saved."),
        "nothing changed, nothing to save"
    );
}

#[test]
fn chosen_folder_is_used_and_saved() {
    let dir = tempfile::tempdir().unwrap();
    let (_steam, gmod) = fake_steam_with_gmod(dir.path());
    let settings = dir.path().join("settings.toml");
    let run = halberd(&[
        "--settings".as_ref(),
        settings.as_os_str(),
        "--gmod-dir".as_ref(),
        gmod.as_os_str(),
        "--steam-dir".as_ref(),
        dir.path().join("no-steam-here").as_os_str(),
    ]);
    assert!(
        run.stdout.contains("found from the folder you chose"),
        "{}",
        run.stdout
    );
    assert!(run.stdout.contains("Settings saved."));
}

#[test]
fn bad_chosen_folder_is_explained_and_steam_is_tried() {
    let dir = tempfile::tempdir().unwrap();
    let (steam, _gmod) = fake_steam_with_gmod(dir.path());
    let settings = dir.path().join("settings.toml");
    let run = halberd(&[
        "--settings".as_ref(),
        settings.as_os_str(),
        "--gmod-dir".as_ref(),
        dir.path().as_os_str(),
        "--steam-dir".as_ref(),
        steam.as_os_str(),
    ]);
    assert!(
        run.stdout.contains("the chosen folder can't be used"),
        "{}",
        run.stdout
    );
    assert!(run.stdout.contains("found through Steam"), "{}", run.stdout);
}

#[test]
fn missing_gmod_gives_a_tip_and_still_saves_settings() {
    let dir = tempfile::tempdir().unwrap();
    let settings = dir.path().join("settings.toml");
    let empty_steam = dir.path().join("EmptySteam");
    fs::create_dir_all(empty_steam.join("steamapps")).unwrap();
    let run = halberd(&[
        "--settings".as_ref(),
        settings.as_os_str(),
        "--steam-dir".as_ref(),
        empty_steam.as_os_str(),
    ]);
    assert!(run.success);
    assert!(
        run.stdout.contains("Garry's Mod: not found."),
        "{}",
        run.stdout
    );
    assert!(run.stdout.contains("--gmod-dir"));
    assert!(settings.is_file(), "first launch creates the settings file");
}

#[test]
fn damaged_settings_are_recovered() {
    let dir = tempfile::tempdir().unwrap();
    let (steam, _gmod) = fake_steam_with_gmod(dir.path());
    let settings = dir.path().join("settings.toml");
    fs::write(&settings, "[[[ not settings").unwrap();
    let run = halberd(&[
        "--settings".as_ref(),
        settings.as_os_str(),
        "--steam-dir".as_ref(),
        steam.as_os_str(),
    ]);
    assert!(run.success);
    assert!(
        run.stdout.contains("damaged file set aside"),
        "{}",
        run.stdout
    );
    assert!(dir.path().join("settings.toml.damaged-1").is_file());
}

#[test]
fn help_is_shown() {
    let run = halberd(&["--help".as_ref()]);
    assert!(run.success);
    assert!(run.stdout.contains("--gmod-dir <folder>"));
}

#[test]
fn unknown_option_fails_with_help() {
    let run = halberd(&["--make-it-fast".as_ref()]);
    assert!(!run.success);
    assert!(run.stdout.contains("unknown option: --make-it-fast"));
    assert!(run.stdout.contains("Usage:"));
}
