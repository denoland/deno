/// Optimizer for #[op]
use crate::Op;
use syn::{
  punctuated::Punctuated, token::Colon2, AngleBracketedGenericArguments, FnArg,
  GenericArgument, ItemFn, Path, PathArguments, PathSegment, ReturnType,
  Signature, Type, TypePath, TypeReference, TypeSlice,
};

#[derive(Debug)]
enum BailoutReason {
  FastAsync,
  MustBeSingleSegment,
  Other(&'static str),
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
struct Transform {
  kind: TransformKind,
  index: usize,
}

#[derive(Debug, PartialEq)]
enum FastValue {
  Void,
}

impl Default for FastValue {
  fn default() -> Self {
    Self::Void
  }
}

#[derive(Default, Debug, PartialEq)]
struct Optimizer {
  returns_result: bool,

  has_ref_opstate: bool,

  has_rc_opstate: bool,

  has_fast_callback_option: bool,

  fast_result: Option<FastValue>,
  fast_parameters: Vec<FastValue>,

  transforms: Vec<Transform>,
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

            // Is `T` a FastValue?
            if let PathArguments::AngleBracketed(ref bracketed) = arguments {}
          }
          // T
          _ => {
            // Is `T` a scalar FastValue?
          }
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
                          PathSegment {
                            ident, arguments, ..
                          } if ident == "FastApiCallbackOption" => {
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
                      PathSegment {
                        ident, arguments, ..
                      } if ident == "RefCell" => {
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
                                PathSegment {
                                  ident, arguments, ..
                                } if ident == "OpState" => {
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
            PathSegment {
              ident, arguments, ..
            } if matches!(
              ident.to_string().as_str(),
              "u32" | "u64" | "i32" | "i64" | "f32" | "f64" | "bool"
            ) =>
            {
              self.fast_parameters.push(FastValue::default());
            }
            _ => {}
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
              PathSegment {
                ident, arguments, ..
              } if ident == "OpState" => {
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
                PathSegment {
                  ident, arguments, ..
                } if ident == "u8" => {
                  self.transforms.push(Transform::slice_u8(index, is_mut_ref));
                }
                // Is `T` a u32?
                PathSegment {
                  ident, arguments, ..
                } if ident == "u32" => {
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

  #[test]
  fn test_analyzer1() {
    test_optimizer(
      parse_quote!(
        fn foo(state: &mut OpState, a: u32, b: u32) -> u32 {
          a + b
        }
      ),
      Attributes {
        must_be_fast: true,
        ..Default::default()
      },
      Optimizer {
        has_ref_opstate: true,
        has_rc_opstate: false,
        fast_result: None,
        has_fast_callback_option: false,
        returns_result: false,
        fast_parameters: vec![FastValue::default(), FastValue::default()],
        transforms: vec![],
      },
    );
  }
}
