//! Which material has which layer of the texture array, and what has
//! become of each layer. Kept apart from the GPU so it can be tested.

use std::collections::{HashMap, HashSet};

/// What has become of a material's layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Layer {
    /// Its texture is wanted and not yet asked for.
    Wanted,
    /// Asked for; waiting.
    Asked,
    /// On the GPU.
    Loaded,
    /// It has no texture that can be shown (the pattern stays).
    Missing,
    /// No material uses it; it can be given to another.
    Free,
}

/// The layers given out.
#[derive(Debug)]
pub(crate) struct Layers {
    /// Upper-case material name to its layer.
    names: HashMap<String, u32>,
    states: Vec<Layer>,
    capacity: u32,
}

impl Layers {
    pub(crate) fn new(capacity: u32) -> Self {
        Self {
            names: HashMap::new(),
            states: Vec::new(),
            capacity,
        }
    }

    /// How many layers there may be.
    pub(crate) fn capacity(&self) -> u32 {
        self.capacity
    }

    /// Material `name`'s layer, if it has one.
    pub(crate) fn of(&self, name: &str) -> Option<u32> {
        self.names.get(&name.to_ascii_uppercase()).copied()
    }

    /// The layer for material `name`, giving it a free one (or a new one,
    /// within the capacity) if it has none.
    pub(crate) fn layer_for(&mut self, name: &str) -> Option<u32> {
        let key = name.to_ascii_uppercase();
        if let Some(&layer) = self.names.get(&key) {
            return Some(layer);
        }
        let layer = match self.states.iter().position(|s| *s == Layer::Free) {
            Some(free) => {
                self.states[free] = Layer::Wanted;
                u32::try_from(free).ok()?
            }
            None => {
                let layer = u32::try_from(self.states.len()).ok()?;
                if layer >= self.capacity {
                    return None;
                }
                self.states.push(Layer::Wanted);
                layer
            }
        };
        self.names.insert(key, layer);
        Some(layer)
    }

    /// Frees the layers of materials not in `used` (upper case). Returns
    /// the layers freed.
    pub(crate) fn keep_only(&mut self, used: &HashSet<String>) -> Vec<u32> {
        let mut freed = Vec::new();
        let states = &mut self.states;
        self.names.retain(|name, layer| {
            let keep = used.contains(name);
            if !keep {
                if let Some(state) = states.get_mut(*layer as usize) {
                    *state = Layer::Free;
                }
                freed.push(*layer);
            }
            keep
        });
        freed.sort_unstable();
        freed
    }

    /// Materials wanted and not yet asked for, sorted; they count as asked
    /// for from now on.
    pub(crate) fn take_wanted(&mut self) -> Vec<String> {
        let mut out = Vec::new();
        for (name, &layer) in &self.names {
            if let Some(state) = self.states.get_mut(layer as usize)
                && *state == Layer::Wanted
            {
                *state = Layer::Asked;
                out.push(name.clone());
            }
        }
        out.sort();
        out
    }

    /// Records what became of `layer`.
    pub(crate) fn set(&mut self, layer: u32, state: Layer) {
        if let Some(s) = self.states.get_mut(layer as usize) {
            *s = state;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn used(names: &[&str]) -> HashSet<String> {
        names.iter().map(|n| n.to_string()).collect()
    }

    #[test]
    fn layers_of_materials_no_longer_shown_are_given_to_others() {
        let mut layers = Layers::new(2);
        assert_eq!(layers.layer_for("brick/a"), Some(0));
        assert_eq!(layers.layer_for("BRICK/A"), Some(0), "any case");
        assert_eq!(layers.layer_for("dev/b"), Some(1));
        assert_eq!(layers.layer_for("tools/c"), None, "full");
        assert_eq!(layers.take_wanted(), ["BRICK/A", "DEV/B"]);
        assert!(layers.take_wanted().is_empty(), "asked once");
        layers.set(1, Layer::Missing);
        // A new map shows only C: both layers come back, missing or not.
        assert_eq!(layers.keep_only(&used(&["TOOLS/C"])), [0, 1]);
        assert_eq!(layers.of("brick/a"), None);
        assert_eq!(layers.layer_for("tools/c"), Some(0));
        assert_eq!(layers.take_wanted(), ["TOOLS/C"]);
        assert_eq!(layers.capacity(), 2);
    }
}
