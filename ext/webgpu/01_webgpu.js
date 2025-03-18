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

const customInspect = SymbolFor("Deno.privateCustomInspect");
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

  [customInspect](inspect, inspectOptions) {
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

ObjectDefineProperty(GPU, customInspect, {
  __proto__: null,
  value(inspect, inspectOptions) {
    return `${this.constructor.name} ${inspect({}, inspectOptions)}`;
  },
});

ObjectDefineProperty(GPUAdapter, customInspect, {
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
          "isFallbackAdapter",
        ],
      }),
      inspectOptions,
    );
  },
});
const GPUAdapterPrototype = GPUAdapter.prototype;

ObjectDefineProperty(GPUAdapterInfo, customInspect, {
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
        ],
      }),
      inspectOptions,
    );
  },
});
const GPUAdapterInfoPrototype = GPUAdapterInfo.prototype;

ObjectDefineProperty(GPUSupportedFeatures, customInspect, {
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
const GPUSupportedFeaturesPrototype = GPUSupportedFeatures.prototype;
webidl.setlikeObjectWrap(GPUSupportedFeaturesPrototype, true);

ObjectDefineProperty(GPUDeviceLostInfo, customInspect, {
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
const GPUDeviceLostInfoPrototype = GPUDeviceLostInfo.prototype;

ObjectDefineProperty(GPUDevice, customInspect, {
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
const GPUDevicePrototype = GPUDevice.prototype;
ObjectSetPrototypeOf(GPUDevicePrototype, EventTargetPrototype);
defineEventHandler(GPUDevice.prototype, "uncapturederror");

ObjectDefineProperty(GPUQueue, customInspect, {
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
const GPUQueuePrototype = GPUQueue.prototype;

ObjectDefineProperty(GPUBuffer, customInspect, {
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
const GPUBufferPrototype = GPUBuffer.prototype;

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

ObjectDefineProperty(GPUTexture, customInspect, {
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
const GPUTexturePrototype = GPUTexture.prototype;

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

ObjectDefineProperty(GPUTextureView, customInspect, {
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
const GPUTextureViewPrototype = GPUTextureView.prototype;

ObjectDefineProperty(GPUSampler, customInspect, {
  __proto__: null,
  value(inspect) {
    return `${this.constructor.name} ${
      inspect({
        label: this.label,
      })
    }`;
  },
});

ObjectDefineProperty(GPUBindGroupLayout, customInspect, {
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
const GPUBindGroupLayoutPrototype = GPUBindGroupLayout.prototype;

ObjectDefineProperty(GPUPipelineLayout, customInspect, {
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
const GPUPipelineLayoutPrototype = GPUPipelineLayout.prototype;

ObjectDefineProperty(GPUBindGroup, customInspect, {
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
const GPUBindGroupPrototype = GPUBindGroup.prototype;

ObjectDefineProperty(GPUShaderModule, customInspect, {
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
const GPUShaderModulePrototype = GPUShaderModule.prototype;

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

ObjectDefineProperty(GPUComputePipeline, customInspect, {
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
const GPUComputePipelinePrototype = GPUComputePipeline.prototype;

ObjectDefineProperty(GPURenderPipeline, customInspect, {
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
const GPURenderPipelinePrototype = GPURenderPipeline.prototype;

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

ObjectDefineProperty(GPUCommandEncoder, customInspect, {
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
const GPUCommandEncoderPrototype = GPUCommandEncoder.prototype;

ObjectDefineProperty(GPURenderPassEncoder, customInspect, {
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
const GPURenderPassEncoderPrototype = GPURenderPassEncoder.prototype;

ObjectDefineProperty(GPUComputePassEncoder, customInspect, {
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
const GPUComputePassEncoderPrototype = GPUComputePassEncoder.prototype;

ObjectDefineProperty(GPUCommandBuffer, customInspect, {
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
const GPUCommandBufferPrototype = GPUCommandBuffer.prototype;

ObjectDefineProperty(GPURenderBundleEncoder, customInspect, {
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
const GPURenderBundleEncoderPrototype = GPURenderBundleEncoder.prototype;

ObjectDefineProperty(GPURenderBundle, customInspect, {
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
const GPURenderBundlePrototype = GPURenderBundle.prototype;

ObjectDefineProperty(GPUQuerySet, customInspect, {
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
const GPUQuerySetPrototype = GPUQuerySet.prototype;

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
