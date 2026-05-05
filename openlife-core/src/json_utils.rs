//! Shared JSON extraction utilities.

/// Extract the first JSON object from text using brace balancing.
/// Returns the substring including the outermost braces.
pub fn extract_first_json_object(text: &str) -> Option<&str> {
    let start = text.find('{')?;
    let mut depth = 0;
    let mut in_string = false;
    let mut escape = false;
    for (idx, b) in text[start..].bytes().enumerate() {
        if escape {
            escape = false;
            continue;
        }
        if in_string {
            if b == b'\\' {
                escape = true;
                continue;
            }
            if b == b'"' {
                in_string = false;
            }
            continue;
        }
        if b == b'"' {
            in_string = true;
            continue;
        }
        if b == b'{' {
            depth += 1;
        } else if b == b'}' {
            depth -= 1;
            if depth == 0 {
                return Some(&text[start..=start + idx]);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_simple_json() {
        let text = r#"some text {"key": "value"} more text"#;
        assert_eq!(extract_first_json_object(text), Some(r#"{"key": "value"}"#));
    }

    #[test]
    fn test_extract_nested_json() {
        let text = r#"{"outer": {"inner": 1}}"#;
        assert_eq!(extract_first_json_object(text), Some(text));
    }

    #[test]
    fn test_no_json() {
        let text = "no json here";
        assert_eq!(extract_first_json_object(text), None);
    }
}
