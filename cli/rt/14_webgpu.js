// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

((window) => {
  const core = window.Deno.core;

  const GPU = {
    async requestAdapter(options = {}) {
      const { rid, name, extensions } = await core.jsonOpAsync(
        "op_webgpu_request_adapter",
        options,
      );
      return new GPUAdapter(rid, name, extensions);
    },
  };

  class GPUAdapter {
    #rid;
    #name;
    get name() {
      return this.#name;
    }
    #extensions;
    get extensions() {
      return this.#extensions;
    }
    // TODO: limits

    constructor(rid, name, extensions) {
      this.#rid = rid;
      this.#name = name;
      this.#extensions = Object.freeze(extensions);
    }

    async requestDevice(descriptor = {}) {
      const data = await core.jsonOpAsync("op_webgpu_request_device", {
        rid: this.#rid,
        ...descriptor,
      });

      return new GPUDevice(this, data);
    }
  }

  class GPUDevice extends EventTarget {
    #deviceRid;
    #adapter;
    get adapter() {
      return this.#adapter;
    }
    #extensions;
    get extensions() {
      return this.#extensions;
    }
    #limits;
    get limits() {
      return this.#limits;
    }
    #defaultQueue;
    get defaultQueue() {
      return this.#defaultQueue;
    } // TODO

    // TODO: should have label

    constructor(adapter, data) {
      super();

      this.#adapter = adapter;
      this.#deviceRid = data.deviceRid; // TODO: properties
    }

    createBuffer(descriptor) {
      const { rid } = core.jsonOpSync("op_webgpu_create_buffer", {
        rid: this.#deviceRid,
        ...descriptor,
      });

      return new GPUBuffer(rid, descriptor.label); // TODO
    }

    createTexture(descriptor) {
      const { rid } = core.jsonOpSync("op_webgpu_create_texture", {
        rid: this.#deviceRid,
        ...descriptor,
      });

      return new GPUTexture(rid, descriptor.label); // TODO
    }

    createSampler(descriptor = {}) {
      const { rid } = core.jsonOpSync("op_webgpu_create_sampler", {
        rid: this.#deviceRid,
        ...descriptor,
      });

      const sampler = new GPUSampler(descriptor.label);
      GPUSamplerMap.set(sampler, rid);
      return sampler;
    }

    createBindGroupLayout(descriptor) {
      const { rid } = core.jsonOpSync("op_webgpu_create_bind_group_layout", {
        rid: this.#deviceRid,
        ...descriptor,
      });

      const bindGroupLayout = new GPUBindGroupLayout(descriptor.label);
      GPUBindGroupLayoutMap.set(bindGroupLayout, rid);
      return bindGroupLayout;
    }

    createPipelineLayout(descriptor) {
      const { rid } = core.jsonOpSync("op_webgpu_create_pipeline_layout", {
        rid: this.#deviceRid,
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
        rid: this.#deviceRid,
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
            // TODO
          }
        }),
      });

      const bindGroup = new GPUBindGroup(descriptor.label);
      GPUBindGroupMap.set(bindGroup, rid);
      return bindGroup;
    }

    createShaderModule(descriptor) {
      const { rid } = core.jsonOpSync("op_webgpu_create_shader_module", {
        rid: this.#deviceRid,
        ...descriptor,
      });

      const shaderModule = new GPUShaderModule(rid, descriptor.label);
      GPUShaderModuleMap.set(shaderModule, rid);
      return shaderModule;
    }

    createComputePipeline(descriptor) {
      const { rid } = core.jsonOpSync("op_webgpu_create_compute_pipeline", {
        rid: this.#deviceRid,
        label: descriptor.label,
        layout: descriptor.layout &&
          GPUPipelineLayoutMap.get(descriptor.layout),
        computeStage: {
          module: GPUShaderModuleMap.get(descriptor.computeStage.module),
          entryPoint: descriptor.computeStage.entryPoint,
        },
      });

      return new GPUComputePipeline(rid, descriptor.label);
    }

    createRenderPipeline(descriptor) {
      const { rid } = core.jsonOpSync("op_webgpu_create_render_pipeline", {
        rid: this.#deviceRid,
        ...descriptor
      });

      return new GPURenderPipeline(rid, descriptor.label);
    }

    async createReadyComputePipeline(descriptor) {} // TODO

    async createReadyRenderPipeline(descriptor) {} // TODO

    createCommandEncoder(descriptor = {}) {
      const { rid } = core.jsonOpSync("op_webgpu_create_command_encoder", {
        rid: this.#deviceRid,
        ...descriptor
      });

      return new GPUCommandEncoder(rid, descriptor.label);
    }

    createRenderBundleEncoder(descriptor) {
      const { rid } = core.jsonOpSync("op_webgpu_create_render_bundle_encoder", {
        rid: this.#deviceRid,
        ...descriptor,
      });


      new GPURenderBundleEncoder();
    }
  }

  class GPUBuffer {
    #rid;

    constructor(rid, label) {
      this.#rid = rid;
      this.label = label;
    }

    async mapAsync(mode, offset = 0, size = undefined) {
      await core.jsonOpAsync("op_webgpu_buffer_get_map_async", {
        rid: this.#rid,
        offset,
        size,
      });
    }

    getMappedRange(offset = 0, size = undefined) {
      core.jsonOpSync("op_webgpu_buffer_get_mapped_range", {
        rid: this.#rid,
        offset,
        size,
      }); // TODO
    }

    unmap() {
      core.jsonOpSync("op_webgpu_buffer_unmap", {
        rid: this.#rid,
      });
    }

    destroy() {} // TODO
  }

  class GPUTexture {
    #rid;
    constructor(rid, label) {
      this.#rid = rid;
      this.label = label;
    }

    createView(descriptor = {}) {
      const { rid } = core.jsonOpSync("op_webgpu_create_texture_view", {
        rid: this.#rid,
        ...descriptor,
      });

      const view = new GPUTextureView();
      GPUTextureViewMap.set(view, rid);
      return view;
    }

    destroy() {} // TODO
  }

  const GPUTextureViewMap = new WeakMap();
  class GPUTextureView {
    constructor(label) {
      this.label = label;
    }
  }

  const GPUSamplerMap = new WeakMap();
  class GPUSampler {
    constructor(label) {
      this.label = label;
    }
  }

  const GPUBindGroupLayoutMap = new WeakMap();
  class GPUBindGroupLayout {
    constructor(label) {
      this.label = label;
    }
  }

  const GPUPipelineLayoutMap = new WeakMap();
  class GPUPipelineLayout {
    constructor(label) {
      this.label = label;
    }
  }

  const GPUBindGroupMap = new WeakMap();
  class GPUBindGroup {
    constructor(label) {
      this.label = label;
    }
  }

  const GPUShaderModuleMap = new WeakMap();
  class GPUShaderModule {
    #rid;
    constructor(rid, label) {
      this.#rid = rid;
      this.label = label;
    }

    async compilationInfo() {} // TODO
  }

  class GPUComputePipeline {
    #rid;

    constructor(rid, label) {
      this.#rid = rid;
      this.label = label;
    }

    getBindGroupLayout(index) {
      const { rid } = core.jsonOpSync(
        "op_webgpu_compute_pipeline_get_bind_group_layout",
        {
          rid: this.#rid,
          index,
        },
      );

      const bindGroupLayout = new GPUBindGroupLayout(); // TODO
      GPUBindGroupLayoutMap.set(bindGroupLayout, rid);
      return bindGroupLayout;
    }
  }

  class GPURenderPipeline {
    #rid;

    constructor(rid, label) {
      this.#rid = rid;
      this.label = label;
    }

    getBindGroupLayout(index) {
      const { rid } = core.jsonOpSync(
        "op_webgpu_render_pipeline_get_bind_group_layout",
        {
          rid: this.#rid,
          index,
        },
      );

      const bindGroupLayout = new GPUBindGroupLayout(); // TODO
      GPUBindGroupLayoutMap.set(bindGroupLayout, rid);
      return bindGroupLayout;
    }
  }

  class GPUCommandEncoder {
    #rid;

    constructor(rid, label) {
      this.#rid = rid;
      this.label = label;
    }

    beginRenderPass(descriptor) {
      const { rid } = core.jsonOpSync("op_webgpu_command_encoder_begin_render_pass", {
        rid: this.#rid,
        ...descriptor,
      });

      return new GPURenderPassEncoder(rid, descriptor.label);
    }

    beginComputePass(descriptor = {}) {
      const { rid } = core.jsonOpSync("op_webgpu_command_encoder_begin_compute_pass", {
        rid: this.#rid,
        ...descriptor,
      });

      return new GPUComputePassEncoder(rid, descriptor.label);
    }

    copyBufferToBuffer(source, sourceOffset, destination, destinationOffset, size) {} // TODO

    copyBufferToTexture(source, destination, copySize) {} // TODO

    copyTextureToBuffer(source, destination, copySize) {} // TODO

    copyTextureToTexture(source, destination, copySize) {
      const { rid } = core.jsonOpSync("op_webgpu_command_encoder_copy_texture_to_texture", {
        rid: this.#rid,
        source,
        destination,
        copySize,
      });
    }

    pushDebugGroup(groupLabel) {} // TODO
    popDebugGroup() {} // TODO
    insertDebugMarker(markerLabel) {} // TODO

    writeTimestamp(querySet, queryIndex) {} // TODO

    resolveQuerySet(querySet, firstQuery, queryCount, destination, destinationOffset) {} // TODO

    finish(descriptor = {}) {
      const { rid } = core.jsonOpSync("op_webgpu_command_encoder_finish", {
        rid: this.#rid,
        ...descriptor,
      });

      return new GPUCommandBuffer(descriptor.label);
    }
  }

  class GPURenderPassEncoder {
    #rid;

    constructor(rid, label) {
      this.#rid = rid;
      this.label = label;
    }

    setViewport(x, y, width, height, minDepth, maxDepth) {} // TODO

    setScissorRect(x, y, width, height) {} // TODO

    setBlendColor(color) {} // TODO
    setStencilReference(reference) {} // TODO

    beginOcclusionQuery(queryIndex) {} // TODO
    endOcclusionQuery() {} // TODO

    beginPipelineStatisticsQuery(querySet, queryIndex) {} // TODO
    endPipelineStatisticsQuery() {} // TODO

    writeTimestamp(querySet, queryIndex) {} // TODO

    executeBundles(bundles) {} // TODO
    endPass() {} // TODO


    setBindGroup(index, bindGroup, dynamicOffsets = []) {} // TODO

    setBindGroup(index, bindGroup, dynamicOffsetsData, dynamicOffsetsDataStart, dynamicOffsetsDataLength) {} // TODO

    pushDebugGroup(groupLabel) {} // TODO
    popDebugGroup() {} // TODO
    insertDebugMarker(markerLabel) {} // TODO


    setPipeline(pipeline) {} // TODO

    setIndexBuffer(buffer, indexFormat, offset = 0, size = 0) {} // TODO
    setVertexBuffer(slot, buffer, offset = 0, size = 0) {} // TODO

    draw(vertexCount, instanceCount = 1, firstVertex = 0, firstInstance = 0) {} // TODO
    drawIndexed(indexCount, instanceCount = 1, firstIndex = 0, baseVertex = 0, firstInstance = 0) {} // TODO

    drawIndirect(indirectBuffer, indirectOffset) {} // TODO
    drawIndexedIndirect(indirectBuffer, indirectOffset) {} // TODO
  }

  class GPUComputePassEncoder {
    #rid;

    constructor(rid, label) {
      this.#rid = rid;
      this.label = label;
    }

    setPipeline(pipeline) {} // TODO
    dispatch(x, y = 1, z = 1) {} // TODO
    dispatchIndirect(indirectBuffer, indirectOffset) {} // TODO

    beginPipelineStatisticsQuery(querySet, queryIndex) {} // TODO
    endPipelineStatisticsQuery() {} // TODO

    writeTimestamp(querySet, queryIndex) {} // TODO

    endPass() {} // TODO


    setBindGroup(index, bindGroup, dynamicOffsets = []) {} // TODO

    setBindGroup(index, bindGroup, dynamicOffsetsData, dynamicOffsetsDataStart, dynamicOffsetsDataLength) {} // TODO

    pushDebugGroup(groupLabel) {} // TODO
    popDebugGroup() {} // TODO
    insertDebugMarker(markerLabel) {} // TODO
  }

  class GPUCommandBuffer {
    constructor(label) {
      this.label = label;
    }

    async get executionTime() {} // TODO
  }

  class GPURenderBundleEncoder {
    #rid;
    constructor(rid, label) {
      this.#rid = rid;
      this.label = label;
    }

    finish(descriptor = {}) {
      const { rid } = core.jsonOpSync("op_webgpu_render_bundle_encoder_finish", {
        rid: this.#rid,
        ...descriptor,
      });

      return new GPURenderBundle(descriptor.label);
    }


    setBindGroup(index, bindGroup, dynamicOffsets = []) {} // TODO

    setBindGroup(index, bindGroup, dynamicOffsetsData, dynamicOffsetsDataStart, dynamicOffsetsDataLength) {} // TODO

    pushDebugGroup(groupLabel) {} // TODO
    popDebugGroup() {} // TODO
    insertDebugMarker(markerLabel) {} // TODO


    setPipeline(pipeline) {} // TODO

    setIndexBuffer(buffer, indexFormat, offset = 0, size = 0) {} // TODO
    setVertexBuffer(slot, buffer, offset = 0, size = 0) {} // TODO

    draw(vertexCount, instanceCount = 1, firstVertex = 0, firstInstance = 0) {} // TODO
    drawIndexed(indexCount, instanceCount = 1, firstIndex = 0, baseVertex = 0, firstInstance = 0) {} // TODO

    drawIndirect(indirectBuffer, indirectOffset) {} // TODO
    drawIndexedIndirect(indirectBuffer, indirectOffset) {} // TODO

  }

  class GPURenderBundle {
    constructor(label) {
      this.label = label;
    }
  }

  window.__bootstrap.webGPU = {
    webGPU: GPU,
  };
})(this);
