/// Optimizer for #[op]
use crate::Op;
use phf::phf_map;
use proc_macro2::TokenStream;
use quote::{quote, ToTokens};
use std::fmt::Debug;
use std::fmt::Formatter;
use syn::{
  punctuated::Punctuated, token::Colon2, AngleBracketedGenericArguments, FnArg,
  GenericArgument, ItemFn, Path, PathArguments, PathSegment, ReturnType,
  Signature, Type, TypePath, TypeReference, TypeSlice,
};

#[derive(Debug)]
pub(crate) enum BailoutReason {
  FastAsync,
  MustBeSingleSegment,
  Other(&'static str),
}

impl ToTokens for BailoutReason {
  fn to_tokens(&self, tokens: &mut TokenStream) {
    match self {
      BailoutReason::FastAsync => {
        tokens.extend(quote! { "fast async calls are not supported" });
      }
      BailoutReason::MustBeSingleSegment => {
        unreachable!("error not recovered");
      }
      BailoutReason::Other(reason) => {
        tokens.extend(quote! { #reason });
      }
    }
  }
}

#[derive(Debug, PartialEq)]
enum TransformKind {
  // serde_v8::Value
  V8Value,
  SliceU32(bool),
  SliceU8(bool),
}

impl Transform {
  fn serde_v8_value(index: usize) -> Self {
    Transform {
      kind: TransformKind::V8Value,
      index,
    }
  }

  fn slice_u32(index: usize, is_mut: bool) -> Self {
    Transform {
      kind: TransformKind::SliceU32(is_mut),
      index,
    }
  }

  fn slice_u8(index: usize, is_mut: bool) -> Self {
    Transform {
      kind: TransformKind::SliceU8(is_mut),
      index,
    }
  }
}

#[derive(Debug, PartialEq)]
pub(crate) struct Transform {
  kind: TransformKind,
  index: usize,
}

static FAST_SCALAR: phf::Map<&'static str, FastValue> = phf_map! {
  "u32" => FastValue::U32,
  "i32" => FastValue::I32,
  "u64" => FastValue::U64,
  "i64" => FastValue::I64,
  "f32" => FastValue::F32,
  "f64" => FastValue::F64,
  "bool" => FastValue::Bool,
  "ResourceId" => FastValue::U32,
};

#[derive(Debug, PartialEq, Clone)]
pub(crate) enum FastValue {
  Void,
  U32,
  I32,
  U64,
  I64,
  F32,
  F64,
  Bool,
}

impl Default for FastValue {
  fn default() -> Self {
    Self::Void
  }
}

#[derive(Default, PartialEq)]
pub(crate) struct Optimizer {
  pub(crate) returns_result: bool,

  pub(crate) has_ref_opstate: bool,

  pub(crate) has_rc_opstate: bool,

  pub(crate) has_fast_callback_option: bool,

  pub(crate) fast_result: Option<FastValue>,
  pub(crate) fast_parameters: Vec<FastValue>,

  pub(crate) transforms: Vec<Transform>,
}

impl Debug for Optimizer {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    writeln!(f, "=== Optimizer Dump ===")?;
    writeln!(f, "returns_result: {}", self.returns_result)?;
    writeln!(f, "has_ref_opstate: {}", self.has_ref_opstate)?;
    writeln!(f, "has_rc_opstate: {}", self.has_rc_opstate)?;
    writeln!(
      f,
      "has_fast_callback_option: {}",
      self.has_fast_callback_option
    )?;
    writeln!(f, "fast_result: {:?}", self.fast_result)?;
    writeln!(f, "fast_parameters: {:?}", self.fast_parameters)?;
    writeln!(f, "transforms: {:?}", self.transforms)?;
    Ok(())
  }
}

impl Optimizer {
  pub(crate) fn new() -> Self {
    Default::default()
  }

  pub(crate) fn analyze(&mut self, op: &mut Op) -> Result<(), BailoutReason> {
    if op.is_async && op.attrs.must_be_fast {
      return Err(BailoutReason::FastAsync);
    }

    let sig = &op.item.sig;

    // Analyze return type
    match &sig {
      Signature {
        output: ReturnType::Default,
        ..
      } => self.fast_result = Some(FastValue::default()),
      Signature {
        output: ReturnType::Type(_, ty),
        ..
      } => self.analyze_return_type(ty)?,
    };

    // Analyze parameters
    for (index, param) in sig.inputs.iter().enumerate() {
      self.analyze_param_type(index, param)?;
    }

    Ok(())
  }

