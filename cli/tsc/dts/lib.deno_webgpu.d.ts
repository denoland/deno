// Copyright 2018-2026 the Deno authors. MIT license.

// deno-lint-ignore-file no-explicit-any no-var

/// <reference no-default-lib="true" />
/// <reference lib="esnext" />

/** @category GPU */
interface GPUObjectBase {
  label: string;
}

/** @category GPU */
interface GPUObjectDescriptorBase {
  label?: string;
}

/** @category GPU */
interface GPUSupportedLimits {
  readonly maxTextureDimension1D: number;
  readonly maxTextureDimension2D: number;
  readonly maxTextureDimension3D: number;
  readonly maxTextureArrayLayers: number;
  readonly maxBindGroups: number;
  readonly maxBindGroupsPlusVertexBuffers: number;
  readonly maxBindingsPerBindGroup: number;
  readonly maxDynamicUniformBuffersPerPipelineLayout: number;
  readonly maxDynamicStorageBuffersPerPipelineLayout: number;
  readonly maxSampledTexturesPerShaderStage: number;
  readonly maxSamplersPerShaderStage: number;
  readonly maxStorageBuffersPerShaderStage: number;
  readonly maxStorageTexturesPerShaderStage: number;
  readonly maxUniformBuffersPerShaderStage: number;
  readonly maxUniformBufferBindingSize: number;
  readonly maxStorageBufferBindingSize: number;
  readonly minUniformBufferOffsetAlignment: number;
  readonly minStorageBufferOffsetAlignment: number;
  readonly maxVertexBuffers: number;
  readonly maxBufferSize: number;
  readonly maxVertexAttributes: number;
  readonly maxVertexBufferArrayStride: number;
  readonly maxInterStageShaderVariables: number;
  readonly maxColorAttachments: number;
  readonly maxColorAttachmentBytesPerSample: number;
  readonly maxComputeWorkgroupStorageSize: number;
  readonly maxComputeInvocationsPerWorkgroup: number;
  readonly maxComputeWorkgroupSizeX: number;
  readonly maxComputeWorkgroupSizeY: number;
  readonly maxComputeWorkgroupSizeZ: number;
  readonly maxComputeWorkgroupsPerDimension: number;
}

/** @category GPU */
declare var GPUSupportedLimits: {
  prototype: GPUSupportedLimits;
  new (): GPUSupportedLimits;
};

/** @category GPU */
interface GPUSupportedFeatures {
  forEach(
    callbackfn: (
      value: string,
      key: string,
      parent: GPUSupportedFeatures,
    ) => void,
    thisArg?: any,
  ): void;
}

/** @category GPU */
declare var GPUSupportedFeatures: {
  prototype: GPUSupportedFeatures;
  new (): GPUSupportedFeatures;
};

/** @category GPU */
interface GPUAdapterInfo {
  readonly vendor: string;
  readonly architecture: string;
  readonly device: string;
  readonly description: string;
  readonly subgroupMinSize: number;
  readonly subgroupMaxSize: number;
  readonly isFallbackAdapter: boolean;
}

/** @category GPU */
declare var GPUAdapterInfo: {
  prototype: GPUAdapterInfo;
  new (): GPUAdapterInfo;
};

/**
 * The entry point to WebGPU in Deno, accessed via the global navigator.gpu property.
 *
 * @example
 * ```ts
 * // Basic WebGPU initialization in Deno
 * const gpu = navigator.gpu;
 * if (!gpu) {
 *   console.error("WebGPU not supported in this Deno environment");
 *   Deno.exit(1);
 * }
 *
 * // Request an adapter (physical GPU device)
 * const adapter = await gpu.requestAdapter();
 * if (!adapter) {
 *   console.error("Couldn't request WebGPU adapter");
 *   Deno.exit(1);
 * }
 *
 * // Get the preferred format for canvas rendering
 * // Useful when working with canvas in browser/Deno environments
 * const preferredFormat = gpu.getPreferredCanvasFormat();
 * console.log(`Preferred canvas format: ${preferredFormat}`);
 *
 * // Create a device with default settings
 * const device = await adapter.requestDevice();
 * console.log("WebGPU device created successfully");
 * ```
 *
 * @category GPU
 */
interface GPU {
  requestAdapter(
    options?: GPURequestAdapterOptions,
  ): Promise<GPUAdapter | null>;
  getPreferredCanvasFormat(): GPUTextureFormat;
}

/** @category GPU */
declare var GPU: {
  prototype: GPU;
  new (): GPU;
};

/** @category GPU */
interface GPURequestAdapterOptions {
  powerPreference?: GPUPowerPreference;
  forceFallbackAdapter?: boolean;
}

/** @category GPU */
type GPUPowerPreference = "low-power" | "high-performance";

/**
 * Represents a physical GPU device that can be used to create a logical GPU device.
 *
 * @example
 * ```ts
 * // Request an adapter with specific power preference
 * const adapter = await navigator.gpu.requestAdapter({
 *   powerPreference: "high-performance"
 * });
 *
 * if (!adapter) {
 *   console.error("WebGPU not supported or no appropriate adapter found");
 *   Deno.exit(1);
 * }
 *
 * // Check adapter capabilities
 * if (adapter.features.has("shader-f16")) {
 *   console.log("Adapter supports 16-bit shader operations");
 * }
 *
 * console.log(`Maximum buffer size: ${adapter.limits.maxBufferSize} bytes`);
 *
 * // Get adapter info (vendor, device, etc.)
 * console.log(`GPU Vendor: ${adapter.info.vendor}`);
 * console.log(`GPU Device: ${adapter.info.device}`);
 *
 * // Request a logical device with specific features and limits
 * const device = await adapter.requestDevice({
 *   requiredFeatures: ["shader-f16"],
 *   requiredLimits: {
 *     maxStorageBufferBindingSize: 128 * 1024 * 1024, // 128MB
 *   }
 * });
 * ```
 *
 * @category GPU
 */
interface GPUAdapter {
  readonly features: GPUSupportedFeatures;
  readonly limits: GPUSupportedLimits;
  readonly info: GPUAdapterInfo;

  requestDevice(descriptor?: GPUDeviceDescriptor): Promise<GPUDevice>;
}

/** @category GPU */
declare var GPUAdapter: {
  prototype: GPUAdapter;
  new (): GPUAdapter;
};

