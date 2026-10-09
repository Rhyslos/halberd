//! Why a shape could not be made.

use std::fmt;

/// Why a shape could not be made. Messages are written for the user.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GeomError {
    /// A coordinate was not a number or infinite.
    NotFinite,
    /// The shape would be thinner than [`crate::MIN_SIZE`] on some axis.
    TooSmall,
    /// The shape would reach beyond [`crate::MAX_COORD`].
    TooLarge,
    /// More faces than [`crate::MAX_FACES`].
    TooManyFaces,
    /// The planes do not enclose a solid.
    NotClosed,
    /// A shape would need more than [`crate::MAX_SHAPE_BRUSHES`] brushes.
    TooManyPieces,
}

impl fmt::Display for GeomError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::NotFinite => "the shape has an invalid coordinate",
            Self::TooSmall => "the shape is too thin",
            Self::TooLarge => "the shape reaches beyond the edge of the world",
            Self::TooManyFaces => "the shape has too many faces",
            Self::NotClosed => "the faces do not enclose a solid shape",
            Self::TooManyPieces => {
                "it would need more than 256 pieces (for stairs: use taller steps or a lower height)"
            }
        })
    }
}

impl std::error::Error for GeomError {}
