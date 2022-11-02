/// Optimizer for #[op]
use crate::Op;
use syn::{
  punctuated::Punctuated, token::Colon2, AngleBracketedGenericArguments, FnArg,
  GenericArgument, ItemFn, Path, PathArguments, PathSegment, ReturnType,
  Signature, Type, TypePath, TypeReference,
};

enum BailoutReason {
  FastAsync,
  MustBeSingleSegment,
  Other(&'static str),
}

enum TransformKind {
  // serde_v8::Value
  V8Value,
}

impl Transform {
  fn serde_v8_value(index: usize) -> Self {
    Transform {
      kind: TransformKind::V8Value,
      index,
    }
  }
}

struct Transform {
  kind: TransformKind,
  index: usize,
}

enum FastValue {
  Void,
}

impl Default for FastValue {
  fn default() -> Self {
    Self::Void
  }
}

#[derive(Default)]
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
            _ => {}
          };
        }
        // &mut T
        Type::Reference(TypeReference { elem, .. }) => match &**elem {
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
          _ => {}
        },
        _ => {}
      },
      _ => {}
    };

    // Is a scalar FastValue?

    // Is a sequence FastValue?

    // Is &mut [u8] or &mut [u32]?

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
