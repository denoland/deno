// Copyright 2018-2025 the Deno authors. MIT license.

use std::marker::PhantomData;

use super::erased_future::TypeErased;
use super::future_arena::FutureContextMapper;
use crate::OpId;
use crate::PromiseId;
use deno_error::JsErrorBox;
use deno_error::JsErrorClass;

const MAX_RESULT_SIZE: usize = 32;

pub type MappedResult<'s, 'i, C> = Result<
  <C as OpMappingContextLifetime<'s, 'i>>::Result,
  <C as OpMappingContextLifetime<'s, 'i>>::Result,
>;
pub type UnmappedResult<'s, 'i, C> = Result<
  <C as OpMappingContextLifetime<'s, 'i>>::Result,
  <C as OpMappingContextLifetime<'s, 'i>>::MappingError,
>;

pub trait OpMappingContextLifetime<'s, 'i> {
  type Context
  where
    'i: 's;
  type Result;
  type MappingError;

  fn map_error(context: &mut Self::Context, err: JsErrorBox) -> Self::Result;
  fn map_mapping_error(
    context: &mut Self::Context,
    err: Self::MappingError,
  ) -> Self::Result;
}

/// A type that allows for arbitrary mapping systems for op output with lifetime
/// control. We add this extra layer of generics because we want to test our drivers
/// without requiring V8.
pub trait OpMappingContext:
  for<'s, 'i> OpMappingContextLifetime<'s, 'i> + 'static
{
  type MappingFn<R: 'static>: Copy;

  fn erase_mapping_fn<R: 'static>(f: Self::MappingFn<R>) -> *const fn();
  fn unerase_mapping_fn<'s, 'i, R: 'static>(
    f: *const fn(),
    scope: &mut <Self as OpMappingContextLifetime<'s, 'i>>::Context,
    r: R,
  ) -> UnmappedResult<'s, 'i, Self>;
}

#[derive(Default)]
pub struct V8OpMappingContext {}

pub type V8RetValMapper<R> =
  for<'r, 'i> fn(
    &mut v8::PinScope<'r, 'i>,
    R,
  ) -> Result<v8::Local<'r, v8::Value>, serde_v8::Error>;

impl<'s, 'i> OpMappingContextLifetime<'s, 'i> for V8OpMappingContext {
  type Context
    = v8::PinScope<'s, 'i>
  where
    'i: 's;
  type Result = v8::Local<'s, v8::Value>;
  type MappingError = serde_v8::Error;

  #[inline(always)]
  fn map_error(
    scope: &mut v8::PinScope<'s, 'i>,
    err: JsErrorBox,
  ) -> Self::Result {
    crate::error::to_v8_error(scope, &err)
  }

  fn map_mapping_error(
    scope: &mut v8::PinScope<'s, 'i>,
    err: Self::MappingError,
  ) -> v8::Local<'s, v8::Value> {
    crate::error::to_v8_error(scope, &err)
  }
}

impl OpMappingContext for V8OpMappingContext {
  type MappingFn<R: 'static> = V8RetValMapper<R>;

  #[inline(always)]
  fn erase_mapping_fn<R: 'static>(f: Self::MappingFn<R>) -> *const fn() {
    f as _
  }

  #[inline(always)]
  fn unerase_mapping_fn<'s, 'i: 's, R: 'static>(
    f: *const fn(),
    scope: &mut <Self as OpMappingContextLifetime<'s, 'i>>::Context,
    r: R,
  ) -> Result<v8::Local<'s, v8::Value>, serde_v8::Error> {
    let f: Self::MappingFn<R> = unsafe { std::mem::transmute(f) };
    f(scope, r)
  }
}

/// [`PendingOp`] holds the metadata and result (success or failure) of a submitted
/// and completed op.
pub struct PendingOp<C: OpMappingContext>(pub PendingOpInfo, pub OpResult<C>);

impl<C: OpMappingContext> PendingOp<C> {
  #[inline(always)]
  pub fn new<R: 'static, E: JsErrorClass + 'static>(
    info: PendingOpInfo,
    rv_map: C::MappingFn<R>,
    result: Result<R, E>,
  ) -> Self {
    match result {
      Ok(r) => PendingOp(info, OpResult::new_value(r, rv_map)),
      Err(err) => PendingOp(info, OpResult::Err(JsErrorBox::from_err(err))),
    }
  }

  #[inline(always)]
  pub fn ok<R: 'static>(
    info: PendingOpInfo,
    rv_map: C::MappingFn<R>,
    r: R,
  ) -> Self {
    PendingOp(info, OpResult::new_value(r, rv_map))
  }
}

#[derive(Clone, Copy)]
pub struct PendingOpInfo(pub PromiseId, pub OpId);

pub struct PendingOpMappingInfo<
  C: OpMappingContext,
  R: 'static,
  const FALLIBLE: bool,
