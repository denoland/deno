use deno_core::cppgc::SameObject;
use deno_core::{op2, v8, WebIDL};
use deno_core::GarbageCollected;

struct GPUAdapter {
  features: SameObject<GPUAdapter>,
  limits: SameObject<GPUAdapter>,
  is_fallback: bool,
}

impl GarbageCollected for GPUAdapter {}

#[op2]
impl GPUAdapter {
  #[getter]
  #[global]
  fn features(&self, scope: &mut v8::HandleScope) -> v8::Global<v8::Object> {
    self.features.get(scope, || ())
  }
  #[getter]
  #[global]
  fn limits(&self, scope: &mut v8::HandleScope) -> v8::Global<v8::Object> {
    self.limits.get(scope, || ())
  }
  #[getter]
  fn is_fallback_adapter(&self) -> bool {
    self.is_fallback
  }
  
  #[async_method]
  #[cppgc]
  async fn request_device(&self, #[webidl] init: GPUDeviceDescriptor) -> GPUDevice {
    
  }
}

#[derive(Debug, WebIDL)]
#[webidl(enum)]
enum GPUFeatureName {
  // api
  DepthClipControl,
  TimestampQuery,
  IndirectFirstInstance,
  // shader
  ShaderF16,
  // texture formats
  #[webidl(rename = "depth32float-stencil8")]
  Depth32floatStencil8,
  TextureCompressionBc,
  TextureCompressionEtc2,
  TextureCompressionAstc,
  #[webidl(rename = "rg11b10ufloat-renderable")]
  Rg11b10ufloatRenderable,
  #[webidl(rename = "bgra8unorm-storage")]
  Bgra8unormStorage,
  #[webidl(rename = "bgra8unorm-storage")]
  "float32-filterable",
  
  // extended from spec
  
  // texture formats
  "texture-format-16-bit-norm",
  "texture-compression-astc-hdr",
  "texture-adapter-specific-format-features",
  // api
  //"pipeline-statistics-query",
  "timestamp-query-inside-passes",
  "mappable-primary-buffers",
  "texture-binding-array",
  "buffer-binding-array",
  "storage-resource-binding-array",
  "sampled-texture-and-storage-buffer-array-non-uniform-indexing",
  "uniform-buffer-and-storage-texture-array-non-uniform-indexing",
  "partially-bound-binding-array",
  "multi-draw-indirect",
  "multi-draw-indirect-count",
  "push-constants",
  "address-mode-clamp-to-zero",
  "address-mode-clamp-to-border",
  "polygon-mode-line",
  "polygon-mode-point",
  "conservative-rasterization",
  "vertex-writable-storage",
  "clear-texture",
  "spirv-shader-passthrough",
  "multiview",
  "vertex-attribute-64-bit",
  // shader
  "shader-f64",
  "shader-i16",
  "shader-primitive-index",
  "shader-early-depth-test",
}

#[derive(Debug, WebIDL)]
#[webidl(dictionary)]
struct GPUDeviceDescriptor {
  #[webidl(default = vec![])]
  required_features: Vec<()>,
}

struct GPUDevice {}

impl GarbageCollected for GPUDevice {}
