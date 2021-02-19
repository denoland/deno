// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

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

  function normalizeGPUExtent3D(data) {
    if (Array.isArray(data)) {
      return {
        width: data[0],
        height: data[1],
        depth: data[2],
      };
    } else {
      return data;
    }
  }

  function normalizeGPUOrigin3D(data) {
    if (Array.isArray(data)) {
      return {
        x: data[0],
        y: data[1],
        z: data[2],
      };
    } else {
      return data;
    }
  }

  function normalizeGPUColor(data) {
    if (Array.isArray(data)) {
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

  class GPUOutOfMemoryError extends Error {
    constructor() {
      super();
    }
  }

  class GPUValidationError extends Error {
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

  class GPU {
    [webidl.brand] = webidl.brand;

    constructor() {
      webidl.illegalConstructor();
    }

    /**
     * @param {GPURequestAdapterOptions} options
     */
    async requestAdapter(options = {}) {
      webidl.assertBranded(this, GPU);
      options = webidl.converters.GPURequestAdapterOptions(options, {
        prefix: "Failed to execute 'requestAdapter' on 'GPU'",
        context: "Argument 1",
      });

      const { error, ...data } = await core.jsonOpAsync(
        "op_webgpu_request_adapter",
        {
          ...options,
        },
      );

      if (error) {
        return null;
      } else {
        return createGPUAdapter(data.name, data);
      }
    }
  }

  const _name = Symbol("[[name]]");
  const _adapter = Symbol("[[adapter]]");
  const _cleanup = Symbol("[[cleanup]]");

  /**
   * @typedef InnerGPUAdapter
   * @property {number} rid
   * @property {GPUAdapterFeatures} features
   * @property {GPUAdapterLimits} limits
   */

  /**
    * @param {string} name
    * @param {InnerGPUAdapter} inner
    * @returns {GPUAdapter}
    */
  function createGPUAdapter(name, inner) {
    /** @type {GPUAdapter} */
    const adapter = webidl.createBranded(GPUAdapter);
    adapter[_name] = name;
    adapter[_adapter] = {
      ...inner,
      features: createGPUAdapterFeatures(inner.features),
      limits: createGPUAdapterLimits(inner.limits),
    };
    return adapter;
  }

  class GPUAdapter {
    /** @type {string} */
    [_name];
    /** @type {InnerGPUAdapter} */
    [_adapter];

    /** @returns {string} */
    get name() {
      webidl.assertBranded(this, GPUAdapter);
      return this[_name];
    }
    /** @returns {GPUAdapterFeatures} */
    get features() {
      webidl.assertBranded(this, GPUAdapter);
      return this[_adapter].features;
    }
    /** @returns {GPUAdapterLimits} */
    get limits() {
      webidl.assertBranded(this, GPUAdapter);
      return this[_adapter].limits;
    }

    constructor() {
      webidl.illegalConstructor();
    }

    /**
     * @param {GPUDeviceDescriptor} descriptor
     * @returns {Promise<GPUDevice>}
     */
    async requestDevice(descriptor = {}) {
      webidl.assertBranded(this, GPUAdapter);
      const prefix = "Failed to execute 'requestDevice' on 'GPUAdapter'";
      descriptor = webidl.converters.GPUDeviceDescriptor(descriptor, {
        prefix,
        context: "Argument 1",
      });
      const nonGuaranteedFeatures = descriptor.nonGuaranteedFeatures ?? [];
      for (const feature of nonGuaranteedFeatures) {
        if (!this[_adapter].features.has(feature)) {
          throw new TypeError(
            `${prefix}: nonGuaranteedFeatures must be a subset of the adapter features.`,
          );
        }
      }
      const nonGuaranteedLimits = descriptor.nonGuaranteedLimits ?? [];
      // TODO(lucacasonato): validate nonGuaranteedLimits

      const { rid, features, limits } = await core.jsonOpAsync(
        "op_webgpu_request_device",
        {
          adapterRid: this[_adapter].rid,
          labe: descriptor.label,
          nonGuaranteedFeatures,
          nonGuaranteedLimits,
        },
      );

      /** @type {InnerGPUDevice} */
      const inner = {
        rid,
        adapter: this,
        features: Object.freeze(features),
        limits: Object.freeze(limits),
        resources: [],
      };
      return createGPUDevice(
        descriptor.label ?? null,
        inner,
        createGPUQueue(descriptor.label ?? null, inner),
      );
    }

    [Symbol.for("Deno.customInspect")](inspect) {
      return `${this.constructor.name} ${
        inspect({
          name: this.name,
          features: this.features,
          limits: this.limits,
        })
      }`;
    }
  }

  const _limits = Symbol("[[limits]]");

  function createGPUAdapterLimits(features) {
    /** @type {GPUAdapterLimits} */
    const adapterFeatures = webidl.createBranded(GPUAdapterLimits);
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
   * @property {number} maxVertexBuffers
   * @property {number} maxVertexAttributes
   * @property {number} maxVertexBufferArrayStride
   */

  class GPUAdapterLimits {
    /** @type {InnerAdapterLimits} */
    [_limits];
    constructor() {
      webidl.illegalConstructor();
    }

    get maxTextureDimension1D() {
      throw new TypeError("Not yet implemented");
    }
    get maxTextureDimension2D() {
      throw new TypeError("Not yet implemented");
    }
    get maxTextureDimension3D() {
      throw new TypeError("Not yet implemented");
    }
    get maxTextureArrayLayers() {
      throw new TypeError("Not yet implemented");
    }
    get maxBindGroups() {
      return this[_limits].maxBindGroups;
    }
    get maxDynamicUniformBuffersPerPipelineLayout() {
      return this[_limits].maxDynamicUniformBuffersPerPipelineLayout;
    }
    get maxDynamicStorageBuffersPerPipelineLayout() {
      return this[_limits].maxDynamicStorageBuffersPerPipelineLayout;
    }
    get maxSampledTexturesPerShaderStage() {
      return this[_limits].maxSampledTexturesPerShaderStage;
    }
    get maxSamplersPerShaderStage() {
      return this[_limits].maxSamplersPerShaderStage;
    }
    get maxStorageBuffersPerShaderStage() {
      return this[_limits].maxStorageBuffersPerShaderStage;
    }
    get maxStorageTexturesPerShaderStage() {
      return this[_limits].maxStorageTexturesPerShaderStage;
    }
    get maxUniformBuffersPerShaderStage() {
      return this[_limits].maxUniformBuffersPerShaderStage;
    }
    get maxUniformBufferBindingSize() {
      return this[_limits].maxUniformBufferBindingSize;
    }
    get maxStorageBufferBindingSize() {
      throw new TypeError("Not yet implemented");
    }
    get maxVertexBuffers() {
      throw new TypeError("Not yet implemented");
    }
    get maxVertexAttributes() {
      throw new TypeError("Not yet implemented");
    }
    get maxVertexBufferArrayStride() {
      throw new TypeError("Not yet implemented");
    }

    [Symbol.for("Deno.customInspect")](inspect) {
      return `${this.constructor.name} ${inspect(this[_limits])}`;
    }
  }

  const _features = Symbol("[[features]]");

  function createGPUAdapterFeatures(features) {
    /** @type {GPUAdapterFeatures} */
    const adapterFeatures = webidl.createBranded(GPUAdapterFeatures);
    adapterFeatures[_features] = new Set(features);
    return adapterFeatures;
  }

  class GPUAdapterFeatures {
    /** @type {Set<string>} */
    [_features];

    constructor() {
      webidl.illegalConstructor();
    }

    /** @return {IterableIterator<[string, string]>} */
    entries() {
      return this[_features].entries();
    }

    /** @return {void} */
    forEach(callbackfn, thisArg) {
      this[_features].forEach(callbackfn, thisArg);
    }

    /** @return {boolean} */
    has(value) {
      return this[_features].has(value);
    }

    /** @return {IterableIterator<string>} */
    keys() {
      return this[_features].keys();
    }

    /** @return {IterableIterator<string>} */
    values() {
      return this[_features].values();
    }

    /** @return {number} */
    get size() {
      return this[_features].size;
    }

    [Symbol.iterator]() {
      return this[_features][Symbol.iterator]();
    }

    [Symbol.for("Deno.customInspect")](inspect) {
      return `${this.constructor.name} ${inspect([...this.values()])}`;
    }
  }

  const _label = Symbol("[[label]]");

  /**
   * @param {string} name
   * @param {any} type
   */
  function GPUObjectBaseMixin(name, type) {
    type.prototype[_label] = null;
    Object.defineProperty(type.prototype, "label", {
      /**
       * @return {string | null}
       */
      get() {
        webidl.assertBranded(this, type);
        return this[_label];
      },
      /**
       * @param {string | null} label
       */
      set(label) {
        webidl.assertBranded(this, type);
        label = webidl.converters["UVString?"](label, {
          prefix: `Failed to set 'label' on '${name}'`,
          context: "Argument 1",
        });
        this[_label] = label;
      },
    });
  }

  const _device = Symbol("[[device]]");
  const _queue = Symbol("[[queue]]");

  /**
   * @typedef InnerGPUDevice
   * @property {GPUAdapter} adapter
   * @property {number | undefined} rid
   * @property {GPUFeatureName[]} features
   * @property {object} limits
   * @property {WeakRef<any>[]} resources
   */

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

  // TODO(@crowlKats): https://gpuweb.github.io/gpuweb/#errors-and-debugging
  class GPUDevice extends eventTarget.EventTarget {
    /** @type {InnerGPUDevice} */
    [_device];

    /** @type {GPUQueue} */
    [_queue];

    [_cleanup]() {
      const device = this[_device];
      const resources = device.resources;
      while (resources.length > 0) {
        const resource = resources.pop()?.deref();
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

    get adapter() {
      webidl.assertBranded(this, GPUDevice);
      return this[_device].adapter;
    }
    get features() {
      webidl.assertBranded(this, GPUDevice);
      return this[_device].features;
    }
    get limits() {
      webidl.assertBranded(this, GPUDevice);
      return this[_device].limits;
    }
    get queue() {
      webidl.assertBranded(this, GPUDevice);
      return this[_queue];
    }

    constructor() {
      webidl.illegalConstructor();
      super();
    }

    destroy() {
      webidl.assertBranded(this, GPUDevice);
      this[_cleanup]();
    }

    /**
     * @param {GPUBufferDescriptor} descriptor
     * @returns {GPUBuffer}
     */
    createBuffer(descriptor) {
      webidl.assertBranded(this, GPUDevice);
      const prefix = "Failed to execute 'createBuffer' on 'GPUDevice'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      descriptor = webidl.converters.GPUBufferDescriptor(descriptor, {
        prefix,
        context: "Argument 1",
      });
      const { rid } = core.jsonOpSync("op_webgpu_create_buffer", {
        deviceRid: this[_device].rid,
        ...descriptor,
      });
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
        this[_device],
        rid,
        descriptor.size,
        descriptor.usage,
        options,
      );
      this[_device].resources.push(new WeakRef(buffer));
      return buffer;
    }

    /**
     * @param {GPUTextureDescriptor} descriptor
     * @returns {GPUTexture}
     */
    createTexture(descriptor) {
      webidl.assertBranded(this, GPUDevice);
      const prefix = "Failed to execute 'createTexture' on 'GPUDevice'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      descriptor = webidl.converters.GPUTextureDescriptor(descriptor, {
        prefix,
        context: "Argument 1",
      });
      const { rid } = core.jsonOpSync("op_webgpu_create_texture", {
        deviceRid: this[_device].rid,
        ...descriptor,
        size: normalizeGPUExtent3D(descriptor.size),
      });

      const texture = createGPUTexture(
        descriptor.label ?? null,
        this[_device],
        rid,
      );
      this[_device].resources.push(new WeakRef(texture));
      return texture;
    }

    /**
     * @param {GPUSamplerDescriptor} descriptor
     * @returns {GPUSampler}
     */
    createSampler(descriptor = {}) {
      webidl.assertBranded(this, GPUDevice);
      const prefix = "Failed to execute 'createSampler' on 'GPUDevice'";
      descriptor = webidl.converters.GPUSamplerDescriptor(descriptor, {
        prefix,
        context: "Argument 1",
      });
      const { rid } = core.jsonOpSync("op_webgpu_create_sampler", {
        deviceRid: this[_device].rid,
        ...descriptor,
      });

      const sampler = createGPUSampler(
        descriptor.label ?? null,
        this[_device],
        rid,
      );
      this[_device].resources.push(new WeakRef(sampler));
      return sampler;
    }

    /**
     * @param {GPUBindGroupLayoutDescriptor} descriptor
     * @returns {GPUBindGroupLayout}
     */
    createBindGroupLayout(descriptor) {
      webidl.assertBranded(this, GPUDevice);
      const prefix = "Failed to execute 'createBindGroupLayout' on 'GPUDevice'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      descriptor = webidl.converters.GPUBindGroupLayoutDescriptor(descriptor, {
        prefix,
        context: "Argument 1",
      });
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

      const { rid } = core.jsonOpSync("op_webgpu_create_bind_group_layout", {
        deviceRid: this[_device].rid,
        ...descriptor,
      });

      const bindGroupLayout = createGPUBindGroupLayout(
        descriptor.label ?? null,
        this[_device],
        rid,
      );
      this[_device].resources.push(new WeakRef(bindGroupLayout));
      return bindGroupLayout;
    }

    /**
     * @param {GPUPipelineLayoutDescriptor} descriptor
     * @returns {GPUPipelineLayout}
     */
    createPipelineLayout(descriptor) {
      webidl.assertBranded(this, GPUDevice);
      const prefix = "Failed to execute 'createPipelineLayout' on 'GPUDevice'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      descriptor = webidl.converters.GPUPipelineLayoutDescriptor(descriptor, {
        prefix,
        context: "Argument 1",
      });
      const { rid } = core.jsonOpSync("op_webgpu_create_pipeline_layout", {
        deviceRid: this[_device].rid,
        label: descriptor.label,
        bindGroupLayouts: descriptor.bindGroupLayouts.map((bindGroupLayout) =>
          bindGroupLayout[_rid]
        ),
      });

      const pipelineLayout = createGPUPipelineLayout(
        descriptor.label ?? null,
        this[_device],
        rid,
      );
      this[_device].resources.push(new WeakRef(pipelineLayout));
      return pipelineLayout;
    }

    /**
     * @param {GPUBindGroupDescriptor} descriptor
     * @returns {GPUBindGroup}
     */
    createBindGroup(descriptor) {
      webidl.assertBranded(this, GPUDevice);
      const prefix = "Failed to execute 'createBindGroup' on 'GPUDevice'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      descriptor = webidl.converters.GPUBindGroupDescriptor(descriptor, {
        prefix,
        context: "Argument 1",
      });
      const { rid } = core.jsonOpSync("op_webgpu_create_bind_group", {
        deviceRid: this[_device].rid,
        label: descriptor.label,
        layout: descriptor.layout[_rid],
        entries: descriptor.entries.map((entry) => {
          if (entry.resource instanceof GPUSampler) {
            return {
              binding: entry.binding,
              kind: "GPUSampler",
              resource: entry.resource[_rid],
            };
          } else if (entry.resource instanceof GPUTextureView) {
            return {
              binding: entry.binding,
              kind: "GPUTextureView",
              resource: entry.resource[_rid],
            };
          } else {
            return {
              binding: entry.binding,
              kind: "GPUBufferBinding",
              resource: entry.resource.buffer[_rid],
              offset: entry.resource.offset,
              size: entry.resource.size,
            };
          }
        }),
      });

      const bindGroup = createGPUBindGroup(
        descriptor.label ?? null,
        this[_device],
        rid,
      );
      this[_device].resources.push(new WeakRef(bindGroup));
      return bindGroup;
    }

    /**
     * @param {GPUShaderModuleDescriptor} descriptor
     */
    createShaderModule(descriptor) {
      webidl.assertBranded(this, GPUDevice);
      const prefix = "Failed to execute 'createShaderModule' on 'GPUDevice'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      descriptor = webidl.converters.GPUShaderModuleDescriptor(descriptor, {
        prefix,
        context: "Argument 1",
      });
      const { rid } = core.jsonOpSync(
        "op_webgpu_create_shader_module",
        {
          deviceRid: this[_device].rid,
          label: descriptor.label,
          code: (typeof descriptor.code === "string")
            ? descriptor.code
            : undefined,
          sourceMap: descriptor.sourceMap,
        },
        ...(descriptor.code instanceof Uint32Array
          ? [new Uint8Array(descriptor.code.buffer)]
          : []),
      );

      const shaderModule = createGPUShaderModule(
        descriptor.label ?? null,
        this[_device],
        rid,
      );
      this[_device].resources.push(new WeakRef(shaderModule));
      return shaderModule;
    }

    /**
     * @param {GPUComputePipelineDescriptor} descriptor
     * @returns {GPUComputePipeline}
     */
    createComputePipeline(descriptor) {
      webidl.assertBranded(this, GPUDevice);
      const prefix = "Failed to execute 'createComputePipeline' on 'GPUDevice'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      descriptor = webidl.converters.GPUComputePipelineDescriptor(descriptor, {
        prefix,
        context: "Argument 1",
      });
      const { rid } = core.jsonOpSync("op_webgpu_create_compute_pipeline", {
        deviceRid: this[_device].rid,
        label: descriptor.label,
        layout: descriptor.layout ? descriptor.layout[_rid] : undefined,
        compute: {
          module: descriptor.compute.module[_rid],
          entryPoint: descriptor.compute.entryPoint,
        },
      });

      const computePipeline = createGPUComputePipeline(
        descriptor.label ?? null,
        this[_device],
        rid,
      );
      this[_device].resources.push(new WeakRef(computePipeline));
      return computePipeline;
    }

    /**
     * @param {GPURenderPipelineDescriptor} descriptor
     * @returns {GPURenderPipeline}
     */
    createRenderPipeline(descriptor) {
      webidl.assertBranded(this, GPUDevice);
      const prefix = "Failed to execute 'createRenderPipeline' on 'GPUDevice'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      descriptor = webidl.converters.GPURenderPipelineDescriptor(descriptor, {
        prefix,
        context: "Argument 1",
      });
      const d = {
        label: descriptor.label,
        layout: descriptor.layout?.[_rid],
        vertex: {
          module: descriptor.vertex.module[_rid],
          entryPoint: descriptor.vertex.entryPoint,
          buffers: descriptor.vertex.buffers,
        },
        primitive: descriptor.primitive,
        depthStencil: descriptor.depthStencil,
        multisample: descriptor.multisample,
        fragment: descriptor.fragment
          ? {
            module: descriptor.fragment.module[_rid],
            entryPoint: descriptor.fragment.entryPoint,
            targets: descriptor.fragment.targets,
          }
          : undefined,
      };

      const { rid } = core.jsonOpSync("op_webgpu_create_render_pipeline", {
        deviceRid: this[_device].rid,
        ...d,
      });

      const renderPipeline = createGPURenderPipeline(
        descriptor.label ?? null,
        this[_device],
        rid,
      );
      this[_device].resources.push(new WeakRef(renderPipeline));
      return renderPipeline;
    }

    createComputePipelineAsync(descriptor) {
      // TODO(lucacasonato): this should be real async
      return Promise.resolve(this.createComputePipeline(descriptor));
    }

    createRenderPipelineAsync(descriptor) {
      // TODO(lucacasonato): this should be real async
      return Promise.resolve(this.createRenderPipeline(descriptor));
    }

    /**
     * @param {GPUCommandEncoderDescriptor} descriptor
     * @returns {GPUCommandEncoder}
     */
    createCommandEncoder(descriptor = {}) {
      webidl.assertBranded(this, GPUDevice);
      const prefix = "Failed to execute 'createCommandEncoder' on 'GPUDevice'";
      descriptor = webidl.converters.GPUCommandEncoderDescriptor(descriptor, {
        prefix,
        context: "Argument 1",
      });
      const { rid } = core.jsonOpSync("op_webgpu_create_command_encoder", {
        deviceRid: this[_device].rid,
        ...descriptor,
      });

      const commandEncoder = createGPUCommandEncoder(
        descriptor.label ?? null,
        this[_device],
        rid,
      );
      this[_device].resources.push(new WeakRef(commandEncoder));
      return commandEncoder;
    }

    /**
     * @param {GPURenderBundleEncoderDescriptor} descriptor
     * @returns {GPURenderBundleEncoder}
     */
    createRenderBundleEncoder(descriptor) {
      webidl.assertBranded(this, GPUDevice);
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
      const { rid } = core.jsonOpSync(
        "op_webgpu_create_render_bundle_encoder",
        {
          deviceRid: this[_device].rid,
          ...descriptor,
        },
      );

      const renderBundleEncoder = createGPURenderBundleEncoder(
        descriptor.label ?? null,
        this[_device],
        rid,
      );
      this[_device].resources.push(new WeakRef(renderBundleEncoder));
      return renderBundleEncoder;
    }

    /**
     * @param {GPUQuerySetDescriptor} descriptor
     * @returns {GPUQuerySet}
     */
    createQuerySet(descriptor) {
      webidl.assertBranded(this, GPUDevice);
      const prefix = "Failed to execute 'createQuerySet' on 'GPUDevice'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      descriptor = webidl.converters.GPUQuerySetDescriptor(
        descriptor,
        {
          prefix,
          context: "Argument 1",
        },
      );
      const { rid } = core.jsonOpSync("op_webgpu_create_query_set", {
        deviceRid: this[_device].rid,
        ...descriptor,
      });

      const querySet = createGPUQuerySet(
        descriptor.label ?? null,
        this[_device],
        rid,
        descriptor,
      );
      this[_device].resources.push(new WeakRef(querySet));
      return querySet;
    }

    [Symbol.for("Deno.customInspect")](inspect) {
      return `${this.constructor.name} ${
        inspect({
          adapter: this.adapter,
          features: this.features,
          label: this.label,
          limits: this.limits,
          queue: this.queue,
        })
      }`;
    }
  }
  GPUObjectBaseMixin("GPUDevice", GPUDevice);

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
      webidl.assertBranded(this, GPUQueue);
      const prefix = "Failed to execute 'submit' on 'GPUQueue'";
      webidl.requiredArguments(arguments.length, 1, {
        prefix,
      });
      commandBuffers = webidl.converters["sequence<GPUCommandBuffer>"](
        commandBuffers,
        { prefix, context: "Argument 1" },
      );
      core.jsonOpSync("op_webgpu_queue_submit", {
        queueRid: this[_device].rid,
        commandBuffers: commandBuffers.map((buffer) => buffer[_rid]),
      });
    }

    onSubmittedWorkDone() {
      webidl.assertBranded(this, GPUQueue);
      return Promise.resolve();
    }

    /**
     * @param {GPUBuffer} buffer
     * @param {number} bufferOffset
     * @param {BufferSource} data
     * @param {number} [dataOffset]
     * @param {number} [size]
     */
    writeBuffer(buffer, bufferOffset, data, dataOffset = 0, size) {
      webidl.assertBranded(this, GPUQueue);
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
      core.jsonOpSync(
        "op_webgpu_write_buffer",
        {
          queueRid: this[_device].rid,
          buffer: buffer[_rid],
          bufferOffset,
          dataOffset,
          size,
        },
        new Uint8Array(ArrayBuffer.isView(data) ? data.buffer : data),
      );
    }

    /**
     * @param {GPUImageCopyTexture} destination
     * @param {BufferSource} data
     * @param {GPUImageDataLayout} dataLayout
     * @param {GPUExtent3D} size
     */
    writeTexture(destination, data, dataLayout, size) {
      webidl.assertBranded(this, GPUQueue);
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

      core.jsonOpSync(
        "op_webgpu_write_texture",
        {
          queueRid: this[_device].rid,
          destination: {
            texture: destination.texture[_rid],
            mipLevel: destination.mipLevel,
            origin: destination.origin ??
              normalizeGPUOrigin3D(destination.origin),
          },
          dataLayout,
          size: normalizeGPUExtent3D(size),
        },
        new Uint8Array(ArrayBuffer.isView(data) ? data.buffer : data),
      );
    }

    copyImageBitmapToTexture(_source, _destination, _copySize) {
      throw new Error("Not yet implemented");
    }

    [Symbol.for("Deno.customInspect")](inspect) {
      return `${this.constructor.name} ${
        inspect({
          label: this.label,
        })
      }`;
    }
  }
  GPUObjectBaseMixin("GPUQueue", GPUQueue);

  const _rid = Symbol("[[rid]]");

  const _size = Symbol("[[size]]");
  const _usage = Symbol("[[usage]]");
  const _state = Symbol("[[state]]");
  const _mappingRange = Symbol("[[mapping_range]]");
  const _mappedRanges = Symbol("[[mapped_ranges]]");
  const _mapMode = Symbol("[[map_mode]]");

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
          const mappedRange = mappedRanges.pop();
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
      webidl.assertBranded(this, GPUBuffer);
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
      /** @type {number} */
      let rangeSize;
      if (size === undefined) {
        rangeSize = Math.max(0, this[_size] - offset);
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
      await core.jsonOpAsync(
        "op_webgpu_buffer_get_map_async",
        {
          bufferRid: this[_rid],
          deviceRid: this[_device].rid,
          mode,
          offset,
          size: rangeSize,
        },
      );
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
      webidl.assertBranded(this, GPUBuffer);
      const prefix = "Failed to execute 'getMappedRange' on 'GPUBuffer'";
      offset = webidl.converters.GPUSize64(offset, {
        prefix,
        context: "Argument 1",
      });
      size = size === undefined
        ? undefined
        : webidl.converters.GPUSize64(size, {
          prefix,
          context: "Argument 2",
        });
      /** @type {number} */
      let rangeSize;
      if (size === undefined) {
        rangeSize = Math.max(0, this[_size] - offset);
      } else {
        rangeSize = this[_size];
      }
      if (this[_state] !== "mapped" && this[_state] !== "mapped at creation") {
        throw new DOMException(
          `${prefix}: buffer is not mapped.`,
          "OperationError",
        );
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
      const mappingRange = this[_mappingRange];
      if (!mappingRange) {
        throw new DOMException(`${prefix}: invalid state.`, "OperationError");
      }
      if (offset < mappingRange[0]) {
        throw new DOMException(
          `${prefix}: offset is out of bounds.`,
          "OperationError",
        );
      }
      if ((offset + rangeSize) > mappingRange[1]) {
        throw new DOMException(
          `${prefix}: offset is out of bounds.`,
          "OperationError",
        );
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
      const { rid } = core.jsonOpSync(
        "op_webgpu_buffer_get_mapped_range",
        {
          bufferRid: this[_rid],
          offset: offset - mappingRange[0],
          size: rangeSize,
        },
        new Uint8Array(buffer),
      );

      mappedRanges.push([buffer, rid, offset]);

      return buffer;
    }

    unmap() {
      webidl.assertBranded(this, GPUBuffer);
      const prefix = "Failed to execute 'unmap' on 'GPUBuffer'";
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
        for (const [buffer, rid] of mappedRanges) {
          core.jsonOpSync("op_webgpu_buffer_unmap", {
            bufferRid: this[_rid],
            mappedRid: rid,
          }, ...(write ? [new Uint8Array(buffer)] : []));
        }
        this[_mappingRange] = null;
        this[_mappedRanges] = null;
      }

      this[_state] = "unmapped";
    }

    destroy() {
      webidl.assertBranded(this, GPUBuffer);
      this[_cleanup]();
    }

    [Symbol.for("Deno.customInspect")](inspect) {
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

  const _views = Symbol("[[views]]");

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
        const view = views.pop()?.deref();
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
      webidl.assertBranded(this, GPUTexture);
      const prefix = "Failed to execute 'createView' on 'GPUTexture'";
      webidl.requiredArguments(arguments.length, 0, { prefix });
      descriptor = webidl.converters.GPUTextureViewDescriptor(descriptor, {
        prefix,
        context: "Argument 1",
      });

      const { rid } = core.jsonOpSync("op_webgpu_create_texture_view", {
        textureRid: this[_rid],
        ...descriptor,
      });

      const textureView = createGPUTextureView(
        descriptor.label ?? null,
        this,
        rid,
      );
      this[_views].push(new WeakRef(textureView));
      return textureView;
    }

    destroy() {
      webidl.assertBranded(this, GPUTexture);
      this[_cleanup]();
    }

    [Symbol.for("Deno.customInspect")](inspect) {
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
    static get SAMPLED() {
      return 0x04;
    }
    static get STORAGE() {
      return 0x08;
    }
    static get RENDER_ATTACHMENT() {
      return 0x10;
    }
  }

  const _texture = Symbol("[[texture]]");

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

    [Symbol.for("Deno.customInspect")](inspect) {
      return `${this.constructor.name} ${
        inspect({
          label: this.label,
        })
      }`;
    }
  }
  GPUObjectBaseMixin("GPUTextureView", GPUTextureView);

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

    [Symbol.for("Deno.customInspect")](inspect) {
      return `${this.constructor.name} ${
        inspect({
          label: this.label,
        })
      }`;
    }
  }
  GPUObjectBaseMixin("GPUSampler", GPUSampler);

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

    [Symbol.for("Deno.customInspect")](inspect) {
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

    [Symbol.for("Deno.customInspect")](inspect) {
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

    [Symbol.for("Deno.customInspect")](inspect) {
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

    [Symbol.for("Deno.customInspect")](inspect) {
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
      webidl.assertBranded(this, GPURenderPipeline);
      const prefix =
        "Failed to execute 'getBindGroupLayout' on 'GPUComputePipeline'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      index = webidl.converters["unsigned long"](index, {
        prefix,
        context: "Argument 1",
      });
      const { rid, label } = core.jsonOpSync(
        "op_webgpu_compute_pipeline_get_bind_group_layout",
        {
          computePipelineRid: this[_rid],
          index,
        },
      );

      const bindGroupLayout = createGPUBindGroupLayout(
        label,
        this[_device],
        rid,
      );
      this[_device].resources.push(new WeakRef(bindGroupLayout));
      return bindGroupLayout;
    }

    [Symbol.for("Deno.customInspect")](inspect) {
      return `${this.constructor.name} ${
        inspect({
          label: this.label,
        })
      }`;
    }
  }
  GPUObjectBaseMixin("GPUComputePipeline", GPUComputePipeline);

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
      webidl.assertBranded(this, GPURenderPipeline);
      const prefix =
        "Failed to execute 'getBindGroupLayout' on 'GPURenderPipeline'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      index = webidl.converters["unsigned long"](index, {
        prefix,
        context: "Argument 1",
      });
      const { rid, label } = core.jsonOpSync(
        "op_webgpu_render_pipeline_get_bind_group_layout",
        {
          renderPipelineRid: this[_rid],
          index,
        },
      );

      const bindGroupLayout = createGPUBindGroupLayout(
        label,
        this[_device],
        rid,
      );
      this[_device].resources.push(new WeakRef(bindGroupLayout));
      return bindGroupLayout;
    }

    [Symbol.for("Deno.customInspect")](inspect) {
      return `${this.constructor.name} ${
        inspect({
          label: this.label,
        })
      }`;
    }
  }
  GPUObjectBaseMixin("GPURenderPipeline", GPURenderPipeline);

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

  const _encoders = Symbol("[[encoders]]");

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
        const encoder = encoders.pop()?.deref();
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
      if (this[_rid] === undefined) {
        throw new DOMException(
          "Failed to execute 'beginRenderPass' on 'GPUCommandEncoder': already consumed",
          "OperationError",
        );
      }

      webidl.assertBranded(this, GPUCommandEncoder);
      const prefix =
        "Failed to execute 'beginRenderPass' on 'GPUCommandEncoder'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      descriptor = webidl.converters.GPURenderPassDescriptor(descriptor, {
        prefix,
        context: "Argument 1",
      });

      let depthStencilAttachment;
      if (descriptor.depthStencilAttachment) {
        depthStencilAttachment = {
          ...descriptor.depthStencilAttachment,
          view: descriptor.depthStencilAttachment.view[_rid],
        };

        if (
          typeof descriptor.depthStencilAttachment.depthLoadValue === "string"
        ) {
          depthStencilAttachment.depthLoadOp =
            descriptor.depthStencilAttachment.depthLoadValue;
        } else {
          depthStencilAttachment.depthLoadOp = "clear";
          depthStencilAttachment.depthLoadValue =
            descriptor.depthStencilAttachment.depthLoadValue;
        }

        if (
          typeof descriptor.depthStencilAttachment.stencilLoadValue === "string"
        ) {
          depthStencilAttachment.stencilLoadOp =
            descriptor.depthStencilAttachment.stencilLoadValue;
          depthStencilAttachment.stencilLoadValue = undefined;
        } else {
          depthStencilAttachment.stencilLoadOp = "clear";
          depthStencilAttachment.stencilLoadValue =
            descriptor.depthStencilAttachment.stencilLoadValue;
        }
      }

      const { rid } = core.jsonOpSync(
        "op_webgpu_command_encoder_begin_render_pass",
        {
          commandEncoderRid: this[_rid],
          ...descriptor,
          colorAttachments: descriptor.colorAttachments.map(
            (colorAttachment) => {
              const attachment = {
                view: colorAttachment.view[_rid],
                resolveTarget: colorAttachment.resolveTarget
                  ? colorAttachment.resolveTarget[_rid]
                  : undefined,
                storeOp: colorAttachment.storeOp,
              };

              if (typeof colorAttachment.loadValue === "string") {
                attachment.loadOp = colorAttachment.loadValue;
              } else {
                attachment.loadOp = "clear";
                attachment.loadValue = normalizeGPUColor(
                  colorAttachment.loadValue,
                );
              }

              return attachment;
            },
          ),
          depthStencilAttachment,
        },
      );

      const renderPassEncoder = createGPURenderPassEncoder(
        descriptor.label ?? null,
        this,
        rid,
      );
      this[_encoders].push(new WeakRef(renderPassEncoder));
      return renderPassEncoder;
    }

    /**
     * @param {GPUComputePassDescriptor} descriptor
     */
    beginComputePass(descriptor = {}) {
      if (this[_rid] === undefined) {
        throw new DOMException(
          "Failed to execute 'beginComputePass' on 'GPUCommandEncoder': already consumed",
          "OperationError",
        );
      }

      webidl.assertBranded(this, GPUCommandEncoder);
      const prefix =
        "Failed to execute 'beginComputePass' on 'GPUCommandEncoder'";
      descriptor = webidl.converters.GPUComputePassDescriptor(descriptor, {
        prefix,
        context: "Argument 1",
      });

      const { rid } = core.jsonOpSync(
        "op_webgpu_command_encoder_begin_compute_pass",
        {
          commandEncoderRid: this[_rid],
          ...descriptor,
        },
      );

      const computePassEncoder = createGPUComputePassEncoder(
        descriptor.label ?? null,
        this[_rid],
        rid,
      );
      this[_encoders].push(new WeakRef(computePassEncoder));
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
      if (this[_rid] === undefined) {
        throw new DOMException(
          "Failed to execute 'copyBufferToBuffer' on 'GPUCommandEncoder': already consumed",
          "OperationError",
        );
      }

      webidl.assertBranded(this, GPUCommandEncoder);
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
      core.jsonOpSync(
        "op_webgpu_command_encoder_copy_buffer_to_buffer",
        {
          commandEncoderRid: this[_rid],
          source: source[_rid],
          sourceOffset,
          destination: destination[_rid],
          destinationOffset,
          size,
        },
      );
    }

    /**
     * @param {GPUImageCopyBuffer} source
     * @param {GPUImageCopyTexture} destination
     * @param {GPUExtent3D} copySize
     */
    copyBufferToTexture(source, destination, copySize) {
      if (this[_rid] === undefined) {
        throw new DOMException(
          "Failed to execute 'copyBufferToTexture' on 'GPUCommandEncoder': already consumed",
          "OperationError",
        );
      }

      webidl.assertBranded(this, GPUCommandEncoder);
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

      core.jsonOpSync(
        "op_webgpu_command_encoder_copy_buffer_to_texture",
        {
          commandEncoderRid: this[_rid],
          source: {
            ...source,
            buffer: source.buffer[_rid],
          },
          destination: {
            texture: destination.texture[_rid],
            mipLevel: destination.mipLevel,
            origin: destination.origin ??
              normalizeGPUOrigin3D(destination.origin),
          },
          copySize: normalizeGPUExtent3D(copySize),
        },
      );
    }

    /**
     * @param {GPUImageCopyTexture} source
     * @param {GPUImageCopyBuffer} destination
     * @param {GPUExtent3D} copySize
     */
    copyTextureToBuffer(source, destination, copySize) {
      if (this[_rid] === undefined) {
        throw new DOMException(
          "Failed to execute 'copyTextureToBuffer' on 'GPUCommandEncoder': already consumed",
          "OperationError",
        );
      }

      webidl.assertBranded(this, GPUCommandEncoder);
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
      core.jsonOpSync(
        "op_webgpu_command_encoder_copy_texture_to_buffer",
        {
          commandEncoderRid: this[_rid],
          source: {
            texture: source.texture[_rid],
            mipLevel: source.mipLevel,
            origin: source.origin ?? normalizeGPUOrigin3D(source.origin),
          },
          destination: {
            ...destination,
            buffer: destination.buffer[_rid],
          },
          copySize: normalizeGPUExtent3D(copySize),
        },
      );
    }

    /**
     * @param {GPUImageCopyTexture} source
     * @param {GPUImageCopyTexture} destination
     * @param {GPUExtent3D} copySize
     */
    copyTextureToTexture(source, destination, copySize) {
      if (this[_rid] === undefined) {
        throw new DOMException(
          "Failed to execute 'copyTextureToTexture' on 'GPUCommandEncoder': already consumed",
          "OperationError",
        );
      }

      webidl.assertBranded(this, GPUCommandEncoder);
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
      core.jsonOpSync(
        "op_webgpu_command_encoder_copy_texture_to_texture",
        {
          commandEncoderRid: this[_rid],
          source: {
            texture: source.texture[_rid],
            mipLevel: source.mipLevel,
            origin: source.origin ?? normalizeGPUOrigin3D(source.origin),
          },
          destination: {
            texture: destination.texture[_rid],
            mipLevel: destination.mipLevel,
            origin: destination.origin ??
              normalizeGPUOrigin3D(destination.origin),
          },
          copySize: normalizeGPUExtent3D(copySize),
        },
      );
    }

    /**
     * @param {string} groupLabel
     */
    pushDebugGroup(groupLabel) {
      if (this[_rid] === undefined) {
        throw new DOMException(
          "Failed to execute 'pushDebugGroup' on 'GPUCommandEncoder': already consumed",
          "OperationError",
        );
      }

      webidl.assertBranded(this, GPUCommandEncoder);
      const prefix =
        "Failed to execute 'pushDebugGroup' on 'GPUCommandEncoder'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      groupLabel = webidl.converters.USVString(groupLabel, {
        prefix,
        context: "Argument 1",
      });
      core.jsonOpSync("op_webgpu_command_encoder_push_debug_group", {
        commandEncoderRid: this[_rid],
        groupLabel,
      });
    }

    popDebugGroup() {
      if (this[_rid] === undefined) {
        throw new DOMException(
          "Failed to execute 'popDebugGroup' on 'GPUCommandEncoder': already consumed",
          "OperationError",
        );
      }

      webidl.assertBranded(this, GPUCommandEncoder);
      core.jsonOpSync("op_webgpu_command_encoder_pop_debug_group", {
        commandEncoderRid: this[_rid],
      });
    }

    /**
     * @param {string} markerLabel
     */
    insertDebugMarker(markerLabel) {
      if (this[_rid] === undefined) {
        throw new DOMException(
          "Failed to execute 'insertDebugMarker' on 'GPUCommandEncoder': already consumed",
          "OperationError",
        );
      }

      webidl.assertBranded(this, GPUCommandEncoder);
      const prefix =
        "Failed to execute 'insertDebugMarker' on 'GPUCommandEncoder'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      markerLabel = webidl.converters.USVString(markerLabel, {
        prefix,
        context: "Argument 1",
      });
      core.jsonOpSync("op_webgpu_command_encoder_insert_debug_marker", {
        commandEncoderRid: this[_rid],
        markerLabel,
      });
    }

    /**
     * @param {GPUQuerySet} querySet
     * @param {number} queryIndex
     */
    writeTimestamp(querySet, queryIndex) {
      if (this[_rid] === undefined) {
        throw new DOMException(
          "Failed to execute 'writeTimestamp' on 'GPUCommandEncoder': already consumed",
          "OperationError",
        );
      }

      webidl.assertBranded(this, GPUCommandEncoder);
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
      core.jsonOpSync("op_webgpu_command_encoder_write_timestamp", {
        commandEncoderRid: this[_rid],
        querySet: querySet[_rid],
        queryIndex,
      });
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
      if (this[_rid] === undefined) {
        throw new DOMException(
          "Failed to execute 'resolveQuerySet' on 'GPUCommandEncoder': already consumed",
          "OperationError",
        );
      }

      webidl.assertBranded(this, GPUCommandEncoder);
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
      destination = webidl.converters.GPUQuerySet(destination, {
        prefix,
        context: "Argument 4",
      });
      destinationOffset = webidl.converters.GPUSize64(destinationOffset, {
        prefix,
        context: "Argument 5",
      });
      core.jsonOpSync("op_webgpu_command_encoder_resolve_query_set", {
        commandEncoderRid: this[_rid],
        querySet: querySet[_rid],
        firstQuery,
        queryCount,
        destination: destination[_rid],
        destinationOffset,
      });
    }

    /**
     * @param {GPUCommandBufferDescriptor} descriptor
     * @returns {GPUCommandBuffer}
     */
    finish(descriptor = {}) {
      if (this[_rid] === undefined) {
        throw new DOMException(
          "Failed to execute 'finish' on 'GPUCommandEncoder': already consumed",
          "OperationError",
        );
      }

      webidl.assertBranded(this, GPUCommandEncoder);
      const prefix = "Failed to execute 'finish' on 'GPUCommandEncoder'";
      descriptor = webidl.converters.GPUCommandBufferDescriptor(descriptor, {
        prefix,
        context: "Argument 1",
      });
      const { rid } = core.jsonOpSync("op_webgpu_command_encoder_finish", {
        commandEncoderRid: this[_rid],
        ...descriptor,
      });
      /** @type {number | undefined} */
      this[_rid] = undefined;

      const commandBuffer = createGPUCommandBuffer(
        descriptor.label ?? null,
        this[_device],
        rid,
      );
      this[_device].resources.push(new WeakRef(commandBuffer));
      return commandBuffer;
    }

    [Symbol.for("Deno.customInspect")](inspect) {
      return `${this.constructor.name} ${
        inspect({
          label: this.label,
        })
      }`;
    }
  }
  GPUObjectBaseMixin("GPUCommandEncoder", GPUCommandEncoder);

  const _encoder = Symbol("[[encoder]]");

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
      if (this[_rid] === undefined) {
        throw new DOMException(
          "Failed to execute 'setViewport' on 'GPURenderPassEncoder': already consumed",
          "OperationError",
        );
      }

      webidl.assertBranded(this, GPURenderPassEncoder);
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
      core.jsonOpSync("op_webgpu_render_pass_set_viewport", {
        renderPassRid: this[_rid],
        x,
        y,
        width,
        height,
        minDepth,
        maxDepth,
      });
    }

    /**
     *
     * @param {number} x
     * @param {number} y
     * @param {number} width
     * @param {number} height
     */
    setScissorRect(x, y, width, height) {
      if (this[_rid] === undefined) {
        throw new DOMException(
          "Failed to execute 'setScissorRect' on 'GPURenderPassEncoder': already consumed",
          "OperationError",
        );
      }

      webidl.assertBranded(this, GPURenderPassEncoder);
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
      core.jsonOpSync("op_webgpu_render_pass_set_scissor_rect", {
        renderPassRid: this[_rid],
        x,
        y,
        width,
        height,
      });
    }

    /**
     * @param {GPUColor} color
     */
    setBlendColor(color) {
      if (this[_rid] === undefined) {
        throw new DOMException(
          "Failed to execute 'setBlendColor' on 'GPURenderPassEncoder': already consumed",
          "OperationError",
        );
      }

      webidl.assertBranded(this, GPURenderPassEncoder);
      const prefix =
        "Failed to execute 'setBlendColor' on 'GPUComputePassEncoder'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      color = webidl.converters.GPUColor(color, {
        prefix,
        context: "Argument 1",
      });
      core.jsonOpSync("op_webgpu_render_pass_set_blend_color", {
        renderPassRid: this[_rid],
        color: normalizeGPUColor(color),
      });
    }

    /**
     * @param {number} reference
     */
    setStencilReference(reference) {
      if (this[_rid] === undefined) {
        throw new DOMException(
          "Failed to execute 'setStencilReference' on 'GPURenderPassEncoder': already consumed",
          "OperationError",
        );
      }

      webidl.assertBranded(this, GPURenderPassEncoder);
      const prefix =
        "Failed to execute 'setStencilReference' on 'GPUComputePassEncoder'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      reference = webidl.converters.GPUStencilValue(reference, {
        prefix,
        context: "Argument 1",
      });
      core.jsonOpSync("op_webgpu_render_pass_set_stencil_reference", {
        renderPassRid: this[_rid],
        reference,
      });
    }

    beginOcclusionQuery(_queryIndex) {
      if (this[_rid] === undefined) {
        throw new DOMException(
          "Failed to execute 'beginOcclusionQuery' on 'GPURenderPassEncoder': already consumed",
          "OperationError",
        );
      }

      throw new Error("Not yet implemented");
    }

    endOcclusionQuery() {
      if (this[_rid] === undefined) {
        throw new DOMException(
          "Failed to execute 'endOcclusionQuery' on 'GPURenderPassEncoder': already consumed",
          "OperationError",
        );
      }

      throw new Error("Not yet implemented");
    }

    /**
     * @param {GPUQuerySet} querySet
     * @param {number} queryIndex
     */
    beginPipelineStatisticsQuery(querySet, queryIndex) {
      if (this[_rid] === undefined) {
        throw new DOMException(
          "Failed to execute 'beginPipelineStatisticsQuery' on 'GPURenderPassEncoder': already consumed",
          "OperationError",
        );
      }

      webidl.assertBranded(this, GPURenderPassEncoder);
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
      core.jsonOpSync("op_webgpu_render_pass_begin_pipeline_statistics_query", {
        renderPassRid: this[_rid],
        querySet: querySet[_rid],
        queryIndex,
      });
    }

    endPipelineStatisticsQuery() {
      if (this[_rid] === undefined) {
        throw new DOMException(
          "Failed to execute 'endPipelineStatisticsQuery' on 'GPURenderPassEncoder': already consumed",
          "OperationError",
        );
      }

      webidl.assertBranded(this, GPURenderPassEncoder);
      core.jsonOpSync("op_webgpu_render_pass_end_pipeline_statistics_query", {
        renderPassRid: this[_rid],
      });
    }

    /**
     * @param {GPUQuerySet} querySet
     * @param {number} queryIndex
     */
    writeTimestamp(querySet, queryIndex) {
      if (this[_rid] === undefined) {
        throw new DOMException(
          "Failed to execute 'writeTimestamp' on 'GPURenderPassEncoder': already consumed",
          "OperationError",
        );
      }

      webidl.assertBranded(this, GPURenderPassEncoder);
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
      core.jsonOpSync("op_webgpu_render_pass_write_timestamp", {
        renderPassRid: this[_rid],
        querySet: querySet[_rid],
        queryIndex,
      });
    }

    /**
     * @param {GPURenderBundle[]} bundles
     */
    executeBundles(bundles) {
      if (this[_rid] === undefined) {
        throw new DOMException(
          "Failed to execute 'executeBundles' on 'GPURenderPassEncoder': already consumed",
          "OperationError",
        );
      }

      webidl.assertBranded(this, GPURenderPassEncoder);
      const prefix =
        "Failed to execute 'executeBundles' on 'GPURenderPassEncoder'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      bundles = webidl.converters["sequence<GPURenderBundle>"](bundles, {
        prefix,
        context: "Argument 1",
      });
      core.jsonOpSync("op_webgpu_render_pass_execute_bundles", {
        renderPassRid: this[_rid],
        bundles: bundles.map((bundle) => bundle[_rid]),
      });
    }

    endPass() {
      if (this[_rid] === undefined) {
        throw new DOMException(
          "Failed to execute 'endPass' on 'GPURenderPassEncoder': already consumed",
          "OperationError",
        );
      }

      core.jsonOpSync("op_webgpu_render_pass_end_pass", {
        commandEncoderRid: this[_encoder],
        renderPassRid: this[_rid],
      });
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
      if (this[_rid] === undefined) {
        throw new DOMException(
          "Failed to execute 'setBindGroup' on 'GPURenderPassEncoder': already consumed",
          "OperationError",
        );
      }

      const bind = bindGroup[_rid];
      if (dynamicOffsetsData instanceof Uint32Array) {
        core.jsonOpSync(
          "op_webgpu_render_pass_set_bind_group",
          {
            renderPassRid: this[_rid],
            index,
            bindGroup: bind,
            dynamicOffsetsDataStart,
            dynamicOffsetsDataLength,
          },
          dynamicOffsetsData,
        );
      } else {
        dynamicOffsetsData ??= [];
        core.jsonOpSync("op_webgpu_render_pass_set_bind_group", {
          renderPassRid: this[_rid],
          index,
          bindGroup: bind,
          dynamicOffsetsData,
          dynamicOffsetsDataStart: 0,
          dynamicOffsetsDataLength: dynamicOffsetsData.length,
        });
      }
    }

    /**
     * @param {string} groupLabel
     */
    pushDebugGroup(groupLabel) {
      if (this[_rid] === undefined) {
        throw new DOMException(
          "Failed to execute 'pushDebugGroup' on 'GPURenderPassEncoder': already consumed",
          "OperationError",
        );
      }

      webidl.assertBranded(this, GPURenderPassEncoder);
      const prefix =
        "Failed to execute 'pushDebugGroup' on 'GPURenderPassEncoder'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      groupLabel = webidl.converters.USVString(groupLabel, {
        prefix,
        context: "Argument 1",
      });
      core.jsonOpSync("op_webgpu_render_pass_push_debug_group", {
        renderPassRid: this[_rid],
        groupLabel,
      });
    }

    popDebugGroup() {
      if (this[_rid] === undefined) {
        throw new DOMException(
          "Failed to execute 'popDebugGroup' on 'GPURenderPassEncoder': already consumed",
          "OperationError",
        );
      }

      webidl.assertBranded(this, GPURenderPassEncoder);
      core.jsonOpSync("op_webgpu_render_pass_pop_debug_group", {
        renderPassRid: this[_rid],
      });
    }

    /**
     * @param {string} markerLabel
     */
    insertDebugMarker(markerLabel) {
      if (this[_rid] === undefined) {
        throw new DOMException(
          "Failed to execute 'insertDebugMarker' on 'GPURenderPassEncoder': already consumed",
          "OperationError",
        );
      }

      webidl.assertBranded(this, GPURenderPassEncoder);
      const prefix =
        "Failed to execute 'insertDebugMarker' on 'GPURenderPassEncoder'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      markerLabel = webidl.converters.USVString(markerLabel, {
        prefix,
        context: "Argument 1",
      });
      core.jsonOpSync("op_webgpu_render_pass_insert_debug_marker", {
        renderPassRid: this[_rid],
        markerLabel,
      });
    }

    /**
     * @param {GPURenderPipeline} pipeline
     */
    setPipeline(pipeline) {
      if (this[_rid] === undefined) {
        throw new DOMException(
          "Failed to execute 'setPipeline' on 'GPURenderPassEncoder': already consumed",
          "OperationError",
        );
      }

      webidl.assertBranded(this, GPURenderPassEncoder);
      const prefix =
        "Failed to execute 'setPipeline' on 'GPURenderPassEncoder'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      pipeline = webidl.converters.GPURenderPipeline(pipeline, {
        prefix,
        context: "Argument 1",
      });
      core.jsonOpSync("op_webgpu_render_pass_set_pipeline", {
        renderPassRid: this[_rid],
        pipeline: pipeline[_rid],
      });
    }

    /**
     * @param {GPUBuffer} buffer
     * @param {GPUIndexFormat} indexFormat
     * @param {number} offset
     * @param {number} size
     */
    setIndexBuffer(buffer, indexFormat, offset = 0, size = 0) {
      if (this[_rid] === undefined) {
        throw new DOMException(
          "Failed to execute 'setIndexBuffer' on 'GPURenderPassEncoder': already consumed",
          "OperationError",
        );
      }

      webidl.assertBranded(this, GPURenderPassEncoder);
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
      size = webidl.converters.GPUSize64(size, {
        prefix,
        context: "Argument 4",
      });
      core.jsonOpSync("op_webgpu_render_pass_set_index_buffer", {
        renderPassRid: this[_rid],
        buffer: buffer[_rid],
        indexFormat,
        offset,
        size,
      });
    }

    /**
     * @param {number} slot
     * @param {GPUBuffer} buffer
     * @param {number} offset
     * @param {number} size
     */
    setVertexBuffer(slot, buffer, offset = 0, size = 0) {
      if (this[_rid] === undefined) {
        throw new DOMException(
          "Failed to execute 'setVertexBuffer' on 'GPURenderPassEncoder': already consumed",
          "OperationError",
        );
      }

      webidl.assertBranded(this, GPURenderPassEncoder);
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
      size = webidl.converters.GPUSize64(size, {
        prefix,
        context: "Argument 4",
      });
      core.jsonOpSync("op_webgpu_render_pass_set_vertex_buffer", {
        renderPassRid: this[_rid],
        slot,
        buffer: buffer[_rid],
        offset,
        size,
      });
    }

    /**
     * @param {number} vertexCount
     * @param {number} instanceCount
     * @param {number} firstVertex
     * @param {number} firstInstance
     */
    draw(vertexCount, instanceCount = 1, firstVertex = 0, firstInstance = 0) {
      if (this[_rid] === undefined) {
        throw new DOMException(
          "Failed to execute 'draw' on 'GPURenderPassEncoder': already consumed",
          "OperationError",
        );
      }

      webidl.assertBranded(this, GPURenderPassEncoder);
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
      core.jsonOpSync("op_webgpu_render_pass_draw", {
        renderPassRid: this[_rid],
        vertexCount,
        instanceCount,
        firstVertex,
        firstInstance,
      });
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
      if (this[_rid] === undefined) {
        throw new DOMException(
          "Failed to execute 'drawIndexed' on 'GPURenderPassEncoder': already consumed",
          "OperationError",
        );
      }

      webidl.assertBranded(this, GPURenderPassEncoder);
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
      core.jsonOpSync("op_webgpu_render_pass_draw_indexed", {
        renderPassRid: this[_rid],
        indexCount,
        instanceCount,
        firstIndex,
        baseVertex,
        firstInstance,
      });
    }

    /**
     * @param {GPUBuffer} indirectBuffer
     * @param {number} indirectOffset
     */
    drawIndirect(indirectBuffer, indirectOffset) {
      if (this[_rid] === undefined) {
        throw new DOMException(
          "Failed to execute 'drawIndirect' on 'GPURenderPassEncoder': already consumed",
          "OperationError",
        );
      }

      webidl.assertBranded(this, GPURenderPassEncoder);
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
      core.jsonOpSync("op_webgpu_render_pass_draw_indirect", {
        renderPassRid: this[_rid],
        indirectBuffer: indirectBuffer[_rid],
        indirectOffset,
      });
    }

    /**
     * @param {GPUBuffer} indirectBuffer
     * @param {number} indirectOffset
     */
    drawIndexedIndirect(indirectBuffer, indirectOffset) {
      if (this[_rid] === undefined) {
        throw new DOMException(
          "Failed to execute 'drawIndexedIndirect' on 'GPURenderPassEncoder': already consumed",
          "OperationError",
        );
      }

      webidl.assertBranded(this, GPURenderPassEncoder);
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
      core.jsonOpSync("op_webgpu_render_pass_draw_indexed_indirect", {
        renderPassRid: this[_rid],
        indirectBuffer: indirectBuffer[_rid],
        indirectOffset,
      });
    }

    [Symbol.for("Deno.customInspect")](inspect) {
      return `${this.constructor.name} ${
        inspect({
          label: this.label,
        })
      }`;
    }
  }
  GPUObjectBaseMixin("GPURenderPassEncoder", GPURenderPassEncoder);

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
      if (this[_rid] === undefined) {
        throw new DOMException(
          "Failed to execute 'setPipeline' on 'GPUComputePassEncoder': already consumed",
          "OperationError",
        );
      }

      webidl.assertBranded(this, GPUComputePassEncoder);
      const prefix =
        "Failed to execute 'setPipeline' on 'GPUComputePassEncoder'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      pipeline = webidl.converters.GPUComputePipeline(pipeline, {
        prefix,
        context: "Argument 1",
      });
      core.jsonOpSync("op_webgpu_compute_pass_set_pipeline", {
        computePassRid: this[_rid],
        pipeline: pipeline[_rid],
      });
    }

    /**
     * @param {number} x
     * @param {number} y
     * @param {number} z
     */
    dispatch(x, y = 1, z = 1) {
      if (this[_rid] === undefined) {
        throw new DOMException(
          "Failed to execute 'dispatch' on 'GPUComputePassEncoder': already consumed",
          "OperationError",
        );
      }

      webidl.assertBranded(this, GPUComputePassEncoder);
      const prefix = "Failed to execute 'dispatch' on 'GPUComputePassEncoder'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      x = webidl.converters.GPUSize32(x, { prefix, context: "Argument 1" });
      y = webidl.converters.GPUSize32(y, { prefix, context: "Argument 2" });
      z = webidl.converters.GPUSize32(z, { prefix, context: "Argument 3" });
      core.jsonOpSync("op_webgpu_compute_pass_dispatch", {
        computePassRid: this[_rid],
        x,
        y,
        z,
      });
    }

    /**
     * @param {GPUBuffer} indirectBuffer
     * @param {number} indirectOffset
     */
    dispatchIndirect(indirectBuffer, indirectOffset) {
      if (this[_rid] === undefined) {
        throw new DOMException(
          "Failed to execute 'dispatchIndirect' on 'GPUComputePassEncoder': already consumed",
          "OperationError",
        );
      }

      webidl.assertBranded(this, GPUComputePassEncoder);
      const prefix =
        "Failed to execute 'dispatchIndirect' on 'GPUComputePassEncoder'";
      webidl.requiredArguments(arguments.length, 2, { prefix });
      indirectBuffer = webidl.converters.GPUBuffer(indirectBuffer, {
        prefix,
        context: "Argument 1",
      });
      indirectOffset = webidl.converters.GPUSize64(indirectOffset, {
        prefix,
        context: "Argument 2",
      });
      core.jsonOpSync("op_webgpu_compute_pass_dispatch_indirect", {
        computePassRid: this[_rid],
        indirectBuffer: indirectBuffer[_rid],
        indirectOffset,
      });
    }

    /**
     * @param {GPUQuerySet} querySet
     * @param {number} queryIndex
     */
    beginPipelineStatisticsQuery(querySet, queryIndex) {
      if (this[_rid] === undefined) {
        throw new DOMException(
          "Failed to execute 'beginPipelineStatisticsQuery' on 'GPUComputePassEncoder': already consumed",
          "OperationError",
        );
      }

      webidl.assertBranded(this, GPUComputePassEncoder);
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
      core.jsonOpSync(
        "op_webgpu_compute_pass_begin_pipeline_statistics_query",
        {
          computePassRid: this[_rid],
          querySet: querySet[_rid],
          queryIndex,
        },
      );
    }

    endPipelineStatisticsQuery() {
      if (this[_rid] === undefined) {
        throw new DOMException(
          "Failed to execute 'endPipelineStatisticsQuery' on 'GPUComputePassEncoder': already consumed",
          "OperationError",
        );
      }

      webidl.assertBranded(this, GPUComputePassEncoder);
      core.jsonOpSync("op_webgpu_compute_pass_end_pipeline_statistics_query", {
        computePassRid: this[_rid],
      });
    }

    /**
     * @param {GPUQuerySet} querySet
     * @param {number} queryIndex
     */
    writeTimestamp(querySet, queryIndex) {
      if (this[_rid] === undefined) {
        throw new DOMException(
          "Failed to execute 'writeTimestamp' on 'GPUComputePassEncoder': already consumed",
          "OperationError",
        );
      }

      webidl.assertBranded(this, GPUComputePassEncoder);
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
      const querySetRid = querySet[_rid];
      if (querySetRid === undefined) {
        throw new GPUValidationError(`${prefix}: GPUQuerySet is not valid.`);
      }

      core.jsonOpSync("op_webgpu_compute_pass_write_timestamp", {
        computePassRid: this[_rid],
        querySet: querySet[_rid],
        queryIndex,
      });
    }

    endPass() {
      if (this[_rid] === undefined) {
        throw new DOMException(
          "Failed to execute 'endPass' on 'GPUComputePassEncoder': already consumed",
          "OperationError",
        );
      }

      webidl.assertBranded(this, GPUComputePassEncoder);
      core.jsonOpSync("op_webgpu_compute_pass_end_pass", {
        commandEncoderRid: this[_encoder],
        computePassRid: this[_rid],
      });
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
      if (this[_rid] === undefined) {
        throw new DOMException(
          "Failed to execute 'setBindGroup' on 'GPUComputePassEncoder': already consumed",
          "OperationError",
        );
      }

      const bind = bindGroup[_rid];
      if (dynamicOffsetsData instanceof Uint32Array) {
        core.jsonOpSync(
          "op_webgpu_compute_pass_set_bind_group",
          {
            computePassRid: this[_rid],
            index,
            bindGroup: bind,
            dynamicOffsetsDataStart,
            dynamicOffsetsDataLength,
          },
          dynamicOffsetsData,
        );
      } else {
        dynamicOffsetsData ??= [];
        core.jsonOpSync("op_webgpu_compute_pass_set_bind_group", {
          computePassRid: this[_rid],
          index,
          bindGroup: bind,
          dynamicOffsetsData,
          dynamicOffsetsDataStart: 0,
          dynamicOffsetsDataLength: dynamicOffsetsData.length,
        });
      }
    }

    /**
     * @param {string} groupLabel
     */
    pushDebugGroup(groupLabel) {
      if (this[_rid] === undefined) {
        throw new DOMException(
          "Failed to execute 'pushDebugGroup' on 'GPUComputePassEncoder': already consumed",
          "OperationError",
        );
      }

      webidl.assertBranded(this, GPUComputePassEncoder);
      const prefix =
        "Failed to execute 'pushDebugGroup' on 'GPUComputePassEncoder'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      groupLabel = webidl.converters.USVString(groupLabel, {
        prefix,
        context: "Argument 1",
      });
      core.jsonOpSync("op_webgpu_compute_pass_push_debug_group", {
        computePassRid: this[_rid],
        groupLabel,
      });
    }

    popDebugGroup() {
      if (this[_rid] === undefined) {
        throw new DOMException(
          "Failed to execute 'popDebugGroup' on 'GPUComputePassEncoder': already consumed",
          "OperationError",
        );
      }

      webidl.assertBranded(this, GPUComputePassEncoder);
      core.jsonOpSync("op_webgpu_compute_pass_pop_debug_group", {
        computePassRid: this[_rid],
      });
    }

    /**
     * @param {string} markerLabel
     */
    insertDebugMarker(markerLabel) {
      if (this[_rid] === undefined) {
        throw new DOMException(
          "Failed to execute 'insertDebugMarker' on 'GPUComputePassEncoder': already consumed",
          "OperationError",
        );
      }

      webidl.assertBranded(this, GPUComputePassEncoder);
      const prefix =
        "Failed to execute 'insertDebugMarker' on 'GPUComputePassEncoder'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      markerLabel = webidl.converters.USVString(markerLabel, {
        prefix,
        context: "Argument 1",
      });
      core.jsonOpSync("op_webgpu_compute_pass_insert_debug_marker", {
        computePassRid: this[_rid],
        markerLabel,
      });
    }

    [Symbol.for("Deno.customInspect")](inspect) {
      return `${this.constructor.name} ${
        inspect({
          label: this.label,
        })
      }`;
    }
  }
  GPUObjectBaseMixin("GPUComputePassEncoder", GPUComputePassEncoder);

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

    get executionTime() {
      throw new Error("Not yet implemented");
    }

    [Symbol.for("Deno.customInspect")](inspect) {
      return `${this.constructor.name} ${
        inspect({
          label: this.label,
          // TODO(crowlKats): executionTime
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
      if (this[_rid] === undefined) {
        throw new DOMException(
          "Failed to execute 'finish' on 'GPURenderBundleEncoder': already consumed",
          "OperationError",
        );
      }

      webidl.assertBranded(this, GPURenderBundleEncoder);
      descriptor = webidl.converters.GPURenderBundleDescriptor(descriptor, {
        prefix: "Failed to execute 'finish' on 'GPURenderBundleEncoder'",
        context: "Argument 1",
      });
      const { rid } = core.jsonOpSync(
        "op_webgpu_render_bundle_encoder_finish",
        {
          renderBundleEncoderRid: this[_rid],
          ...descriptor,
        },
      );
      this[_rid] = undefined;

      const renderBundle = createGPURenderBundle(
        descriptor.label ?? null,
        this[_device],
        rid,
      );
      this[_device].resources.push(new WeakRef(renderBundle));
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
      if (this[_rid] === undefined) {
        throw new DOMException(
          "Failed to execute 'setBindGroup' on 'GPUComputePassEncoder': already consumed",
          "OperationError",
        );
      }

      const bind = bindGroup[_rid];
      if (dynamicOffsetsData instanceof Uint32Array) {
        core.jsonOpSync(
          "op_webgpu_render_bundle_encoder_set_bind_group",
          {
            renderBundleEncoderRid: this[_rid],
            index,
            bindGroup: bind,
            dynamicOffsetsDataStart,
            dynamicOffsetsDataLength,
          },
          dynamicOffsetsData,
        );
      } else {
        dynamicOffsetsData ??= [];
        core.jsonOpSync("op_webgpu_render_bundle_encoder_set_bind_group", {
          renderBundleEncoderRid: this[_rid],
          index,
          bindGroup: bind,
          dynamicOffsetsData,
          dynamicOffsetsDataStart: 0,
          dynamicOffsetsDataLength: dynamicOffsetsData.length,
        });
      }
    }

    /**
     * @param {string} groupLabel
     */
    pushDebugGroup(groupLabel) {
      if (this[_rid] === undefined) {
        throw new DOMException(
          "Failed to execute 'pushDebugGroup' on 'GPUComputePassEncoder': already consumed",
          "OperationError",
        );
      }

      webidl.assertBranded(this, GPURenderBundleEncoder);
      const prefix =
        "Failed to execute 'pushDebugGroup' on 'GPURenderBundleEncoder'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      groupLabel = webidl.converters.USVString(groupLabel, {
        prefix,
        context: "Argument 1",
      });
      core.jsonOpSync("op_webgpu_render_bundle_encoder_push_debug_group", {
        renderBundleEncoderRid: this[_rid],
        groupLabel,
      });
    }

    popDebugGroup() {
      if (this[_rid] === undefined) {
        throw new DOMException(
          "Failed to execute 'popDebugGroup' on 'GPUComputePassEncoder': already consumed",
          "OperationError",
        );
      }

      webidl.assertBranded(this, GPURenderBundleEncoder);
      core.jsonOpSync("op_webgpu_render_bundle_encoder_pop_debug_group", {
        renderBundleEncoderRid: this[_rid],
      });
    }

    /**
     * @param {string} markerLabel
     */
    insertDebugMarker(markerLabel) {
      if (this[_rid] === undefined) {
        throw new DOMException(
          "Failed to execute 'insertDebugMarker' on 'GPUComputePassEncoder': already consumed",
          "OperationError",
        );
      }

      webidl.assertBranded(this, GPURenderBundleEncoder);
      const prefix =
        "Failed to execute 'insertDebugMarker' on 'GPURenderBundleEncoder'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      markerLabel = webidl.converters.USVString(markerLabel, {
        prefix,
        context: "Argument 1",
      });
      core.jsonOpSync("op_webgpu_render_bundle_encoder_push_debug_group", {
        renderBundleEncoderRid: this[_rid],
        markerLabel,
      });
    }

    /**
     * @param {GPURenderPipeline} pipeline
     */
    setPipeline(pipeline) {
      if (this[_rid] === undefined) {
        throw new DOMException(
          "Failed to execute 'setPipeline' on 'GPUComputePassEncoder': already consumed",
          "OperationError",
        );
      }

      webidl.assertBranded(this, GPURenderBundleEncoder);
      const prefix =
        "Failed to execute 'setPipeline' on 'GPURenderBundleEncoder'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      pipeline = webidl.converters.GPURenderPipeline(pipeline, {
        prefix,
        context: "Argument 1",
      });
      core.jsonOpSync("op_webgpu_render_bundle_encoder_set_pipeline", {
        renderBundleEncoderRid: this[_rid],
        pipeline: pipeline[_rid],
      });
    }

    /**
     * @param {GPUBuffer} buffer
     * @param {GPUIndexFormat} indexFormat
     * @param {number} offset
     * @param {number} size
     */
    setIndexBuffer(buffer, indexFormat, offset = 0, size = 0) {
      if (this[_rid] === undefined) {
        throw new DOMException(
          "Failed to execute 'setIndexBuffer' on 'GPUComputePassEncoder': already consumed",
          "OperationError",
        );
      }

      webidl.assertBranded(this, GPURenderBundleEncoder);
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
      core.jsonOpSync("op_webgpu_render_bundle_encoder_set_index_buffer", {
        renderBundleEncoderRid: this[_rid],
        buffer: buffer[_rid],
        indexFormat,
        offset,
        size,
      });
    }

    /**
     * @param {number} slot
     * @param {GPUBuffer} buffer
     * @param {number} offset
     * @param {number} size
     */
    setVertexBuffer(slot, buffer, offset = 0, size = 0) {
      if (this[_rid] === undefined) {
        throw new DOMException(
          "Failed to execute 'setVertexBuffer' on 'GPUComputePassEncoder': already consumed",
          "OperationError",
        );
      }

      webidl.assertBranded(this, GPURenderBundleEncoder);
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
      core.jsonOpSync("op_webgpu_render_bundle_encoder_set_vertex_buffer", {
        renderBundleEncoderRid: this[_rid],
        slot,
        buffer: buffer[_rid],
        offset,
        size,
      });
    }

    /**
     * @param {number} vertexCount
     * @param {number} instanceCount
     * @param {number} firstVertex
     * @param {number} firstInstance
     */
    draw(vertexCount, instanceCount = 1, firstVertex = 0, firstInstance = 0) {
      if (this[_rid] === undefined) {
        throw new DOMException(
          "Failed to execute 'draw' on 'GPUComputePassEncoder': already consumed",
          "OperationError",
        );
      }

      webidl.assertBranded(this, GPURenderBundleEncoder);
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
      core.jsonOpSync("op_webgpu_render_bundle_encoder_draw", {
        renderBundleEncoderRid: this[_rid],
        vertexCount,
        instanceCount,
        firstVertex,
        firstInstance,
      });
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
      if (this[_rid] === undefined) {
        throw new DOMException(
          "Failed to execute 'drawIndexed' on 'GPUComputePassEncoder': already consumed",
          "OperationError",
        );
      }

      webidl.assertBranded(this, GPURenderBundleEncoder);
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
      core.jsonOpSync("op_webgpu_render_bundle_encoder_draw_indexed", {
        renderBundleEncoderRid: this[_rid],
        indexCount,
        instanceCount,
        firstIndex,
        baseVertex,
        firstInstance,
      });
    }

    /**
     * @param {GPUBuffer} indirectBuffer
     * @param {number} indirectOffset
     */
    drawIndirect(indirectBuffer, indirectOffset) {
      if (this[_rid] === undefined) {
        throw new DOMException(
          "Failed to execute 'drawIndirect' on 'GPUComputePassEncoder': already consumed",
          "OperationError",
        );
      }

      webidl.assertBranded(this, GPURenderBundleEncoder);
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
      core.jsonOpSync("op_webgpu_render_bundle_encoder_draw_indirect", {
        renderBundleEncoderRid: this[_rid],
        indirectBuffer: indirectBuffer[_rid],
        indirectOffset,
      });
    }

    drawIndexedIndirect(_indirectBuffer, _indirectOffset) {
      if (this[_rid] === undefined) {
        throw new DOMException(
          "Failed to execute 'drawIndexedIndirect' on 'GPUComputePassEncoder': already consumed",
          "OperationError",
        );
      }

      throw new Error("Not yet implemented");
    }

    [Symbol.for("Deno.customInspect")](inspect) {
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

    [Symbol.for("Deno.customInspect")](inspect) {
      return `${this.constructor.name} ${
        inspect({
          label: this.label,
        })
      }`;
    }
  }
  GPUObjectBaseMixin("GPURenderBundle", GPURenderBundle);

  const _descriptor = Symbol("[[descriptor]]");

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
      webidl.assertBranded(this, GPUQuerySet);
      this[_cleanup]();
    }

    [Symbol.for("Deno.customInspect")](inspect) {
      return `${this.constructor.name} ${
        inspect({
          label: this.label,
        })
      }`;
    }
  }
  GPUObjectBaseMixin("GPUQuerySet", GPUQuerySet);

  window.__bootstrap.webgpu = {
    gpu: webidl.createBranded(GPU),
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
  };
})(this);
