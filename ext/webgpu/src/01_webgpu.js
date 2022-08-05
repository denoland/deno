// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

// @ts-check
/// <reference path="../../core/lib.deno_core.d.ts" />
/// <reference path="../web/internal.d.ts" />
/// <reference path="../web/lib.deno_web.d.ts" />
/// <reference path="./lib.deno_webgpu.d.ts" />

"use strict";

((window) => {
  const core = window.Deno.core;
  const webidl = window.__bootstrap.webidl;
  const eventTarget = window.__bootstrap.eventTarget;
  const { DOMException } = window.__bootstrap.domException;
  const {
    ArrayBuffer,
    ArrayBufferIsView,
    ArrayIsArray,
    ArrayPrototypeFilter,
    ArrayPrototypeMap,
    ArrayPrototypePop,
    ArrayPrototypePush,
    Error,
    MathMax,
    ObjectDefineProperty,
    ObjectFreeze,
    ObjectPrototypeIsPrototypeOf,
    Promise,
    PromiseAll,
    PromisePrototypeCatch,
    PromisePrototypeThen,
    PromiseReject,
    PromiseResolve,
    SafeArrayIterator,
    Set,
    SetPrototypeEntries,
    SetPrototypeForEach,
    SetPrototypeHas,
    SetPrototypeKeys,
    SetPrototypeValues,
    Symbol,
    SymbolFor,
    SymbolIterator,
    TypeError,
    Uint32Array,
    Uint32ArrayPrototype,
    Uint8Array,
  } = window.__bootstrap.primordials;

  const _rid = Symbol("[[rid]]");
  const _size = Symbol("[[size]]");
  const _usage = Symbol("[[usage]]");
  const _state = Symbol("[[state]]");
  const _mappingRange = Symbol("[[mapping_range]]");
  const _mappedRanges = Symbol("[[mapped_ranges]]");
  const _mapMode = Symbol("[[map_mode]]");
  const _adapter = Symbol("[[adapter]]");
  const _cleanup = Symbol("[[cleanup]]");
  const _vendor = Symbol("[[vendor]]");
  const _architecture = Symbol("[[architecture]]");
  const _description = Symbol("[[description]]");
  const _limits = Symbol("[[limits]]");
  const _features = Symbol("[[features]]");
  const _reason = Symbol("[[reason]]");
  const _message = Symbol("[[message]]");
  const _label = Symbol("[[label]]");
  const _device = Symbol("[[device]]");
  const _queue = Symbol("[[queue]]");
  const _views = Symbol("[[views]]");
  const _texture = Symbol("[[texture]]");
  const _encoders = Symbol("[[encoders]]");
  const _encoder = Symbol("[[encoder]]");
  const _descriptor = Symbol("[[descriptor]]");

  /**
   * @param {any} self
   * @param {{prefix: string, context: string}} opts
   * @returns {InnerGPUDevice & {rid: number}}
   */
  function assertDevice(self, { prefix, context }) {
    const device = self[_device];
    const deviceRid = device?.rid;
    if (deviceRid === undefined) {
      throw new DOMException(
        `${prefix}: ${context} references an invalid or destroyed device.`,
        "OperationError",
      );
    }
    return device;
  }

  /**
   * @param {InnerGPUDevice} self
   * @param {any} resource
   * @param {{prefix: string, resourceContext: string, selfContext: string}} opts
   * @returns {InnerGPUDevice & {rid: number}}
   */
  function assertDeviceMatch(
    self,
    resource,
    { prefix, resourceContext, selfContext },
  ) {
    const resourceDevice = assertDevice(resource, {
      prefix,
      context: resourceContext,
    });
    if (resourceDevice.rid !== self.rid) {
      throw new DOMException(
        `${prefix}: ${resourceContext} belongs to a diffent device than ${selfContext}.`,
        "OperationError",
      );
    }
    return { ...resourceDevice, rid: resourceDevice.rid };
  }

  /**
   * @param {any} self
   * @param {{prefix: string, context: string}} opts
   * @returns {number}
   */
  function assertResource(self, { prefix, context }) {
    const rid = self[_rid];
    if (rid === undefined) {
      throw new DOMException(
        `${prefix}: ${context} an invalid or destroyed resource.`,
        "OperationError",
      );
    }
    return rid;
  }

  /**
   * @param {number[] | GPUExtent3DDict} data
   * @returns {GPUExtent3DDict}
   */
  function normalizeGPUExtent3D(data) {
    if (ArrayIsArray(data)) {
      return {
        width: data[0],
        height: data[1],
        depthOrArrayLayers: data[2],
      };
    } else {
      return data;
    }
  }

  /**
   * @param {number[] | GPUOrigin3DDict} data
   * @returns {GPUOrigin3DDict}
   */
  function normalizeGPUOrigin3D(data) {
    if (ArrayIsArray(data)) {
      return {
        x: data[0],
        y: data[1],
        z: data[2],
      };
    } else {
      return data;
    }
  }

  /**
   * @param {number[] | GPUColor} data
   * @returns {GPUColor}
   */
  function normalizeGPUColor(data) {
    if (ArrayIsArray(data)) {
      return {
        r: data[0],
        g: data[1],
        b: data[2],
        a: data[3],
      };
    } else {
      return data;
    }
  }

  class GPUError extends Error {
    constructor() {
      super();
      webidl.illegalConstructor();
    }

    [_message];
    get message() {
      webidl.assertBranded(this, GPUErrorPrototype);
      return this[_message];
    }
  }
  const GPUErrorPrototype = GPUError.prototype;

  class GPUOutOfMemoryError extends GPUError {
    name = "GPUOutOfMemoryError";
    constructor(message) {
      const prefix = "Failed to construct 'GPUOutOfMemoryError'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      message = webidl.converters.DOMString(message, {
        prefix,
        context: "Argument 1",
      });
      super(message);
    }
  }
  const GPUOutOfMemoryErrorPrototype = GPUOutOfMemoryError.prototype;

  class GPUValidationError extends GPUError {
    name = "GPUValidationError";
    /** @param {string} message */
    constructor(message) {
      const prefix = "Failed to construct 'GPUValidationError'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      message = webidl.converters.DOMString(message, {
        prefix,
        context: "Argument 1",
      });
      super(message);
    }
  }
  const GPUValidationErrorPrototype = GPUValidationError.prototype;

  class GPU {
    [webidl.brand] = webidl.brand;

    constructor() {
      webidl.illegalConstructor();
    }

    /**
     * @param {GPURequestAdapterOptions} options
     */
    async requestAdapter(options = {}) {
      webidl.assertBranded(this, GPUPrototype);
      options = webidl.converters.GPURequestAdapterOptions(options, {
        prefix: "Failed to execute 'requestAdapter' on 'GPU'",
        context: "Argument 1",
      });

      const { err, ...data } = await core.opAsync(
        "op_webgpu_request_adapter",
        options.powerPreference,
        options.forceFallbackAdapter,
      );

      if (err) {
        return null;
      } else {
        return createGPUAdapter(data);
      }
    }

    [SymbolFor("Deno.privateCustomInspect")](inspect) {
      return `${this.constructor.name} ${inspect({})}`;
    }
  }
  const GPUPrototype = GPU.prototype;

  /**
   * @typedef InnerGPUAdapter
   * @property {number} rid
   * @property {GPUSupportedFeatures} features
   * @property {GPUSupportedLimits} limits
   * @property {boolean} isFallbackAdapter
   */

  /**
   * @param {InnerGPUAdapter} inner
   * @returns {GPUAdapter}
   */
  function createGPUAdapter(inner) {
    /** @type {GPUAdapter} */
    const adapter = webidl.createBranded(GPUAdapter);
    adapter[_adapter] = {
      ...inner,
      features: createGPUSupportedFeatures(inner.features),
      limits: createGPUSupportedLimits(inner.limits),
    };
    return adapter;
  }

  class GPUAdapter {
    /** @type {InnerGPUAdapter} */
    [_adapter];

    /** @returns {GPUSupportedFeatures} */
    get features() {
      webidl.assertBranded(this, GPUAdapterPrototype);
      return this[_adapter].features;
    }
    /** @returns {GPUSupportedLimits} */
    get limits() {
      webidl.assertBranded(this, GPUAdapterPrototype);
      return this[_adapter].limits;
    }
    /** @returns {boolean} */
    get isFallbackAdapter() {
      return this[_adapter].isFallbackAdapter;
    }

    constructor() {
      webidl.illegalConstructor();
    }

    /**
     * @param {GPUDeviceDescriptor} descriptor
     * @returns {Promise<GPUDevice>}
     */
    async requestDevice(descriptor = {}) {
      webidl.assertBranded(this, GPUAdapterPrototype);
      const prefix = "Failed to execute 'requestDevice' on 'GPUAdapter'";
      descriptor = webidl.converters.GPUDeviceDescriptor(descriptor, {
        prefix,
        context: "Argument 1",
      });
      const requiredFeatures = descriptor.requiredFeatures ?? [];
      for (const feature of requiredFeatures) {
        if (!SetPrototypeHas(this[_adapter].features[_features], feature)) {
          throw new TypeError(
            `${prefix}: nonGuaranteedFeatures must be a subset of the adapter features.`,
          );
        }
      }
      let requiredLimits = descriptor.requiredLimits;
      if (requiredLimits) {
        requiredLimits = {
          ...this[_adapter].limits[_limits],
          ...requiredLimits,
        };
      }
      // TODO(lucacasonato): validate requiredLimits

      const { rid, features, limits } = await core.opAsync(
        "op_webgpu_request_device",
        this[_adapter].rid,
        descriptor.label,
        requiredFeatures,
        requiredLimits,
      );

      const inner = new InnerGPUDevice({
        rid,
        adapter: this,
        features: ObjectFreeze(features),
        limits: ObjectFreeze(limits),
      });
      return createGPUDevice(
        descriptor.label ?? null,
        inner,
        createGPUQueue(descriptor.label ?? null, inner),
      );
    }

    /**
     * @param {string[]} unmaskHints
     * @returns {Promise<GPUAdapterInfo>}
     */
    async requestAdapterInfo(unmaskHints = []) {
      webidl.assertBranded(this, GPUAdapterPrototype);
      const prefix = "Failed to execute 'requestAdapterInfo' on 'GPUAdapter'";
      unmaskHints = webidl.converters["sequence<DOMString>"](unmaskHints, {
        prefix,
        context: "Argument 1",
      });

      const {
        vendor,
        architecture,
        device,
        description,
      } = await core.opAsync(
        "op_webgpu_request_adapter_info",
        this[_adapter].rid,
      );

      const adapterInfo = webidl.createBranded(GPUAdapterInfo);
      adapterInfo[_vendor] = unmaskHints.includes("vendor") ? vendor : "";
      adapterInfo[_architecture] = unmaskHints.includes("architecture")
        ? architecture
        : "";
      adapterInfo[_device] = unmaskHints.includes("device") ? device : "";
      adapterInfo[_description] = unmaskHints.includes("description")
        ? description
        : "";
      return adapterInfo;
    }

    [SymbolFor("Deno.privateCustomInspect")](inspect) {
      return `${this.constructor.name} ${
        inspect({
          features: this.features,
          limits: this.limits,
        })
      }`;
    }
  }
  const GPUAdapterPrototype = GPUAdapter.prototype;

  class GPUAdapterInfo {
    /** @type {string} */
    [_vendor];
    /** @returns {string} */
    get vendor() {
      webidl.assertBranded(this, GPUAdapterInfoPrototype);
      return this[_vendor];
    }

    /** @type {string} */
    [_architecture];
    /** @returns {string} */
    get architecture() {
      webidl.assertBranded(this, GPUAdapterInfoPrototype);
      return this[_architecture];
    }

    /** @type {string} */
    [_device];
    /** @returns {string} */
    get device() {
      webidl.assertBranded(this, GPUAdapterInfoPrototype);
      return this[_device];
    }

    /** @type {string} */
    [_description];
    /** @returns {string} */
    get description() {
      webidl.assertBranded(this, GPUAdapterInfoPrototype);
      return this[_description];
    }

    [SymbolFor("Deno.privateCustomInspect")](inspect) {
      return `${this.constructor.name} ${
        inspect({
          vendor: this.vendor,
          architecture: this.architecture,
          device: this.device,
          description: this.description,
        })
      }`;
    }
  }
  const GPUAdapterInfoPrototype = GPUAdapterInfo.prototype;

  function createGPUSupportedLimits(features) {
    /** @type {GPUSupportedLimits} */
    const adapterFeatures = webidl.createBranded(GPUSupportedLimits);
    adapterFeatures[_limits] = features;
    return adapterFeatures;
  }

  /**
   * @typedef InnerAdapterLimits
   * @property {number} maxTextureDimension1D
   * @property {number} maxTextureDimension2D
   * @property {number} maxTextureDimension3D
   * @property {number} maxTextureArrayLayers
   * @property {number} maxBindGroups
   * @property {number} maxDynamicUniformBuffersPerPipelineLayout
   * @property {number} maxDynamicStorageBuffersPerPipelineLayout
   * @property {number} maxSampledTexturesPerShaderStage
   * @property {number} maxSamplersPerShaderStage
   * @property {number} maxStorageBuffersPerShaderStage
   * @property {number} maxStorageTexturesPerShaderStage
   * @property {number} maxUniformBuffersPerShaderStage
   * @property {number} maxUniformBufferBindingSize
   * @property {number} maxStorageBufferBindingSize
   * @property {number} minUniformBufferOffsetAlignment
   * @property {number} minStorageBufferOffsetAlignment
   * @property {number} maxVertexBuffers
   * @property {number} maxVertexAttributes
   * @property {number} maxVertexBufferArrayStride
   * @property {number} maxInterStageShaderComponents
   * @property {number} maxComputeWorkgroupStorageSize
   * @property {number} maxComputeInvocationsPerWorkgroup
   * @property {number} maxComputeWorkgroupSizeX
   * @property {number} maxComputeWorkgroupSizeY
   * @property {number} maxComputeWorkgroupSizeZ
   * @property {number} maxComputeWorkgroupsPerDimension
   */

  class GPUSupportedLimits {
    /** @type {InnerAdapterLimits} */
    [_limits];
    constructor() {
      webidl.illegalConstructor();
    }

    get maxTextureDimension1D() {
      webidl.assertBranded(this, GPUSupportedLimitsPrototype);
      return this[_limits].maxTextureDimension1D;
    }
    get maxTextureDimension2D() {
      webidl.assertBranded(this, GPUSupportedLimitsPrototype);
      return this[_limits].maxTextureDimension2D;
    }
    get maxTextureDimension3D() {
      webidl.assertBranded(this, GPUSupportedLimitsPrototype);
      return this[_limits].maxTextureDimension3D;
    }
    get maxTextureArrayLayers() {
      webidl.assertBranded(this, GPUSupportedLimitsPrototype);
      return this[_limits].maxTextureArrayLayers;
    }
    get maxBindGroups() {
      webidl.assertBranded(this, GPUSupportedLimitsPrototype);
      return this[_limits].maxBindGroups;
    }
    get maxDynamicUniformBuffersPerPipelineLayout() {
      webidl.assertBranded(this, GPUSupportedLimitsPrototype);
      return this[_limits].maxDynamicUniformBuffersPerPipelineLayout;
    }
    get maxDynamicStorageBuffersPerPipelineLayout() {
      webidl.assertBranded(this, GPUSupportedLimitsPrototype);
      return this[_limits].maxDynamicStorageBuffersPerPipelineLayout;
    }
    get maxSampledTexturesPerShaderStage() {
      webidl.assertBranded(this, GPUSupportedLimitsPrototype);
      return this[_limits].maxSampledTexturesPerShaderStage;
    }
    get maxSamplersPerShaderStage() {
      webidl.assertBranded(this, GPUSupportedLimitsPrototype);
      return this[_limits].maxSamplersPerShaderStage;
    }
    get maxStorageBuffersPerShaderStage() {
      webidl.assertBranded(this, GPUSupportedLimitsPrototype);
      return this[_limits].maxStorageBuffersPerShaderStage;
    }
    get maxStorageTexturesPerShaderStage() {
      webidl.assertBranded(this, GPUSupportedLimitsPrototype);
      return this[_limits].maxStorageTexturesPerShaderStage;
    }
    get maxUniformBuffersPerShaderStage() {
      webidl.assertBranded(this, GPUSupportedLimitsPrototype);
      return this[_limits].maxUniformBuffersPerShaderStage;
    }
    get maxUniformBufferBindingSize() {
      webidl.assertBranded(this, GPUSupportedLimitsPrototype);
      return this[_limits].maxUniformBufferBindingSize;
    }
    get maxStorageBufferBindingSize() {
      webidl.assertBranded(this, GPUSupportedLimitsPrototype);
      return this[_limits].maxStorageBufferBindingSize;
    }
    get minUniformBufferOffsetAlignment() {
      webidl.assertBranded(this, GPUSupportedLimitsPrototype);
      return this[_limits].minUniformBufferOffsetAlignment;
    }
    get minStorageBufferOffsetAlignment() {
      webidl.assertBranded(this, GPUSupportedLimitsPrototype);
      return this[_limits].minStorageBufferOffsetAlignment;
    }
    get maxVertexBuffers() {
      webidl.assertBranded(this, GPUSupportedLimitsPrototype);
      return this[_limits].maxVertexBuffers;
    }
    get maxVertexAttributes() {
      webidl.assertBranded(this, GPUSupportedLimitsPrototype);
      return this[_limits].maxVertexAttributes;
    }
    get maxVertexBufferArrayStride() {
      webidl.assertBranded(this, GPUSupportedLimitsPrototype);
      return this[_limits].maxVertexBufferArrayStride;
    }
    get maxInterStageShaderComponents() {
      webidl.assertBranded(this, GPUSupportedLimitsPrototype);
      return this[_limits].maxInterStageShaderComponents;
    }
    get maxComputeWorkgroupStorageSize() {
      webidl.assertBranded(this, GPUSupportedLimitsPrototype);
      return this[_limits].maxComputeWorkgroupStorageSize;
    }
    get maxComputeInvocationsPerWorkgroup() {
      webidl.assertBranded(this, GPUSupportedLimitsPrototype);
      return this[_limits].maxComputeInvocationsPerWorkgroup;
    }
    get maxComputeWorkgroupSizeX() {
      webidl.assertBranded(this, GPUSupportedLimitsPrototype);
      return this[_limits].maxComputeWorkgroupSizeX;
    }
    get maxComputeWorkgroupSizeY() {
      webidl.assertBranded(this, GPUSupportedLimitsPrototype);
      return this[_limits].maxComputeWorkgroupSizeY;
    }
    get maxComputeWorkgroupSizeZ() {
      webidl.assertBranded(this, GPUSupportedLimitsPrototype);
      return this[_limits].maxComputeWorkgroupSizeZ;
    }
    get maxComputeWorkgroupsPerDimension() {
      webidl.assertBranded(this, GPUSupportedLimitsPrototype);
      return this[_limits].maxComputeWorkgroupsPerDimension;
    }

    [SymbolFor("Deno.privateCustomInspect")](inspect) {
      return `${this.constructor.name} ${inspect(this[_limits])}`;
    }
  }
  const GPUSupportedLimitsPrototype = GPUSupportedLimits.prototype;

  function createGPUSupportedFeatures(features) {
    /** @type {GPUSupportedFeatures} */
    const adapterFeatures = webidl.createBranded(GPUSupportedFeatures);
    adapterFeatures[_features] = new Set(features);
    return adapterFeatures;
  }

  class GPUSupportedFeatures {
    /** @type {Set<string>} */
    [_features];

    constructor() {
      webidl.illegalConstructor();
    }

    /** @return {IterableIterator<[string, string]>} */
    entries() {
      webidl.assertBranded(this, GPUSupportedFeaturesPrototype);
      return SetPrototypeEntries(this[_features]);
    }

    /** @return {void} */
    forEach(callbackfn, thisArg) {
      webidl.assertBranded(this, GPUSupportedFeaturesPrototype);
      SetPrototypeForEach(this[_features], callbackfn, thisArg);
    }

    /** @return {boolean} */
    has(value) {
      webidl.assertBranded(this, GPUSupportedFeaturesPrototype);
      return SetPrototypeHas(this[_features], value);
    }

    /** @return {IterableIterator<string>} */
    keys() {
      webidl.assertBranded(this, GPUSupportedFeaturesPrototype);
      return SetPrototypeKeys(this[_features]);
    }

    /** @return {IterableIterator<string>} */
    values() {
      webidl.assertBranded(this, GPUSupportedFeaturesPrototype);
      return SetPrototypeValues(this[_features]);
    }

    /** @return {number} */
    get size() {
      webidl.assertBranded(this, GPUSupportedFeaturesPrototype);
      return this[_features].size;
    }

    [SymbolIterator]() {
      webidl.assertBranded(this, GPUSupportedFeaturesPrototype);
      return this[_features][SymbolIterator]();
    }

    [SymbolFor("Deno.privateCustomInspect")](inspect) {
      return `${this.constructor.name} ${
        inspect([...new SafeArrayIterator(this.values())])
      }`;
    }
  }

  const GPUSupportedFeaturesPrototype = GPUSupportedFeatures.prototype;

  /**
   * @param {string | undefined} reason
   * @param {string} message
   * @returns {GPUDeviceLostInfo}
   */
  function createGPUDeviceLostInfo(reason, message) {
    /** @type {GPUDeviceLostInfo} */
    const deviceLostInfo = webidl.createBranded(GPUDeviceLostInfo);
    deviceLostInfo[_reason] = reason;
    deviceLostInfo[_message] = message;
    return deviceLostInfo;
  }

  class GPUDeviceLostInfo {
    /** @type {string | undefined} */
    [_reason];
    /** @type {string} */
    [_message];

    constructor() {
      webidl.illegalConstructor();
    }

    get reason() {
      webidl.assertBranded(this, GPUDeviceLostInfoPrototype);
      return this[_reason];
    }
    get message() {
      webidl.assertBranded(this, GPUDeviceLostInfoPrototype);
      return this[_message];
    }

    [SymbolFor("Deno.privateCustomInspect")](inspect) {
      return `${this.constructor.name} ${
        inspect({ reason: this[_reason], message: this[_message] })
      }`;
    }
  }

  const GPUDeviceLostInfoPrototype = GPUDeviceLostInfo.prototype;

  /**
   * @param {string} name
   * @param {any} type
   */
  function GPUObjectBaseMixin(name, type) {
    type.prototype[_label] = null;
    ObjectDefineProperty(type.prototype, "label", {
      /**
       * @return {string | null}
       */
      get() {
        webidl.assertBranded(this, type.prototype);
        return this[_label];
      },
      /**
       * @param {string | null} label
       */
      set(label) {
        webidl.assertBranded(this, type.prototype);
        label = webidl.converters["UVString?"](label, {
          prefix: `Failed to set 'label' on '${name}'`,
          context: "Argument 1",
        });
        this[_label] = label;
      },
    });
  }

  /**
   * @typedef ErrorScope
   * @property {string} filter
   * @property {Promise<void>[]} operations
   */

  /**
   * @typedef InnerGPUDeviceOptions
   * @property {GPUAdapter} adapter
   * @property {number | undefined} rid
   * @property {GPUFeatureName[]} features
   * @property {object} limits
   */

  class InnerGPUDevice {
    /** @type {GPUAdapter} */
    adapter;
    /** @type {number | undefined} */
    rid;
    /** @type {GPUFeatureName[]} */
    features;
    /** @type {object} */
    limits;
    /** @type {WeakRef<any>[]} */
    resources;
    /** @type {boolean} */
    isLost;
    /** @type {Promise<GPUDeviceLostInfo>} */
    lost;
    /** @type {(info: GPUDeviceLostInfo) => void} */
    resolveLost;
    /** @type {ErrorScope[]} */
    errorScopeStack;

    /**
     * @param {InnerGPUDeviceOptions} options
     */
    constructor(options) {
      this.adapter = options.adapter;
      this.rid = options.rid;
      this.features = options.features;
      this.limits = options.limits;
      this.resources = [];
      this.isLost = false;
      this.resolveLost = () => {};
      this.lost = new Promise((resolve) => {
        this.resolveLost = resolve;
      });
      this.errorScopeStack = [];
    }

    /** @param {any} resource */
    trackResource(resource) {
      ArrayPrototypePush(this.resources, new WeakRef(resource));
    }

    /** @param {{ type: string, value: string | null } | undefined} err */
    pushError(err) {
      this.pushErrorPromise(PromiseResolve(err));
    }

    /** @param {Promise<{ type: string, value: string | null } | undefined>} promise */
    pushErrorPromise(promise) {
      const operation = PromisePrototypeThen(promise, (err) => {
        if (err) {
          switch (err.type) {
            case "lost":
              this.isLost = true;
              this.resolveLost(
                createGPUDeviceLostInfo(undefined, "device was lost"),
              );
              break;
            case "validation":
              return PromiseReject(
                new GPUValidationError(err.value ?? "validation error"),
              );
            case "out-of-memory":
              return PromiseReject(new GPUOutOfMemoryError());
          }
        }
      });

      const validationStack = ArrayPrototypeFilter(
        this.errorScopeStack,
        ({ filter }) => filter == "validation",
      );
      const validationScope = validationStack[validationStack.length - 1];
      const validationFilteredPromise = PromisePrototypeCatch(
        operation,
        (err) => {
          if (ObjectPrototypeIsPrototypeOf(GPUValidationErrorPrototype, err)) {
            return PromiseReject(err);
          }
          return PromiseResolve();
        },
      );
      if (validationScope) {
        ArrayPrototypePush(
          validationScope.operations,
          validationFilteredPromise,
        );
      } else {
        PromisePrototypeCatch(validationFilteredPromise, () => {
          // TODO(lucacasonato): emit an UncapturedErrorEvent
        });
      }
      // prevent uncaptured promise rejections
      PromisePrototypeCatch(validationFilteredPromise, (_err) => {});

      const oomStack = ArrayPrototypeFilter(
        this.errorScopeStack,
        ({ filter }) => filter == "out-of-memory",
      );
      const oomScope = oomStack[oomStack.length - 1];
      const oomFilteredPromise = PromisePrototypeCatch(operation, (err) => {
        if (ObjectPrototypeIsPrototypeOf(GPUOutOfMemoryErrorPrototype, err)) {
          return PromiseReject(err);
        }
        return PromiseResolve();
      });
      if (oomScope) {
        ArrayPrototypePush(oomScope.operations, oomFilteredPromise);
      } else {
        PromisePrototypeCatch(oomFilteredPromise, () => {
          // TODO(lucacasonato): emit an UncapturedErrorEvent
        });
      }
      // prevent uncaptured promise rejections
      PromisePrototypeCatch(oomFilteredPromise, (_err) => {});
    }
  }

  /**
   * @param {string | null} label
   * @param {InnerGPUDevice} inner
   * @param {GPUQueue} queue
   * @returns {GPUDevice}
   */
  function createGPUDevice(label, inner, queue) {
    /** @type {GPUDevice} */
    const device = webidl.createBranded(GPUDevice);
    device[_label] = label;
    device[_device] = inner;
    device[_queue] = queue;
    return device;
  }

  class GPUDevice extends eventTarget.EventTarget {
    /** @type {InnerGPUDevice} */
    [_device];

    /** @type {GPUQueue} */
    [_queue];

    [_cleanup]() {
      const device = this[_device];
      const resources = device.resources;
      while (resources.length > 0) {
        const resource = ArrayPrototypePop(resources)?.deref();
        if (resource) {
          resource[_cleanup]();
        }
      }
      const rid = device.rid;
      if (rid !== undefined) {
        core.close(rid);
        /** @type {number | undefined} */
        device.rid = undefined;
      }
    }

    get features() {
      webidl.assertBranded(this, GPUDevicePrototype);
      return this[_device].features;
    }
    get limits() {
      webidl.assertBranded(this, GPUDevicePrototype);
      return this[_device].limits;
    }
    get queue() {
      webidl.assertBranded(this, GPUDevicePrototype);
      return this[_queue];
    }

    constructor() {
      webidl.illegalConstructor();
      super();
    }

    destroy() {
      webidl.assertBranded(this, GPUDevicePrototype);
      this[_cleanup]();
    }

    /**
     * @param {GPUBufferDescriptor} descriptor
     * @returns {GPUBuffer}
     */
    createBuffer(descriptor) {
      webidl.assertBranded(this, GPUDevicePrototype);
      const prefix = "Failed to execute 'createBuffer' on 'GPUDevice'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      descriptor = webidl.converters.GPUBufferDescriptor(descriptor, {
        prefix,
        context: "Argument 1",
      });
      const device = assertDevice(this, { prefix, context: "this" });
      const { rid, err } = core.opSync(
        "op_webgpu_create_buffer",
        device.rid,
        descriptor.label,
        descriptor.size,
        descriptor.usage,
        descriptor.mappedAtCreation,
      );
      device.pushError(err);
      /** @type {CreateGPUBufferOptions} */
      let options;
      if (descriptor.mappedAtCreation) {
        options = {
          mapping: new ArrayBuffer(descriptor.size),
          mappingRange: [0, descriptor.size],
          mappedRanges: [],
          state: "mapped at creation",
        };
      } else {
        options = {
          mapping: null,
          mappedRanges: null,
          mappingRange: null,
          state: "unmapped",
        };
      }
      const buffer = createGPUBuffer(
        descriptor.label ?? null,
        device,
        rid,
        descriptor.size,
        descriptor.usage,
        options,
      );
      device.trackResource(buffer);
      return buffer;
    }

    /**
     * @param {GPUTextureDescriptor} descriptor
     * @returns {GPUTexture}
     */
    createTexture(descriptor) {
      webidl.assertBranded(this, GPUDevicePrototype);
      const prefix = "Failed to execute 'createTexture' on 'GPUDevice'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      descriptor = webidl.converters.GPUTextureDescriptor(descriptor, {
        prefix,
        context: "Argument 1",
      });
      const device = assertDevice(this, { prefix, context: "this" });
      const { rid, err } = core.opSync("op_webgpu_create_texture", {
        deviceRid: device.rid,
        ...descriptor,
        size: normalizeGPUExtent3D(descriptor.size),
      });
      device.pushError(err);

      const texture = createGPUTexture(
        descriptor.label ?? null,
        device,
        rid,
      );
      device.trackResource(texture);
      return texture;
    }

    /**
     * @param {GPUSamplerDescriptor} descriptor
     * @returns {GPUSampler}
     */
    createSampler(descriptor = {}) {
      webidl.assertBranded(this, GPUDevicePrototype);
      const prefix = "Failed to execute 'createSampler' on 'GPUDevice'";
      descriptor = webidl.converters.GPUSamplerDescriptor(descriptor, {
        prefix,
        context: "Argument 1",
      });
      const device = assertDevice(this, { prefix, context: "this" });
      const { rid, err } = core.opSync("op_webgpu_create_sampler", {
        deviceRid: device.rid,
        ...descriptor,
      });
      device.pushError(err);

      const sampler = createGPUSampler(
        descriptor.label ?? null,
        device,
        rid,
      );
      device.trackResource(sampler);
      return sampler;
    }

    /**
     * @param {GPUBindGroupLayoutDescriptor} descriptor
     * @returns {GPUBindGroupLayout}
     */
    createBindGroupLayout(descriptor) {
      webidl.assertBranded(this, GPUDevicePrototype);
      const prefix = "Failed to execute 'createBindGroupLayout' on 'GPUDevice'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      descriptor = webidl.converters.GPUBindGroupLayoutDescriptor(descriptor, {
        prefix,
        context: "Argument 1",
      });
      const device = assertDevice(this, { prefix, context: "this" });
      for (const entry of descriptor.entries) {
        let i = 0;
        if (entry.buffer) i++;
        if (entry.sampler) i++;
        if (entry.texture) i++;
        if (entry.storageTexture) i++;

        if (i !== 1) {
          throw new Error(); // TODO(@crowlKats): correct error
        }
      }

      const { rid, err } = core.opSync(
        "op_webgpu_create_bind_group_layout",
        device.rid,
        descriptor.label,
        descriptor.entries,
      );
      device.pushError(err);

      const bindGroupLayout = createGPUBindGroupLayout(
        descriptor.label ?? null,
        device,
        rid,
      );
      device.trackResource(bindGroupLayout);
      return bindGroupLayout;
    }

    /**
     * @param {GPUPipelineLayoutDescriptor} descriptor
     * @returns {GPUPipelineLayout}
     */
    createPipelineLayout(descriptor) {
      webidl.assertBranded(this, GPUDevicePrototype);
      const prefix = "Failed to execute 'createPipelineLayout' on 'GPUDevice'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      descriptor = webidl.converters.GPUPipelineLayoutDescriptor(descriptor, {
        prefix,
        context: "Argument 1",
      });
      const device = assertDevice(this, { prefix, context: "this" });
      const bindGroupLayouts = ArrayPrototypeMap(
        descriptor.bindGroupLayouts,
        (layout, i) => {
          const context = `bind group layout ${i + 1}`;
          const rid = assertResource(layout, { prefix, context });
          assertDeviceMatch(device, layout, {
            prefix,
            selfContext: "this",
            resourceContext: context,
          });
          return rid;
        },
      );
      const { rid, err } = core.opSync(
        "op_webgpu_create_pipeline_layout",
        device.rid,
        descriptor.label,
        bindGroupLayouts,
      );
      device.pushError(err);

      const pipelineLayout = createGPUPipelineLayout(
        descriptor.label ?? null,
        device,
        rid,
      );
      device.trackResource(pipelineLayout);
      return pipelineLayout;
    }

    /**
     * @param {GPUBindGroupDescriptor} descriptor
     * @returns {GPUBindGroup}
     */
    createBindGroup(descriptor) {
      webidl.assertBranded(this, GPUDevicePrototype);
      const prefix = "Failed to execute 'createBindGroup' on 'GPUDevice'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      descriptor = webidl.converters.GPUBindGroupDescriptor(descriptor, {
        prefix,
        context: "Argument 1",
      });
      const device = assertDevice(this, { prefix, context: "this" });
      const layout = assertResource(descriptor.layout, {
        prefix,
        context: "layout",
      });
      assertDeviceMatch(device, descriptor.layout, {
        prefix,
        resourceContext: "layout",
        selfContext: "this",
      });
      const entries = ArrayPrototypeMap(descriptor.entries, (entry, i) => {
        const context = `entry ${i + 1}`;
        const resource = entry.resource;
        if (ObjectPrototypeIsPrototypeOf(GPUSamplerPrototype, resource)) {
          const rid = assertResource(resource, {
            prefix,
            context,
          });
          assertDeviceMatch(device, resource, {
            prefix,
            resourceContext: context,
            selfContext: "this",
          });
          return {
            binding: entry.binding,
            kind: "GPUSampler",
            resource: rid,
          };
        } else if (
          ObjectPrototypeIsPrototypeOf(GPUTextureViewPrototype, resource)
        ) {
          const rid = assertResource(resource, {
            prefix,
            context,
          });
          assertResource(resource[_texture], {
            prefix,
            context,
          });
          assertDeviceMatch(device, resource[_texture], {
            prefix,
            resourceContext: context,
            selfContext: "this",
          });
          return {
            binding: entry.binding,
            kind: "GPUTextureView",
            resource: rid,
          };
        } else {
          const rid = assertResource(resource.buffer, { prefix, context });
          assertDeviceMatch(device, resource.buffer, {
            prefix,
            resourceContext: context,
            selfContext: "this",
          });
          return {
            binding: entry.binding,
            kind: "GPUBufferBinding",
            resource: rid,
            offset: entry.resource.offset,
            size: entry.resource.size,
          };
        }
      });

      const { rid, err } = core.opSync(
        "op_webgpu_create_bind_group",
        device.rid,
        descriptor.label,
        layout,
        entries,
      );
      device.pushError(err);

      const bindGroup = createGPUBindGroup(
        descriptor.label ?? null,
        device,
        rid,
      );
      device.trackResource(bindGroup);
      return bindGroup;
    }

    /**
     * @param {GPUShaderModuleDescriptor} descriptor
     */
    createShaderModule(descriptor) {
      webidl.assertBranded(this, GPUDevicePrototype);
      const prefix = "Failed to execute 'createShaderModule' on 'GPUDevice'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      descriptor = webidl.converters.GPUShaderModuleDescriptor(descriptor, {
        prefix,
        context: "Argument 1",
      });
      const device = assertDevice(this, { prefix, context: "this" });
      const { rid, err } = core.opSync(
        "op_webgpu_create_shader_module",
        device.rid,
        descriptor.label,
        descriptor.code,
        descriptor.sourceMap,
      );
      device.pushError(err);

      const shaderModule = createGPUShaderModule(
        descriptor.label ?? null,
        device,
        rid,
      );
      device.trackResource(shaderModule);
      return shaderModule;
    }

    /**
     * @param {GPUComputePipelineDescriptor} descriptor
     * @returns {GPUComputePipeline}
     */
    createComputePipeline(descriptor) {
      webidl.assertBranded(this, GPUDevicePrototype);
      const prefix = "Failed to execute 'createComputePipeline' on 'GPUDevice'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      descriptor = webidl.converters.GPUComputePipelineDescriptor(descriptor, {
        prefix,
        context: "Argument 1",
      });
      const device = assertDevice(this, { prefix, context: "this" });
      let layout = descriptor.layout;
      if (typeof descriptor.layout !== "string") {
        const context = "layout";
        layout = assertResource(descriptor.layout, { prefix, context });
        assertDeviceMatch(device, descriptor.layout, {
          prefix,
          resourceContext: context,
          selfContext: "this",
        });
      }
      const module = assertResource(descriptor.compute.module, {
        prefix,
        context: "compute shader module",
      });
      assertDeviceMatch(device, descriptor.compute.module, {
        prefix,
        resourceContext: "compute shader module",
        selfContext: "this",
      });

      const { rid, err } = core.opSync(
        "op_webgpu_create_compute_pipeline",
        device.rid,
        descriptor.label,
        layout,
        {
          module,
          entryPoint: descriptor.compute.entryPoint,
          constants: descriptor.compute.constants,
        },
      );
      device.pushError(err);

      const computePipeline = createGPUComputePipeline(
        descriptor.label ?? null,
        device,
        rid,
      );
      device.trackResource(computePipeline);
      return computePipeline;
    }

    /**
     * @param {GPURenderPipelineDescriptor} descriptor
     * @returns {GPURenderPipeline}
     */
    createRenderPipeline(descriptor) {
      webidl.assertBranded(this, GPUDevicePrototype);
      const prefix = "Failed to execute 'createRenderPipeline' on 'GPUDevice'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      descriptor = webidl.converters.GPURenderPipelineDescriptor(descriptor, {
        prefix,
        context: "Argument 1",
      });
      const device = assertDevice(this, { prefix, context: "this" });
      let layout = descriptor.layout;
      if (typeof descriptor.layout !== "string") {
        const context = "layout";
        layout = assertResource(descriptor.layout, { prefix, context });
        assertDeviceMatch(device, descriptor.layout, {
          prefix,
          resourceContext: context,
          selfContext: "this",
        });
      }
      const module = assertResource(descriptor.vertex.module, {
        prefix,
        context: "vertex shader module",
      });
      assertDeviceMatch(device, descriptor.vertex.module, {
        prefix,
        resourceContext: "vertex shader module",
        selfContext: "this",
      });
      let fragment = undefined;
      if (descriptor.fragment) {
        const module = assertResource(descriptor.fragment.module, {
          prefix,
          context: "fragment shader module",
        });
        assertDeviceMatch(device, descriptor.fragment.module, {
          prefix,
          resourceContext: "fragment shader module",
          selfContext: "this",
        });
        fragment = {
          module,
          entryPoint: descriptor.fragment.entryPoint,
          targets: descriptor.fragment.targets,
        };
      }

      const { rid, err } = core.opSync("op_webgpu_create_render_pipeline", {
        deviceRid: device.rid,
        label: descriptor.label,
        layout,
        vertex: {
          module,
          entryPoint: descriptor.vertex.entryPoint,
          buffers: descriptor.vertex.buffers,
        },
        primitive: descriptor.primitive,
        depthStencil: descriptor.depthStencil,
        multisample: descriptor.multisample,
        fragment,
      });
      device.pushError(err);

      const renderPipeline = createGPURenderPipeline(
        descriptor.label ?? null,
        device,
        rid,
      );
      device.trackResource(renderPipeline);
      return renderPipeline;
    }

    createComputePipelineAsync(descriptor) {
      // TODO(lucacasonato): this should be real async
      return PromiseResolve(this.createComputePipeline(descriptor));
    }

    createRenderPipelineAsync(descriptor) {
      // TODO(lucacasonato): this should be real async
      return PromiseResolve(this.createRenderPipeline(descriptor));
    }

    /**
     * @param {GPUCommandEncoderDescriptor} descriptor
     * @returns {GPUCommandEncoder}
     */
    createCommandEncoder(descriptor = {}) {
      webidl.assertBranded(this, GPUDevicePrototype);
      const prefix = "Failed to execute 'createCommandEncoder' on 'GPUDevice'";
      descriptor = webidl.converters.GPUCommandEncoderDescriptor(descriptor, {
        prefix,
        context: "Argument 1",
      });
      const device = assertDevice(this, { prefix, context: "this" });
      const { rid, err } = core.opSync(
        "op_webgpu_create_command_encoder",
        device.rid,
        descriptor.label,
      );
      device.pushError(err);

      const commandEncoder = createGPUCommandEncoder(
        descriptor.label ?? null,
        device,
        rid,
      );
      device.trackResource(commandEncoder);
      return commandEncoder;
    }

    /**
     * @param {GPURenderBundleEncoderDescriptor} descriptor
     * @returns {GPURenderBundleEncoder}
     */
    createRenderBundleEncoder(descriptor) {
      webidl.assertBranded(this, GPUDevicePrototype);
      const prefix =
        "Failed to execute 'createRenderBundleEncoder' on 'GPUDevice'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      descriptor = webidl.converters.GPURenderBundleEncoderDescriptor(
        descriptor,
        {
          prefix,
          context: "Argument 1",
        },
      );
      const device = assertDevice(this, { prefix, context: "this" });
      const { rid, err } = core.opSync(
        "op_webgpu_create_render_bundle_encoder",
        {
          deviceRid: device.rid,
          ...descriptor,
        },
      );
      device.pushError(err);

      const renderBundleEncoder = createGPURenderBundleEncoder(
        descriptor.label ?? null,
        device,
        rid,
      );
      device.trackResource(renderBundleEncoder);
      return renderBundleEncoder;
    }

    /**
     * @param {GPUQuerySetDescriptor} descriptor
     * @returns {GPUQuerySet}
     */
    createQuerySet(descriptor) {
      webidl.assertBranded(this, GPUDevicePrototype);
      const prefix = "Failed to execute 'createQuerySet' on 'GPUDevice'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      descriptor = webidl.converters.GPUQuerySetDescriptor(
        descriptor,
        {
          prefix,
          context: "Argument 1",
        },
      );
      const device = assertDevice(this, { prefix, context: "this" });
      const { rid, err } = core.opSync("op_webgpu_create_query_set", {
        deviceRid: device.rid,
        ...descriptor,
      });
      device.pushError(err);

      const querySet = createGPUQuerySet(
        descriptor.label ?? null,
        device,
        rid,
        descriptor,
      );
      device.trackResource(querySet);
      return querySet;
    }

    get lost() {
      webidl.assertBranded(this, GPUDevicePrototype);
      const device = this[_device];
      if (!device) {
        return PromiseResolve(true);
      }
      if (device.rid === undefined) {
        return PromiseResolve(true);
      }
      return device.lost;
    }

    /**
     * @param {GPUErrorFilter} filter
     */
    pushErrorScope(filter) {
      webidl.assertBranded(this, GPUDevicePrototype);
      const prefix = "Failed to execute 'pushErrorScope' on 'GPUDevice'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      filter = webidl.converters.GPUErrorFilter(filter, {
        prefix,
        context: "Argument 1",
      });
      const device = assertDevice(this, { prefix, context: "this" });
      ArrayPrototypePush(device.errorScopeStack, { filter, operations: [] });
    }

    /**
     * @returns {Promise<GPUError | null>}
     */
    // deno-lint-ignore require-await
    async popErrorScope() {
      webidl.assertBranded(this, GPUDevicePrototype);
      const prefix = "Failed to execute 'popErrorScope' on 'GPUDevice'";
      const device = assertDevice(this, { prefix, context: "this" });
      if (device.isLost) {
        throw new DOMException("Device has been lost.", "OperationError");
      }
      const scope = ArrayPrototypePop(device.errorScopeStack);
      if (!scope) {
        throw new DOMException(
          "There are no error scopes on the error scope stack.",
          "OperationError",
        );
      }
      const operations = PromiseAll(scope.operations);
      return PromisePrototypeThen(
        operations,
        () => PromiseResolve(null),
        (err) => PromiseResolve(err),
      );
    }

    [SymbolFor("Deno.privateCustomInspect")](inspect) {
      return `${this.constructor.name} ${
        inspect({
          features: this.features,
          label: this.label,
          limits: this.limits,
          queue: this.queue,
        })
      }`;
    }
  }
  GPUObjectBaseMixin("GPUDevice", GPUDevice);
  const GPUDevicePrototype = GPUDevice.prototype;

  /**
   * @param {string | null} label
   * @param {InnerGPUDevice} device
   * @returns {GPUQueue}
   */
  function createGPUQueue(label, device) {
    /** @type {GPUQueue} */
    const queue = webidl.createBranded(GPUQueue);
    queue[_label] = label;
    queue[_device] = device;
    return queue;
  }

  class GPUQueue {
    /** @type {InnerGPUDevice} */
    [_device];

    constructor() {
      webidl.illegalConstructor();
    }

    /**
     * @param {GPUCommandBuffer[]} commandBuffers
     */
    submit(commandBuffers) {
      webidl.assertBranded(this, GPUQueue.prototype);
      const prefix = "Failed to execute 'submit' on 'GPUQueue'";
      webidl.requiredArguments(arguments.length, 1, {
        prefix,
      });
      commandBuffers = webidl.converters["sequence<GPUCommandBuffer>"](
        commandBuffers,
        { prefix, context: "Argument 1" },
      );
      const device = assertDevice(this, { prefix, context: "this" });
      const commandBufferRids = ArrayPrototypeMap(
        commandBuffers,
        (buffer, i) => {
          const context = `command buffer ${i + 1}`;
          const rid = assertResource(buffer, { prefix, context });
          assertDeviceMatch(device, buffer, {
            prefix,
            selfContext: "this",
            resourceContext: context,
          });
          return rid;
        },
      );
      const { err } = core.opSync(
        "op_webgpu_queue_submit",
        device.rid,
        commandBufferRids,
      );
      for (const commandBuffer of commandBuffers) {
        commandBuffer[_rid] = undefined;
      }
      device.pushError(err);
    }

    onSubmittedWorkDone() {
      webidl.assertBranded(this, GPUQueue.prototype);
      return PromiseResolve();
    }

    /**
     * @param {GPUBuffer} buffer
     * @param {number} bufferOffset
     * @param {BufferSource} data
     * @param {number} [dataOffset]
     * @param {number} [size]
     */
    writeBuffer(buffer, bufferOffset, data, dataOffset = 0, size) {
      webidl.assertBranded(this, GPUQueue.prototype);
      const prefix = "Failed to execute 'writeBuffer' on 'GPUQueue'";
      webidl.requiredArguments(arguments.length, 3, { prefix });
      buffer = webidl.converters["GPUBuffer"](buffer, {
        prefix,
        context: "Argument 1",
      });
      bufferOffset = webidl.converters["GPUSize64"](bufferOffset, {
        prefix,
        context: "Argument 2",
      });
      data = webidl.converters.BufferSource(data, {
        prefix,
        context: "Argument 3",
      });
      dataOffset = webidl.converters["GPUSize64"](dataOffset, {
        prefix,
        context: "Argument 4",
      });
      size = size === undefined
        ? undefined
        : webidl.converters["GPUSize64"](size, {
          prefix,
          context: "Argument 5",
        });
      const device = assertDevice(this, { prefix, context: "this" });
      const bufferRid = assertResource(buffer, {
        prefix,
        context: "Argument 1",
      });
      assertDeviceMatch(device, buffer, {
        prefix,
        selfContext: "this",
        resourceContext: "Argument 1",
      });
      const { err } = core.opSync(
        "op_webgpu_write_buffer",
        device.rid,
        bufferRid,
        bufferOffset,
        dataOffset,
        size,
        new Uint8Array(ArrayBufferIsView(data) ? data.buffer : data),
      );
      device.pushError(err);
    }

    /**
     * @param {GPUImageCopyTexture} destination
     * @param {BufferSource} data
     * @param {GPUImageDataLayout} dataLayout
     * @param {GPUExtent3D} size
     */
    writeTexture(destination, data, dataLayout, size) {
      webidl.assertBranded(this, GPUQueue.prototype);
      const prefix = "Failed to execute 'writeTexture' on 'GPUQueue'";
      webidl.requiredArguments(arguments.length, 4, { prefix });
      destination = webidl.converters.GPUImageCopyTexture(destination, {
        prefix,
        context: "Argument 1",
      });
      data = webidl.converters.BufferSource(data, {
        prefix,
        context: "Argument 2",
      });
      dataLayout = webidl.converters.GPUImageDataLayout(dataLayout, {
        prefix,
        context: "Argument 3",
      });
      size = webidl.converters.GPUExtent3D(size, {
        prefix,
        context: "Argument 4",
      });
      const device = assertDevice(this, { prefix, context: "this" });
      const textureRid = assertResource(destination.texture, {
        prefix,
        context: "texture",
      });
      assertDeviceMatch(device, destination.texture, {
        prefix,
        selfContext: "this",
        resourceContext: "texture",
      });
      const { err } = core.opSync(
        "op_webgpu_write_texture",
        device.rid,
        {
          texture: textureRid,
          mipLevel: destination.mipLevel,
          origin: destination.origin
            ? normalizeGPUOrigin3D(destination.origin)
            : undefined,
          aspect: destination.aspect,
        },
        dataLayout,
        normalizeGPUExtent3D(size),
        new Uint8Array(ArrayBufferIsView(data) ? data.buffer : data),
      );
      device.pushError(err);
    }

    copyImageBitmapToTexture(_source, _destination, _copySize) {
      throw new Error("Not yet implemented");
    }

    [SymbolFor("Deno.privateCustomInspect")](inspect) {
      return `${this.constructor.name} ${
        inspect({
          label: this.label,
        })
      }`;
    }
  }
  GPUObjectBaseMixin("GPUQueue", GPUQueue);

  /**
   * @typedef CreateGPUBufferOptions
   * @property {ArrayBuffer | null} mapping
   * @property {number[] | null} mappingRange
   * @property {[ArrayBuffer, number, number][] | null} mappedRanges
   * @property {"mapped" | "mapped at creation" | "mapped pending" | "unmapped" | "destroy" } state
   */

  /**
   * @param {string | null} label
   * @param {InnerGPUDevice} device
   * @param {number} rid
   * @param {number} size
   * @param {number} usage
   * @param {CreateGPUBufferOptions} options
   * @returns {GPUBuffer}
   */
  function createGPUBuffer(label, device, rid, size, usage, options) {
    /** @type {GPUBuffer} */
    const buffer = webidl.createBranded(GPUBuffer);
    buffer[_label] = label;
    buffer[_device] = device;
    buffer[_rid] = rid;
    buffer[_size] = size;
    buffer[_usage] = usage;
    buffer[_mappingRange] = options.mappingRange;
    buffer[_mappedRanges] = options.mappedRanges;
    buffer[_state] = options.state;
    return buffer;
  }

  class GPUBuffer {
    /** @type {InnerGPUDevice} */
    [_device];

    /** @type {number} */
    [_rid];

    /** @type {number} */
    [_size];

    /** @type {number} */
    [_usage];

    /** @type {"mapped" | "mapped at creation" | "mapped pending" | "unmapped" | "destroy"} */
    [_state];

    /** @type {[number, number] | null} */
    [_mappingRange];

    /** @type {[ArrayBuffer, number, number][] | null} */
    [_mappedRanges];

    /** @type {number} */
    [_mapMode];

    [_cleanup]() {
      const mappedRanges = this[_mappedRanges];
      if (mappedRanges) {
        while (mappedRanges.length > 0) {
          const mappedRange = ArrayPrototypePop(mappedRanges);
          if (mappedRange !== undefined) {
            core.close(mappedRange[1]);
          }
        }
      }
      const rid = this[_rid];
      if (rid !== undefined) {
        core.close(rid);
        /** @type {number | undefined} */
        this[_rid] = undefined;
      }
      this[_state] = "destroy";
    }

    constructor() {
      webidl.illegalConstructor();
    }

    /**
     * @param {number} mode
     * @param {number} offset
     * @param {number} [size]
     */
    async mapAsync(mode, offset = 0, size) {
      webidl.assertBranded(this, GPUBuffer.prototype);
      const prefix = "Failed to execute 'mapAsync' on 'GPUBuffer'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      mode = webidl.converters.GPUMapModeFlags(mode, {
        prefix,
        context: "Argument 1",
      });
      offset = webidl.converters.GPUSize64(offset, {
        prefix,
        context: "Argument 2",
      });
      size = size === undefined
        ? undefined
        : webidl.converters.GPUSize64(size, {
          prefix,
          context: "Argument 3",
        });
      const device = assertDevice(this, { prefix, context: "this" });
      const bufferRid = assertResource(this, { prefix, context: "this" });
      /** @type {number} */
      let rangeSize;
      if (size === undefined) {
        rangeSize = MathMax(0, this[_size] - offset);
      } else {
        rangeSize = this[_size];
      }
      if ((offset % 8) !== 0) {
        throw new DOMException(
          `${prefix}: offset must be a multiple of 8.`,
          "OperationError",
        );
      }
      if ((rangeSize % 4) !== 0) {
        throw new DOMException(
          `${prefix}: rangeSize must be a multiple of 4.`,
          "OperationError",
        );
      }
      if ((offset + rangeSize) > this[_size]) {
        throw new DOMException(
          `${prefix}: offset + rangeSize must be less than or equal to buffer size.`,
          "OperationError",
        );
      }
      if (this[_state] !== "unmapped") {
        throw new DOMException(
          `${prefix}: GPUBuffer is not currently unmapped.`,
          "OperationError",
        );
      }
      const readMode = (mode & 0x0001) === 0x0001;
      const writeMode = (mode & 0x0002) === 0x0002;
      if ((readMode && writeMode) || (!readMode && !writeMode)) {
        throw new DOMException(
          `${prefix}: exactly one of READ or WRITE map mode must be set.`,
          "OperationError",
        );
      }
      if (readMode && !((this[_usage] && 0x0001) === 0x0001)) {
        throw new DOMException(
          `${prefix}: READ map mode not valid because buffer does not have MAP_READ usage.`,
          "OperationError",
        );
      }
      if (writeMode && !((this[_usage] && 0x0002) === 0x0002)) {
        throw new DOMException(
          `${prefix}: WRITE map mode not valid because buffer does not have MAP_WRITE usage.`,
          "OperationError",
        );
      }

      this[_mapMode] = mode;
      this[_state] = "mapping pending";
      const promise = PromisePrototypeThen(
        core.opAsync(
          "op_webgpu_buffer_get_map_async",
          bufferRid,
          device.rid,
          mode,
          offset,
          rangeSize,
        ),
        ({ err }) => err,
      );
      device.pushErrorPromise(promise);
      const err = await promise;
      if (err) {
        throw new DOMException("validation error occured", "OperationError");
      }
      this[_state] = "mapped";
      this[_mappingRange] = [offset, offset + rangeSize];
      /** @type {[ArrayBuffer, number, number][] | null} */
      this[_mappedRanges] = [];
    }

    /**
     * @param {number} offset
     * @param {number} size
     */
    getMappedRange(offset = 0, size) {
      webidl.assertBranded(this, GPUBuffer.prototype);
      const prefix = "Failed to execute 'getMappedRange' on 'GPUBuffer'";
      offset = webidl.converters.GPUSize64(offset, {
        prefix,
        context: "Argument 1",
      });
      if (size !== undefined) {
        size = webidl.converters.GPUSize64(size, {
          prefix,
          context: "Argument 2",
        });
      }
      assertDevice(this, { prefix, context: "this" });
      const bufferRid = assertResource(this, { prefix, context: "this" });
      /** @type {number} */
      let rangeSize;
      if (size === undefined) {
        rangeSize = MathMax(0, this[_size] - offset);
      } else {
        rangeSize = size;
      }

      const mappedRanges = this[_mappedRanges];
      if (!mappedRanges) {
        throw new DOMException(`${prefix}: invalid state.`, "OperationError");
      }
      for (const [buffer, _rid, start] of mappedRanges) {
        // TODO(lucacasonato): is this logic correct?
        const end = start + buffer.byteLength;
        if (
          (start >= offset && start < (offset + rangeSize)) ||
          (end >= offset && end < (offset + rangeSize))
        ) {
          throw new DOMException(
            `${prefix}: requested buffer overlaps with another mapped range.`,
            "OperationError",
          );
        }
      }

      const buffer = new ArrayBuffer(rangeSize);
      const { rid } = core.opSync(
        "op_webgpu_buffer_get_mapped_range",
        bufferRid,
        offset,
        size,
        new Uint8Array(buffer),
      );

      ArrayPrototypePush(mappedRanges, [buffer, rid, offset]);

      return buffer;
    }

    unmap() {
      webidl.assertBranded(this, GPUBuffer.prototype);
      const prefix = "Failed to execute 'unmap' on 'GPUBuffer'";
      const device = assertDevice(this, { prefix, context: "this" });
      const bufferRid = assertResource(this, { prefix, context: "this" });
      if (this[_state] === "unmapped" || this[_state] === "destroyed") {
        throw new DOMException(
          `${prefix}: buffer is not ready to be unmapped.`,
          "OperationError",
        );
      }
      if (this[_state] === "mapping pending") {
        // TODO(lucacasonato): this is not spec compliant.
        throw new DOMException(
          `${prefix}: can not unmap while mapping. This is a Deno limitation.`,
          "OperationError",
        );
      } else if (
        this[_state] === "mapped" || this[_state] === "mapped at creation"
      ) {
        /** @type {boolean} */
        let write = false;
        if (this[_state] === "mapped at creation") {
          write = true;
        } else if (this[_state] === "mapped") {
          const mapMode = this[_mapMode];
          if (mapMode === undefined) {
            throw new DOMException(
              `${prefix}: invalid state.`,
              "OperationError",
            );
          }
          if ((mapMode & 0x0002) === 0x0002) {
            write = true;
          }
        }

        const mappedRanges = this[_mappedRanges];
        if (!mappedRanges) {
          throw new DOMException(`${prefix}: invalid state.`, "OperationError");
        }
        for (const [buffer, mappedRid] of mappedRanges) {
          const { err } = core.opSync(
            "op_webgpu_buffer_unmap",
            bufferRid,
            mappedRid,
            ...new SafeArrayIterator(write ? [new Uint8Array(buffer)] : []),
          );
          device.pushError(err);
          if (err) return;
        }
        this[_mappingRange] = null;
        this[_mappedRanges] = null;
      }

      this[_state] = "unmapped";
    }

    destroy() {
      webidl.assertBranded(this, GPUBuffer.prototype);
      this[_cleanup]();
    }

    [SymbolFor("Deno.privateCustomInspect")](inspect) {
      return `${this.constructor.name} ${
        inspect({
          label: this.label,
        })
      }`;
    }
  }
  GPUObjectBaseMixin("GPUBuffer", GPUBuffer);

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

  /**
   * @param {string | null} label
   * @param {InnerGPUDevice} device
   * @param {number} rid
   * @returns {GPUTexture}
   */
  function createGPUTexture(label, device, rid) {
    /** @type {GPUTexture} */
    const texture = webidl.createBranded(GPUTexture);
    texture[_label] = label;
    texture[_device] = device;
    texture[_rid] = rid;
    texture[_views] = [];
    return texture;
  }

  class GPUTexture {
    /** @type {InnerGPUDevice} */
    [_device];
    /** @type {number | undefined} */
    [_rid];
    /** @type {WeakRef<GPUTextureView>[]} */
    [_views];

    [_cleanup]() {
      const views = this[_views];
      while (views.length > 0) {
        const view = ArrayPrototypePop(views)?.deref();
        if (view) {
          view[_cleanup]();
        }
      }
      const rid = this[_rid];
      if (rid !== undefined) {
        core.close(rid);
        /** @type {number | undefined} */
        this[_rid] = undefined;
      }
    }

    constructor() {
      webidl.illegalConstructor();
    }

    /**
     * @param {GPUTextureViewDescriptor} descriptor
     */
    createView(descriptor = {}) {
      webidl.assertBranded(this, GPUTexture.prototype);
      const prefix = "Failed to execute 'createView' on 'GPUTexture'";
      webidl.requiredArguments(arguments.length, 0, { prefix });
      descriptor = webidl.converters.GPUTextureViewDescriptor(descriptor, {
        prefix,
        context: "Argument 1",
      });
      const device = assertDevice(this, { prefix, context: "this" });
      const textureRid = assertResource(this, { prefix, context: "this" });
      const { rid, err } = core.opSync("op_webgpu_create_texture_view", {
        textureRid,
        ...descriptor,
      });
      device.pushError(err);

      const textureView = createGPUTextureView(
        descriptor.label ?? null,
        this,
        rid,
      );
      ArrayPrototypePush(this[_views], new WeakRef(textureView));
      return textureView;
    }

    destroy() {
      webidl.assertBranded(this, GPUTexture.prototype);
      this[_cleanup]();
    }

    [SymbolFor("Deno.privateCustomInspect")](inspect) {
      return `${this.constructor.name} ${
        inspect({
          label: this.label,
        })
      }`;
    }
  }
  GPUObjectBaseMixin("GPUTexture", GPUTexture);

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

  /**
   * @param {string | null} label
   * @param {GPUTexture} texture
   * @param {number} rid
   * @returns {GPUTextureView}
   */
  function createGPUTextureView(label, texture, rid) {
    /** @type {GPUTextureView} */
    const textureView = webidl.createBranded(GPUTextureView);
    textureView[_label] = label;
    textureView[_texture] = texture;
    textureView[_rid] = rid;
    return textureView;
  }
  class GPUTextureView {
    /** @type {GPUTexture} */
    [_texture];
    /** @type {number | undefined} */
    [_rid];

    [_cleanup]() {
      const rid = this[_rid];
      if (rid !== undefined) {
        core.close(rid);
        /** @type {number | undefined} */
        this[_rid] = undefined;
      }
    }

    constructor() {
      webidl.illegalConstructor();
    }

    [SymbolFor("Deno.privateCustomInspect")](inspect) {
      return `${this.constructor.name} ${
        inspect({
          label: this.label,
        })
      }`;
    }
  }
  GPUObjectBaseMixin("GPUTextureView", GPUTextureView);
  const GPUTextureViewPrototype = GPUTextureView.prototype;
  /**
   * @param {string | null} label
   * @param {InnerGPUDevice} device
   * @param {number} rid
   * @returns {GPUSampler}
   */
  function createGPUSampler(label, device, rid) {
    /** @type {GPUSampler} */
    const sampler = webidl.createBranded(GPUSampler);
    sampler[_label] = label;
    sampler[_device] = device;
    sampler[_rid] = rid;
    return sampler;
  }
  class GPUSampler {
    /** @type {InnerGPUDevice} */
    [_device];
    /** @type {number | undefined} */
    [_rid];

    [_cleanup]() {
      const rid = this[_rid];
      if (rid !== undefined) {
        core.close(rid);
        /** @type {number | undefined} */
        this[_rid] = undefined;
      }
    }

    constructor() {
      webidl.illegalConstructor();
    }

    [SymbolFor("Deno.privateCustomInspect")](inspect) {
      return `${this.constructor.name} ${
        inspect({
          label: this.label,
        })
      }`;
    }
  }
  GPUObjectBaseMixin("GPUSampler", GPUSampler);
  const GPUSamplerPrototype = GPUSampler.prototype;
  /**
   * @param {string | null} label
   * @param {InnerGPUDevice} device
   * @param {number} rid
   * @returns {GPUBindGroupLayout}
   */
  function createGPUBindGroupLayout(label, device, rid) {
    /** @type {GPUBindGroupLayout} */
    const bindGroupLayout = webidl.createBranded(GPUBindGroupLayout);
    bindGroupLayout[_label] = label;
    bindGroupLayout[_device] = device;
    bindGroupLayout[_rid] = rid;
    return bindGroupLayout;
  }
  class GPUBindGroupLayout {
    /** @type {InnerGPUDevice} */
    [_device];
    /** @type {number | undefined} */
    [_rid];

    [_cleanup]() {
      const rid = this[_rid];
      if (rid !== undefined) {
        core.close(rid);
        /** @type {number | undefined} */
        this[_rid] = undefined;
      }
    }

    constructor() {
      webidl.illegalConstructor();
    }

    [SymbolFor("Deno.privateCustomInspect")](inspect) {
      return `${this.constructor.name} ${
        inspect({
          label: this.label,
        })
      }`;
    }
  }
  GPUObjectBaseMixin("GPUBindGroupLayout", GPUBindGroupLayout);

  /**
   * @param {string | null} label
   * @param {InnerGPUDevice} device
   * @param {number} rid
   * @returns {GPUPipelineLayout}
   */
  function createGPUPipelineLayout(label, device, rid) {
    /** @type {GPUPipelineLayout} */
    const pipelineLayout = webidl.createBranded(GPUPipelineLayout);
    pipelineLayout[_label] = label;
    pipelineLayout[_device] = device;
    pipelineLayout[_rid] = rid;
    return pipelineLayout;
  }
  class GPUPipelineLayout {
    /** @type {InnerGPUDevice} */
    [_device];
    /** @type {number | undefined} */
    [_rid];

    [_cleanup]() {
      const rid = this[_rid];
      if (rid !== undefined) {
        core.close(rid);
        /** @type {number | undefined} */
        this[_rid] = undefined;
      }
    }

    constructor() {
      webidl.illegalConstructor();
    }

    [SymbolFor("Deno.privateCustomInspect")](inspect) {
      return `${this.constructor.name} ${
        inspect({
          label: this.label,
        })
      }`;
    }
  }
  GPUObjectBaseMixin("GPUPipelineLayout", GPUPipelineLayout);

  /**
   * @param {string | null} label
   * @param {InnerGPUDevice} device
   * @param {number} rid
   * @returns {GPUBindGroup}
   */
  function createGPUBindGroup(label, device, rid) {
    /** @type {GPUBindGroup} */
    const bindGroup = webidl.createBranded(GPUBindGroup);
    bindGroup[_label] = label;
    bindGroup[_device] = device;
    bindGroup[_rid] = rid;
    return bindGroup;
  }
  class GPUBindGroup {
    /** @type {InnerGPUDevice} */
    [_device];
    /** @type {number | undefined} */
    [_rid];

    [_cleanup]() {
      const rid = this[_rid];
      if (rid !== undefined) {
        core.close(rid);
        /** @type {number | undefined} */
        this[_rid] = undefined;
      }
    }

    constructor() {
      webidl.illegalConstructor();
    }

    [SymbolFor("Deno.privateCustomInspect")](inspect) {
      return `${this.constructor.name} ${
        inspect({
          label: this.label,
        })
      }`;
    }
  }
  GPUObjectBaseMixin("GPUBindGroup", GPUBindGroup);

  /**
   * @param {string | null} label
   * @param {InnerGPUDevice} device
   * @param {number} rid
   * @returns {GPUShaderModule}
   */
  function createGPUShaderModule(label, device, rid) {
    /** @type {GPUShaderModule} */
    const bindGroup = webidl.createBranded(GPUShaderModule);
    bindGroup[_label] = label;
    bindGroup[_device] = device;
    bindGroup[_rid] = rid;
    return bindGroup;
  }
  class GPUShaderModule {
    /** @type {InnerGPUDevice} */
    [_device];
    /** @type {number | undefined} */
    [_rid];

    [_cleanup]() {
      const rid = this[_rid];
      if (rid !== undefined) {
        core.close(rid);
        /** @type {number | undefined} */
        this[_rid] = undefined;
      }
    }

    constructor() {
      webidl.illegalConstructor();
    }

    compilationInfo() {
      throw new Error("Not yet implemented");
    }

    [SymbolFor("Deno.privateCustomInspect")](inspect) {
      return `${this.constructor.name} ${
        inspect({
          label: this.label,
        })
      }`;
    }
  }
  GPUObjectBaseMixin("GPUShaderModule", GPUShaderModule);

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

  /**
   * @param {string | null} label
   * @param {InnerGPUDevice} device
   * @param {number} rid
   * @returns {GPUComputePipeline}
   */
  function createGPUComputePipeline(label, device, rid) {
    /** @type {GPUComputePipeline} */
    const pipeline = webidl.createBranded(GPUComputePipeline);
    pipeline[_label] = label;
    pipeline[_device] = device;
    pipeline[_rid] = rid;
    return pipeline;
  }
  class GPUComputePipeline {
    /** @type {InnerGPUDevice} */
    [_device];
    /** @type {number | undefined} */
    [_rid];

    [_cleanup]() {
      const rid = this[_rid];
      if (rid !== undefined) {
        core.close(rid);
        /** @type {number | undefined} */
        this[_rid] = undefined;
      }
    }

    constructor() {
      webidl.illegalConstructor();
    }

    /**
     * @param {number} index
     * @returns {GPUBindGroupLayout}
     */
    getBindGroupLayout(index) {
      webidl.assertBranded(this, GPUComputePipelinePrototype);
      const prefix =
        "Failed to execute 'getBindGroupLayout' on 'GPUComputePipeline'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      index = webidl.converters["unsigned long"](index, {
        prefix,
        context: "Argument 1",
      });
      const device = assertDevice(this, { prefix, context: "this" });
      const computePipelineRid = assertResource(this, {
        prefix,
        context: "this",
      });
      const { rid, label, err } = core.opSync(
        "op_webgpu_compute_pipeline_get_bind_group_layout",
        computePipelineRid,
        index,
      );
      device.pushError(err);

      const bindGroupLayout = createGPUBindGroupLayout(
        label,
        device,
        rid,
      );
      device.trackResource(bindGroupLayout);
      return bindGroupLayout;
    }

    [SymbolFor("Deno.privateCustomInspect")](inspect) {
      return `${this.constructor.name} ${
        inspect({
          label: this.label,
        })
      }`;
    }
  }
  GPUObjectBaseMixin("GPUComputePipeline", GPUComputePipeline);
  const GPUComputePipelinePrototype = GPUComputePipeline.prototype;

  /**
   * @param {string | null} label
   * @param {InnerGPUDevice} device
   * @param {number} rid
   * @returns {GPURenderPipeline}
   */
  function createGPURenderPipeline(label, device, rid) {
    /** @type {GPURenderPipeline} */
    const pipeline = webidl.createBranded(GPURenderPipeline);
    pipeline[_label] = label;
    pipeline[_device] = device;
    pipeline[_rid] = rid;
    return pipeline;
  }
  class GPURenderPipeline {
    /** @type {InnerGPUDevice} */
    [_device];
    /** @type {number | undefined} */
    [_rid];

    [_cleanup]() {
      const rid = this[_rid];
      if (rid !== undefined) {
        core.close(rid);
        /** @type {number | undefined} */
        this[_rid] = undefined;
      }
    }

    constructor() {
      webidl.illegalConstructor();
    }

    /**
     * @param {number} index
     */
    getBindGroupLayout(index) {
      webidl.assertBranded(this, GPURenderPipelinePrototype);
      const prefix =
        "Failed to execute 'getBindGroupLayout' on 'GPURenderPipeline'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      index = webidl.converters["unsigned long"](index, {
        prefix,
        context: "Argument 1",
      });
      const device = assertDevice(this, { prefix, context: "this" });
      const renderPipelineRid = assertResource(this, {
        prefix,
        context: "this",
      });
      const { rid, label, err } = core.opSync(
        "op_webgpu_render_pipeline_get_bind_group_layout",
        renderPipelineRid,
        index,
      );
      device.pushError(err);

      const bindGroupLayout = createGPUBindGroupLayout(
        label,
        device,
        rid,
      );
      device.trackResource(bindGroupLayout);
      return bindGroupLayout;
    }

    [SymbolFor("Deno.privateCustomInspect")](inspect) {
      return `${this.constructor.name} ${
        inspect({
          label: this.label,
        })
      }`;
    }
  }
  GPUObjectBaseMixin("GPURenderPipeline", GPURenderPipeline);
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

  /**
   * @param {string | null} label
   * @param {InnerGPUDevice} device
   * @param {number} rid
   * @returns {GPUCommandEncoder}
   */
  function createGPUCommandEncoder(label, device, rid) {
    /** @type {GPUCommandEncoder} */
    const encoder = webidl.createBranded(GPUCommandEncoder);
    encoder[_label] = label;
    encoder[_device] = device;
    encoder[_rid] = rid;
    encoder[_encoders] = [];
    return encoder;
  }
  class GPUCommandEncoder {
    /** @type {InnerGPUDevice} */
    [_device];
    /** @type {number | undefined} */
    [_rid];
    /** @type {WeakRef<GPURenderPassEncoder | GPUComputePassEncoder>[]} */
    [_encoders];

    [_cleanup]() {
      const encoders = this[_encoders];
      while (encoders.length > 0) {
        const encoder = ArrayPrototypePop(encoders)?.deref();
        if (encoder) {
          encoder[_cleanup]();
        }
      }
      const rid = this[_rid];
      if (rid !== undefined) {
        core.close(rid);
        /** @type {number | undefined} */
        this[_rid] = undefined;
      }
    }

    constructor() {
      webidl.illegalConstructor();
    }

    /**
     * @param {GPURenderPassDescriptor} descriptor
     * @return {GPURenderPassEncoder}
     */
    beginRenderPass(descriptor) {
      webidl.assertBranded(this, GPUCommandEncoderPrototype);
      const prefix =
        "Failed to execute 'beginRenderPass' on 'GPUCommandEncoder'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      descriptor = webidl.converters.GPURenderPassDescriptor(descriptor, {
        prefix,
        context: "Argument 1",
      });
      const device = assertDevice(this, { prefix, context: "this" });
      const commandEncoderRid = assertResource(this, {
        prefix,
        context: "this",
      });

      if (this[_rid] === undefined) {
        throw new DOMException(
          "Failed to execute 'beginRenderPass' on 'GPUCommandEncoder': already consumed",
          "OperationError",
        );
      }

      let depthStencilAttachment;
      if (descriptor.depthStencilAttachment) {
        const view = assertResource(descriptor.depthStencilAttachment.view, {
          prefix,
          context: "texture view for depth stencil attachment",
        });
        assertDeviceMatch(
          device,
          descriptor.depthStencilAttachment.view[_texture],
          {
            prefix,
            resourceContext: "texture view for depth stencil attachment",
            selfContext: "this",
          },
        );

        depthStencilAttachment = {
          ...descriptor.depthStencilAttachment,
          view,
        };
      }
      const colorAttachments = ArrayPrototypeMap(
        descriptor.colorAttachments,
        (colorAttachment, i) => {
          const context = `color attachment ${i + 1}`;
          const view = assertResource(colorAttachment.view, {
            prefix,
            context: `texture view for ${context}`,
          });
          assertResource(colorAttachment.view[_texture], {
            prefix,
            context: `texture backing texture view for ${context}`,
          });
          assertDeviceMatch(
            device,
            colorAttachment.view[_texture],
            {
              prefix,
              resourceContext: `texture view for ${context}`,
              selfContext: "this",
            },
          );
          let resolveTarget;
          if (colorAttachment.resolveTarget) {
            resolveTarget = assertResource(
              colorAttachment.resolveTarget,
              {
                prefix,
                context: `resolve target texture view for ${context}`,
              },
            );
            assertResource(colorAttachment.resolveTarget[_texture], {
              prefix,
              context:
                `texture backing resolve target texture view for ${context}`,
            });
            assertDeviceMatch(
              device,
              colorAttachment.resolveTarget[_texture],
              {
                prefix,
                resourceContext: `resolve target texture view for ${context}`,
                selfContext: "this",
              },
            );
          }
          return {
            view: view,
            resolveTarget,
            storeOp: colorAttachment.storeOp,
            loadOp: colorAttachment.loadOp,
            clearValue: normalizeGPUColor(colorAttachment.clearValue),
          };
        },
      );

      const { rid } = core.opSync(
        "op_webgpu_command_encoder_begin_render_pass",
        commandEncoderRid,
        descriptor.label,
        colorAttachments,
        depthStencilAttachment,
      );

      const renderPassEncoder = createGPURenderPassEncoder(
        descriptor.label ?? null,
        this,
        rid,
      );
      ArrayPrototypePush(this[_encoders], new WeakRef(renderPassEncoder));
      return renderPassEncoder;
    }

    /**
     * @param {GPUComputePassDescriptor} descriptor
     */
    beginComputePass(descriptor = {}) {
      webidl.assertBranded(this, GPUCommandEncoderPrototype);
      const prefix =
        "Failed to execute 'beginComputePass' on 'GPUCommandEncoder'";
      descriptor = webidl.converters.GPUComputePassDescriptor(descriptor, {
        prefix,
        context: "Argument 1",
      });

      assertDevice(this, { prefix, context: "this" });
      const commandEncoderRid = assertResource(this, {
        prefix,
        context: "this",
      });

      const { rid } = core.opSync(
        "op_webgpu_command_encoder_begin_compute_pass",
        commandEncoderRid,
        descriptor.label,
      );

      const computePassEncoder = createGPUComputePassEncoder(
        descriptor.label ?? null,
        this,
        rid,
      );
      ArrayPrototypePush(this[_encoders], new WeakRef(computePassEncoder));
      return computePassEncoder;
    }

    /**
     * @param {GPUBuffer} source
     * @param {number} sourceOffset
     * @param {GPUBuffer} destination
     * @param {number} destinationOffset
     * @param {number} size
     */
    copyBufferToBuffer(
      source,
      sourceOffset,
      destination,
      destinationOffset,
      size,
    ) {
      webidl.assertBranded(this, GPUCommandEncoderPrototype);
      const prefix =
        "Failed to execute 'copyBufferToBuffer' on 'GPUCommandEncoder'";
      webidl.requiredArguments(arguments.length, 5, { prefix });
      source = webidl.converters.GPUBuffer(source, {
        prefix,
        context: "Argument 1",
      });
      sourceOffset = webidl.converters.GPUSize64(sourceOffset, {
        prefix,
        context: "Argument 2",
      });
      destination = webidl.converters.GPUBuffer(destination, {
        prefix,
        context: "Argument 3",
      });
      destinationOffset = webidl.converters.GPUSize64(destinationOffset, {
        prefix,
        context: "Argument 4",
      });
      size = webidl.converters.GPUSize64(size, {
        prefix,
        context: "Argument 5",
      });
      const device = assertDevice(this, { prefix, context: "this" });
      const commandEncoderRid = assertResource(this, {
        prefix,
        context: "this",
      });
      const sourceRid = assertResource(source, {
        prefix,
        context: "Argument 1",
      });
      assertDeviceMatch(device, source, {
        prefix,
        resourceContext: "Argument 1",
        selfContext: "this",
      });
      const destinationRid = assertResource(destination, {
        prefix,
        context: "Argument 3",
      });
      assertDeviceMatch(device, destination, {
        prefix,
        resourceContext: "Argument 3",
        selfContext: "this",
      });

      const { err } = core.opSync(
        "op_webgpu_command_encoder_copy_buffer_to_buffer",
        commandEncoderRid,
        sourceRid,
        sourceOffset,
        destinationRid,
        destinationOffset,
        size,
      );
      device.pushError(err);
    }

    /**
     * @param {GPUImageCopyBuffer} source
     * @param {GPUImageCopyTexture} destination
     * @param {GPUExtent3D} copySize
     */
    copyBufferToTexture(source, destination, copySize) {
      webidl.assertBranded(this, GPUCommandEncoderPrototype);
      const prefix =
        "Failed to execute 'copyBufferToTexture' on 'GPUCommandEncoder'";
      webidl.requiredArguments(arguments.length, 3, { prefix });
      source = webidl.converters.GPUImageCopyBuffer(source, {
        prefix,
        context: "Argument 1",
      });
      destination = webidl.converters.GPUImageCopyTexture(destination, {
        prefix,
        context: "Argument 2",
      });
      copySize = webidl.converters.GPUExtent3D(copySize, {
        prefix,
        context: "Argument 3",
      });
      const device = assertDevice(this, { prefix, context: "this" });
      const commandEncoderRid = assertResource(this, {
        prefix,
        context: "this",
      });
      const sourceBufferRid = assertResource(source.buffer, {
        prefix,
        context: "source in Argument 1",
      });
      assertDeviceMatch(device, source.buffer, {
        prefix,
        resourceContext: "source in Argument 1",
        selfContext: "this",
      });
      const destinationTextureRid = assertResource(destination.texture, {
        prefix,
        context: "texture in Argument 2",
      });
      assertDeviceMatch(device, destination.texture, {
        prefix,
        resourceContext: "texture in Argument 2",
        selfContext: "this",
      });

      const { err } = core.opSync(
        "op_webgpu_command_encoder_copy_buffer_to_texture",
        commandEncoderRid,
        {
          ...source,
          buffer: sourceBufferRid,
        },
        {
          texture: destinationTextureRid,
          mipLevel: destination.mipLevel,
          origin: destination.origin
            ? normalizeGPUOrigin3D(destination.origin)
            : undefined,
          aspect: destination.aspect,
        },
        normalizeGPUExtent3D(copySize),
      );
      device.pushError(err);
    }

    /**
     * @param {GPUImageCopyTexture} source
     * @param {GPUImageCopyBuffer} destination
     * @param {GPUExtent3D} copySize
     */
    copyTextureToBuffer(source, destination, copySize) {
      webidl.assertBranded(this, GPUCommandEncoderPrototype);
      const prefix =
        "Failed to execute 'copyTextureToBuffer' on 'GPUCommandEncoder'";
      webidl.requiredArguments(arguments.length, 3, { prefix });
      source = webidl.converters.GPUImageCopyTexture(source, {
        prefix,
        context: "Argument 1",
      });
      destination = webidl.converters.GPUImageCopyBuffer(destination, {
        prefix,
        context: "Argument 2",
      });
      copySize = webidl.converters.GPUExtent3D(copySize, {
        prefix,
        context: "Argument 3",
      });
      const device = assertDevice(this, { prefix, context: "this" });
      const commandEncoderRid = assertResource(this, {
        prefix,
        context: "this",
      });
      const sourceTextureRid = assertResource(source.texture, {
        prefix,
        context: "texture in Argument 1",
      });
      assertDeviceMatch(device, source.texture, {
        prefix,
        resourceContext: "texture in Argument 1",
        selfContext: "this",
      });
      const destinationBufferRid = assertResource(destination.buffer, {
        prefix,
        context: "buffer in Argument 2",
      });
      assertDeviceMatch(device, destination.buffer, {
        prefix,
        resourceContext: "buffer in Argument 2",
        selfContext: "this",
      });
      const { err } = core.opSync(
        "op_webgpu_command_encoder_copy_texture_to_buffer",
        commandEncoderRid,
        {
          texture: sourceTextureRid,
          mipLevel: source.mipLevel,
          origin: source.origin
            ? normalizeGPUOrigin3D(source.origin)
            : undefined,
          aspect: source.aspect,
        },
        {
          ...destination,
          buffer: destinationBufferRid,
        },
        normalizeGPUExtent3D(copySize),
      );
      device.pushError(err);
    }

    /**
     * @param {GPUImageCopyTexture} source
     * @param {GPUImageCopyTexture} destination
     * @param {GPUExtent3D} copySize
     */
    copyTextureToTexture(source, destination, copySize) {
      webidl.assertBranded(this, GPUCommandEncoderPrototype);
      const prefix =
        "Failed to execute 'copyTextureToTexture' on 'GPUCommandEncoder'";
      webidl.requiredArguments(arguments.length, 3, { prefix });
      source = webidl.converters.GPUImageCopyTexture(source, {
        prefix,
        context: "Argument 1",
      });
      destination = webidl.converters.GPUImageCopyTexture(destination, {
        prefix,
        context: "Argument 2",
      });
      copySize = webidl.converters.GPUExtent3D(copySize, {
        prefix,
        context: "Argument 3",
      });
      const device = assertDevice(this, { prefix, context: "this" });
      const commandEncoderRid = assertResource(this, {
        prefix,
        context: "this",
      });
      const sourceTextureRid = assertResource(source.texture, {
        prefix,
        context: "texture in Argument 1",
      });
      assertDeviceMatch(device, source.texture, {
        prefix,
        resourceContext: "texture in Argument 1",
        selfContext: "this",
      });
      const destinationTextureRid = assertResource(destination.texture, {
        prefix,
        context: "texture in Argument 2",
      });
      assertDeviceMatch(device, destination.texture, {
        prefix,
        resourceContext: "texture in Argument 2",
        selfContext: "this",
      });
      const { err } = core.opSync(
        "op_webgpu_command_encoder_copy_texture_to_texture",
        commandEncoderRid,
        {
          texture: sourceTextureRid,
          mipLevel: source.mipLevel,
          origin: source.origin
            ? normalizeGPUOrigin3D(source.origin)
            : undefined,
          aspect: source.aspect,
        },
        {
          texture: destinationTextureRid,
          mipLevel: destination.mipLevel,
          origin: destination.origin
            ? normalizeGPUOrigin3D(destination.origin)
            : undefined,
          aspect: source.aspect,
        },
        normalizeGPUExtent3D(copySize),
      );
      device.pushError(err);
    }

    /**
     * @param {GPUBuffer} buffer
     * @param {GPUSize64} offset
     * @param {GPUSize64} size
     */
    clearBuffer(buffer, offset = 0, size = undefined) {
      webidl.assertBranded(this, GPUCommandEncoderPrototype);
      const prefix = "Failed to execute 'clearBuffer' on 'GPUCommandEncoder'";
      webidl.requiredArguments(arguments.length, 3, { prefix });
      buffer = webidl.converters.GPUBuffer(buffer, {
        prefix,
        context: "Argument 1",
      });
      offset = webidl.converters.GPUSize64(offset, {
        prefix,
        context: "Argument 2",
      });
      size = webidl.converters.GPUSize64(size, {
        prefix,
        context: "Argument 3",
      });
      const device = assertDevice(this, { prefix, context: "this" });
      const commandEncoderRid = assertResource(this, {
        prefix,
        context: "this",
      });
      const bufferRid = assertResource(buffer, {
        prefix,
        context: "Argument 1",
      });
      const { err } = core.opSync(
        "op_webgpu_command_encoder_clear_buffer",
        commandEncoderRid,
        bufferRid,
        offset,
        size,
      );
      device.pushError(err);
    }

    /**
     * @param {string} groupLabel
     */
    pushDebugGroup(groupLabel) {
      webidl.assertBranded(this, GPUCommandEncoderPrototype);
      const prefix =
        "Failed to execute 'pushDebugGroup' on 'GPUCommandEncoder'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      groupLabel = webidl.converters.USVString(groupLabel, {
        prefix,
        context: "Argument 1",
      });
      const device = assertDevice(this, { prefix, context: "this" });
      const commandEncoderRid = assertResource(this, {
        prefix,
        context: "this",
      });
      const { err } = core.opSync(
        "op_webgpu_command_encoder_push_debug_group",
        commandEncoderRid,
        groupLabel,
      );
      device.pushError(err);
    }

    popDebugGroup() {
      webidl.assertBranded(this, GPUCommandEncoderPrototype);
      const prefix = "Failed to execute 'popDebugGroup' on 'GPUCommandEncoder'";
      const device = assertDevice(this, { prefix, context: "this" });
      const commandEncoderRid = assertResource(this, {
        prefix,
        context: "this",
      });
      const { err } = core.opSync(
        "op_webgpu_command_encoder_pop_debug_group",
        commandEncoderRid,
      );
      device.pushError(err);
    }

    /**
     * @param {string} markerLabel
     */
    insertDebugMarker(markerLabel) {
      webidl.assertBranded(this, GPUCommandEncoderPrototype);
      const prefix =
        "Failed to execute 'insertDebugMarker' on 'GPUCommandEncoder'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      markerLabel = webidl.converters.USVString(markerLabel, {
        prefix,
        context: "Argument 1",
      });
      const device = assertDevice(this, { prefix, context: "this" });
      const commandEncoderRid = assertResource(this, {
        prefix,
        context: "this",
      });
      const { err } = core.opSync(
        "op_webgpu_command_encoder_insert_debug_marker",
        commandEncoderRid,
        markerLabel,
      );
      device.pushError(err);
    }

    /**
     * @param {GPUQuerySet} querySet
     * @param {number} queryIndex
     */
    writeTimestamp(querySet, queryIndex) {
      webidl.assertBranded(this, GPUCommandEncoderPrototype);
      const prefix =
        "Failed to execute 'writeTimestamp' on 'GPUCommandEncoder'";
      webidl.requiredArguments(arguments.length, 2, { prefix });
      querySet = webidl.converters.GPUQuerySet(querySet, {
        prefix,
        context: "Argument 1",
      });
      queryIndex = webidl.converters.GPUSize32(queryIndex, {
        prefix,
        context: "Argument 2",
      });
      const device = assertDevice(this, { prefix, context: "this" });
      const commandEncoderRid = assertResource(this, {
        prefix,
        context: "this",
      });
      const querySetRid = assertResource(querySet, {
        prefix,
        context: "Argument 1",
      });
      assertDeviceMatch(device, querySet, {
        prefix,
        resourceContext: "Argument 1",
        selfContext: "this",
      });
      const { err } = core.opSync(
        "op_webgpu_command_encoder_write_timestamp",
        commandEncoderRid,
        querySetRid,
        queryIndex,
      );
      device.pushError(err);
    }

    /**
     * @param {GPUQuerySet} querySet
     * @param {number} firstQuery
     * @param {number} queryCount
     * @param {GPUBuffer} destination
     * @param {number} destinationOffset
     */
    resolveQuerySet(
      querySet,
      firstQuery,
      queryCount,
      destination,
      destinationOffset,
    ) {
      webidl.assertBranded(this, GPUCommandEncoderPrototype);
      const prefix =
        "Failed to execute 'resolveQuerySet' on 'GPUCommandEncoder'";
      webidl.requiredArguments(arguments.length, 5, { prefix });
      querySet = webidl.converters.GPUQuerySet(querySet, {
        prefix,
        context: "Argument 1",
      });
      firstQuery = webidl.converters.GPUSize32(firstQuery, {
        prefix,
        context: "Argument 2",
      });
      queryCount = webidl.converters.GPUSize32(queryCount, {
        prefix,
        context: "Argument 3",
      });
      destination = webidl.converters.GPUBuffer(destination, {
        prefix,
        context: "Argument 4",
      });
      destinationOffset = webidl.converters.GPUSize64(destinationOffset, {
        prefix,
        context: "Argument 5",
      });
      const device = assertDevice(this, { prefix, context: "this" });
      const commandEncoderRid = assertResource(this, {
        prefix,
        context: "this",
      });
      const querySetRid = assertResource(querySet, {
        prefix,
        context: "Argument 1",
      });
      assertDeviceMatch(device, querySet, {
        prefix,
        resourceContext: "Argument 1",
        selfContext: "this",
      });
      const destinationRid = assertResource(destination, {
        prefix,
        context: "Argument 3",
      });
      assertDeviceMatch(device, destination, {
        prefix,
        resourceContext: "Argument 3",
        selfContext: "this",
      });
      const { err } = core.opSync(
        "op_webgpu_command_encoder_resolve_query_set",
        commandEncoderRid,
        querySetRid,
        firstQuery,
        queryCount,
        destinationRid,
        destinationOffset,
      );
      device.pushError(err);
    }

    /**
     * @param {GPUCommandBufferDescriptor} descriptor
     * @returns {GPUCommandBuffer}
     */
    finish(descriptor = {}) {
      webidl.assertBranded(this, GPUCommandEncoderPrototype);
      const prefix = "Failed to execute 'finish' on 'GPUCommandEncoder'";
      descriptor = webidl.converters.GPUCommandBufferDescriptor(descriptor, {
        prefix,
        context: "Argument 1",
      });
      const device = assertDevice(this, { prefix, context: "this" });
      const commandEncoderRid = assertResource(this, {
        prefix,
        context: "this",
      });
      const { rid, err } = core.opSync(
        "op_webgpu_command_encoder_finish",
        commandEncoderRid,
        descriptor.label,
      );
      device.pushError(err);
      /** @type {number | undefined} */
      this[_rid] = undefined;

      const commandBuffer = createGPUCommandBuffer(
        descriptor.label ?? null,
        device,
        rid,
      );
      device.trackResource(commandBuffer);
      return commandBuffer;
    }

    [SymbolFor("Deno.privateCustomInspect")](inspect) {
      return `${this.constructor.name} ${
        inspect({
          label: this.label,
        })
      }`;
    }
  }
  GPUObjectBaseMixin("GPUCommandEncoder", GPUCommandEncoder);
  const GPUCommandEncoderPrototype = GPUCommandEncoder.prototype;

  /**
   * @param {string | null} label
   * @param {GPUCommandEncoder} encoder
   * @param {number} rid
   * @returns {GPURenderPassEncoder}
   */
  function createGPURenderPassEncoder(label, encoder, rid) {
    /** @type {GPURenderPassEncoder} */
    const passEncoder = webidl.createBranded(GPURenderPassEncoder);
    passEncoder[_label] = label;
    passEncoder[_encoder] = encoder;
    passEncoder[_rid] = rid;
    return passEncoder;
  }

  class GPURenderPassEncoder {
    /** @type {GPUCommandEncoder} */
    [_encoder];
    /** @type {number | undefined} */
    [_rid];

    [_cleanup]() {
      const rid = this[_rid];
      if (rid !== undefined) {
        core.close(rid);
        /** @type {number | undefined} */
        this[_rid] = undefined;
      }
    }

    constructor() {
      webidl.illegalConstructor();
    }

    /**
     * @param {number} x
     * @param {number} y
     * @param {number} width
     * @param {number} height
     * @param {number} minDepth
     * @param {number} maxDepth
     */
    setViewport(x, y, width, height, minDepth, maxDepth) {
      webidl.assertBranded(this, GPURenderPassEncoderPrototype);
      const prefix =
        "Failed to execute 'setViewport' on 'GPUComputePassEncoder'";
      webidl.requiredArguments(arguments.length, 6, { prefix });
      x = webidl.converters.float(x, { prefix, context: "Argument 1" });
      y = webidl.converters.float(y, { prefix, context: "Argument 2" });
      width = webidl.converters.float(width, { prefix, context: "Argument 3" });
      height = webidl.converters.float(height, {
        prefix,
        context: "Argument 4",
      });
      minDepth = webidl.converters.float(minDepth, {
        prefix,
        context: "Argument 5",
      });
      maxDepth = webidl.converters.float(maxDepth, {
        prefix,
        context: "Argument 6",
      });
      assertDevice(this[_encoder], {
        prefix,
        context: "encoder referenced by this",
      });
      assertResource(this[_encoder], {
        prefix,
        context: "encoder referenced by this",
      });
      const renderPassRid = assertResource(this, { prefix, context: "this" });
      core.opSync("op_webgpu_render_pass_set_viewport", {
        renderPassRid,
        x,
        y,
        width,
        height,
        minDepth,
        maxDepth,
      });
    }

    /**
     * @param {number} x
     * @param {number} y
     * @param {number} width
     * @param {number} height
     */
    setScissorRect(x, y, width, height) {
      webidl.assertBranded(this, GPURenderPassEncoderPrototype);
      const prefix =
        "Failed to execute 'setScissorRect' on 'GPUComputePassEncoder'";
      webidl.requiredArguments(arguments.length, 4, { prefix });
      x = webidl.converters.GPUIntegerCoordinate(x, {
        prefix,
        context: "Argument 1",
      });
      y = webidl.converters.GPUIntegerCoordinate(y, {
        prefix,
        context: "Argument 2",
      });
      width = webidl.converters.GPUIntegerCoordinate(width, {
        prefix,
        context: "Argument 3",
      });
      height = webidl.converters.GPUIntegerCoordinate(height, {
        prefix,
        context: "Argument 4",
      });
      assertDevice(this[_encoder], {
        prefix,
        context: "encoder referenced by this",
      });
      assertResource(this[_encoder], {
        prefix,
        context: "encoder referenced by this",
      });
      const renderPassRid = assertResource(this, { prefix, context: "this" });
      core.opSync(
        "op_webgpu_render_pass_set_scissor_rect",
        renderPassRid,
        x,
        y,
        width,
        height,
      );
    }

    /**
     * @param {GPUColor} color
     */
    setBlendConstant(color) {
      webidl.assertBranded(this, GPURenderPassEncoderPrototype);
      const prefix =
        "Failed to execute 'setBlendConstant' on 'GPUComputePassEncoder'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      color = webidl.converters.GPUColor(color, {
        prefix,
        context: "Argument 1",
      });
      assertDevice(this[_encoder], {
        prefix,
        context: "encoder referenced by this",
      });
      assertResource(this[_encoder], {
        prefix,
        context: "encoder referenced by this",
      });
      const renderPassRid = assertResource(this, { prefix, context: "this" });
      core.opSync(
        "op_webgpu_render_pass_set_blend_constant",
        renderPassRid,
        normalizeGPUColor(color),
      );
    }

    /**
     * @param {number} reference
     */
    setStencilReference(reference) {
      webidl.assertBranded(this, GPURenderPassEncoderPrototype);
      const prefix =
        "Failed to execute 'setStencilReference' on 'GPUComputePassEncoder'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      reference = webidl.converters.GPUStencilValue(reference, {
        prefix,
        context: "Argument 1",
      });
      assertDevice(this[_encoder], {
        prefix,
        context: "encoder referenced by this",
      });
      assertResource(this[_encoder], {
        prefix,
        context: "encoder referenced by this",
      });
      const renderPassRid = assertResource(this, { prefix, context: "this" });
      core.opSync(
        "op_webgpu_render_pass_set_stencil_reference",
        renderPassRid,
        reference,
      );
    }

    beginOcclusionQuery(_queryIndex) {
      throw new Error("Not yet implemented");
    }

    endOcclusionQuery() {
      throw new Error("Not yet implemented");
    }

    /**
     * @param {GPUQuerySet} querySet
     * @param {number} queryIndex
     */
    beginPipelineStatisticsQuery(querySet, queryIndex) {
      webidl.assertBranded(this, GPURenderPassEncoderPrototype);
      const prefix =
        "Failed to execute 'beginPipelineStatisticsQuery' on 'GPURenderPassEncoder'";
      webidl.requiredArguments(arguments.length, 2, { prefix });
      querySet = webidl.converters.GPUQuerySet(querySet, {
        prefix,
        context: "Argument 1",
      });
      queryIndex = webidl.converters.GPUSize32(queryIndex, {
        prefix,
        context: "Argument 2",
      });
      const device = assertDevice(this[_encoder], {
        prefix,
        context: "encoder referenced by this",
      });
      assertResource(this[_encoder], {
        prefix,
        context: "encoder referenced by this",
      });
      const renderPassRid = assertResource(this, { prefix, context: "this" });
      const querySetRid = assertResource(querySet, {
        prefix,
        context: "Argument 1",
      });
      assertDeviceMatch(device, querySet, {
        prefix,
        resourceContext: "Argument 1",
        selfContext: "this",
      });
      core.opSync(
        "op_webgpu_render_pass_begin_pipeline_statistics_query",
        renderPassRid,
        querySetRid,
        queryIndex,
      );
    }

    endPipelineStatisticsQuery() {
      webidl.assertBranded(this, GPURenderPassEncoderPrototype);
      const prefix =
        "Failed to execute 'endPipelineStatisticsQuery' on 'GPURenderPassEncoder'";
      assertDevice(this[_encoder], {
        prefix,
        context: "encoder referenced by this",
      });
      assertResource(this[_encoder], {
        prefix,
        context: "encoder referenced by this",
      });
      const renderPassRid = assertResource(this, { prefix, context: "this" });
      core.opSync(
        "op_webgpu_render_pass_end_pipeline_statistics_query",
        renderPassRid,
      );
    }

    /**
     * @param {GPUQuerySet} querySet
     * @param {number} queryIndex
     */
    writeTimestamp(querySet, queryIndex) {
      webidl.assertBranded(this, GPURenderPassEncoderPrototype);
      const prefix =
        "Failed to execute 'writeTimestamp' on 'GPURenderPassEncoder'";
      webidl.requiredArguments(arguments.length, 2, { prefix });
      querySet = webidl.converters.GPUQuerySet(querySet, {
        prefix,
        context: "Argument 1",
      });
      queryIndex = webidl.converters.GPUSize32(queryIndex, {
        prefix,
        context: "Argument 2",
      });
      const device = assertDevice(this[_encoder], {
        prefix,
        context: "encoder referenced by this",
      });
      assertResource(this[_encoder], {
        prefix,
        context: "encoder referenced by this",
      });
      const renderPassRid = assertResource(this, { prefix, context: "this" });
      const querySetRid = assertResource(querySet, {
        prefix,
        context: "Argument 1",
      });
      assertDeviceMatch(device, querySet, {
        prefix,
        resourceContext: "Argument 1",
        selfContext: "this",
      });
      core.opSync(
        "op_webgpu_render_pass_write_timestamp",
        renderPassRid,
        querySetRid,
        queryIndex,
      );
    }

    /**
     * @param {GPURenderBundle[]} bundles
     */
    executeBundles(bundles) {
      webidl.assertBranded(this, GPURenderPassEncoderPrototype);
      const prefix =
        "Failed to execute 'executeBundles' on 'GPURenderPassEncoder'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      bundles = webidl.converters["sequence<GPURenderBundle>"](bundles, {
        prefix,
        context: "Argument 1",
      });
      const device = assertDevice(this[_encoder], {
        prefix,
        context: "encoder referenced by this",
      });
      assertResource(this[_encoder], {
        prefix,
        context: "encoder referenced by this",
      });
      const renderPassRid = assertResource(this, { prefix, context: "this" });
      const bundleRids = ArrayPrototypeMap(bundles, (bundle, i) => {
        const context = `bundle ${i + 1}`;
        const rid = assertResource(bundle, { prefix, context });
        assertDeviceMatch(device, bundle, {
          prefix,
          resourceContext: context,
          selfContext: "this",
        });
        return rid;
      });
      core.opSync(
        "op_webgpu_render_pass_execute_bundles",
        renderPassRid,
        bundleRids,
      );
    }

    end() {
      webidl.assertBranded(this, GPURenderPassEncoderPrototype);
      const prefix = "Failed to execute 'end' on 'GPURenderPassEncoder'";
      const device = assertDevice(this[_encoder], {
        prefix,
        context: "encoder referenced by this",
      });
      const commandEncoderRid = assertResource(this[_encoder], {
        prefix,
        context: "encoder referenced by this",
      });
      const renderPassRid = assertResource(this, { prefix, context: "this" });
      const { err } = core.opSync(
        "op_webgpu_render_pass_end",
        commandEncoderRid,
        renderPassRid,
      );
      device.pushError(err);
      this[_rid] = undefined;
    }

    // TODO(lucacasonato): has an overload
    setBindGroup(
      index,
      bindGroup,
      dynamicOffsetsData,
      dynamicOffsetsDataStart,
      dynamicOffsetsDataLength,
    ) {
      webidl.assertBranded(this, GPURenderPassEncoderPrototype);
      const prefix =
        "Failed to execute 'setBindGroup' on 'GPURenderPassEncoder'";
      const device = assertDevice(this[_encoder], {
        prefix,
        context: "encoder referenced by this",
      });
      assertResource(this[_encoder], {
        prefix,
        context: "encoder referenced by this",
      });
      const renderPassRid = assertResource(this, { prefix, context: "this" });
      const bindGroupRid = assertResource(bindGroup, {
        prefix,
        context: "Argument 2",
      });
      assertDeviceMatch(device, bindGroup, {
        prefix,
        resourceContext: "Argument 2",
        selfContext: "this",
      });
      if (
        !(ObjectPrototypeIsPrototypeOf(
          Uint32ArrayPrototype,
          dynamicOffsetsData,
        ))
      ) {
        dynamicOffsetsData = new Uint32Array(dynamicOffsetsData ?? []);
        dynamicOffsetsDataStart = 0;
        dynamicOffsetsDataLength = dynamicOffsetsData.length;
      }
      core.opSync(
        "op_webgpu_render_pass_set_bind_group",
        renderPassRid,
        index,
        bindGroupRid,
        dynamicOffsetsData,
        dynamicOffsetsDataStart,
        dynamicOffsetsDataLength,
      );
    }

    /**
     * @param {string} groupLabel
     */
    pushDebugGroup(groupLabel) {
      webidl.assertBranded(this, GPURenderPassEncoderPrototype);
      const prefix =
        "Failed to execute 'pushDebugGroup' on 'GPURenderPassEncoder'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      groupLabel = webidl.converters.USVString(groupLabel, {
        prefix,
        context: "Argument 1",
      });
      assertDevice(this[_encoder], {
        prefix,
        context: "encoder referenced by this",
      });
      assertResource(this[_encoder], {
        prefix,
        context: "encoder referenced by this",
      });
      const renderPassRid = assertResource(this, { prefix, context: "this" });
      core.opSync(
        "op_webgpu_render_pass_push_debug_group",
        renderPassRid,
        groupLabel,
      );
    }

    popDebugGroup() {
      webidl.assertBranded(this, GPURenderPassEncoderPrototype);
      const prefix =
        "Failed to execute 'popDebugGroup' on 'GPURenderPassEncoder'";
      assertDevice(this[_encoder], {
        prefix,
        context: "encoder referenced by this",
      });
      assertResource(this[_encoder], {
        prefix,
        context: "encoder referenced by this",
      });
      const renderPassRid = assertResource(this, { prefix, context: "this" });
      core.opSync("op_webgpu_render_pass_pop_debug_group", renderPassRid);
    }

    /**
     * @param {string} markerLabel
     */
    insertDebugMarker(markerLabel) {
      webidl.assertBranded(this, GPURenderPassEncoderPrototype);
      const prefix =
        "Failed to execute 'insertDebugMarker' on 'GPURenderPassEncoder'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      markerLabel = webidl.converters.USVString(markerLabel, {
        prefix,
        context: "Argument 1",
      });
      assertDevice(this[_encoder], {
        prefix,
        context: "encoder referenced by this",
      });
      assertResource(this[_encoder], {
        prefix,
        context: "encoder referenced by this",
      });
      const renderPassRid = assertResource(this, { prefix, context: "this" });
      core.opSync(
        "op_webgpu_render_pass_insert_debug_marker",
        renderPassRid,
        markerLabel,
      );
    }

    /**
     * @param {GPURenderPipeline} pipeline
     */
    setPipeline(pipeline) {
      webidl.assertBranded(this, GPURenderPassEncoderPrototype);
      const prefix =
        "Failed to execute 'setPipeline' on 'GPURenderPassEncoder'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      pipeline = webidl.converters.GPURenderPipeline(pipeline, {
        prefix,
        context: "Argument 1",
      });
      const device = assertDevice(this[_encoder], {
        prefix,
        context: "encoder referenced by this",
      });
      assertResource(this[_encoder], {
        prefix,
        context: "encoder referenced by this",
      });
      const renderPassRid = assertResource(this, { prefix, context: "this" });
      const pipelineRid = assertResource(pipeline, {
        prefix,
        context: "Argument 1",
      });
      assertDeviceMatch(device, pipeline, {
        prefix,
        resourceContext: "Argument 1",
        selfContext: "this",
      });
      core.opSync(
        "op_webgpu_render_pass_set_pipeline",
        renderPassRid,
        pipelineRid,
      );
    }

    /**
     * @param {GPUBuffer} buffer
     * @param {GPUIndexFormat} indexFormat
     * @param {number} offset
     * @param {number} size
     */
    setIndexBuffer(buffer, indexFormat, offset = 0, size) {
      webidl.assertBranded(this, GPURenderPassEncoderPrototype);
      const prefix =
        "Failed to execute 'setIndexBuffer' on 'GPURenderPassEncoder'";
      webidl.requiredArguments(arguments.length, 2, { prefix });
      buffer = webidl.converters.GPUBuffer(buffer, {
        prefix,
        context: "Argument 1",
      });
      indexFormat = webidl.converters.GPUIndexFormat(indexFormat, {
        prefix,
        context: "Argument 2",
      });
      offset = webidl.converters.GPUSize64(offset, {
        prefix,
        context: "Argument 3",
      });
      if (size !== undefined) {
        size = webidl.converters.GPUSize64(size, {
          prefix,
          context: "Argument 4",
        });
      }
      const device = assertDevice(this[_encoder], {
        prefix,
        context: "encoder referenced by this",
      });
      assertResource(this[_encoder], {
        prefix,
        context: "encoder referenced by this",
      });
      const renderPassRid = assertResource(this, { prefix, context: "this" });
      const bufferRid = assertResource(buffer, {
        prefix,
        context: "Argument 1",
      });
      assertDeviceMatch(device, buffer, {
        prefix,
        resourceContext: "Argument 1",
        selfContext: "this",
      });
      core.opSync(
        "op_webgpu_render_pass_set_index_buffer",
        renderPassRid,
        bufferRid,
        indexFormat,
        offset,
        size,
      );
    }

    /**
     * @param {number} slot
     * @param {GPUBuffer} buffer
     * @param {number} offset
     * @param {number} size
     */
    setVertexBuffer(slot, buffer, offset = 0, size) {
      webidl.assertBranded(this, GPURenderPassEncoderPrototype);
      const prefix =
        "Failed to execute 'setVertexBuffer' on 'GPURenderPassEncoder'";
      webidl.requiredArguments(arguments.length, 2, { prefix });
      slot = webidl.converters.GPUSize32(slot, {
        prefix,
        context: "Argument 2",
      });
      buffer = webidl.converters.GPUBuffer(buffer, {
        prefix,
        context: "Argument 2",
      });
      offset = webidl.converters.GPUSize64(offset, {
        prefix,
        context: "Argument 3",
      });
      if (size !== undefined) {
        size = webidl.converters.GPUSize64(size, {
          prefix,
          context: "Argument 4",
        });
      }
      const device = assertDevice(this[_encoder], {
        prefix,
        context: "encoder referenced by this",
      });
      assertResource(this[_encoder], {
        prefix,
        context: "encoder referenced by this",
      });
      const renderPassRid = assertResource(this, { prefix, context: "this" });
      const bufferRid = assertResource(buffer, {
        prefix,
        context: "Argument 2",
      });
      assertDeviceMatch(device, buffer, {
        prefix,
        resourceContext: "Argument 2",
        selfContext: "this",
      });
      core.opSync(
        "op_webgpu_render_pass_set_vertex_buffer",
        renderPassRid,
        slot,
        bufferRid,
        offset,
        size,
      );
    }

    /**
     * @param {number} vertexCount
     * @param {number} instanceCount
     * @param {number} firstVertex
     * @param {number} firstInstance
     */
    draw(vertexCount, instanceCount = 1, firstVertex = 0, firstInstance = 0) {
      webidl.assertBranded(this, GPURenderPassEncoderPrototype);
      const prefix = "Failed to execute 'draw' on 'GPURenderPassEncoder'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      vertexCount = webidl.converters.GPUSize32(vertexCount, {
        prefix,
        context: "Argument 1",
      });
      instanceCount = webidl.converters.GPUSize32(instanceCount, {
        prefix,
        context: "Argument 2",
      });
      firstVertex = webidl.converters.GPUSize32(firstVertex, {
        prefix,
        context: "Argument 3",
      });
      firstInstance = webidl.converters.GPUSize32(firstInstance, {
        prefix,
        context: "Argument 4",
      });
      assertDevice(this[_encoder], {
        prefix,
        context: "encoder referenced by this",
      });
      assertResource(this[_encoder], {
        prefix,
        context: "encoder referenced by this",
      });
      const renderPassRid = assertResource(this, { prefix, context: "this" });
      core.opSync(
        "op_webgpu_render_pass_draw",
        renderPassRid,
        vertexCount,
        instanceCount,
        firstVertex,
        firstInstance,
      );
    }

    /**
     * @param {number} indexCount
     * @param {number} instanceCount
     * @param {number} firstIndex
     * @param {number} baseVertex
     * @param {number} firstInstance
     */
    drawIndexed(
      indexCount,
      instanceCount = 1,
      firstIndex = 0,
      baseVertex = 0,
      firstInstance = 0,
    ) {
      webidl.assertBranded(this, GPURenderPassEncoderPrototype);
      const prefix =
        "Failed to execute 'drawIndexed' on 'GPURenderPassEncoder'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      indexCount = webidl.converters.GPUSize32(indexCount, {
        prefix,
        context: "Argument 1",
      });
      instanceCount = webidl.converters.GPUSize32(instanceCount, {
        prefix,
        context: "Argument 2",
      });
      firstIndex = webidl.converters.GPUSize32(firstIndex, {
        prefix,
        context: "Argument 3",
      });
      baseVertex = webidl.converters.GPUSignedOffset32(baseVertex, {
        prefix,
        context: "Argument 4",
      });
      firstInstance = webidl.converters.GPUSize32(firstInstance, {
        prefix,
        context: "Argument 5",
      });
      assertDevice(this[_encoder], {
        prefix,
        context: "encoder referenced by this",
      });
      assertResource(this[_encoder], {
        prefix,
        context: "encoder referenced by this",
      });
      const renderPassRid = assertResource(this, { prefix, context: "this" });
      core.opSync(
        "op_webgpu_render_pass_draw_indexed",
        renderPassRid,
        indexCount,
        instanceCount,
        firstIndex,
        baseVertex,
        firstInstance,
      );
    }

    /**
     * @param {GPUBuffer} indirectBuffer
     * @param {number} indirectOffset
     */
    drawIndirect(indirectBuffer, indirectOffset) {
      webidl.assertBranded(this, GPURenderPassEncoderPrototype);
      const prefix =
        "Failed to execute 'drawIndirect' on 'GPURenderPassEncoder'";
      webidl.requiredArguments(arguments.length, 2, { prefix });
      indirectBuffer = webidl.converters.GPUBuffer(indirectBuffer, {
        prefix,
        context: "Argument 1",
      });
      indirectOffset = webidl.converters.GPUSize64(indirectOffset, {
        prefix,
        context: "Argument 2",
      });
      const device = assertDevice(this[_encoder], {
        prefix,
        context: "encoder referenced by this",
      });
      assertResource(this[_encoder], {
        prefix,
        context: "encoder referenced by this",
      });
      const renderPassRid = assertResource(this, { prefix, context: "this" });
      const indirectBufferRid = assertResource(indirectBuffer, {
        prefix,
        context: "Argument 1",
      });
      assertDeviceMatch(device, indirectBuffer, {
        prefix,
        resourceContext: "Argument 1",
        selfContext: "this",
      });
      core.opSync(
        "op_webgpu_render_pass_draw_indirect",
        renderPassRid,
        indirectBufferRid,
        indirectOffset,
      );
    }

    /**
     * @param {GPUBuffer} indirectBuffer
     * @param {number} indirectOffset
     */
    drawIndexedIndirect(indirectBuffer, indirectOffset) {
      webidl.assertBranded(this, GPURenderPassEncoderPrototype);
      const prefix =
        "Failed to execute 'drawIndirect' on 'GPURenderPassEncoder'";
      webidl.requiredArguments(arguments.length, 2, { prefix });
      indirectBuffer = webidl.converters.GPUBuffer(indirectBuffer, {
        prefix,
        context: "Argument 1",
      });
      indirectOffset = webidl.converters.GPUSize64(indirectOffset, {
        prefix,
        context: "Argument 2",
      });
      const device = assertDevice(this[_encoder], {
        prefix,
        context: "encoder referenced by this",
      });
      assertResource(this[_encoder], {
        prefix,
        context: "encoder referenced by this",
      });
      const renderPassRid = assertResource(this, { prefix, context: "this" });
      const indirectBufferRid = assertResource(indirectBuffer, {
        prefix,
        context: "Argument 1",
      });
      assertDeviceMatch(device, indirectBuffer, {
        prefix,
        resourceContext: "Argument 1",
        selfContext: "this",
      });
      core.opSync(
        "op_webgpu_render_pass_draw_indexed_indirect",
        renderPassRid,
        indirectBufferRid,
        indirectOffset,
      );
    }

    [SymbolFor("Deno.privateCustomInspect")](inspect) {
      return `${this.constructor.name} ${
        inspect({
          label: this.label,
        })
      }`;
    }
  }
  GPUObjectBaseMixin("GPURenderPassEncoder", GPURenderPassEncoder);
  const GPURenderPassEncoderPrototype = GPURenderPassEncoder.prototype;

  /**
   * @param {string | null} label
   * @param {GPUCommandEncoder} encoder
   * @param {number} rid
   * @returns {GPUComputePassEncoder}
   */
  function createGPUComputePassEncoder(label, encoder, rid) {
    /** @type {GPUComputePassEncoder} */
    const computePassEncoder = webidl.createBranded(GPUComputePassEncoder);
    computePassEncoder[_label] = label;
    computePassEncoder[_encoder] = encoder;
    computePassEncoder[_rid] = rid;
    return computePassEncoder;
  }

  class GPUComputePassEncoder {
    /** @type {GPUCommandEncoder} */
    [_encoder];

    /** @type {number | undefined} */
    [_rid];

    [_cleanup]() {
      const rid = this[_rid];
      if (rid !== undefined) {
        core.close(rid);
        /** @type {number | undefined} */
        this[_rid] = undefined;
      }
    }

    constructor() {
      webidl.illegalConstructor();
    }

    /**
     * @param {GPUComputePipeline} pipeline
     */
    setPipeline(pipeline) {
      webidl.assertBranded(this, GPUComputePassEncoderPrototype);
      const prefix =
        "Failed to execute 'setPipeline' on 'GPUComputePassEncoder'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      pipeline = webidl.converters.GPUComputePipeline(pipeline, {
        prefix,
        context: "Argument 1",
      });
      const device = assertDevice(this[_encoder], {
        prefix,
        context: "encoder referenced by this",
      });
      assertResource(this[_encoder], {
        prefix,
        context: "encoder referenced by this",
      });
      const computePassRid = assertResource(this, { prefix, context: "this" });
      const pipelineRid = assertResource(pipeline, {
        prefix,
        context: "Argument 1",
      });
      assertDeviceMatch(device, pipeline, {
        prefix,
        resourceContext: "Argument 1",
        selfContext: "this",
      });
      core.opSync(
        "op_webgpu_compute_pass_set_pipeline",
        computePassRid,
        pipelineRid,
      );
    }

    /**
     * @param {number} workgroupCountX
     * @param {number} workgroupCountY
     * @param {number} workgroupCountZ
     */
    dispatchWorkgroups(
      workgroupCountX,
      workgroupCountY = 1,
      workgroupCountZ = 1,
    ) {
      webidl.assertBranded(this, GPUComputePassEncoderPrototype);
      const prefix =
        "Failed to execute 'dispatchWorkgroups' on 'GPUComputePassEncoder'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      workgroupCountX = webidl.converters.GPUSize32(workgroupCountX, {
        prefix,
        context: "Argument 1",
      });
      workgroupCountY = webidl.converters.GPUSize32(workgroupCountY, {
        prefix,
        context: "Argument 2",
      });
      workgroupCountZ = webidl.converters.GPUSize32(workgroupCountZ, {
        prefix,
        context: "Argument 3",
      });
      assertDevice(this[_encoder], {
        prefix,
        context: "encoder referenced by this",
      });
      assertResource(this[_encoder], {
        prefix,
        context: "encoder referenced by this",
      });
      const computePassRid = assertResource(this, { prefix, context: "this" });
      core.opSync(
        "op_webgpu_compute_pass_dispatch_workgroups",
        computePassRid,
        workgroupCountX,
        workgroupCountY,
        workgroupCountZ,
      );
    }

    /**
     * @param {GPUBuffer} indirectBuffer
     * @param {number} indirectOffset
     */
    dispatchWorkgroupsIndirect(indirectBuffer, indirectOffset) {
      webidl.assertBranded(this, GPUComputePassEncoderPrototype);
      const prefix =
        "Failed to execute 'dispatchWorkgroupsIndirect' on 'GPUComputePassEncoder'";
      webidl.requiredArguments(arguments.length, 2, { prefix });
      indirectBuffer = webidl.converters.GPUBuffer(indirectBuffer, {
        prefix,
        context: "Argument 1",
      });
      indirectOffset = webidl.converters.GPUSize64(indirectOffset, {
        prefix,
        context: "Argument 2",
      });
      const device = assertDevice(this[_encoder], {
        prefix,
        context: "encoder referenced by this",
      });
      assertResource(this[_encoder], {
        prefix,
        context: "encoder referenced by this",
      });
      const computePassRid = assertResource(this, { prefix, context: "this" });
      const indirectBufferRid = assertResource(indirectBuffer, {
        prefix,
        context: "Argument 1",
      });
      assertDeviceMatch(device, indirectBuffer, {
        prefix,
        resourceContext: "Argument 1",
        selfContext: "this",
      });
      core.opSync(
        "op_webgpu_compute_pass_dispatch_workgroups_indirect",
        computePassRid,
        indirectBufferRid,
        indirectOffset,
      );
    }

    /**
     * @param {GPUQuerySet} querySet
     * @param {number} queryIndex
     */
    beginPipelineStatisticsQuery(querySet, queryIndex) {
      webidl.assertBranded(this, GPUComputePassEncoderPrototype);
      const prefix =
        "Failed to execute 'beginPipelineStatisticsQuery' on 'GPUComputePassEncoder'";
      webidl.requiredArguments(arguments.length, 2, { prefix });
      querySet = webidl.converters.GPUQuerySet(querySet, {
        prefix,
        context: "Argument 1",
      });
      queryIndex = webidl.converters.GPUSize32(queryIndex, {
        prefix,
        context: "Argument 2",
      });
      const device = assertDevice(this[_encoder], {
        prefix,
        context: "encoder referenced by this",
      });
      assertResource(this[_encoder], {
        prefix,
        context: "encoder referenced by this",
      });
      const computePassRid = assertResource(this, { prefix, context: "this" });
      const querySetRid = assertResource(querySet, {
        prefix,
        context: "Argument 1",
      });
      assertDeviceMatch(device, querySet, {
        prefix,
        resourceContext: "Argument 1",
        selfContext: "this",
      });
      core.opSync(
        "op_webgpu_compute_pass_begin_pipeline_statistics_query",
        computePassRid,
        querySetRid,
        queryIndex,
      );
    }

    endPipelineStatisticsQuery() {
      webidl.assertBranded(this, GPUComputePassEncoderPrototype);
      const prefix =
        "Failed to execute 'endPipelineStatisticsQuery' on 'GPUComputePassEncoder'";
      assertDevice(this[_encoder], {
        prefix,
        context: "encoder referenced by this",
      });
      assertResource(this[_encoder], {
        prefix,
        context: "encoder referenced by this",
      });
      const computePassRid = assertResource(this, { prefix, context: "this" });
      core.opSync(
        "op_webgpu_compute_pass_end_pipeline_statistics_query",
        computePassRid,
      );
    }

    /**
     * @param {GPUQuerySet} querySet
     * @param {number} queryIndex
     */
    writeTimestamp(querySet, queryIndex) {
      webidl.assertBranded(this, GPUComputePassEncoderPrototype);
      const prefix =
        "Failed to execute 'writeTimestamp' on 'GPUComputePassEncoder'";
      webidl.requiredArguments(arguments.length, 2, { prefix });
      querySet = webidl.converters.GPUQuerySet(querySet, {
        prefix,
        context: "Argument 1",
      });
      queryIndex = webidl.converters.GPUSize32(queryIndex, {
        prefix,
        context: "Argument 2",
      });
      const device = assertDevice(this[_encoder], {
        prefix,
        context: "encoder referenced by this",
      });
      assertResource(this[_encoder], {
        prefix,
        context: "encoder referenced by this",
      });
      const computePassRid = assertResource(this, { prefix, context: "this" });
      const querySetRid = assertResource(querySet, {
        prefix,
        context: "Argument 1",
      });
      assertDeviceMatch(device, querySet, {
        prefix,
        resourceContext: "Argument 1",
        selfContext: "this",
      });
      core.opSync(
        "op_webgpu_compute_pass_write_timestamp",
        computePassRid,
        querySetRid,
        queryIndex,
      );
    }

    end() {
      webidl.assertBranded(this, GPUComputePassEncoderPrototype);
      const prefix = "Failed to execute 'end' on 'GPUComputePassEncoder'";
      const device = assertDevice(this[_encoder], {
        prefix,
        context: "encoder referenced by this",
      });
      const commandEncoderRid = assertResource(this[_encoder], {
        prefix,
        context: "encoder referenced by this",
      });
      const computePassRid = assertResource(this, { prefix, context: "this" });
      const { err } = core.opSync(
        "op_webgpu_compute_pass_end",
        commandEncoderRid,
        computePassRid,
      );
      device.pushError(err);
      this[_rid] = undefined;
    }

    // TODO(lucacasonato): has an overload
    setBindGroup(
      index,
      bindGroup,
      dynamicOffsetsData,
      dynamicOffsetsDataStart,
      dynamicOffsetsDataLength,
    ) {
      webidl.assertBranded(this, GPUComputePassEncoderPrototype);
      const prefix =
        "Failed to execute 'setBindGroup' on 'GPUComputePassEncoder'";
      const device = assertDevice(this[_encoder], {
        prefix,
        context: "encoder referenced by this",
      });
      assertResource(this[_encoder], {
        prefix,
        context: "encoder referenced by this",
      });
      const computePassRid = assertResource(this, { prefix, context: "this" });
      const bindGroupRid = assertResource(bindGroup, {
        prefix,
        context: "Argument 2",
      });
      assertDeviceMatch(device, bindGroup, {
        prefix,
        resourceContext: "Argument 2",
        selfContext: "this",
      });
      if (
        !(ObjectPrototypeIsPrototypeOf(
          Uint32ArrayPrototype,
          dynamicOffsetsData,
        ))
      ) {
        dynamicOffsetsData = new Uint32Array(dynamicOffsetsData ?? []);
        dynamicOffsetsDataStart = 0;
        dynamicOffsetsDataLength = dynamicOffsetsData.length;
      }
      core.opSync(
        "op_webgpu_compute_pass_set_bind_group",
        computePassRid,
        index,
        bindGroupRid,
        dynamicOffsetsData,
        dynamicOffsetsDataStart,
        dynamicOffsetsDataLength,
      );
    }

    /**
     * @param {string} groupLabel
     */
    pushDebugGroup(groupLabel) {
      webidl.assertBranded(this, GPUComputePassEncoderPrototype);
      const prefix =
        "Failed to execute 'pushDebugGroup' on 'GPUComputePassEncoder'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      groupLabel = webidl.converters.USVString(groupLabel, {
        prefix,
        context: "Argument 1",
      });
      assertDevice(this[_encoder], {
        prefix,
        context: "encoder referenced by this",
      });
      assertResource(this[_encoder], {
        prefix,
        context: "encoder referenced by this",
      });
      const computePassRid = assertResource(this, { prefix, context: "this" });
      core.opSync(
        "op_webgpu_compute_pass_push_debug_group",
        computePassRid,
        groupLabel,
      );
    }

    popDebugGroup() {
      webidl.assertBranded(this, GPUComputePassEncoderPrototype);
      const prefix =
        "Failed to execute 'popDebugGroup' on 'GPUComputePassEncoder'";
      assertDevice(this[_encoder], {
        prefix,
        context: "encoder referenced by this",
      });
      assertResource(this[_encoder], {
        prefix,
        context: "encoder referenced by this",
      });
      const computePassRid = assertResource(this, { prefix, context: "this" });
      core.opSync("op_webgpu_compute_pass_pop_debug_group", computePassRid);
    }

    /**
     * @param {string} markerLabel
     */
    insertDebugMarker(markerLabel) {
      webidl.assertBranded(this, GPUComputePassEncoderPrototype);
      const prefix =
        "Failed to execute 'insertDebugMarker' on 'GPUComputePassEncoder'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      markerLabel = webidl.converters.USVString(markerLabel, {
        prefix,
        context: "Argument 1",
      });
      assertDevice(this[_encoder], {
        prefix,
        context: "encoder referenced by this",
      });
      assertResource(this[_encoder], {
        prefix,
        context: "encoder referenced by this",
      });
      const computePassRid = assertResource(this, { prefix, context: "this" });
      core.opSync(
        "op_webgpu_compute_pass_insert_debug_marker",
        computePassRid,
        markerLabel,
      );
    }

    [SymbolFor("Deno.privateCustomInspect")](inspect) {
      return `${this.constructor.name} ${
        inspect({
          label: this.label,
        })
      }`;
    }
  }
  GPUObjectBaseMixin("GPUComputePassEncoder", GPUComputePassEncoder);
  const GPUComputePassEncoderPrototype = GPUComputePassEncoder.prototype;

  /**
   * @param {string | null} label
   * @param {InnerGPUDevice} device
   * @param {number} rid
   * @returns {GPUCommandBuffer}
   */
  function createGPUCommandBuffer(label, device, rid) {
    /** @type {GPUCommandBuffer} */
    const commandBuffer = webidl.createBranded(GPUCommandBuffer);
    commandBuffer[_label] = label;
    commandBuffer[_device] = device;
    commandBuffer[_rid] = rid;
    return commandBuffer;
  }

  class GPUCommandBuffer {
    /** @type {InnerGPUDevice} */
    [_device];
    /** @type {number | undefined} */
    [_rid];

    [_cleanup]() {
      const rid = this[_rid];
      if (rid !== undefined) {
        core.close(rid);
        /** @type {number | undefined} */
        this[_rid] = undefined;
      }
    }

    constructor() {
      webidl.illegalConstructor();
    }

    [SymbolFor("Deno.privateCustomInspect")](inspect) {
      return `${this.constructor.name} ${
        inspect({
          label: this.label,
        })
      }`;
    }
  }
  GPUObjectBaseMixin("GPUCommandBuffer", GPUCommandBuffer);

  /**
   * @param {string | null} label
   * @param {InnerGPUDevice} device
   * @param {number} rid
   * @returns {GPURenderBundleEncoder}
   */
  function createGPURenderBundleEncoder(label, device, rid) {
    /** @type {GPURenderBundleEncoder} */
    const bundleEncoder = webidl.createBranded(GPURenderBundleEncoder);
    bundleEncoder[_label] = label;
    bundleEncoder[_device] = device;
    bundleEncoder[_rid] = rid;
    return bundleEncoder;
  }

  class GPURenderBundleEncoder {
    /** @type {InnerGPUDevice} */
    [_device];
    /** @type {number | undefined} */
    [_rid];

    [_cleanup]() {
      const rid = this[_rid];
      if (rid !== undefined) {
        core.close(rid);
        /** @type {number | undefined} */
        this[_rid] = undefined;
      }
    }

    constructor() {
      webidl.illegalConstructor();
    }

    /**
     * @param {GPURenderBundleDescriptor} descriptor
     */
    finish(descriptor = {}) {
      webidl.assertBranded(this, GPURenderBundleEncoder.prototype);
      const prefix = "Failed to execute 'finish' on 'GPURenderBundleEncoder'";
      descriptor = webidl.converters.GPURenderBundleDescriptor(descriptor, {
        prefix,
        context: "Argument 1",
      });
      const device = assertDevice(this, { prefix, context: "this" });
      const renderBundleEncoderRid = assertResource(this, {
        prefix,
        context: "this",
      });
      const { rid, err } = core.opSync(
        "op_webgpu_render_bundle_encoder_finish",
        renderBundleEncoderRid,
        descriptor.label,
      );
      device.pushError(err);
      this[_rid] = undefined;

      const renderBundle = createGPURenderBundle(
        descriptor.label ?? null,
        device,
        rid,
      );
      device.trackResource(renderBundle);
      return renderBundle;
    }

    // TODO(lucacasonato): has an overload
    setBindGroup(
      index,
      bindGroup,
      dynamicOffsetsData,
      dynamicOffsetsDataStart,
      dynamicOffsetsDataLength,
    ) {
      webidl.assertBranded(this, GPURenderBundleEncoder.prototype);
      const prefix =
        "Failed to execute 'setBindGroup' on 'GPURenderBundleEncoder'";
      const device = assertDevice(this, { prefix, context: "this" });
      const renderBundleEncoderRid = assertResource(this, {
        prefix,
        context: "this",
      });
      const bindGroupRid = assertResource(bindGroup, {
        prefix,
        context: "Argument 2",
      });
      assertDeviceMatch(device, bindGroup, {
        prefix,
        resourceContext: "Argument 2",
        selfContext: "this",
      });
      if (
        !(ObjectPrototypeIsPrototypeOf(
          Uint32ArrayPrototype,
          dynamicOffsetsData,
        ))
      ) {
        dynamicOffsetsData = new Uint32Array(dynamicOffsetsData ?? []);
        dynamicOffsetsDataStart = 0;
        dynamicOffsetsDataLength = dynamicOffsetsData.length;
      }
      core.opSync(
        "op_webgpu_render_bundle_encoder_set_bind_group",
        renderBundleEncoderRid,
        index,
        bindGroupRid,
        dynamicOffsetsData,
        dynamicOffsetsDataStart,
        dynamicOffsetsDataLength,
      );
    }

    /**
     * @param {string} groupLabel
     */
    pushDebugGroup(groupLabel) {
      webidl.assertBranded(this, GPURenderBundleEncoder.prototype);
      const prefix =
        "Failed to execute 'pushDebugGroup' on 'GPURenderBundleEncoder'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      groupLabel = webidl.converters.USVString(groupLabel, {
        prefix,
        context: "Argument 1",
      });
      assertDevice(this, { prefix, context: "this" });
      const renderBundleEncoderRid = assertResource(this, {
        prefix,
        context: "this",
      });
      core.opSync(
        "op_webgpu_render_bundle_encoder_push_debug_group",
        renderBundleEncoderRid,
        groupLabel,
      );
    }

    popDebugGroup() {
      webidl.assertBranded(this, GPURenderBundleEncoder.prototype);
      const prefix =
        "Failed to execute 'popDebugGroup' on 'GPURenderBundleEncoder'";
      assertDevice(this, { prefix, context: "this" });
      const renderBundleEncoderRid = assertResource(this, {
        prefix,
        context: "this",
      });
      core.opSync(
        "op_webgpu_render_bundle_encoder_pop_debug_group",
        renderBundleEncoderRid,
      );
    }

    /**
     * @param {string} markerLabel
     */
    insertDebugMarker(markerLabel) {
      webidl.assertBranded(this, GPURenderBundleEncoder.prototype);
      const prefix =
        "Failed to execute 'insertDebugMarker' on 'GPURenderBundleEncoder'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      markerLabel = webidl.converters.USVString(markerLabel, {
        prefix,
        context: "Argument 1",
      });
      assertDevice(this, { prefix, context: "this" });
      const renderBundleEncoderRid = assertResource(this, {
        prefix,
        context: "this",
      });
      core.opSync(
        "op_webgpu_render_bundle_encoder_insert_debug_marker",
        renderBundleEncoderRid,
        markerLabel,
      );
    }

    /**
     * @param {GPURenderPipeline} pipeline
     */
    setPipeline(pipeline) {
      webidl.assertBranded(this, GPURenderBundleEncoder.prototype);
      const prefix =
        "Failed to execute 'setPipeline' on 'GPURenderBundleEncoder'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      pipeline = webidl.converters.GPURenderPipeline(pipeline, {
        prefix,
        context: "Argument 1",
      });
      const device = assertDevice(this, { prefix, context: "this" });
      const renderBundleEncoderRid = assertResource(this, {
        prefix,
        context: "this",
      });
      const pipelineRid = assertResource(pipeline, {
        prefix,
        context: "Argument 1",
      });
      assertDeviceMatch(device, pipeline, {
        prefix,
        resourceContext: "Argument 1",
        selfContext: "this",
      });
      core.opSync(
        "op_webgpu_render_bundle_encoder_set_pipeline",
        renderBundleEncoderRid,
        pipelineRid,
      );
    }

    /**
     * @param {GPUBuffer} buffer
     * @param {GPUIndexFormat} indexFormat
     * @param {number} offset
     * @param {number} size
     */
    setIndexBuffer(buffer, indexFormat, offset = 0, size = 0) {
      webidl.assertBranded(this, GPURenderBundleEncoder.prototype);
      const prefix =
        "Failed to execute 'setIndexBuffer' on 'GPURenderBundleEncoder'";
      webidl.requiredArguments(arguments.length, 2, { prefix });
      buffer = webidl.converters.GPUBuffer(buffer, {
        prefix,
        context: "Argument 1",
      });
      indexFormat = webidl.converters.GPUIndexFormat(indexFormat, {
        prefix,
        context: "Argument 2",
      });
      offset = webidl.converters.GPUSize64(offset, {
        prefix,
        context: "Argument 3",
      });
      size = webidl.converters.GPUSize64(size, {
        prefix,
        context: "Argument 4",
      });
      const device = assertDevice(this, { prefix, context: "this" });
      const renderBundleEncoderRid = assertResource(this, {
        prefix,
        context: "this",
      });
      const bufferRid = assertResource(buffer, {
        prefix,
        context: "Argument 1",
      });
      assertDeviceMatch(device, buffer, {
        prefix,
        resourceContext: "Argument 1",
        selfContext: "this",
      });
      core.opSync(
        "op_webgpu_render_bundle_encoder_set_index_buffer",
        renderBundleEncoderRid,
        bufferRid,
        indexFormat,
        offset,
        size,
      );
    }

    /**
     * @param {number} slot
     * @param {GPUBuffer} buffer
     * @param {number} offset
     * @param {number} size
     */
    setVertexBuffer(slot, buffer, offset = 0, size = 0) {
      webidl.assertBranded(this, GPURenderBundleEncoder.prototype);
      const prefix =
        "Failed to execute 'setVertexBuffer' on 'GPURenderBundleEncoder'";
      webidl.requiredArguments(arguments.length, 2, { prefix });
      slot = webidl.converters.GPUSize32(slot, {
        prefix,
        context: "Argument 2",
      });
      buffer = webidl.converters.GPUBuffer(buffer, {
        prefix,
        context: "Argument 2",
      });
      offset = webidl.converters.GPUSize64(offset, {
        prefix,
        context: "Argument 3",
      });
      size = webidl.converters.GPUSize64(size, {
        prefix,
        context: "Argument 4",
      });
      const device = assertDevice(this, { prefix, context: "this" });
      const renderBundleEncoderRid = assertResource(this, {
        prefix,
        context: "this",
      });
      const bufferRid = assertResource(buffer, {
        prefix,
        context: "Argument 2",
      });
      assertDeviceMatch(device, buffer, {
        prefix,
        resourceContext: "Argument 2",
        selfContext: "this",
      });
      core.opSync(
        "op_webgpu_render_bundle_encoder_set_vertex_buffer",
        renderBundleEncoderRid,
        slot,
        bufferRid,
        offset,
        size,
      );
    }

    /**
     * @param {number} vertexCount
     * @param {number} instanceCount
     * @param {number} firstVertex
     * @param {number} firstInstance
     */
    draw(vertexCount, instanceCount = 1, firstVertex = 0, firstInstance = 0) {
      webidl.assertBranded(this, GPURenderBundleEncoder.prototype);
      const prefix = "Failed to execute 'draw' on 'GPURenderBundleEncoder'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      vertexCount = webidl.converters.GPUSize32(vertexCount, {
        prefix,
        context: "Argument 1",
      });
      instanceCount = webidl.converters.GPUSize32(instanceCount, {
        prefix,
        context: "Argument 2",
      });
      firstVertex = webidl.converters.GPUSize32(firstVertex, {
        prefix,
        context: "Argument 3",
      });
      firstInstance = webidl.converters.GPUSize32(firstInstance, {
        prefix,
        context: "Argument 4",
      });
      assertDevice(this, { prefix, context: "this" });
      const renderBundleEncoderRid = assertResource(this, {
        prefix,
        context: "this",
      });
      core.opSync(
        "op_webgpu_render_bundle_encoder_draw",
        renderBundleEncoderRid,
        vertexCount,
        instanceCount,
        firstVertex,
        firstInstance,
      );
    }

    /**
     * @param {number} indexCount
     * @param {number} instanceCount
     * @param {number} firstIndex
     * @param {number} baseVertex
     * @param {number} firstInstance
     */
    drawIndexed(
      indexCount,
      instanceCount = 1,
      firstIndex = 0,
      baseVertex = 0,
      firstInstance = 0,
    ) {
      webidl.assertBranded(this, GPURenderBundleEncoder.prototype);
      const prefix =
        "Failed to execute 'drawIndexed' on 'GPURenderBundleEncoder'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      indexCount = webidl.converters.GPUSize32(indexCount, {
        prefix,
        context: "Argument 1",
      });
      instanceCount = webidl.converters.GPUSize32(instanceCount, {
        prefix,
        context: "Argument 2",
      });
      firstIndex = webidl.converters.GPUSize32(firstIndex, {
        prefix,
        context: "Argument 3",
      });
      baseVertex = webidl.converters.GPUSignedOffset32(baseVertex, {
        prefix,
        context: "Argument 4",
      });
      firstInstance = webidl.converters.GPUSize32(firstInstance, {
        prefix,
        context: "Argument 5",
      });
      assertDevice(this, { prefix, context: "this" });
      const renderBundleEncoderRid = assertResource(this, {
        prefix,
        context: "this",
      });
      core.opSync(
        "op_webgpu_render_bundle_encoder_draw_indexed",
        renderBundleEncoderRid,
        indexCount,
        instanceCount,
        firstIndex,
        baseVertex,
        firstInstance,
      );
    }

    /**
     * @param {GPUBuffer} indirectBuffer
     * @param {number} indirectOffset
     */
    drawIndirect(indirectBuffer, indirectOffset) {
      webidl.assertBranded(this, GPURenderBundleEncoder.prototype);
      const prefix =
        "Failed to execute 'drawIndirect' on 'GPURenderBundleEncoder'";
      webidl.requiredArguments(arguments.length, 2, { prefix });
      indirectBuffer = webidl.converters.GPUBuffer(indirectBuffer, {
        prefix,
        context: "Argument 1",
      });
      indirectOffset = webidl.converters.GPUSize64(indirectOffset, {
        prefix,
        context: "Argument 2",
      });
      const device = assertDevice(this, { prefix, context: "this" });
      const renderBundleEncoderRid = assertResource(this, {
        prefix,
        context: "this",
      });
      const indirectBufferRid = assertResource(indirectBuffer, {
        prefix,
        context: "Argument 1",
      });
      assertDeviceMatch(device, indirectBuffer, {
        prefix,
        resourceContext: "Argument 1",
        selfContext: "this",
      });
      core.opSync(
        "op_webgpu_render_bundle_encoder_draw_indirect",
        renderBundleEncoderRid,
        indirectBufferRid,
        indirectOffset,
      );
    }

    drawIndexedIndirect(_indirectBuffer, _indirectOffset) {
      throw new Error("Not yet implemented");
    }

    [SymbolFor("Deno.privateCustomInspect")](inspect) {
      return `${this.constructor.name} ${
        inspect({
          label: this.label,
        })
      }`;
    }
  }
  GPUObjectBaseMixin("GPURenderBundleEncoder", GPURenderBundleEncoder);

  /**
   * @param {string | null} label
   * @param {InnerGPUDevice} device
   * @param {number} rid
   * @returns {GPURenderBundle}
   */
  function createGPURenderBundle(label, device, rid) {
    /** @type {GPURenderBundle} */
    const bundle = webidl.createBranded(GPURenderBundle);
    bundle[_label] = label;
    bundle[_device] = device;
    bundle[_rid] = rid;
    return bundle;
  }

  class GPURenderBundle {
    /** @type {InnerGPUDevice} */
    [_device];
    /** @type {number | undefined} */
    [_rid];

    [_cleanup]() {
      const rid = this[_rid];
      if (rid !== undefined) {
        core.close(rid);
        /** @type {number | undefined} */
        this[_rid] = undefined;
      }
    }

    constructor() {
      webidl.illegalConstructor();
    }

    [SymbolFor("Deno.privateCustomInspect")](inspect) {
      return `${this.constructor.name} ${
        inspect({
          label: this.label,
        })
      }`;
    }
  }
  GPUObjectBaseMixin("GPURenderBundle", GPURenderBundle);

  /**
   * @param {string | null} label
   * @param {InnerGPUDevice} device
   * @param {number} rid
   * @returns {GPUQuerySet}
   */
  function createGPUQuerySet(label, device, rid, descriptor) {
    /** @type {GPUQuerySet} */
    const queue = webidl.createBranded(GPUQuerySet);
    queue[_label] = label;
    queue[_device] = device;
    queue[_rid] = rid;
    queue[_descriptor] = descriptor;
    return queue;
  }

  class GPUQuerySet {
    /** @type {InnerGPUDevice} */
    [_device];
    /** @type {number | undefined} */
    [_rid];
    /** @type {GPUQuerySetDescriptor} */
    [_descriptor];

    [_cleanup]() {
      const rid = this[_rid];
      if (rid !== undefined) {
        core.close(rid);
        /** @type {number | undefined} */
        this[_rid] = undefined;
      }
    }

    constructor() {
      webidl.illegalConstructor();
    }

    destroy() {
      webidl.assertBranded(this, GPUQuerySetPrototype);
      this[_cleanup]();
    }

    [SymbolFor("Deno.privateCustomInspect")](inspect) {
      return `${this.constructor.name} ${
        inspect({
          label: this.label,
        })
      }`;
    }
  }
  GPUObjectBaseMixin("GPUQuerySet", GPUQuerySet);
  const GPUQuerySetPrototype = GPUQuerySet.prototype;

  window.__bootstrap.webgpu = {
    gpu: webidl.createBranded(GPU),
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
    GPUError,
    GPUOutOfMemoryError,
    GPUValidationError,
  };
})(this);