  fn analyze_return_type(&mut self, ty: &Type) -> Result<(), BailoutReason> {
    match ty {
      Type::Path(TypePath {
        path: Path { segments, .. },
        ..
      }) => {
        let segment = single_segment(segments)?;

        match segment {
          // Result<T, E>
          PathSegment {
            ident, arguments, ..
          } if ident == "Result" => {
            self.returns_result = true;

            if let PathArguments::AngleBracketed(
              AngleBracketedGenericArguments { args, .. },
            ) = arguments
            {
              match args.last() {
                Some(GenericArgument::Type(Type::Path(TypePath {
                  path: Path { segments, .. },
                  ..
                }))) => {
                  let segment = single_segment(segments)?;
                  match segment {
                    // Is `T` a scalar FastValue?
                    PathSegment { ident, .. } => {
                      if let Some(val) =
                        FAST_SCALAR.get(ident.to_string().as_str())
                      {
                        self.fast_result = Some(val.clone());
                      }
                    }
                  }
                }
                _ => {}
              }
            }
          }
          // Is `T` a scalar FastValue?
          PathSegment { ident, .. } => {
            if let Some(val) = FAST_SCALAR.get(ident.to_string().as_str()) {
              self.fast_result = Some(val.clone());
            }
          }
          // T
          _ => {}
        };
      }
      _ => {}
    };

    Ok(())
  }

