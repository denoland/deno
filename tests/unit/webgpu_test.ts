// Copyright 2018-2026 the Deno authors. MIT license.

import { assert, assertEquals, assertThrows } from "./test_util.ts";

let isCI: boolean;
try {
  isCI = (Deno.env.get("CI")?.length ?? 0) > 0;
} catch {
  isCI = true;
}

// Skip these tests on linux CI, because the vulkan emulator is not good enough
// yet, and skip on macOS x86 CI because these do not have virtual GPUs.
const isCIWithoutGPU = (Deno.build.os === "linux" ||
  (Deno.build.os === "darwin" && Deno.build.arch === "x86_64")) && isCI;
// Skip these tests in WSL because it doesn't have good GPU support.
const isWsl = await checkIsWsl();
// Skip the window surface tests when there is no windowing system available
// (e.g. headless or Wayland-only). Checked up front so those tests are
// reported as ignored rather than silently passing.
const hasWindowingSystem = checkWindowingSystem();

function checkWindowingSystem(): boolean {
  if (Deno.build.os === "windows") {
    // Window creation is asserted in the test itself.
    return true;
  }
  if (Deno.build.os !== "linux") {
    return false;
  }
  try {
    const x11 = Deno.dlopen(
      "libX11.so.6",
      {
        XOpenDisplay: { parameters: ["pointer"], result: "pointer" },
        XCloseDisplay: { parameters: ["pointer"], result: "i32" },
      } as const,
    );
    try {
      const display = x11.symbols.XOpenDisplay(null);
      if (display === null) {
        return false;
      }
      // Safe to close: this probe connection never backs a wgpu surface.
      x11.symbols.XCloseDisplay(display);
      return true;
    } finally {
      x11.close();
    }
  } catch {
    // libX11 is not present
    return false;
  }
}

Deno.test({
  permissions: { read: true, env: true },
  ignore: isWsl || isCIWithoutGPU,
}, async function webgpuComputePass() {
  const adapter = await navigator.gpu.requestAdapter();
  assert(adapter);

  const numbers = [1, 4, 3, 295];

  const device = await adapter.requestDevice();
  assert(device);

  const shaderCode = await Deno.readTextFile(
    "tests/testdata/webgpu/computepass_shader.wgsl",
  );

  const shaderModule = device.createShaderModule({
    code: shaderCode,
  });

  const size = new Float32Array(numbers).byteLength;

  const inputDataBuffer = device.createBuffer({
    label: "Input Data Buffer",
    size: size,
    usage: GPUBufferUsage.STORAGE,
    mappedAtCreation: true,
  });
  const buf = new Float32Array(inputDataBuffer.getMappedRange());
  buf.set(numbers);
  inputDataBuffer.unmap();

  const outputDataBuffer = device.createBuffer({
    label: "Output Data Buffer",
    size: size,
    usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_SRC,
  });

  const downloadBuffer = device.createBuffer({
    label: "Download Buffer",
    size: size,
    usage: GPUBufferUsage.COPY_DST | GPUBufferUsage.MAP_READ,
  });

  const bindGroupLayout = device.createBindGroupLayout({
    entries: [
      // input buffer
      {
        binding: 0,
        visibility: GPUShaderStage.COMPUTE,
        buffer: {
          type: "read-only-storage",
          minBindingSize: 4,
        },
      },
      // output buffer
      {
        binding: 1,
        visibility: GPUShaderStage.COMPUTE,
        buffer: {
          type: "storage",
          minBindingSize: 4,
        },
      },
    ],
  });
  const bindGroup = device.createBindGroup({
    layout: bindGroupLayout,
    entries: [
      {
        binding: 0,
        resource: {
          buffer: inputDataBuffer,
        },
      },
      {
        binding: 1,
        resource: {
          buffer: outputDataBuffer,
        },
      },
    ],
  });
  const pipelineLayout = device.createPipelineLayout({
    bindGroupLayouts: [bindGroupLayout],
  });
  const computePipeline = device.createComputePipeline({
    layout: pipelineLayout,
    compute: {
      module: shaderModule,
      entryPoint: "doubleMe",
    },
  });
  const encoder = device.createCommandEncoder();

  const computePass = encoder.beginComputePass();
  computePass.setPipeline(computePipeline);
  computePass.setBindGroup(0, bindGroup);
  computePass.dispatchWorkgroups(numbers.length);
  computePass.end();

  encoder.copyBufferToBuffer(outputDataBuffer, 0, downloadBuffer, 0, size);

  device.queue.submit([encoder.finish()]);

  await downloadBuffer.mapAsync(GPUMapMode.READ);

  const data = downloadBuffer.getMappedRange();

  assertEquals(new Float32Array(data), new Float32Array([2, 8, 6, 590]));

  downloadBuffer.unmap();

  device.destroy();
});

