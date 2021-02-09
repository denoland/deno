// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

((window) => {
  const core = window.Deno.core;

  const ridSymbol = Symbol("rid");

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

  let instanceRid;

  function getInstanceRid() {
    if (!instanceRid) {
      const { rid } = core.jsonOpSync("op_webgpu_create_instance");
      instanceRid = rid;
    }
    return instanceRid;
  }

  const gpu = {
    async requestAdapter(options = {}) {
      const { rid, ...data } = await core.jsonOpAsync(
        "op_webgpu_request_adapter",
        {
          instanceRid: getInstanceRid(),
          ...options,
        },
      );
      return new GPUAdapter(rid, data);
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

    constructor(rid, data) {
      this.#rid = rid;
      this.#name = data.name;
      this.#features = Object.freeze(data.features);
      this.#limits = Object.freeze(data.limits);
    }

    async requestDevice(descriptor = {}) {
      const { rid, ...data } = await core.jsonOpAsync(
        "op_webgpu_request_device",
        {
          instanceRid,
          adapterRid: this.#rid,
          ...descriptor,
        },
      );

      return new GPUDevice(this, rid, {
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

    constructor(adapter, rid, data) {
      super();

      this.#adapter = adapter;
      this.#rid = rid;
      this.#features = Object.freeze(data.features);
      this.#limits = data.limits;
      this.#queue = new GPUQueue(rid, data.label);
      this.label = data.label;
    }

    destroy() {
      throw new Error("Not yet implemented");
    }

    createBuffer(descriptor) {
      const { rid } = core.jsonOpSync("op_webgpu_create_buffer", {
        instanceRid,
        deviceRid: this.#rid,
        ...descriptor,
      });

      return new GPUBuffer(
        rid,
        this.#rid,
        descriptor.label,
        descriptor.size,
        descriptor.mappedAtCreation,
      );
    }

    createTexture(descriptor) {
      const { rid } = core.jsonOpSync("op_webgpu_create_texture", {
        instanceRid,
        deviceRid: this.#rid,
        ...descriptor,
        size: normalizeGPUExtent3D(descriptor.size),
      });

      return new GPUTexture(rid, descriptor.label);
    }

    createSampler(descriptor = {}) {
      const { rid } = core.jsonOpSync("op_webgpu_create_sampler", {
        instanceRid,
        deviceRid: this.#rid,
        ...descriptor,
      });

      return new GPUSampler(rid, descriptor.label);
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
        instanceRid,
        deviceRid: this.#rid,
        ...descriptor,
      });

      return new GPUBindGroupLayout(rid, descriptor.label);
    }

    createPipelineLayout(descriptor) {
      const { rid } = core.jsonOpSync("op_webgpu_create_pipeline_layout", {
        instanceRid,
        deviceRid: this.#rid,
        label: descriptor.label,
        bindGroupLayouts: descriptor.bindGroupLayouts.map((bindGroupLayout) =>
          bindGroupLayout[ridSymbol]
        ),
      });

      return new GPUPipelineLayout(rid, descriptor.label);
    }

    createBindGroup(descriptor) {
      const { rid } = core.jsonOpSync("op_webgpu_create_bind_group", {
        instanceRid,
        deviceRid: this.#rid,
        label: descriptor.label,
        layout: descriptor.layout[ridSymbol],
        entries: descriptor.entries.map((entry) => {
          if (entry instanceof GPUSampler) {
            return {
              binding: entry.binding,
              kind: "GPUSampler",
              resource: entry.resource[ridSymbol],
            };
          } else if (entry instanceof GPUTextureView) {
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

      return new GPUBindGroup(rid, descriptor.label);
    }

    createShaderModule(descriptor) {
      const { rid } = core.jsonOpSync(
        "op_webgpu_create_shader_module",
        {
          instanceRid,
          deviceRid: this.#rid,
          label: descriptor.label,
          code: (typeof descriptor.code === "string")
            ? descriptor.code
            : undefined,
          sourceMap: descriptor.sourceMap,
        },
        ...(descriptor.code instanceof Uint32Array ? [descriptor.code] : [])
      );

      return new GPUShaderModule(rid, descriptor.label);
    }

    createComputePipeline(descriptor) {
      const { rid } = core.jsonOpSync("op_webgpu_create_compute_pipeline", {
        instanceRid,
        deviceRid: this.#rid,
        label: descriptor.label,
        layout: descriptor.layout ? descriptor.layout[ridSymbol] : undefined,
        compute: {
          module: descriptor.compute.module[ridSymbol],
          entryPoint: descriptor.compute.entryPoint,
        },
      });

      return new GPUComputePipeline(rid, descriptor.label);
    }

    createRenderPipeline(descriptor) {
      const { rid } = core.jsonOpSync("op_webgpu_create_render_pipeline", {
        instanceRid,
        deviceRid: this.#rid,
        ...descriptor,
      });

      return new GPURenderPipeline(rid, descriptor.label);
    }

    createComputePipelineAsync(_descriptor) {
      throw new Error("Not yet implemented"); // easy polyfill
    }

    createRenderPipelineAsync(_descriptor) {
      throw new Error("Not yet implemented"); // easy polyfill
    }

    createCommandEncoder(descriptor = {}) {
      const { rid } = core.jsonOpSync("op_webgpu_create_command_encoder", {
        instanceRid,
        deviceRid: this.#rid,
        ...descriptor,
      });

      return new GPUCommandEncoder(rid, descriptor.label);
    }

    createRenderBundleEncoder(descriptor) {
      const { rid } = core.jsonOpSync(
        "op_webgpu_create_render_bundle_encoder",
        {
          deviceRid: this.#rid,
          ...descriptor,
        },
      );

      return new GPURenderBundleEncoder(rid, descriptor.label);
    }

    createQuerySet(descriptor) {
      const { rid } = core.jsonOpSync("op_webgpu_create_query_set", {
        instanceRid,
        deviceRid: this.#rid,
        ...descriptor,
      });

      return new GPUQuerySet(rid, descriptor.label);
    }
  }

  class GPUQueue {
    #rid;
    constructor(rid, label) {
      this.#rid = rid;
      this.label = label ?? null;
    }

    submit(commandBuffers) {
      core.jsonOpSync("op_webgpu_queue_submit", {
        instanceRid,
        queueRid: this.#rid,
        commandBuffers: commandBuffers.map((buffer) => buffer[ridSymbol]),
      });
    }

    async onSubmittedWorkDone() {
    }

    writeBuffer(buffer, bufferOffset, data, dataOffset = 0, size) {
      core.jsonOpSync(
        "op_webgpu_write_texture",
        {
          instanceRid,
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
          instanceRid,
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

    constructor(rid, deviceRid, label, size, mappedAtCreation) {
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
        instanceRid,
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
          instanceRid,
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
        instanceRid,
        bufferRid: this[ridSymbol],
        mappedRid: this.#mappedRid,
      }, this.#mappedBuffer);
    }

    destroy() {
      throw new Error("Not yet implemented");
    }
  }

  class GPUTexture {
    constructor(rid, label) {
      this[ridSymbol] = rid;
      this.label = label ?? null;
    }

    createView(descriptor = {}) {
      const { rid } = core.jsonOpSync("op_webgpu_create_texture_view", {
        instanceRid,
        textureRid: this[ridSymbol],
        ...descriptor,
      });

      return new GPUTextureView(rid, descriptor.label);
    }

    destroy() {
      throw new Error("Not yet implemented");
    }
  }

  class GPUTextureView {
    constructor(rid, label) {
      this[ridSymbol] = rid;
      this.label = label ?? null;
    }
  }

  class GPUSampler {
    constructor(rid, label) {
      this[ridSymbol] = rid;
      this.label = label ?? null;
    }
  }

  class GPUBindGroupLayout {
    constructor(rid, label) {
      this[ridSymbol] = rid;
      this.label = label ?? null;
    }
  }

  class GPUPipelineLayout {
    constructor(rid, label) {
      this[ridSymbol] = rid;
      this.label = label ?? null;
    }
  }

  class GPUBindGroup {
    constructor(rid, label) {
      this[ridSymbol] = rid;
      this.label = label ?? null;
    }
  }

  class GPUShaderModule {
    constructor(rid, label) {
      this[ridSymbol] = rid;
      this.label = label ?? null;
    }

    compilationInfo() {
      throw new Error("Not yet implemented");
    }
  }

  class GPUComputePipeline {
    constructor(rid, label) {
      this[ridSymbol] = rid;
      this.label = label ?? null;
    }

    getBindGroupLayout(index) {
      const { rid, label } = core.jsonOpSync(
        "op_webgpu_compute_pipeline_get_bind_group_layout",
        {
          instanceRid,
          computePipelineRid: this[ridSymbol],
          index,
        },
      );

      return new GPUBindGroupLayout(rid, label);
    }
  }

  class GPURenderPipeline {
    constructor(rid, label) {
      this[ridSymbol] = rid;
      this.label = label ?? null;
    }

    getBindGroupLayout(index) {
      const { rid, label } = core.jsonOpSync(
        "op_webgpu_render_pipeline_get_bind_group_layout",
        {
          instanceRid,
          renderPipelineRid: this[ridSymbol],
          index,
        },
      );

      return new GPUBindGroupLayout(rid, label);
    }
  }

  class GPUCommandEncoder {
    #rid;

    constructor(rid, label) {
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

      return new GPURenderPassEncoder(this.#rid, rid, descriptor.label);
    }

    beginComputePass(descriptor = {}) {
      const { rid } = core.jsonOpSync(
        "op_webgpu_command_encoder_begin_compute_pass",
        {
          commandEncoderRid: this.#rid,
          ...descriptor,
        },
      );

      return new GPUComputePassEncoder(this.#rid, rid, descriptor.label);
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
          instanceRid,
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
          instanceRid,
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
          instanceRid,
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
          instanceRid,
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
        instanceRid,
        commandEncoderRid: this.#rid,
        groupLabel,
      });
    }
    popDebugGroup() {
      core.jsonOpSync("op_webgpu_command_encoder_pop_debug_group", {
        instanceRid,
        commandEncoderRid: this.#rid,
      });
    }
    insertDebugMarker(markerLabel) {
      core.jsonOpSync("op_webgpu_command_encoder_insert_debug_marker", {
        instanceRid,
        commandEncoderRid: this.#rid,
        markerLabel,
      });
    }

    writeTimestamp(querySet, queryIndex) {
      core.jsonOpSync("op_webgpu_command_encoder_write_timestamp", {
        instanceRid,
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
        instanceRid,
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
        instanceRid,
        commandEncoderRid: this.#rid,
        ...descriptor,
      });

      return new GPUCommandBuffer(rid, descriptor.label);
    }
  }

  class GPURenderPassEncoder {
    #commandEncoderRid;
    #rid;

    constructor(commandEncoderRid, rid, label) {
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
        instanceRid,
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

    constructor(commandEncoderRid, rid, label) {
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
        instanceRid,
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
    constructor(rid, label) {
      this[ridSymbol] = rid;
      this.label = label ?? null;
    }

    get executionTime() {
      throw new Error("Not yet implemented");
    }
  }

  class GPURenderBundleEncoder {
    #rid;
    constructor(rid, label) {
      this.#rid = rid;
      this.label = label ?? null;
    }

    finish(descriptor = {}) {
      const { rid } = core.jsonOpSync(
        "op_webgpu_render_bundle_encoder_finish",
        {
          instanceRid,
          renderBundleEncoderRid: this.#rid,
          ...descriptor,
        },
      );

      return new GPURenderBundle(rid, descriptor.label);
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
    constructor(rid, label) {
      this[ridSymbol] = rid;
      this.label = label ?? null;
    }
  }

  class GPUQuerySet {
    constructor(rid, label) {
      this[ridSymbol] = rid;
      this.label = label ?? null;
    }

    destroy() {
      throw new Error("Not yet implemented");
    }
  }

  window.__bootstrap.webGPU = {
    gpu,
  };
})(this);
