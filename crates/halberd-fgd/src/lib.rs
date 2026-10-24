//! Read FGD entity definition files.
//!
//! FGD files list every entity a game supports: its class name, what sort
//! it is (point, solid, or a base for others), its keyvalues (with types,
//! labels, defaults and choices), inputs and outputs, and editor helpers
//! such as its size and colour. Garry's Mod's `garrysmod.fgd` includes
//! Half-Life 2's and the base game's.
//!
//! [`parse`] reads one file's text; [`load`] reads a file and the files it
//! includes. [`Fgd::resolved`] gives a class with everything from its
//! bases. Reading never panics: a class that can't be read is reported in
//! [`Fgd::problems`] (with its file and line) and the rest is still read.
//! Input is capped: file size, tokens, items per class, choices, includes.
//!
//! ```
//! let fgd = halberd_fgd::parse(
//!     r#"@BaseClass = Targetname [ targetname(target_source) : "Name" ]
//!        @PointClass base(Targetname) size(-8 -8 -8, 8 8 8) = info_target : "A point" []"#,
//!     "example.fgd",
//! );
//! let class = fgd.resolved("info_target").unwrap();
//! assert_eq!(class.keys[0].name, "targetname");
//! assert!(fgd.problems.is_empty());
//! ```

mod lex;
mod load;
mod model;
mod parse;

pub use load::{MAX_FGD_BYTES, load, parse};
pub use model::{
    Choice, ClassKind, EntityClass, Fgd, FgdProblem, Helper, IoDef, KeyDef, KeyType, ResolvedClass,
};

/// The name of this crate, used in logs and diagnostics.
pub const CRATE_NAME: &str = "halberd-fgd";

#[cfg(test)]
mod tests;
