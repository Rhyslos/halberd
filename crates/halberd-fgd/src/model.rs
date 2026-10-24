//! What an FGD file describes: entity classes with their keyvalues,
//! inputs and outputs, and the editor helpers (size, colour, model).

use std::collections::HashMap;

/// What sort of class an FGD entry is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClassKind {
    /// Shared keys for other classes to include (`@BaseClass`); never
    /// placed.
    Base,
    /// An entity with a position (`@PointClass`, and the NPC, key frame,
    /// move and filter kinds, which are point entities too).
    Point,
    /// An entity made of brushes (`@SolidClass`).
    Solid,
}

/// A keyvalue's type, which decides how it is edited.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyType {
    /// Free text.
    String,
    /// A whole number.
    Integer,
    /// A number.
    Float,
    /// One of a list (`choices`).
    Choices,
    /// Several on/off flags added together (`flags`, as in spawnflags).
    Flags,
    /// A colour, "R G B" (and sometimes brightness), 0 to 255 each.
    Color255,
    /// A colour, "R G B" from 0 to 1.
    Color1,
    /// A model file (`studio`).
    Studio,
    /// A sound name.
    Sound,
    /// A material (`material`, `decal`, `sprite`).
    Material,
    /// The name this entity is known by (`target_source`).
    TargetSource,
    /// The name of another entity (`target_destination`, `filterclass`…).
    TargetDestination,
    /// Pitch, yaw and roll (`angle`).
    Angle,
    /// A position (`origin`, `vecline`).
    Vector,
    /// Any other type, by its FGD name.
    Other(String),
}

impl KeyType {
    /// The type for an FGD type name (any letter case).
    pub fn from_name(name: &str) -> Self {
        match name.to_ascii_lowercase().as_str() {
            "string" | "node_dest" | "pointentityclass" | "npcclass" | "scene" => Self::String,
            "integer" | "node_id" => Self::Integer,
            "float" => Self::Float,
            "choices" => Self::Choices,
            "flags" => Self::Flags,
            "color255" => Self::Color255,
            "color1" => Self::Color1,
            "studio" => Self::Studio,
            "sound" => Self::Sound,
            "material" | "decal" | "sprite" => Self::Material,
            "target_source" => Self::TargetSource,
            "target_destination" | "target_name_or_class" | "filterclass" => {
                Self::TargetDestination
            }
            "angle" | "angle_negative_pitch" => Self::Angle,
            "origin" | "vecline" | "vector" | "sidelist" => Self::Vector,
            other => Self::Other(other.to_string()),
        }
    }
}

/// One of a keyvalue's choices, or one of its flags.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Choice {
    /// The value written to the map ("1", "64", "models/x.mdl").
    pub value: String,
    /// What the editor shows.
    pub label: String,
    /// For a flag: whether it is on by default.
    pub on_by_default: bool,
}

/// One keyvalue a class has.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyDef {
    /// The key written to the map (`targetname`, `rendercolor`).
    pub name: String,
    /// How it is edited.
    pub kind: KeyType,
    /// What the editor shows ("Name", "Brightness").
    pub label: String,
    /// The value it has unless changed.
    pub default: Option<String>,
    /// A longer explanation, for a tooltip.
    pub description: String,
    /// The choices or flags, for [`KeyType::Choices`] and [`KeyType::Flags`].
    pub choices: Vec<Choice>,
    /// Shown but not edited.
    pub read_only: bool,
}

/// One input or output a class has.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IoDef {
    /// Its name (`Toggle`, `OnTrigger`).
    pub name: String,
    /// The type of value it takes or gives (`void`, `integer`, `string`…).
    pub param: String,
    /// What it does.
    pub description: String,
}

/// An editor helper on a class: `size(-8 -8 -8, 8 8 8)`, `color(255 0 0)`,
/// `studio("models/x.mdl")`, `iconsprite("editor/light.vmt")`…
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Helper {
    /// The helper's name, in lower case.
    pub name: String,
    /// Its arguments, as written (quotes taken off, spaces trimmed).
    pub args: Vec<String>,
}

