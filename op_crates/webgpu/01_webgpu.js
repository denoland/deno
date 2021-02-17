// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

// @ts-check
/// <reference path="../../core/lib.deno_core.d.ts" />
/// <reference path="../web/internal.d.ts" />
/// <reference path="./lib.deno_webgpu.d.ts" />

"use strict";

((window) => {
  const core = window.Deno.core;
  const webidl = window.__bootstrap.webidl;
  const eventTarget = window.__bootstrap.eventTarget;

  const ridSymbol = Symbol("rid");

  const keySymbol = Symbol("key");
  function checkKey(key) {
    if (key !== keySymbol) {
      throw new TypeError("Illegal constructor");
    }
  }

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
    adapter[_adapter] = inner;
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
      descriptor = webidl.converters.GPUDeviceDescriptor(descriptor, {
        prefix: "Failed to execute 'requestDevice' on 'GPUAdapter'",
        context: "Argument 1",
      });

      const { rid, features, limits } = await core.jsonOpAsync(
        "op_webgpu_request_device",
        {
          adapterRid: this[_adapter].rid,
          ...descriptor,
        },
      );

      return createGPUDevice(
        descriptor.label ?? null,
        {
          rid,
          adapter: this,
          features: Object.freeze(features),
          limits: Object.freeze(limits),
          queue: createGPUQueue(descriptor.label ?? null, rid),
        },
      );
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

  /**
   * @typedef InnerGPUDevice
   * @property {GPUAdapter} adapter
   * @property {number} rid
   * @property {GPUFeatureName[]} features
   * @property {object} limits
   * @property {GPUQueue} queue
   */

  /**
   * @param {string | null} label
   * @param {InnerGPUDevice} inner
   * @returns {GPUDevice}
   */
  function createGPUDevice(label, inner) {
    /** @type {GPUDevice} */
    const device = webidl.createBranded(GPUDevice);
    device[_label] = label;
    device[_device] = inner;
    return device;
  }

  webidl.converters["UVString?"] = webidl.createNullableConverter(
    webidl.converters.USVString,
  );

  // TODO(@crowlKats): https://gpuweb.github.io/gpuweb/#errors-and-debugging
  class GPUDevice extends eventTarget.EventTarget {
    /** @type {InnerGPUDevice} */
    [_device];

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
      return this[_device].queue;
    }

    constructor() {
      webidl.illegalConstructor();
      super();
    }

    destroy() {
      throw new Error("Not yet implemented");
    }

    /**
     * @param {GPUBufferDescriptor} descriptor 
     */
    createBuffer(descriptor) {
      webidl.assertBranded(this, GPUDevice);
      descriptor = webidl.converters.GPUBufferDescriptor(descriptor, {
        prefix: "Failed to execute 'createBuffer' on 'GPUDevice'",
        context: "Argument 1",
      });
      const { rid } = core.jsonOpSync("op_webgpu_create_buffer", {
        deviceRid: this[_device].rid,
        ...descriptor,
      });
      return new GPUBuffer(
        keySymbol,
        rid,
        this[_device].rid,
        descriptor.label,
        descriptor.size,
        descriptor.mappedAtCreation,
      );
    }

    createTexture(descriptor) {
      descriptor = webidl.converters.GPUTextureDescriptor(descriptor, {
        prefix: "Failed to execute 'createTexture' on 'GPUDevice'",
        context: "Argument 1",
      });
      const { rid } = core.jsonOpSync("op_webgpu_create_texture", {
        deviceRid: this[_device].rid,
        ...descriptor,
        size: normalizeGPUExtent3D(descriptor.size),
      });

      return new GPUTexture(keySymbol, rid, descriptor.label);
    }

    createSampler(descriptor = {}) {
      const { rid } = core.jsonOpSync("op_webgpu_create_sampler", {
        deviceRid: this[_device].rid,
        ...descriptor,
      });

      return new GPUSampler(keySymbol, rid, descriptor.label);
    }

    createBindGroupLayout(descriptor) {
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

      return new GPUBindGroupLayout(keySymbol, rid, descriptor.label);
    }

    createPipelineLayout(descriptor) {
      const { rid } = core.jsonOpSync("op_webgpu_create_pipeline_layout", {
        deviceRid: this[_device].rid,
        label: descriptor.label,
        bindGroupLayouts: descriptor.bindGroupLayouts.map((bindGroupLayout) =>
          bindGroupLayout[ridSymbol]
        ),
      });

      return new GPUPipelineLayout(keySymbol, rid, descriptor.label);
    }

    createBindGroup(descriptor) {
      const { rid } = core.jsonOpSync("op_webgpu_create_bind_group", {
        deviceRid: this[_device].rid,
        label: descriptor.label,
        layout: descriptor.layout[ridSymbol],
        entries: descriptor.entries.map((entry) => {
          if (entry.resource instanceof GPUSampler) {
            return {
              binding: entry.binding,
              kind: "GPUSampler",
              resource: entry.resource[ridSymbol],
            };
          } else if (entry.resource instanceof GPUTextureView) {
            return {
              binding: entry.binding,
              kind: "GPUTextureView",
              resource: entry.resource[ridSymbol],
            };
          } else {
            return {
              binding: entry.binding,
              kind: "GPUBufferBinding",
              resource: entry.resource.buffer[ridSymbol],
              offset: entry.resource.offset,
              size: entry.resource.size,
            };
          }
        }),
      });

      return new GPUBindGroup(keySymbol, rid, descriptor.label);
    }

    createShaderModule(descriptor) {
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
        ...(descriptor.code instanceof Uint32Array ? [descriptor.code] : []),
      );

      return new GPUShaderModule(keySymbol, rid, descriptor.label);
    }

    createComputePipeline(descriptor) {
      const { rid } = core.jsonOpSync("op_webgpu_create_compute_pipeline", {
        deviceRid: this[_device].rid,
        label: descriptor.label,
        layout: descriptor.layout ? descriptor.layout[ridSymbol] : undefined,
        compute: {
          module: descriptor.compute.module[ridSymbol],
          entryPoint: descriptor.compute.entryPoint,
        },
      });

      return new GPUComputePipeline(keySymbol, rid, descriptor.label);
    }

    createRenderPipeline(descriptor) {
      const d = {
        label: descriptor.label,
        layout: descriptor.layout?.[ridSymbol],
        vertex: {
          module: descriptor.vertex.module[ridSymbol],
          entryPoint: descriptor.vertex.entryPoint,
          buffers: descriptor.vertex.buffers,
        },
        primitive: descriptor.primitive,
        depthStencil: descriptor.depthStencil,
        multisample: descriptor.multisample,
        fragment: descriptor.fragment
          ? {
            module: descriptor.fragment.module[ridSymbol],
            entryPoint: descriptor.fragment.entryPoint,
            targets: descriptor.fragment.targets,
          }
          : undefined,
      };

      const { rid } = core.jsonOpSync("op_webgpu_create_render_pipeline", {
        deviceRid: this[_device].rid,
        ...d,
      });

      return new GPURenderPipeline(keySymbol, rid, descriptor.label);
    }

    createComputePipelineAsync(_descriptor) {
      throw new Error("Not yet implemented"); // easy polyfill
    }

    createRenderPipelineAsync(_descriptor) {
      throw new Error("Not yet implemented"); // easy polyfill
    }

    createCommandEncoder(descriptor = {}) {
      const { rid } = core.jsonOpSync("op_webgpu_create_command_encoder", {
        deviceRid: this[_device].rid,
        ...descriptor,
      });

      return new GPUCommandEncoder(keySymbol, rid, descriptor.label);
    }

    createRenderBundleEncoder(descriptor) {
      const { rid } = core.jsonOpSync(
        "op_webgpu_create_render_bundle_encoder",
        {
          deviceRid: this[_device].rid,
          ...descriptor,
        },
      );

      return createGPURenderBundleEncoder(descriptor.label ?? null, rid);
    }

    createQuerySet(descriptor) {
      const { rid } = core.jsonOpSync("op_webgpu_create_query_set", {
        deviceRid: this[_device].rid,
        ...descriptor,
      });

      return createGPUQuerySet(descriptor.label ?? null, rid);
    }
  }
  GPUObjectBaseMixin("GPUDevice", GPUDevice);

  /**
   * @param {string | null} label
   * @param {number} rid
   * @returns {GPUQueue}
   */
  function createGPUQueue(label, rid) {
    /** @type {GPUQueue} */
    const queue = webidl.createBranded(GPUQueue);
    queue[_label] = label;
    queue[_device] = rid;
    return queue;
  }

  class GPUQueue {
    /**
     * The rid of the related device. 
     * @type {number}
     */
    [_device];

    constructor() {
      webidl.illegalConstructor();
    }

    /**
     * @param {GPUCommandBuffer[]} commandBuffers 
     */
    submit(commandBuffers) {
      webidl.assertBranded(this, GPUQueue);
      webidl.requiredArguments(arguments.length, 1, {
        prefix: "Failed to execute 'submit' on 'GPUQueue'",
      });
      // TODO(lucacasonato): should be real converter
      commandBuffers = webidl.converters.any(commandBuffers);
      core.jsonOpSync("op_webgpu_queue_submit", {
        queueRid: this[_device],
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
          queueRid: this[_device],
          buffer: buffer[ridSymbol],
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
          queueRid: this[_device],
          destination: {
            texture: destination.texture[ridSymbol],
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
  }
  GPUObjectBaseMixin("GPUQueue", GPUQueue);

  const _rid = Symbol("[[rid]]");

  class GPUBuffer {
    [webidl.brand] = webidl.brand;

    #deviceRid;
    #size;
    #mappedSize;
    #mappedOffset;
    #mappedRid;
    #mappedBuffer;

    constructor(key, rid, deviceRid, label, size, mappedAtCreation) {
      checkKey(key);

      this[ridSymbol] = rid;
      this.#deviceRid = deviceRid;
      this.label = label ?? null;
      this.#size = size;

      if (mappedAtCreation) {
        this.#mappedSize = size;
        this.#mappedOffset = 0;
      }
    }

    async mapAsync(mode, offset = 0, size = undefined) {
      this.#mappedOffset = offset;
      this.#mappedSize = size ?? (this.#size - offset);
      await core.jsonOpAsync("op_webgpu_buffer_get_map_async", {
        bufferRid: this[ridSymbol],
        deviceRid: this.#deviceRid,
        mode,
        offset,
        size: this.#mappedSize,
      });
    }

    getMappedRange(offset = 0, size = undefined) {
      const buffer = new Uint8Array(size ?? this.#mappedSize);
      const { rid } = core.jsonOpSync(
        "op_webgpu_buffer_get_mapped_range",
        {
          bufferRid: this[ridSymbol],
          offset,
          size: size ?? this.#mappedSize,
        },
        buffer,
      );

      this.#mappedRid = rid;
      this.#mappedBuffer = buffer;
      return this.#mappedBuffer.buffer;
    }

    unmap() {
      core.jsonOpSync("op_webgpu_buffer_unmap", {
        bufferRid: this[ridSymbol],
        mappedRid: this.#mappedRid,
      }, this.#mappedBuffer);
    }

    destroy() {
      throw new Error("Not yet implemented");
    }
  }
  class GPUTexture {
    constructor(key, rid, label) {
      checkKey(key);

      this[ridSymbol] = rid;
      this.label = label ?? null;
    }

    createView(descriptor = {}) {
      const { rid } = core.jsonOpSync("op_webgpu_create_texture_view", {
        textureRid: this[ridSymbol],
        ...descriptor,
      });

      return new GPUTextureView(keySymbol, rid, descriptor.label);
    }

    destroy() {
      throw new Error("Not yet implemented");
    }
  }

  class GPUTextureView {
    constructor(key, rid, label) {
      checkKey(key);

      this[ridSymbol] = rid;
      this.label = label ?? null;
    }
  }

  class GPUSampler {
    constructor(key, rid, label) {
      checkKey(key);

      this[ridSymbol] = rid;
      this.label = label ?? null;
    }
  }

  class GPUBindGroupLayout {
    constructor(key, rid, label) {
      checkKey(key);

      this[ridSymbol] = rid;
      this.label = label ?? null;
    }
  }

  class GPUPipelineLayout {
    constructor(key, rid, label) {
      checkKey(key);

      this[ridSymbol] = rid;
      this.label = label ?? null;
    }
  }

  class GPUBindGroup {
    constructor(key, rid, label) {
      checkKey(key);

      this[ridSymbol] = rid;
      this.label = label ?? null;
    }
  }

  class GPUShaderModule {
    constructor(key, rid, label) {
      checkKey(key);

      this[ridSymbol] = rid;
      this.label = label ?? null;
    }

    compilationInfo() {
      throw new Error("Not yet implemented");
    }
  }

  class GPUComputePipeline {
    [webidl.brand] = webidl.brand;

    constructor(key, rid, label) {
      checkKey(key);

      this[ridSymbol] = rid;
      this.label = label ?? null;
    }

    getBindGroupLayout(index) {
      const { rid, label } = core.jsonOpSync(
        "op_webgpu_compute_pipeline_get_bind_group_layout",
        {
          computePipelineRid: this[ridSymbol],
          index,
        },
      );

      return new GPUBindGroupLayout(keySymbol, rid, label);
    }
  }

  class GPURenderPipeline {
    [webidl.brand] = webidl.brand;

    constructor(key, rid, label) {
      checkKey(key);

      this[ridSymbol] = rid;
      this.label = label ?? null;
    }

    getBindGroupLayout(index) {
      const { rid, label } = core.jsonOpSync(
        "op_webgpu_render_pipeline_get_bind_group_layout",
        {
          renderPipelineRid: this[ridSymbol],
          index,
        },
      );

      return new GPUBindGroupLayout(keySymbol, rid, label);
    }
  }

  class GPUCommandEncoder {
    #rid;

    constructor(key, rid, label) {
      checkKey(key);

      this.#rid = rid;
      this.label = label ?? null;
    }

    beginRenderPass(descriptor) {
      let depthStencilAttachment;
      if (descriptor.depthStencilAttachment) {
        depthStencilAttachment = {
          ...descriptor.depthStencilAttachment,
          view: descriptor.depthStencilAttachment.view[ridSymbol],
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
          commandEncoderRid: this.#rid,
          ...descriptor,
          colorAttachments: descriptor.colorAttachments.map(
            (colorAttachment) => {
              const attachment = {
                view: colorAttachment.view[ridSymbol],
                resolveTarget: colorAttachment.resolveTarget
                  ? colorAttachment.resolveTarget[ridSymbol]
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

      return new GPURenderPassEncoder(
        keySymbol,
        this.#rid,
        rid,
        descriptor.label,
      );
    }

    beginComputePass(descriptor = {}) {
      const { rid } = core.jsonOpSync(
        "op_webgpu_command_encoder_begin_compute_pass",
        {
          commandEncoderRid: this.#rid,
          ...descriptor,
        },
      );

      return createGPUComputePassEncoder(
        descriptor.label ?? null,
        this.#rid,
        rid,
      );
    }

    copyBufferToBuffer(
      source,
      sourceOffset,
      destination,
      destinationOffset,
      size,
    ) {
      core.jsonOpSync(
        "op_webgpu_command_encoder_copy_buffer_to_buffer",
        {
          commandEncoderRid: this.#rid,
          source: source[ridSymbol],
          sourceOffset,
          destination: destination[ridSymbol],
          destinationOffset,
          size,
        },
      );
    }

    copyBufferToTexture(source, destination, copySize) {
      core.jsonOpSync(
        "op_webgpu_command_encoder_copy_buffer_to_texture",
        {
          commandEncoderRid: this.#rid,
          source: {
            ...source,
            buffer: source.buffer[ridSymbol],
          },
          destination: {
            texture: destination.texture[ridSymbol],
            mipLevel: destination.mipLevel,
            origin: destination.origin ??
              normalizeGPUOrigin3D(destination.origin),
          },
          copySize: normalizeGPUExtent3D(copySize),
        },
      );
    }

    copyTextureToBuffer(source, destination, copySize) {
      core.jsonOpSync(
        "op_webgpu_command_encoder_copy_texture_to_buffer",
        {
          commandEncoderRid: this.#rid,
          source: {
            texture: source.texture[ridSymbol],
            mipLevel: source.mipLevel,
            origin: source.origin ?? normalizeGPUOrigin3D(source.origin),
          },
          destination: {
            ...destination,
            buffer: destination.buffer[ridSymbol],
          },
          copySize: normalizeGPUExtent3D(copySize),
        },
      );
    }

    copyTextureToTexture(source, destination, copySize) {
      core.jsonOpSync(
        "op_webgpu_command_encoder_copy_texture_to_texture",
        {
          commandEncoderRid: this.#rid,
          source: {
            texture: source.texture[ridSymbol],
            mipLevel: source.mipLevel,
            origin: source.origin ?? normalizeGPUOrigin3D(source.origin),
          },
          destination: {
            texture: destination.texture[ridSymbol],
            mipLevel: destination.mipLevel,
            origin: destination.origin ??
              normalizeGPUOrigin3D(destination.origin),
          },
          copySize: normalizeGPUExtent3D(copySize),
        },
      );
    }

    pushDebugGroup(groupLabel) {
      core.jsonOpSync("op_webgpu_command_encoder_push_debug_group", {
        commandEncoderRid: this.#rid,
        groupLabel,
      });
    }
    popDebugGroup() {
      core.jsonOpSync("op_webgpu_command_encoder_pop_debug_group", {
        commandEncoderRid: this.#rid,
      });
    }
    insertDebugMarker(markerLabel) {
      core.jsonOpSync("op_webgpu_command_encoder_insert_debug_marker", {
        commandEncoderRid: this.#rid,
        markerLabel,
      });
    }

    writeTimestamp(querySet, queryIndex) {
      core.jsonOpSync("op_webgpu_command_encoder_write_timestamp", {
        commandEncoderRid: this.#rid,
        querySet: querySet[ridSymbol],
        queryIndex,
      });
    }

    resolveQuerySet(
      querySet,
      firstQuery,
      queryCount,
      destination,
      destinationOffset,
    ) {
      core.jsonOpSync("op_webgpu_command_encoder_resolve_query_set", {
        commandEncoderRid: this.#rid,
        querySet: querySet[ridSymbol],
        firstQuery,
        queryCount,
        destination: destination[ridSymbol],
        destinationOffset,
      });
    }

    finish(descriptor = {}) {
      const { rid } = core.jsonOpSync("op_webgpu_command_encoder_finish", {
        commandEncoderRid: this.#rid,
        ...descriptor,
      });

      return createGPUCommandBuffer(descriptor.label ?? null, rid);
    }
  }

  class GPURenderPassEncoder {
    #commandEncoderRid;
    #rid;

    constructor(key, commandEncoderRid, rid, label) {
      checkKey(key);

      this.#commandEncoderRid = commandEncoderRid;
      this.#rid = rid;
      this.label = label ?? null;
    }

    setViewport(x, y, width, height, minDepth, maxDepth) {
      core.jsonOpSync("op_webgpu_render_pass_set_viewport", {
        renderPassRid: this.#rid,
        x,
        y,
        width,
        height,
        minDepth,
        maxDepth,
      });
    }

    setScissorRect(x, y, width, height) {
      core.jsonOpSync("op_webgpu_render_pass_set_scissor_rect", {
        renderPassRid: this.#rid,
        x,
        y,
        width,
        height,
      });
    }

    setBlendColor(color) {
      core.jsonOpSync("op_webgpu_render_pass_set_blend_color", {
        renderPassRid: this.#rid,
        color: normalizeGPUColor(color),
      });
    }
    setStencilReference(reference) {
      core.jsonOpSync("op_webgpu_render_pass_set_stencil_reference", {
        renderPassRid: this.#rid,
        reference,
      });
    }

    beginOcclusionQuery(_queryIndex) {
      throw new Error("Not yet implemented");
    }
    endOcclusionQuery() {
      throw new Error("Not yet implemented");
    }

    beginPipelineStatisticsQuery(querySet, queryIndex) {
      core.jsonOpSync("op_webgpu_render_pass_begin_pipeline_statistics_query", {
        renderPassRid: this.#rid,
        querySet: querySet[ridSymbol],
        queryIndex,
      });
    }
    endPipelineStatisticsQuery() {
      core.jsonOpSync("op_webgpu_render_pass_end_pipeline_statistics_query", {
        renderPassRid: this.#rid,
      });
    }

    writeTimestamp(querySet, queryIndex) {
      core.jsonOpSync("op_webgpu_render_pass_write_timestamp", {
        renderPassRid: this.#rid,
        querySet: querySet[ridSymbol],
        queryIndex,
      });
    }

    executeBundles(bundles) {
      core.jsonOpSync("op_webgpu_render_pass_execute_bundles", {
        renderPassRid: this.#rid,
        bundles: bundles.map((bundle) => bundle[ridSymbol]),
      });
    }
    endPass() {
      core.jsonOpSync("op_webgpu_render_pass_end_pass", {
        commandEncoderRid: this.#commandEncoderRid,
        renderPassRid: this.#rid,
      });
    }

    setBindGroup(
      index,
      bindGroup,
      dynamicOffsetsData,
      dynamicOffsetsDataStart,
      dynamicOffsetsDataLength,
    ) {
      const bind = bindGroup[ridSymbol];
      if (dynamicOffsetsData instanceof Uint32Array) {
        core.jsonOpSync(
          "op_webgpu_render_pass_set_bind_group",
          {
            renderPassRid: this.#rid,
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
          renderPassRid: this.#rid,
          index,
          bindGroup: bind,
          dynamicOffsetsData,
          dynamicOffsetsDataStart: 0,
          dynamicOffsetsDataLength: dynamicOffsetsData.length,
        });
      }
    }

    pushDebugGroup(groupLabel) {
      core.jsonOpSync("op_webgpu_render_pass_push_debug_group", {
        renderPassRid: this.#rid,
        groupLabel,
      });
    }
    popDebugGroup() {
      core.jsonOpSync("op_webgpu_render_pass_pop_debug_group", {
        renderPassRid: this.#rid,
      });
    }
    insertDebugMarker(markerLabel) {
      core.jsonOpSync("op_webgpu_render_pass_insert_debug_marker", {
        renderPassRid: this.#rid,
        markerLabel,
      });
    }

    setPipeline(pipeline) {
      core.jsonOpSync("op_webgpu_render_pass_set_pipeline", {
        renderPassRid: this.#rid,
        pipeline: pipeline[ridSymbol],
      });
    }

    setIndexBuffer(buffer, indexFormat, offset = 0, size = 0) {
      core.jsonOpSync("op_webgpu_render_pass_set_index_buffer", {
        renderPassRid: this.#rid,
        buffer: buffer[ridSymbol],
        indexFormat,
        offset,
        size,
      });
    }
    setVertexBuffer(slot, buffer, offset = 0, size = 0) {
      core.jsonOpSync("op_webgpu_render_pass_set_vertex_buffer", {
        renderPassRid: this.#rid,
        slot,
        buffer: buffer[ridSymbol],
        offset,
        size,
      });
    }

    draw(vertexCount, instanceCount = 1, firstVertex = 0, firstInstance = 0) {
      core.jsonOpSync("op_webgpu_render_pass_draw", {
        renderPassRid: this.#rid,
        vertexCount,
        instanceCount,
        firstVertex,
        firstInstance,
      });
    }
    drawIndexed(
      indexCount,
      instanceCount = 1,
      firstIndex = 0,
      baseVertex = 0,
      firstInstance = 0,
    ) {
      core.jsonOpSync("op_webgpu_render_pass_draw_indexed", {
        renderPassRid: this.#rid,
        indexCount,
        instanceCount,
        firstIndex,
        baseVertex,
        firstInstance,
      });
    }

    drawIndirect(indirectBuffer, indirectOffset) {
      core.jsonOpSync("op_webgpu_render_pass_draw_indirect", {
        renderPassRid: this.#rid,
        indirectBuffer: indirectBuffer[ridSymbol],
        indirectOffset,
      });
    }
    drawIndexedIndirect(indirectBuffer, indirectOffset) {
      core.jsonOpSync("op_webgpu_render_pass_draw_indexed_indirect", {
        renderPassRid: this.#rid,
        indirectBuffer: indirectBuffer[ridSymbol],
        indirectOffset,
      });
    }
  }

  const _encoder = Symbol("[[encoder]]");

  /**
   * @param {string | null} label
   * @param {number} encoderRid
   * @param {number} rid
   * @returns {GPUComputePassEncoder}
   */
  function createGPUComputePassEncoder(label, encoderRid, rid) {
    /** @type {GPUComputePassEncoder} */
    const commandBuffer = webidl.createBranded(GPUComputePassEncoder);
    commandBuffer[_label] = label;
    commandBuffer[_encoder] = encoderRid;
    commandBuffer[_rid] = rid;
    return commandBuffer;
  }

  class GPUComputePassEncoder {
    /** @type {number} */
    [_encoder];

    /** @type {number} */
    [_rid];

    constructor() {
      webidl.illegalConstructor();
    }

    /**
     * @param {GPUComputePipeline} pipeline 
     */
    setPipeline(pipeline) {
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
        pipeline: pipeline[ridSymbol],
      });
    }

    /**
     * @param {number} x 
     * @param {number} y 
     * @param {number} z 
     */
    dispatch(x, y = 1, z = 1) {
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
        indirectBuffer: indirectBuffer[ridSymbol],
        indirectOffset,
      });
    }

    /**
     * @param {GPUQuerySet} querySet 
     * @param {number} queryIndex 
     */
    beginPipelineStatisticsQuery(querySet, queryIndex) {
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
      core.jsonOpSync("op_webgpu_compute_pass_write_timestamp", {
        computePassRid: this[_rid],
        querySet: querySet[_rid],
        queryIndex,
      });
    }

    endPass() {
      webidl.assertBranded(this, GPUComputePassEncoder);
      core.jsonOpSync("op_webgpu_compute_pass_end_pass", {
        commandEncoderRid: this[_encoder],
        computePassRid: this[_rid],
      });
    }

    // TODO(lucacasonato): has an overload
    setBindGroup(
      index,
      bindGroup,
      dynamicOffsetsData,
      dynamicOffsetsDataStart,
      dynamicOffsetsDataLength,
    ) {
      const bind = bindGroup[ridSymbol];
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
      webidl.assertBranded(this, GPUComputePassEncoder);
      const prefix =
        "Failed to execute 'pushDebugGroup' on 'GPUComputePassEncoder'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      groupLabel = webidl.converters.GPURenderBundleDescriptor(groupLabel, {
        prefix,
        context: "Argument 1",
      });
      core.jsonOpSync("op_webgpu_compute_pass_push_debug_group", {
        computePassRid: this[_rid],
        groupLabel,
      });
    }

    popDebugGroup() {
      core.jsonOpSync("op_webgpu_compute_pass_pop_debug_group", {
        computePassRid: this[_rid],
      });
    }

    /**
     * @param {string} markerLabel 
     */
    insertDebugMarker(markerLabel) {
      webidl.assertBranded(this, GPUComputePassEncoder);
      const prefix =
        "Failed to execute 'insertDebugMarker' on 'GPUComputePassEncoder'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      markerLabel = webidl.converters.GPURenderBundleDescriptor(markerLabel, {
        prefix,
        context: "Argument 1",
      });
      core.jsonOpSync("op_webgpu_compute_pass_insert_debug_marker", {
        computePassRid: this[_rid],
        markerLabel,
      });
    }
  }
  GPUObjectBaseMixin("GPUComputePassEncoder", GPUComputePassEncoder);

  /**
   * @param {string | null} label
   * @param {number} rid
   * @returns {GPUCommandBuffer}
   */
  function createGPUCommandBuffer(label, rid) {
    /** @type {GPUCommandBuffer} */
    const commandBuffer = webidl.createBranded(GPUCommandBuffer);
    commandBuffer[_label] = label;
    commandBuffer[_rid] = rid;
    return commandBuffer;
  }

  class GPUCommandBuffer {
    /** @type {number} */
    [_rid];

    constructor() {
      webidl.illegalConstructor();
    }

    get executionTime() {
      throw new Error("Not yet implemented");
    }
  }
  GPUObjectBaseMixin("GPUCommandBuffer", GPUCommandBuffer);

  /**
   * @param {string | null} label
   * @param {number} rid
   * @returns {GPURenderBundleEncoder}
   */
  function createGPURenderBundleEncoder(label, rid) {
    /** @type {GPURenderBundleEncoder} */
    const bundle = webidl.createBranded(GPURenderBundleEncoder);
    bundle[_label] = label;
    bundle[_rid] = rid;
    return bundle;
  }

  class GPURenderBundleEncoder {
    /** @type {number} */
    [_rid];

    constructor() {
      webidl.illegalConstructor();
    }

    /**
     * @param {GPURenderBundleDescriptor} descriptor 
     */
    finish(descriptor = {}) {
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

      return createGPURenderBundle(descriptor.label ?? null, rid);
    }

    // TODO(lucacasonato): has an overload
    setBindGroup(
      index,
      bindGroup,
      dynamicOffsetsData,
      dynamicOffsetsDataStart,
      dynamicOffsetsDataLength,
    ) {
      const bind = bindGroup[ridSymbol];
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
      webidl.assertBranded(this, GPURenderBundleEncoder);
      const prefix =
        "Failed to execute 'pushDebugGroup' on 'GPURenderBundleEncoder'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      groupLabel = webidl.converters.GPURenderBundleDescriptor(groupLabel, {
        prefix,
        context: "Argument 1",
      });
      core.jsonOpSync("op_webgpu_render_bundle_encoder_push_debug_group", {
        renderBundleEncoderRid: this[_rid],
        groupLabel,
      });
    }

    popDebugGroup() {
      webidl.assertBranded(this, GPURenderBundleEncoder);
      core.jsonOpSync("op_webgpu_render_bundle_encoder_pop_debug_group", {
        renderBundleEncoderRid: this[_rid],
      });
    }

    /**
     * @param {string} markerLabel 
     */
    insertDebugMarker(markerLabel) {
      webidl.assertBranded(this, GPURenderBundleEncoder);
      const prefix =
        "Failed to execute 'insertDebugMarker' on 'GPURenderBundleEncoder'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      markerLabel = webidl.converters.GPURenderBundleDescriptor(markerLabel, {
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
        pipeline: pipeline[ridSymbol],
      });
    }

    /**
     * @param {GPUBuffer} buffer 
     * @param {GPUIndexFormat} indexFormat 
     * @param {number} offset 
     * @param {number} size 
     */
    setIndexBuffer(buffer, indexFormat, offset = 0, size = 0) {
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
        buffer: buffer[ridSymbol],
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
        buffer: buffer[ridSymbol],
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
      webidl.assertBranded(this, GPURenderBundleEncoder);
      const prefix = "Failed to execute 'draw' on 'GPURenderBundleEncoder'";
      webidl.requiredArguments(arguments.length, 2, { prefix });
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
      webidl.assertBranded(this, GPURenderBundleEncoder);
      const prefix =
        "Failed to execute 'drawIndexed' on 'GPURenderBundleEncoder'";
      webidl.requiredArguments(arguments.length, 2, { prefix });
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
        indirectBuffer: indirectBuffer[ridSymbol],
        indirectOffset,
      });
    }

    drawIndexedIndirect(_indirectBuffer, _indirectOffset) {
      throw new Error("Not yet implemented");
    }
  }
  GPUObjectBaseMixin("GPURenderBundleEncoder", GPURenderBundleEncoder);

  /**
   * @param {string | null} label
   * @param {number} rid
   * @returns {GPURenderBundle}
   */
  function createGPURenderBundle(label, rid) {
    /** @type {GPURenderBundle} */
    const bundle = webidl.createBranded(GPURenderBundle);
    bundle[_label] = label;
    bundle[_rid] = rid;
    return bundle;
  }

  class GPURenderBundle {
    /** @type {number} */
    [_rid];

    constructor() {
      webidl.illegalConstructor();
    }
  }
  GPUObjectBaseMixin("GPURenderBundle", GPURenderBundle);

  /**
   * @param {string | null} label
   * @param {number} rid
   * @returns {GPUQuerySet}
   */
  function createGPUQuerySet(label, rid) {
    /** @type {GPUQuerySet} */
    const queue = webidl.createBranded(GPUQuerySet);
    queue[_label] = label;
    queue[_rid] = rid;
    return queue;
  }

  class GPUQuerySet {
    /** @type {number} */
    [_rid];

    constructor() {
      webidl.illegalConstructor();
    }

    destroy() {
      webidl.assertBranded(this, GPUQuerySet);
      throw new Error("Not yet implemented");
    }
  }
  GPUObjectBaseMixin("GPUQuerySet", GPUQuerySet);

  window.__bootstrap.webgpu = {
    gpu: webidl.createBranded(GPU),
    GPU,
    GPUAdapter,
    GPUDevice,
    GPUQueue,
    GPUBuffer,
    GPUTexture,
    GPUTextureView,
    GPUSampler,
    GPUBindGroupLayout,
    GPUPipelineLayout,
    GPUBindGroup,
    GPUShaderModule,
    GPUComputePipeline,
    GPURenderPipeline,
    GPUCommandEncoder,
    GPURenderPassEncoder,
    GPUComputePassEncoder,
    GPUCommandBuffer,
    GPURenderBundleEncoder,
    GPURenderBundle,
    GPUQuerySet,
  };
})(this);