/** @category GPU */
interface GPUDeviceDescriptor extends GPUObjectDescriptorBase {
  requiredFeatures?: GPUFeatureName[];
  requiredLimits?: Record<string, number | undefined>;
}

/** @category GPU */
type GPUFeatureName =
  | "depth-clip-control"
  | "timestamp-query"
  | "indirect-first-instance"
  | "shader-f16"
  | "depth32float-stencil8"
  | "texture-compression-bc"
  | "texture-compression-bc-sliced-3d"
  | "texture-compression-etc2"
  | "texture-compression-astc"
  | "rg11b10ufloat-renderable"
  | "bgra8unorm-storage"
  | "float32-filterable"
  | "clip-distances"
  | "dual-source-blending"
  | "float32-blendable"
  | "subgroups"
  | "core-features-and-limits"
  | "texture-compression-astc-sliced-3d"
  | "texture-formats-tier1"
  | "primitive-index"
  // extended from spec
  | "texture-format-16-bit-norm"
  | "texture-compression-astc-hdr"
  | "texture-adapter-specific-format-features"
  | "pipeline-statistics-query"
  | "timestamp-query-inside-passes"
  | "mappable-primary-buffers"
  | "texture-binding-array"
  | "buffer-binding-array"
  | "storage-resource-binding-array"
  | "sampled-texture-and-storage-buffer-array-non-uniform-indexing"
  | "uniform-buffer-and-storage-texture-array-non-uniform-indexing"
  | "partially-bound-binding-array"
  | "multi-draw-indirect"
  | "multi-draw-indirect-count"
  | "push-constants"
  | "address-mode-clamp-to-zero"
  | "address-mode-clamp-to-border"
  | "polygon-mode-line"
  | "polygon-mode-point"
  | "conservative-rasterization"
  | "vertex-writable-storage"
  | "clear-texture"
  | "spirv-shader-passthrough"
  | "multiview"
  | "vertex-attribute-64-bit"
  | "shader-f64"
  | "shader-i16"
  | "shader-primitive-index"
  | "shader-early-depth-test";

/**
 * The primary interface for interacting with a WebGPU device.
 *
 * @example
 * ```ts
 * // Request a GPU adapter from the browser/Deno
 * const adapter = await navigator.gpu.requestAdapter();
 * if (!adapter) throw new Error("WebGPU not supported");
 *
 * // Request a device from the adapter
 * const device = await adapter.requestDevice();
 *
 * // Create a buffer on the GPU
 * const buffer = device.createBuffer({
 *   size: 128,
 *   usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST,
 * });
 *
 * // Use device.queue to submit commands
 * device.queue.writeBuffer(buffer, 0, new Uint8Array([1, 2, 3, 4]));
 * ```
 *
 * @category GPU
 */
interface GPUDevice extends EventTarget, GPUObjectBase {
  readonly lost: Promise<GPUDeviceLostInfo>;
  pushErrorScope(filter: GPUErrorFilter): void;
  popErrorScope(): Promise<GPUError | null>;

  readonly features: GPUSupportedFeatures;
  readonly limits: GPUSupportedLimits;
  readonly adapterInfo: GPUAdapterInfo;
  readonly queue: GPUQueue;

  destroy(): void;

  createBuffer(descriptor: GPUBufferDescriptor): GPUBuffer;
  createTexture(descriptor: GPUTextureDescriptor): GPUTexture;
  createSampler(descriptor?: GPUSamplerDescriptor): GPUSampler;

  createBindGroupLayout(
    descriptor: GPUBindGroupLayoutDescriptor,
  ): GPUBindGroupLayout;
  createPipelineLayout(
    descriptor: GPUPipelineLayoutDescriptor,
  ): GPUPipelineLayout;
  createBindGroup(descriptor: GPUBindGroupDescriptor): GPUBindGroup;

  createShaderModule(descriptor: GPUShaderModuleDescriptor): GPUShaderModule;
  createComputePipeline(
    descriptor: GPUComputePipelineDescriptor,
  ): GPUComputePipeline;
  createRenderPipeline(
    descriptor: GPURenderPipelineDescriptor,
  ): GPURenderPipeline;
  createComputePipelineAsync(
    descriptor: GPUComputePipelineDescriptor,
  ): Promise<GPUComputePipeline>;
  createRenderPipelineAsync(
    descriptor: GPURenderPipelineDescriptor,
  ): Promise<GPURenderPipeline>;

  createCommandEncoder(
    descriptor?: GPUCommandEncoderDescriptor,
  ): GPUCommandEncoder;
  createRenderBundleEncoder(
    descriptor: GPURenderBundleEncoderDescriptor,
  ): GPURenderBundleEncoder;

  createQuerySet(descriptor: GPUQuerySetDescriptor): GPUQuerySet;
}

/** @category GPU */
declare var GPUDevice: {
  prototype: GPUDevice;
  new (): GPUDevice;
};

/**
 * Represents a block of memory allocated on the GPU.
 *
 * @example
 * ```ts
 * // Create a buffer that can be used as a vertex buffer and can be written to
 * const vertexBuffer = device.createBuffer({
 *   label: "Vertex Buffer",
 *   size: vertices.byteLength,
 *   usage: GPUBufferUsage.VERTEX | GPUBufferUsage.COPY_DST,
 * });
 *
 * // Write data to the buffer
 * device.queue.writeBuffer(vertexBuffer, 0, vertices);
 *
 * // Example of creating a mapped buffer for CPU access
 * const stagingBuffer = device.createBuffer({
 *   size: data.byteLength,
 *   usage: GPUBufferUsage.MAP_WRITE | GPUBufferUsage.COPY_SRC,
 *   mappedAtCreation: true,
 * });
 *
 * // Copy data to the mapped buffer
 * new Uint8Array(stagingBuffer.getMappedRange()).set(data);
 * stagingBuffer.unmap();
 * ```
 *
 * @category GPU
 */
interface GPUBuffer extends GPUObjectBase {
  readonly size: number;
  readonly usage: GPUFlagsConstant;
  readonly mapState: GPUBufferMapState;

  mapAsync(
    mode: GPUMapModeFlags,
    offset?: number,
    size?: number,
  ): Promise<void>;
  getMappedRange(offset?: number, size?: number): ArrayBuffer;
  unmap(): void;

  destroy(): void;
}