  fn analyze_param_type(
    &mut self,
    index: usize,
    arg: &FnArg,
  ) -> Result<(), BailoutReason> {
    match arg {
      FnArg::Typed(typed) => match &*typed.ty {
        Type::Path(TypePath {
          path: Path { segments, .. },
          ..
        }) if segments.len() == 2 => {
          match double_segment(segments)? {
            // -> serde_v8::Value
            [PathSegment { ident: first, .. }, PathSegment { ident: last, .. }]
              if first == "serde_v8" && last == "Value" =>
            {
              self.transforms.push(Transform::serde_v8_value(index));
            }
            _ => unreachable!(),
          }
        }
        Type::Path(TypePath {
          path: Path { segments, .. },
          ..
        }) => {
          let segment = single_segment(segments)?;

          match segment {
            // -> Option<T>
            PathSegment {
              ident, arguments, ..
            } if ident == "Option" => {
              if let PathArguments::AngleBracketed(
                AngleBracketedGenericArguments { args, .. },
              ) = arguments
              {
                match args.last() {
                  // -> Option<&mut T>
                  Some(GenericArgument::Type(Type::Reference(
                    TypeReference { elem, .. },
                  ))) => {
                    match &**elem {
                      Type::Path(TypePath {
                        path: Path { segments, .. },
                        ..
                      }) => {
                        let segment = single_segment(segments)?;
                        match segment {
                          // Is `T` a FastApiCallbackOption?
                          PathSegment { ident, .. }
                            if ident == "FastApiCallbackOption" =>
                          {
                            self.has_fast_callback_option = true;
                          }
                          _ => {}
                        }
                      }
                      _ => {}
                    }
                  }
                  _ => {}
                }
              }
            }
            // -> Rc<T>
            PathSegment {
              ident, arguments, ..
            } if ident == "Rc" => {
              if let PathArguments::AngleBracketed(
                AngleBracketedGenericArguments { args, .. },
              ) = arguments
              {
                match args.last() {
                  Some(GenericArgument::Type(Type::Path(TypePath {
                    path: Path { segments, .. },
                    ..
                  }))) => {
                    let segment = single_segment(segments)?;
                    match segment {
                      // -> Rc<RefCell<T>>
                      PathSegment { ident, .. } if ident == "RefCell" => {
                        if let PathArguments::AngleBracketed(
                          AngleBracketedGenericArguments { args, .. },
                        ) = arguments
                        {
                          match args.last() {
                            // -> Rc<RefCell<OpState>>
                            Some(GenericArgument::Type(Type::Path(
                              TypePath {
                                path: Path { segments, .. },
                                ..
                              },
                            ))) => {
                              let segment = single_segment(segments)?;
                              match segment {
                                PathSegment { ident, .. }
                                  if ident == "OpState" =>
                                {
                                  self.has_rc_opstate = true;
                                }
                                _ => {}
                              }
                            }
                            _ => {}
                          }
                        }
                      }
                      _ => {}
                    }
                  }
                  _ => {}
                }
              }
            }
            // Is `T` a fast scalar?
            PathSegment { ident, .. } => {
              if let Some(val) = FAST_SCALAR.get(ident.to_string().as_str()) {
                self.fast_parameters.push(val.clone());
              }
            }
          };
        }
        // &mut T
        Type::Reference(TypeReference {
          elem, mutability, ..
        }) => match &**elem {
          Type::Path(TypePath {
            path: Path { segments, .. },
            ..
          }) => {
            let segment = single_segment(segments)?;
            match segment {
              // Is `T` a OpState?
              PathSegment { ident, .. } if ident == "OpState" => {
                self.has_ref_opstate = true;
              }
              _ => {}
            }
          }
          // &mut [T]
          Type::Slice(TypeSlice { elem, .. }) => match &**elem {
            Type::Path(TypePath {
              path: Path { segments, .. },
              ..
            }) => {
              let segment = single_segment(&segments)?;
              let is_mut_ref = mutability.is_some();
              match segment {
                // Is `T` a u8?
                PathSegment { ident, .. } if ident == "u8" => {
                  self.transforms.push(Transform::slice_u8(index, is_mut_ref));
                }
                // Is `T` a u32?
                PathSegment { ident, .. } if ident == "u32" => {
                  self
                    .transforms
                    .push(Transform::slice_u32(index, is_mut_ref));
                }
                _ => {}
              }
            }
            _ => {}
          },
          _ => {}
        },
        _ => {}
      },
      _ => {}
    };
    Ok(())
  }
}

fn single_segment(
  segments: &Punctuated<PathSegment, Colon2>,
) -> Result<&PathSegment, BailoutReason> {
  if segments.len() != 1 {
    return Err(BailoutReason::MustBeSingleSegment);
  }

  match segments.last() {
    Some(segment) => Ok(segment),
    None => Err(BailoutReason::MustBeSingleSegment),
  }
}

fn double_segment(
  segments: &Punctuated<PathSegment, Colon2>,
) -> Result<[&PathSegment; 2], BailoutReason> {
  match (segments.first(), segments.last()) {
    (Some(first), Some(last)) => Ok([first, last]),
    // Caller ensures that there are only two segments.
    _ => unreachable!(),
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::attrs::Attributes;
  use crate::Op;
  use std::path::PathBuf;
  use syn::parse_quote;

  #[test]
  fn test_single_segment() {
    let segments = parse_quote!(foo);
    assert!(single_segment(&segments).is_ok());

    let segments = parse_quote!(foo::bar);
    assert!(single_segment(&segments).is_err());
  }

  #[test]
  fn test_double_segment() {
    let segments = parse_quote!(foo::bar);
    assert!(double_segment(&segments).is_ok());
    assert_eq!(double_segment(&segments).unwrap()[0].ident, "foo");
    assert_eq!(double_segment(&segments).unwrap()[1].ident, "bar");
  }

  fn test_optimizer(item: ItemFn, attributes: Attributes, expected: Optimizer) {
    let mut op = Op::new(item, attributes);
    let mut optimizer = Optimizer::new();
    optimizer.analyze(&mut op).expect("Optimizer failed");
    assert_eq!(optimizer, expected);
  }

  #[testing::fixture("optimizer_tests/**/*.rs")]
  fn test_analyzer(input: PathBuf) {
    let update_expected = std::env::var("UPDATE_EXPECTED").is_ok();

    let source =
      std::fs::read_to_string(&input).expect("Failed to read test file");
    let expected = std::fs::read_to_string(input.with_extension("expected"))
      .expect("Failed to read expected file");

    let item = syn::parse_str(&source).expect("Failed to parse test file");
    let mut op = Op::new(item, Default::default());
    let mut optimizer = Optimizer::new();
    optimizer.analyze(&mut op).expect("Optimizer failed");

    if update_expected {
      std::fs::write(
        input.with_extension("expected"),
        format!("{:#?}", optimizer),
      )
      .expect("Failed to write expected file");
    } else {
      assert_eq!(format!("{:#?}", optimizer), expected);
    }
  }
}
