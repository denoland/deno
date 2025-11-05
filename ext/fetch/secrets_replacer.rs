// Copyright 2018-2025 the Deno authors. MIT license.

use std::collections::BTreeMap;

/// Secrets are specified via the environment key:
///   DENO_SECRETS_MANAGER="ANTHROPIC_API_KEY:secret1,NEXT_KEY:secret2"
/// The environment is then populated with:
///   ANTHROPIC_API_KEY=___D3N0_S3CR3T_1___
///   NEXT_KEY=___D3N0_S3CR3T_2___
/// The secret replacer then searches for this string to replace
/// in the provided string with the original value
pub struct SecretsReplacer {
  secrets: BTreeMap<u32, String>,
}

impl SecretsReplacer {
  /// Creates a new SecretsReplacer by parsing the DENO_SECRETS_MANAGER environment variable.
  /// Returns None if the environment variable is not set.
  /// Also sets environment variables with placeholder values.
  pub fn from_env(
    sys: &(impl sys_traits::EnvVar + sys_traits::EnvSetVar + sys_traits::Env),
  ) -> Self {
    const VAR_NAME: &str = "DENO_SECRETS_MANAGER";
    let env_value = sys.env_var(VAR_NAME).ok();
    set_env_placeholders(sys, &env_value);
    Self::new(&env_value)
  }

  /// Creates a new SecretsReplacer from a secrets manager string.
  /// Format: "KEY1:value1,KEY2:value2"
  fn new(secrets_manager: &str) -> Self {
    let mut secrets = BTreeMap::new();

    for (index, entry) in secrets_manager.split(',').enumerate() {
      if let Some((_key, value)) = entry.split_once(':') {
        secrets.insert((index + 1) as u32, value.to_string());
      }
    }

    Self { secrets }
  }

  /// Replaces all secret placeholders in the text with their actual values.
  /// Returns Some(new_string) if any replacements were made, None otherwise.
  pub fn replace(&self, text: &str) -> Option<String> {
    if self.secrets.is_empty() {
      return None;
    }

    const PREFIX: &str = "___D3N0_S3CR3T_";
    const SUFFIX: &str = "___";
    let prefix_len = PREFIX.len();
    let suffix_len = SUFFIX.len();

    let mut result: Option<String> = None;
    let mut last_pos = 0;
    let mut search_pos = 0;
    let bytes = text.as_bytes();
    let len = bytes.len();

    while search_pos < len {
      // Find next occurrence of prefix
      if let Some(prefix_pos) = text[search_pos..].find(PREFIX) {
        let abs_prefix_pos = search_pos + prefix_pos;
        let number_start = abs_prefix_pos + prefix_len;

        // Parse the number
        let mut number_end = number_start;
        while number_end < len && bytes[number_end].is_ascii_digit() {
          number_end += 1;
        }

        // Check if we have a valid suffix
        if number_end + suffix_len <= len
          && &text[number_end..number_end + suffix_len] == SUFFIX
          && number_start < number_end
        {
          // Parse the index
          if let Ok(index) = text[number_start..number_end].parse::<u32>() {
            if let Some(secret_value) = self.secrets.get(&index) {
              // Lazily allocate result only when we find the first replacement
              let output =
                result.get_or_insert_with(|| String::with_capacity(text.len()));
              // Append everything before the placeholder
              output.push_str(&text[last_pos..abs_prefix_pos]);
              // Append the secret value
              output.push_str(secret_value);
              // Move past the placeholder
              last_pos = number_end + suffix_len;
              search_pos = number_end + suffix_len;
              continue;
            }
          }
        }

        // Not a valid placeholder, skip past the prefix and keep searching
        search_pos = abs_prefix_pos + prefix_len;
      } else {
        // No more occurrences, copy the rest
        if let Some(output) = result.as_mut() {
          output.push_str(&text[last_pos..]);
        }
        break;
      }
    }

    result
  }
}

/// Sets environment variables with placeholder values based on the secrets manager string.
fn set_env_placeholders(
  sys: &impl sys_traits::EnvSetVar,
  secrets_manager: &str,
) {
  for (index, entry) in secrets_manager.split(',').enumerate() {
    if let Some((key, _value)) = entry.split_once(':') {
      let placeholder = format!("___D3N0_S3CR3T_{}___", index + 1);
      sys.env_set_var(key, placeholder);
    }
  }
}