/** @category GPU */
declare var GPUBuffer: {
  prototype: GPUBuffer;
  new (): GPUBuffer;
};

/** @category GPU */
type GPUBufferMapState = "unmapped" | "pending" | "mapped";

/** @category GPU */
interface GPUBufferDescriptor extends GPUObjectDescriptorBase {
  size: number;
  usage: GPUBufferUsageFlags;
  mappedAtCreation?: boolean;
}

/** @category GPU */
type GPUBufferUsageFlags = number;

/** @category GPU */
type GPUFlagsConstant = number;

/** @category GPU */
declare class GPUBufferUsage {
  static MAP_READ: 0x0001;
  static MAP_WRITE: 0x0002;
  static COPY_SRC: 0x0004;
  static COPY_DST: 0x0008;
  static INDEX: 0x0010;
  static VERTEX: 0x0020;
  static UNIFORM: 0x0040;
  static STORAGE: 0x0080;
  static INDIRECT: 0x0100;
  static QUERY_RESOLVE: 0x0200;
}

/** @category GPU */
type GPUMapModeFlags = number;

/** @category GPU */
declare class GPUMapMode {
  static READ: 0x0001;
  static WRITE: 0x0002;
}

/**
 * Represents a texture (image) in GPU memory.
 *
 * @example
 * ```ts
 * // Create a texture to render to
 * const texture = device.createTexture({
 *   label: "Output Texture",
 *   size: { width: 640, height: 480 },
 *   format: "rgba8unorm",
 *   usage: GPUTextureUsage.RENDER_ATTACHMENT | GPUTextureUsage.TEXTURE_BINDING,
 * });
 *
 * // Get a view of the texture (needed for most operations)
 * const textureView = texture.createView();
 *
 * // When the texture is no longer needed
 * texture.destroy();
 *
 * // Example: Creating a depth texture
 * const depthTexture = device.createTexture({
 *   size: { width: 640, height: 480 },
 *   format: "depth24plus",
 *   usage: GPUTextureUsage.RENDER_ATTACHMENT,
 * });
 * ```
 *
 * @category GPU
 */
interface GPUTexture extends GPUObjectBase {
  createView(descriptor?: GPUTextureViewDescriptor): GPUTextureView;
  destroy(): void;

  readonly width: number;
  readonly height: number;
  readonly depthOrArrayLayers: number;
  readonly mipLevelCount: number;
  readonly sampleCount: number;
  readonly dimension: GPUTextureDimension;
  readonly format: GPUTextureFormat;
  readonly usage: GPUFlagsConstant;
}

/** @category GPU */
declare var GPUTexture: {
  prototype: GPUTexture;
  new (): GPUTexture;
};

/** @category GPU */
type GPUTextureDimension = "1d" | "2d" | "3d";

/** @category GPU */
type GPUTextureUsageFlags = number;

/** @category GPU */
interface GPUTextureDescriptor extends GPUObjectDescriptorBase {
  size: GPUExtent3D;
  mipLevelCount?: number;
  sampleCount?: number;
  dimension?: GPUTextureDimension;
  format: GPUTextureFormat;
  usage: GPUTextureUsageFlags;
  viewFormats?: GPUTextureFormat[];
}

/** @category GPU */
declare class GPUTextureUsage {
  static COPY_SRC: 0x01;
  static COPY_DST: 0x02;
  static TEXTURE_BINDING: 0x04;
  static STORAGE_BINDING: 0x08;
  static RENDER_ATTACHMENT: 0x10;
}

/** @category GPU */
interface GPUTextureView extends GPUObjectBase {
}

/** @category GPU */
declare var GPUTextureView: {
  prototype: GPUTextureView;
  new (): GPUTextureView;
};

/** @category GPU */
type GPUTextureViewDimension =
  | "1d"
  | "2d"
  | "2d-array"
  | "cube"
  | "cube-array"
  | "3d";

/** @category GPU */
type GPUTextureAspect = "all" | "stencil-only" | "depth-only";

/** @category GPU */
type GPUTextureFormat =
  | "r8unorm"
  | "r8snorm"
  | "r8uint"
  | "r8sint"
  | "r16uint"
  | "r16sint"
  | "r16float"
  | "rg8unorm"
  | "rg8snorm"
  | "rg8uint"
  | "rg8sint"
  | "r32uint"
  | "r32sint"
  | "r32float"
  | "rg16uint"
  | "rg16sint"
  | "rg16float"
  | "rgba8unorm"
  | "rgba8unorm-srgb"
  | "rgba8snorm"
  | "rgba8uint"
  | "rgba8sint"
  | "bgra8unorm"
  | "bgra8unorm-srgb"
  | "rgb9e5ufloat"
  | "rgb10a2uint"
  | "rgb10a2unorm"
  | "rg11b10ufloat"
  | "rg32uint"
  | "rg32sint"
  | "rg32float"
  | "rgba16uint"
  | "rgba16sint"
  | "rgba16float"
  | "rgba32uint"
  | "rgba32sint"
  | "rgba32float"
  | "stencil8"
  | "depth16unorm"
  | "depth24plus"
  | "depth24plus-stencil8"
  | "depth32float"
  | "depth32float-stencil8"
  | "bc1-rgba-unorm"
  | "bc1-rgba-unorm-srgb"
  | "bc2-rgba-unorm"
  | "bc2-rgba-unorm-srgb"
  | "bc3-rgba-unorm"
  | "bc3-rgba-unorm-srgb"
  | "bc4-r-unorm"
  | "bc4-r-snorm"
  | "bc5-rg-unorm"
  | "bc5-rg-snorm"
  | "bc6h-rgb-ufloat"
  | "bc6h-rgb-float"
  | "bc7-rgba-unorm"
  | "bc7-rgba-unorm-srgb"
  | "etc2-rgb8unorm"
  | "etc2-rgb8unorm-srgb"
  | "etc2-rgb8a1unorm"
  | "etc2-rgb8a1unorm-srgb"
  | "etc2-rgba8unorm"
  | "etc2-rgba8unorm-srgb"
  | "eac-r11unorm"
  | "eac-r11snorm"
  | "eac-rg11unorm"
  | "eac-rg11snorm"
  | "astc-4x4-unorm"
  | "astc-4x4-unorm-srgb"
  | "astc-5x4-unorm"
  | "astc-5x4-unorm-srgb"
  | "astc-5x5-unorm"
  | "astc-5x5-unorm-srgb"
  | "astc-6x5-unorm"
  | "astc-6x5-unorm-srgb"
  | "astc-6x6-unorm"
  | "astc-6x6-unorm-srgb"
  | "astc-8x5-unorm"
  | "astc-8x5-unorm-srgb"
  | "astc-8x6-unorm"
  | "astc-8x6-unorm-srgb"
  | "astc-8x8-unorm"
  | "astc-8x8-unorm-srgb"
  | "astc-10x5-unorm"
  | "astc-10x5-unorm-srgb"
  | "astc-10x6-unorm"
  | "astc-10x6-unorm-srgb"
  | "astc-10x8-unorm"
  | "astc-10x8-unorm-srgb"
  | "astc-10x10-unorm"
  | "astc-10x10-unorm-srgb"
  | "astc-12x10-unorm"
  | "astc-12x10-unorm-srgb"
  | "astc-12x12-unorm"
  | "astc-12x12-unorm-srgb";

