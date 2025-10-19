use anyhow::Result;

pub fn normalize_mime_input(input: &str) -> Result<String> {
    let trimmed = input.trim();

    if trimmed.contains('*') {
        return Ok(trimmed.to_string());
    }

    if let Some((type_part_raw, subtype_raw)) = trimmed.split_once('/') {
        let type_part = type_part_raw.trim().to_ascii_lowercase();
        let subtype = subtype_raw.trim().to_ascii_lowercase();

        if subtype.is_empty() {
            anyhow::bail!("Invalid MIME type: {}", input);
        }

        if let Some(guess) = mime_guess::from_ext(subtype.as_str()).first() {
            if guess.type_().as_str() == type_part {
                return Ok(guess.essence_str().to_string());
            }
        }

        let candidate = format!("{type_part}/{subtype}");
        if let Ok(parsed) = candidate.parse::<mime::Mime>() {
            return Ok(parsed.essence_str().to_string());
        }

        anyhow::bail!("Invalid MIME type: {}", input);
    }

    let normalized = trimmed.trim_start_matches('.');
    mime_guess::from_ext(normalized)
        .first()
        .map(|mime| mime.essence_str().to_string())
        .ok_or_else(|| anyhow::anyhow!("Unable to resolve MIME type for extension: {}", input))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_extension() {
        assert_eq!(normalize_mime_input(".txt").unwrap(), "text/plain");
    }

    #[test]
    fn normalizes_mime_case() {
        assert_eq!(normalize_mime_input("image/jpeg").unwrap(), "image/jpeg");
        assert_eq!(normalize_mime_input("image/JPG").unwrap(), "image/jpeg");
        assert_eq!(normalize_mime_input("image/png").unwrap(), "image/png");
    }

    #[test]
    fn preserves_wildcard() {
        assert_eq!(normalize_mime_input("image/*").unwrap(), "image/*");
    }

    #[test]
    fn rejects_invalid_mime() {
        let err = normalize_mime_input("invalid/").unwrap_err();
        assert!(format!("{err}").contains("Invalid MIME type"));
    }
}
