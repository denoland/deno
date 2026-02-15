// Copyright 2018-2026 the Deno authors. MIT license.

#![allow(clippy::disallowed_methods)]

use std::path::Path;

mod data;
mod structs;

fn main() {
  println!("cargo:rerun-if-changed=data.rs");
  println!("cargo:rerun-if-changed=structs.rs");

  let out_dir = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());

  let mut js_list = String::from(
    "export const unstableIds = {
",
  );

  let mut rs_list = String::from(
    "pub static UNSTABLE_FEATURES: &[crate::structs::UnstableFeatureDefinition] = &[\n",
  );

  let mut descriptions = data::FEATURE_DESCRIPTIONS.to_vec();
  descriptions.sort_by_key(|desc| desc.name);

  for (id, feature) in descriptions.iter().enumerate() {
    let flag_name = format!("unstable-{}", feature.name);
    let feature_kind = match feature.kind {
      structs::UnstableFeatureKind::Cli => {
        "crate::structs::UnstableFeatureKind::Cli"
      }
      structs::UnstableFeatureKind::Runtime => {
        "crate::structs::UnstableFeatureKind::Runtime"
      }
    };

    rs_list += &format!(
      r#"  crate::structs::UnstableFeatureDefinition {{
    name: "{}",
    flag_name: "{}",
    help_text: "{}",
    show_in_help: {},
    id: {},
    kind: {},
  }},
"#,
      feature.name,
      flag_name,
      feature.help_text,
      feature.show_in_help,
      id,
      feature_kind
    );

    if matches!(feature.kind, structs::UnstableFeatureKind::Runtime) {
      let camel = camel_case(feature.name);
      js_list += &format!("  {}: {},\n", camel, id);
    }
  }

  js_list += "};\n";
  rs_list += "];\n";

  let mut env_var_def = "pub struct UnstableEnvVarNames {\n".to_string();
  let mut env_var_impl =
    "pub static UNSTABLE_ENV_VAR_NAMES: UnstableEnvVarNames = UnstableEnvVarNames {\n"
      .to_string();
  for feature in &descriptions {
    let value = match feature.env_var {
      Some(v) => v,
      None => continue,
    };
    let prop_name = feature.name.replace("-", "_");
    env_var_def.push_str(&format!("  pub {}: &'static str,\n", prop_name));
    env_var_impl.push_str(&format!("  {}: \"{}\",\n", prop_name, value));
  }
  env_var_def.push_str("}\n");
  env_var_impl.push_str("};\n");

  rs_list.push_str(&env_var_def);
  rs_list.push_str(&env_var_impl);

  std::fs::write(out_dir.join("gen.js"), &js_list).unwrap();
  std::fs::write(out_dir.join("gen.rs"), &rs_list).unwrap();
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