>(pub PendingOpInfo, pub C::MappingFn<R>);

impl<C: OpMappingContext, R: 'static, const FALLIBLE: bool> Copy
  for PendingOpMappingInfo<C, R, FALLIBLE>
{
}
impl<C: OpMappingContext, R: 'static, const FALLIBLE: bool> Clone
  for PendingOpMappingInfo<C, R, FALLIBLE>
{
  #[inline(always)]
  fn clone(&self) -> Self {
    *self
  }
}

impl<C: OpMappingContext, R: 'static, E: JsErrorClass + 'static>
  FutureContextMapper<PendingOp<C>, PendingOpInfo, Result<R, E>>
  for PendingOpMappingInfo<C, R, true>
{
  fn context(&self) -> PendingOpInfo {
    self.0
  }

  fn map(&self, r: Result<R, E>) -> PendingOp<C> {
    PendingOp::new(self.0, self.1, r)
  }
}

impl<C: OpMappingContext, R: 'static>
  FutureContextMapper<PendingOp<C>, PendingOpInfo, R>
  for PendingOpMappingInfo<C, R, false>
{
  fn context(&self) -> PendingOpInfo {
    self.0
  }

  fn map(&self, r: R) -> PendingOp<C> {
    PendingOp::ok(self.0, self.1, r)
  }
}

type MapRawFn<C> = for<'a, 'i> fn(
  _lifetime: PhantomData<(&'a (), &'i ())>,
  scope: &mut <C as OpMappingContextLifetime<'a, 'i>>::Context,
  rv_map: *const fn(),
  value: TypeErased<MAX_RESULT_SIZE>,
) -> UnmappedResult<'a, 'i, C>;

#[allow(clippy::type_complexity)]
pub struct OpValue<C: OpMappingContext> {
  value: TypeErased<MAX_RESULT_SIZE>,
  rv_map: *const fn(),
  map_fn: MapRawFn<C>,
}

impl<C: OpMappingContext> OpValue<C> {
  pub fn new<R: 'static>(rv_map: C::MappingFn<R>, v: R) -> Self {
    Self {
      value: TypeErased::new(v),
      rv_map: C::erase_mapping_fn(rv_map),
      map_fn: |_, scope, rv_map, erased| unsafe {
        let r = erased.take();
        C::unerase_mapping_fn::<R>(rv_map, scope, r)
      },
    }
  }
}

pub trait ValueLargeFn<C: OpMappingContext> {
  fn unwrap<'a, 'i>(
    self: Box<Self>,
    ctx: &mut <C as OpMappingContextLifetime<'a, 'i>>::Context,
  ) -> UnmappedResult<'a, 'i, C>;
}

struct ValueLarge<C: OpMappingContext, R: 'static> {
  v: R,
  rv_map: C::MappingFn<R>,
}

impl<C: OpMappingContext, R> ValueLargeFn<C> for ValueLarge<C, R> {
  fn unwrap<'a, 'i>(
    self: Box<Self>,
    ctx: &mut <C as OpMappingContextLifetime<'a, 'i>>::Context,
  ) -> UnmappedResult<'a, 'i, C> {
    C::unerase_mapping_fn(C::erase_mapping_fn(self.rv_map), ctx, self.v)
  }
}

pub enum OpResult<C: OpMappingContext> {
  /// Errors.
  Err(JsErrorBox),
  /// For small ops, we include them in an erased type container.
  Value(OpValue<C>),
  /// For ops that return "large" results (> MAX_RESULT_SIZE bytes) we just box a function
  /// that can turn it into a v8 value.
  ValueLarge(Box<dyn ValueLargeFn<C>>),
}

impl<C: OpMappingContext> OpResult<C> {
  pub fn new_value<R: 'static>(v: R, rv_map: C::MappingFn<R>) -> Self {
    if std::mem::size_of::<R>() > MAX_RESULT_SIZE {
      OpResult::ValueLarge(Box::new(ValueLarge { v, rv_map }))
    } else {
      OpResult::Value(OpValue::new(rv_map, v))
    }
  }

  pub fn unwrap<'a, 'i>(
    self,
    context: &mut <C as OpMappingContextLifetime<'a, 'i>>::Context,
  ) -> MappedResult<'a, 'i, C> {
    let (success, res) = match self {
      Self::Err(err) => (false, Ok(C::map_error(context, err))),
      Self::Value(f) => {
        (true, (f.map_fn)(PhantomData, context, f.rv_map, f.value))
      }
      Self::ValueLarge(f) => (true, f.unwrap(context)),
    };
    match (success, res) {
      (true, Ok(x)) => Ok(x),
      (false, Ok(x)) => Err(x),
      (_, Err(err)) => {
        Err(<C as OpMappingContextLifetime<'a, 'i>>::map_mapping_error(
          context, err,
        ))
      }
    }
  }
}
