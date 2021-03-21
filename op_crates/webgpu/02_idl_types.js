// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

// @ts-check
/// <reference path="../web/internal.d.ts" />

"use strict";

((window) => {
  const webidl = window.__bootstrap.webidl;
  const {
    GPU,
    GPUAdapter,
    GPUAdapterLimits,
    GPUAdapterFeatures,
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

  // This needs to be initalized after all of the base classes are implmented,
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

  // INTERFACE: GPUAdapterLimits
  webidl.converters.GPUAdapterLimits = webidl.createInterfaceConverter(
    "GPUAdapterLimits",
    GPUAdapterLimits,
  );

  // INTERFACE: GPUAdapterFeatures
  webidl.converters.GPUAdapterFeatures = webidl.createInterfaceConverter(
    "GPUAdapterFeatures",
    GPUAdapterFeatures,
  );

  // INTERFACE: GPU
  webidl.converters.GPU = webidl.createInterfaceConverter("GPU", GPU);

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
  ];
  webidl.converters["GPURequestAdapterOptions"] = webidl
    .createDictionaryConverter(
      "GPURequestAdapterOptions",
      dictMembersGPURequestAdapterOptions,
    );

  // INTERFACE: GPUAdapter
  webidl.converters.GPUAdapter = webidl.createInterfaceConverter(
    "GPUAdapter",
    GPUAdapter,
  );

  // ENUM: GPUFeatureName
  webidl.converters["GPUFeatureName"] = webidl.createEnumConverter(
    "GPUFeatureName",
    [
      "depth-clamping",
      "depth24unorm-stencil8",
      "depth32float-stencil8",
      "pipeline-statistics-query",
      "texture-compression-bc",
      "timestamp-query",
    ],
  );

  // TYPEDEF: GPUSize32
  webidl.converters["GPUSize32"] = (V, opts) =>
    webidl.converters["unsigned long"](V, { ...opts, enforceRange: true });

  // DICTIONARY: GPUDeviceDescriptor
  const dictMembersGPUDeviceDescriptor = [
    {
      key: "nonGuaranteedFeatures",
      converter: webidl.createSequenceConverter(
        webidl.converters["GPUFeatureName"],
      ),
      defaultValue: [],
    },
    {
      key: "nonGuaranteedLimits",
      converter: webidl.createRecordConverter(
        webidl.converters["DOMString"],
        webidl.converters["GPUSize32"],
      ),
      defaultValue: {},
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
    GPUDevice,
  );

  // INTERFACE: GPUBuffer
  webidl.converters.GPUBuffer = webidl.createInterfaceConverter(
    "GPUBuffer",
    GPUBuffer,
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
    GPUBufferUsage,
  );

  // TYPEDEF: GPUMapModeFlags
  webidl.converters["GPUMapModeFlags"] = (V, opts) =>
    webidl.converters["unsigned long"](V, { ...opts, enforceRange: true });

  // INTERFACE: GPUMapMode
  webidl.converters.GPUMapMode = webidl.createInterfaceConverter(
    "GPUMapMode",
    GPUMapMode,
  );

  // INTERFACE: GPUTexture
  webidl.converters.GPUTexture = webidl.createInterfaceConverter(
    "GPUTexture",
    GPUTexture,
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
      defaultValue: 1,
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
      const method = V[Symbol.iterator];
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
      "depth24unorm-stencil8",
      "depth32float-stencil8",
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
    GPUTextureUsage,
  );

  // INTERFACE: GPUTextureView
  webidl.converters.GPUTextureView = webidl.createInterfaceConverter(
    "GPUTextureView",
    GPUTextureView,
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
    GPUSampler,
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
      converter: webidl.converters["GPUFilterMode"],
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
    GPUBindGroupLayout,
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
      "read-only",
      "write-only",
    ],
  );

  // DICTIONARY: GPUStorageTextureBindingLayout
  const dictMembersGPUStorageTextureBindingLayout = [
    {
      key: "access",
      converter: webidl.converters["GPUStorageTextureAccess"],
      required: true,
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
    GPUShaderStage,
  );

  // INTERFACE: GPUBindGroup
  webidl.converters.GPUBindGroup = webidl.createInterfaceConverter(
    "GPUBindGroup",
    GPUBindGroup,
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
    GPUPipelineLayout,
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

  // ENUM: GPUCompilationMessageType
  webidl.converters["GPUCompilationMessageType"] = webidl.createEnumConverter(
    "GPUCompilationMessageType",
    [
      "error",
      "warning",
      "info",
    ],
  );

  // // INTERFACE: GPUCompilationMessage
  // webidl.converters.GPUCompilationMessage = webidl.createInterfaceConverter(
  //   "GPUCompilationMessage",
  //   GPUCompilationMessage,
  // );

  // // INTERFACE: GPUCompilationInfo
  // webidl.converters.GPUCompilationInfo = webidl.createInterfaceConverter(
  //   "GPUCompilationInfo",
  //   GPUCompilationInfo,
  // );

  // INTERFACE: GPUShaderModule
  webidl.converters.GPUShaderModule = webidl.createInterfaceConverter(
    "GPUShaderModule",
    GPUShaderModule,
  );

  // DICTIONARY: GPUShaderModuleDescriptor
  const dictMembersGPUShaderModuleDescriptor = [
    {
      key: "code",
      converter: (V, opts) => {
        if (V instanceof Uint32Array) {
          return webidl.converters["Uint32Array"](V, opts);
        }
        if (typeof V === "string") {
          return webidl.converters["DOMString"](V, opts);
        }
        throw webidl.makeException(
          TypeError,
          "can not be converted to Uint32Array or DOMString.",
          opts,
        );
      },
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

  // DICTIONARY: GPUPipelineDescriptorBase
  const dictMembersGPUPipelineDescriptorBase = [
    { key: "layout", converter: webidl.converters["GPUPipelineLayout"] },
  ];
  webidl.converters["GPUPipelineDescriptorBase"] = webidl
    .createDictionaryConverter(
      "GPUPipelineDescriptorBase",
      dictMembersGPUObjectDescriptorBase,
      dictMembersGPUPipelineDescriptorBase,
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
  ];
  webidl.converters["GPUProgrammableStage"] = webidl.createDictionaryConverter(
    "GPUProgrammableStage",
    dictMembersGPUProgrammableStage,
  );

  // INTERFACE: GPUComputePipeline
  webidl.converters.GPUComputePipeline = webidl.createInterfaceConverter(
    "GPUComputePipeline",
    GPUComputePipeline,
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
      dictMembersGPUPipelineDescriptorBase,
      dictMembersGPUComputePipelineDescriptor,
    );

  // INTERFACE: GPURenderPipeline
  webidl.converters.GPURenderPipeline = webidl.createInterfaceConverter(
    "GPURenderPipeline",
    GPURenderPipeline,
  );

  // ENUM: GPUInputStepMode
  webidl.converters["GPUInputStepMode"] = webidl.createEnumConverter(
    "GPUInputStepMode",
    [
      "vertex",
      "instance",
    ],
  );

  // ENUM: GPUVertexFormat
  webidl.converters["GPUVertexFormat"] = webidl.createEnumConverter(
    "GPUVertexFormat",
    [
      "uchar2",
      "uchar4",
      "char2",
      "char4",
      "uchar2norm",
      "uchar4norm",
      "char2norm",
      "char4norm",
      "ushort2",
      "ushort4",
      "short2",
      "short4",
      "ushort2norm",
      "ushort4norm",
      "short2norm",
      "short4norm",
      "half2",
      "half4",
      "float",
      "float2",
      "float3",
      "float4",
      "uint",
      "uint2",
      "uint3",
      "uint4",
      "int",
      "int2",
      "int3",
      "int4",
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
      converter: webidl.converters["GPUInputStepMode"],
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
      defaultValue: [],
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
      defaultValue: {},
    },
    {
      key: "stencilBack",
      converter: webidl.converters["GPUStencilFaceState"],
      defaultValue: {},
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
    {
      key: "clampDepth",
      converter: webidl.converters["boolean"],
      defaultValue: false,
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
      "src-color",
      "one-minus-src-color",
      "src-alpha",
      "one-minus-src-alpha",
      "dst-color",
      "one-minus-dst-color",
      "dst-alpha",
      "one-minus-dst-alpha",
      "src-alpha-saturated",
      "blend-color",
      "one-minus-blend-color",
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
        webidl.converters["GPUColorTargetState"],
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
      defaultValue: {},
    },
    {
      key: "depthStencil",
      converter: webidl.converters["GPUDepthStencilState"],
    },
    {
      key: "multisample",
      converter: webidl.converters["GPUMultisampleState"],
      defaultValue: {},
    },
    { key: "fragment", converter: webidl.converters["GPUFragmentState"] },
  ];
  webidl.converters["GPURenderPipelineDescriptor"] = webidl
    .createDictionaryConverter(
      "GPURenderPipelineDescriptor",
      dictMembersGPUPipelineDescriptorBase,
      dictMembersGPURenderPipelineDescriptor,
    );

  // INTERFACE: GPUColorWrite
  webidl.converters.GPUColorWrite = webidl.createInterfaceConverter(
    "GPUColorWrite",
    GPUColorWrite,
  );

  // INTERFACE: GPUCommandBuffer
  webidl.converters.GPUCommandBuffer = webidl.createInterfaceConverter(
    "GPUCommandBuffer",
    GPUCommandBuffer,
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
    GPUCommandEncoder,
  );

  // DICTIONARY: GPUCommandEncoderDescriptor
  const dictMembersGPUCommandEncoderDescriptor = [
    {
      key: "measureExecutionTime",
      converter: webidl.converters["boolean"],
      defaultValue: false,
    },
  ];
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
      const method = V[Symbol.iterator];
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
      defaultValue: {},
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

  // INTERFACE: GPUComputePassEncoder
  webidl.converters.GPUComputePassEncoder = webidl.createInterfaceConverter(
    "GPUComputePassEncoder",
    GPUComputePassEncoder,
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
    GPURenderPassEncoder,
  );

  // ENUM: GPULoadOp
  webidl.converters["GPULoadOp"] = webidl.createEnumConverter("GPULoadOp", [
    "load",
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
      const method = V[Symbol.iterator];
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
  }; // ENUM: GPUStoreOp

  webidl.converters["GPUStoreOp"] = webidl.createEnumConverter("GPUStoreOp", [
    "store",
    "clear",
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
      key: "loadValue",
      converter: webidl.converters.any, /** put union here! **/
      required: true,
    },
    {
      key: "storeOp",
      converter: webidl.converters["GPUStoreOp"],
      defaultValue: "store",
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
      key: "depthLoadValue",
      converter: webidl.converters.any, /** put union here! **/
      required: true,
    },
    {
      key: "depthStoreOp",
      converter: webidl.converters["GPUStoreOp"],
      required: true,
    },
    {
      key: "depthReadOnly",
      converter: webidl.converters["boolean"],
      defaultValue: false,
    },
    {
      key: "stencilLoadValue",
      converter: webidl.converters.any, /** put union here! **/
      required: true,
    },
    {
      key: "stencilStoreOp",
      converter: webidl.converters["GPUStoreOp"],
      required: true,
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
    GPUQuerySet,
  );

  // DICTIONARY: GPURenderPassDescriptor
  const dictMembersGPURenderPassDescriptor = [
    {
      key: "colorAttachments",
      converter: webidl.createSequenceConverter(
        webidl.converters["GPURenderPassColorAttachment"],
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
    GPURenderBundle,
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
    GPURenderBundleEncoder,
  );

  // DICTIONARY: GPURenderBundleEncoderDescriptor
  const dictMembersGPURenderBundleEncoderDescriptor = [
    {
      key: "colorFormats",
      converter: webidl.createSequenceConverter(
        webidl.converters["GPUTextureFormat"],
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
  webidl.converters["GPURenderBundleEncoderDescriptor"] = webidl
    .createDictionaryConverter(
      "GPURenderBundleEncoderDescriptor",
      dictMembersGPUObjectDescriptorBase,
      dictMembersGPURenderBundleEncoderDescriptor,
    );

  // INTERFACE: GPUQueue
  webidl.converters.GPUQueue = webidl.createInterfaceConverter(
    "GPUQueue",
    GPUQueue,
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
      defaultValue: [],
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
  //   GPUDeviceLostInfo,
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
    GPUOutOfMemoryError,
  );

  // INTERFACE: GPUValidationError
  webidl.converters.GPUValidationError = webidl.createInterfaceConverter(
    "GPUValidationError",
    GPUValidationError,
  );

  // TYPEDEF: GPUError
  webidl.converters["GPUError"] = webidl.converters.any /** put union here! **/;

  // // INTERFACE: GPUUncapturedErrorEvent
  // webidl.converters.GPUUncapturedErrorEvent = webidl.createInterfaceConverter(
  //   "GPUUncapturedErrorEvent",
  //   GPUUncapturedErrorEvent,
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
      const method = V[Symbol.iterator];
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
})(this);