/** @category GPU */
interface GPUTextureViewDescriptor extends GPUObjectDescriptorBase {
  format?: GPUTextureFormat;
  dimension?: GPUTextureViewDimension;
  usage?: GPUTextureUsageFlags;
  aspect?: GPUTextureAspect;
  baseMipLevel?: number;
  mipLevelCount?: number;
  baseArrayLayer?: number;
  arrayLayerCount?: number;
}

/** @category GPU */
interface GPUSampler extends GPUObjectBase {
}

/** @category GPU */
declare var GPUSampler: {
  prototype: GPUSampler;
  new (): GPUSampler;
};

/** @category GPU */
interface GPUSamplerDescriptor extends GPUObjectDescriptorBase {
  addressModeU?: GPUAddressMode;
  addressModeV?: GPUAddressMode;
  addressModeW?: GPUAddressMode;
  magFilter?: GPUFilterMode;
  minFilter?: GPUFilterMode;
  mipmapFilter?: GPUMipmapFilterMode;
  lodMinClamp?: number;
  lodMaxClamp?: number;
  compare?: GPUCompareFunction;
  maxAnisotropy?: number;
}

/** @category GPU */
type GPUAddressMode = "clamp-to-edge" | "repeat" | "mirror-repeat";

/** @category GPU */
type GPUFilterMode = "nearest" | "linear";

/** @category GPU */
type GPUMipmapFilterMode = "nearest" | "linear";

/** @category GPU */
type GPUCompareFunction =
  | "never"
  | "less"
  | "equal"
  | "less-equal"
  | "greater"
  | "not-equal"
  | "greater-equal"
  | "always";

/** @category GPU */
interface GPUBindGroupLayout extends GPUObjectBase {
}

/** @category GPU */
declare var GPUBindGroupLayout: {
  prototype: GPUBindGroupLayout;
  new (): GPUBindGroupLayout;
};

/** @category GPU */
interface GPUBindGroupLayoutDescriptor extends GPUObjectDescriptorBase {
  entries: GPUBindGroupLayoutEntry[];
}

/** @category GPU */
type GPUShaderStageFlags = number;

/** @category GPU */
interface GPUBindGroupLayoutEntry {
  binding: number;
  visibility: GPUShaderStageFlags;

  buffer?: GPUBufferBindingLayout;
  sampler?: GPUSamplerBindingLayout;
  texture?: GPUTextureBindingLayout;
  storageTexture?: GPUStorageTextureBindingLayout;
}

/** @category GPU */
declare class GPUShaderStage {
  static VERTEX: 0x1;
  static FRAGMENT: 0x2;
  static COMPUTE: 0x4;
}

/** @category GPU */
interface GPUBufferBindingLayout {
  type?: GPUBufferBindingType;
  hasDynamicOffset?: boolean;
  minBindingSize?: number;
}

/** @category GPU */
type GPUBufferBindingType = "uniform" | "storage" | "read-only-storage";

/** @category GPU */
interface GPUSamplerBindingLayout {
  type?: GPUSamplerBindingType;
}

/** @category GPU */
type GPUSamplerBindingType =
  | "filtering"
  | "non-filtering"
  | "comparison";

/** @category GPU */
interface GPUTextureBindingLayout {
  sampleType?: GPUTextureSampleType;
  viewDimension?: GPUTextureViewDimension;
  multisampled?: boolean;
}

/** @category GPU */
type GPUTextureSampleType =
  | "float"
  | "unfilterable-float"
  | "depth"
  | "sint"
  | "uint";

/** @category GPU */
interface GPUStorageTextureBindingLayout {
  access?: GPUStorageTextureAccess;
  format: GPUTextureFormat;
  viewDimension?: GPUTextureViewDimension;
}

/** @category GPU */
type GPUStorageTextureAccess =
  | "write-only"
  | "read-only"
  | "read-write";

/** @category GPU */
interface GPUBindGroup extends GPUObjectBase {
}

/** @category GPU */
declare var GPUBindGroup: {
  prototype: GPUBindGroup;
  new (): GPUBindGroup;
};

/** @category GPU */
interface GPUBindGroupDescriptor extends GPUObjectDescriptorBase {
  layout: GPUBindGroupLayout;
  entries: GPUBindGroupEntry[];
}

/** @category GPU */
interface GPUBindGroupEntry {
  binding: number;
  resource: GPUBindingResource;
}

/** @category GPU */
type GPUBindingResource =
  | GPUSampler
  | GPUTextureView
  | GPUBufferBinding;

/** @category GPU */
interface GPUBufferBinding {
  buffer: GPUBuffer;
  offset?: number;
  size?: number;
}

/** @category GPU */
interface GPUPipelineLayout extends GPUObjectBase {
}

/** @category GPU */
declare var GPUPipelineLayout: {
  prototype: GPUPipelineLayout;
  new (): GPUPipelineLayout;
};

/** @category GPU */
interface GPUPipelineLayoutDescriptor extends GPUObjectDescriptorBase {
  bindGroupLayouts: (GPUBindGroupLayout | null)[];
}

/** @category GPU */
interface GPUCompilationMessage {
  readonly message: string;
  readonly type: GPUCompilationMessageType;
  readonly lineNum: number;
  readonly linePos: number;
  readonly offset: number;
  readonly length: number;
}

/** @category GPU */
declare var GPUCompilationMessage: {
  prototype: GPUCompilationMessage;
  new (): GPUCompilationMessage;
};

