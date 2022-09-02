// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

// @ts-check
/// <reference path="../web/internal.d.ts" />

"use strict";

((window) => {
  const webidl = window.__bootstrap.webidl;
  const {
    GPU,
    GPUAdapter,
    GPUSupportedLimits,
    GPUSupportedFeatures,
    GPUDevice,
    GPUQueue,
    GPUBuffer,
    GPUBufferUsage,
    GPUMapMode,
    GPUTextureUsage,
    GPUTexture,
    GPUTextureView,
    GPUSampler,
    GPUBindGroupLayout,
    GPUPipelineLayout,
    GPUBindGroup,
    GPUShaderModule,
    GPUShaderStage,
    GPUComputePipeline,
    GPURenderPipeline,
    GPUColorWrite,
    GPUCommandEncoder,
    GPURenderPassEncoder,
    GPUComputePassEncoder,
    GPUCommandBuffer,
    GPURenderBundleEncoder,
    GPURenderBundle,
    GPUQuerySet,
    GPUOutOfMemoryError,
    GPUValidationError,
  } = window.__bootstrap.webgpu;
  const { SymbolIterator, TypeError } = window.__bootstrap.primordials;

  // This needs to be initialized after all of the base classes are implemented,
  // otherwise their converters might not be available yet.
  // DICTIONARY: GPUObjectDescriptorBase
  const dictMembersGPUObjectDescriptorBase = [
    { key: "label", converter: webidl.converters["USVString"] },
  ];
  webidl.converters["GPUObjectDescriptorBase"] = webidl
    .createDictionaryConverter(
      "GPUObjectDescriptorBase",
      dictMembersGPUObjectDescriptorBase,
    );

  // INTERFACE: GPUSupportedLimits
  webidl.converters.GPUSupportedLimits = webidl.createInterfaceConverter(
    "GPUSupportedLimits",
    GPUSupportedLimits.prototype,
  );

  // INTERFACE: GPUSupportedFeatures
  webidl.converters.GPUSupportedFeatures = webidl.createInterfaceConverter(
    "GPUSupportedFeatures",
    GPUSupportedFeatures.prototype,
  );

  // ENUM: GPUPredefinedColorSpace
  webidl.converters.GPUPredefinedColorSpace = webidl.createEnumConverter(
    "GPUPredefinedColorSpace",
    ["srgb"],
  );

  // INTERFACE: GPU
  webidl.converters.GPU = webidl.createInterfaceConverter("GPU", GPU.prototype);

  // ENUM: GPUPowerPreference
  webidl.converters["GPUPowerPreference"] = webidl.createEnumConverter(
    "GPUPowerPreference",
    [
      "low-power",
      "high-performance",
    ],
  );

  // DICTIONARY: GPURequestAdapterOptions
  const dictMembersGPURequestAdapterOptions = [
    {
      key: "powerPreference",
      converter: webidl.converters["GPUPowerPreference"],
    },
    {
      key: "forceFallbackAdapter",
      converter: webidl.converters.boolean,
      defaultValue: false,
    },
  ];
  webidl.converters["GPURequestAdapterOptions"] = webidl
    .createDictionaryConverter(
      "GPURequestAdapterOptions",
      dictMembersGPURequestAdapterOptions,
    );

  // INTERFACE: GPUAdapter
  webidl.converters.GPUAdapter = webidl.createInterfaceConverter(
    "GPUAdapter",
    GPUAdapter.prototype,
  );

  // ENUM: GPUFeatureName
  webidl.converters["GPUFeatureName"] = webidl.createEnumConverter(
    "GPUFeatureName",
    [
      "depth-clip-control",
      "depth24unorm-stencil8",
      "depth32float-stencil8",
      "pipeline-statistics-query",
      "texture-compression-bc",
      "texture-compression-etc2",
      "texture-compression-astc",
      "timestamp-query",
      "indirect-first-instance",
      "shader-f16",
      // extended from spec
      "mappable-primary-buffers",
      "texture-binding-array",
      "buffer-binding-array",
      "storage-resource-binding-array",
      "sampled-texture-and-storage-buffer-array-non-uniform-indexing",
      "uniform-buffer-and-storage-buffer-texture-non-uniform-indexing",
      "unsized-binding-array",
      "multi-draw-indirect",
      "multi-draw-indirect-count",
      "push-constants",
      "address-mode-clamp-to-border",
      "texture-adapter-specific-format-features",
      "shader-float64",
      "vertex-attribute-64bit",
      "conservative-rasterization",
      "vertex-writable-storage",
      "clear-commands",
      "spirv-shader-passthrough",
      "shader-primitive-index",
    ],
  );

  // TYPEDEF: GPUSize32
  webidl.converters["GPUSize32"] = (V, opts) =>
    webidl.converters["unsigned long"](V, { ...opts, enforceRange: true });

  // DICTIONARY: GPUDeviceDescriptor
  const dictMembersGPUDeviceDescriptor = [
    {
      key: "requiredFeatures",
      converter: webidl.createSequenceConverter(
        webidl.converters["GPUFeatureName"],
      ),
      get defaultValue() {
        return [];
      },
    },
    {
      key: "requiredLimits",
      converter: webidl.createRecordConverter(
        webidl.converters["DOMString"],
        webidl.converters["GPUSize32"],
      ),
    },
  ];
  webidl.converters["GPUDeviceDescriptor"] = webidl.createDictionaryConverter(
    "GPUDeviceDescriptor",
    dictMembersGPUObjectDescriptorBase,
    dictMembersGPUDeviceDescriptor,
  );

  // INTERFACE: GPUDevice
  webidl.converters.GPUDevice = webidl.createInterfaceConverter(
    "GPUDevice",
    GPUDevice.prototype,
  );

  // INTERFACE: GPUBuffer
  webidl.converters.GPUBuffer = webidl.createInterfaceConverter(
    "GPUBuffer",
    GPUBuffer.prototype,
  );

  // TYPEDEF: GPUSize64
  webidl.converters["GPUSize64"] = (V, opts) =>
    webidl.converters["unsigned long long"](V, { ...opts, enforceRange: true });

  // TYPEDEF: GPUBufferUsageFlags
  webidl.converters["GPUBufferUsageFlags"] = (V, opts) =>
    webidl.converters["unsigned long"](V, { ...opts, enforceRange: true });

  // DICTIONARY: GPUBufferDescriptor
  const dictMembersGPUBufferDescriptor = [
    { key: "size", converter: webidl.converters["GPUSize64"], required: true },
    {
      key: "usage",
      converter: webidl.converters["GPUBufferUsageFlags"],
      required: true,
    },
    {
      key: "mappedAtCreation",
      converter: webidl.converters["boolean"],
      defaultValue: false,
    },
  ];
  webidl.converters["GPUBufferDescriptor"] = webidl.createDictionaryConverter(
    "GPUBufferDescriptor",
    dictMembersGPUObjectDescriptorBase,
    dictMembersGPUBufferDescriptor,
  );

  // INTERFACE: GPUBufferUsage
  webidl.converters.GPUBufferUsage = webidl.createInterfaceConverter(
    "GPUBufferUsage",
    GPUBufferUsage.prototype,
  );

  // TYPEDEF: GPUMapModeFlags
  webidl.converters["GPUMapModeFlags"] = (V, opts) =>
    webidl.converters["unsigned long"](V, { ...opts, enforceRange: true });

  // INTERFACE: GPUMapMode
  webidl.converters.GPUMapMode = webidl.createInterfaceConverter(
    "GPUMapMode",
    GPUMapMode.prototype,
  );

  // INTERFACE: GPUTexture
  webidl.converters.GPUTexture = webidl.createInterfaceConverter(
    "GPUTexture",
    GPUTexture.prototype,
  );

  // TYPEDEF: GPUIntegerCoordinate
  webidl.converters["GPUIntegerCoordinate"] = (V, opts) =>
    webidl.converters["unsigned long"](V, { ...opts, enforceRange: true });
  webidl.converters["sequence<GPUIntegerCoordinate>"] = webidl
    .createSequenceConverter(webidl.converters["GPUIntegerCoordinate"]);

  // DICTIONARY: GPUExtent3DDict
  const dictMembersGPUExtent3DDict = [
    {
      key: "width",
      converter: webidl.converters["GPUIntegerCoordinate"],
      required: true,
    },
    {
      key: "height",
      converter: webidl.converters["GPUIntegerCoordinate"],
      defaultValue: 1,
    },
    {
      key: "depthOrArrayLayers",
      converter: webidl.converters["GPUIntegerCoordinate"],
      defaultValue: 1,
    },
  ];
  webidl.converters["GPUExtent3DDict"] = webidl.createDictionaryConverter(
    "GPUExtent3DDict",
    dictMembersGPUExtent3DDict,
  );

  // TYPEDEF: GPUExtent3D
  webidl.converters["GPUExtent3D"] = (V, opts) => {
    // Union for (sequence<GPUIntegerCoordinate> or GPUExtent3DDict)
    if (V === null || V === undefined) {
      return webidl.converters["GPUExtent3DDict"](V, opts);
    }
    if (typeof V === "object") {
      const method = V[SymbolIterator];
      if (method !== undefined) {
        return webidl.converters["sequence<GPUIntegerCoordinate>"](V, opts);
      }
      return webidl.converters["GPUExtent3DDict"](V, opts);
    }
    throw webidl.makeException(
      TypeError,
      "can not be converted to sequence<GPUIntegerCoordinate> or GPUExtent3DDict.",
      opts,
    );
  };

  // ENUM: GPUTextureDimension
  webidl.converters["GPUTextureDimension"] = webidl.createEnumConverter(
    "GPUTextureDimension",
    [
      "1d",
      "2d",
      "3d",
    ],
  );

  // ENUM: GPUTextureFormat
  webidl.converters["GPUTextureFormat"] = webidl.createEnumConverter(
    "GPUTextureFormat",
    [
      "r8unorm",
      "r8snorm",
      "r8uint",
      "r8sint",
      "r16uint",
      "r16sint",
      "r16float",
      "rg8unorm",
      "rg8snorm",
      "rg8uint",
      "rg8sint",
      "r32uint",
      "r32sint",
      "r32float",
      "rg16uint",
      "rg16sint",
      "rg16float",
      "rgba8unorm",
      "rgba8unorm-srgb",
      "rgba8snorm",
      "rgba8uint",
      "rgba8sint",
      "bgra8unorm",
      "bgra8unorm-srgb",
      "rgb9e5ufloat",
      "rgb10a2unorm",
      "rg11b10ufloat",
      "rg32uint",
      "rg32sint",
      "rg32float",
      "rgba16uint",
      "rgba16sint",
      "rgba16float",
      "rgba32uint",
      "rgba32sint",
      "rgba32float",
      "stencil8",
      "depth16unorm",
      "depth24plus",
      "depth24plus-stencil8",
      "depth32float",
      "depth24unorm-stencil8",
      "depth32float-stencil8",
      "bc1-rgba-unorm",
      "bc1-rgba-unorm-srgb",
      "bc2-rgba-unorm",
      "bc2-rgba-unorm-srgb",
      "bc3-rgba-unorm",
      "bc3-rgba-unorm-srgb",
      "bc4-r-unorm",
      "bc4-r-snorm",
      "bc5-rg-unorm",
      "bc5-rg-snorm",
      "bc6h-rgb-ufloat",
      "bc6h-rgb-float",
      "bc7-rgba-unorm",
      "bc7-rgba-unorm-srgb",
      "etc2-rgb8unorm",
      "etc2-rgb8unorm-srgb",
      "etc2-rgb8a1unorm",
      "etc2-rgb8a1unorm-srgb",
      "etc2-rgba8unorm",
      "etc2-rgba8unorm-srgb",
      "eac-r11unorm",
      "eac-r11snorm",
      "eac-rg11unorm",
      "eac-rg11snorm",
      "astc-4x4-unorm",
      "astc-4x4-unorm-srgb",
      "astc-5x4-unorm",
      "astc-5x4-unorm-srgb",
      "astc-5x5-unorm",
      "astc-5x5-unorm-srgb",
      "astc-6x5-unorm",
      "astc-6x5-unorm-srgb",
      "astc-6x6-unorm",
      "astc-6x6-unorm-srgb",
      "astc-8x5-unorm",
      "astc-8x5-unorm-srgb",
      "astc-8x6-unorm",
      "astc-8x6-unorm-srgb",
      "astc-8x8-unorm",
      "astc-8x8-unorm-srgb",
      "astc-10x5-unorm",
      "astc-10x5-unorm-srgb",
      "astc-10x6-unorm",
      "astc-10x6-unorm-srgb",
      "astc-10x8-unorm",
      "astc-10x8-unorm-srgb",
      "astc-10x10-unorm",
      "astc-10x10-unorm-srgb",
      "astc-12x10-unorm",
      "astc-12x10-unorm-srgb",
      "astc-12x12-unorm",
      "astc-12x12-unorm-srgb",
    ],
  );

  // TYPEDEF: GPUTextureUsageFlags
  webidl.converters["GPUTextureUsageFlags"] = (V, opts) =>
    webidl.converters["unsigned long"](V, { ...opts, enforceRange: true });

  // DICTIONARY: GPUTextureDescriptor
  const dictMembersGPUTextureDescriptor = [
    {
      key: "size",
      converter: webidl.converters["GPUExtent3D"],
      required: true,
    },
    {
      key: "mipLevelCount",
      converter: webidl.converters["GPUIntegerCoordinate"],
      defaultValue: 1,
    },
    {
      key: "sampleCount",
      converter: webidl.converters["GPUSize32"],
      defaultValue: 1,
    },
    {
      key: "dimension",
      converter: webidl.converters["GPUTextureDimension"],
      defaultValue: "2d",
    },
    {
      key: "format",
      converter: webidl.converters["GPUTextureFormat"],
      required: true,
    },
    {
      key: "usage",
      converter: webidl.converters["GPUTextureUsageFlags"],
      required: true,
    },
  ];
  webidl.converters["GPUTextureDescriptor"] = webidl.createDictionaryConverter(
    "GPUTextureDescriptor",
    dictMembersGPUObjectDescriptorBase,
    dictMembersGPUTextureDescriptor,
  );

  // INTERFACE: GPUTextureUsage
  webidl.converters.GPUTextureUsage = webidl.createInterfaceConverter(
    "GPUTextureUsage",
    GPUTextureUsage.prototype,
  );

  // INTERFACE: GPUTextureView
  webidl.converters.GPUTextureView = webidl.createInterfaceConverter(
    "GPUTextureView",
    GPUTextureView.prototype,
  );

  // ENUM: GPUTextureViewDimension
  webidl.converters["GPUTextureViewDimension"] = webidl.createEnumConverter(
    "GPUTextureViewDimension",
    [
      "1d",
      "2d",
      "2d-array",
      "cube",
      "cube-array",
      "3d",
    ],
  );

  // ENUM: GPUTextureAspect
  webidl.converters["GPUTextureAspect"] = webidl.createEnumConverter(
    "GPUTextureAspect",
    [
      "all",
      "stencil-only",
      "depth-only",
    ],
  );

  // DICTIONARY: GPUTextureViewDescriptor
  const dictMembersGPUTextureViewDescriptor = [
    { key: "format", converter: webidl.converters["GPUTextureFormat"] },
    {
      key: "dimension",
      converter: webidl.converters["GPUTextureViewDimension"],
    },
    {
      key: "aspect",
      converter: webidl.converters["GPUTextureAspect"],
      defaultValue: "all",
    },
    {
      key: "baseMipLevel",
      converter: webidl.converters["GPUIntegerCoordinate"],
      defaultValue: 0,
    },
    {
      key: "mipLevelCount",
      converter: webidl.converters["GPUIntegerCoordinate"],
    },
    {
      key: "baseArrayLayer",
      converter: webidl.converters["GPUIntegerCoordinate"],
      defaultValue: 0,
    },
    {
      key: "arrayLayerCount",
      converter: webidl.converters["GPUIntegerCoordinate"],
    },
  ];
  webidl.converters["GPUTextureViewDescriptor"] = webidl
    .createDictionaryConverter(
      "GPUTextureViewDescriptor",
      dictMembersGPUObjectDescriptorBase,
      dictMembersGPUTextureViewDescriptor,
    );

  // INTERFACE: GPUSampler
  webidl.converters.GPUSampler = webidl.createInterfaceConverter(
    "GPUSampler",
    GPUSampler.prototype,
  );

  // ENUM: GPUAddressMode
  webidl.converters["GPUAddressMode"] = webidl.createEnumConverter(
    "GPUAddressMode",
    [
      "clamp-to-edge",
      "repeat",
      "mirror-repeat",
    ],
  );

  // ENUM: GPUFilterMode
  webidl.converters["GPUFilterMode"] = webidl.createEnumConverter(
    "GPUFilterMode",
    [
      "nearest",
      "linear",
    ],
  );

  // ENUM: GPUMipmapFilterMode
  webidl.converters["GPUMipmapFilterMode"] = webidl.createEnumConverter(
    "GPUMipmapFilterMode",
    [
      "nearest",
      "linear",
    ],
  );

  // ENUM: GPUCompareFunction
  webidl.converters["GPUCompareFunction"] = webidl.createEnumConverter(
    "GPUCompareFunction",
    [
      "never",
      "less",
      "equal",
      "less-equal",
      "greater",
      "not-equal",
      "greater-equal",
      "always",
    ],
  );

  // DICTIONARY: GPUSamplerDescriptor
  const dictMembersGPUSamplerDescriptor = [
    {
      key: "addressModeU",
      converter: webidl.converters["GPUAddressMode"],
      defaultValue: "clamp-to-edge",
    },
    {
      key: "addressModeV",
      converter: webidl.converters["GPUAddressMode"],
      defaultValue: "clamp-to-edge",
    },
    {
      key: "addressModeW",
      converter: webidl.converters["GPUAddressMode"],
      defaultValue: "clamp-to-edge",
    },
    {
      key: "magFilter",
      converter: webidl.converters["GPUFilterMode"],
      defaultValue: "nearest",
    },
    {
      key: "minFilter",
      converter: webidl.converters["GPUFilterMode"],
      defaultValue: "nearest",
    },
    {
      key: "mipmapFilter",
      converter: webidl.converters["GPUMipmapFilterMode"],
      defaultValue: "nearest",
    },
    {
      key: "lodMinClamp",
      converter: webidl.converters["float"],
      defaultValue: 0,
    },
    {
      key: "lodMaxClamp",
      converter: webidl.converters["float"],
      defaultValue: 0xffffffff,
    },
    { key: "compare", converter: webidl.converters["GPUCompareFunction"] },
    {
      key: "maxAnisotropy",
      converter: (V, opts) =>
        webidl.converters["unsigned short"](V, { ...opts, clamp: true }),
      defaultValue: 1,
    },
  ];
  webidl.converters["GPUSamplerDescriptor"] = webidl.createDictionaryConverter(
    "GPUSamplerDescriptor",
    dictMembersGPUObjectDescriptorBase,
    dictMembersGPUSamplerDescriptor,
  );

  // INTERFACE: GPUBindGroupLayout
  webidl.converters.GPUBindGroupLayout = webidl.createInterfaceConverter(
    "GPUBindGroupLayout",
    GPUBindGroupLayout.prototype,
  );

  // TYPEDEF: GPUIndex32
  webidl.converters["GPUIndex32"] = (V, opts) =>
    webidl.converters["unsigned long"](V, { ...opts, enforceRange: true });

  // TYPEDEF: GPUShaderStageFlags
  webidl.converters["GPUShaderStageFlags"] = (V, opts) =>
    webidl.converters["unsigned long"](V, { ...opts, enforceRange: true });

  // ENUM: GPUBufferBindingType
  webidl.converters["GPUBufferBindingType"] = webidl.createEnumConverter(
    "GPUBufferBindingType",
    [
      "uniform",
      "storage",
      "read-only-storage",
    ],
  );

  // DICTIONARY: GPUBufferBindingLayout
  const dictMembersGPUBufferBindingLayout = [
    {
      key: "type",
      converter: webidl.converters["GPUBufferBindingType"],
      defaultValue: "uniform",
    },
    {
      key: "hasDynamicOffset",
      converter: webidl.converters["boolean"],
      defaultValue: false,
    },
    {
      key: "minBindingSize",
      converter: webidl.converters["GPUSize64"],
      defaultValue: 0,
    },
  ];
  webidl.converters["GPUBufferBindingLayout"] = webidl
    .createDictionaryConverter(
      "GPUBufferBindingLayout",
      dictMembersGPUBufferBindingLayout,
    );

  // ENUM: GPUSamplerBindingType
  webidl.converters["GPUSamplerBindingType"] = webidl.createEnumConverter(
    "GPUSamplerBindingType",
    [
      "filtering",
      "non-filtering",
      "comparison",
    ],
  );

  // DICTIONARY: GPUSamplerBindingLayout
  const dictMembersGPUSamplerBindingLayout = [
    {
      key: "type",
      converter: webidl.converters["GPUSamplerBindingType"],
      defaultValue: "filtering",
    },
  ];
  webidl.converters["GPUSamplerBindingLayout"] = webidl
    .createDictionaryConverter(
      "GPUSamplerBindingLayout",
      dictMembersGPUSamplerBindingLayout,
    );

  // ENUM: GPUTextureSampleType
  webidl.converters["GPUTextureSampleType"] = webidl.createEnumConverter(
    "GPUTextureSampleType",
    [
      "float",
      "unfilterable-float",
      "depth",
      "sint",
      "uint",
    ],
  );

  // DICTIONARY: GPUTextureBindingLayout
  const dictMembersGPUTextureBindingLayout = [
    {
      key: "sampleType",
      converter: webidl.converters["GPUTextureSampleType"],
      defaultValue: "float",
    },
    {
      key: "viewDimension",
      converter: webidl.converters["GPUTextureViewDimension"],
      defaultValue: "2d",
    },
    {
      key: "multisampled",
      converter: webidl.converters["boolean"],
      defaultValue: false,
    },
  ];
  webidl.converters["GPUTextureBindingLayout"] = webidl
    .createDictionaryConverter(
      "GPUTextureBindingLayout",
      dictMembersGPUTextureBindingLayout,
    );

  // ENUM: GPUStorageTextureAccess
  webidl.converters["GPUStorageTextureAccess"] = webidl.createEnumConverter(
    "GPUStorageTextureAccess",
    [
      "write-only",
    ],
  );

  // DICTIONARY: GPUStorageTextureBindingLayout
  const dictMembersGPUStorageTextureBindingLayout = [
    {
      key: "access",
      converter: webidl.converters["GPUStorageTextureAccess"],
      defaultValue: "write-only",
    },
    {
      key: "format",
      converter: webidl.converters["GPUTextureFormat"],
      required: true,
    },
    {
      key: "viewDimension",
      converter: webidl.converters["GPUTextureViewDimension"],
      defaultValue: "2d",
    },
  ];
  webidl.converters["GPUStorageTextureBindingLayout"] = webidl
    .createDictionaryConverter(
      "GPUStorageTextureBindingLayout",
      dictMembersGPUStorageTextureBindingLayout,
    );

  // DICTIONARY: GPUBindGroupLayoutEntry
  const dictMembersGPUBindGroupLayoutEntry = [
    {
      key: "binding",
      converter: webidl.converters["GPUIndex32"],
      required: true,
    },
    {
      key: "visibility",
      converter: webidl.converters["GPUShaderStageFlags"],
      required: true,
    },
    { key: "buffer", converter: webidl.converters["GPUBufferBindingLayout"] },
    { key: "sampler", converter: webidl.converters["GPUSamplerBindingLayout"] },
    { key: "texture", converter: webidl.converters["GPUTextureBindingLayout"] },
    {
      key: "storageTexture",
      converter: webidl.converters["GPUStorageTextureBindingLayout"],
    },
  ];
  webidl.converters["GPUBindGroupLayoutEntry"] = webidl
    .createDictionaryConverter(
      "GPUBindGroupLayoutEntry",
      dictMembersGPUBindGroupLayoutEntry,
    );

  // DICTIONARY: GPUBindGroupLayoutDescriptor
  const dictMembersGPUBindGroupLayoutDescriptor = [
    {
      key: "entries",
      converter: webidl.createSequenceConverter(
        webidl.converters["GPUBindGroupLayoutEntry"],
      ),
      required: true,
    },
  ];
  webidl.converters["GPUBindGroupLayoutDescriptor"] = webidl
    .createDictionaryConverter(
      "GPUBindGroupLayoutDescriptor",
      dictMembersGPUObjectDescriptorBase,
      dictMembersGPUBindGroupLayoutDescriptor,
    );

  // INTERFACE: GPUShaderStage
  webidl.converters.GPUShaderStage = webidl.createInterfaceConverter(
    "GPUShaderStage",
    GPUShaderStage.prototype,
  );

  // INTERFACE: GPUBindGroup
  webidl.converters.GPUBindGroup = webidl.createInterfaceConverter(
    "GPUBindGroup",
    GPUBindGroup.prototype,
  );

  // DICTIONARY: GPUBufferBinding
  const dictMembersGPUBufferBinding = [
    {
      key: "buffer",
      converter: webidl.converters["GPUBuffer"],
      required: true,
    },
    {
      key: "offset",
      converter: webidl.converters["GPUSize64"],
      defaultValue: 0,
    },
    { key: "size", converter: webidl.converters["GPUSize64"] },
  ];
  webidl.converters["GPUBufferBinding"] = webidl.createDictionaryConverter(
    "GPUBufferBinding",
    dictMembersGPUBufferBinding,
  );

  // TYPEDEF: GPUBindingResource
  webidl.converters["GPUBindingResource"] =
    webidl.converters.any /** put union here! **/;

  // DICTIONARY: GPUBindGroupEntry
  const dictMembersGPUBindGroupEntry = [
    {
      key: "binding",
      converter: webidl.converters["GPUIndex32"],
      required: true,
    },
    {
      key: "resource",
      converter: webidl.converters["GPUBindingResource"],
      required: true,
    },
  ];
  webidl.converters["GPUBindGroupEntry"] = webidl.createDictionaryConverter(
    "GPUBindGroupEntry",
    dictMembersGPUBindGroupEntry,
  );

  // DICTIONARY: GPUBindGroupDescriptor
  const dictMembersGPUBindGroupDescriptor = [
    {
      key: "layout",
      converter: webidl.converters["GPUBindGroupLayout"],
      required: true,
    },
    {
      key: "entries",
      converter: webidl.createSequenceConverter(
        webidl.converters["GPUBindGroupEntry"],
      ),
      required: true,
    },
  ];
  webidl.converters["GPUBindGroupDescriptor"] = webidl
    .createDictionaryConverter(
      "GPUBindGroupDescriptor",
      dictMembersGPUObjectDescriptorBase,
      dictMembersGPUBindGroupDescriptor,
    );

  // INTERFACE: GPUPipelineLayout
  webidl.converters.GPUPipelineLayout = webidl.createInterfaceConverter(
    "GPUPipelineLayout",
    GPUPipelineLayout.prototype,
  );

  // DICTIONARY: GPUPipelineLayoutDescriptor
  const dictMembersGPUPipelineLayoutDescriptor = [
    {
      key: "bindGroupLayouts",
      converter: webidl.createSequenceConverter(
        webidl.converters["GPUBindGroupLayout"],
      ),
      required: true,
    },
  ];
  webidl.converters["GPUPipelineLayoutDescriptor"] = webidl
    .createDictionaryConverter(
      "GPUPipelineLayoutDescriptor",
      dictMembersGPUObjectDescriptorBase,
      dictMembersGPUPipelineLayoutDescriptor,
    );

  // INTERFACE: GPUShaderModule
  webidl.converters.GPUShaderModule = webidl.createInterfaceConverter(
    "GPUShaderModule",
    GPUShaderModule.prototype,
  );

  // DICTIONARY: GPUShaderModuleDescriptor
  const dictMembersGPUShaderModuleDescriptor = [
    {
      key: "code",
      converter: webidl.converters["DOMString"],
      required: true,
    },
    { key: "sourceMap", converter: webidl.converters["object"] },
  ];
  webidl.converters["GPUShaderModuleDescriptor"] = webidl
    .createDictionaryConverter(
      "GPUShaderModuleDescriptor",
      dictMembersGPUObjectDescriptorBase,
      dictMembersGPUShaderModuleDescriptor,
    );

  // // ENUM: GPUCompilationMessageType
  // webidl.converters["GPUCompilationMessageType"] = webidl.createEnumConverter(
  //   "GPUCompilationMessageType",
  //   [
  //     "error",
  //     "warning",
  //     "info",
  //   ],
  // );

  // // INTERFACE: GPUCompilationMessage
  // webidl.converters.GPUCompilationMessage = webidl.createInterfaceConverter(
  //   "GPUCompilationMessage",
  //   GPUCompilationMessage.prototype,
  // );

  // // INTERFACE: GPUCompilationInfo
  // webidl.converters.GPUCompilationInfo = webidl.createInterfaceConverter(
  //   "GPUCompilationInfo",
  //   GPUCompilationInfo.prototype,
  // );

  webidl.converters["GPUAutoLayoutMode"] = webidl.createEnumConverter(
    "GPUAutoLayoutMode",
    [
      "auto",
    ],
  );

  webidl.converters["GPUPipelineLayout or GPUAutoLayoutMode"] = (V, opts) => {
    if (typeof V === "object") {
      return webidl.converters["GPUPipelineLayout"](V, opts);
    }
    return webidl.converters["GPUAutoLayoutMode"](V, opts);
  };

  // DICTIONARY: GPUPipelineDescriptorBase
  const dictMembersGPUPipelineDescriptorBase = [
    {
      key: "layout",
      converter: webidl.converters["GPUPipelineLayout or GPUAutoLayoutMode"],
    },
  ];
  webidl.converters["GPUPipelineDescriptorBase"] = webidl
    .createDictionaryConverter(
      "GPUPipelineDescriptorBase",
      dictMembersGPUObjectDescriptorBase,
      dictMembersGPUPipelineDescriptorBase,
    );

  // TYPEDEF: GPUPipelineConstantValue
  webidl.converters.GPUPipelineConstantValue = webidl.converters.double;

  webidl.converters["record<USVString, GPUPipelineConstantValue>"] = webidl
    .createRecordConverter(
      webidl.converters.USVString,
      webidl.converters.GPUPipelineConstantValue,
    );

  // DICTIONARY: GPUProgrammableStage
  const dictMembersGPUProgrammableStage = [
    {
      key: "module",
      converter: webidl.converters["GPUShaderModule"],
      required: true,
    },
    {
      key: "entryPoint",
      converter: webidl.converters["USVString"],
      required: true,
    },
    {
      key: "constants",
      converter:
        webidl.converters["record<USVString, GPUPipelineConstantValue>"],
    },
  ];
  webidl.converters["GPUProgrammableStage"] = webidl.createDictionaryConverter(
    "GPUProgrammableStage",
    dictMembersGPUProgrammableStage,
  );

  // INTERFACE: GPUComputePipeline
  webidl.converters.GPUComputePipeline = webidl.createInterfaceConverter(
    "GPUComputePipeline",
    GPUComputePipeline.prototype,
  );

  // DICTIONARY: GPUComputePipelineDescriptor
  const dictMembersGPUComputePipelineDescriptor = [
    {
      key: "compute",
      converter: webidl.converters["GPUProgrammableStage"],
      required: true,
    },
  ];
  webidl.converters["GPUComputePipelineDescriptor"] = webidl
    .createDictionaryConverter(
      "GPUComputePipelineDescriptor",
      dictMembersGPUObjectDescriptorBase,
      dictMembersGPUPipelineDescriptorBase,
      dictMembersGPUComputePipelineDescriptor,
    );

  // INTERFACE: GPURenderPipeline
  webidl.converters.GPURenderPipeline = webidl.createInterfaceConverter(
    "GPURenderPipeline",
    GPURenderPipeline.prototype,
  );

  // ENUM: GPUVertexStepMode
  webidl.converters["GPUVertexStepMode"] = webidl.createEnumConverter(
    "GPUVertexStepMode",
    [
      "vertex",
      "instance",
    ],
  );

  // ENUM: GPUVertexFormat
  webidl.converters["GPUVertexFormat"] = webidl.createEnumConverter(
    "GPUVertexFormat",
    [
      "uint8x2",
      "uint8x4",
      "sint8x2",
      "sint8x4",
      "unorm8x2",
      "unorm8x4",
      "snorm8x2",
      "snorm8x4",
      "uint16x2",
      "uint16x4",
      "sint16x2",
      "sint16x4",
      "unorm16x2",
      "unorm16x4",
      "snorm16x2",
      "snorm16x4",
      "float16x2",
      "float16x4",
      "float32",
      "float32x2",
      "float32x3",
      "float32x4",
      "uint32",
      "uint32x2",
      "uint32x3",
      "uint32x4",
      "sint32",
      "sint32x2",
      "sint32x3",
      "sint32x4",
    ],
  );

  // DICTIONARY: GPUVertexAttribute
  const dictMembersGPUVertexAttribute = [
    {
      key: "format",
      converter: webidl.converters["GPUVertexFormat"],
      required: true,
    },
    {
      key: "offset",
      converter: webidl.converters["GPUSize64"],
      required: true,
    },
    {
      key: "shaderLocation",
      converter: webidl.converters["GPUIndex32"],
      required: true,
    },
  ];
  webidl.converters["GPUVertexAttribute"] = webidl.createDictionaryConverter(
    "GPUVertexAttribute",
    dictMembersGPUVertexAttribute,
  );

  // DICTIONARY: GPUVertexBufferLayout
  const dictMembersGPUVertexBufferLayout = [
    {
      key: "arrayStride",
      converter: webidl.converters["GPUSize64"],
      required: true,
    },
    {
      key: "stepMode",
      converter: webidl.converters["GPUVertexStepMode"],
      defaultValue: "vertex",
    },
    {
      key: "attributes",
      converter: webidl.createSequenceConverter(
        webidl.converters["GPUVertexAttribute"],
      ),
      required: true,
    },
  ];
  webidl.converters["GPUVertexBufferLayout"] = webidl.createDictionaryConverter(
    "GPUVertexBufferLayout",
    dictMembersGPUVertexBufferLayout,
  );

  // DICTIONARY: GPUVertexState
  const dictMembersGPUVertexState = [
    {
      key: "buffers",
      converter: webidl.createSequenceConverter(
        webidl.createNullableConverter(
          webidl.converters["GPUVertexBufferLayout"],
        ),
      ),
      get defaultValue() {
        return [];
      },
    },
  ];
  webidl.converters["GPUVertexState"] = webidl.createDictionaryConverter(
    "GPUVertexState",
    dictMembersGPUProgrammableStage,
    dictMembersGPUVertexState,
  );

  // ENUM: GPUPrimitiveTopology
  webidl.converters["GPUPrimitiveTopology"] = webidl.createEnumConverter(
    "GPUPrimitiveTopology",
    [
      "point-list",
      "line-list",
      "line-strip",
      "triangle-list",
      "triangle-strip",
    ],
  );

  // ENUM: GPUIndexFormat
  webidl.converters["GPUIndexFormat"] = webidl.createEnumConverter(
    "GPUIndexFormat",
    [
      "uint16",
      "uint32",
    ],
  );

  // ENUM: GPUFrontFace
  webidl.converters["GPUFrontFace"] = webidl.createEnumConverter(
    "GPUFrontFace",
    [
      "ccw",
      "cw",
    ],
  );

  // ENUM: GPUCullMode
  webidl.converters["GPUCullMode"] = webidl.createEnumConverter("GPUCullMode", [
    "none",
    "front",
    "back",
  ]);

  // DICTIONARY: GPUPrimitiveState
  const dictMembersGPUPrimitiveState = [
    {
      key: "topology",
      converter: webidl.converters["GPUPrimitiveTopology"],
      defaultValue: "triangle-list",
    },
    { key: "stripIndexFormat", converter: webidl.converters["GPUIndexFormat"] },
    {
      key: "frontFace",
      converter: webidl.converters["GPUFrontFace"],
      defaultValue: "ccw",
    },
    {
      key: "cullMode",
      converter: webidl.converters["GPUCullMode"],
      defaultValue: "none",
    },
    {
      key: "unclippedDepth",
      converter: webidl.converters["boolean"],
      defaultValue: false,
    },
  ];
  webidl.converters["GPUPrimitiveState"] = webidl.createDictionaryConverter(
    "GPUPrimitiveState",
    dictMembersGPUPrimitiveState,
  );

  // ENUM: GPUStencilOperation
  webidl.converters["GPUStencilOperation"] = webidl.createEnumConverter(
    "GPUStencilOperation",
    [
      "keep",
      "zero",
      "replace",
      "invert",
      "increment-clamp",
      "decrement-clamp",
      "increment-wrap",
      "decrement-wrap",
    ],
  );

  // DICTIONARY: GPUStencilFaceState
  const dictMembersGPUStencilFaceState = [
    {
      key: "compare",
      converter: webidl.converters["GPUCompareFunction"],
      defaultValue: "always",
    },
    {
      key: "failOp",
      converter: webidl.converters["GPUStencilOperation"],
      defaultValue: "keep",
    },
    {
      key: "depthFailOp",
      converter: webidl.converters["GPUStencilOperation"],
      defaultValue: "keep",
    },
    {
      key: "passOp",
      converter: webidl.converters["GPUStencilOperation"],
      defaultValue: "keep",
    },
  ];
  webidl.converters["GPUStencilFaceState"] = webidl.createDictionaryConverter(
    "GPUStencilFaceState",
    dictMembersGPUStencilFaceState,
  );

  // TYPEDEF: GPUStencilValue
  webidl.converters["GPUStencilValue"] = (V, opts) =>
    webidl.converters["unsigned long"](V, { ...opts, enforceRange: true });

  // TYPEDEF: GPUDepthBias
  webidl.converters["GPUDepthBias"] = (V, opts) =>
    webidl.converters["long"](V, { ...opts, enforceRange: true });

  // DICTIONARY: GPUDepthStencilState
  const dictMembersGPUDepthStencilState = [
    {
      key: "format",
      converter: webidl.converters["GPUTextureFormat"],
      required: true,
    },
    {
      key: "depthWriteEnabled",
      converter: webidl.converters["boolean"],
      defaultValue: false,
    },
    {
      key: "depthCompare",
      converter: webidl.converters["GPUCompareFunction"],
      defaultValue: "always",
    },
    {
      key: "stencilFront",
      converter: webidl.converters["GPUStencilFaceState"],
      get defaultValue() {
        return {};
      },
    },
    {
      key: "stencilBack",
      converter: webidl.converters["GPUStencilFaceState"],
      get defaultValue() {
        return {};
      },
    },
    {
      key: "stencilReadMask",
      converter: webidl.converters["GPUStencilValue"],
      defaultValue: 0xFFFFFFFF,
    },
    {
      key: "stencilWriteMask",
      converter: webidl.converters["GPUStencilValue"],
      defaultValue: 0xFFFFFFFF,
    },
    {
      key: "depthBias",
      converter: webidl.converters["GPUDepthBias"],
      defaultValue: 0,
    },
    {
      key: "depthBiasSlopeScale",
      converter: webidl.converters["float"],
      defaultValue: 0,
    },
    {
      key: "depthBiasClamp",
      converter: webidl.converters["float"],
      defaultValue: 0,
    },
  ];
  webidl.converters["GPUDepthStencilState"] = webidl.createDictionaryConverter(
    "GPUDepthStencilState",
    dictMembersGPUDepthStencilState,
  );

  // TYPEDEF: GPUSampleMask
  webidl.converters["GPUSampleMask"] = (V, opts) =>
    webidl.converters["unsigned long"](V, { ...opts, enforceRange: true });

  // DICTIONARY: GPUMultisampleState
  const dictMembersGPUMultisampleState = [
    {
      key: "count",
      converter: webidl.converters["GPUSize32"],
      defaultValue: 1,
    },
    {
      key: "mask",
      converter: webidl.converters["GPUSampleMask"],
      defaultValue: 0xFFFFFFFF,
    },
    {
      key: "alphaToCoverageEnabled",
      converter: webidl.converters["boolean"],
      defaultValue: false,
    },
  ];
  webidl.converters["GPUMultisampleState"] = webidl.createDictionaryConverter(
    "GPUMultisampleState",
    dictMembersGPUMultisampleState,
  );

  // ENUM: GPUBlendFactor
  webidl.converters["GPUBlendFactor"] = webidl.createEnumConverter(
    "GPUBlendFactor",
    [
      "zero",
      "one",
      "src",
      "one-minus-src",
      "src-alpha",
      "one-minus-src-alpha",
      "dst",
      "one-minus-dst",
      "dst-alpha",
      "one-minus-dst-alpha",
      "src-alpha-saturated",
      "constant",
      "one-minus-constant",
    ],
  );

  // ENUM: GPUBlendOperation
  webidl.converters["GPUBlendOperation"] = webidl.createEnumConverter(
    "GPUBlendOperation",
    [
      "add",
      "subtract",
      "reverse-subtract",
      "min",
      "max",
    ],
  );

  // DICTIONARY: GPUBlendComponent
  const dictMembersGPUBlendComponent = [
    {
      key: "srcFactor",
      converter: webidl.converters["GPUBlendFactor"],
      defaultValue: "one",
    },
    {
      key: "dstFactor",
      converter: webidl.converters["GPUBlendFactor"],
      defaultValue: "zero",
    },
    {
      key: "operation",
      converter: webidl.converters["GPUBlendOperation"],
      defaultValue: "add",
    },
  ];
  webidl.converters["GPUBlendComponent"] = webidl.createDictionaryConverter(
    "GPUBlendComponent",
    dictMembersGPUBlendComponent,
  );

  // DICTIONARY: GPUBlendState
  const dictMembersGPUBlendState = [
    {
      key: "color",
      converter: webidl.converters["GPUBlendComponent"],
      required: true,
    },
    {
      key: "alpha",
      converter: webidl.converters["GPUBlendComponent"],
      required: true,
    },
  ];
  webidl.converters["GPUBlendState"] = webidl.createDictionaryConverter(
    "GPUBlendState",
    dictMembersGPUBlendState,
  );

  // TYPEDEF: GPUColorWriteFlags
  webidl.converters["GPUColorWriteFlags"] = (V, opts) =>
    webidl.converters["unsigned long"](V, { ...opts, enforceRange: true });

  // DICTIONARY: GPUColorTargetState
  const dictMembersGPUColorTargetState = [
    {
      key: "format",
      converter: webidl.converters["GPUTextureFormat"],
      required: true,
    },
    { key: "blend", converter: webidl.converters["GPUBlendState"] },
    {
      key: "writeMask",
      converter: webidl.converters["GPUColorWriteFlags"],
      defaultValue: 0xF,
    },
  ];
  webidl.converters["GPUColorTargetState"] = webidl.createDictionaryConverter(
    "GPUColorTargetState",
    dictMembersGPUColorTargetState,
  );

  // DICTIONARY: GPUFragmentState
  const dictMembersGPUFragmentState = [
    {
      key: "targets",
      converter: webidl.createSequenceConverter(
        webidl.createNullableConverter(
          webidl.converters["GPUColorTargetState"],
        ),
      ),
      required: true,
    },
  ];
  webidl.converters["GPUFragmentState"] = webidl.createDictionaryConverter(
    "GPUFragmentState",
    dictMembersGPUProgrammableStage,
    dictMembersGPUFragmentState,
  );

  // DICTIONARY: GPURenderPipelineDescriptor
  const dictMembersGPURenderPipelineDescriptor = [
    {
      key: "vertex",
      converter: webidl.converters["GPUVertexState"],
      required: true,
    },
    {
      key: "primitive",
      converter: webidl.converters["GPUPrimitiveState"],
      get defaultValue() {
        return {};
      },
    },
    {
      key: "depthStencil",
      converter: webidl.converters["GPUDepthStencilState"],
    },
    {
      key: "multisample",
      converter: webidl.converters["GPUMultisampleState"],
      get defaultValue() {
        return {};
      },
    },
    { key: "fragment", converter: webidl.converters["GPUFragmentState"] },
  ];
  webidl.converters["GPURenderPipelineDescriptor"] = webidl
    .createDictionaryConverter(
      "GPURenderPipelineDescriptor",
      dictMembersGPUObjectDescriptorBase,
      dictMembersGPUPipelineDescriptorBase,
      dictMembersGPURenderPipelineDescriptor,
    );

  // INTERFACE: GPUColorWrite
  webidl.converters.GPUColorWrite = webidl.createInterfaceConverter(
    "GPUColorWrite",
    GPUColorWrite.prototype,
  );

  // INTERFACE: GPUCommandBuffer
  webidl.converters.GPUCommandBuffer = webidl.createInterfaceConverter(
    "GPUCommandBuffer",
    GPUCommandBuffer.prototype,
  );
  webidl.converters["sequence<GPUCommandBuffer>"] = webidl
    .createSequenceConverter(webidl.converters["GPUCommandBuffer"]);

  // DICTIONARY: GPUCommandBufferDescriptor
  const dictMembersGPUCommandBufferDescriptor = [];
  webidl.converters["GPUCommandBufferDescriptor"] = webidl
    .createDictionaryConverter(
      "GPUCommandBufferDescriptor",
      dictMembersGPUObjectDescriptorBase,
      dictMembersGPUCommandBufferDescriptor,
    );

  // INTERFACE: GPUCommandEncoder
  webidl.converters.GPUCommandEncoder = webidl.createInterfaceConverter(
    "GPUCommandEncoder",
    GPUCommandEncoder.prototype,
  );

  // DICTIONARY: GPUCommandEncoderDescriptor
  const dictMembersGPUCommandEncoderDescriptor = [];
  webidl.converters["GPUCommandEncoderDescriptor"] = webidl
    .createDictionaryConverter(
      "GPUCommandEncoderDescriptor",
      dictMembersGPUObjectDescriptorBase,
      dictMembersGPUCommandEncoderDescriptor,
    );

  // DICTIONARY: GPUImageDataLayout
  const dictMembersGPUImageDataLayout = [
    {
      key: "offset",
      converter: webidl.converters["GPUSize64"],
      defaultValue: 0,
    },
    { key: "bytesPerRow", converter: webidl.converters["GPUSize32"] },
    { key: "rowsPerImage", converter: webidl.converters["GPUSize32"] },
  ];
  webidl.converters["GPUImageDataLayout"] = webidl.createDictionaryConverter(
    "GPUImageDataLayout",
    dictMembersGPUImageDataLayout,
  );

  // DICTIONARY: GPUImageCopyBuffer
  const dictMembersGPUImageCopyBuffer = [
    {
      key: "buffer",
      converter: webidl.converters["GPUBuffer"],
      required: true,
    },
  ];
  webidl.converters["GPUImageCopyBuffer"] = webidl.createDictionaryConverter(
    "GPUImageCopyBuffer",
    dictMembersGPUImageDataLayout,
    dictMembersGPUImageCopyBuffer,
  );

  // DICTIONARY: GPUOrigin3DDict
  const dictMembersGPUOrigin3DDict = [
    {
      key: "x",
      converter: webidl.converters["GPUIntegerCoordinate"],
      defaultValue: 0,
    },
    {
      key: "y",
      converter: webidl.converters["GPUIntegerCoordinate"],
      defaultValue: 0,
    },
    {
      key: "z",
      converter: webidl.converters["GPUIntegerCoordinate"],
      defaultValue: 0,
    },
  ];
  webidl.converters["GPUOrigin3DDict"] = webidl.createDictionaryConverter(
    "GPUOrigin3DDict",
    dictMembersGPUOrigin3DDict,
  );

  // TYPEDEF: GPUOrigin3D
  webidl.converters["GPUOrigin3D"] = (V, opts) => {
    // Union for (sequence<GPUIntegerCoordinate> or GPUOrigin3DDict)
    if (V === null || V === undefined) {
      return webidl.converters["GPUOrigin3DDict"](V, opts);
    }
    if (typeof V === "object") {
      const method = V[SymbolIterator];
      if (method !== undefined) {
        return webidl.converters["sequence<GPUIntegerCoordinate>"](V, opts);
      }
      return webidl.converters["GPUOrigin3DDict"](V, opts);
    }
    throw webidl.makeException(
      TypeError,
      "can not be converted to sequence<GPUIntegerCoordinate> or GPUOrigin3DDict.",
      opts,
    );
  };

  // DICTIONARY: GPUImageCopyTexture
  const dictMembersGPUImageCopyTexture = [
    {
      key: "texture",
      converter: webidl.converters["GPUTexture"],
      required: true,
    },
    {
      key: "mipLevel",
      converter: webidl.converters["GPUIntegerCoordinate"],
      defaultValue: 0,
    },
    {
      key: "origin",
      converter: webidl.converters["GPUOrigin3D"],
      get defaultValue() {
        return {};
      },
    },
    {
      key: "aspect",
      converter: webidl.converters["GPUTextureAspect"],
      defaultValue: "all",
    },
  ];
  webidl.converters["GPUImageCopyTexture"] = webidl.createDictionaryConverter(
    "GPUImageCopyTexture",
    dictMembersGPUImageCopyTexture,
  );

  // DICTIONARY: GPUOrigin2DDict
  const dictMembersGPUOrigin2DDict = [
    {
      key: "x",
      converter: webidl.converters["GPUIntegerCoordinate"],
      defaultValue: 0,
    },
    {
      key: "y",
      converter: webidl.converters["GPUIntegerCoordinate"],
      defaultValue: 0,
    },
  ];
  webidl.converters["GPUOrigin2DDict"] = webidl.createDictionaryConverter(
    "GPUOrigin2DDict",
    dictMembersGPUOrigin2DDict,
  );

  // TYPEDEF: GPUOrigin2D
  webidl.converters["GPUOrigin2D"] = (V, opts) => {
    // Union for (sequence<GPUIntegerCoordinate> or GPUOrigin2DDict)
    if (V === null || V === undefined) {
      return webidl.converters["GPUOrigin2DDict"](V, opts);
    }
    if (typeof V === "object") {
      const method = V[SymbolIterator];
      if (method !== undefined) {
        return webidl.converters["sequence<GPUIntegerCoordinate>"](V, opts);
      }
      return webidl.converters["GPUOrigin2DDict"](V, opts);
    }
    throw webidl.makeException(
      TypeError,
      "can not be converted to sequence<GPUIntegerCoordinate> or GPUOrigin2DDict.",
      opts,
    );
  };

  // INTERFACE: GPUComputePassEncoder
  webidl.converters.GPUComputePassEncoder = webidl.createInterfaceConverter(
    "GPUComputePassEncoder",
    GPUComputePassEncoder.prototype,
  );

  // DICTIONARY: GPUComputePassDescriptor
  const dictMembersGPUComputePassDescriptor = [];
  webidl.converters["GPUComputePassDescriptor"] = webidl
    .createDictionaryConverter(
      "GPUComputePassDescriptor",
      dictMembersGPUObjectDescriptorBase,
      dictMembersGPUComputePassDescriptor,
    );

  // INTERFACE: GPURenderPassEncoder
  webidl.converters.GPURenderPassEncoder = webidl.createInterfaceConverter(
    "GPURenderPassEncoder",
    GPURenderPassEncoder.prototype,
  );

  // ENUM: GPULoadOp
  webidl.converters["GPULoadOp"] = webidl.createEnumConverter("GPULoadOp", [
    "load",
    "clear",
  ]);

  // DICTIONARY: GPUColorDict
  const dictMembersGPUColorDict = [
    { key: "r", converter: webidl.converters["double"], required: true },
    { key: "g", converter: webidl.converters["double"], required: true },
    { key: "b", converter: webidl.converters["double"], required: true },
    { key: "a", converter: webidl.converters["double"], required: true },
  ];
  webidl.converters["GPUColorDict"] = webidl.createDictionaryConverter(
    "GPUColorDict",
    dictMembersGPUColorDict,
  );

  // TYPEDEF: GPUColor
  webidl.converters["GPUColor"] = (V, opts) => {
    // Union for (sequence<double> or GPUColorDict)
    if (V === null || V === undefined) {
      return webidl.converters["GPUColorDict"](V, opts);
    }
    if (typeof V === "object") {
      const method = V[SymbolIterator];
      if (method !== undefined) {
        return webidl.converters["sequence<double>"](V, opts);
      }
      return webidl.converters["GPUColorDict"](V, opts);
    }
    throw webidl.makeException(
      TypeError,
      "can not be converted to sequence<double> or GPUColorDict.",
      opts,
    );
  };

  // ENUM: GPUStoreOp
  webidl.converters["GPUStoreOp"] = webidl.createEnumConverter("GPUStoreOp", [
    "store",
    "discard",
  ]);

  // DICTIONARY: GPURenderPassColorAttachment
  const dictMembersGPURenderPassColorAttachment = [
    {
      key: "view",
      converter: webidl.converters["GPUTextureView"],
      required: true,
    },
    { key: "resolveTarget", converter: webidl.converters["GPUTextureView"] },
    {
      key: "clearValue",
      converter: webidl.converters["GPUColor"],
    },
    {
      key: "loadOp",
      converter: webidl.converters["GPULoadOp"],
      required: true,
    },
    {
      key: "storeOp",
      converter: webidl.converters["GPUStoreOp"],
      required: true,
    },
  ];
  webidl.converters["GPURenderPassColorAttachment"] = webidl
    .createDictionaryConverter(
      "GPURenderPassColorAttachment",
      dictMembersGPURenderPassColorAttachment,
    );

  // DICTIONARY: GPURenderPassDepthStencilAttachment
  const dictMembersGPURenderPassDepthStencilAttachment = [
    {
      key: "view",
      converter: webidl.converters["GPUTextureView"],
      required: true,
    },
    {
      key: "depthClearValue",
      converter: webidl.converters["float"],
      defaultValue: 0,
    },
    {
      key: "depthLoadOp",
      converter: webidl.converters["GPULoadOp"],
    },
    {
      key: "depthStoreOp",
      converter: webidl.converters["GPUStoreOp"],
    },
    {
      key: "depthReadOnly",
      converter: webidl.converters["boolean"],
      defaultValue: false,
    },
    {
      key: "stencilClearValue",
      converter: webidl.converters["GPUStencilValue"],
      defaultValue: 0,
    },
    {
      key: "stencilLoadOp",
      converter: webidl.converters["GPULoadOp"],
    },
    {
      key: "stencilStoreOp",
      converter: webidl.converters["GPUStoreOp"],
    },
    {
      key: "stencilReadOnly",
      converter: webidl.converters["boolean"],
      defaultValue: false,
    },
  ];
  webidl.converters["GPURenderPassDepthStencilAttachment"] = webidl
    .createDictionaryConverter(
      "GPURenderPassDepthStencilAttachment",
      dictMembersGPURenderPassDepthStencilAttachment,
    );

  // INTERFACE: GPUQuerySet
  webidl.converters.GPUQuerySet = webidl.createInterfaceConverter(
    "GPUQuerySet",
    GPUQuerySet.prototype,
  );

  // DICTIONARY: GPURenderPassDescriptor
  const dictMembersGPURenderPassDescriptor = [
    {
      key: "colorAttachments",
      converter: webidl.createSequenceConverter(
        webidl.createNullableConverter(
          webidl.converters["GPURenderPassColorAttachment"],
        ),
      ),
      required: true,
    },
    {
      key: "depthStencilAttachment",
      converter: webidl.converters["GPURenderPassDepthStencilAttachment"],
    },
    { key: "occlusionQuerySet", converter: webidl.converters["GPUQuerySet"] },
  ];
  webidl.converters["GPURenderPassDescriptor"] = webidl
    .createDictionaryConverter(
      "GPURenderPassDescriptor",
      dictMembersGPUObjectDescriptorBase,
      dictMembersGPURenderPassDescriptor,
    );

  // INTERFACE: GPURenderBundle
  webidl.converters.GPURenderBundle = webidl.createInterfaceConverter(
    "GPURenderBundle",
    GPURenderBundle.prototype,
  );
  webidl.converters["sequence<GPURenderBundle>"] = webidl
    .createSequenceConverter(webidl.converters["GPURenderBundle"]);

  // DICTIONARY: GPURenderBundleDescriptor
  const dictMembersGPURenderBundleDescriptor = [];
  webidl.converters["GPURenderBundleDescriptor"] = webidl
    .createDictionaryConverter(
      "GPURenderBundleDescriptor",
      dictMembersGPUObjectDescriptorBase,
      dictMembersGPURenderBundleDescriptor,
    );

  // INTERFACE: GPURenderBundleEncoder
  webidl.converters.GPURenderBundleEncoder = webidl.createInterfaceConverter(
    "GPURenderBundleEncoder",
    GPURenderBundleEncoder.prototype,
  );

  // DICTIONARY: GPURenderPassLayout
  const dictMembersGPURenderPassLayout = [
    {
      key: "colorFormats",
      converter: webidl.createSequenceConverter(
        webidl.createNullableConverter(webidl.converters["GPUTextureFormat"]),
      ),
      required: true,
    },
    {
      key: "depthStencilFormat",
      converter: webidl.converters["GPUTextureFormat"],
    },
    {
      key: "sampleCount",
      converter: webidl.converters["GPUSize32"],
      defaultValue: 1,
    },
  ];
  webidl.converters["GPURenderPassLayout"] = webidl
    .createDictionaryConverter(
      "GPURenderPassLayout",
      dictMembersGPUObjectDescriptorBase,
      dictMembersGPURenderPassLayout,
    );

  // DICTIONARY: GPURenderBundleEncoderDescriptor
  const dictMembersGPURenderBundleEncoderDescriptor = [
    {
      key: "depthReadOnly",
      converter: webidl.converters.boolean,
      defaultValue: false,
    },
    {
      key: "stencilReadOnly",
      converter: webidl.converters.boolean,
      defaultValue: false,
    },
  ];
  webidl.converters["GPURenderBundleEncoderDescriptor"] = webidl
    .createDictionaryConverter(
      "GPURenderBundleEncoderDescriptor",
      dictMembersGPUObjectDescriptorBase,
      dictMembersGPURenderPassLayout,
      dictMembersGPURenderBundleEncoderDescriptor,
    );

  // INTERFACE: GPUQueue
  webidl.converters.GPUQueue = webidl.createInterfaceConverter(
    "GPUQueue",
    GPUQueue.prototype,
  );

  // ENUM: GPUQueryType
  webidl.converters["GPUQueryType"] = webidl.createEnumConverter(
    "GPUQueryType",
    [
      "occlusion",
      "pipeline-statistics",
      "timestamp",
    ],
  );

  // ENUM: GPUPipelineStatisticName
  webidl.converters["GPUPipelineStatisticName"] = webidl.createEnumConverter(
    "GPUPipelineStatisticName",
    [
      "vertex-shader-invocations",
      "clipper-invocations",
      "clipper-primitives-out",
      "fragment-shader-invocations",
      "compute-shader-invocations",
    ],
  );

  // DICTIONARY: GPUQuerySetDescriptor
  const dictMembersGPUQuerySetDescriptor = [
    {
      key: "type",
      converter: webidl.converters["GPUQueryType"],
      required: true,
    },
    { key: "count", converter: webidl.converters["GPUSize32"], required: true },
    {
      key: "pipelineStatistics",
      converter: webidl.createSequenceConverter(
        webidl.converters["GPUPipelineStatisticName"],
      ),
      get defaultValue() {
        return [];
      },
    },
  ];
  webidl.converters["GPUQuerySetDescriptor"] = webidl.createDictionaryConverter(
    "GPUQuerySetDescriptor",
    dictMembersGPUObjectDescriptorBase,
    dictMembersGPUQuerySetDescriptor,
  );

  // ENUM: GPUDeviceLostReason
  webidl.converters["GPUDeviceLostReason"] = webidl.createEnumConverter(
    "GPUDeviceLostReason",
    [
      "destroyed",
    ],
  );

  // // INTERFACE: GPUDeviceLostInfo
  // webidl.converters.GPUDeviceLostInfo = webidl.createInterfaceConverter(
  //   "GPUDeviceLostInfo",
  //   GPUDeviceLostInfo.prototype,
  // );

  // ENUM: GPUErrorFilter
  webidl.converters["GPUErrorFilter"] = webidl.createEnumConverter(
    "GPUErrorFilter",
    [
      "out-of-memory",
      "validation",
    ],
  );

  // INTERFACE: GPUOutOfMemoryError
  webidl.converters.GPUOutOfMemoryError = webidl.createInterfaceConverter(
    "GPUOutOfMemoryError",
    GPUOutOfMemoryError.prototype,
  );

  // INTERFACE: GPUValidationError
  webidl.converters.GPUValidationError = webidl.createInterfaceConverter(
    "GPUValidationError",
    GPUValidationError.prototype,
  );

  // TYPEDEF: GPUError
  webidl.converters["GPUError"] = webidl.converters.any /** put union here! **/;

  // // INTERFACE: GPUUncapturedErrorEvent
  // webidl.converters.GPUUncapturedErrorEvent = webidl.createInterfaceConverter(
  //   "GPUUncapturedErrorEvent",
  //   GPUUncapturedErrorEvent.prototype,
  // );

  // DICTIONARY: GPUUncapturedErrorEventInit
  const dictMembersGPUUncapturedErrorEventInit = [
    { key: "error", converter: webidl.converters["GPUError"], required: true },
  ];
  webidl.converters["GPUUncapturedErrorEventInit"] = webidl
    .createDictionaryConverter(
      "GPUUncapturedErrorEventInit",
      // dictMembersEventInit,
      dictMembersGPUUncapturedErrorEventInit,
    );

  // TYPEDEF: GPUBufferDynamicOffset
  webidl.converters["GPUBufferDynamicOffset"] = (V, opts) =>
    webidl.converters["unsigned long"](V, { ...opts, enforceRange: true });

  // TYPEDEF: GPUSignedOffset32
  webidl.converters["GPUSignedOffset32"] = (V, opts) =>
    webidl.converters["long"](V, { ...opts, enforceRange: true });

  // TYPEDEF: GPUFlagsConstant
  webidl.converters["GPUFlagsConstant"] = webidl.converters["unsigned long"];
})(this);
