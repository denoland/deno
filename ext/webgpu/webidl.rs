// Copyright 2018-2026 the Deno authors. MIT license.

use std::borrow::Cow;

use deno_core::WebIDL;
use deno_core::cppgc::Ref;
use deno_core::v8;
use deno_core::webidl::ConstrainedSequence;
use deno_core::webidl::ContextFn;
use deno_core::webidl::IntOptions;
use deno_core::webidl::SequenceLengthAtMost;
use deno_core::webidl::SequenceLengthExact;
use deno_core::webidl::SequenceLengthPolicy;
use deno_core::webidl::SequenceLengthRange;
use deno_core::webidl::WebIdlConverter;
use deno_core::webidl::WebIdlError;
use deno_core::webidl::WebIdlErrorKind;
use deno_error::JsErrorBox;

fn convert_webgpu_number_sequence<'a, 'b, 'i, T, P, const MAX: usize>(
  scope: &mut v8::PinScope<'a, 'i>,
  value: v8::Local<'a, v8::Value>,
  prefix: Cow<'static, str>,
  context: ContextFn<'b>,
  options: &T::Options,
  type_name: &'static str,
) -> Result<ConstrainedSequence<T, P, MAX>, WebIdlError>
where
  T: WebIdlConverter<'a>,
  P: SequenceLengthPolicy,
{
  match ConstrainedSequence::<T, P, MAX>::convert(
    scope,
    value,
    prefix.clone(),
    context.borrowed(),
    options,
  ) {
    Ok(sequence) => Ok(sequence),
    Err(WebIdlError {
      kind: WebIdlErrorKind::InvalidSequenceLength { actual, .. },
      ..
    }) => Err(WebIdlError::other(
      prefix,
      context,
      JsErrorBox::type_error(format!(
        "A sequence of number used as a {type_name} must have {}, received {actual} elements",
        P::expectation()
      )),
    )),
    Err(err) => Err(err),
  }
}

#[derive(WebIDL)]
#[webidl(dictionary)]
pub(crate) struct GPUExtent3DDict {
  #[options(enforce_range = true)]
  width: u32,
  #[webidl(default = 1)]
  #[options(enforce_range = true)]
  height: u32,
  #[webidl(default = 1)]
  #[options(enforce_range = true)]
  depth_or_array_layers: u32,
}

pub(crate) enum GPUExtent3D {
  Dict(GPUExtent3DDict),
  Sequence((u32, u32, u32)),
}

impl<'a> WebIdlConverter<'a> for GPUExtent3D {
  type Options = ();

  fn convert<'b>(
    scope: &mut v8::PinScope<'a, '_>,
    value: v8::Local<'a, v8::Value>,
    prefix: Cow<'static, str>,
    context: ContextFn<'b>,
    options: &Self::Options,
  ) -> Result<Self, WebIdlError> {
    if value.is_null_or_undefined() {
      return Ok(GPUExtent3D::Dict(GPUExtent3DDict::convert(
        scope,
        value,
        prefix,
        context.borrowed(),
        options,
      )?));
    }
    if let Ok(obj) = value.try_cast::<v8::Object>() {
      let iter = v8::Symbol::get_iterator(scope);
      if let Some(iter) = obj.get(scope, iter.into())
        && !iter.is_undefined()
      {
        let conv =
          convert_webgpu_number_sequence::<u32, SequenceLengthRange<1, 3>, 3>(
            scope,
            value,
            prefix,
            context,
            &IntOptions {
              clamp: false,
              enforce_range: true,
            },
            "GPUExtent3D",
          )?;
        return Ok(GPUExtent3D::Sequence((
          conv[0],
          conv.get(1).copied().unwrap_or(1),
          conv.get(2).copied().unwrap_or(1),
        )));
      }

      return Ok(GPUExtent3D::Dict(GPUExtent3DDict::convert(
        scope, value, prefix, context, options,
      )?));
    }

    Err(WebIdlError::new(
      prefix,
      context,
      WebIdlErrorKind::ConvertToConverterType(
        "sequence<GPUIntegerCoordinate> or GPUExtent3DDict",
      ),
    ))
  }
}