/** @category GPU */
type GPUCompilationMessageType = "error" | "warning" | "info";

/** @category GPU */
interface GPUCompilationInfo {
  readonly messages: ReadonlyArray<GPUCompilationMessage>;
}

/** @category GPU */
declare var GPUCompilationInfo: {
  prototype: GPUCompilationInfo;
  new (): GPUCompilationInfo;
};

/**
 * The **`GPUPipelineError`** interface of the WebGPU API describes a pipeline failure.
 * Available only in secure contexts.
 *
 * [MDN Reference](https://developer.mozilla.org/docs/Web/API/GPUPipelineError)
 * @category GPU
 */
interface GPUPipelineError extends DOMException {
  /**
   * The **`reason`** read-only property of the GPUPipelineError interface defines the reason the pipeline creation failed in a machine-readable way.
   *
   * [MDN Reference](https://developer.mozilla.org/docs/Web/API/GPUPipelineError/reason)
   */
  readonly reason: "validation" | "internal";
}

/** @category GPU */
declare var GPUPipelineError: {
  prototype: GPUPipelineError;
  new (message: string, options: GPUPipelineErrorInit): GPUPipelineError;
};

/** @category GPU */
interface GPUPipelineErrorInit {
  reason: "validation" | "internal";
}

/**
 * Represents a compiled shader module that can be used to create graphics or compute pipelines.
 *
 * @example
 * ```ts
 * // Create a shader module using WGSL (WebGPU Shading Language)
 * const shaderModule = device.createShaderModule({
 *   label: "My Shader",
 *   code: `
 *     @vertex
 *     fn vertexMain(@location(0) pos: vec2f) -> @builtin(position) vec4f {
 *       return vec4f(pos, 0.0, 1.0);
 *     }
 *
 *     @fragment
 *     fn fragmentMain() -> @location(0) vec4f {
 *       return vec4f(1.0, 0.0, 0.0, 1.0); // red color
 *     }
 *   `
 * });
 *
 * // Can optionally check for compilation errors/warnings
 * const compilationInfo = await shaderModule.getCompilationInfo();
 * for (const message of compilationInfo.messages) {
 *   console.log(`${message.type}: ${message.message} at ${message.lineNum}:${message.linePos}`);
 * }
 * ```
 *
 * @category GPU
 */
interface GPUShaderModule extends GPUObjectBase {
  /**
   * Returns compilation messages for this shader module,
   * which can include errors, warnings and info messages.
   */
  getCompilationInfo(): Promise<GPUCompilationInfo>;
}

/** @category GPU */
declare var GPUShaderModule: {
  prototype: GPUShaderModule;
  new (): GPUShaderModule;
};

/** @category GPU */
interface GPUShaderModuleDescriptor extends GPUObjectDescriptorBase {
  code: string;
  sourceMap?: any;
}

/** @category GPU */
type GPUAutoLayoutMode = "auto";

/** @category GPU */
interface GPUPipelineDescriptorBase extends GPUObjectDescriptorBase {
  layout: GPUPipelineLayout | GPUAutoLayoutMode;
}

/** @category GPU */
interface GPUPipelineBase {
  getBindGroupLayout(index: number): GPUBindGroupLayout;
}

/** @category GPU */
interface GPUProgrammableStage {
  module: GPUShaderModule;
  entryPoint?: string;
  constants?: Record<string, number>;
}

/** @category GPU */
interface GPUComputePipeline extends GPUObjectBase, GPUPipelineBase {
}

/** @category GPU */
declare var GPUComputePipeline: {
  prototype: GPUComputePipeline;
  new (): GPUComputePipeline;
};

/** @category GPU */
interface GPUComputePipelineDescriptor extends GPUPipelineDescriptorBase {
  compute: GPUProgrammableStage;
}

/** @category GPU */
interface GPURenderPipeline extends GPUObjectBase, GPUPipelineBase {
}

/** @category GPU */
declare var GPURenderPipeline: {
  prototype: GPURenderPipeline;
  new (): GPURenderPipeline;
};

/** @category GPU */
interface GPURenderPipelineDescriptor extends GPUPipelineDescriptorBase {
  vertex: GPUVertexState;
  primitive?: GPUPrimitiveState;
  depthStencil?: GPUDepthStencilState;
  multisample?: GPUMultisampleState;
  fragment?: GPUFragmentState;
}

/** @category GPU */
type GPUPrimitiveTopology =
  | "point-list"
  | "line-list"
  | "line-strip"
  | "triangle-list"
  | "triangle-strip";

/** @category GPU */
type GPUFrontFace = "ccw" | "cw";

/** @category GPU */
type GPUCullMode = "none" | "front" | "back";

/** @category GPU */
interface GPUPrimitiveState {
  topology?: GPUPrimitiveTopology;
  stripIndexFormat?: GPUIndexFormat;
  frontFace?: GPUFrontFace;
  cullMode?: GPUCullMode;
  unclippedDepth?: boolean;
}

/** @category GPU */
interface GPUMultisampleState {
  count?: number;
  mask?: number;
  alphaToCoverageEnabled?: boolean;
}

/** @category GPU */
interface GPUFragmentState extends GPUProgrammableStage {
  targets: (GPUColorTargetState | null)[];
}

/** @category GPU */
interface GPUColorTargetState {
  format: GPUTextureFormat;

  blend?: GPUBlendState;
  writeMask?: GPUColorWriteFlags;
}

/** @category GPU */
interface GPUBlendState {
  color: GPUBlendComponent;
  alpha: GPUBlendComponent;
}

/** @category GPU */
type GPUColorWriteFlags = number;

/** @category GPU */
declare class GPUColorWrite {
  static RED: 0x1;
  static GREEN: 0x2;
  static BLUE: 0x4;
  static ALPHA: 0x8;
  static ALL: 0xF;
}

/** @category GPU */
interface GPUBlendComponent {
  operation?: GPUBlendOperation;
  srcFactor?: GPUBlendFactor;
  dstFactor?: GPUBlendFactor;
}

/** @category GPU */
type GPUBlendFactor =
  | "zero"
  | "one"
  | "src"
  | "one-minus-src"
  | "src-alpha"
  | "one-minus-src-alpha"
  | "dst"
  | "one-minus-dst"
  | "dst-alpha"
  | "one-minus-dst-alpha"
  | "src-alpha-saturated"
  | "constant"
  | "one-minus-constant"
  | "src1"
  | "one-minus-src1"
  | "src1-alpha"
  | "one-minus-src1-alpha";

