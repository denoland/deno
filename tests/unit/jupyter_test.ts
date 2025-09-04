// Copyright 2018-2025 the Deno authors. MIT license.

import { assert, assertEquals, assertThrows } from "./test_util.ts";

// @ts-expect-error TypeScript (as of 3.7) does not support indexing namespaces by symbol
const format = Deno[Deno.internal].jupyter.formatInner;

Deno.test("Deno.jupyter is not available", () => {
  assertThrows(
    () => Deno.jupyter,
    "Deno.jupyter is only available in `deno jupyter` subcommand.",
  );
});

export async function assertFormattedAs(obj: unknown, result: object) {
  const formatted = await format(obj);
  assertEquals(formatted, result);
}

Deno.test("display(canvas) creates a PNG", async () => {
  // Let's make a fake Canvas with a fake Data URL
  class FakeCanvas {
    toDataURL() {
      return "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAUAAAAFCAYAAACNbyblAAAAAXNSR0IArs4c6QAAAARzQklUCAgICHwIZIgAAAAVSURBVAiZY/zPwPCfAQ0woQtQQRAAzqkCCB/D3o0AAAAASUVORK5CYII=";
    }
  }
  const canvas = new FakeCanvas();

  await assertFormattedAs(canvas, {
    "image/png":
      "iVBORw0KGgoAAAANSUhEUgAAAAUAAAAFCAYAAACNbyblAAAAAXNSR0IArs4c6QAAAARzQklUCAgICHwIZIgAAAAVSURBVAiZY/zPwPCfAQ0woQtQQRAAzqkCCB/D3o0AAAAASUVORK5CYII=",
  });
});

Deno.test(
  "class with a Symbol.for('Jupyter.display') function gets displayed",
  async () => {
    class Example {
      x: number;

      constructor(x: number) {
        this.x = x;
      }

      [Symbol.for("Jupyter.display")]() {
        return { "application/json": { x: this.x } };
      }
    }

    const example = new Example(5);

    // Now to check on the broadcast call being made
    await assertFormattedAs(example, { "application/json": { x: 5 } });
  },
);

Deno.test(
  "class with an async Symbol.for('Jupyter.display') function gets displayed",
  async () => {
    class Example {
      x: number;

      constructor(x: number) {
        this.x = x;
      }

      async [Symbol.for("Jupyter.display")]() {
        await new Promise((resolve) => setTimeout(resolve, 0));

        return { "application/json": { x: this.x } };
      }
    }

    const example = new Example(3);

    // Now to check on the broadcast call being made
    await assertFormattedAs(example, { "application/json": { x: 3 } });
  },
);

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

Deno.test(
  {
    ignore: isWsl || isCIWithoutGPU,
    name: "display GPUTexture",
  },
  async () => {
    const dimensions = {
      width: 200,
      height: 200,
    };

    const adapter = await navigator.gpu.requestAdapter();
    const device = await adapter?.requestDevice();

    assert(device);

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
            format: "rgba8unorm-srgb",
          },
        ],
      },
    });

    const texture = device.createTexture({
      label: "Capture",
      size: {
        width: dimensions.width,
        height: dimensions.height,
      },
      format: "rgba8unorm-srgb",
      usage: GPUTextureUsage.RENDER_ATTACHMENT | GPUTextureUsage.COPY_SRC,
    });

    const encoder = device.createCommandEncoder();
    const renderPass = encoder.beginRenderPass({
      colorAttachments: [
        {
          view: texture.createView(),
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

    await assertFormattedAs(texture, {
      "image/png":
        "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",
    });
  },
);

Deno.test(
  {
    ignore: isWsl || isCIWithoutGPU,
    name: "display GPUBuffer",
  },
  async () => {
    // Get some numbers from the command line, or use the default 1, 4, 3, 295.
    let numbers: Uint32Array;
    if (Deno.args.length > 0) {
      numbers = new Uint32Array(Deno.args.map((a) => parseInt(a)));
    } else {
      numbers = new Uint32Array([1, 4, 3, 295]);
    }

    const adapter = await navigator.gpu.requestAdapter();
    const device = await adapter?.requestDevice();
    assert(device);

    const shaderCode = `
@group(0)
@binding(0)
var<storage, read_write> v_indices: array<u32>; // this is used as both input and output for convenience

// The Collatz Conjecture states that for any integer n:
// If n is even, n = n/2
// If n is odd, n = 3n+1
// And repeat this process for each new n, you will always eventually reach 1.
// Though the conjecture has not been proven, no counterexample has ever been found.
// This function returns how many times this recurrence needs to be applied to reach 1.
fn collatz_iterations(n_base: u32) -> u32{
    var n: u32 = n_base;
    var i: u32 = 0u;
    loop {
        if (n <= 1u) {
            break;
        }
        if (n % 2u == 0u) {
            n = n / 2u;
        }
        else {
            // Overflow? (i.e. 3*n + 1 > 0xffffffffu?)
            if (n >= 1431655765u) {   // 0x55555555u
                return 4294967295u;   // 0xffffffffu
            }

            n = 3u * n + 1u;
        }
        i = i + 1u;
    }
    return i;
}

@compute
@workgroup_size(1)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    v_indices[global_id.x] = collatz_iterations(v_indices[global_id.x]);
}
`;

    const shaderModule = device.createShaderModule({
      code: shaderCode,
    });

    const stagingBuffer = device.createBuffer({
      size: numbers.byteLength,
      usage: GPUBufferUsage.MAP_READ | GPUBufferUsage.COPY_DST,
    });

    const contents = new Uint8Array(numbers.buffer);

    const alignMask = 4 - 1;
    const paddedSize = Math.max(
      (contents.byteLength + alignMask) & ~alignMask,
      4,
    );

    const storageBuffer = device.createBuffer({
      label: "Storage Buffer",
      usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST |
        GPUBufferUsage.COPY_SRC,
      mappedAtCreation: true,
      size: paddedSize,
    });
    const data = new Uint8Array(storageBuffer.getMappedRange());
    data.set(contents);
    storageBuffer.unmap();

    const computePipeline = device.createComputePipeline({
      layout: "auto",
      compute: {
        module: shaderModule,
        entryPoint: "main",
      },
    });

    const bindGroupLayout = computePipeline.getBindGroupLayout(0);
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

    const encoder = device.createCommandEncoder();

    const computePass = encoder.beginComputePass();
    computePass.setPipeline(computePipeline);
    computePass.setBindGroup(0, bindGroup);
    computePass.insertDebugMarker("compute collatz iterations");
    computePass.dispatchWorkgroups(numbers.length);
    computePass.end();

    encoder.copyBufferToBuffer(
      storageBuffer,
      0,
      stagingBuffer,
      0,
      numbers.byteLength,
    );

    device.queue.submit([encoder.finish()]);

    await assertFormattedAs(stagingBuffer, {
      "text/plain":
        "[\n   \x1b[33m0\x1b[39m, \x1b[33m0\x1b[39m, \x1b[33m0\x1b[39m, \x1b[33m0\x1b[39m, \x1b[33m2\x1b[39m, \x1b[33m0\x1b[39m,\n   \x1b[33m0\x1b[39m, \x1b[33m0\x1b[39m, \x1b[33m7\x1b[39m, \x1b[33m0\x1b[39m, \x1b[33m0\x1b[39m, \x1b[33m0\x1b[39m,\n  \x1b[33m55\x1b[39m, \x1b[33m0\x1b[39m, \x1b[33m0\x1b[39m, \x1b[33m0\x1b[39m\n]",
    });
  },
);
