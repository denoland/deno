// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

((window) => {
  const core = window.Deno.core;

  let instanceRid; // TODO: use op_webgpu_create_instance

  const GPU = {
    async requestAdapter(options = {}) {
      const { rid, ...data } = await core.jsonOpAsync(
        "op_webgpu_request_adapter",
        {
          instanceRid,
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

    constructor(rid, data) {
      this.#rid = rid;
      this.#name = data.name;
      this.#extensions = Object.freeze(data.features);
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

      return new GPUDevice(this, data);
    }
  }

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
    #defaultQueue;
    get defaultQueue() {
      return this.#defaultQueue;
    }

    constructor(adapter, rid, data) {
      super();

      this.#adapter = adapter;
      this.#rid = rid;
      this.#features = Object.freeze(data.features);
      this.#limits = data.limits;
      this.#defaultQueue = new GPUQueue(); // TODO
    }

    createBuffer(descriptor) {
      const { rid } = core.jsonOpSync("op_webgpu_create_buffer", {
        instanceRid,
        deviceRid: this.#rid,
        ...descriptor,
      });

      return new GPUBuffer(rid, descriptor.label); // TODO
    }

    createTexture(descriptor) {
      const { rid } = core.jsonOpSync("op_webgpu_create_texture", {
        instanceRid,
        deviceRid: this.#rid,
        ...descriptor,
      });

      return new GPUTexture(rid, descriptor.label); // TODO
    }

    createSampler(descriptor = {}) {
      const { rid } = core.jsonOpSync("op_webgpu_create_sampler", {
        instanceRid,
        deviceRid: this.#rid,
        ...descriptor,
      });

      const sampler = new GPUSampler(descriptor.label);
      GPUSamplerMap.set(sampler, rid);
      return sampler;
    }

    createBindGroupLayout(descriptor) {
      const { rid } = core.jsonOpSync("op_webgpu_create_bind_group_layout", {
        instanceRid,
        deviceRid: this.#rid,
        ...descriptor,
      });

      const bindGroupLayout = new GPUBindGroupLayout(descriptor.label);
      GPUBindGroupLayoutMap.set(bindGroupLayout, rid);
      return bindGroupLayout;
    }

    createPipelineLayout(descriptor) {
      const { rid } = core.jsonOpSync("op_webgpu_create_pipeline_layout", {
        instanceRid,
        deviceRid: this.#rid,
        label: descriptor.label,
        bindGroupLayouts: descriptor.bindGroupLayouts.map((bindGroupLayout) => {
          return GPUBindGroupLayoutMap.get(bindGroupLayout);
        }),
      });

      const pipelineLayout = new GPUPipelineLayout(descriptor.label);
      GPUPipelineLayoutMap.set(pipelineLayout, rid);
      return pipelineLayout;
    }

    createBindGroup(descriptor) {
      const { rid } = core.jsonOpSync("op_webgpu_create_bind_group", {
        instanceRid,
        deviceRid: this.#rid,
        label: descriptor.label,
        layout: GPUBindGroupLayoutMap.get(descriptor.layout),
        entries: descriptor.entries.map((entry) => {
          if (entry instanceof GPUSampler) {
            return {
              kind: "GPUSampler",
              resource: GPUSamplerMap.get(entry),
            };
          } else if (entry instanceof GPUTextureView) {
            return {
              kind: "GPUTextureView",
              resource: GPUTextureViewMap.get(entry),
            };
          } else {
            // TODO: buffer
          }
        }),
      });

      const bindGroup = new GPUBindGroup(descriptor.label);
      GPUBindGroupMap.set(bindGroup, rid);
      return bindGroup;
    }

    createShaderModule(descriptor) {
      const { rid } = core.jsonOpSync("op_webgpu_create_shader_module", {
        instanceRid,
        deviceRid: this.#rid,
        ...descriptor,
      });

      const shaderModule = new GPUShaderModule(rid, descriptor.label);
      GPUShaderModuleMap.set(shaderModule, rid);
      return shaderModule;
    }

    createComputePipeline(descriptor) {
      const { rid } = core.jsonOpSync("op_webgpu_create_compute_pipeline", {
        instanceRid,
        deviceRid: this.#rid,
        label: descriptor.label,
        layout: descriptor.layout &&
          GPUPipelineLayoutMap.get(descriptor.layout),
        computeStage: {
          module: GPUShaderModuleMap.get(descriptor.computeStage.module),
          entryPoint: descriptor.computeStage.entryPoint,
        },
      });

      const pipeline = new GPUComputePipeline(rid, descriptor.label);
      GPUComputePipelineMap.set(pipeline, rid);
      return pipeline;
    }

    createRenderPipeline(descriptor) {
      const { rid } = core.jsonOpSync("op_webgpu_create_render_pipeline", {
        instanceRid,
        deviceRid: this.#rid,
        ...descriptor,
      });

      const pipeline = new GPURenderPipeline(rid, descriptor.label);
      GPURenderPipelineMap.set(pipeline, rid);
      return pipeline;
    }

    async createReadyComputePipeline(descriptor) {
      throw new Error("Not yet implemented"); // easy polyfill
    }

    async createReadyRenderPipeline(descriptor) {
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
          instanceRid,
          deviceRid: this.#rid,
          ...descriptor,
        },
      );

      return new GPURenderBundleEncoder(rid, descriptor.label);
    }

    createQuerySet(descriptor) {
      throw new Error("Not yet implemented"); // wgpu#721
    }
  }

  class GPUQueue {
    constructor(label) {
      this.label = label ?? null;
    }

    submit(commandBuffers) {} // TODO
    createFence(descriptor = {}) {} // TODO

    writeBuffer(buffer, bufferOffset, data, dataOffset = 0, size) {}

    writeTexture(destination, data, dataLayout, size) {} // TODO

    copyImageBitmapToTexture(source, destination, copySize) {} // TODO
  }

  class GPUBuffer {
    #rid;

    constructor(rid, label) {
      this.#rid = rid;
      this.label = label ?? null;
    }

    async mapAsync(mode, offset = 0, size = undefined) {
      await core.jsonOpAsync("op_webgpu_buffer_get_map_async", {
        instanceRid,
        bufferRid: this.#rid,
        mode,
        offset,
        size,
      });
    }

    getMappedRange(offset = 0, size = undefined) {
      core.jsonOpSync("op_webgpu_buffer_get_mapped_range", {
        instanceRid,
        bufferRid: this.#rid,
        offset,
        size,
      });
    }

    unmap() {
      core.jsonOpSync("op_webgpu_buffer_unmap", {
        instanceRid,
        bufferRid: this.#rid,
      });
    }

    destroy() {
      throw new Error("Not yet implemented"); // master
    }
  }

  class GPUTexture {
    #rid;
    constructor(rid, label) {
      this.#rid = rid;
      this.label = label ?? null;
    }

    createView(descriptor = {}) {
      const { rid } = core.jsonOpSync("op_webgpu_create_texture_view", {
        instanceRid,
        textureRid: this.#rid,
        ...descriptor,
      });

      const view = new GPUTextureView();
      GPUTextureViewMap.set(view, rid);
      return view;
    }

    destroy() {
      throw new Error("Not yet implemented"); // master
    }
  }

  const GPUTextureViewMap = new WeakMap();
  class GPUTextureView {
    constructor(label) {
      this.label = label ?? null;
    }
  }

  const GPUSamplerMap = new WeakMap();
  class GPUSampler {
    constructor(label) {
      this.label = label ?? null;
    }
  }

  const GPUBindGroupLayoutMap = new WeakMap();
  class GPUBindGroupLayout {
    constructor(label) {
      this.label = label ?? null;
    }
  }

  const GPUPipelineLayoutMap = new WeakMap();
  class GPUPipelineLayout {
    constructor(label) {
      this.label = label ?? null;
    }
  }

  const GPUBindGroupMap = new WeakMap();
  class GPUBindGroup {
    constructor(label) {
      this.label = label ?? null;
    }
  }

  const GPUShaderModuleMap = new WeakMap();
  class GPUShaderModule {
    #rid;
    constructor(rid, label) {
      this.#rid = rid;
      this.label = label ?? null;
    }

    async compilationInfo() {
      throw new Error("Not yet implemented"); // wgpu#977
    }
  }

  const GPUComputePipelineMap = new WeakMap();
  class GPUComputePipeline {
    #rid;

    constructor(rid, label) {
      this.#rid = rid;
      this.label = label ?? null;
    }

    getBindGroupLayout(index) {
      const { rid } = core.jsonOpSync(
        "op_webgpu_compute_pipeline_get_bind_group_layout",
        {
          instanceRid,
          computePipelineRid: this.#rid,
          index,
        },
      );

      const bindGroupLayout = new GPUBindGroupLayout(); // TODO: label?
      GPUBindGroupLayoutMap.set(bindGroupLayout, rid);
      return bindGroupLayout;
    }
  }

  const GPURenderPipelineMap = new WeakMap();
  class GPURenderPipeline {
    #rid;

    constructor(rid, label) {
      this.#rid = rid;
      this.label = label ?? null;
    }

    getBindGroupLayout(index) {
      const { rid } = core.jsonOpSync(
        "op_webgpu_render_pipeline_get_bind_group_layout",
        {
          instanceRid,
          renderPipelineRid: this.#rid,
          index,
        },
      );

      const bindGroupLayout = new GPUBindGroupLayout(); // TODO: label?
      GPUBindGroupLayoutMap.set(bindGroupLayout, rid);
      return bindGroupLayout;
    }
  }

  class GPUCommandEncoder {
    #rid;

    constructor(rid, label) {
      this.#rid = rid;
      this.label = label ?? null;
    }

    beginRenderPass(descriptor) {
      const { rid } = core.jsonOpSync(
        "op_webgpu_command_encoder_begin_render_pass",
        {
          instanceRid,
          commandEncoderRid: this.#rid,
          ...descriptor,
        },
      );

      return new GPURenderPassEncoder(this.#rid, rid, descriptor.label);
    }

    beginComputePass(descriptor = {}) {
      const { rid } = core.jsonOpSync(
        "op_webgpu_command_encoder_begin_compute_pass",
        {
          instanceRid,
          commandEncoderRid: this.#rid,
          ...descriptor,
        },
      );

      return new GPUComputePassEncoder(this.#rid, rid, descriptor.label);
    }

    copyBufferToBuffer(source, sourceOffset, destination, destinationOffset, size) {} // TODO: buffer

    copyBufferToTexture(source, destination, copySize) {} // TODO: buffer

    copyTextureToBuffer(source, destination, copySize) {} // TODO: buffer

    copyTextureToTexture(source, destination, copySize) {
      const { rid } = core.jsonOpSync(
        "op_webgpu_command_encoder_copy_texture_to_texture",
        {
          instanceRid,
          commandEncoderRid: this.#rid,
          source,
          destination,
          copySize,
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
      core.jsonOpSync("op_webgpu_command_encoder_push_debug_group", {
        instanceRid,
        commandEncoderRid: this.#rid,
        markerLabel,
      });
    }

    writeTimestamp(querySet, queryIndex) {
      throw new Error("Not yet implemented"); // wgpu#721
    }

    resolveQuerySet(
      querySet,
      firstQuery,
      queryCount,
      destination,
      destinationOffset,
    ) {
      throw new Error("Not yet implemented"); // wgpu#721
    }

    finish(descriptor = {}) {
      const { rid } = core.jsonOpSync("op_webgpu_command_encoder_finish", {
        instanceRid,
        commandEncoderRid: this.#rid,
        ...descriptor,
      });

      return new GPUCommandBuffer(descriptor.label); // TODO
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
        color,
      });
    }
    setStencilReference(reference) {
      core.jsonOpSync("op_webgpu_render_pass_set_stencil_reference", {
        renderPassRid: this.#rid,
        reference,
      });
    }

    beginOcclusionQuery(queryIndex) {
      throw new Error("Not yet implemented"); // wgpu#721
    }
    endOcclusionQuery() {
      throw new Error("Not yet implemented"); // wgpu#721
    }

    beginPipelineStatisticsQuery(querySet, queryIndex) {
      throw new Error("Not yet implemented"); // wgpu#721
    }
    endPipelineStatisticsQuery() {
      throw new Error("Not yet implemented"); // wgpu#721
    }

    writeTimestamp(querySet, queryIndex) {
      throw new Error("Not yet implemented"); // wgpu#721
    }

    executeBundles(bundles) {
      core.jsonOpSync("op_webgpu_render_pass_execute_bundles", {
        renderPassRid: this.#rid,
        bundles,
      });
    }
    endPass() {
      core.jsonOpSync("op_webgpu_render_pass_end_pass", {
        instanceRid,
        commandEncoderRid: this.#commandEncoderRid,
        renderPassRid: this.#rid,
      });
    }

    setBindGroup(index, bindGroup, dynamicOffsets = []) {} // TODO

    setBindGroup(
      index,
      bindGroup,
      dynamicOffsetsData,
      dynamicOffsetsDataStart,
      dynamicOffsetsDataLength,
    ) {} // TODO

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
        pipeline: GPURenderPipelineMap.get(pipeline),
      });
    }

    setIndexBuffer(buffer, indexFormat, offset = 0, size = 0) {} // TODO: buffer
    setVertexBuffer(slot, buffer, offset = 0, size = 0) {} // TODO: buffer

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

    drawIndirect(indirectBuffer, indirectOffset) {} // TODO: buffer
    drawIndexedIndirect(indirectBuffer, indirectOffset) {} // TODO: buffer
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
        pipeline: GPUComputePipelineMap.get(pipeline),
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
    dispatchIndirect(indirectBuffer, indirectOffset) {} // TODO: buffer

    beginPipelineStatisticsQuery(querySet, queryIndex) {
      throw new Error("Not yet implemented"); // wgpu#721
    }
    endPipelineStatisticsQuery() {
      throw new Error("Not yet implemented"); // wgpu#721
    }

    writeTimestamp(querySet, queryIndex) {
      throw new Error("Not yet implemented"); // wgpu#721
    }

    endPass() {
      core.jsonOpSync("op_webgpu_compute_pass_end_pass", {
        instanceRid,
        commandEncoderRid: this.#commandEncoderRid,
        computePassRid: this.#rid,
      });
    }

    setBindGroup(index, bindGroup, dynamicOffsets = []) {} // TODO

    setBindGroup(
      index,
      bindGroup,
      dynamicOffsetsData,
      dynamicOffsetsDataStart,
      dynamicOffsetsDataLength,
    ) {} // TODO

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
    constructor(label) {
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

      return new GPURenderBundle(descriptor.label);
    }

    setBindGroup(index, bindGroup, dynamicOffsets = []) {} // TODO

    setBindGroup(
      index,
      bindGroup,
      dynamicOffsetsData,
      dynamicOffsetsDataStart,
      dynamicOffsetsDataLength,
    ) {} // TODO

    pushDebugGroup(groupLabel) {} // TODO
    popDebugGroup() {} // TODO
    insertDebugMarker(markerLabel) {} // TODO

    setPipeline(pipeline) {} // TODO

    setIndexBuffer(buffer, indexFormat, offset = 0, size = 0) {} // TODO
    setVertexBuffer(slot, buffer, offset = 0, size = 0) {} // TODO

    draw(vertexCount, instanceCount = 1, firstVertex = 0, firstInstance = 0) {} // TODO
    drawIndexed(
      indexCount,
      instanceCount = 1,
      firstIndex = 0,
      baseVertex = 0,
      firstInstance = 0,
    ) {} // TODO

    drawIndirect(indirectBuffer, indirectOffset) {} // TODO
    drawIndexedIndirect(indirectBuffer, indirectOffset) {} // TODO
  }

  class GPURenderBundle {
    constructor(label) {
      this.label = label ?? null;
    }
  }

  window.__bootstrap.webGPU = {
    webGPU: GPU,
  };
})(this);
