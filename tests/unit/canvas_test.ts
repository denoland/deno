// Copyright 2018-2026 the Deno authors. MIT license.

import {
  assert,
  assertEquals,
  assertRejects,
  assertThrows,
} from "./test_util.ts";

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

Deno.test({
  permissions: { read: true, env: true },
  ignore: isWsl || isCIWithoutGPU,
}, async function offscreenCanvasWebGPURender() {
  const offscreenCanvas = new OffscreenCanvas(200, 200);
  const webgpuContext = offscreenCanvas.getContext("webgpu");
  assert(webgpuContext);

  const adapter = await navigator.gpu.requestAdapter();
  assert(adapter);
  const device = await adapter.requestDevice();
  assert(device);

  webgpuContext.configure({
    device,
    format: "rgba8unorm",
  });

  const shaderCode = `
@vertex
fn vs_main(@builtin(vertex_index) in_vertex_index: u32) -> @builtin(position) vec4<f32> {
    let x = f32(i32(in_vertex_index) - 1);
    let y = f32(i32(in_vertex_index & 1u) * 2 - 1);
    return vec4<f32>(x, y, 0.0, 1.0);
}

@fragment
fn fs_main() -> @location(0) vec4<f32> {
    return vec4<f32>(1.0, 0.0, 0.0, 1.0);
}
`;

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
    },
    fragment: {
      module: shaderModule,
      entryPoint: "fs_main",
      targets: [
        {
          format: "rgba8unorm",
        },
      ],
    },
  });

  const view = webgpuContext.getCurrentTexture().createView();

  const encoder = device.createCommandEncoder();
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
  device.queue.submit([encoder.finish()]);

  const outputBlob = await offscreenCanvas.convertToBlob({
    type: "image/png",
  });
  const imageBitmap = await createImageBitmap(outputBlob);
  // @ts-ignore: Deno[Deno.internal] allowed
  const bitmapData: Uint8Array = Deno[Deno.internal].getBitmapData(imageBitmap);

  const expectedData = await Deno.readFile(
    "tests/testdata/webgpu/hellotriangle_canvas.out",
  );

  assertEquals(bitmapData.length, expectedData.length);
  for (let i = 0; i < bitmapData.length; i++) {
    if (bitmapData[i] !== expectedData[i]) {
      throw new Error(
        `Mismatch at byte ${i} (pixel ${Math.floor(i / 4)}, channel ${
          i % 4
        }): got ${bitmapData[i]}, expected ${expectedData[i]}`,
      );
    }
  }

  device.destroy();
});

Deno.test({
  permissions: { read: true, env: true },
  ignore: isWsl || isCIWithoutGPU,
}, async function offscreenCanvasBitmapRenderer() {
  // Create a source image via OffscreenCanvas with WebGPU
  const sourceCanvas = new OffscreenCanvas(200, 200);
  const webgpuContext = sourceCanvas.getContext("webgpu");
  assert(webgpuContext);

  const adapter = await navigator.gpu.requestAdapter();
  assert(adapter);
  const device = await adapter.requestDevice();
  assert(device);

  webgpuContext.configure({
    device,
    format: "rgba8unorm",
  });

  const encoder = device.createCommandEncoder();
  const view = webgpuContext.getCurrentTexture().createView();
  const renderPass = encoder.beginRenderPass({
    colorAttachments: [
      {
        view,
        storeOp: "store",
        loadOp: "clear",
        clearValue: [1, 0, 0, 1],
      },
    ],
  });
  renderPass.end();
  device.queue.submit([encoder.finish()]);

  const sourceBlob = await sourceCanvas.convertToBlob({
    type: "image/png",
  });

  // Round-trip through bitmaprenderer
  const bitmap = await createImageBitmap(sourceBlob);
  assert(bitmap);
  assertEquals(bitmap.width, 200);
  assertEquals(bitmap.height, 200);

  // Get source data before transfer (transferFromImageBitmap detaches the bitmap)
  // @ts-ignore: Deno[Deno.internal] allowed
  const sourceData: Uint8Array = Deno[Deno.internal].getBitmapData(bitmap);

  const canvas = new OffscreenCanvas(200, 200);
  const bitmaprenderer = canvas.getContext("bitmaprenderer");
  assert(bitmaprenderer);
  bitmaprenderer.transferFromImageBitmap(bitmap);

  const outputBlob = await canvas.convertToBlob();
  const outputBitmap = await createImageBitmap(outputBlob);
  // @ts-ignore: Deno[Deno.internal] allowed
  const outputData: Uint8Array = Deno[Deno.internal].getBitmapData(
    outputBitmap,
  );

  assertEquals(outputData.length, sourceData.length);
  for (let i = 0; i < outputData.length; i++) {
    if (outputData[i] !== sourceData[i]) {
      throw new Error(
        `Mismatch at byte ${i} (pixel ${Math.floor(i / 4)}, channel ${
          i % 4
        }): got ${outputData[i]}, expected ${sourceData[i]}`,
      );
    }
  }

  device.destroy();
});

