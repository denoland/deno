// Copyright 2018-2025 the Deno authors. MIT license.

// @ts-check
/// <reference path="../../core/lib.deno_core.d.ts" />
/// <reference path="../web/internal.d.ts" />
/// <reference path="../../cli/tsc/dts/lib.deno_web.d.ts" />
/// <reference path="../../cli/tsc/dts/lib.deno_webgpu.d.ts" />

import { core, primordials } from "ext:core/mod.js";
import {
  GPU,
  GPUAdapter,
  GPUAdapterInfo,
  GPUBindGroup,
  GPUBindGroupLayout,
  GPUBuffer,
  GPUCommandBuffer,
  GPUCommandEncoder,
  GPUComputePassEncoder,
  GPUComputePipeline,
  GPUDevice,
  GPUDeviceLostInfo,
  GPUPipelineLayout,
  GPUQuerySet,
  GPUQueue,
  GPURenderBundle,
  GPURenderBundleEncoder,
  GPURenderPassEncoder,
  GPURenderPipeline,
  GPUSampler,
  GPUShaderModule,
  GPUSupportedFeatures,
  GPUSupportedLimits,
  GPUTexture,
  GPUTextureView,
  op_create_gpu,
  op_webgpu_device_start_capture,
  op_webgpu_device_stop_capture,
} from "ext:core/ops";
const {
  ObjectDefineProperty,
  ObjectPrototypeIsPrototypeOf,
  ObjectSetPrototypeOf,
  Symbol,
  SymbolFor,
} = primordials;

import * as webidl from "ext:deno_webidl/00_webidl.js";
import {
  defineEventHandler,
  Event,
  EventTargetPrototype,
  setEventTargetData,
} from "ext:deno_web/02_event.js";
import { DOMException } from "ext:deno_web/01_dom_exception.js";
import { createFilteredInspectProxy } from "ext:deno_console/01_console.js";

const privateCustomInspect = SymbolFor("Deno.privateCustomInspect");
const _message = Symbol("[[message]]");
const illegalConstructorKey = Symbol("illegalConstructorKey");

class GPUError {
  constructor(key = null) {
    if (key !== illegalConstructorKey) {
      webidl.illegalConstructor();
    }
  }

  [_message];
  get message() {
    webidl.assertBranded(this, GPUErrorPrototype);
    return this[_message];
  }

  [privateCustomInspect](inspect, inspectOptions) {
    return inspect(
      createFilteredInspectProxy({
        object: this,
        evaluate: ObjectPrototypeIsPrototypeOf(GPUErrorPrototype, this),
        keys: [
          "message",
        ],
      }),
      inspectOptions,
    );
  }
}
const GPUErrorPrototype = GPUError.prototype;

class GPUValidationError extends GPUError {
  /** @param {string} message */
  constructor(message) {
    const prefix = "Failed to construct 'GPUValidationError'";
    webidl.requiredArguments(arguments.length, 1, prefix);
    message = webidl.converters.DOMString(message, prefix, "Argument 1");
    super(illegalConstructorKey);
    this[webidl.brand] = webidl.brand;
    this[_message] = message;
  }
}
core.registerErrorClass("GPUValidationError", GPUValidationError);

class GPUOutOfMemoryError extends GPUError {
  constructor(message) {
    const prefix = "Failed to construct 'GPUOutOfMemoryError'";
    webidl.requiredArguments(arguments.length, 1, prefix);
    message = webidl.converters.DOMString(message, prefix, "Argument 1");
    super(illegalConstructorKey);
    this[webidl.brand] = webidl.brand;
    this[_message] = message;
  }
}
core.registerErrorClass("GPUOutOfMemoryError", GPUOutOfMemoryError);

class GPUInternalError extends GPUError {
  constructor() {
    super(illegalConstructorKey);
    this[webidl.brand] = webidl.brand;
  }
}
core.registerErrorClass("GPUInternalError", GPUInternalError);

class GPUPipelineError extends DOMException {
  #reason;

