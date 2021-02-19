import { assert, assertEquals, unitTest } from "./test_util.ts";

let isCI: boolean;
try {
  isCI = (Deno.env.get("CI")?.length ?? 0) > 0;
} catch {
  isCI = true;
}

const adapter = await navigator.gpu.requestAdapter();
assert(adapter);

// Skip this test on linux CI, because the vulkan emulator is not good enough
// yet, and skip on macOS because these do not have virtual GPUs.
unitTest({
  perms: { read: true, env: true },
  ignore: (Deno.build.os === "linux" || Deno.build.os === "darwin") && isCI,
}, async function webgpuComputePass() {
  const numbers = [1, 4, 3, 295];

  const device = await adapter.requestDevice();
  assert(device);

  const shaderCode = await Deno.readTextFile("cli/tests/shader.wgsl");

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
});