Deno.test(function offscreenCanvasConstructor() {
  const canvas = new OffscreenCanvas(100, 50);
  assertEquals(canvas.width, 100);
  assertEquals(canvas.height, 50);
});

Deno.test(function offscreenCanvasConstructorRequiresArgs() {
  // @ts-expect-error: testing webidl required-arg behavior
  assertThrows(() => new OffscreenCanvas(), TypeError);
  // @ts-expect-error: testing webidl required-arg behavior
  assertThrows(() => new OffscreenCanvas(100), TypeError);
});

Deno.test(function offscreenCanvasResize() {
  const canvas = new OffscreenCanvas(100, 100);
  canvas.width = 200;
  canvas.height = 50;
  assertEquals(canvas.width, 200);
  assertEquals(canvas.height, 50);
});

Deno.test(function offscreenCanvasGetContextStickyId() {
  const canvas = new OffscreenCanvas(10, 10);
  const ctx1 = canvas.getContext("bitmaprenderer");
  const ctx2 = canvas.getContext("bitmaprenderer");
  assert(ctx1);
  assertEquals(ctx1, ctx2);
});

Deno.test(function offscreenCanvasGetContextDifferentIdReturnsNull() {
  const canvas = new OffscreenCanvas(10, 10);
  const bm = canvas.getContext("bitmaprenderer");
  assert(bm);
  // Once a context is bound, requesting a different supported id yields null.
  assertEquals(canvas.getContext("webgpu"), null);
});

Deno.test(function offscreenCanvasGetContextUnsupportedReturnsNull() {
  const canvas = new OffscreenCanvas(10, 10);
  // @ts-expect-error: testing unsupported context id
  assertEquals(canvas.getContext("not-a-real-context"), null);
  // After an unsupported probe, a supported id still binds.
  assert(canvas.getContext("bitmaprenderer"));
});

Deno.test(function offscreenCanvasTransferToImageBitmapWithoutContextThrows() {
  const canvas = new OffscreenCanvas(10, 10);
  assertThrows(() => canvas.transferToImageBitmap(), Error);
});

Deno.test(async function offscreenCanvasConvertToBlobWithoutContextRejects() {
  const canvas = new OffscreenCanvas(10, 10);
  await assertRejects(() => canvas.convertToBlob(), Error);
});

Deno.test(async function offscreenCanvasConvertToBlobReturnsPromise() {
  const canvas = new OffscreenCanvas(2, 2);
  canvas.getContext("bitmaprenderer");
  const ret = canvas.convertToBlob();
  assert(ret instanceof Promise);
  const blob = await ret;
  assert(blob instanceof Blob);
  assertEquals(blob.type, "image/png");
});

Deno.test(async function offscreenCanvasConvertToBlobJpeg() {
  const canvas = new OffscreenCanvas(2, 2);
  canvas.getContext("bitmaprenderer");
  const blob = await canvas.convertToBlob({ type: "image/jpeg" });
  assertEquals(blob.type, "image/jpeg");
  assert(blob.size > 0);
});

Deno.test(async function offscreenCanvasConvertToBlobIco() {
  const canvas = new OffscreenCanvas(2, 2);
  canvas.getContext("bitmaprenderer");
  const blob = await canvas.convertToBlob({ type: "image/x-icon" });
  assertEquals(blob.type, "image/x-icon");
  assert(blob.size > 0);
});

Deno.test(async function offscreenCanvasConvertToBlobBmp() {
  const canvas = new OffscreenCanvas(2, 2);
  canvas.getContext("bitmaprenderer");
  const blob = await canvas.convertToBlob({ type: "image/bmp" });
  assertEquals(blob.type, "image/bmp");
  assert(blob.size > 0);
});

