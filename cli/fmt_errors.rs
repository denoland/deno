// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::env;
use std::path::Path;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use color_print::cformat;
use color_print::cstr;
use deno_ast::MediaType;
use deno_ast::ModuleSpecifier;
use deno_core::anyhow::anyhow;
use deno_core::error::AnyError;
use deno_core::error::JsError;
use deno_core::ModuleLoader;
use deno_core::ResolutionKind;
use deno_runtime::fmt_errors::find_recursive_cause;
use deno_runtime::fmt_errors::format_js_error_inner;
use deno_runtime::fmt_errors::FixSuggestion;
use deno_runtime::fmt_errors::IndexedErrorReference;
use regex::Regex;

use crate::args::flags_from_vec;
use crate::factory::CliFactory;
use crate::util::fs::specifier_from_file_path;

/// # Suggestions for non-explicit CommonJS imports
///
/// A package may have CommonJS modules that are not all listed in the package.json exports.  
/// In this case, it cannot be statically resolved when imported from ESM unless you include the extension of the target module.  
/// So, this function suggests adding the extension of the target module to the imports or exports.
///
/// reference: https://github.com/facebook/react/tree/f9e41e3a519f12cfdc3207e1df44e0d2d9602df9/packages/react-dom
///
/// ```javascript
/// // ❌
/// import ReactDomServer from 'npm:react-dom@16.8.5/server';
/// // ✅
/// import ReactDomServer from 'npm:react-dom@16.8.5/server.js';
/// ```
///
/// ## Known limitation
/// It cannot handle the case where the target module is an import call(dynamic import)
/// which argument is not a string literal due to needing of the runtime evaluation.
///
/// ```javascript
/// // ❌
/// const specifier = 'react-dom/server';
/// import(specifier);
/// // ✅
/// import('react-dom/server');
/// ```
fn get_message_for_non_explicit_cjs_import(
  message: &str,
  module_loader: &Rc<dyn ModuleLoader>,
) -> Result<String, AnyError> {
  // setup the utilities
  let args: Vec<_> = env::args_os().collect();
  let flags = Arc::new(flags_from_vec(args)?);
  let factory = CliFactory::from_flags(flags);
  let cjs_tracker = factory.cjs_tracker()?.clone();
  let fs = factory.fs().clone();

  let captures = Regex::new(r"Unable to load (\S+) imported from (\S+)")?
    .captures(message)
    .ok_or_else(|| anyhow!("Could not capture the message"))?;
  let unable_to_load_file_name = captures
    .get(1)
    .map(|m| m.as_str())
    .ok_or_else(|| anyhow!("No matches the specifier"))?;
  let referrer = captures
    .get(2)
    .map(|m| m.as_str())
    .ok_or_else(|| anyhow!("No matches the referrer"))?;

  let referrer_specifier = ModuleSpecifier::parse(referrer)?;
  let Ok(referrer_path) = referrer_specifier.to_file_path() else {
    return Err(anyhow!("The referrer is not a file"));
  };

  let referrer_parsed_source =
    deno_ast::parse_program(deno_ast::ParseParams {
      specifier: referrer_specifier.clone(),
      text: fs.read_text_file_lossy_sync(&referrer_path, None)?.into(),
      media_type: MediaType::from_specifier(&referrer_specifier),
      capture_tokens: false,
      scope_analysis: false,
      maybe_syntax: None,
    })?;

  // check the referrer is ESM
  let referrer_is_cjs = cjs_tracker.is_cjs_with_known_is_script(
    &referrer_specifier,
    referrer_parsed_source.media_type(),
    referrer_parsed_source.compute_is_script(),
  )?;
  if referrer_is_cjs {
    return Err(anyhow!("The referrer is not an ESM"));
  }

  // TODO(Hajime-san): Use `with_added_extension` when it becomes stable.
  //
  // This extended implementation for `Path` defines an ad-hoc method with the same name,
  // since `with_added_extension` is currently only available in the nightly version.
  // This implementation should be replaced when it becomes stable.
  // https://github.com/rust-lang/rust/issues/127292
  trait PathExt {
    fn _with_added_extension(&self, extension: &str) -> PathBuf;
  }

  impl PathExt for Path {
    fn _with_added_extension(&self, extension: &str) -> PathBuf {
      let mut path = self.to_path_buf();

      let new_extension = match self.extension() {
        Some(ext) => {
          format!("{}.{}", ext.to_string_lossy(), extension)
        }
        None => extension.to_string(),
      };

      path.set_extension(new_extension);
      path
    }
  }

  let unable_to_load_path = Path::new(unable_to_load_file_name);
  // resolve the exact file that unable to load
  let extension = ["js", "cjs"]
    .iter()
    .find(|e| unable_to_load_path._with_added_extension(e).is_file())
    .ok_or_else(|| anyhow!("Cound not find the file"))?;
  let resolved_path = unable_to_load_path._with_added_extension(extension);
  let resolved_specifier = specifier_from_file_path(&resolved_path)?;

  let target_parsed_source = deno_ast::parse_program(deno_ast::ParseParams {
    specifier: resolved_specifier.clone(),
    text: fs.read_text_file_lossy_sync(&resolved_path, None)?.into(),
    media_type: MediaType::from_specifier(&resolved_specifier),
    capture_tokens: false,
    scope_analysis: false,
    maybe_syntax: None,
  })?;

  // check the target module that unable to load is CommonJS
  let target_is_cjs = cjs_tracker.is_cjs_with_known_is_script(
    &resolved_specifier,
    target_parsed_source.media_type(),
    target_parsed_source.compute_is_script(),
  )?;
  if !target_is_cjs {
    return Err(anyhow!("The target is not a CommonJS"));
  }

  // check the raw specifier that unable to load exactly
  let maybe_exact_specifier =
    |raw_specifier: &str, kind: ResolutionKind| -> Option<String> {
      let resolved_specifier =
        module_loader.resolve(raw_specifier, referrer, kind);
      if let (Ok(resolved_specifier), Ok(unable_to_load_specifier)) = (
        resolved_specifier,
        specifier_from_file_path(unable_to_load_path),
      ) {
        if resolved_specifier == unable_to_load_specifier {
          return Some(raw_specifier.to_string());
        }
      }

      None
    };

  // search the exact raw specifier from the referrer source
  let raw_specifier = referrer_parsed_source
    .analyze_dependencies()
    .iter()
    .find_map(|dep| match dep {
      deno_ast::dep::DependencyDescriptor::Static(d) => {
        maybe_exact_specifier(&d.specifier, ResolutionKind::Import)
      }
      deno_ast::dep::DependencyDescriptor::Dynamic(d) => match &d.argument {
        deno_ast::dep::DynamicArgument::String(s) => {
          maybe_exact_specifier(s, ResolutionKind::DynamicImport)
        }
        deno_ast::dep::DynamicArgument::Template(t) => {
          t.iter().find_map(|dynamic| match dynamic {
            deno_ast::dep::DynamicTemplatePart::String(s) => {
              maybe_exact_specifier(s, ResolutionKind::DynamicImport)
            }
            _ => None,
          })
        }
        _ => None,
      },
    })
    .ok_or_else(|| anyhow!("Could not find the raw specifier"))?;

  // add the extension to the raw specifier
  let suggest_specifier = Path::new(raw_specifier.as_str())
    ._with_added_extension(extension)
    .to_string_lossy()
    .into_owned();
  let hint =
    cformat!("Did you mean to import <u>\"{}\"</>?", suggest_specifier);
  let suggestions = vec![
    FixSuggestion::info_multiline(&[
      "The module that you are trying to import seems to be a CommonJS.",
      cstr!(
        "However it is not listed in the <i>exports</> of <u>package.json</>."
      ),
      "So it cannot be statically resolved when imported from ESM.",
      "Consider to include an extension of the module.",
    ]),
    FixSuggestion::hint(&hint),
  ];

  let mut message = String::new();
  FixSuggestion::append_suggestion(&mut message, suggestions);

  Ok(message)
}

