/// Optimizer for #[op]
use crate::Op;
use syn::{
  FnArg, ItemFn, Path, PathArguments, PathSegment, ReturnType, Signature, Type,
  TypePath,
};

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
        if segments.len() != 1 {
          return Err(BailoutReason::MustBeSingleSegment);
        }

        let segment = match segments.last() {
          Some(segment) => segment,
          None => return Err(BailoutReason::MustBeSingleSegment),
        };

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
    // Is FastApiCallbackOption?

    // Is &mut OpState?

    // Is Rc<RefCell<OpState>>?

    // Is serde_v8::Value?

    // Is a scalar FastValue?

    // Is a sequence FastValue?

    // Is &mut [u8] or &mut [u32]?

    Ok(())
  }
}
