//! Fast single-pass lexical secret scanner and redactor.
//!
//! Protects LLM context payloads from accidentally leaking hardcoded credentials:
//! - OpenAI API keys (`sk-...`, `sk-proj-...`)
//! - Anthropic API keys (`sk-ant-...`)
//! - GitHub Personal & Fine-Grained tokens (`ghp_...`, `gho_...`, `github_pat_...`)
//! - AWS Access Key IDs (`AKIA` + 16 uppercase alphanumeric characters)
//! - PEM / OpenSSH Private Key blocks (`-----BEGIN ... PRIVATE KEY-----`)

/// Replacement placeholder substituted for every detected secret.
pub const REDACTED_PLACEHOLDER: &str = "[REDACTED_SECRET]";

/// Scans `content` in a single lexical pass, replacing any recognized secret tokens
/// or private key blocks with `[REDACTED_SECRET]`.
///
/// Returns `(redacted_content, redacted_count)`.
pub fn redact_secrets(content: &str) -> (String, usize) {
    let bytes = content.as_bytes();
    let len = bytes.len();
    let mut out = String::with_capacity(len);
    let mut count = 0usize;
    let mut last_copied = 0usize;
    let mut i = 0usize;

    while i < len {
        // 1. Check for PEM / OpenSSH private key block: `-----BEGIN ... PRIVATE KEY-----`
        if bytes[i] == b'-'
            && let Some(secret_end) = match_private_key_block(&content[i..])
        {
            out.push_str(&content[last_copied..i]);
            out.push_str(REDACTED_PLACEHOLDER);
            count += 1;
            i += secret_end;
            last_copied = i;
            continue;
        }

        // Token-based secrets must start on a non-identifier boundary
        let at_boundary = i == 0 || !is_token_char(bytes[i - 1]);
        if at_boundary {
            // 2. Check OpenAI (`sk-...`, `sk-proj-...`) and Anthropic (`sk-ant-...`) keys
            if bytes[i] == b's'
                && i + 3 < len
                && bytes[i + 1] == b'k'
                && bytes[i + 2] == b'-'
                && let Some(token_len) = match_sk_secret(&content[i..])
            {
                out.push_str(&content[last_copied..i]);
                out.push_str(REDACTED_PLACEHOLDER);
                count += 1;
                i += token_len;
                last_copied = i;
                continue;
            }

            // 3. Check GitHub tokens (`ghp_...`, `gho_...`, `github_pat_...`)
            if bytes[i] == b'g'
                && let Some(token_len) = match_github_secret(&content[i..])
            {
                out.push_str(&content[last_copied..i]);
                out.push_str(REDACTED_PLACEHOLDER);
                count += 1;
                i += token_len;
                last_copied = i;
                continue;
            }

            // 4. Check AWS Access Key IDs (`AKIA` + 16 uppercase alphanumeric chars)
            if bytes[i] == b'A'
                && i + 20 <= len
                && &bytes[i..i + 4] == b"AKIA"
                && let Some(token_len) = match_aws_access_key(&content[i..])
            {
                out.push_str(&content[last_copied..i]);
                out.push_str(REDACTED_PLACEHOLDER);
                count += 1;
                i += token_len;
                last_copied = i;
                continue;
            }
        }

        i += 1;
    }

    if count == 0 {
        return (content.to_string(), 0);
    }

    out.push_str(&content[last_copied..]);
    (out, count)
}

#[inline]
fn is_token_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b'-'
}

/// Matches `-----BEGIN ... PRIVATE KEY-----` and, if present, spans through the
/// matching `-----END ... PRIVATE KEY-----` footer.
fn match_private_key_block(slice: &str) -> Option<usize> {
    let begin_prefix = "-----BEGIN ";
    if !slice.starts_with(begin_prefix) {
        return None;
    }

    let first_line_end = slice.find('\n').unwrap_or(slice.len());
    let first_line = &slice[..first_line_end];
    let pk_suffix = "PRIVATE KEY-----";
    let header_rel_end = first_line.find(pk_suffix)?;
    let header_label = first_line[begin_prefix.len()..header_rel_end].trim();
    if header_label.len() > 32
        || !header_label
            .bytes()
            .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b' ')
    {
        return None;
    }

    let header_end = header_rel_end + pk_suffix.len();

    // Look for a matching `-----END ... PRIVATE KEY-----` block
    let rest = &slice[header_end..];
    let mut search_offset = 0usize;
    while let Some(end_idx) = rest[search_offset..].find("-----END ") {
        let marker_start = search_offset + end_idx;
        let line_end = rest[marker_start..]
            .find('\n')
            .map_or(rest.len(), |nl| marker_start + nl);
        let end_line = &rest[marker_start..line_end];
        if let Some(pk_end) = end_line.find(pk_suffix) {
            return Some(header_end + marker_start + pk_end + pk_suffix.len());
        }
        search_offset = marker_start + "-----END ".len();
    }

    Some(header_end)
}

