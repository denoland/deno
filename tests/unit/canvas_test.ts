// Copyright 2018-2026 the Deno authors. MIT license.

import { assert, assertEquals } from "./test_util.ts";

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
        `Mismatch at byte ${i} (pixel ${Math.floor(i / 4)}, channel ${i % 4}): got ${bitmapData[i]}, expected ${expectedData[i]}`,
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
        `Mismatch at byte ${i} (pixel ${Math.floor(i / 4)}, channel ${i % 4}): got ${outputData[i]}, expected ${sourceData[i]}`,
      );
    }
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