/** @category GPU */
type GPUBlendOperation =
  | "add"
  | "subtract"
  | "reverse-subtract"
  | "min"
  | "max";

/** @category GPU */
interface GPUDepthStencilState {
  format: GPUTextureFormat;

  depthWriteEnabled?: boolean;
  depthCompare?: GPUCompareFunction;

  stencilFront?: GPUStencilFaceState;
  stencilBack?: GPUStencilFaceState;

  stencilReadMask?: number;
  stencilWriteMask?: number;

  depthBias?: number;
  depthBiasSlopeScale?: number;
  depthBiasClamp?: number;
}

/** @category GPU */
interface GPUStencilFaceState {
  compare?: GPUCompareFunction;
  failOp?: GPUStencilOperation;
  depthFailOp?: GPUStencilOperation;
  passOp?: GPUStencilOperation;
}

/** @category GPU */
type GPUStencilOperation =
  | "keep"
  | "zero"
  | "replace"
  | "invert"
  | "increment-clamp"
  | "decrement-clamp"
  | "increment-wrap"
  | "decrement-wrap";

/** @category GPU */
type GPUIndexFormat = "uint16" | "uint32";

/** @category GPU */
type GPUVertexFormat =
  | "uint8x2"
  | "uint8x4"
  | "sint8x2"
  | "sint8x4"
  | "unorm8x2"
  | "unorm8x4"
  | "snorm8x2"
  | "snorm8x4"
  | "uint16x2"
  | "uint16x4"
  | "sint16x2"
  | "sint16x4"
  | "unorm16x2"
  | "unorm16x4"
  | "snorm16x2"
  | "snorm16x4"
  | "float16x2"
  | "float16x4"
  | "float32"
  | "float32x2"
  | "float32x3"
  | "float32x4"
  | "uint32"
  | "uint32x2"
  | "uint32x3"
  | "uint32x4"
  | "sint32"
  | "sint32x2"
  | "sint32x3"
  | "sint32x4"
  | "unorm10-10-10-2";

/** @category GPU */
type GPUVertexStepMode = "vertex" | "instance";

/** @category GPU */
interface GPUVertexState extends GPUProgrammableStage {
  buffers?: (GPUVertexBufferLayout | null)[];
}

/** @category GPU */
interface GPUVertexBufferLayout {
  arrayStride: number;
  stepMode?: GPUVertexStepMode;
  attributes: GPUVertexAttribute[];
}

/** @category GPU */
interface GPUVertexAttribute {
  format: GPUVertexFormat;
  offset: number;

  shaderLocation: number;
}

/** @category GPU */
interface GPUTexelCopyBufferLayout {
  offset?: number;
  bytesPerRow?: number;
  rowsPerImage?: number;
}

/** @category GPU */
interface GPUCommandBuffer extends GPUObjectBase {
}

/** @category GPU */
declare var GPUCommandBuffer: {
  prototype: GPUCommandBuffer;
  new (): GPUCommandBuffer;
};

/** @category GPU */
interface GPUCommandBufferDescriptor extends GPUObjectDescriptorBase {}

/**
 * Used to record GPU commands for later execution by the GPU.
 *
 * @example
 * ```ts
 * // Create a command encoder
 * const commandEncoder = device.createCommandEncoder({
 *   label: "Main Command Encoder"
 * });
 *
 * // Record a copy from one buffer to another
 * commandEncoder.copyBufferToBuffer(
 *   sourceBuffer, 0, // Source buffer and offset
 *   destinationBuffer, 0, // Destination buffer and offset
 *   sourceBuffer.size // Size to copy
 * );
 *
 * // Begin a compute pass to execute a compute shader
 * const computePass = commandEncoder.beginComputePass();
 * computePass.setPipeline(computePipeline);
 * computePass.setBindGroup(0, bindGroup);
 * computePass.dispatchWorkgroups(32, 1, 1); // Run 32 workgroups
 * computePass.end();
 *
 * // Begin a render pass to draw to a texture
 * const renderPass = commandEncoder.beginRenderPass({
 *   colorAttachments: [{
 *     view: textureView,
 *     clearValue: { r: 0.0, g: 0.0, b: 0.0, a: 1.0 },
 *     loadOp: "clear",
 *     storeOp: "store"
 *   }]
 * });
 * renderPass.setPipeline(renderPipeline);
 * renderPass.draw(3, 1, 0, 0); // Draw a triangle
 * renderPass.end();
 *
 * // Finish encoding and submit to GPU
 * const commandBuffer = commandEncoder.finish();
 * device.queue.submit([commandBuffer]);
 * ```
 *
 * @category GPU
 */
interface GPUCommandEncoder extends GPUObjectBase {
  beginRenderPass(descriptor: GPURenderPassDescriptor): GPURenderPassEncoder;
  beginComputePass(
    descriptor?: GPUComputePassDescriptor,
  ): GPUComputePassEncoder;

  copyBufferToBuffer(
    source: GPUBuffer,
    sourceOffset: number,
    destination: GPUBuffer,
    destinationOffset: number,
    size: number,
  ): void;

  copyBufferToTexture(
    source: GPUTexelCopyBufferInfo,
    destination: GPUTexelCopyTextureInfo,
    copySize: GPUExtent3D,
  ): void;

  copyTextureToBuffer(
    source: GPUTexelCopyTextureInfo,
    destination: GPUTexelCopyBufferInfo,
    copySize: GPUExtent3D,
  ): void;

  copyTextureToTexture(
    source: GPUTexelCopyTextureInfo,
    destination: GPUTexelCopyTextureInfo,
    copySize: GPUExtent3D,
  ): void;

  clearBuffer(
    destination: GPUBuffer,
    destinationOffset?: number,
    size?: number,
  ): void;

  pushDebugGroup(groupLabel: string): void;
  popDebugGroup(): void;
  insertDebugMarker(markerLabel: string): void;

  writeTimestamp(querySet: GPUQuerySet, queryIndex: number): void;

  resolveQuerySet(
    querySet: GPUQuerySet,
    firstQuery: number,
    queryCount: number,
    destination: GPUBuffer,
    destinationOffset: number,
  ): void;

