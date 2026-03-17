const offscreenCanvas = new OffscreenCanvas(200, 200);
const webgpuContext = offscreenCanvas.getContext("webgpu");
const adapter = await navigator.gpu.requestAdapter();
const device = await adapter?.requestDevice();
device.onuncapturederror = (e) => console.error(e.error.message);

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

await Deno.writeFile("./output.png", outputBlob.stream());
