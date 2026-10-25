//! Materials from the game's content: their names, and the picture each
//! shows, read and decoded on a background thread.

use crate::GameContent;
use halberd_vtf::{Material, RgbaImage, Vtf};
use std::collections::{BTreeSet, HashSet};
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender};

/// Most loose material files listed (a damaged or huge folder can't make
/// the list endless).
const MAX_LOOSE_MATERIALS: usize = 200_000;
/// How many `include`s a patch material may follow.
const MAX_PATCH_DEPTH: usize = 8;
/// Largest material file read (real ones are a few hundred bytes).
const MAX_MATERIAL_TEXT: u64 = 1024 * 1024;

/// Every material in the game's content, as Hammer names them
/// (`BRICK/BRICKWALL001A`: the path under `materials/`, without `.vmt`,
/// upper case), sorted. Slow: call it off the interface thread.
pub fn material_names(content: &GameContent) -> Vec<String> {
    let mut names = BTreeSet::new();
    let archived = content.archived_files("materials/", ".vmt");
    let loose = content.loose_files("materials", ".vmt", MAX_LOOSE_MATERIALS);
    for path in archived.iter().chain(&loose) {
        if let Some(name) = name_of(path) {
            names.insert(name);
        }
    }
    names.into_iter().collect()
}

/// `materials/brick/wall.vmt` as `BRICK/WALL`.
fn name_of(path: &str) -> Option<String> {
    let lower = path.to_ascii_lowercase();
    let inner = lower.strip_prefix("materials/")?.strip_suffix(".vmt")?;
    (!inner.is_empty()).then(|| inner.to_ascii_uppercase())
}

/// A material's picture, as loaded.
#[derive(Debug, Clone, PartialEq)]
pub struct MaterialImage {
    /// The material's name (`BRICK/BRICKWALL001A`), as asked for.
    pub name: String,
    /// The size it was asked for (pixels on the longer side, at most).
    pub max_size: u32,
    /// Its texture, at most `max_size` pixels on each side; `None` if it
    /// has none that can be shown.
    pub image: Option<RgbaImage>,
    /// The texture's full size, for laying it out on faces (texture
    /// coordinates are in its full-size pixels).
    pub texture_size: (u32, u32),
    /// True for tool materials (nodraw, clip, trigger…).
    pub tool: bool,
    /// Why there is no picture, in plain words.
    pub problem: Option<String>,
}

/// Reads material `name` and decodes its texture at most `max_size`
/// pixels on each side. Slow: call it off the interface thread.
pub fn load_material(content: &GameContent, name: &str, max_size: u32) -> MaterialImage {
    let mut result = MaterialImage {
        name: name.to_string(),
        max_size,
        image: None,
        texture_size: (0, 0),
        tool: false,
        problem: None,
    };
    let path = format!(
        "materials/{}.vmt",
        name.to_ascii_lowercase().replace('\\', "/")
    );
    let material = match read_material(content, &path) {
        Ok(material) => material,
        Err(why) => {
            result.problem = Some(why);
            return result;
        }
    };
    result.tool = material.is_tool();
    let Some(texture) = material.texture_path() else {
        result.problem = Some("it names no texture".into());
        return result;
    };
    let bytes = match content.read(&texture) {
        Ok(bytes) => bytes,
        Err(e) => {
            result.problem = Some(e.to_string());
            return result;
        }
    };
    match Vtf::parse(&bytes).and_then(|vtf| Ok((vtf.width(), vtf.height(), vtf.decode(max_size)?)))
    {
        Ok((w, h, image)) => {
            result.texture_size = (w, h);
            result.image = Some(image);
        }
        Err(e) => result.problem = Some(e.to_string()),
    }
    result
}

/// A material, following patch materials' `include`s (their own
/// parameters win).
fn read_material(content: &GameContent, path: &str) -> Result<Material, String> {
    let mut path = path.to_string();
    let mut overrides: Vec<Material> = Vec::new();
    for _ in 0..MAX_PATCH_DEPTH {
        let bytes = content
            .read_at_most(&path, MAX_MATERIAL_TEXT)
            .map_err(|e| e.to_string())?;
        let text = String::from_utf8_lossy(&bytes);
        let material = Material::parse(&text).map_err(|e| e.to_string())?;
        match material.include().map(str::to_string) {
            Some(include) => {
                overrides.push(material);
                path = include.replace('\\', "/");
            }
            None => {
                // Each patch's parameters go over what it includes; the
                // outermost patch is laid on last, so it wins.
                let base = overrides
                    .iter()
                    .rev()
                    .fold(material, |base, patch| base.patched_by(patch));
                return Ok(base);
            }
        }
    }
    Err("its patch materials include each other too deeply".into())
}

/// The request to read next: the biggest size first (the views' textures
/// before the browser's pictures), and among those the newest (what is on
/// screen now, not what scrolled past).
fn next_request(queue: &mut Vec<(String, u32)>) -> Option<(String, u32)> {
    let biggest = queue.iter().map(|(_, size)| *size).max()?;
    let at = queue.iter().rposition(|(_, size)| *size == biggest)?;
    Some(queue.remove(at))
}

/// Loads material pictures on a background thread.
pub struct MaterialLoader {
    requests: Sender<(String, u32)>,
    results: Receiver<MaterialImage>,
    /// Asked for and not yet back, so nothing is asked twice.
    pending: HashSet<(String, u32)>,
}

impl MaterialLoader {
    /// Starts a loader reading from `content`; `wake` is called whenever a
    /// picture is ready (to redraw the window).
    pub fn new(content: Arc<GameContent>, wake: impl Fn() + Send + 'static) -> Self {
        let (requests, todo) = mpsc::channel::<(String, u32)>();
        let (done, results) = mpsc::channel();
        std::thread::Builder::new()
            .name("halberd-materials".into())
            .spawn(move || {
                let mut queue: Vec<(String, u32)> = Vec::new();
                loop {
                    // Waits when there is nothing to do; stops when the
                    // loader is dropped (its sender goes).
                    if queue.is_empty() {
                        match todo.recv() {
                            Ok(request) => queue.push(request),
                            Err(_) => break,
                        }
                    }
                    queue.extend(todo.try_iter());
                    let Some((name, max_size)) = next_request(&mut queue) else {
                        continue;
                    };
                    let image = load_material(&content, &name, max_size);
                    if done.send(image).is_err() {
                        break;
                    }
                    wake();
                }
            })
            .ok();
        Self {
            requests,
            results,
            pending: HashSet::new(),
        }
    }

    /// Asks for material `name`'s picture at `max_size`, unless it was
    /// already asked for and has not come back.
    pub fn request(&mut self, name: &str, max_size: u32) {
        let key = (name.to_ascii_uppercase(), max_size);
        if self.pending.insert(key.clone()) {
            self.requests.send(key).ok();
        }
    }

    /// True if `name` at `max_size` was asked for and has not come back.
    pub fn is_pending(&self, name: &str, max_size: u32) -> bool {
        self.pending
            .contains(&(name.to_ascii_uppercase(), max_size))
    }

    /// The pictures ready since the last call.
    pub fn poll(&mut self) -> Vec<MaterialImage> {
        let ready: Vec<MaterialImage> = self.results.try_iter().collect();
        for image in &ready {
            self.pending
                .remove(&(image.name.to_ascii_uppercase(), image.max_size));
        }
        ready
    }
}

#[cfg(test)]
mod tests;
