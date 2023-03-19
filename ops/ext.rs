use pmutil::q;
use pmutil::Quote;
use quote::format_ident;
use syn::parse::Parse;
use syn::parse::ParseStream;
use syn::parse::Result;
use syn::parse_quote;
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::Expr;
use syn::ExprClosure;
use syn::FieldValue;
use syn::GenericParam;
use syn::Ident;
use syn::Lit;
use syn::LitStr;
use syn::Member;
use syn::Token;

#[derive(Debug)]
pub struct ExtensionDef {
  name: Ident,
  ops: Punctuated<Expr, Token![,]>,
  esm: (Option<LitStr>, Punctuated<Lit, Token![,]>),
  js: (Option<LitStr>, Punctuated<Lit, Token![,]>),
  // Module dependencies
  deps: Punctuated<Ident, Token![,]>,
  parameters: Punctuated<GenericParam, Token![,]>,
  // ops_fn
  esm_entry_point: Option<LitStr>,
  esm_setup_script: Option<LitStr>,
  options: Punctuated<FieldValue, Token![,]>,
  middleware: Option<Expr>,
  state: Option<ExprClosure>,
  event_loop_middleware: Option<Ident>,
  customizer: Option<Expr>,
}

macro_rules! parse_optional_list {
    ($input:ident, $ident:ident, $item_ty:ty) => {
      parse_optional_list!($input, $ident, $item_ty, bracketed)
    };
    ($input:ident, $ident:ident, $item_ty:ty, $encl:ident) => {{
      let lookahead = $input.lookahead1();
      let e = if lookahead.peek(Ident) {
        let i = $input.fork().parse::<Ident>()?;
        if i.to_string() != stringify!($ident) {
          Punctuated::new()
        } else {
            let _ = $input.parse::<Ident>()?;
            $input.parse::<Token![=]>()?;
            let content;
            let _ = syn::$encl!(content in $input);
            content.parse_terminated(<$item_ty> ::parse)?
        }
      } else {
        Punctuated::new()
      };
      let _ = $input.parse::<Token![,]>();
      e
    }};
  }

macro_rules! parse_optional {
    ($input:ident, $ident:ident) => {{
      let lookahead = $input.lookahead1();
      let e = if lookahead.peek(Ident) {
        let i = $input.fork().parse::<Ident>()?;
        if i.to_string() != stringify!($ident) {
          None
        } else {
            let _ = $input.parse::<Ident>()?;
            $input.parse::<Token![=]>()?;
            Some($input.parse()?)
        }
      } else {
        None
      };
        let _ = $input.parse::<Token![,]>();
        e
    }};
}

fn parse_js_files(
  input: &mut ParseStream,
  name: &str,
) -> Result<(Option<LitStr>, Punctuated<Lit, Token![,]>)> {
  let lookahead = input.lookahead1();
  let e = if lookahead.peek(Ident) {
    let i = input.fork().parse::<Ident>()?;
    if i.to_string() != name {
      (None, Punctuated::new())
    } else {
      let _ = input.parse::<Ident>()?;
      input.parse::<Token![=]>()?;
      let content;
      let _ = syn::bracketed!(content in input);

      let mut dir = None;
      if content.peek(Ident) {
        let id = content.parse::<Ident>()?;
        if id.to_string() != "dir" {
          return Err(syn::Error::new(id.span(), "expected `dir`"));
        }

        // The next string literal is the directory
        dir = Some(content.parse::<LitStr>()?);
      }

      let files = content.parse_terminated(Lit::parse)?;
      (dir, files)
    }
  } else {
    (None, Punctuated::new())
  };

  let _ = input.parse::<Token![,]>();
  Ok(e)
}