  constructor(message = "", options = { __proto__: null }) {
    const prefix = "Failed to construct 'GPUPipelineError'";
    message = webidl.converters.DOMString(message, prefix, "Argument 1");
    options = webidl.converters.GPUPipelineErrorInit(
      options,
      prefix,
      "Argument 2",
    );
    super(message, "GPUPipelineError");

    this.#reason = options.reason;
  }

  get reason() {
    webidl.assertBranded(this, GPUPipelineErrorPrototype);
    return this.#reason;
  }
}
const GPUPipelineErrorPrototype = GPUPipelineError.prototype;

class GPUUncapturedErrorEvent extends Event {
  #error;

  constructor(type, gpuUncapturedErrorEventInitDict) {
    super(type, gpuUncapturedErrorEventInitDict);
    this[webidl.brand] = webidl.brand;

    const prefix = "Failed to construct 'GPUUncapturedErrorEvent'";
    webidl.requiredArguments(arguments.length, 2, prefix);
    gpuUncapturedErrorEventInitDict = webidl.converters
      .GPUUncapturedErrorEventInit(
        gpuUncapturedErrorEventInitDict,
        prefix,
        "Argument 2",
      );

    this.#error = gpuUncapturedErrorEventInitDict.error;
  }

  get error() {
    webidl.assertBranded(this, GPUUncapturedErrorEventPrototype);
    return this.#error;
  }
}
const GPUUncapturedErrorEventPrototype = GPUUncapturedErrorEvent.prototype;

const GPUPrototype = GPU.prototype;
ObjectDefineProperty(GPUPrototype, privateCustomInspect, {
  __proto__: null,
  value(inspect, inspectOptions) {
    return `${this.constructor.name} ${inspect({}, inspectOptions)}`;
  },
});

const GPUAdapterPrototype = GPUAdapter.prototype;
ObjectDefineProperty(GPUAdapterPrototype, privateCustomInspect, {
  __proto__: null,
  value(inspect, inspectOptions) {
    return inspect(
      createFilteredInspectProxy({
        object: this,
        evaluate: ObjectPrototypeIsPrototypeOf(GPUAdapterPrototype, this),
        keys: [
          "features",
          "limits",
          "info",
        ],
      }),
      inspectOptions,
    );
  },
});

const GPUAdapterInfoPrototype = GPUAdapterInfo.prototype;
ObjectDefineProperty(GPUAdapterInfoPrototype, privateCustomInspect, {
  __proto__: null,
  value(inspect, inspectOptions) {
    return inspect(
      createFilteredInspectProxy({
        object: this,
        evaluate: ObjectPrototypeIsPrototypeOf(GPUAdapterInfoPrototype, this),
        keys: [
          "vendor",
          "architecture",
          "device",
          "description",
          "subgroupMinSize",
          "subgroupMaxSize",
          "isFallbackAdapter",
        ],
      }),
      inspectOptions,
    );
  },
});

const GPUSupportedFeaturesPrototype = GPUSupportedFeatures.prototype;
webidl.setlikeObjectWrap(GPUSupportedFeaturesPrototype, true);
ObjectDefineProperty(GPUSupportedFeaturesPrototype, privateCustomInspect, {
  __proto__: null,
  value(inspect, inspectOptions) {
    if (ObjectPrototypeIsPrototypeOf(GPUSupportedFeaturesPrototype, this)) {
      return `${this.constructor.name} ${
        // deno-lint-ignore prefer-primordials
        inspect([...this], inspectOptions)}`;
    } else {
      return `${this.constructor.name} ${inspect({}, inspectOptions)}`;
    }
  },
});