Deno.test({
  permissions: { read: true, env: true },
  ignore: isWsl || isCIWithoutGPU,
}, async function webgpuHelloTriangle() {
  const adapter = await navigator.gpu.requestAdapter();
  assert(adapter);

  const device = await adapter.requestDevice();
  assert(device);

  const shaderCode = await Deno.readTextFile(
    "tests/testdata/webgpu/hellotriangle_shader.wgsl",
  );

  const shaderModule = device.createShaderModule({
    code: shaderCode,
  });

  const pipelineLayout = device.createPipelineLayout({
    bindGroupLayouts: [],
  });

  const renderPipeline = device.createRenderPipeline({
    layout: pipelineLayout,
    vertex: {
      module: shaderModule,
      entryPoint: "vs_main",
      // only test purpose
      constants: {
        value: 0.5,
      },
    },
    fragment: {
      module: shaderModule,
      entryPoint: "fs_main",
      // only test purpose
      constants: {
        value: 0.5,
      },
      targets: [
        {
          format: "rgba8unorm-srgb",
        },
      ],
    },
  });

  const dimensions = {
    width: 200,
    height: 200,
  };
  const unpaddedBytesPerRow = dimensions.width * 4;
  const align = 256;
  const paddedBytesPerRowPadding = (align - unpaddedBytesPerRow % align) %
    align;
  const paddedBytesPerRow = unpaddedBytesPerRow + paddedBytesPerRowPadding;

  const outputBuffer = device.createBuffer({
    label: "Capture",
    size: paddedBytesPerRow * dimensions.height,
    usage: GPUBufferUsage.MAP_READ | GPUBufferUsage.COPY_DST,
  });
  const texture = device.createTexture({
    label: "Capture",
    size: dimensions,
    format: "rgba8unorm-srgb",
    usage: GPUTextureUsage.RENDER_ATTACHMENT | GPUTextureUsage.COPY_SRC,
  });

  const encoder = device.createCommandEncoder();
  const view = texture.createView();
  const renderPass = encoder.beginRenderPass({
    colorAttachments: [
      {
        view,
        storeOp: "store",
        loadOp: "clear",
        clearValue: [0, 1, 0, 1],
      },
    ],
  });
  renderPass.setPipeline(renderPipeline);
  renderPass.draw(3, 1);
  renderPass.end();

  encoder.copyTextureToBuffer(
    {
      texture,
    },
    {
      buffer: outputBuffer,
      bytesPerRow: paddedBytesPerRow,
      rowsPerImage: 0,
    },
    dimensions,
  );

  device.queue.submit([encoder.finish()]);

  await outputBuffer.mapAsync(1);
  const data = new Uint8Array(outputBuffer.getMappedRange());

  assertEquals(
    data,
    await Deno.readFile("tests/testdata/webgpu/hellotriangle.out"),
  );

  outputBuffer.unmap();

  device.destroy();
});

Deno.test({
  ignore: isWsl || isCIWithoutGPU,
}, async function webgpuAdapterHasFeatures() {
  const adapter = await navigator.gpu.requestAdapter();
  assert(adapter);
  assert(adapter.features);
  const device = await adapter.requestDevice();
  device.destroy();
});

Deno.test({
  ignore: isWsl || isCIWithoutGPU,
}, async function webgpuNullWindowSurfaceThrows() {
  const adapter = await navigator.gpu.requestAdapter();
  assert(adapter);

  const device = await adapter.requestDevice();
  assert(device);

  const msgIncludes = "Invalid parameters";

  assertThrows(
    () => {
      new Deno.UnsafeWindowSurface({
        system: "cocoa",
        windowHandle: null,
        displayHandle: null,
        width: 0,
        height: 0,
      });
    },
    TypeError,
    msgIncludes,
  );

  device.destroy();
});