// extension!(
///   my_extension,
///   ops = [ op_xyz ],
///   esm = [ "my_script.js" ],
/// );
impl Parse for ExtensionDef {
  fn parse(mut input: ParseStream) -> Result<Self> {
    let name = input.parse()?;
    input.parse::<Token![,]>()?;

    let deps = parse_optional_list!(input, deps, Ident);
    let parameters = parse_optional_list!(input, parameters, GenericParam);
    let ops = parse_optional_list!(input, ops, Expr);
    let esm_entry_point = parse_optional!(input, esm_entry_point);
    // The esm & js options have an optional special syntax for specifying a
    // directory of files to include. Rest of the stuff is treated as
    // string literals.
    //
    // esm = [ dir "foo" "file.js" "file2.js"]
    let esm = parse_js_files(&mut input, "esm")?;
    let esm_setup_script = parse_optional!(input, esm_setup_script);
    let js = parse_js_files(&mut input, "js")?;
    let options = parse_optional_list!(input, options, FieldValue, braced);

    let event_loop_middleware = parse_optional!(input, event_loop_middleware);
    let middleware = parse_optional!(input, middleware);

    let state = parse_optional!(input, state);
    let customizer = parse_optional!(input, customizer);

    Ok(Self {
      name,
      ops,
      esm,
      deps,
      parameters,
      esm_entry_point,
      esm_setup_script,
      js,
      event_loop_middleware,
      middleware,
      state,
      customizer,
      options,
    })
  }
}

fn literals<T: ToString + syn::spanned::Spanned>(
  list: &Punctuated<T, Token![,]>,
) -> Punctuated<LitStr, Token![,]> {
  list
    .iter()
    .map(|i| LitStr::new(&i.to_string(), i.span()))
    .collect()
}

fn generate_with_js(ext: &ExtensionDef, builder: &mut Quote) {
  if !ext.esm.1.is_empty() {
    match ext.esm.0 {
      Some(ref dir) => {
        builder.push_tokens(&q!(
          Vars {
            name: &ext.name,
            directory: dir,
            files: &ext.esm.1
          },
          {
            ext.esm(include_js_files!(name dir directory files));
          }
        ));
      }
      None => {
        builder.push_tokens(&q!(
          Vars {
            name: &ext.name,
            files: &ext.esm.1
          },
          {
            ext.esm(include_js_files!(name files));
          }
        ));
      }
    }
  }

  if let Some(ref script) = ext.esm_setup_script {
    builder.push_tokens(&q!(Vars { script: script }, {
      ext.esm_setup_script(vec![ExtensionFileSource {
        specifier: "ext:setup",
        code: ExtensionFileSourceCode::IncludedInBinary(script),
      }]);
    }));
  }

  if let Some(ref entry_point) = ext.esm_entry_point {
    builder.push_tokens(&q!(
      Vars {
        entry_point: entry_point
      },
      {
        ext.esm_entry_point(entry_point);
      }
    ));
  }

  if !ext.js.1.is_empty() {
    match ext.js.0 {
      Some(ref dir) => {
        builder.push_tokens(&q!(
          Vars {
            name: &ext.name,
            directory: dir,
            files: &ext.js.1
          },
          {
            ext.js(include_js_files!(name dir directory files));
          }
        ));
      }
      None => {
        builder.push_tokens(&q!(
          Vars {
            name: &ext.name,
            files: &ext.js.1
          },
          {
            ext.js(include_js_files!(name files));
          }
        ));
      }
    }
  }
}

fn generate_with_ops(ext: &ExtensionDef, builder: &mut Quote) {
  if !ext.ops.is_empty() {
    let exprs = ext
      .ops
      .iter()
      .map(|o| {
        let mut path = o.clone();
        match path {
          Expr::Path(ref mut path) => {
            let angle_bracketed = match path.path.segments.last_mut() {
              Some(syn::PathSegment { arguments, .. }) => {
                let cloned = arguments.clone();
                *arguments = syn::PathArguments::None;
                Some(cloned)
              }
              _ => None,
            };
            path.path.segments.push(syn::PathSegment {
              ident: Ident::new("decl", path.path.span()),
              arguments: angle_bracketed.unwrap_or(syn::PathArguments::None),
            });
          }
          _ => unreachable!(),
        }
        Expr::Call(syn::ExprCall {
          args: Punctuated::new(),
          attrs: vec![],
          paren_token: Default::default(),
          func: Box::new(path),
        })
      })
      .collect::<Punctuated<Expr, Token![,]>>();
    builder.push_tokens(&q!(Vars { ops_expr: exprs }, {
      ext.ops(vec![ops_expr]);
    }));
  }
}

