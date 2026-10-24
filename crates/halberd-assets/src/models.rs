//! Listing the game's models and reading one, with its materials found.

use crate::GameContent;
use halberd_mdl::Model;
use std::collections::{BTreeSet, HashSet};
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender};

/// Most loose model files listed (a huge addons folder can't stall it).
const MAX_LOOSE_MODELS: usize = 200_000;
/// Triangle files tried, best first.
const TRIANGLE_FILES: [&str; 4] = [".dx90.vtx", ".vtx", ".dx80.vtx", ".sw.vtx"];

/// Every model in the game's content, as maps name them
/// (`models/props_c17/oildrum001.mdl`, lower case), sorted. Slow: call it
/// off the interface thread.
pub fn model_names(content: &GameContent) -> Vec<String> {
    let mut names = BTreeSet::new();
    let archived = content.archived_files("models/", ".mdl");
    let loose = content.loose_files("models", ".mdl", MAX_LOOSE_MODELS);
    for path in archived.iter().chain(&loose) {
        let lower = path.to_ascii_lowercase().replace('\\', "/");
        if lower.starts_with("models/") && lower.len() > "models/.mdl".len() {
            names.insert(lower);
        }
    }
    names.into_iter().collect()
}

/// A model as read, or why it couldn't be.
#[derive(Debug, Clone, PartialEq)]
pub struct LoadedModel {
    /// Its name, as asked for (`models/props_c17/oildrum001.mdl`).
    pub name: String,
    /// The model, or why it could not be read, in plain words.
    pub model: Result<Model, String>,
    /// For each of the model's materials, the material's full name
    /// (`MODELS/PROPS_C17/OILDRUM001`), found in the model's material
    /// folders; the first folder is assumed when none has it.
    pub materials: Vec<String>,
}

/// Reads model `name` (its `.mdl`, `.vvd` and `.vtx` files) and finds its
/// materials. Slow: call it off the interface thread.
pub fn load_model(content: &GameContent, name: &str) -> LoadedModel {
    let mut loaded = LoadedModel {
        name: name.to_string(),
        model: Err(String::new()),
        materials: Vec::new(),
    };
    let path = name.to_ascii_lowercase().replace('\\', "/");
    let Some(stem) = path.strip_suffix(".mdl") else {
        loaded.model = Err("its name does not end in .mdl".into());
        return loaded;
    };
    let read = |ext: &str| content.read(&format!("{stem}{ext}"));
    let files = read(".mdl").and_then(|mdl| Ok((mdl, read(".vvd")?)));
    let (mdl, vvd) = match files {
        Ok(files) => files,
        Err(e) => {
            loaded.model = Err(e.to_string());
            return loaded;
        }
    };
    let Some(vtx) = TRIANGLE_FILES.iter().find_map(|ext| read(ext).ok()) else {
        loaded.model = Err("its triangles (.vtx file) are missing".into());
        return loaded;
    };
    match Model::parse(&mdl, &vvd, &vtx) {
        Ok(model) => {
            loaded.materials = model
                .materials
                .iter()
                .map(|m| find_material(content, &model.material_folders, m))
                .collect();
            loaded.model = Ok(model);
        }
        Err(e) => loaded.model = Err(e.to_string()),
    }
    loaded
}

/// The full name of material `name`, looked for in `folders` in order.
fn find_material(content: &GameContent, folders: &[String], name: &str) -> String {
    let tidy = |s: &str| -> String {
        s.replace('\\', "/")
            .split('/')
            .filter(|p| !p.is_empty())
            .collect::<Vec<_>>()
            .join("/")
            .to_ascii_uppercase()
    };
    let name = tidy(name);
    let candidates: Vec<String> = folders
        .iter()
        .map(|folder| {
            let folder = tidy(folder);
            if folder.is_empty() {
                name.clone()
            } else {
                format!("{folder}/{name}")
            }
        })
        .collect();
    candidates
        .iter()
        .find(|c| content.contains(&format!("materials/{}.vmt", c.to_ascii_lowercase())))
        .or(candidates.first())
        .cloned()
        .unwrap_or(name)
}

/// Loads models on a background thread.
pub struct ModelLoader {
    requests: Sender<String>,
    results: Receiver<LoadedModel>,
    /// Asked for and not yet back, so nothing is asked twice.
    pending: HashSet<String>,
}

impl ModelLoader {
    /// Starts a loader reading from `content`; `wake` is called whenever a
    /// model is ready (to redraw the window).
    pub fn new(content: Arc<GameContent>, wake: impl Fn() + Send + 'static) -> Self {
        let (requests, todo) = mpsc::channel::<String>();
        let (done, results) = mpsc::channel();
        std::thread::Builder::new()
            .name("halberd-models".into())
            .spawn(move || {
                // Stops when the loader is dropped (its sender goes).
                while let Ok(name) = todo.recv() {
                    if done.send(load_model(&content, &name)).is_err() {
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

    /// Asks for model `name`, unless it was already asked for and has not
    /// come back.
    pub fn request(&mut self, name: &str) {
        let key = name.to_ascii_lowercase();
        if self.pending.insert(key.clone()) {
            self.requests.send(key).ok();
        }
    }

    /// True if `name` was asked for and has not come back.
    pub fn is_pending(&self, name: &str) -> bool {
        self.pending.contains(&name.to_ascii_lowercase())
    }

    /// The models ready since the last call.
    pub fn poll(&mut self) -> Vec<LoadedModel> {
        let ready: Vec<LoadedModel> = self.results.try_iter().collect();
        for model in &ready {
            self.pending.remove(&model.name);
        }
        ready
    }
}

#[cfg(test)]
mod tests;
