// Copyright 2018-2025 the Deno authors. MIT license.

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
  );

  device.destroy();
});

Deno.test(function webgpuWindowSurfaceNoWidthHeight() {
  assertThrows(
    () => {
      // @ts-expect-error width and height are required
      new Deno.UnsafeWindowSurface({
        system: "x11",
        windowHandle: null,
        displayHandle: null,
      });
    },
  );
});

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
