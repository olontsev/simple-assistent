use crate::config::Profile;

/// Split a profile args string into tokens, respecting double quotes.
pub fn split_args(input: &str) -> Result<Vec<String>, String> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut chars = input.chars().peekable();

    while let Some(c) = chars.next() {
        match c {
            '"' => {
                in_quotes = !in_quotes;
                current.push(c);
            }
            '\\' if in_quotes => {
                if let Some(next) = chars.next() {
                    current.push('\\');
                    current.push(next);
                } else {
                    current.push('\\');
                }
            }
            c if c.is_whitespace() && !in_quotes => {
                if !current.is_empty() {
                    args.push(current.clone());
                    current.clear();
                }
            }
            _ => current.push(c),
        }
    }

    if in_quotes {
        return Err("Незакрытая кавычка в аргументах профиля".into());
    }
    if !current.is_empty() {
        args.push(current);
    }
    Ok(args)
}

pub fn validate_profile_args(args: &str) -> Result<(), String> {
    let tokens = split_args(args)?;
    for (i, token) in tokens.iter().enumerate() {
        let bare = token.trim_matches('"');
        if bare == "-m"
            || bare == "--model"
            || bare == "--alias"
            || bare.starts_with("-m=")
            || bare.starts_with("--model=")
            || bare.starts_with("--alias=")
        {
            return Err(
                "Профиль не должен содержать -m / --model / --alias — модель выбирается отдельно"
                    .into(),
            );
        }
        // also flag if previous was -m without = form already caught
        if i > 0 {
            let prev = tokens[i - 1].trim_matches('"');
            if prev == "-m" || prev == "--model" || prev == "--alias" {
                // already caught above when scanning prev
            }
        }
    }
    Ok(())
}

pub fn extract_host_port(profile: Option<&Profile>) -> (String, u16) {
    let default = ("127.0.0.1".to_string(), 8080u16);
    let Some(profile) = profile else {
        return default;
    };
    let Ok(tokens) = split_args(&profile.args) else {
        return default;
    };

    let mut host = "127.0.0.1".to_string();
    let mut port = 8080u16;

    let mut i = 0;
    while i < tokens.len() {
        let t = tokens[i].trim_matches('"');
        if t == "--host" || t == "-h" {
            if let Some(v) = tokens.get(i + 1) {
                host = v.trim_matches('"').to_string();
                i += 2;
                continue;
            }
        } else if let Some(v) = t.strip_prefix("--host=") {
            host = v.trim_matches('"').to_string();
        } else if t == "--port" || t == "-p" {
            if let Some(v) = tokens.get(i + 1) {
                if let Ok(p) = v.trim_matches('"').parse::<u16>() {
                    port = p;
                }
                i += 2;
                continue;
            }
        } else if let Some(v) = t.strip_prefix("--port=") {
            if let Ok(p) = v.trim_matches('"').parse::<u16>() {
                port = p;
            }
        }
        i += 1;
    }

    // Health check should hit localhost even if bind is 0.0.0.0
    if host == "0.0.0.0" || host == "::" || host == "[::]" {
        host = "127.0.0.1".to_string();
    }

    (host, port)
}

pub fn model_alias(model_path: &str) -> String {
    std::path::Path::new(model_path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("model")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_quoted_json() {
        let args = r#"--host 0.0.0.0 --port 8080 --chat-template-kwargs "{\"reasoning_effort\":\"low\"}""#;
        let tokens = split_args(args).unwrap();
        assert!(tokens.iter().any(|t| t.contains("reasoning_effort")));
        assert_eq!(tokens.iter().position(|t| t == "--port").map(|i| &tokens[i + 1]), Some(&"8080".to_string()));
    }

    #[test]
    fn rejects_model_flag() {
        assert!(validate_profile_args("-ngl 99 -m foo.gguf").is_err());
        assert!(validate_profile_args("-ngl 99 --alias x").is_err());
        assert!(validate_profile_args("-ngl 99 --port 8080").is_ok());
    }
}