/// Keep in mind the function `get_message_for_terminal_errors` in `runtime/fmt_errors.rs`
/// behaves almost identically to this function.
fn get_message_for_terminal_errors(
  message: &str,
  module_loader: &Rc<dyn ModuleLoader>,
) -> String {
  if message.contains("Unable to load") && message.contains("imported from") {
    let result =
      get_message_for_non_explicit_cjs_import(message, module_loader);
    match result {
      Ok(suggestion) => suggestion,
      Err(_) => message.to_string(),
    }
  } else {
    message.to_string()
  }
}

/// Keep in mind the function `format_js_error` in `runtime/fmt_errors.rs`
/// behaves almost identically to this function.
fn format_js_error(
  js_error: &JsError,
  module_loader: &Rc<dyn ModuleLoader>,
) -> String {
  let circular =
    find_recursive_cause(js_error).map(|reference| IndexedErrorReference {
      reference,
      index: 1,
    });
  let mut message = format_js_error_inner(js_error, circular, true, vec![]);
  message.push_str(&get_message_for_terminal_errors(&message, module_loader));

  message
}

fn format_error(
  error: &AnyError,
  module_loader: &Rc<dyn ModuleLoader>,
) -> String {
  let mut message = format!("{error:?}");
  message.push_str(&get_message_for_terminal_errors(&message, module_loader));

  message
}

/// This function should only used to map to the place where the error could caught.
pub fn map_err_suggestions(
  error: AnyError,
  module_loader: &Rc<dyn ModuleLoader>,
) -> AnyError {
  if let Some(js_error) = error.downcast_ref::<JsError>() {
    let message = format_js_error(js_error, module_loader);

    anyhow!(message)
  } else {
    let message = format_error(&error, module_loader);

    anyhow!(message)
  }
}