#[cfg(test)]
mod tests {
  use sys_traits::EnvSetVar;
  use sys_traits::EnvVar;
  use sys_traits::impls::InMemorySys;

  use super::*;

  #[test]
  fn test_new_empty() {
    let replacer = SecretsReplacer::new("");
    assert!(replacer.secrets.is_empty());
  }

  #[test]
  fn test_new_single_secret() {
    let replacer = SecretsReplacer::new("API_KEY:secret123");
    assert_eq!(replacer.secrets.len(), 1);
    assert_eq!(replacer.secrets.get(&1), Some(&"secret123".to_string()));
  }

  #[test]
  fn test_new_multiple_secrets() {
    let replacer =
      SecretsReplacer::new("API_KEY:secret123,DB_PASS:pass456,TOKEN:token789");
    assert_eq!(replacer.secrets.len(), 3);
    assert_eq!(replacer.secrets.get(&1), Some(&"secret123".to_string()));
    assert_eq!(replacer.secrets.get(&2), Some(&"pass456".to_string()));
    assert_eq!(replacer.secrets.get(&3), Some(&"token789".to_string()));
  }

  #[test]
  fn test_new_malformed_entries() {
    let replacer = SecretsReplacer::new("GOOD:value1,NOCOLON,ALSO_GOOD:value2");
    assert_eq!(replacer.secrets.len(), 2);
    assert_eq!(replacer.secrets.get(&1), Some(&"value1".to_string()));
    assert_eq!(replacer.secrets.get(&3), Some(&"value2".to_string()));
  }

  #[test]
  fn test_replace_no_secrets() {
    let replacer = SecretsReplacer::new("");
    let result = replacer.replace("Some text with ___D3N0_S3CR3T_1___");
    assert_eq!(result, None);
  }

  #[test]
  fn test_replace_no_placeholders() {
    let replacer = SecretsReplacer::new("API_KEY:secret123");
    let result = replacer.replace("Some text without placeholders");
    assert_eq!(result, None);
  }

  #[test]
  fn test_replace_single_placeholder() {
    let replacer = SecretsReplacer::new("API_KEY:my_secret_key");
    let result = replacer.replace("Bearer ___D3N0_S3CR3T_1___");
    assert_eq!(result, Some("Bearer my_secret_key".to_string()));
  }

  #[test]
  fn test_replace_multiple_placeholders() {
    let replacer = SecretsReplacer::new("KEY1:secret1,KEY2:secret2");
    let result = replacer
      .replace("First: ___D3N0_S3CR3T_1___ Second: ___D3N0_S3CR3T_2___");
    assert_eq!(result, Some("First: secret1 Second: secret2".to_string()));
  }

  #[test]
  fn test_replace_same_placeholder_multiple_times() {
    let replacer = SecretsReplacer::new("KEY:secret");
    let result =
      replacer.replace("___D3N0_S3CR3T_1___ and ___D3N0_S3CR3T_1___ again");
    assert_eq!(result, Some("secret and secret again".to_string()));
  }

  #[test]
  fn test_replace_nonexistent_index() {
    let replacer = SecretsReplacer::new("KEY:secret");
    let result = replacer.replace("___D3N0_S3CR3T_99___");
    assert_eq!(result, None);
  }

  #[test]
  fn test_replace_invalid_placeholder_format() {
    let replacer = SecretsReplacer::new("KEY:secret");
    // Missing suffix
    let result = replacer.replace("___D3N0_S3CR3T_1");
    assert_eq!(result, None);
    // Missing number
    let result = replacer.replace("___D3N0_S3CR3T____");
    assert_eq!(result, None);
    // Non-numeric index
    let result = replacer.replace("___D3N0_S3CR3T_abc___");
    assert_eq!(result, None);
  }

  #[test]
  fn test_replace_partial_matches() {
    let replacer = SecretsReplacer::new("KEY:secret");
    // Text that contains the prefix but not a valid placeholder
    let result =
      replacer.replace("___D3N0_S3CR3T_1___ and ___D3N0_S3CR3T_invalid");
    assert_eq!(
      result,
      Some("secret and ___D3N0_S3CR3T_invalid".to_string())
    );
  }

