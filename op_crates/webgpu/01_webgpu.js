// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

((window) => {
  const core = window.Deno.core;

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

  const gpu = {
    async requestAdapter(options = {}) {
      const { error, ...data } = await core.jsonOpAsync(
        "op_webgpu_request_adapter",
        {
          ...options,
        },
      );

      if (error) {
        return null;
      } else {
        return new GPUAdapter(keySymbol, data);
      }
    },
  };

  class GPUAdapter {
    #rid;
    #name;
    get name() {
      return this.#name;
    }
    #features;
    get features() {
      return this.#features;
    }
    #limits;
    get limits() {
      return this.#limits;
    }

    constructor(key, data) {
      checkKey(key);

      this.#rid = data.rid;
      this.#name = data.name;
      this.#features = Object.freeze(data.features);
      this.#limits = Object.freeze(data.limits);
    }

    async requestDevice(descriptor = {}) {
      const { rid, ...data } = await core.jsonOpAsync(
        "op_webgpu_request_device",
        {
          adapterRid: this.#rid,
          ...descriptor,
        },
      );

      return new GPUDevice(keySymbol, this, rid, {
        label: descriptor.label,
        ...data,
      });
    }
  }

  // TODO(@crowlKats): https://gpuweb.github.io/gpuweb/#errors-and-debugging
  class GPUDevice extends EventTarget {
    #rid;
    #adapter;
    get adapter() {
      return this.#adapter;
    }
    #features;
    get features() {
      return this.#features;
    }
    #limits;
    get limits() {
      return this.#limits;
    }
    #queue;
    get queue() {
      return this.#queue;
    }

    constructor(key, adapter, rid, data) {
      checkKey(key);

      super();

      this.#adapter = adapter;
      this.#rid = rid;
      this.#features = Object.freeze(data.features);
      this.#limits = data.limits;
      this.#queue = new GPUQueue(keySymbol, rid, data.label);
      this.label = data.label;
    }

    destroy() {
      throw new Error("Not yet implemented");
    }

    createBuffer(descriptor) {
      const { rid } = core.jsonOpSync("op_webgpu_create_buffer", {
        deviceRid: this.#rid,
        ...descriptor,
      });

      return new GPUBuffer(
        keySymbol,
        rid,
        this.#rid,
        descriptor.label,
        descriptor.size,
        descriptor.mappedAtCreation,
      );
    }

    createTexture(descriptor) {
      const { rid } = core.jsonOpSync("op_webgpu_create_texture", {
        deviceRid: this.#rid,
        ...descriptor,
        size: normalizeGPUExtent3D(descriptor.size),
      });

      return new GPUTexture(keySymbol, rid, descriptor.label);
    }

    createSampler(descriptor = {}) {
      const { rid } = core.jsonOpSync("op_webgpu_create_sampler", {
        deviceRid: this.#rid,
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
        deviceRid: this.#rid,
        ...descriptor,
      });

      return new GPUBindGroupLayout(keySymbol, rid, descriptor.label);
    }

    createPipelineLayout(descriptor) {
      const { rid } = core.jsonOpSync("op_webgpu_create_pipeline_layout", {
        deviceRid: this.#rid,
        label: descriptor.label,
        bindGroupLayouts: descriptor.bindGroupLayouts.map((bindGroupLayout) =>
          bindGroupLayout[ridSymbol]
        ),
      });

      return new GPUPipelineLayout(keySymbol, rid, descriptor.label);
    }

    createBindGroup(descriptor) {
      const { rid } = core.jsonOpSync("op_webgpu_create_bind_group", {
        deviceRid: this.#rid,
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
          deviceRid: this.#rid,
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
        deviceRid: this.#rid,
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
        deviceRid: this.#rid,
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
        deviceRid: this.#rid,
        ...descriptor,
      });

      return new GPUCommandEncoder(keySymbol, rid, descriptor.label);
    }

    createRenderBundleEncoder(descriptor) {
      const { rid } = core.jsonOpSync(
        "op_webgpu_create_render_bundle_encoder",
        {
          deviceRid: this.#rid,
          ...descriptor,
        },
      );

      return new GPURenderBundleEncoder(keySymbol, rid, descriptor.label);
    }

    createQuerySet(descriptor) {
      const { rid } = core.jsonOpSync("op_webgpu_create_query_set", {
        deviceRid: this.#rid,
        ...descriptor,
      });

      return new GPUQuerySet(keySymbol, rid, descriptor.label);
    }
  }

  class GPUQueue {
    #rid;
    constructor(key, rid, label) {
      checkKey(key);

      this.#rid = rid;
      this.label = label ?? null;
    }

    submit(commandBuffers) {
      core.jsonOpSync("op_webgpu_queue_submit", {
        queueRid: this.#rid,
        commandBuffers: commandBuffers.map((buffer) => buffer[ridSymbol]),
      });
    }

    async onSubmittedWorkDone() {
    }

    writeBuffer(buffer, bufferOffset, data, dataOffset = 0, size) {
      core.jsonOpSync(
        "op_webgpu_write_buffer",
        {
          queueRid: this.#rid,
          buffer: buffer[ridSymbol],
          bufferOffset,
          dataOffset,
          size,
        },
        data,
      );
    }

    writeTexture(destination, data, dataLayout, size) {
      core.jsonOpSync(
        "op_webgpu_write_texture",
        {
          queueRid: this.#rid,
          destination: {
            texture: destination.texture[ridSymbol],
            mipLevel: destination.mipLevel,
            origin: destination.origin ??
              normalizeGPUOrigin3D(destination.origin),
          },
          dataLayout,
          size: normalizeGPUExtent3D(size),
        },
        data,
      );
    }

    copyImageBitmapToTexture(_source, _destination, _copySize) {
      throw new Error("Not yet implemented");
    }
  }

  class GPUBuffer {
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

      return new GPUComputePassEncoder(
        keySymbol,
        this.#rid,
        rid,
        descriptor.label,
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

      return new GPUCommandBuffer(keySymbol, rid, descriptor.label);
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

  class GPUComputePassEncoder {
    #commandEncoderRid;
    #rid;

    constructor(key, commandEncoderRid, rid, label) {
      checkKey(key);

      this.#commandEncoderRid = commandEncoderRid;
      this.#rid = rid;
      this.label = label ?? null;
    }

    setPipeline(pipeline) {
      core.jsonOpSync("op_webgpu_compute_pass_set_pipeline", {
        computePassRid: this.#rid,
        pipeline: pipeline[ridSymbol],
      });
    }
    dispatch(x, y = 1, z = 1) {
      core.jsonOpSync("op_webgpu_compute_pass_dispatch", {
        computePassRid: this.#rid,
        x,
        y,
        z,
      });
    }
    dispatchIndirect(indirectBuffer, indirectOffset) {
      core.jsonOpSync("op_webgpu_compute_pass_dispatch_indirect", {
        computePassRid: this.#rid,
        indirectBuffer: indirectBuffer[ridSymbol],
        indirectOffset,
      });
    }

    beginPipelineStatisticsQuery(querySet, queryIndex) {
      core.jsonOpSync(
        "op_webgpu_compute_pass_begin_pipeline_statistics_query",
        {
          computePassRid: this.#rid,
          querySet: querySet[ridSymbol],
          queryIndex,
        },
      );
    }
    endPipelineStatisticsQuery() {
      core.jsonOpSync("op_webgpu_compute_pass_end_pipeline_statistics_query", {
        computePassRid: this.#rid,
      });
    }

    writeTimestamp(querySet, queryIndex) {
      core.jsonOpSync("op_webgpu_compute_pass_write_timestamp", {
        computePassRid: this.#rid,
        querySet: querySet[ridSymbol],
        queryIndex,
      });
    }

    endPass() {
      core.jsonOpSync("op_webgpu_compute_pass_end_pass", {
        commandEncoderRid: this.#commandEncoderRid,
        computePassRid: this.#rid,
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
          "op_webgpu_compute_pass_set_bind_group",
          {
            computePassRid: this.#rid,
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
          computePassRid: this.#rid,
          index,
          bindGroup: bind,
          dynamicOffsetsData,
          dynamicOffsetsDataStart: 0,
          dynamicOffsetsDataLength: dynamicOffsetsData.length,
        });
      }
    }

    pushDebugGroup(groupLabel) {
      core.jsonOpSync("op_webgpu_compute_pass_push_debug_group", {
        computePassRid: this.#rid,
        groupLabel,
      });
    }
    popDebugGroup() {
      core.jsonOpSync("op_webgpu_compute_pass_pop_debug_group", {
        computePassRid: this.#rid,
      });
    }
    insertDebugMarker(markerLabel) {
      core.jsonOpSync("op_webgpu_compute_pass_insert_debug_marker", {
        computePassRid: this.#rid,
        markerLabel,
      });
    }
  }

  class GPUCommandBuffer {
    constructor(key, rid, label) {
      checkKey(key);

      this[ridSymbol] = rid;
      this.label = label ?? null;
    }

    get executionTime() {
      throw new Error("Not yet implemented");
    }
  }

  class GPURenderBundleEncoder {
    #rid;
    constructor(key, rid, label) {
      checkKey(key);

      this.#rid = rid;
      this.label = label ?? null;
    }

    finish(descriptor = {}) {
      const { rid } = core.jsonOpSync(
        "op_webgpu_render_bundle_encoder_finish",
        {
          renderBundleEncoderRid: this.#rid,
          ...descriptor,
        },
      );

      return new GPURenderBundle(keySymbol, rid, descriptor.label);
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
          "op_webgpu_render_bundle_encoder_set_bind_group",
          {
            renderBundleEncoderRid: this.#rid,
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
          renderBundleEncoderRid: this.#rid,
          index,
          bindGroup: bind,
          dynamicOffsetsData,
          dynamicOffsetsDataStart: 0,
          dynamicOffsetsDataLength: dynamicOffsetsData.length,
        });
      }
    }

    pushDebugGroup(groupLabel) {
      core.jsonOpSync("op_webgpu_render_bundle_encoder_push_debug_group", {
        renderBundleEncoderRid: this.#rid,
        groupLabel,
      });
    }
    popDebugGroup() {
      core.jsonOpSync("op_webgpu_render_bundle_encoder_pop_debug_group", {
        renderBundleEncoderRid: this.#rid,
      });
    }
    insertDebugMarker(markerLabel) {
      core.jsonOpSync("op_webgpu_render_bundle_encoder_push_debug_group", {
        renderBundleEncoderRid: this.#rid,
        markerLabel,
      });
    }

    setPipeline(pipeline) {
      core.jsonOpSync("op_webgpu_render_bundle_encoder_set_pipeline", {
        renderBundleEncoderRid: this.#rid,
        pipeline: pipeline[ridSymbol],
      });
    }

    setIndexBuffer(buffer, indexFormat, offset = 0, size = 0) {
      core.jsonOpSync("op_webgpu_render_bundle_encoder_set_index_buffer", {
        renderBundleEncoderRid: this.#rid,
        buffer: buffer[ridSymbol],
        indexFormat,
        offset,
        size,
      });
    }
    setVertexBuffer(slot, buffer, offset = 0, size = 0) {
      core.jsonOpSync("op_webgpu_render_bundle_encoder_set_vertex_buffer", {
        renderBundleEncoderRid: this.#rid,
        slot,
        buffer: buffer[ridSymbol],
        offset,
        size,
      });
    }

    draw(vertexCount, instanceCount = 1, firstVertex = 0, firstInstance = 0) {
      core.jsonOpSync("op_webgpu_render_bundle_encoder_draw", {
        renderBundleEncoderRid: this.#rid,
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
      core.jsonOpSync("op_webgpu_render_bundle_encoder_draw_indexed", {
        renderBundleEncoderRid: this.#rid,
        indexCount,
        instanceCount,
        firstIndex,
        baseVertex,
        firstInstance,
      });
    }

    drawIndirect(indirectBuffer, indirectOffset) {
      core.jsonOpSync("op_webgpu_render_bundle_encoder_draw_indirect", {
        renderBundleEncoderRid: this.#rid,
        indirectBuffer: indirectBuffer[ridSymbol],
        indirectOffset,
      });
    }
    drawIndexedIndirect(_indirectBuffer, _indirectOffset) {
      throw new Error("Not yet implemented");
    }
  }

  class GPURenderBundle {
    constructor(key, rid, label) {
      checkKey(key);

      this[ridSymbol] = rid;
      this.label = label ?? null;
    }
  }

  class GPUQuerySet {
    constructor(key, rid, label) {
      checkKey(key);

      this[ridSymbol] = rid;
      this.label = label ?? null;
    }

    destroy() {
      throw new Error("Not yet implemented");
    }
  }

  window.__bootstrap.webGPU = {
    gpu,
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
