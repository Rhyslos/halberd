//! Reads an FGD file and the files it includes.

use crate::model::{Fgd, FgdProblem};
use crate::parse::parse_file;
use std::io::Read;
use std::path::{Path, PathBuf};

/// Largest FGD file read (GMod's largest is under 1 MB).
pub const MAX_FGD_BYTES: u64 = 16 * 1024 * 1024;
/// Most files read through `@include`.
const MAX_FILES: usize = 64;
/// Deepest chain of includes.
const MAX_INCLUDE_DEPTH: usize = 16;

/// Reads the classes in `text` (one file, named `file` in problems).
/// `@include`s aren't followed; [`load`] follows them.
pub fn parse(text: &str, file: &str) -> Fgd {
    let mut fgd = Fgd::default();
    add_contents(&mut fgd, parse_file(text), file);
    fgd
}

fn add_contents(fgd: &mut Fgd, contents: crate::parse::FileContents, file: &str) {
    fgd.files.push(file.to_string());
    fgd.problems.extend(
        contents
            .problems
            .into_iter()
            .map(|(line, message)| FgdProblem {
                file: file.to_string(),
                line,
                message,
            }),
    );
    if contents.map_size.is_some() {
        fgd.map_size = contents.map_size;
    }
    for class in contents.classes {
        fgd.add(class);
    }
}

/// Reads the FGD file at `path` and every file it includes (looked for
/// beside it), included files first, as Hammer does when the includes are
/// at the top. Files that can't be read are noted in `problems`; a class
/// defined twice keeps the later definition.
pub fn load(path: &Path) -> Fgd {
    let mut fgd = Fgd::default();
    let mut seen = Vec::new();
    load_into(&mut fgd, path, 0, &mut seen);
    fgd
}

fn load_into(fgd: &mut Fgd, path: &Path, depth: usize, seen: &mut Vec<PathBuf>) {
    let name = path.file_name().map_or_else(
        || path.display().to_string(),
        |n| n.to_string_lossy().into_owned(),
    );
    let problem = |fgd: &mut Fgd, message: String| {
        fgd.problems.push(FgdProblem {
            file: name.clone(),
            line: 0,
            message,
        });
    };
    let key = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    if seen.contains(&key) {
        return;
    }
    if seen.len() >= MAX_FILES || depth > MAX_INCLUDE_DEPTH {
        problem(
            fgd,
            "too many included files; the rest were left out".into(),
        );
        return;
    }
    seen.push(key);
    let text = match read_capped(path) {
        Ok(text) => text,
        Err(why) => return problem(fgd, why),
    };
    // Includes first: read the file's own classes after them.
    let contents = parse_file(&text);
    let folder = path.parent().unwrap_or(Path::new("."));
    for include in &contents.includes {
        let file = Path::new(include);
        // Only files beside this one or below it.
        if file.is_absolute() || file.components().any(|c| c.as_os_str() == "..") {
            problem(
                fgd,
                format!("\"{include}\" is outside the FGD folder and was not read"),
            );
            continue;
        }
        load_into(fgd, &folder.join(file), depth + 1, seen);
    }
    add_contents(fgd, contents, &name);
}

/// The text of `path`, at most [`MAX_FGD_BYTES`].
fn read_capped(path: &Path) -> Result<String, String> {
    let file = std::fs::File::open(path).map_err(|e| format!("could not be opened ({e})"))?;
    let mut bytes = Vec::new();
    file.take(MAX_FGD_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| format!("could not be read ({e})"))?;
    if bytes.len() as u64 > MAX_FGD_BYTES {
        return Err(format!(
            "is larger than {} MB, so it was not read",
            MAX_FGD_BYTES / (1024 * 1024)
        ));
    }
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}