/// One entity class, as written in its file (without its bases).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntityClass {
    /// Its class name (`light`, `prop_physics`).
    pub name: String,
    /// What sort of class it is.
    pub kind: ClassKind,
    /// What it is for.
    pub description: String,
    /// The classes whose keys, inputs and outputs it includes.
    pub bases: Vec<String>,
    /// Editor helpers other than `base`.
    pub helpers: Vec<Helper>,
    /// Its own keyvalues.
    pub keys: Vec<KeyDef>,
    /// Its own inputs.
    pub inputs: Vec<IoDef>,
    /// Its own outputs.
    pub outputs: Vec<IoDef>,
}

impl EntityClass {
    /// The helper called `name` (lower case), if it has one.
    pub fn helper(&self, name: &str) -> Option<&Helper> {
        self.helpers.iter().find(|h| h.name == name)
    }

    /// The `size(...)` helper as two corners, if present and readable.
    pub fn size(&self) -> Option<([f32; 3], [f32; 3])> {
        size_of(self.helper("size")?)
    }

    /// The `color(...)` helper, if present and readable.
    pub fn color(&self) -> Option<[u8; 3]> {
        color_of(self.helper("color")?)
    }
}

/// A `size(...)` helper's two corners.
fn size_of(helper: &Helper) -> Option<([f32; 3], [f32; 3])> {
    let numbers = |text: &str| -> Option<[f32; 3]> {
        let v: Vec<f32> = text
            .split_whitespace()
            .map(|n| n.parse::<f32>().ok().filter(|n| n.is_finite()))
            .collect::<Option<_>>()?;
        Some([*v.first()?, *v.get(1)?, *v.get(2)?])
    };
    match helper.args.as_slice() {
        // size(16 16 16) is a box of that size around the origin.
        [one] => numbers(one).map(|s| (s.map(|v| -v / 2.0), s.map(|v| v / 2.0))),
        [a, b, ..] => Some((numbers(a)?, numbers(b)?)),
        [] => None,
    }
}

/// A `color(...)` helper's colour.
fn color_of(helper: &Helper) -> Option<[u8; 3]> {
    let v: Vec<u8> = helper
        .args
        .first()?
        .split_whitespace()
        .map(|n| n.parse::<u8>().ok())
        .collect::<Option<_>>()?;
    Some([*v.first()?, *v.get(1)?, *v.get(2)?])
}

/// A problem found while reading FGD files: the file, the line, and what
/// was wrong. The rest of the file is still read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FgdProblem {
    /// The file it was found in.
    pub file: String,
    /// The line, from 1.
    pub line: usize,
    /// What was wrong, in plain words.
    pub message: String,
}

impl std::fmt::Display for FgdProblem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} line {}: {}", self.file, self.line, self.message)
    }
}

/// Every class read from a set of FGD files.
#[derive(Debug, Clone, Default)]
pub struct Fgd {
    /// Point and solid classes by lower-case name. A later definition
    /// replaces an earlier one, as in Hammer.
    pub(crate) classes: HashMap<String, EntityClass>,
    /// Base classes by lower-case name, kept apart: Valve's files have a
    /// base class `Light` and a point class `light`.
    pub(crate) bases: HashMap<String, EntityClass>,
    /// The classes in the order first defined.
    pub(crate) order: Vec<String>,
    /// `@mapsize`, if given.
    pub map_size: Option<(i64, i64)>,
    /// What couldn't be read.
    pub problems: Vec<FgdProblem>,
    /// The files read, in order.
    pub files: Vec<String>,
}

/// A class with everything from its bases included: what the editor uses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedClass {
    /// The class itself (its own description and kind).
    pub class: EntityClass,
    /// Keyvalues, bases' first, each key once (a later one replaces an
    /// earlier one in place).
    pub keys: Vec<KeyDef>,
    /// Inputs, each once.
    pub inputs: Vec<IoDef>,
    /// Outputs, each once.
    pub outputs: Vec<IoDef>,
    /// Helpers, the class's own first, then its bases'.
    pub helpers: Vec<Helper>,
}

/// How deep bases may include bases.
const MAX_BASE_DEPTH: usize = 32;

impl Fgd {
    /// Adds or replaces a class.
    pub(crate) fn add(&mut self, class: EntityClass) {
        let key = class.name.to_ascii_lowercase();
        if class.kind == ClassKind::Base {
            self.bases.insert(key, class);
            return;
        }
        if !self.classes.contains_key(&key) {
            self.order.push(key.clone());
        }
        self.classes.insert(key, class);
    }

