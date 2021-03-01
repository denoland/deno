// TODO(lucacasonato): remove when GPUBufferUsage and friends are added to dlint
// deno-lint-ignore-file no-undef

import { assert, assertEquals, unitTest } from "./test_util.ts";

let isCI: boolean;
try {
  isCI = (Deno.env.get("CI")?.length ?? 0) > 0;
} catch {
  isCI = true;
}

// Skip this test on linux CI, because the vulkan emulator is not good enough
// yet, and skip on macOS because these do not have virtual GPUs.
unitTest({
  perms: { read: true, env: true },
  ignore: (Deno.build.os === "linux" || Deno.build.os === "darwin") && isCI,
}, async function webgpuComputePass() {
  const adapter = await navigator.gpu.requestAdapter();
  assert(adapter);

  const numbers = [1, 4, 3, 295];

  const device = await adapter.requestDevice();
  assert(device);

  const shaderCode = await Deno.readTextFile(
    "cli/tests/webgpu_computepass_shader.wgsl",
  );

  const shaderModule = device.createShaderModule({
    code: shaderCode,
  });

  const size = new Uint32Array(numbers).byteLength;

  const stagingBuffer = device.createBuffer({
    size: size,
    usage: 1 | 8,
  });

  const storageBuffer = device.createBuffer({
    label: "Storage Buffer",
    size: size,
    usage: 0x80 | 8 | 4,
    mappedAtCreation: true,
  });

  const buf = new Uint32Array(storageBuffer.getMappedRange());

  buf.set(numbers);

  storageBuffer.unmap();

  const bindGroupLayout = device.createBindGroupLayout({
    entries: [
      {
        binding: 0,
        visibility: 4,
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
          buffer: storageBuffer,
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
      entryPoint: "main",
    },
  });

  const encoder = device.createCommandEncoder();

  const computePass = encoder.beginComputePass();
  computePass.setPipeline(computePipeline);
  computePass.setBindGroup(0, bindGroup);
  computePass.insertDebugMarker("compute collatz iterations");
  computePass.dispatch(numbers.length);
  computePass.endPass();

  encoder.copyBufferToBuffer(storageBuffer, 0, stagingBuffer, 0, size);

  device.queue.submit([encoder.finish()]);

  await stagingBuffer.mapAsync(1);

  const data = stagingBuffer.getMappedRange();

  assertEquals(new Uint32Array(data), new Uint32Array([0, 2, 7, 55]));

  stagingBuffer.unmap();

  device.destroy();

  // TODO(lucacasonato): webgpu spec should add a explicit destroy method for
  // adapters.
  const resources = Object.keys(Deno.resources());
  Deno.close(Number(resources[resources.length - 1]));
});

// Skip this test on linux CI, because the vulkan emulator is not good enough
// yet, and skip on macOS because these do not have virtual GPUs.
unitTest({
  perms: { read: true, env: true },
  ignore: (Deno.build.os === "linux" || Deno.build.os === "darwin") && isCI,
}, async function webgpuHelloTriangle() {
  const adapter = await navigator.gpu.requestAdapter();
  assert(adapter);

  const device = await adapter.requestDevice();
  assert(device);

  const shaderCode = await Deno.readTextFile(
    "cli/tests/webgpu_hellotriangle_shader.wgsl",
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
    },
    fragment: {
      module: shaderModule,
      entryPoint: "fs_main",
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
  const renderPass = encoder.beginRenderPass({
    colorAttachments: [
      {
        view: texture.createView(),
        storeOp: "store",
        loadValue: [0, 1, 0, 1],
      },
    ],
  });
  renderPass.setPipeline(renderPipeline);
  renderPass.draw(3, 1);
  renderPass.endPass();

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

  assertEquals(data, await Deno.readFile("cli/tests/webgpu_hellotriangle.out"));

  outputBuffer.unmap();

  device.destroy();

  // TODO(lucacasonato): webgpu spec should add a explicit destroy method for
  // adapters.
  const resources = Object.keys(Deno.resources());
  Deno.close(Number(resources[resources.length - 1]));
});