  finish(descriptor?: GPUCommandBufferDescriptor): GPUCommandBuffer;
}

/** @category GPU */
declare var GPUCommandEncoder: {
  prototype: GPUCommandEncoder;
  new (): GPUCommandEncoder;
};

/** @category GPU */
interface GPUCommandEncoderDescriptor extends GPUObjectDescriptorBase {}

/** @category GPU */
interface GPUTexelCopyBufferInfo extends GPUTexelCopyBufferLayout {
  buffer: GPUBuffer;
}

/** @category GPU */
interface GPUTexelCopyTextureInfo {
  texture: GPUTexture;
  mipLevel?: number;
  origin?: GPUOrigin3D;
  aspect?: GPUTextureAspect;
}

/** @category GPU */
interface GPUProgrammablePassEncoder {
  setBindGroup(
    index: number,
    bindGroup: GPUBindGroup | null,
    dynamicOffsets?: number[],
  ): void;

  setBindGroup(
    index: number,
    bindGroup: GPUBindGroup | null,
    dynamicOffsetsData: Uint32Array,
    dynamicOffsetsDataStart: number,
    dynamicOffsetsDataLength: number,
  ): void;

  pushDebugGroup(groupLabel: string): void;
  popDebugGroup(): void;
  insertDebugMarker(markerLabel: string): void;
}

/** @category GPU */
interface GPUComputePassEncoder
  extends GPUObjectBase, GPUProgrammablePassEncoder {
  setPipeline(pipeline: GPUComputePipeline): void;
  dispatchWorkgroups(x: number, y?: number, z?: number): void;
  dispatchWorkgroupsIndirect(
    indirectBuffer: GPUBuffer,
    indirectOffset: number,
  ): void;

  end(): void;
}

/** @category GPU */
declare var GPUComputePassEncoder: {
  prototype: GPUComputePassEncoder;
  new (): GPUComputePassEncoder;
};

/** @category GPU */
interface GPUComputePassTimestampWrites {
  querySet: GPUQuerySet;
  beginningOfPassWriteIndex?: number;
  endOfPassWriteIndex?: number;
}

/** @category GPU */
interface GPUComputePassDescriptor extends GPUObjectDescriptorBase {
  timestampWrites?: GPUComputePassTimestampWrites;
}

/** @category GPU */
interface GPURenderEncoderBase {
  setPipeline(pipeline: GPURenderPipeline): void;

  setIndexBuffer(
    buffer: GPUBuffer,
    indexFormat: GPUIndexFormat,
    offset?: number,
    size?: number,
  ): void;
  setVertexBuffer(
    slot: number,
    buffer: GPUBuffer,
    offset?: number,
    size?: number,
  ): void;

  draw(
    vertexCount: number,
    instanceCount?: number,
    firstVertex?: number,
    firstInstance?: number,
  ): void;
  drawIndexed(
    indexCount: number,
    instanceCount?: number,
    firstIndex?: number,
    baseVertex?: number,
    firstInstance?: number,
  ): void;

  drawIndirect(indirectBuffer: GPUBuffer, indirectOffset: number): void;
  drawIndexedIndirect(
    indirectBuffer: GPUBuffer,
    indirectOffset: number,
  ): void;
}

/** @category GPU */
interface GPURenderPassEncoder
  extends GPUObjectBase, GPUProgrammablePassEncoder, GPURenderEncoderBase {
  setViewport(
    x: number,
    y: number,
    width: number,
    height: number,
    minDepth: number,
    maxDepth: number,
  ): void;

  setScissorRect(
    x: number,
    y: number,
    width: number,
    height: number,
  ): void;

  setBlendConstant(color: GPUColor): void;
  setStencilReference(reference: number): void;

  beginOcclusionQuery(queryIndex: number): void;
  endOcclusionQuery(): void;

  executeBundles(bundles: GPURenderBundle[]): void;
  end(): void;
}

/** @category GPU */
declare var GPURenderPassEncoder: {
  prototype: GPURenderPassEncoder;
  new (): GPURenderPassEncoder;
};

/** @category GPU */
interface GPURenderPassTimestampWrites {
  querySet: GPUQuerySet;
  beginningOfPassWriteIndex?: number;
  endOfPassWriteIndex?: number;
}

/** @category GPU */
interface GPURenderPassDescriptor extends GPUObjectDescriptorBase {
  colorAttachments: (GPURenderPassColorAttachment | null)[];
  depthStencilAttachment?: GPURenderPassDepthStencilAttachment;
  occlusionQuerySet?: GPUQuerySet;
  timestampWrites?: GPURenderPassTimestampWrites;
}

/** @category GPU */
type GPULoadOp = "load" | "clear";

/** @category GPU */
type GPUStoreOp = "store" | "discard";

/** @category GPU */
interface GPURenderPassColorAttachment {
  view: GPUTexture | GPUTextureView;
  resolveTarget?: GPUTexture | GPUTextureView;

  clearValue?: GPUColor;
  loadOp: GPULoadOp;
  storeOp: GPUStoreOp;
}

/** @category GPU */
interface GPURenderPassDepthStencilAttachment {
  view: GPUTexture | GPUTextureView;

  depthClearValue?: number;
  depthLoadOp?: GPULoadOp;
  depthStoreOp?: GPUStoreOp;
  depthReadOnly?: boolean;

  stencilClearValue?: number;
  stencilLoadOp?: GPULoadOp;
  stencilStoreOp?: GPUStoreOp;
  stencilReadOnly?: boolean;
}

/** @category GPU */
interface GPURenderBundle extends GPUObjectBase {
}

/** @category GPU */
declare var GPURenderBundle: {
  prototype: GPURenderBundle;
  new (): GPURenderBundle;
};

/** @category GPU */
interface GPURenderBundleDescriptor extends GPUObjectDescriptorBase {}

/** @category GPU */
interface GPURenderBundleEncoder
  extends GPUObjectBase, GPUProgrammablePassEncoder, GPURenderEncoderBase {
  finish(descriptor?: GPURenderBundleDescriptor): GPURenderBundle;
}

/** @category GPU */
declare var GPURenderBundleEncoder: {
  prototype: GPURenderBundleEncoder;
  new (): GPURenderBundleEncoder;
};

