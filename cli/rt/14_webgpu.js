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
    }

    constructor(adapter, data) {
      super();

      this.#adapter = adapter;
      this.#deviceRid = data.deviceRid;
    }

    createBuffer(descriptor) {
      const { rid } = core.jsonOpSync("op_webgpu_create_buffer", {
        rid: this.#deviceRid,
        ...descriptor,
      });

      return new GPUBuffer(rid);
    }

    createTexture(descriptor) {
      const { rid } = core.jsonOpSync("op_webgpu_create_texture", {
        rid: this.#deviceRid,
        ...descriptor,
      });

      new GPUTexture(rid);
    }
  }

  class GPUBuffer {
    #rid;

    constructor(rid) {
      this.#rid = rid;
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

    destroy() {
    } // TODO
  }

  class GPUTexture {
    #rid;
    constructor(rid) {
      this.#rid = rid;
    }

    createView(descriptor = {}) {
      const { rid } = core.jsonOpSync("op_webgpu_create_texture_view", {
        rid: this.#deviceRid,
        ...descriptor,
      });

      new GPUTextureView(rid);
    }

    destroy() {} // TODO
  }

  class GPUTextureView {
    #rid;
    constructor(rid) {
      this.#rid = rid;
    }
  }

  window.__bootstrap.webGPU = {
    webGPU: GPU,
  };
})(this);
