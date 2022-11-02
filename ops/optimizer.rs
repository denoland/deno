/// Optimizer for #[op]
use crate::Op;
use syn::{ItemFn, ReturnType, Signature, TypePath, Path, Type, PathSegment, PathArguments};

enum BailoutReason {
  FastAsync,
  MustBeSingleSegment,
  Other(&'static str),
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

  fast_result: Option<FastValue>,
  fast_parameters: Vec<FastValue>,
}

impl Optimizer {
  pub(crate) fn new() -> Self {
    Default::default()
  }

  pub(crate) fn analyze(&mut self, op: &mut Op) -> Result<(), BailoutReason> {
    if op.is_async && op.attrs.must_be_fast {
      return Err(BailoutReason::FastAsync);
    }

    match &op.item.sig {
      Signature {
        output: ReturnType::Default,
        ..
      } => self.fast_result = Some(FastValue::default()),
      Signature {
        output: ReturnType::Type(_, ty),
        ..
      } => self.analyze_return_type(ty)?,
    };

    Ok(())
  }

  fn analyze_return_type(&mut self, ty: &Type) -> Result<(), BailoutReason> {
    match ty {
        Type::Path(TypePath { path: Path { segments, .. }, .. }) => {
          if segments.len() != 1 {
            return Err(BailoutReason::MustBeSingleSegment);             
          }

          let segment = match segments.last() {
            Some(segment) => segment,
            None => return Err(BailoutReason::MustBeSingleSegment),
          };

          match segment {
            PathSegment { ident, arguments, .. } if ident == "Result" => {
                self.returns_result = true;
                if let PathArguments::AngleBracketed(ref bracketed) = arguments {
                    
                }
            }
            _ => {}
          };
        }
        _ => {},
    };

    Ok(())
  }
}
