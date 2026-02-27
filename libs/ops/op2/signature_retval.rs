// Copyright 2018-2025 the Deno authors. MIT license.

use crate::op2::signature::*;

use syn::PathArguments;
use syn::ReturnType;
use syn::Type;
use syn::TypeParamBound;
use syn::spanned::Spanned;

/// One level of type unwrapping for a return value. We cannot rely on `proc-macro-rules` to correctly
/// unwrap `impl Future<...>`, so we do it by hand.
enum UnwrappedReturn {
  Type(Type),
  Result(Type),
  Future(Type),
}

fn unwrap_return(ty: &Type) -> Result<UnwrappedReturn, RetError> {
  match ty {
    Type::ImplTrait(imp) => {
      if imp
        .bounds
        .iter()
        .filter(|b| {
          matches!(
            b,
            TypeParamBound::Lifetime(_)
              | TypeParamBound::Trait(_)
              | TypeParamBound::Verbatim(_)
          )
        })
        .count()
        > 1
      {
        return Err(RetError::InvalidType(ArgError::InvalidType(
          ty.span(),
          "for impl trait bounds",
        )));
      }
      match imp.bounds.first() {
        Some(TypeParamBound::Trait(t)) => {
          if let Some(seg) = t.path.segments.last()
            && seg.ident == "Future"
            && let PathArguments::AngleBracketed(args) = &seg.arguments
            && let Some(syn::GenericArgument::AssocType(assoc)) =
              args.args.first()
            && assoc.ident == "Output"
          {
            Ok(UnwrappedReturn::Future(assoc.ty.clone()))
          } else {
            Err(RetError::InvalidType(ArgError::InvalidType(
              ty.span(),
              "for impl Future",
            )))
          }
        }
        _ => Err(RetError::InvalidType(ArgError::InvalidType(
          ty.span(),
          "for impl",
        ))),
      }
    }
    Type::Path(tp) => {
      if let Some(seg) = tp.path.segments.last()
        && seg.ident == "Result"
        && let PathArguments::AngleBracketed(args) = &seg.arguments
        && let Some(syn::GenericArgument::Type(ty)) = args.args.first()
      {
        Ok(UnwrappedReturn::Result(ty.clone()))
      } else {
        Ok(UnwrappedReturn::Type(ty.clone()))
      }
    }
    Type::Tuple(_) => Ok(UnwrappedReturn::Type(ty.clone())),
    Type::Ptr(_) => Ok(UnwrappedReturn::Type(ty.clone())),
    Type::Reference(_) => Ok(UnwrappedReturn::Type(ty.clone())),
    _ => Err(RetError::InvalidType(ArgError::InvalidType(
      ty.span(),
      "for return type",
    ))),
  }
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[allow(clippy::large_enum_variant)]
pub enum RetVal {
  /// An op that can never fail.
  Value(Arg),
  /// An op returning Result<Something, ...>
  Result(Box<RetVal>),
  /// An op returning a future, either `async fn() -> Something` or `fn() -> impl Future<Output = Something>`.
  Future(Box<RetVal>),
}

impl RetVal {
  pub fn is_async(&self) -> bool {
    match self {
      RetVal::Value(_) => false,
      RetVal::Result(inner) => inner.is_async(),
      RetVal::Future(_) => true,
    }
  }

  pub fn get_future(&self) -> Option<&RetVal> {
    match self {
      RetVal::Value(_) => None,
      RetVal::Result(inner) => inner.get_future(),
      RetVal::Future(arg) => Some(&**arg),
    }
  }

  /// If this function returns a `Result<T, E>` (including if `T` is a `Future`), return `Some(T)`.
  pub fn unwrap_result(&self) -> Option<&RetVal> {
    match self {
      RetVal::Result(arg) => Some(&**arg),
      RetVal::Future(_) => None,
      RetVal::Value(_) => None,
    }
  }

  pub fn arg(&self) -> &Arg {
    match self {
      RetVal::Value(arg) => arg,
      RetVal::Result(inner) => inner.arg(),
      RetVal::Future(inner) => inner.arg(),
    }
  }
}

impl RetVal {
  pub(crate) fn try_parse(
    is_async: bool,
    attrs: Attributes,
    rt: &ReturnType,
  ) -> Result<RetVal, RetError> {
    fn handle_type(ty: &Type, attrs: Attributes) -> Result<RetVal, RetError> {
      Ok(match unwrap_return(ty)? {
        UnwrappedReturn::Type(ty) => {
          RetVal::Value(parse_type(Position::RetVal, attrs, &ty)?)
        }
        UnwrappedReturn::Result(ty) => {
          RetVal::Result(Box::new(handle_type(&ty, attrs)?))
        }
        UnwrappedReturn::Future(ty) => {
          RetVal::Future(Box::new(handle_type(&ty, attrs)?))
        }
      })
    }

    let res = match rt {
      ReturnType::Default => {
        RetVal::Value(if attrs.primary == Some(AttributeModifier::Undefined) {
          Arg::VoidUndefined
        } else {
          Arg::Void
        })
      }
      ReturnType::Type(_, rt) => handle_type(rt, attrs)?,
    };

    if is_async {
      Ok(RetVal::Future(Box::new(res)))
    } else {
      Ok(res)
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use syn::parse_str;

  #[test]
  fn test_parse_result() {
    use Arg::*;
    use RetVal::*;

    for (expected, input) in [
      (Value(Void), "()"),
      (Result(Box::new(Value(Void))), "Result<()>"),
      (Result(Box::new(Value(Void))), "Result<(), ()>"),
      (Result(Box::new(Value(Void))), "Result<(), (),>"),
      (Future(Box::new(Value(Void))), "impl Future<Output = ()>"),
      (
        Future(Box::new(Result(Box::new(Value(Void))))),
        "impl Future<Output = Result<()>>",
      ),
      (
        Result(Box::new(Future(Box::new(Value(Void))))),
        "Result<impl Future<Output = ()>>",
      ),
      (
        Result(Box::new(Future(Box::new(Result(Box::new(Value(Void))))))),
        "Result<impl Future<Output = Result<()>>>",
      ),
    ] {
      let rt = parse_str::<ReturnType>(&format!("-> {input}"))
        .expect("Failed to parse");
      let actual = RetVal::try_parse(false, Attributes::default(), &rt)
        .expect("Failed to parse return");
      assert_eq!(expected, actual);
    }
  }
}