const GPUSupportedLimitsPrototype = GPUSupportedLimits.prototype;
ObjectDefineProperty(GPUSupportedLimitsPrototype, privateCustomInspect, {
  __proto__: null,
  value(inspect, inspectOptions) {
    return inspect(
      createFilteredInspectProxy({
        object: this,
        evaluate: ObjectPrototypeIsPrototypeOf(
          GPUSupportedLimitsPrototype,
          this,
        ),
        keys: [
          "maxTextureDimension1D",
          "maxTextureDimension2D",
          "maxTextureDimension3D",
          "maxTextureArrayLayers",
          "maxBindGroups",
          // TODO(@crowlKats): support max_bind_groups_plus_vertex_buffers
          // "maxBindGroupsPlusVertexBuffers",
          "maxBindingsPerBindGroup",
          "maxDynamicUniformBuffersPerPipelineLayout",
          "maxDynamicStorageBuffersPerPipelineLayout",
          "maxSampledTexturesPerShaderStage",
          "maxSamplersPerShaderStage",
          "maxStorageBuffersPerShaderStage",
          "maxStorageTexturesPerShaderStage",
          "maxUniformBuffersPerShaderStage",
          "maxUniformBufferBindingSize",
          "maxStorageBufferBindingSize",
          "minUniformBufferOffsetAlignment",
          "minStorageBufferOffsetAlignment",
          "maxVertexBuffers",
          "maxBufferSize",
          "maxVertexAttributes",
          "maxVertexBufferArrayStride",
          // TODO(@crowlKats): support max_inter_stage_shader_variables
          // "maxInterStageShaderVariables",
          "maxColorAttachments",
          "maxColorAttachmentBytesPerSample",
          "maxComputeWorkgroupStorageSize",
          "maxComputeInvocationsPerWorkgroup",
          "maxComputeWorkgroupSizeX",
          "maxComputeWorkgroupSizeY",
          "maxComputeWorkgroupSizeZ",
          "maxComputeWorkgroupsPerDimension",
        ],
      }),
      inspectOptions,
    );
  },
});

const GPUDeviceLostInfoPrototype = GPUDeviceLostInfo.prototype;
ObjectDefineProperty(GPUDeviceLostInfoPrototype, privateCustomInspect, {
  __proto__: null,
  value(inspect, inspectOptions) {
    return inspect(
      createFilteredInspectProxy({
        object: this,
        evaluate: ObjectPrototypeIsPrototypeOf(
          GPUDeviceLostInfoPrototype,
          this,
        ),
        keys: [
          "reason",
          "message",
        ],
      }),
      inspectOptions,
    );
  },
});

const GPUDevicePrototype = GPUDevice.prototype;
ObjectSetPrototypeOf(GPUDevicePrototype, EventTargetPrototype);
defineEventHandler(GPUDevicePrototype, "uncapturederror");
ObjectDefineProperty(GPUDevicePrototype, privateCustomInspect, {
  __proto__: null,
  value(inspect, inspectOptions) {
    return inspect(
      createFilteredInspectProxy({
        object: this,
        evaluate: ObjectPrototypeIsPrototypeOf(GPUDevicePrototype, this),
        keys: [
          "features",
          "label",
          "limits",
          "lost",
          "queue",
          // TODO(lucacasonato): emit an UncapturedErrorEvent
          // "onuncapturederror"
        ],
      }),
      inspectOptions,
    );
  },
});

const GPUQueuePrototype = GPUQueue.prototype;
ObjectDefineProperty(GPUQueuePrototype, privateCustomInspect, {
  __proto__: null,
  value(inspect, inspectOptions) {
    return inspect(
      createFilteredInspectProxy({
        object: this,
        evaluate: ObjectPrototypeIsPrototypeOf(GPUQueuePrototype, this),
        keys: [
          "label",
        ],
      }),
      inspectOptions,
    );
  },
});

const GPUBufferPrototype = GPUBuffer.prototype;
ObjectDefineProperty(GPUBufferPrototype, privateCustomInspect, {
  __proto__: null,
  value(inspect, inspectOptions) {
    return inspect(
      createFilteredInspectProxy({
        object: this,
        evaluate: ObjectPrototypeIsPrototypeOf(GPUBufferPrototype, this),
        keys: [
          "label",
          "mapState",
          "size",
          "usage",
        ],
      }),
      inspectOptions,
    );
  },
});

class GPUBufferUsage {
  constructor() {
    webidl.illegalConstructor();
  }

