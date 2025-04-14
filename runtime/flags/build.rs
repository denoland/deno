// Copyright 2018-2025 the Deno authors. MIT license.

#![allow(clippy::disallowed_methods)]

#[derive(serde::Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct JsonEntry {
  name: String,
  text: String,
  show_in_help: bool,
}

fn main() {
  let mut entries: Vec<JsonEntry> =
    serde_json::from_str(include_str!("./flags.json")).unwrap();

  entries.sort_by(|a, b| a.name.cmp(&b.name));

  let mut rust_list = String::from(
    "
#[derive(Debug)]
pub struct FlagDefinition {
  pub name: &'static str,
  pub help_text: &'static str,
  pub show_in_help: bool,
  pub id: i32,
}

pub static FLAGS: &[FlagDefinition] = &[
",
  );

  let mut js_list = String::from("export const unstableIds = {\n");

  for (id, entry) in entries.iter().enumerate() {
    let camel = camel_case(&entry.name);
    rust_list += &format!(
      r#"  FlagDefinition {{ name: {:?}, help_text: {:?}, show_in_help: {}, id: {} }},{}"#,
      entry.name, entry.text, entry.show_in_help, id, "\n"
    );
    js_list += &format!("  {}: {},\n", camel, id);
  }

  rust_list += "];";
  js_list += "};";

  let out_dir = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());

  std::fs::write(out_dir.join("rust_list.rs"), rust_list).unwrap();
  std::fs::write(out_dir.join("js_list.js"), js_list).unwrap();
}

fn camel_case(name: &str) -> String {
  let mut output = String::new();
  let mut upper = false;
  for c in name.chars() {
    if c == '-' {
      upper = true;
    } else if upper {
      upper = false;
      output.push(c.to_ascii_uppercase());
    } else {
      output.push(c);
    }
  }
  output
}
