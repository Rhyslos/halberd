//! VMT materials: a shader name and its parameters, such as the texture
//! to draw (`$basetexture`).

use crate::error::VtfError;
use halberd_kv::{Entry, parse_game_file};

/// Largest material text read.
const MAX_TEXT: usize = 1024 * 1024;

/// A material's shader and its parameters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Material {
    /// The shader (`LightmappedGeneric`, `UnlitGeneric`, `Patch`…).
    pub shader: String,
    /// Parameters as written (`$basetexture` → `brick/brickwall001a`), keys
    /// in lower case, the first of each kept. Blocks inside (proxies,
    /// fallbacks) are left out.
    params: Vec<(String, String)>,
}

impl Material {
    /// Reads a VMT's text.
    pub fn parse(text: &str) -> Result<Self, VtfError> {
        if text.len() > MAX_TEXT {
            return Err(VtfError::BadMaterial("it is far too long".into()));
        }
        let entries = parse_game_file(text).map_err(|e| VtfError::BadMaterial(e.to_string()))?;
        let block = entries
            .iter()
            .find_map(|e| match e {
                Entry::Block(b) => Some(b),
                Entry::Pair(..) => None,
            })
            .ok_or_else(|| VtfError::BadMaterial("it has no shader block".into()))?;
        let mut params: Vec<(String, String)> = Vec::new();
        for entry in &block.entries {
            if let Entry::Pair(key, value) = entry {
                let key = key.to_ascii_lowercase();
                if !params.iter().any(|(k, _)| *k == key) {
                    params.push((key, value.clone()));
                }
            }
        }
        // A patch keeps its replacements in a block of their own.
        if block.name.eq_ignore_ascii_case("patch") {
            for inner in &block.entries {
                if let Entry::Block(b) = inner
                    && (b.name.eq_ignore_ascii_case("replace")
                        || b.name.eq_ignore_ascii_case("insert"))
                {
                    for entry in &b.entries {
                        if let Entry::Pair(key, value) = entry {
                            let key = key.to_ascii_lowercase();
                            params.retain(|(k, _)| *k != key);
                            params.push((key, value.clone()));
                        }
                    }
                }
            }
        }
        Ok(Self {
            shader: block.name.clone(),
            params,
        })
    }

    /// A parameter's value (`$basetexture`, any case).
    pub fn param(&self, key: &str) -> Option<&str> {
        self.params
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(key))
            .map(|(_, v)| v.as_str())
    }

    /// The material this one patches (`Patch` materials' `include`).
    pub fn include(&self) -> Option<&str> {
        self.shader
            .eq_ignore_ascii_case("patch")
            .then(|| self.param("include"))
            .flatten()
    }

    /// The texture to show for this material, as a path inside the game
    /// (`materials/brick/brickwall001a.vtf`): the tool texture if it has
    /// one (`%tooltexture`, for nodraw and the like), else its base texture.
    pub fn texture_path(&self) -> Option<String> {
        let name = self
            .param("%tooltexture")
            .or_else(|| self.param("$basetexture"))
            .filter(|n| !n.trim().is_empty())?;
        // Backslashes (sometimes doubled in VMTs) become single slashes.
        let name = name.trim().replace('\\', "/").to_ascii_lowercase();
        let parts: Vec<&str> = name.split('/').filter(|p| !p.is_empty()).collect();
        let name = parts.join("/");
        let name = name.as_str();
        let name = name.strip_prefix("materials/").unwrap_or(name);
        let name = name.strip_suffix(".vtf").unwrap_or(name);
        Some(format!("materials/{name}.vtf"))
    }

    /// True for tool materials (nodraw, clip, trigger…), which are hidden
    /// in the game.
    pub fn is_tool(&self) -> bool {
        self.param("%compilenodraw")
            .is_some_and(|v| v.trim() == "1")
            || self.param("%compileclip").is_some_and(|v| v.trim() == "1")
            || self
                .param("%compiletrigger")
                .is_some_and(|v| v.trim() == "1")
            || self.param("%tooltexture").is_some()
    }
}