/** @category GPU */
interface GPURenderPassLayout extends GPUObjectDescriptorBase {
  colorFormats: (GPUTextureFormat | null)[];
  depthStencilFormat?: GPUTextureFormat;
  sampleCount?: number;
}

/** @category GPU */
interface GPURenderBundleEncoderDescriptor extends GPURenderPassLayout {
  depthReadOnly?: boolean;
  stencilReadOnly?: boolean;
}

/**
 * Represents a queue to submit commands to the GPU.
 *
 * @example
 * ```ts
 * // Get a queue from the device (each device has a default queue)
 * const queue = device.queue;
 *
 * // Write data to a buffer
 * const buffer = device.createBuffer({
 *   size: data.byteLength,
 *   usage: GPUBufferUsage.COPY_DST | GPUBufferUsage.STORAGE
 * });
 * queue.writeBuffer(buffer, 0, data);
 *
 * // Submit command buffers to the GPU for execution
 * const commandBuffer = commandEncoder.finish();
 * queue.submit([commandBuffer]);
 *
 * // Wait for all submitted operations to complete
 * await queue.onSubmittedWorkDone();
 *
 * // Example: Write data to a texture
 * const texture = device.createTexture({
 *   size: { width: 256, height: 256 },
 *   format: "rgba8unorm",
 *   usage: GPUTextureUsage.TEXTURE_BINDING | GPUTextureUsage.COPY_DST
 * });
 *
 * const data = new Uint8Array(256 * 256 * 4); // RGBA data
 * // Fill data with your texture content...
 *
 * queue.writeTexture(
 *   { texture },
 *   data,
 *   { bytesPerRow: 256 * 4 },
 *   { width: 256, height: 256 }
 * );
 * ```
 *
 * @category GPU
 */
interface GPUQueue extends GPUObjectBase {
  submit(commandBuffers: GPUCommandBuffer[]): void;

  onSubmittedWorkDone(): Promise<void>;

  writeBuffer(
    buffer: GPUBuffer,
    bufferOffset: number,
    data: BufferSource,
    dataOffset?: number,
    size?: number,
  ): void;

  writeTexture(
    destination: GPUTexelCopyTextureInfo,
    data: BufferSource,
    dataLayout: GPUTexelCopyBufferLayout,
    size: GPUExtent3D,
  ): void;
}

/** @category GPU */
declare var GPUQueue: {
  prototype: GPUQueue;
  new (): GPUQueue;
};

/** @category GPU */
interface GPUQuerySet extends GPUObjectBase {
  destroy(): void;

  readonly type: GPUQueryType;
  readonly count: number;
}

/** @category GPU */
declare var GPUQuerySet: {
  prototype: GPUQuerySet;
  new (): GPUQuerySet;
};

/** @category GPU */
interface GPUQuerySetDescriptor extends GPUObjectDescriptorBase {
  type: GPUQueryType;
  count: number;
}

/** @category GPU */
type GPUQueryType = "occlusion" | "timestamp";

/** @category GPU */
type GPUDeviceLostReason = "destroyed";

/** @category GPU */
interface GPUDeviceLostInfo {
  readonly reason: GPUDeviceLostReason;
  readonly message: string;
}

/**
 * The **`GPUError`** interface of the WebGPU API is the base interface for errors surfaced by GPUDevice.popErrorScope and the GPUDevice.uncapturederror_event event.
 * Available only in secure contexts.
 *
 * [MDN Reference](https://developer.mozilla.org/docs/Web/API/GPUError)
 * @category GPU
 */
interface GPUError {
  /**
   * The **`message`** read-only property of the A string.
   * The **`message`** read-only property of the GPUError interface provides a human-readable message that explains why the error occurred.
   *
   * [MDN Reference](https://developer.mozilla.org/docs/Web/API/GPUError/message)
   */
  readonly message: string;
}

/** @category GPU */
declare var GPUError: {
  prototype: GPUError;
  new (): GPUError;
};

/** @category GPU */
interface GPUOutOfMemoryError extends GPUError {}

/** @category GPU */
declare var GPUOutOfMemoryError: {
  prototype: GPUOutOfMemoryError;
  new (message: string): GPUOutOfMemoryError;
};

/** @category GPU */
interface GPUValidationError extends GPUError {}

/** @category GPU */
declare var GPUValidationError: {
  prototype: GPUValidationError;
  new (message: string): GPUValidationError;
};

/** @category GPU */
interface GPUInternalError extends GPUError {}

/** @category GPU */
declare var GPUInternalError: {
  prototype: GPUInternalError;
  new (message: string): GPUInternalError;
};

/** @category GPU */
type GPUErrorFilter = "out-of-memory" | "validation" | "internal";

/** @category GPU */
interface GPUUncapturedErrorEvent extends Event {
  readonly error: GPUError;
}

/** @category GPU */
declare var GPUUncapturedErrorEvent: {
  prototype: GPUUncapturedErrorEvent;
  new (
    type: string,
    gpuUncapturedErrorEventInitDict: GPUUncapturedErrorEventInit,
  ): GPUUncapturedErrorEvent;
};

/** @category GPU */
interface GPUUncapturedErrorEventInit extends EventInit {
  error: GPUError;
}

/** @category GPU */
interface GPUColorDict {
  r: number;
  g: number;
  b: number;
  a: number;
}

/** @category GPU */
type GPUColor = number[] | GPUColorDict;

/** @category GPU */
interface GPUOrigin3DDict {
  x?: number;
  y?: number;
  z?: number;
}

/** @category GPU */
type GPUOrigin3D = number[] | GPUOrigin3DDict;

/** @category GPU */
interface GPUExtent3DDict {
  width: number;
  height?: number;
  depthOrArrayLayers?: number;
}

/** @category GPU */
type GPUExtent3D = number[] | GPUExtent3DDict;

/** @category GPU */
type GPUCanvasAlphaMode = "opaque" | "premultiplied";

/** @category GPU */
interface GPUCanvasConfiguration {
  device: GPUDevice;
  format: GPUTextureFormat;
  usage?: GPUTextureUsageFlags;
  viewFormats?: GPUTextureFormat[];
  colorSpace?: "srgb" | "display-p3";
  alphaMode?: GPUCanvasAlphaMode;
}

/** @category GPU */
interface GPUCanvasContext {
  configure(configuration: GPUCanvasConfiguration): void;
  unconfigure(): void;
  getCurrentTexture(): GPUTexture;
}