fn generate_with_state(ext: &ExtensionDef, builder: &mut Quote) {
  if let Some(ref state) = ext.state {
    if !ext.options.is_empty() {
      let member_names = ext
        .options
        .iter()
        .map(|f| f.member.clone())
        .collect::<Punctuated<Member, Token![,]>>();
      builder.push_tokens(&q!(Vars { state_cl: state, fieldvalues: &ext.options, member_names: member_names }, {
        struct Config {
          fieldvalues
        }
        let config = Config { member_names };
        let state_fn: fn(&mut OpState, Config) = state_cl;
        ext.state(move |s: &mut OpState| {
          state_fn(s, config);
        });
      }));
    } else {
      builder.push_tokens(&q!(Vars { state: state }, {
        ext.state(state);
      }));
    }
  }

  if let Some(ref middleware) = ext.event_loop_middleware {
    builder.push_tokens(&q!(
      Vars {
        middleware: middleware
      },
      {
        ext.event_loop_middleware(middleware);
      }
    ));
  }

  if let Some(ref middleware) = ext.middleware {
    builder.push_tokens(&q!(
      Vars {
        middleware: middleware
      },
      {
        ext.middleware(middleware);
      }
    ));
  }
}

fn generate_builder(ext: &ExtensionDef, generate_js: bool) -> Quote {
  let deps = literals(&ext.deps);
  let mut builder = q!(
    Vars {
      name: &ext.name,
      deps: deps
    },
    {
      let mut ext =
        crate::Extension::builder_with_deps(stringify!(name), &[deps]);
    }
  );

  if generate_js {
    generate_with_js(&ext, &mut builder)
  };
  generate_with_ops(&ext, &mut builder);
  generate_with_state(&ext, &mut builder);

  builder.push_tokens(&q!({ ext.take() }));

  builder
}

pub(crate) fn generate(ext: ExtensionDef) -> proc_macro2::TokenStream {
  let builder = generate_builder(&ext, true);
  let builder2 = generate_builder(&ext, false);

  let params = if !ext.parameters.is_empty() {
    q!(Vars { params: &ext.parameters }, { < params > })
  } else {
    q!({})
  };
  let mut tokens = q!(Vars { name: &ext.name, params: params, options: &ext.options, ops_and_esm: builder, ops: builder2 }, {
    #[allow(non_camel_case_types)]
    pub struct name;

    impl name {
      pub fn init_ops_and_esm params ( options ) -> crate::Extension {
        ops_and_esm
      }

      pub fn init_ops params ( options ) -> crate::Extension {
        ops
      }
    }
  });

  tokens.into()
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::path::PathBuf;

  fn extract_tokenstream(input: &PathBuf) -> proc_macro2::TokenStream {
    let source =
      std::fs::read_to_string(&input).expect("Failed to read test file");

    // strip `extension! {` and `}` from the source.
    // extension! {
    //   ...
    // }

    let mut source = source
      .lines()
      .filter(|line| !line.trim().starts_with("extension!"))
      .collect::<Vec<_>>();
    if source.last().map(|s| s.trim() == "}") == Some(true) {
      source.pop();
    }
    let source = source.join("\n");

    syn::parse_str(&source).expect("Failed to parse test file")
  }

  #[testing_macros::fixture("extension_tests/**/*.rs")]
  fn test_extension(input: PathBuf) {
    let update_expected = std::env::var("UPDATE_EXPECTED").is_ok();
    let item = extract_tokenstream(&input);
    let expected = std::fs::read_to_string(input.with_extension("out"))
      .expect("Failed to read expected output file");

    let actual = generate(syn::parse2::<ExtensionDef>(item).unwrap());
    // Validate syntax tree.
    let tree = syn::parse2(actual).unwrap();
    let actual = prettyplease::unparse(&tree);
    if update_expected {
      std::fs::write(input.with_extension("out"), actual)
        .expect("Failed to write expected file");
    } else {
      assert_eq!(actual, expected);
    }
  }
}