  static get MAP_READ() {
    return 0x0001;
  }
  static get MAP_WRITE() {
    return 0x0002;
  }
  static get COPY_SRC() {
    return 0x0004;
  }
  static get COPY_DST() {
    return 0x0008;
  }
  static get INDEX() {
    return 0x0010;
  }
  static get VERTEX() {
    return 0x0020;
  }
  static get UNIFORM() {
    return 0x0040;
  }
  static get STORAGE() {
    return 0x0080;
  }
  static get INDIRECT() {
    return 0x0100;
  }
  static get QUERY_RESOLVE() {
    return 0x0200;
  }
}

class GPUMapMode {
  constructor() {
    webidl.illegalConstructor();
  }

  static get READ() {
    return 0x0001;
  }
  static get WRITE() {
    return 0x0002;
  }
}

const GPUTexturePrototype = GPUTexture.prototype;
ObjectDefineProperty(GPUTexturePrototype, privateCustomInspect, {
  __proto__: null,
  value(inspect, inspectOptions) {
    return inspect(
      createFilteredInspectProxy({
        object: this,
        evaluate: ObjectPrototypeIsPrototypeOf(GPUTexturePrototype, this),
        keys: [
          "label",
          "width",
          "height",
          "depthOrArrayLayers",
          "mipLevelCount",
          "sampleCount",
          "dimension",
          "format",
          "usage",
        ],
      }),
      inspectOptions,
    );
  },
});

class GPUTextureUsage {
  constructor() {
    webidl.illegalConstructor();
  }

  static get COPY_SRC() {
    return 0x01;
  }
  static get COPY_DST() {
    return 0x02;
  }
  static get TEXTURE_BINDING() {
    return 0x04;
  }
  static get STORAGE_BINDING() {
    return 0x08;
  }
  static get RENDER_ATTACHMENT() {
    return 0x10;
  }
}

const GPUTextureViewPrototype = GPUTextureView.prototype;
ObjectDefineProperty(GPUTextureViewPrototype, privateCustomInspect, {
  __proto__: null,
  value(inspect, inspectOptions) {
    return inspect(
      createFilteredInspectProxy({
        object: this,
        evaluate: ObjectPrototypeIsPrototypeOf(GPUTextureViewPrototype, this),
        keys: [
          "label",
        ],
      }),
      inspectOptions,
    );
  },
});

const GPUSamplerPrototype = GPUSampler.prototype;
ObjectDefineProperty(GPUSamplerPrototype, privateCustomInspect, {
  __proto__: null,
  value(inspect, inspectOptions) {
    return inspect(
      createFilteredInspectProxy({
        object: this,
        evaluate: ObjectPrototypeIsPrototypeOf(
          GPUSamplerPrototype,
          this,
        ),
        keys: [
          "label",
        ],
      }),
      inspectOptions,
    );
  },
});

const GPUBindGroupLayoutPrototype = GPUBindGroupLayout.prototype;
ObjectDefineProperty(GPUBindGroupLayout, privateCustomInspect, {
  __proto__: null,
  value(inspect, inspectOptions) {
    return inspect(
      createFilteredInspectProxy({
        object: this,
        evaluate: ObjectPrototypeIsPrototypeOf(
          GPUBindGroupLayoutPrototype,
          this,
        ),
        keys: [
          "label",
        ],
      }),
      inspectOptions,
    );
  },
});

const GPUPipelineLayoutPrototype = GPUPipelineLayout.prototype;
ObjectDefineProperty(GPUPipelineLayoutPrototype, privateCustomInspect, {
  __proto__: null,
  value(inspect, inspectOptions) {
    return inspect(
      createFilteredInspectProxy({
        object: this,
        evaluate: ObjectPrototypeIsPrototypeOf(
          GPUPipelineLayoutPrototype,
          this,
        ),
        keys: [
          "label",
        ],
      }),
      inspectOptions,
    );
  },
});

const GPUBindGroupPrototype = GPUBindGroup.prototype;
ObjectDefineProperty(GPUBindGroupPrototype, privateCustomInspect, {
  __proto__: null,
  value(inspect, inspectOptions) {
    return inspect(
      createFilteredInspectProxy({
        object: this,
        evaluate: ObjectPrototypeIsPrototypeOf(GPUBindGroupPrototype, this),
        keys: [
          "label",
        ],
      }),
      inspectOptions,
    );
  },
});

