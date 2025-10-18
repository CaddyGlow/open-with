use wildmatch::WildMatch;

pub fn matches(pattern: &str, target: &str) -> bool {
    if pattern.eq_ignore_ascii_case(target) {
        return true;
    }

    let pattern = pattern.trim();
    let target = target.trim();

    if pattern.is_empty() || target.is_empty() {
        return false;
    }

    let pattern_norm = pattern.to_ascii_lowercase();
    let target_norm = target.to_ascii_lowercase();

    if pattern_norm == target_norm {
        return true;
    }

    if !pattern_norm.contains('/') || !target_norm.contains('/') {
        return false;
    }

    if pattern_norm.contains('*') || pattern_norm.contains('?') {
        return WildMatch::new(&pattern_norm).matches(&target_norm);
    }

    pattern_norm == target_norm
}

#[cfg(test)]
mod tests {
    use super::matches;

    #[test]
    fn exact_match_is_true() {
        assert!(matches("image/jpeg", "image/jpeg"));
    }

    #[test]
    fn wildcard_match_is_true() {
        assert!(matches("image/*", "image/png"));
    }

    #[test]
    fn mismatched_types_are_false() {
        assert!(!matches("text/*", "image/png"));
    }

    #[test]
    fn case_insensitive_match() {
        assert!(matches("APPLICATION/JSON", "application/json"));
    }

    #[test]
    fn empty_inputs_do_not_match() {
        assert!(!matches("", "application/json"));
        assert!(!matches("text/plain", ""));
    }
}
