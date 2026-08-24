//! Comparing version strings.

/// Is `candidate` newer than `current`?
///
/// Compared component by component as numbers, so `1.10` is correctly newer
/// than `1.9` — which string comparison gets backwards, and which is the
/// classic way an updater ends up nagging about a version the user already has.
/// Non-numeric parts are ignored for ordering but a longer version with a
/// trailing number still wins, so `6.34.1` beats `6.34`.
pub fn is_newer(candidate: &str, current: &str) -> bool {
    let a = components(candidate);
    let b = components(current);
    for i in 0..a.len().max(b.len()) {
        let x = a.get(i).copied().unwrap_or(0);
        let y = b.get(i).copied().unwrap_or(0);
        if x != y {
            return x > y;
        }
    }
    false
}

/// Split on anything that is not a digit, keeping the numbers.
fn components(v: &str) -> Vec<u64> {
    v.split(|c: char| !c.is_ascii_digit())
        .filter(|s| !s.is_empty())
        .filter_map(|s| s.parse::<u64>().ok())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compares_numerically_not_lexically() {
        assert!(is_newer("1.10", "1.9"));
        assert!(!is_newer("1.9", "1.10"));
        assert!(is_newer("151.0.7922.174", "151.0.7922.170"));
        assert!(!is_newer("151.0.7922.170", "151.0.7922.174"));
    }

    #[test]
    fn equal_versions_are_not_newer() {
        assert!(!is_newer("6.34.1", "6.34.1"));
        assert!(!is_newer("2026.1.4", "2026.1.4"));
    }

    #[test]
    fn a_longer_version_is_newer_only_when_it_adds_something() {
        assert!(is_newer("6.34.1", "6.34"));
        assert!(!is_newer("6.34.0", "6.34"));
        assert!(!is_newer("6.34", "6.34.1"));
    }

    #[test]
    fn decorations_do_not_confuse_it() {
        assert!(is_newer("v2.1.0", "2.0.9"));
        assert!(is_newer("2026.2.0", "2026.1.4"));
        assert!(!is_newer("1.0.0-beta", "1.0.0"));
    }

    #[test]
    fn a_version_with_no_digits_never_wins() {
        assert!(!is_newer("unknown", "1.0"));
    }
}
