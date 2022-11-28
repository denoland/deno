/// Optimizer for #[op]
use crate::Op;
use pmutil::{q, Quote};
use proc_macro2::TokenStream;
use std::collections::BTreeMap;
use std::fmt::Debug;
use std::fmt::Formatter;
use syn::{
  parse_quote, punctuated::Punctuated, token::Colon2,
  AngleBracketedGenericArguments, FnArg, GenericArgument, PatType, Path,
  PathArguments, PathSegment, ReturnType, Signature, Type, TypePath, TypePtr,
  TypeReference, TypeSlice, TypeTuple,
};

#[derive(Debug)]
pub(crate) enum BailoutReason {
  // Recoverable errors
  MustBeSingleSegment,
  FastUnsupportedParamType,
}

#[derive(Debug, PartialEq)]
enum TransformKind {
  // serde_v8::Value
  V8Value,
  SliceU32(bool),
  SliceU8(bool),
  PtrU8,
  WasmMemory,
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

  fn wasm_memory(index: usize) -> Self {
    Transform {
      kind: TransformKind::WasmMemory,
      index,
    }
  }

  fn u8_ptr(index: usize) -> Self {
    Transform {
      kind: TransformKind::PtrU8,
      index,
    }
  }
}

#[derive(Debug, PartialEq)]
pub(crate) struct Transform {
  kind: TransformKind,
  index: usize,
}

impl Transform {
  pub(crate) fn apply_for_fast_call(
    &self,
    core: &TokenStream,
    input: &mut FnArg,
  ) -> Quote {
    let (ty, ident) = match input {
      FnArg::Typed(PatType {
        ref mut ty,
        ref pat,
        ..
      }) => {
        let ident = match &**pat {
          syn::Pat::Ident(ident) => &ident.ident,
          _ => unreachable!("error not recovered"),
        };
        (ty, ident)
      }
      _ => unreachable!("error not recovered"),
    };

    match &self.kind {
      // serde_v8::Value
      TransformKind::V8Value => {
        *ty = parse_quote! { #core::v8::Local<v8::Value> };

        q!(Vars { var: &ident }, {
          let var = serde_v8::Value { v8_value: var };
        })
      }
      // &[u32]
      TransformKind::SliceU32(_) => {
        *ty =
          parse_quote! { *const #core::v8::fast_api::FastApiTypedArray<u32> };

        q!(Vars { var: &ident }, {
          let var = match unsafe { &*var }.get_storage_if_aligned() {
            Some(v) => v,
            None => {
              unsafe { &mut *fast_api_callback_options }.fallback = true;
              return Default::default();
            }
          };
        })
      }
      // &[u8]
      TransformKind::SliceU8(_) => {
        *ty =
          parse_quote! { *const #core::v8::fast_api::FastApiTypedArray<u8> };

        q!(Vars { var: &ident }, {
          let var = match unsafe { &*var }.get_storage_if_aligned() {
            Some(v) => v,
            None => {
              unsafe { &mut *fast_api_callback_options }.fallback = true;
              return Default::default();
            }
          };
        })
      }
      TransformKind::WasmMemory => {
        // Note: `ty` is correctly set to __opts by the fast call tier.
        q!(Vars { var: &ident, core }, {
          let var = unsafe {
            &*(__opts.wasm_memory
              as *const core::v8::fast_api::FastApiTypedArray<u8>)
          }
          .get_storage_if_aligned();
        })
      }
      // *const u8
      TransformKind::PtrU8 => {
        *ty =
          parse_quote! { *const #core::v8::fast_api::FastApiTypedArray<u8> };

        q!(Vars { var: &ident }, {
          let var = match unsafe { &*var }.get_storage_if_aligned() {
            Some(v) => v.as_ptr(),
            None => {
              unsafe { &mut *fast_api_callback_options }.fallback = true;
              return Default::default();
            }
          };
        })
      }
    }
  }
}

fn get_fast_scalar(s: &str) -> Option<FastValue> {
  match s {
    "u32" => Some(FastValue::U32),
    "i32" => Some(FastValue::I32),
    "u64" => Some(FastValue::U64),
    "i64" => Some(FastValue::I64),
    "f32" => Some(FastValue::F32),
    "f64" => Some(FastValue::F64),
    "bool" => Some(FastValue::Bool),
    "ResourceId" => Some(FastValue::U32),
    _ => None,
  }
}

fn can_return_fast(v: &FastValue) -> bool {
  !matches!(
    v,
    FastValue::U64
      | FastValue::I64
      | FastValue::Uint8Array
      | FastValue::Uint32Array
  )
}

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
  V8Value,
  Uint8Array,
  Uint32Array,
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

  // Do we need an explict FastApiCallbackOptions argument?
  pub(crate) has_fast_callback_option: bool,
  // Do we depend on FastApiCallbackOptions?
  pub(crate) needs_fast_callback_option: bool,

  pub(crate) has_wasm_memory: bool,

  pub(crate) fast_result: Option<FastValue>,
  pub(crate) fast_parameters: Vec<FastValue>,

  pub(crate) transforms: BTreeMap<usize, Transform>,
  pub(crate) fast_compatible: bool,

  pub(crate) is_async: bool,
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
    writeln!(
      f,
      "needs_fast_callback_option: {}",
      self.needs_fast_callback_option
    )?;
    writeln!(f, "fast_result: {:?}", self.fast_result)?;
    writeln!(f, "fast_parameters: {:?}", self.fast_parameters)?;
    writeln!(f, "transforms: {:?}", self.transforms)?;
    writeln!(f, "is_async: {}", self.is_async)?;
    writeln!(f, "fast_compatible: {}", self.fast_compatible)?;
    Ok(())
  }
}

impl Optimizer {
  pub(crate) fn new() -> Self {
    Default::default()
  }

  pub(crate) const fn has_opstate_in_parameters(&self) -> bool {
    self.has_ref_opstate || self.has_rc_opstate
  }

  pub(crate) const fn needs_opstate(&self) -> bool {
    self.has_ref_opstate || self.has_rc_opstate || self.returns_result
  }

  pub(crate) fn analyze(&mut self, op: &mut Op) -> Result<(), BailoutReason> {
    // Fast async ops are opt-in as they have a lazy polling behavior.
    if op.is_async && !op.attrs.must_be_fast {
      self.fast_compatible = false;
      return Ok(());
    }

    if op.attrs.is_v8 {
      self.fast_compatible = false;
      return Ok(());
    }

    self.is_async = op.is_async;
    self.fast_compatible = true;
    // Just assume for now. We will validate later.
    self.has_wasm_memory = op.attrs.is_wasm;

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
      } if !self.is_async => self.analyze_return_type(ty)?,

      // No need to error on the return type for async ops, its OK if
      // it's not a fast value.
      Signature {
        output: ReturnType::Type(_, ty),
        ..
      } => {
        let _ = self.analyze_return_type(ty);
        // Recover.
        self.fast_result = None;
        self.fast_compatible = true;
      }
    };

    // The reciever, which we don't actually care about.
    self.fast_parameters.push(FastValue::V8Value);

    if self.is_async {
      // The promise ID.
      self.fast_parameters.push(FastValue::I32);
    }

    // Analyze parameters
    for (index, param) in sig.inputs.iter().enumerate() {
      self.analyze_param_type(index, param)?;
    }

    Ok(())
  }

