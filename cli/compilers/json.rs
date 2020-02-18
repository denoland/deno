// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use crate::compilers::CompiledModule;
use crate::file_fetcher::SourceFile;
use deno_core::ErrBox;
use regex::Regex;

// From https://github.com/mathiasbynens/mothereff.in/blob/master/js-variables/eff.js
static JS_RESERVED_WORDS: &str = r"^(?:do|if|in|for|let|new|try|var|case|else|enum|eval|false|null|this|true|void|with|await|break|catch|class|const|super|throw|while|yield|delete|export|import|public|return|static|switch|typeof|default|extends|finally|package|private|continue|debugger|function|arguments|interface|protected|implements|instanceof)$";

pub struct JsonCompiler {}

impl JsonCompiler {
  pub async fn compile_async(
    &self,
    source_file: &SourceFile,
  ) -> Result<CompiledModule, ErrBox> {
    let maybe_json_value = serde_json::from_slice(&source_file.source_code);
    if let Err(err) = maybe_json_value {
      return Err(ErrBox::from(err));
    }

    let mut code = format!(
      "export default {};\n",
      std::str::from_utf8(&source_file.source_code).unwrap()
    );

    if let serde_json::Value::Object(m) = maybe_json_value.unwrap() {
      // Best effort variable name exports
      // Actual all allowed JS variable names are way tricker.
      // We only handle a subset of alphanumeric names.
      let js_var_regex = Regex::new(r"^[a-zA-Z_$][0-9a-zA-Z_$]*$").unwrap();
      // Also avoid collision with reserved words.
      let reserved_words = Regex::new(JS_RESERVED_WORDS).unwrap();
      for (key, value) in m.iter() {
        if js_var_regex.is_match(&key) && !reserved_words.is_match(&key) {
          code.push_str(&format!(
            "export const {} = {};\n",
            key,
            value.to_string()
          ));
        }
      }
    }

    Ok(CompiledModule {
      code,
      name: source_file.url.to_string(),
    })
  }
}
