const win = new Deno.BrowserWindow({ title: "GPU app" });

const adapter = await navigator.gpu.requestAdapter();
const device = await adapter!.requestDevice();

// the OS window, as a WebGPU surface
const surface = win.getNativeWindow();

const format = navigator.gpu.getPreferredCanvasFormat();
const ctx = surface.getContext("webgpu")!;
ctx.configure({
  device,
  format,
});

// A single triangle, with per-vertex colors, generated entirely in the shader.
const shader = device.createShaderModule({
  code: /* wgsl */ `
    struct VertexOut {
      @builtin(position) position: vec4<f32>,
      @location(0) color: vec3<f32>,
    };

    @vertex
    fn vs_main(@builtin(vertex_index) i: u32) -> VertexOut {
      var positions = array<vec2<f32>, 3>(
        vec2<f32>( 0.0,  0.6),
        vec2<f32>(-0.6, -0.6),
        vec2<f32>( 0.6, -0.6),
      );
      var colors = array<vec3<f32>, 3>(
        vec3<f32>(1.0, 0.0, 0.0),
        vec3<f32>(0.0, 1.0, 0.0),
        vec3<f32>(0.0, 0.0, 1.0),
      );

      var out: VertexOut;
      out.position = vec4<f32>(positions[i], 0.0, 1.0);
      out.color = colors[i];
      return out;
    }

    @fragment
    fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
      return vec4<f32>(in.color, 1.0);
    }
  `,
});

const pipeline = device.createRenderPipeline({
  layout: "auto",
  vertex: { module: shader, entryPoint: "vs_main" },
  fragment: {
    module: shader,
    entryPoint: "fs_main",
    targets: [{ format }],
  },
  primitive: { topology: "triangle-list" },
});

function frame() {
  const texture = ctx.getCurrentTexture();
  const encoder = device.createCommandEncoder();
  const pass = encoder.beginRenderPass({
    colorAttachments: [
      {
        view: texture.createView(),
        loadOp: "clear",
        storeOp: "store",
        clearValue: [0.1, 0.1, 0.12, 1.0],
      },
    ],
  });
  pass.setPipeline(pipeline);
  pass.draw(3);
  pass.end();

  device.queue.submit([encoder.finish()]);
  surface.present();
}

frame();