    /// The point or solid class called `name` (any letter case), or else
    /// the base class, as written.
    pub fn class(&self, name: &str) -> Option<&EntityClass> {
        let key = name.to_ascii_lowercase();
        self.classes.get(&key).or_else(|| self.bases.get(&key))
    }

    /// The class a `base(...)` names: a base class first, else a point or
    /// solid class.
    fn base(&self, name: &str) -> Option<&EntityClass> {
        let key = name.to_ascii_lowercase();
        self.bases.get(&key).or_else(|| self.classes.get(&key))
    }

    /// Every class that can be placed (point and solid classes), in the
    /// order the files define them.
    pub fn placeable(&self) -> impl Iterator<Item = &EntityClass> {
        self.order
            .iter()
            .filter_map(|n| self.classes.get(n))
            .filter(|c| c.kind != ClassKind::Base)
    }

    /// How many classes there are (base classes included).
    pub fn len(&self) -> usize {
        self.classes.len() + self.bases.len()
    }

    /// True if no class was read.
    pub fn is_empty(&self) -> bool {
        self.classes.is_empty() && self.bases.is_empty()
    }

    /// The class called `name` with its bases' keys, inputs, outputs and
    /// helpers included. Bases that are missing, or include themselves,
    /// are left out.
    pub fn resolved(&self, name: &str) -> Option<ResolvedClass> {
        let class = self.class(name)?.clone();
        let mut chain = Vec::new();
        self.collect_bases(&class, 0, &mut chain);
        let mut out = ResolvedClass {
            class: class.clone(),
            keys: Vec::new(),
            inputs: Vec::new(),
            outputs: Vec::new(),
            helpers: class.helpers.clone(),
        };
        for c in chain.iter().chain(std::iter::once(&class)) {
            merge_by(&mut out.keys, &c.keys, |k| k.name.to_ascii_lowercase());
            merge_by(&mut out.inputs, &c.inputs, |k| k.name.to_ascii_lowercase());
            merge_by(&mut out.outputs, &c.outputs, |k| {
                k.name.to_ascii_lowercase()
            });
        }
        for c in chain.iter().rev() {
            for helper in &c.helpers {
                if !out.helpers.iter().any(|h| h.name == helper.name) {
                    out.helpers.push(helper.clone());
                }
            }
        }
        Some(out)
    }

    /// Every base of `class`, deepest first, each once.
    fn collect_bases(&self, class: &EntityClass, depth: usize, out: &mut Vec<EntityClass>) {
        if depth >= MAX_BASE_DEPTH {
            return;
        }
        let same = |a: &EntityClass, b: &EntityClass| {
            a.kind == b.kind && a.name.eq_ignore_ascii_case(&b.name)
        };
        for base in &class.bases {
            let Some(found) = self.base(base) else {
                continue;
            };
            if same(found, class) || out.iter().any(|c| same(c, found)) {
                continue;
            }
            self.collect_bases(found, depth + 1, out);
            if !out.iter().any(|c| same(c, found)) {
                out.push(found.clone());
            }
        }
    }
}

/// Adds `more` to `into`; an item whose key is already there replaces it
/// in place.
fn merge_by<T: Clone>(into: &mut Vec<T>, more: &[T], key: impl Fn(&T) -> String) {
    for item in more {
        let k = key(item);
        match into.iter_mut().find(|i| key(i) == k) {
            Some(existing) => *existing = item.clone(),
            None => into.push(item.clone()),
        }
    }
}

impl ResolvedClass {
    /// The keyvalue called `name` (any letter case).
    pub fn key(&self, name: &str) -> Option<&KeyDef> {
        self.keys.iter().find(|k| k.name.eq_ignore_ascii_case(name))
    }

    /// The helper called `name` (lower case), its own or a base's.
    pub fn helper(&self, name: &str) -> Option<&Helper> {
        self.helpers.iter().find(|h| h.name == name)
    }

    /// The size from its own or a base's `size(...)` helper.
    pub fn size(&self) -> Option<([f32; 3], [f32; 3])> {
        size_of(self.helper("size")?)
    }

    /// The colour from its own or a base's `color(...)` helper.
    pub fn color(&self) -> Option<[u8; 3]> {
        color_of(self.helper("color")?)
    }

    /// The model from its `studio("…")` helper, if it names one.
    pub fn model(&self) -> Option<&str> {
        self.helper("studio")
            .and_then(|h| h.args.first())
            .map(String::as_str)
            .filter(|m| !m.is_empty())
    }
}