Deno.test(function webgpuWindowSurfaceNoWidthHeight() {
  const msgIncludes = "expected type `v8::data::Number`, got `v8::data::Value`";

  assertThrows(
    () => {
      // @ts-expect-error width and height are required
      new Deno.UnsafeWindowSurface({
        system: "x11",
        windowHandle: null,
        displayHandle: null,
      });
    },
    TypeError,
    msgIncludes,
  );
});

Deno.test(
  { permissions: { ffi: false } },
  function webgpuWindowSurfaceFfiPermission() {
    const response = new Response("ok", { status: 201 });
    const nativeResponseSymbol = Object.getOwnPropertySymbols(response).find(
      (symbol) => symbol.description === "serve native response",
    );
    assert(nativeResponseSymbol !== undefined);
    const nativeResponse =
      (response as unknown as Record<symbol, Deno.PointerValue>)[
        nativeResponseSymbol
      ];
    assert(nativeResponse);
    const system = Deno.build.os === "darwin" ? "x11" : "cocoa";

    assertThrows(
      () => {
        new Deno.UnsafeWindowSurface({
          system,
          windowHandle: nativeResponse,
          displayHandle: nativeResponse,
          width: 1,
          height: 1,
        });
      },
      Deno.errors.NotCapable,
      "Requires ffi access",
    );
  },
);

Deno.test({
  permissions: { ffi: true },
  ignore: isWsl || isCIWithoutGPU || !hasWindowingSystem,
}, async function webgpuWindowSurfaceResizeAfterConfigure() {
  // Regression test for the RefCell double-borrow panic where the
  // UnsafeWindowSurface width/height setters held the SurfaceData borrow
  // across the canvas context resize.
  const nativeWindow = Deno.build.os === "windows"
    ? createWin32Window()
    : createX11Window();
  try {
    const adapter = await navigator.gpu.requestAdapter();
    assert(adapter);
    const device = await adapter.requestDevice();
    assert(device);
    try {
      const surface = new Deno.UnsafeWindowSurface({
        system: nativeWindow.system,
        windowHandle: nativeWindow.windowHandle,
        displayHandle: nativeWindow.displayHandle,
        width: 320,
        height: 240,
      });
      const context = surface.getContext("webgpu") as GPUCanvasContext;
      context.configure({
        device,
        format: navigator.gpu.getPreferredCanvasFormat(),
      });
      // Each assignment triggers a resize of the configured context, which
      // reads the same SurfaceData the setter just mutated. The getter reads
      // prove the setters released their borrows.
      surface.width = 640;
      surface.height = 480;
      assertEquals(surface.width, 640);
      assertEquals(surface.height, 480);
    } finally {
      device.destroy();
    }
  } finally {
    nativeWindow.close();
  }
});

// The native window and display are deliberately leaked: the wgpu surface
// held by UnsafeWindowSurface is a GC-managed object that is dropped at
// isolate teardown, after the test body has finished. Destroying the window
// or display connection earlier makes that surface drop a use-after-free in
// the driver. close() only releases the dlopen handle.
interface NativeWindow {
  system: "win32" | "x11";
  windowHandle: Deno.PointerValue;
  displayHandle: Deno.PointerValue;
  close(): void;
}

function createWin32Window(): NativeWindow {
  const user32 = Deno.dlopen(
    "user32.dll",
    {
      CreateWindowExW: {
        parameters: [
          "u32", // dwExStyle
          "buffer", // lpClassName
          "buffer", // lpWindowName
          "u32", // dwStyle
          "i32", // X
          "i32", // Y
          "i32", // nWidth
          "i32", // nHeight
          "pointer", // hWndParent
          "pointer", // hMenu
          "pointer", // hInstance
          "pointer", // lpParam
        ],
        result: "pointer",
      },
    } as const,
  );

  function wide(s: string): Uint16Array {
    const buf = new Uint16Array(s.length + 1);
    for (let i = 0; i < s.length; i++) {
      buf[i] = s.charCodeAt(i);
    }
    return buf;
  }

  const WS_OVERLAPPEDWINDOW = 0x00CF0000;
  // The predefined "STATIC" window class avoids needing RegisterClassW. The
  // window is deliberately not WS_VISIBLE; surface creation works on hidden
  // windows.
  const hwnd = user32.symbols.CreateWindowExW(
    0,
    wide("STATIC"),
    wide("deno webgpu test"),
    WS_OVERLAPPEDWINDOW,
    0,
    0,
    320,
    240,
    null,
    null,
    null,
    null,
  );
  assert(hwnd !== null, "CreateWindowExW failed");
  return {
    system: "win32",
    windowHandle: hwnd,
    displayHandle: null,
    close() {
      user32.close();
    },
  };
}