impl From<GPUExtent3D> for wgpu_types::Extent3d {
  fn from(value: GPUExtent3D) -> Self {
    match value {
      GPUExtent3D::Dict(dict) => Self {
        width: dict.width,
        height: dict.height,
        depth_or_array_layers: dict.depth_or_array_layers,
      },
      GPUExtent3D::Sequence((width, height, depth)) => Self {
        width,
        height,
        depth_or_array_layers: depth,
      },
    }
  }
}

#[derive(WebIDL)]
#[webidl(dictionary)]
pub(crate) struct GPUOrigin3DDict {
  #[webidl(default = 0)]
  #[options(enforce_range = true)]
  x: u32,
  #[webidl(default = 0)]
  #[options(enforce_range = true)]
  y: u32,
  #[webidl(default = 0)]
  #[options(enforce_range = true)]
  z: u32,
}

pub(crate) enum GPUOrigin3D {
  Dict(GPUOrigin3DDict),
  Sequence((u32, u32, u32)),
}

impl Default for GPUOrigin3D {
  fn default() -> Self {
    GPUOrigin3D::Sequence((0, 0, 0))
  }
}

impl<'a> WebIdlConverter<'a> for GPUOrigin3D {
  type Options = ();

  fn convert<'b>(
    scope: &mut v8::PinScope<'a, '_>,
    value: v8::Local<'a, v8::Value>,
    prefix: Cow<'static, str>,
    context: ContextFn<'b>,
    options: &Self::Options,
  ) -> Result<Self, WebIdlError> {
    if value.is_null_or_undefined() {
      return Ok(GPUOrigin3D::Dict(GPUOrigin3DDict::convert(
        scope,
        value,
        prefix,
        context.borrowed(),
        options,
      )?));
    }
    if let Ok(obj) = value.try_cast::<v8::Object>() {
      let iter = v8::Symbol::get_iterator(scope);
      if let Some(iter) = obj.get(scope, iter.into())
        && !iter.is_undefined()
      {
        let conv =
          convert_webgpu_number_sequence::<u32, SequenceLengthAtMost<3>, 3>(
            scope,
            value,
            prefix,
            context,
            &IntOptions {
              clamp: false,
              enforce_range: true,
            },
            "GPUOrigin3D",
          )?;
        return Ok(GPUOrigin3D::Sequence((
          conv.first().copied().unwrap_or(0),
          conv.get(1).copied().unwrap_or(0),
          conv.get(2).copied().unwrap_or(0),
        )));
      }

      return Ok(GPUOrigin3D::Dict(GPUOrigin3DDict::convert(
        scope, value, prefix, context, options,
      )?));
    }

    Err(WebIdlError::new(
      prefix,
      context,
      WebIdlErrorKind::ConvertToConverterType(
        "sequence<GPUIntegerCoordinate> or GPUOrigin3DDict",
      ),
    ))
  }
}

impl From<GPUOrigin3D> for wgpu_types::Origin3d {
  fn from(value: GPUOrigin3D) -> Self {
    match value {
      GPUOrigin3D::Dict(dict) => Self {
        x: dict.x,
        y: dict.y,
        z: dict.z,
      },
      GPUOrigin3D::Sequence((x, y, z)) => Self { x, y, z },
    }
  }
}

#[derive(WebIDL)]
#[webidl(dictionary)]
pub(crate) struct GPUColorDict {
  r: f64,
  g: f64,
  b: f64,
  a: f64,
}

pub(crate) enum GPUColor {
  Dict(GPUColorDict),
  Sequence((f64, f64, f64, f64)),
}

impl<'a> WebIdlConverter<'a> for GPUColor {
  type Options = ();

