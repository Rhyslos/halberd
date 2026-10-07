//! Read and write Valve's KeyValues text format.
//!
//! KeyValues is the nested `"key" "value"` text format behind VMF maps, VMT materials and Steam's VDF files. This crate turns that text into a plain tree and back, exactly, so other format crates can build on it.
//!
//! Status: skeleton. See this crate's README for what it must never do.

/// The name of this crate, used in logs and diagnostics.
pub const CRATE_NAME: &str = "halberd-kv";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crate_name_matches_package() {
        assert_eq!(CRATE_NAME, env!("CARGO_PKG_NAME"));
    }
}