function createX11Window(): NativeWindow {
  const symbols = {
    XOpenDisplay: { parameters: ["pointer"], result: "pointer" },
    XDefaultRootWindow: { parameters: ["pointer"], result: "u64" },
    XCreateSimpleWindow: {
      // (display, parent, x, y, width, height, border_width, border,
      // background)
      parameters: [
        "pointer",
        "u64",
        "i32",
        "i32",
        "u32",
        "u32",
        "u32",
        "u64",
        "u64",
      ],
      result: "u64",
    },
    XFlush: { parameters: ["pointer"], result: "i32" },
  } as const;
  // The top-level windowing system check already dlopened libX11 and opened a
  // display, so any failure here is a real error rather than a reason to skip.
  const x11 = Deno.dlopen("libX11.so.6", symbols);
  const display = x11.symbols.XOpenDisplay(null);
  assert(display !== null, "XOpenDisplay failed");
  const root = x11.symbols.XDefaultRootWindow(display);
  const win = x11.symbols.XCreateSimpleWindow(
    display,
    root,
    0,
    0,
    320,
    240,
    0,
    0n,
    0n,
  );
  x11.symbols.XFlush(display);
  return {
    system: "x11",
    // The windowHandle external's value is the X11 window XID itself.
    windowHandle: Deno.UnsafePointer.create(BigInt(win)),
    displayHandle: display,
    close() {
      x11.close();
    },
  };
}

Deno.test(function getPreferredCanvasFormat() {
  const preferredFormat = navigator.gpu.getPreferredCanvasFormat();
  assert(preferredFormat === "bgra8unorm" || preferredFormat === "rgba8unorm");
});

Deno.test({
  ignore: isWsl || isCIWithoutGPU,
}, async function validateGPUColor() {
  const adapter = await navigator.gpu.requestAdapter();
  assert(adapter);
  const device = await adapter.requestDevice();
  assert(device);

  const format = "rgba8unorm-srgb";
  const encoder = device.createCommandEncoder();
  const texture = device.createTexture({
    size: [256, 256],
    format,
    usage: GPUTextureUsage.RENDER_ATTACHMENT | GPUTextureUsage.COPY_SRC,
  });
  const view = texture.createView();
  const storeOp = "store";
  const loadOp = "clear";

  // values for validating GPUColor
  const invalidSize = [0, 0, 0];

  const msgIncludes =
    "A sequence of number used as a GPUColor must have exactly 4 elements, received 3 elements";

  // validate the argument of descriptor.colorAttachments[@@iterator].clearValue property's length of GPUCommandEncoder.beginRenderPass when its a sequence
  // https://www.w3.org/TR/2024/WD-webgpu-20240409/#dom-gpucommandencoder-beginrenderpass
  assertThrows(
    () =>
      encoder.beginRenderPass({
        colorAttachments: [
          {
            view,
            storeOp,
            loadOp,
            clearValue: invalidSize,
          },
        ],
      }),
    TypeError,
    msgIncludes,
  );
  const renderPass = encoder.beginRenderPass({
    colorAttachments: [
      {
        view,
        storeOp,
        loadOp,
        clearValue: [0, 0, 0, 1],
      },
    ],
  });
  // validate the argument of color length of GPURenderPassEncoder.setBlendConstant when its a sequence
  // https://www.w3.org/TR/2024/WD-webgpu-20240409/#dom-gpurenderpassencoder-setblendconstant
  assertThrows(
    () => renderPass.setBlendConstant(invalidSize),
    TypeError,
    msgIncludes,
  );

  device.destroy();
});