  fn convert<'b>(
    scope: &mut v8::PinScope<'a, '_>,
    value: v8::Local<'a, v8::Value>,
    prefix: Cow<'static, str>,
    context: ContextFn<'b>,
    options: &Self::Options,
  ) -> Result<Self, WebIdlError> {
    if value.is_null_or_undefined() {
      return Ok(GPUColor::Dict(GPUColorDict::convert(
        scope,
        value,
        prefix,
        context.borrowed(),
        options,
      )?));
    }
    if let Ok(obj) = value.try_cast::<v8::Object>() {
      let iter = v8::Symbol::get_iterator(scope);
      if let Some(iter) = obj.get(scope, iter.into())
        && !iter.is_undefined()
      {
        let conv =
          convert_webgpu_number_sequence::<f64, SequenceLengthExact<4>, 4>(
            scope, value, prefix, context, options, "GPUColor",
          )?;
        return Ok(GPUColor::Sequence((conv[0], conv[1], conv[2], conv[3])));
      }

      return Ok(GPUColor::Dict(GPUColorDict::convert(
        scope, value, prefix, context, options,
      )?));
    }

    Err(WebIdlError::new(
      prefix,
      context,
      WebIdlErrorKind::ConvertToConverterType(
        "sequence<GPUIntegerCoordinate> or GPUOrigin3DDict",
      ),
    ))
  }
}

impl From<GPUColor> for wgpu_types::Color {
  fn from(value: GPUColor) -> Self {
    match value {
      GPUColor::Dict(dict) => Self {
        r: dict.r,
        g: dict.g,
        b: dict.b,
        a: dict.a,
      },
      GPUColor::Sequence((r, g, b, a)) => Self { r, g, b, a },
    }
  }
}

#[derive(WebIDL)]
#[webidl(enum)]
pub(crate) enum GPUAutoLayoutMode {
  Auto,
}

pub(crate) enum GPUPipelineLayoutOrGPUAutoLayoutMode {
  PipelineLayout(Ref<crate::pipeline_layout::GPUPipelineLayout>),
  AutoLayoutMode(GPUAutoLayoutMode),
}

impl From<GPUPipelineLayoutOrGPUAutoLayoutMode>
  for Option<wgpu_core::id::PipelineLayoutId>
{
  fn from(value: GPUPipelineLayoutOrGPUAutoLayoutMode) -> Self {
    match value {
      GPUPipelineLayoutOrGPUAutoLayoutMode::PipelineLayout(layout) => {
        Some(layout.id)
      }
      GPUPipelineLayoutOrGPUAutoLayoutMode::AutoLayoutMode(
        GPUAutoLayoutMode::Auto,
      ) => None,
    }
  }
}

impl<'a> WebIdlConverter<'a> for GPUPipelineLayoutOrGPUAutoLayoutMode {
  type Options = ();

  fn convert<'b>(
    scope: &mut v8::PinScope<'a, '_>,
    value: v8::Local<'a, v8::Value>,
    prefix: Cow<'static, str>,
    context: ContextFn<'b>,
    options: &Self::Options,
  ) -> Result<Self, WebIdlError> {
    if value.is_object() {
      Ok(Self::PipelineLayout(WebIdlConverter::convert(
        scope, value, prefix, context, options,
      )?))
    } else {
      Ok(Self::AutoLayoutMode(WebIdlConverter::convert(
        scope, value, prefix, context, options,
      )?))
    }
  }
}

impl<'a> WebIdlConverter<'a> for GPUFeatureName {
  type Options = ();

  fn convert<'b>(
    scope: &mut v8::PinScope<'a, '_>,
    value: v8::Local<'a, v8::Value>,
    prefix: Cow<'static, str>,
    context: ContextFn<'b>,
    _options: &Self::Options,
  ) -> Result<Self, WebIdlError> {
    let s = value.to_rust_string_lossy(scope);
    s.parse().map(Self).map_err(|()| {
      WebIdlError::new(
        prefix,
        context,
        WebIdlErrorKind::InvalidEnumVariant {
          converter: "GPUFeatureName",
          variant: s,
        },
      )
    })
  }
}