Deno.test(async function bitmapRendererCanvasGetter() {
  const canvas = new OffscreenCanvas(10, 10);
  const ctx = canvas.getContext("bitmaprenderer");
  assert(ctx);
  assertEquals(ctx.canvas, canvas);
});

Deno.test(async function bitmapRendererTransferFromDetachedThrows() {
  const canvas = new OffscreenCanvas(2, 2);
  const ctx = canvas.getContext("bitmaprenderer");
  assert(ctx);

  const source = new OffscreenCanvas(2, 2);
  source.getContext("bitmaprenderer");
  const blob = await source.convertToBlob();
  const bitmap = await createImageBitmap(blob);

  // First transfer detaches the bitmap.
  ctx.transferFromImageBitmap(bitmap);
  // Second transfer with the same (now-detached) bitmap throws.
  assertThrows(() => ctx.transferFromImageBitmap(bitmap), Error);
});

Deno.test(async function bitmapRendererTransferFromNullClears() {
  const canvas = new OffscreenCanvas(4, 4);
  const ctx = canvas.getContext("bitmaprenderer");
  assert(ctx);
  // Per spec: passing null clears the renderer's bitmap. Must not throw.
  ctx.transferFromImageBitmap(null);
});

Deno.test({
  permissions: { read: true, env: true },
  ignore: isWsl || isCIWithoutGPU,
}, async function gpuCanvasContextGetConfigurationNullByDefault() {
  const canvas = new OffscreenCanvas(10, 10);
  const ctx = canvas.getContext("webgpu");
  assert(ctx);
  assertEquals(ctx.getConfiguration(), null);
});

Deno.test({
  permissions: { read: true, env: true },
  ignore: isWsl || isCIWithoutGPU,
}, async function gpuCanvasContextGetConfigurationRoundTrip() {
  const canvas = new OffscreenCanvas(10, 10);
  const ctx = canvas.getContext("webgpu");
  assert(ctx);

  const adapter = await navigator.gpu.requestAdapter();
  assert(adapter);
  const device = await adapter.requestDevice();
  assert(device);

  const config = {
    device,
    format: "rgba8unorm" as GPUTextureFormat,
    alphaMode: "opaque" as GPUCanvasAlphaMode,
  };
  ctx.configure(config);

  const got = ctx.getConfiguration();
  assert(got);
  assertEquals(got.device, device);
  assertEquals(got.format, "rgba8unorm");
  assertEquals(got.alphaMode, "opaque");

  ctx.unconfigure();
  assertEquals(ctx.getConfiguration(), null);

  device.destroy();
});

Deno.test({
  permissions: { read: true, env: true },
  ignore: isWsl || isCIWithoutGPU,
}, async function gpuCanvasContextConfigureUnsupportedFormatThrows() {
  const canvas = new OffscreenCanvas(10, 10);
  const ctx = canvas.getContext("webgpu");
  assert(ctx);

  const adapter = await navigator.gpu.requestAdapter();
  assert(adapter);
  const device = await adapter.requestDevice();
  assert(device);

  assertThrows(
    () =>
      ctx.configure({
        device,
        // depth24plus is not a valid canvas format
        format: "depth24plus" as GPUTextureFormat,
      }),
    TypeError,
  );

  device.destroy();
});

Deno.test({
  permissions: { read: true, env: true },
  ignore: isWsl || isCIWithoutGPU,
}, async function gpuCanvasContextGetCurrentTextureRequiresConfigure() {
  const canvas = new OffscreenCanvas(10, 10);
  const ctx = canvas.getContext("webgpu");
  assert(ctx);
  assertThrows(() => ctx.getCurrentTexture(), TypeError);
});

Deno.test({
  permissions: { read: true, env: true },
  ignore: isWsl || isCIWithoutGPU,
}, async function gpuCanvasContextResizeReplacesTexture() {
  const canvas = new OffscreenCanvas(10, 10);
  const ctx = canvas.getContext("webgpu");
  assert(ctx);

  const adapter = await navigator.gpu.requestAdapter();
  assert(adapter);
  const device = await adapter.requestDevice();
  assert(device);

  ctx.configure({ device, format: "rgba8unorm" });

  const tex1 = ctx.getCurrentTexture();
  // Resizing the canvas must replace the drawing buffer, so a fresh texture
  // is returned on the next call.
  canvas.width = 20;
  const tex2 = ctx.getCurrentTexture();
  assert(tex1 !== tex2);

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