  fn analyze_return_type(&mut self, ty: &Type) -> Result<(), BailoutReason> {
    match ty {
      Type::Tuple(TypeTuple { elems, .. }) if elems.is_empty() => {
        self.fast_result = Some(FastValue::Void);
      }
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
              match args.first() {
                Some(GenericArgument::Type(Type::Path(TypePath {
                  path: Path { segments, .. },
                  ..
                }))) => {
                  let PathSegment { ident, .. } = single_segment(segments)?;
                  // Is `T` a scalar FastValue?
                  if let Some(val) = get_fast_scalar(ident.to_string().as_str())
                  {
                    if can_return_fast(&val) {
                      self.fast_result = Some(val);
                      return Ok(());
                    }
                  }

                  self.fast_compatible = false;
                  return Err(BailoutReason::FastUnsupportedParamType);
                }
                Some(GenericArgument::Type(Type::Tuple(TypeTuple {
                  elems,
                  ..
                })))
                  if elems.is_empty() =>
                {
                  self.fast_result = Some(FastValue::Void);
                }
                _ => return Err(BailoutReason::FastUnsupportedParamType),
              }
            }
          }
          // Is `T` a scalar FastValue?
          PathSegment { ident, .. } => {
            if let Some(val) = get_fast_scalar(ident.to_string().as_str()) {
              self.fast_result = Some(val);
              return Ok(());
            }

            self.fast_compatible = false;
            return Err(BailoutReason::FastUnsupportedParamType);
          }
        };
      }
      _ => return Err(BailoutReason::FastUnsupportedParamType),
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
              self.fast_parameters.push(FastValue::V8Value);
              assert!(self
                .transforms
                .insert(index, Transform::serde_v8_value(index))
                .is_none());
            }
            _ => return Err(BailoutReason::FastUnsupportedParamType),
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
                // -> Option<&mut T>
                if let Some(GenericArgument::Type(Type::Reference(
                  TypeReference { elem, .. },
                ))) = args.last()
                {
                  if self.has_wasm_memory {
                    // -> Option<&mut [u8]>
                    if let Type::Slice(TypeSlice { elem, .. }) = &**elem {
                      if let Type::Path(TypePath {
                        path: Path { segments, .. },
                        ..
                      }) = &**elem
                      {
                        let segment = single_segment(segments)?;

                        match segment {
                          // Is `T` a u8?
                          PathSegment { ident, .. } if ident == "u8" => {
                            self.needs_fast_callback_option = true;
                            assert!(self
                              .transforms
                              .insert(index, Transform::wasm_memory(index))
                              .is_none());
                          }
                          _ => {
                            return Err(BailoutReason::FastUnsupportedParamType)
                          }
                        }
                      }
                    }
                  } else if let Type::Path(TypePath {
                    path: Path { segments, .. },
                    ..
                  }) = &**elem
                  {
                    let segment = single_segment(segments)?;
                    match segment {
                      // Is `T` a FastApiCallbackOptions?
                      PathSegment { ident, .. }
                        if ident == "FastApiCallbackOptions" =>
                      {
                        self.has_fast_callback_option = true;
                      }
                      _ => return Err(BailoutReason::FastUnsupportedParamType),
                    }
                  } else {
                    return Err(BailoutReason::FastUnsupportedParamType);
                  }
                } else {
                  return Err(BailoutReason::FastUnsupportedParamType);
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
                                PathSegment { ident, .. }
                                  if ident == "OpState" =>
                                {
                                  self.has_rc_opstate = true;
                                }
                                _ => {
                                  return Err(
                                    BailoutReason::FastUnsupportedParamType,
                                  )
                                }
                              }
                            }
                            _ => {
                              return Err(
                                BailoutReason::FastUnsupportedParamType,
                              )
                            }
                          }
                        }
                      }
                      _ => return Err(BailoutReason::FastUnsupportedParamType),
                    }
                  }
                  _ => return Err(BailoutReason::FastUnsupportedParamType),
                }
              }
            }
            // Is `T` a fast scalar?
            PathSegment { ident, .. } => {
              if let Some(val) = get_fast_scalar(ident.to_string().as_str()) {
                self.fast_parameters.push(val);
              } else {
                return Err(BailoutReason::FastUnsupportedParamType);
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
              _ => return Err(BailoutReason::FastUnsupportedParamType),
            }
          }
          // &mut [T]
          Type::Slice(TypeSlice { elem, .. }) => match &**elem {
            Type::Path(TypePath {
              path: Path { segments, .. },
              ..
            }) => {
              let segment = single_segment(segments)?;
              let is_mut_ref = mutability.is_some();
              match segment {
                // Is `T` a u8?
                PathSegment { ident, .. } if ident == "u8" => {
                  self.needs_fast_callback_option = true;
                  self.fast_parameters.push(FastValue::Uint8Array);
                  assert!(self
                    .transforms
                    .insert(index, Transform::slice_u8(index, is_mut_ref))
                    .is_none());
                }
                // Is `T` a u32?
                PathSegment { ident, .. } if ident == "u32" => {
                  self.needs_fast_callback_option = true;
                  self.fast_parameters.push(FastValue::Uint32Array);
                  assert!(self
                    .transforms
                    .insert(index, Transform::slice_u32(index, is_mut_ref))
                    .is_none());
                }
                _ => return Err(BailoutReason::FastUnsupportedParamType),
              }
            }
            _ => return Err(BailoutReason::FastUnsupportedParamType),
          },
          _ => return Err(BailoutReason::FastUnsupportedParamType),
        },
        // *const T
        Type::Ptr(TypePtr {
          elem,
          const_token: Some(_),
          ..
        }) => match &**elem {
          Type::Path(TypePath {
            path: Path { segments, .. },
            ..
          }) => {
            let segment = single_segment(segments)?;
            match segment {
              // Is `T` a u8?
              PathSegment { ident, .. } if ident == "u8" => {
                self.needs_fast_callback_option = true;
                self.fast_parameters.push(FastValue::Uint8Array);
                assert!(self
                  .transforms
                  .insert(index, Transform::u8_ptr(index))
                  .is_none());
              }
              _ => return Err(BailoutReason::FastUnsupportedParamType),
            }
          }
          _ => return Err(BailoutReason::FastUnsupportedParamType),
        },
        _ => return Err(BailoutReason::FastUnsupportedParamType),
      },
      _ => return Err(BailoutReason::FastUnsupportedParamType),
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
  use crate::{Attributes, Op};
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

  #[testing_macros::fixture("optimizer_tests/**/*.rs")]
  fn test_analyzer(input: PathBuf) {
    let update_expected = std::env::var("UPDATE_EXPECTED").is_ok();

    let source =
      std::fs::read_to_string(&input).expect("Failed to read test file");
    let expected = std::fs::read_to_string(input.with_extension("expected"))
      .expect("Failed to read expected file");

    let mut attrs = Attributes::default();
    if source.contains("// @test-attr:wasm") {
      attrs.must_be_fast = true;
      attrs.is_wasm = true;
    }
    if source.contains("// @test-attr:fast") {
      attrs.must_be_fast = true;
    }

    let item = syn::parse_str(&source).expect("Failed to parse test file");
    let mut op = Op::new(item, attrs);
    let mut optimizer = Optimizer::new();
    if let Err(e) = optimizer.analyze(&mut op) {
      let e_str = format!("{:?}", e);
      if update_expected {
        std::fs::write(input.with_extension("expected"), e_str)
          .expect("Failed to write expected file");
      } else {
        assert_eq!(e_str, expected);
      }
      return;
    }

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