const GPUShaderModulePrototype = GPUShaderModule.prototype;
ObjectDefineProperty(GPUShaderModulePrototype, privateCustomInspect, {
  __proto__: null,
  value(inspect, inspectOptions) {
    return inspect(
      createFilteredInspectProxy({
        object: this,
        evaluate: ObjectPrototypeIsPrototypeOf(GPUShaderModulePrototype, this),
        keys: [
          "label",
        ],
      }),
      inspectOptions,
    );
  },
});

class GPUShaderStage {
  constructor() {
    webidl.illegalConstructor();
  }

  static get VERTEX() {
    return 0x1;
  }

  static get FRAGMENT() {
    return 0x2;
  }

  static get COMPUTE() {
    return 0x4;
  }
}

const GPUComputePipelinePrototype = GPUComputePipeline.prototype;
ObjectDefineProperty(GPUComputePipelinePrototype, privateCustomInspect, {
  __proto__: null,
  value(inspect, inspectOptions) {
    return inspect(
      createFilteredInspectProxy({
        object: this,
        evaluate: ObjectPrototypeIsPrototypeOf(
          GPUComputePipelinePrototype,
          this,
        ),
        keys: [
          "label",
        ],
      }),
      inspectOptions,
    );
  },
});

const GPURenderPipelinePrototype = GPURenderPipeline.prototype;
ObjectDefineProperty(GPURenderPipelinePrototype, privateCustomInspect, {
  __proto__: null,
  value(inspect, inspectOptions) {
    return inspect(
      createFilteredInspectProxy({
        object: this,
        evaluate: ObjectPrototypeIsPrototypeOf(
          GPURenderPipelinePrototype,
          this,
        ),
        keys: [
          "label",
        ],
      }),
      inspectOptions,
    );
  },
});

class GPUColorWrite {
  constructor() {
    webidl.illegalConstructor();
  }

  static get RED() {
    return 0x1;
  }
  static get GREEN() {
    return 0x2;
  }
  static get BLUE() {
    return 0x4;
  }
  static get ALPHA() {
    return 0x8;
  }
  static get ALL() {
    return 0xF;
  }
}

const GPUCommandEncoderPrototype = GPUCommandEncoder.prototype;
ObjectDefineProperty(GPUCommandEncoderPrototype, privateCustomInspect, {
  __proto__: null,
  value(inspect, inspectOptions) {
    return inspect(
      createFilteredInspectProxy({
        object: this,
        evaluate: ObjectPrototypeIsPrototypeOf(
          GPUCommandEncoderPrototype,
          this,
        ),
        keys: [
          "label",
        ],
      }),
      inspectOptions,
    );
  },
});

const GPURenderPassEncoderPrototype = GPURenderPassEncoder.prototype;
ObjectDefineProperty(GPURenderPassEncoderPrototype, privateCustomInspect, {
  __proto__: null,
  value(inspect, inspectOptions) {
    return inspect(
      createFilteredInspectProxy({
        object: this,
        evaluate: ObjectPrototypeIsPrototypeOf(
          GPURenderPassEncoderPrototype,
          this,
        ),
        keys: [
          "label",
        ],
      }),
      inspectOptions,
    );
  },
});

const GPUComputePassEncoderPrototype = GPUComputePassEncoder.prototype;
ObjectDefineProperty(GPUComputePassEncoderPrototype, privateCustomInspect, {
  __proto__: null,
  value(inspect, inspectOptions) {
    return inspect(
      createFilteredInspectProxy({
        object: this,
        evaluate: ObjectPrototypeIsPrototypeOf(
          GPUComputePassEncoderPrototype,
          this,
        ),
        keys: [
          "label",
        ],
      }),
      inspectOptions,
    );
  },
});

const GPUCommandBufferPrototype = GPUCommandBuffer.prototype;
ObjectDefineProperty(GPUCommandBufferPrototype, privateCustomInspect, {
  __proto__: null,
  value(inspect, inspectOptions) {
    return inspect(
      createFilteredInspectProxy({
        object: this,
        evaluate: ObjectPrototypeIsPrototypeOf(GPUCommandBufferPrototype, this),
        keys: [
          "label",
        ],
      }),
      inspectOptions,
    );
  },
});