Deno.test({
  ignore: isWsl || isCIWithoutGPU,
}, async function validateGPUExtent3D() {
  const adapter = await navigator.gpu.requestAdapter();
  assert(adapter);
  const device = await adapter.requestDevice();
  assert(device);

  const format = "rgba8unorm-srgb";
  const encoder = device.createCommandEncoder();
  const buffer = device.createBuffer({
    size: new Uint32Array([1, 4, 3, 295]).byteLength,
    usage: GPUBufferUsage.MAP_READ | GPUBufferUsage.COPY_DST,
  });
  const usage = GPUTextureUsage.RENDER_ATTACHMENT | GPUTextureUsage.COPY_SRC;
  const texture = device.createTexture({
    size: [256, 256],
    format,
    usage,
  });

  // values for validating GPUExtent3D
  const belowSize: Array<number> = [];
  const overSize = [256, 256, 1, 1];

  const msgIncludes =
    "A sequence of number used as a GPUExtent3D must have between 1 and 3 elements";

  // validate the argument of descriptor.size property's length of GPUDevice.createTexture when its a sequence
  // https://www.w3.org/TR/2024/WD-webgpu-20240409/#dom-gpudevice-createtexture
  assertThrows(
    () => device.createTexture({ size: belowSize, format, usage }),
    TypeError,
    msgIncludes,
  );
  assertThrows(
    () => device.createTexture({ size: overSize, format, usage }),
    TypeError,
    msgIncludes,
  );
  // validate the argument of copySize property's length of GPUCommandEncoder.copyBufferToTexture when its a sequence
  // https://www.w3.org/TR/2024/WD-webgpu-20240409/#dom-gpucommandencoder-copybuffertotexture
  assertThrows(
    () => encoder.copyBufferToTexture({ buffer }, { texture }, belowSize),
    TypeError,
    msgIncludes,
  );
  assertThrows(
    () => encoder.copyBufferToTexture({ buffer }, { texture }, overSize),
    TypeError,
    msgIncludes,
  );
  // validate the argument of copySize property's length of GPUCommandEncoder.copyTextureToBuffer when its a sequence
  // https://www.w3.org/TR/2024/WD-webgpu-20240409/#dom-gpucommandencoder-copytexturetobuffer
  assertThrows(
    () => encoder.copyTextureToBuffer({ texture }, { buffer }, belowSize),
    TypeError,
    msgIncludes,
  );
  assertThrows(
    () => encoder.copyTextureToBuffer({ texture }, { buffer }, overSize),
    TypeError,
    msgIncludes,
  );
  // validate the argument of copySize property's length of GPUCommandEncoder.copyTextureToTexture when its a sequence
  // https://www.w3.org/TR/2024/WD-webgpu-20240409/#dom-gpucommandencoder-copytexturetotexture
  assertThrows(
    () => encoder.copyTextureToTexture({ texture }, { texture }, belowSize),
    TypeError,
    msgIncludes,
  );
  assertThrows(
    () => encoder.copyTextureToTexture({ texture }, { texture }, overSize),
    TypeError,
    msgIncludes,
  );
  const data = new Uint8Array([1 * 255, 1 * 255, 1 * 255, 1 * 255]);
  // validate the argument of size property's length of GPUQueue.writeTexture when its a sequence
  // https://www.w3.org/TR/2024/WD-webgpu-20240409/#dom-gpuqueue-writetexture
  assertThrows(
    () => device.queue.writeTexture({ texture }, data, {}, belowSize),
    TypeError,
    msgIncludes,
  );
  assertThrows(
    () => device.queue.writeTexture({ texture }, data, {}, overSize),
    TypeError,
    msgIncludes,
  );
  // NOTE: GPUQueue.copyExternalImageToTexture needs to be validated the argument of copySize property's length when its a sequence, but it is not implemented yet

  device.destroy();
});

Deno.test({
  ignore: true,
}, async function validateGPUOrigin2D() {
  // NOTE: GPUQueue.copyExternalImageToTexture needs to be validated the argument of source.origin property's length when its a sequence, but it is not implemented yet
});