  #[test]
  fn test_replace_in_json() {
    let replacer = SecretsReplacer::new("API_KEY:sk-1234567890");
    let json = r#"{"api_key":"___D3N0_S3CR3T_1___","endpoint":"https://api.example.com"}"#;
    let result = replacer.replace(json);
    assert_eq!(
      result,
      Some(
        r#"{"api_key":"sk-1234567890","endpoint":"https://api.example.com"}"#
          .to_string()
      )
    );
  }

  #[test]
  fn test_replace_at_boundaries() {
    let replacer = SecretsReplacer::new("KEY:secret");
    // At start
    let result = replacer.replace("___D3N0_S3CR3T_1___ end");
    assert_eq!(result, Some("secret end".to_string()));
    // At end
    let result = replacer.replace("start ___D3N0_S3CR3T_1___");
    assert_eq!(result, Some("start secret".to_string()));
    // Entire string
    let result = replacer.replace("___D3N0_S3CR3T_1___");
    assert_eq!(result, Some("secret".to_string()));
  }

  #[test]
  fn test_replace_with_special_characters() {
    let replacer = SecretsReplacer::new("KEY:p@ss$w0rd!#%");
    let result = replacer.replace("Password: ___D3N0_S3CR3T_1___");
    assert_eq!(result, Some("Password: p@ss$w0rd!#%".to_string()));
  }

  #[test]
  fn test_set_env_placeholders() {
    let sys = InMemorySys::default();
    set_env_placeholders(&sys, "KEY1:value1,KEY2:value2");
    assert_eq!(
      sys.env_var("KEY1").ok(),
      Some("___D3N0_S3CR3T_1___".to_string())
    );
    assert_eq!(
      sys.env_var("KEY2").ok(),
      Some("___D3N0_S3CR3T_2___".to_string())
    );
  }

  #[test]
  fn test_from_env() {
    let sys = InMemorySys::default();
    sys
      .env_set_var("DENO_SECRETS_MANAGER", "API_KEY:secret123,DB_PASS:pass456");

    let replacer = SecretsReplacer::from_env(&sys);

    // Check that placeholders were set
    assert_eq!(
      sys.env_var("API_KEY").ok(),
      Some("___D3N0_S3CR3T_1___".to_string())
    );
    assert_eq!(
      sys.env_var("DB_PASS").ok(),
      Some("___D3N0_S3CR3T_2___".to_string())
    );

    // Check that secrets can be replaced
    let result = replacer.replace("Using ___D3N0_S3CR3T_1___");
    assert_eq!(result, Some("Using secret123".to_string()));
  }

  #[test]
  fn test_from_env_empty() {
    let sys = InMemorySys::default();
    // Don't set DENO_SECRETS_MANAGER
    let replacer = SecretsReplacer::from_env(&sys);
    assert!(replacer.secrets.is_empty());
  }

  #[test]
  fn test_replace_preserves_surrounding_text() {
    let replacer = SecretsReplacer::new("KEY:SECRET");
    let input = "before ___D3N0_S3CR3T_1___ middle ___D3N0_S3CR3T_1___ after";
    let result = replacer.replace(input);
    assert_eq!(
      result,
      Some("before SECRET middle SECRET after".to_string())
    );
  }

  #[test]
  fn test_replace_large_index() {
    let replacer =
      SecretsReplacer::new("A:1,B:2,C:3,D:4,E:5,F:6,G:7,H:8,I:9,J:10");
    let result = replacer.replace("Value: ___D3N0_S3CR3T_10___");
    assert_eq!(result, Some("Value: 10".to_string()));
  }

  #[test]
  fn test_replace_mixed_valid_invalid() {
    let replacer = SecretsReplacer::new("KEY:secret");
    let input = "___D3N0_S3CR3T_1___ valid, ___D3N0_S3CR3T_2___ invalid, ___D3N0_S3CR3T_1___ valid again";
    let result = replacer.replace(input);
    // Only index 1 should be replaced
    assert_eq!(
      result,
      Some(
        "secret valid, ___D3N0_S3CR3T_2___ invalid, secret valid again"
          .to_string()
      )
    );
  }
}