const GPURenderBundleEncoderPrototype = GPURenderBundleEncoder.prototype;
ObjectDefineProperty(GPURenderBundleEncoderPrototype, privateCustomInspect, {
  __proto__: null,
  value(inspect, inspectOptions) {
    return inspect(
      createFilteredInspectProxy({
        object: this,
        evaluate: ObjectPrototypeIsPrototypeOf(
          GPURenderBundleEncoderPrototype,
          this,
        ),
        keys: [
          "label",
        ],
      }),
      inspectOptions,
    );
  },
});

const GPURenderBundlePrototype = GPURenderBundle.prototype;
ObjectDefineProperty(GPURenderBundlePrototype, privateCustomInspect, {
  __proto__: null,
  value(inspect, inspectOptions) {
    return inspect(
      createFilteredInspectProxy({
        object: this,
        evaluate: ObjectPrototypeIsPrototypeOf(GPURenderBundlePrototype, this),
        keys: [
          "label",
        ],
      }),
      inspectOptions,
    );
  },
});

const GPUQuerySetPrototype = GPUQuerySet.prototype;
ObjectDefineProperty(GPUQuerySetPrototype, privateCustomInspect, {
  __proto__: null,
  value(inspect, inspectOptions) {
    return inspect(
      createFilteredInspectProxy({
        object: this,
        evaluate: ObjectPrototypeIsPrototypeOf(GPUQuerySetPrototype, this),
        keys: [
          "label",
          "type",
          "count",
        ],
      }),
      inspectOptions,
    );
  },
});

// Converters

webidl.converters["GPUPipelineErrorInit"] = webidl.createDictionaryConverter(
  "GPUPipelineErrorInit",
  [
    {
      key: "reason",
      converter: webidl.converters.GPUPipelineErrorReason,
      required: true,
    },
  ],
);

webidl.converters["GPUPipelineErrorReason"] = webidl.createEnumConverter(
  "GPUPipelineErrorReason",
  [
    "validation",
    "internal",
  ],
);

webidl.converters["GPUError"] = webidl.converters.any /* put union here! */;

const dictMembersGPUUncapturedErrorEventInit = [
  { key: "error", converter: webidl.converters["GPUError"], required: true },
];
webidl.converters["GPUUncapturedErrorEventInit"] = webidl
  .createDictionaryConverter(
    "GPUUncapturedErrorEventInit",
    // dictMembersEventInit,
    dictMembersGPUUncapturedErrorEventInit,
  );

function deviceStartCapture(device) {
  op_webgpu_device_start_capture(device);
}

function deviceStopCapture(device) {
  op_webgpu_device_stop_capture(device);
}

const denoNsWebGPU = {
  deviceStartCapture,
  deviceStopCapture,
};

let gpu;
function initGPU() {
  if (!gpu) {
    gpu = op_create_gpu(
      webidl.brand,
      setEventTargetData,
      GPUUncapturedErrorEvent,
    );
  }
}

export {
  denoNsWebGPU,
  GPU,
  gpu,
  GPUAdapter,
  GPUAdapterInfo,
  GPUBindGroup,
  GPUBindGroupLayout,
  GPUBuffer,
  GPUBufferUsage,
  GPUColorWrite,
  GPUCommandBuffer,
  GPUCommandEncoder,
  GPUComputePassEncoder,
  GPUComputePipeline,
  GPUDevice,
  GPUDeviceLostInfo,
  GPUError,
  GPUInternalError,
  GPUMapMode,
  GPUOutOfMemoryError,
  GPUPipelineLayout,
  GPUQuerySet,
  GPUQueue,
  GPURenderBundle,
  GPURenderBundleEncoder,
  GPURenderPassEncoder,
  GPURenderPipeline,
  GPUSampler,
  GPUShaderModule,
  GPUShaderStage,
  GPUSupportedFeatures,
  GPUSupportedLimits,
  GPUTexture,
  GPUTextureUsage,
  GPUTextureView,
  GPUUncapturedErrorEvent,
  GPUValidationError,
  initGPU,
};