Deno.test({
  ignore: isWsl || isCIWithoutGPU,
}, async function validateGPUOrigin3D() {
  const adapter = await navigator.gpu.requestAdapter();
  assert(adapter);
  const device = await adapter.requestDevice();
  assert(device);

  const format = "rgba8unorm-srgb";
  const encoder = device.createCommandEncoder();
  const buffer = device.createBuffer({
    size: new Uint32Array([1, 4, 3, 295]).byteLength,
    usage: GPUBufferUsage.MAP_READ | GPUBufferUsage.COPY_DST,
  });
  const usage = GPUTextureUsage.RENDER_ATTACHMENT | GPUTextureUsage.COPY_SRC;
  const size = [256, 256, 1];
  const texture = device.createTexture({
    size,
    format,
    usage,
  });

  // value for validating GPUOrigin3D
  const overSize = [256, 256, 1, 1];

  const msgIncludes =
    "A sequence of number used as a GPUOrigin3D must have at most 3 elements, received 4 elements";

  // validate the argument of destination.origin property's length of GPUCommandEncoder.copyBufferToTexture when its a sequence
  // https://www.w3.org/TR/2024/WD-webgpu-20240409/#dom-gpucommandencoder-copybuffertotexture
  assertThrows(
    () =>
      encoder.copyBufferToTexture(
        { buffer },
        { texture, origin: overSize },
        size,
      ),
    TypeError,
    msgIncludes,
  );
  // validate the argument of source.origin property's length of GPUCommandEncoder.copyTextureToBuffer when its a sequence
  // https://www.w3.org/TR/2024/WD-webgpu-20240409/#dom-gpucommandencoder-copytexturetobuffer
  assertThrows(
    () =>
      encoder.copyTextureToBuffer(
        { texture, origin: overSize },
        { buffer },
        size,
      ),
    TypeError,
    msgIncludes,
  );
  // validate the argument of source.origin property's length of GPUCommandEncoder.copyTextureToTexture when its a sequence
  // https://www.w3.org/TR/2024/WD-webgpu-20240409/#dom-gpucommandencoder-copytexturetotexture
  assertThrows(
    () =>
      encoder.copyTextureToTexture(
        { texture, origin: overSize },
        { texture },
        size,
      ),
    TypeError,
    msgIncludes,
  );
  // validate the argument of destination.origin property's length of GPUCommandEncoder.copyTextureToTexture when its a sequence
  assertThrows(
    () =>
      encoder.copyTextureToTexture(
        { texture },
        { texture, origin: overSize },
        size,
      ),
    TypeError,
    msgIncludes,
  );
  // validate the argument of destination.origin property's length of GPUQueue.writeTexture when its a sequence
  // https://www.w3.org/TR/2024/WD-webgpu-20240409/#dom-gpuqueue-writetexture
  assertThrows(
    () =>
      device.queue.writeTexture(
        { texture, origin: overSize },
        new Uint8Array([1 * 255, 1 * 255, 1 * 255, 1 * 255]),
        {},
        size,
      ),
    TypeError,
    msgIncludes,
  );
  // NOTE: GPUQueue.copyExternalImageToTexture needs to be validated the argument of destination.origin property's length when its a sequence, but it is not implemented yet

  device.destroy();
});

Deno.test({
  ignore: isWsl || isCIWithoutGPU,
}, async function beginRenderPassWithoutDepthClearValue() {
  const adapter = await navigator.gpu.requestAdapter();
  assert(adapter);
  const device = await adapter.requestDevice();
  assert(device);

  const encoder = device.createCommandEncoder();

  const depthTexture = device.createTexture({
    size: [256, 256],
    format: "depth32float",
    usage: GPUTextureUsage.RENDER_ATTACHMENT,
  });
  const depthView = depthTexture.createView();

  const renderPass = encoder.beginRenderPass({
    colorAttachments: [],
    depthStencilAttachment: {
      view: depthView,
      depthLoadOp: "load",
    },
  });

  assert(renderPass);

  device.destroy();
});

