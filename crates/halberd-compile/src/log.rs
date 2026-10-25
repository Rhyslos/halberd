//! Reading the compilers' output: what each line is, and the signs of a
//! leak.

/// What a line of compiler output is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineKind {
    /// Progress and information.
    Info,
    /// Something worth a look, but the compile goes on.
    Warning,
    /// Something went wrong.
    Error,
}

/// What `line` is.
pub fn classify(line: &str) -> LineKind {
    let lower = line.to_ascii_lowercase();
    if lower.contains("error") || lower.contains("leaked") || lower.contains("**** leak") {
        LineKind::Error
    } else if lower.contains("warning") {
        LineKind::Warning
    } else {
        LineKind::Info
    }
}

/// True if `line` says the map leaks (vbsp prints "**** leaked ****").
pub fn is_leak(line: &str) -> bool {
    line.to_ascii_lowercase().contains("leaked")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_are_sorted_by_what_they_say() {
        assert_eq!(classify("Processing areas...done (0)"), LineKind::Info);
        assert_eq!(classify("WARNING: brush has no sides"), LineKind::Warning);
        assert_eq!(classify("Error loading material"), LineKind::Error);
        assert_eq!(classify("**** leaked ****"), LineKind::Error);
        assert!(is_leak("**** leaked ****"));
        assert!(!is_leak("Building visibility clusters..."));
    }
}