/// A WebGPU optional feature.
///
/// Named after the WebIDL enum, which represents features as strings, but we store the
/// feature as bitflag, which must always have exactly one bit set (across both the WebGPU
/// and wgpu native features).
#[derive(Clone, Copy, Hash, Eq, PartialEq)]
pub struct GPUFeatureName(wgpu_types::Features);

impl From<GPUFeatureName> for wgpu_types::Features {
  fn from(value: GPUFeatureName) -> wgpu_types::Features {
    value.0
  }
}

#[derive(Clone, Copy)]
pub struct GPUTextureUsageFlags(pub wgpu_types::TextureUsages);

impl<'a> WebIdlConverter<'a> for GPUTextureUsageFlags {
  type Options = ();

  fn convert<'b>(
    scope: &mut v8::PinScope<'a, '_>,
    value: v8::Local<'a, v8::Value>,
    prefix: Cow<'static, str>,
    context: ContextFn<'b>,
    _options: &Self::Options,
  ) -> Result<Self, WebIdlError> {
    let flags_value = u32::convert(
      scope,
      value,
      prefix.clone(),
      context.borrowed(),
      &IntOptions {
        clamp: false,
        enforce_range: true,
      },
    )?;

    let flags =
      wgpu_types::TextureUsages::from_bits(flags_value).ok_or_else(|| {
        WebIdlError::other(
          prefix,
          context,
          JsErrorBox::type_error("usage is not valid"),
        )
      })?;

    Ok(GPUTextureUsageFlags(flags))
  }
}

impl From<GPUTextureUsageFlags> for wgpu_types::TextureUsages {
  fn from(value: GPUTextureUsageFlags) -> Self {
    value.0
  }
}

impl GPUTextureUsageFlags {
  pub fn bits(&self) -> u32 {
    self.0.bits()
  }
}

#[derive(Clone, Copy)]
pub(crate) struct GPUShaderStageFlags(pub(crate) wgpu_types::ShaderStages);

impl<'a> WebIdlConverter<'a> for GPUShaderStageFlags {
  type Options = ();

  fn convert<'b>(
    scope: &mut v8::PinScope<'a, '_>,
    value: v8::Local<'a, v8::Value>,
    prefix: Cow<'static, str>,
    context: ContextFn<'b>,
    _options: &Self::Options,
  ) -> Result<Self, WebIdlError> {
    let flags_value = u32::convert(
      scope,
      value,
      prefix.clone(),
      context.borrowed(),
      &IntOptions {
        clamp: false,
        enforce_range: true,
      },
    )?;

    let flags =
      wgpu_types::ShaderStages::from_bits(flags_value).ok_or_else(|| {
        WebIdlError::other(
          prefix,
          context,
          JsErrorBox::type_error("shader stage is not valid"),
        )
      })?;

    Ok(GPUShaderStageFlags(flags))
  }
}

impl From<GPUShaderStageFlags> for wgpu_types::ShaderStages {
  fn from(value: GPUShaderStageFlags) -> Self {
    value.0
  }
}

#[derive(Clone, Copy)]
pub(crate) struct GPUColorWriteFlags(pub(crate) wgpu_types::ColorWrites);

impl<'a> WebIdlConverter<'a> for GPUColorWriteFlags {
  type Options = ();

  fn convert<'b>(
    scope: &mut v8::PinScope<'a, '_>,
    value: v8::Local<'a, v8::Value>,
    prefix: Cow<'static, str>,
    context: ContextFn<'b>,
    _options: &Self::Options,
  ) -> Result<Self, WebIdlError> {
    let flags_value = u32::convert(
      scope,
      value,
      prefix.clone(),
      context.borrowed(),
      &IntOptions {
        clamp: false,
        enforce_range: true,
      },
    )?;

    // WebGPU specifies a validation error for invalid color write mask values.
    // We propagate invalid bits here; wgpu_core will validate it.
    Ok(GPUColorWriteFlags(
      wgpu_types::ColorWrites::from_bits_retain(flags_value),
    ))
  }
}

impl From<GPUColorWriteFlags> for wgpu_types::ColorWrites {
  fn from(value: GPUColorWriteFlags) -> Self {
    value.0
  }
}