// Regression test for https://github.com/denoland/deno/issues/24821.
//
// Using a `"2d-array"` view of a single-layered 2D texture as a color
// attachment used to invalidate the command encoder, causing every command
// recorded after the render pass (including a compute pass writing to an
// unrelated storage buffer) to silently no-op. The storage buffer was returned
// in its zero-initialized state.
Deno.test({
  ignore: isWsl || isCIWithoutGPU,
}, async function renderPass2dArrayColorAttachmentDoesNotCorruptComputePass() {
  const adapter = await navigator.gpu.requestAdapter();
  assert(adapter);
  const device = await adapter.requestDevice();
  assert(device);

  const texture = device.createTexture({
    format: "bgra8unorm",
    size: { width: 4, height: 4, depthOrArrayLayers: 1 },
    usage: GPUTextureUsage.RENDER_ATTACHMENT,
  });
  const view = texture.createView({ dimension: "2d-array" });

  const encoder = device.createCommandEncoder();
  const renderPass = encoder.beginRenderPass({
    colorAttachments: [{
      view,
      loadOp: "load",
      storeOp: "discard",
    }],
  });
  renderPass.end();

  const shaderModule = device.createShaderModule({
    code: `
      @group(0) @binding(0) var<storage, read_write> output: array<u32, 4>;
      @compute @workgroup_size(1)
      fn main() {
        output[0] = 0xdeadbeefu;
        output[1] = 0xcafebabeu;
        output[2] = 0xfeedfaceu;
        output[3] = 0x12345678u;
      }
    `,
  });
  const pipeline = device.createComputePipeline({
    layout: "auto",
    compute: { module: shaderModule, entryPoint: "main" },
  });

  const storageBuffer = device.createBuffer({
    size: 16,
    usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_SRC,
  });
  const readBuffer = device.createBuffer({
    size: 16,
    usage: GPUBufferUsage.COPY_DST | GPUBufferUsage.MAP_READ,
  });

  const bindGroup = device.createBindGroup({
    layout: pipeline.getBindGroupLayout(0),
    entries: [{ binding: 0, resource: { buffer: storageBuffer } }],
  });

  const computePass = encoder.beginComputePass();
  computePass.setPipeline(pipeline);
  computePass.setBindGroup(0, bindGroup);
  computePass.dispatchWorkgroups(1);
  computePass.end();

  encoder.copyBufferToBuffer(storageBuffer, 0, readBuffer, 0, 16);
  device.queue.submit([encoder.finish()]);

  await readBuffer.mapAsync(GPUMapMode.READ);
  const result = new Uint32Array(readBuffer.getMappedRange().slice(0));
  readBuffer.unmap();

  assertEquals(
    result,
    new Uint32Array([0xdeadbeef, 0xcafebabe, 0xfeedface, 0x12345678]),
  );

  device.destroy();
});

Deno.test({
  ignore: isWsl || isCIWithoutGPU,
}, async function adapterLimitsAreNumbers() {
  const limitNames = [
    "maxTextureDimension1D",
    "maxTextureDimension2D",
    "maxTextureDimension3D",
    "maxTextureArrayLayers",
    "maxBindGroups",
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
    "maxVertexAttributes",
    "maxVertexBufferArrayStride",
    "maxComputeWorkgroupStorageSize",
    "maxComputeInvocationsPerWorkgroup",
    "maxComputeWorkgroupSizeX",
    "maxComputeWorkgroupSizeY",
    "maxComputeWorkgroupSizeZ",
    "maxComputeWorkgroupsPerDimension",
  ];

  const adapter = await navigator.gpu.requestAdapter();
  assert(adapter);

  for (const limitName of limitNames) {
    // deno-lint-ignore ban-ts-comment
    // @ts-ignore
    assertEquals(typeof adapter.limits[limitName], "number", limitName);
  }

  const device = await adapter.requestDevice({
    // deno-lint-ignore ban-ts-comment
    // @ts-ignore
    requiredLimits: adapter.limits,
  });
  assert(device);

  device.destroy();
});

Deno.test({
  ignore: isWsl || isCIWithoutGPU,
}, async function testOnSubmittedWorkDone() {
  const adapter = await navigator.gpu.requestAdapter();
  assert(adapter);

  const device = await adapter.requestDevice();
  assert(device);

  const encoder = device.createCommandEncoder();
  const fut = device.queue.onSubmittedWorkDone();
  device.queue.submit([encoder.finish()]);
  await fut;

  device.destroy();
});

Deno.test({
  permissions: { read: true, env: true },
  ignore: isWsl || isCIWithoutGPU,
}, async function webgpuWriteBufferTypedArrayElementOffsets() {
  const adapter = await navigator.gpu.requestAdapter();
  assert(adapter);
  const device = await adapter.requestDevice();
  assert(device);

  const source = new Float32Array([1.0, 2.0, 3.0, 4.0]);
  const gpuBuffer = device.createBuffer({
    size: 2 * Float32Array.BYTES_PER_ELEMENT,
    usage: GPUBufferUsage.COPY_SRC | GPUBufferUsage.COPY_DST,
  });

  // dataOffset=1 and size=2 should be in elements for a Float32Array
  device.queue.writeBuffer(gpuBuffer, 0, source, 1, 2);

  const readBuffer = device.createBuffer({
    size: 2 * Float32Array.BYTES_PER_ELEMENT,
    usage: GPUBufferUsage.COPY_DST | GPUBufferUsage.MAP_READ,
  });

  const encoder = device.createCommandEncoder();
  encoder.copyBufferToBuffer(
    gpuBuffer,
    0,
    readBuffer,
    0,
    2 * Float32Array.BYTES_PER_ELEMENT,
  );
  device.queue.submit([encoder.finish()]);

  await readBuffer.mapAsync(GPUMapMode.READ);
  const result = new Float32Array(readBuffer.getMappedRange());
  assertEquals(result, new Float32Array([2.0, 3.0]));

  readBuffer.unmap();
  device.destroy();
});