/// Matches `sk-ant-...`, `sk-proj-...`, or `sk-...` tokens with >= 20 characters after the prefix.
fn match_sk_secret(slice: &str) -> Option<usize> {
    let token_len = slice
        .bytes()
        .take_while(|&b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
        .count();
    let token = &slice[..token_len];

    if let Some(rest) = token.strip_prefix("sk-ant-") {
        if rest.len() >= 20 && rest.bytes().filter(|b| b.is_ascii_alphanumeric()).count() >= 16 {
            return Some(token_len);
        }
        return None;
    }

    if let Some(rest) = token.strip_prefix("sk-proj-") {
        let alnum_count = rest.bytes().filter(|b| b.is_ascii_alphanumeric()).count();
        if rest.len() >= 20 && alnum_count >= 20 {
            return Some(token_len);
        }
        return None;
    }

    if let Some(rest) = token.strip_prefix("sk-") {
        let alnum_count = rest.bytes().filter(|b| b.is_ascii_alphanumeric()).count();
        if rest.len() >= 20 && alnum_count >= 20 {
            return Some(token_len);
        }
    }

    None
}

/// Matches GitHub tokens (`ghp_...`, `gho_...`, `ghu_...`, `ghs_...`, `ghr_...`, `github_pat_...`)
/// with >= 20 characters following the prefix.
fn match_github_secret(slice: &str) -> Option<usize> {
    let prefixes = ["github_pat_", "ghp_", "gho_", "ghu_", "ghs_", "ghr_"];
    let matched_prefix = prefixes.into_iter().find(|&p| slice.starts_with(p))?;

    let token_len = slice
        .bytes()
        .take_while(|&b| b.is_ascii_alphanumeric() || b == b'_')
        .count();
    let rest_len = token_len.saturating_sub(matched_prefix.len());

    if rest_len >= 20 {
        Some(token_len)
    } else {
        None
    }
}

/// Matches AWS Access Key IDs (`AKIA` followed by exactly 16 uppercase ASCII alphanumeric chars).
fn match_aws_access_key(slice: &str) -> Option<usize> {
    let token_len = slice
        .bytes()
        .take_while(|&b| b.is_ascii_alphanumeric() || b == b'_')
        .count();
    if token_len != 20 {
        return None;
    }

    let suffix = &slice.as_bytes()[4..20];
    if suffix
        .iter()
        .all(|&b| b.is_ascii_uppercase() || b.is_ascii_digit())
    {
        Some(20)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_redact_openai_and_anthropic_keys() {
        let input = r#"
const OPENAI_KEY: &str = "sk-proj-AbCdEfGhIjKlMnOpQrStUvWxYz0123456789";
const LEGACY_OPENAI: &str = "sk-1234567890abcdefghijklmnop";
const ANTHROPIC_KEY: &str = "sk-ant-api03-AbCdEfGhIjKlMnOpQrStUvWxYz012345";
const SHORT_ID: &str = "sk-short123";
"#;

        let (redacted, count) = redact_secrets(input);
        assert_eq!(count, 3);
        assert!(redacted.contains("const OPENAI_KEY: &str = \"[REDACTED_SECRET]\";"));
        assert!(redacted.contains("const LEGACY_OPENAI: &str = \"[REDACTED_SECRET]\";"));
        assert!(redacted.contains("const ANTHROPIC_KEY: &str = \"[REDACTED_SECRET]\";"));
        assert!(redacted.contains("const SHORT_ID: &str = \"sk-short123\";"));
    }

    #[test]
    fn test_redact_github_and_aws_tokens() {
        let input = r#"
export GITHUB_TOKEN=ghp_1234567890abcdefghijKLMNOPQRST
export OAUTH_TOKEN=gho_abcdefghijklmnopqrst1234567890
export FINE_GRAINED=github_pat_11AABBCCDDEEFF0123456789_abcdefghijklmnop
export AWS_ACCESS_KEY_ID="AKIAIOSFODNN7EXAMPLE"
export NOT_AWS="AKIAIOSFODNN7example"
"#;

        let (redacted, count) = redact_secrets(input);
        assert_eq!(count, 4);
        assert!(redacted.contains("GITHUB_TOKEN=[REDACTED_SECRET]"));
        assert!(redacted.contains("OAUTH_TOKEN=[REDACTED_SECRET]"));
        assert!(redacted.contains("FINE_GRAINED=[REDACTED_SECRET]"));
        assert!(redacted.contains("AWS_ACCESS_KEY_ID=\"[REDACTED_SECRET]\""));
        assert!(redacted.contains("NOT_AWS=\"AKIAIOSFODNN7example\""));
    }

    #[test]
    fn test_redact_private_key_blocks() {
        let input = r#"
let cert = "-----BEGIN RSA PRIVATE KEY-----
MIIEowIBAAKCAQEA0123456789abcdefghijklmnopqrstuvwxyz
-----END RSA PRIVATE KEY-----";
let single_header = "-----BEGIN OPENSSH PRIVATE KEY-----";
"#;

        let (redacted, count) = redact_secrets(input);
        assert_eq!(count, 2);
        assert!(redacted.contains("let cert = \"[REDACTED_SECRET]\";"));
        assert!(redacted.contains("let single_header = \"[REDACTED_SECRET]\";"));
        assert!(!redacted.contains("MIIEowIBAAKCAQEA"));
    }

    #[test]
    fn test_clean_source_passes_unchanged() {
        let input = "fn main() {\n    println!(\"Hello, repOx!\");\n}\n";
        let (redacted, count) = redact_secrets(input);
        assert_eq!(count, 0);
        assert_eq!(redacted, input);
    }
}