Deno.test({
  permissions: { env: true },
  ignore: isWsl || isCIWithoutGPU,
}, async function writeBufferAcceptsArrayBuffer() {
  const adapter = await navigator.gpu.requestAdapter();
  assert(adapter);
  const device = await adapter.requestDevice();
  assert(device);

  const source = new ArrayBuffer(4 * Float32Array.BYTES_PER_ELEMENT);
  new Float32Array(source).set([1.0, 2.0, 3.0, 4.0]);

  const gpuBuffer = device.createBuffer({
    size: source.byteLength,
    usage: GPUBufferUsage.COPY_SRC | GPUBufferUsage.COPY_DST,
  });

  device.queue.writeBuffer(gpuBuffer, 0, source);

  const readBuffer = device.createBuffer({
    size: source.byteLength,
    usage: GPUBufferUsage.COPY_DST | GPUBufferUsage.MAP_READ,
  });

  const encoder = device.createCommandEncoder();
  encoder.copyBufferToBuffer(gpuBuffer, 0, readBuffer, 0, source.byteLength);
  device.queue.submit([encoder.finish()]);

  await readBuffer.mapAsync(GPUMapMode.READ);
  const result = new Float32Array(readBuffer.getMappedRange());
  assertEquals(result, new Float32Array([1.0, 2.0, 3.0, 4.0]));

  readBuffer.unmap();
  device.destroy();
});

// Regression test for https://github.com/denoland/deno/issues/33956.
// Before the fix, the Uint32Array fast path of setBindGroup sliced the
// backing ArrayBuffer with an unchecked `&data[start..start+len]`. An
// out-of-range length panicked inside the op's `extern "C"` callback,
// which crosses the C ABI as a process abort. After the fix, the
// bounds-check surfaces a GPUValidationError instead.
Deno.test({
  permissions: { read: true, env: true },
  ignore: isWsl || isCIWithoutGPU,
}, async function webgpuSetBindGroupBoundsCheck() {
  const adapter = await navigator.gpu.requestAdapter();
  assert(adapter);
  const device = await adapter.requestDevice();

  const layout = device.createBindGroupLayout({ entries: [] });
  const bg = device.createBindGroup({ layout, entries: [] });

  // ComputePass.setBindGroup: out-of-range len pushes a validation
  // error instead of aborting the process.
  {
    const encoder = device.createCommandEncoder();
    const pass = encoder.beginComputePass();
    device.pushErrorScope("validation");
    pass.setBindGroup(0, bg, new Uint32Array(4), 0, 1_000_000);
    pass.end();
    const err = await device.popErrorScope();
    assert(err, "expected GPUValidationError on out-of-range setBindGroup");
  }

  // RenderBundleEncoder.setBindGroup: same input shape, different code
  // path (returns the validation as a thrown JS error, since there is
  // no error_handler.push_error at this site).
  {
    const bundleEncoder = device.createRenderBundleEncoder({
      colorFormats: ["rgba8unorm"],
    });
    let threw = false;
    try {
      bundleEncoder.setBindGroup(0, bg, new Uint32Array(4), 0, 1_000_000);
    } catch (_) {
      threw = true;
    }
    assert(
      threw,
      "expected setBindGroup on a RenderBundleEncoder to throw on out-of-range args",
    );
  }

  // Sanity: valid args still work (no regression on the happy path).
  {
    const encoder = device.createCommandEncoder();
    const pass = encoder.beginComputePass();
    pass.setBindGroup(0, bg, new Uint32Array(4), 0, 0);
    pass.end();
  }

  device.destroy();
});

async function checkIsWsl() {
  return Deno.build.os === "linux" && await hasMicrosoftProcVersion();

  async function hasMicrosoftProcVersion() {
    // https://github.com/microsoft/WSL/issues/423#issuecomment-221627364
    try {
      const procVersion = await Deno.readTextFile("/proc/version");
      return /microsoft/i.test(procVersion);
    } catch {
      return false;
    }
  }
}
