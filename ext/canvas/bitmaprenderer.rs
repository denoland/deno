// Copyright 2018-2025 the Deno authors. MIT license.
use deno_core::GarbageCollected;
use deno_core::WebIDL;
use deno_core::cppgc::Ref;
use deno_core::op2;
use deno_core::v8;
use deno_core::v8::cppgc::Visitor;
use deno_core::webidl::Nullable;
use deno_core::webidl::WebIdlConverter;
use deno_error::JsErrorBox;
use deno_image::bitmap::ImageBitmap;
use deno_image::image;
use deno_image::image::DynamicImage;
use deno_image::image::GenericImageView;
use deno_webgpu::canvas::ContextData;
use deno_webgpu::error::GPUError;
use deno_webgpu::wgpu_core;
use deno_webgpu::wgpu_types;

#[derive(Clone)]
pub struct SurfaceBitmap {
  pub instance: deno_webgpu::Instance,
  pub device: wgpu_core::id::DeviceId,
  pub queue: wgpu_core::id::QueueId,

  render_pipeline: wgpu_core::id::RenderPipelineId,
  vertex_buffer: wgpu_core::id::BufferId,
  index_buffer: wgpu_core::id::BufferId,
  bind_group_layout: wgpu_core::id::BindGroupLayoutId,
  sampler: wgpu_core::id::SamplerId,
}

impl Drop for SurfaceBitmap {
  fn drop(&mut self) {
    self.instance.device_drop(self.device);
    self.instance.queue_drop(self.queue);
    self.instance.render_pipeline_drop(self.render_pipeline);
    self.instance.buffer_drop(self.vertex_buffer);
    self.instance.buffer_drop(self.index_buffer);
    self.instance.bind_group_layout_drop(self.bind_group_layout);
    self.instance.sampler_drop(self.sampler);
  }
}

pub struct ImageBitmapRenderingContext {
  canvas: v8::Global<v8::Object>,
  data: ContextData,

  pub surface_only: Option<SurfaceBitmap>,

  alpha: bool,
}

// SAFETY: we're sure this can be GCed
unsafe impl GarbageCollected for ImageBitmapRenderingContext {
  fn trace(&self, _visitor: &mut Visitor) {}

  fn get_name(&self) -> &'static std::ffi::CStr {
    c"ImageBitmapRenderingContext"
  }
}

#[op2]
impl ImageBitmapRenderingContext {
  #[getter]
  #[global]
  fn canvas(&self) -> v8::Global<v8::Object> {
    self.canvas.clone()
  }

  fn transfer_from_image_bitmap(
    &self,
    #[webidl] bitmap: Nullable<Ref<ImageBitmap>>,
  ) -> Result<(), JsErrorBox> {
    if let Some(bitmap) = bitmap.into_option() {
      if bitmap.detached.get().is_some() {
        return Err(JsErrorBox::new(
          "DOMExceptionInvalidStateError",
          "The provided bitmap is detached.",
        ));
      }

      // the spec states to set ImageBitmapRenderingContext's bitmap to the same at ImageBitmap's without copy,
      // and then it detaches and clears it. So we move it instead and then detach it.
      // Maybe storing it as an RefCell<Rc> might be necessary at some point
      // but that case hasnt been hit yet

      let _ = bitmap.detached.set(());
      let new_data =
        bitmap
          .data
          .replace(DynamicImage::new(0, 0, image::ColorType::Rgba8));

      match &self.data {
        ContextData::Canvas(image) => {
          *image.borrow_mut() = new_data;
        }
        ContextData::Surface(surface_data) => {
          let surface = surface_data.borrow().id;
          let SurfaceBitmap {
            instance,
            device,
            queue,
            render_pipeline,
            vertex_buffer,
            index_buffer,
            bind_group_layout,
            sampler,
          } = self.surface_only.as_ref().unwrap();

          let (width, height) = new_data.dimensions();
          let size = wgpu_types::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
          };
          let texture_desc = wgpu_core::resource::TextureDescriptor {
            label: None,
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu_types::TextureDimension::D2,
            format: wgpu_types::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu_types::TextureUsages::TEXTURE_BINDING
              | wgpu_types::TextureUsages::COPY_DST,
            view_formats: vec![],
          };

          let (texture, err) =
            instance.device_create_texture(*device, &texture_desc, None);
          maybe_err_to_err(err)?;

          instance
            .queue_write_texture(
              *queue,
              &wgpu_types::TexelCopyTextureInfo {
                texture,
                mip_level: 0,
                origin: wgpu_types::Origin3d::ZERO,
                aspect: wgpu_types::TextureAspect::All,
              },
              &new_data.to_rgba8(),
              &wgpu_types::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * width),
                rows_per_image: Some(height),
              },
              &size,
            )
            .map_err(|e| JsErrorBox::from_err(GPUError::from(e)))?;

          let (view, err) = instance.texture_create_view(
            texture,
            &wgpu_core::resource::TextureViewDescriptor::default(),
            None,
          );
          maybe_err_to_err(err)?;

          let (bind_group, err) = instance.device_create_bind_group(
            *device,
            &wgpu_core::binding_model::BindGroupDescriptor {
              label: None,
              layout: *bind_group_layout,
              entries:
                vec![
                  wgpu_core::binding_model::BindGroupEntry {
                    binding: 0,
                    resource:
                      wgpu_core::binding_model::BindingResource::TextureView(
                        view,
                      ),
                  },
                  wgpu_core::binding_model::BindGroupEntry {
                    binding: 1,
                    resource:
                      wgpu_core::binding_model::BindingResource::Sampler(
                        *sampler,
                      ),
                  },
                ]
                .into(),
            },
            None,
          );
          maybe_err_to_err(err)?;

          let surface_output = instance
            .surface_get_current_texture(surface, None)
            .map_err(|e| JsErrorBox::from_err(GPUError::from(e)))?;
          let Some(frame) = surface_output.texture else {
            return Ok(());
          };

          let (command_encoder, err) = instance.device_create_command_encoder(
            *device,
            &wgpu_types::CommandEncoderDescriptor { label: None },
            None,
          );
          maybe_err_to_err(err)?;

          let (swap_view_id, err) = instance.texture_create_view(
            frame,
            &wgpu_core::resource::TextureViewDescriptor::default(),
            None,
          );
          maybe_err_to_err(err)?;

          let desc_with_view = wgpu_core::command::RenderPassDescriptor {
            label: None,
            color_attachments: vec![Some(
              wgpu_core::command::RenderPassColorAttachment {
                view: swap_view_id,
                depth_slice: None,
                resolve_target: None,
                load_op: wgpu_types::LoadOp::Clear(wgpu_types::Color::BLACK),
                store_op: Default::default(),
              },
            )]
            .into(),
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
          };

          {
            let (mut pass, err) = instance.command_encoder_begin_render_pass(
              command_encoder,
              &desc_with_view,
            );
            maybe_err_to_err(err)?;

            instance
              .render_pass_set_pipeline(&mut pass, *render_pipeline)
              .map_err(|e| JsErrorBox::from_err(GPUError::from(e)))?;
            instance
              .render_pass_set_bind_group(&mut pass, 0, Some(bind_group), &[])
              .map_err(|e| JsErrorBox::from_err(GPUError::from(e)))?;
            instance
              .render_pass_set_vertex_buffer(
                &mut pass,
                0,
                *vertex_buffer,
                0,
                std::num::NonZeroU64::new(size_of_val(VERTICES) as _),
              )
              .map_err(|e| JsErrorBox::from_err(GPUError::from(e)))?;
            instance
              .render_pass_set_index_buffer(
                &mut pass,
                *index_buffer,
                wgpu_types::IndexFormat::Uint16,
                0,
                std::num::NonZeroU64::new(size_of_val(INDICES) as _),
              )
              .map_err(|e| JsErrorBox::from_err(GPUError::from(e)))?;
            instance
              .render_pass_draw_indexed(&mut pass, 6, 1, 0, 0, 0)
              .map_err(|e| JsErrorBox::from_err(GPUError::from(e)))?;

            instance
              .render_pass_end(&mut pass)
              .map_err(|e| JsErrorBox::from_err(GPUError::from(e)))?;
          }

          let (command_buffer, err) = instance.command_encoder_finish(
            command_encoder,
            &wgpu_types::CommandBufferDescriptor { label: None },
            None,
          );
          if let Some((_, err)) = err {
            maybe_err_to_err(Some(err))?;
          }

          instance
            .queue_submit(*queue, &[command_buffer])
            .map_err(|(_, e)| JsErrorBox::from_err(GPUError::from(e)))?;

          instance.texture_view_drop(swap_view_id).unwrap();
          instance.command_encoder_drop(command_encoder);
          instance.bind_group_drop(bind_group);
          instance.texture_view_drop(view).unwrap();
          instance.texture_drop(texture);
        }
      }
    } else {
      match &self.data {
        ContextData::Canvas(image) => {
          image.replace_with(|image| {
            let (width, height) = image.dimensions();
            DynamicImage::new(width, height, image::ColorType::Rgba8)
          });
        }
        ContextData::Surface(surface_data) => {
          let surface = surface_data.borrow().id;
          let SurfaceBitmap {
            instance,
            device,
            queue,
            ..
          } = self.surface_only.as_ref().unwrap();

          let surface_output = instance
            .surface_get_current_texture(surface, None)
            .map_err(|e| JsErrorBox::from_err(GPUError::from(e)))?;
          let Some(frame) = surface_output.texture else {
            return Ok(());
          };

          let (command_encoder, err) = instance.device_create_command_encoder(
            *device,
            &wgpu_types::CommandEncoderDescriptor { label: None },
            None,
          );
          maybe_err_to_err(err)?;

          let (swap_view_id, err) = instance.texture_create_view(
            frame,
            &wgpu_core::resource::TextureViewDescriptor::default(),
            None,
          );
          maybe_err_to_err(err)?;

          {
            let (mut pass, err) = instance.command_encoder_begin_render_pass(
              command_encoder,
              &wgpu_core::command::RenderPassDescriptor {
                label: None,
                color_attachments: vec![Some(
                  wgpu_core::command::RenderPassColorAttachment {
                    view: swap_view_id,
                    depth_slice: None,
                    resolve_target: None,
                    load_op: wgpu_types::LoadOp::Clear(
                      wgpu_types::Color::BLACK,
                    ),
                    store_op: Default::default(),
                  },
                )]
                .into(),
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
              },
            );
            maybe_err_to_err(err)?;

            instance
              .render_pass_end(&mut pass)
              .map_err(|e| JsErrorBox::from_err(GPUError::from(e)))?;
          }

          let (command_buffer, err) = instance.command_encoder_finish(
            command_encoder,
            &wgpu_types::CommandBufferDescriptor { label: None },
            None,
          );
          if let Some((_, err)) = err {
            maybe_err_to_err(Some(err))?;
          }

          instance
            .queue_submit(*queue, &[command_buffer])
            .map_err(|(_, e)| JsErrorBox::from_err(GPUError::from(e)))?;

          instance.texture_view_drop(swap_view_id).unwrap();
          instance.command_encoder_drop(command_encoder);
        }
      }
    }

    Ok(())
  }
}

impl ImageBitmapRenderingContext {
  pub fn resize(&self, scope: &mut v8::PinScope<'_, '_>) {
    let SurfaceBitmap {
      instance,
      device,
      queue,
      ..
    } = self.surface_only.as_ref().unwrap();

    todo!()
  }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
  position: [f32; 3],
  tex_coords: [f32; 2],
}

const VERTICES: &[Vertex] = &[
  Vertex {
    position: [-1.0, 1.0, 0.0],
    tex_coords: [0.0, 0.0],
  }, // Top Left
  Vertex {
    position: [-1.0, -1.0, 0.0],
    tex_coords: [0.0, 1.0],
  }, // Bottom Left
  Vertex {
    position: [1.0, -1.0, 0.0],
    tex_coords: [1.0, 1.0],
  }, // Bottom Right
  Vertex {
    position: [1.0, 1.0, 0.0],
    tex_coords: [1.0, 0.0],
  }, // Top Right
];

const INDICES: &[u16] = &[0, 1, 2, 0, 2, 3];

pub const CONTEXT_ID: &str = "bitmaprenderer";

pub fn create<'s>(
  instance: Option<deno_webgpu::Instance>,
  canvas: v8::Global<v8::Object>,
  data: ContextData,
  scope: &mut v8::PinScope<'s, '_>,
  options: v8::Local<'s, v8::Value>,
  prefix: &'static str,
  context: &'static str,
) -> Result<v8::Global<v8::Value>, JsErrorBox> {
  let settings = ImageBitmapRenderingContextSettings::convert(
    scope,
    options,
    prefix.into(),
    (|| context.into()).into(),
    &(),
  )
  .map_err(JsErrorBox::from_err)?;

  let surface_only = if let ContextData::Surface(surface_data) = &data {
    let deno_webgpu::canvas::SurfaceData { id, width, height } =
      &*surface_data.borrow();
    let instance = instance.unwrap();
    let backends = std::env::var("DENO_WEBGPU_BACKEND").map_or_else(
      |_| wgpu_types::Backends::all(),
      |s| wgpu_types::Backends::from_comma_list(&s),
    );
    let adapter = instance
      .request_adapter(
        &wgpu_core::instance::RequestAdapterOptions {
          power_preference: Default::default(),
          force_fallback_adapter: false,
          compatible_surface: Some(*id),
        },
        backends,
        None,
      )
      .unwrap();

    let (device, queue) = instance
      .adapter_request_device(
        adapter,
        &wgpu_core::device::DeviceDescriptor {
          label: None,
          required_features: Default::default(),
          required_limits: Default::default(),
          experimental_features: Default::default(),
          memory_hints: Default::default(),
          trace: Default::default(),
        },
        None,
        None,
      )
      .unwrap();

    let caps = instance.surface_get_capabilities(*id, adapter).unwrap();
    let format = caps.formats[0];

    let config = wgpu_types::SurfaceConfiguration {
      usage: wgpu_types::TextureUsages::RENDER_ATTACHMENT,
      format,
      width: *width,
      height: *height,
      present_mode: wgpu_types::PresentMode::Fifo,
      desired_maximum_frame_latency: 0,
      alpha_mode: wgpu_types::CompositeAlphaMode::Opaque,
      view_formats: vec![],
    };
    let err = instance.surface_configure(*id, device, &config);
    maybe_err_to_err(err)?;

    let (sampler, err) = instance.device_create_sampler(
      device,
      &wgpu_core::resource::SamplerDescriptor {
        label: None,
        address_modes: [
          wgpu_types::AddressMode::ClampToEdge,
          wgpu_types::AddressMode::ClampToEdge,
          wgpu_types::AddressMode::ClampToEdge,
        ],
        mag_filter: wgpu_types::FilterMode::Linear,
        min_filter: wgpu_types::FilterMode::Nearest,
        mipmap_filter: wgpu_types::MipmapFilterMode::Nearest,
        lod_min_clamp: 0.0,
        lod_max_clamp: 0.0,
        compare: None,
        anisotropy_clamp: 0,
        border_color: None,
      },
      None,
    );
    maybe_err_to_err(err)?;

    let (bind_group_layout, err) = instance.device_create_bind_group_layout(
      device,
      &wgpu_core::binding_model::BindGroupLayoutDescriptor {
        label: None,
        entries: vec![
          wgpu_types::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu_types::ShaderStages::FRAGMENT,
            ty: wgpu_types::BindingType::Texture {
              multisampled: false,
              view_dimension: wgpu_types::TextureViewDimension::D2,
              sample_type: wgpu_types::TextureSampleType::Float {
                filterable: true,
              },
            },
            count: None,
          },
          wgpu_types::BindGroupLayoutEntry {
            binding: 1,
            visibility: wgpu_types::ShaderStages::FRAGMENT,
            ty: wgpu_types::BindingType::Sampler(
              wgpu_types::SamplerBindingType::Filtering,
            ),
            count: None,
          },
        ]
        .into(),
      },
      None,
    );
    maybe_err_to_err(err)?;

    let (shader, err) = instance.device_create_shader_module(
      device,
      &wgpu_core::pipeline::ShaderModuleDescriptor {
        label: None,
        runtime_checks: Default::default(),
      },
      wgpu_core::pipeline::ShaderModuleSource::Wgsl(SHADER.into()),
      None,
    );
    maybe_err_to_err(err)?;

    let (render_pipeline_layout, err) = instance.device_create_pipeline_layout(
      device,
      &wgpu_core::binding_model::PipelineLayoutDescriptor {
        label: None,
        bind_group_layouts: vec![bind_group_layout].into(),
        immediate_size: 0,
      },
      None,
    );
    maybe_err_to_err(err)?;

    let (render_pipeline, err) = instance.device_create_render_pipeline(
      device,
      &wgpu_core::pipeline::RenderPipelineDescriptor {
        label: None,
        layout: Some(render_pipeline_layout),
        vertex: wgpu_core::pipeline::VertexState {
          stage: wgpu_core::pipeline::ProgrammableStageDescriptor {
            module: shader,
            entry_point: Some("vs_main".into()),
            constants: Default::default(),
            zero_initialize_workgroup_memory: false,
          },
          buffers: vec![wgpu_core::pipeline::VertexBufferLayout {
            array_stride: size_of::<Vertex>() as _,
            step_mode: wgpu_types::VertexStepMode::Vertex,
            attributes: vec![
              wgpu_types::VertexAttribute {
                offset: 0,
                shader_location: 0,
                format: wgpu_types::VertexFormat::Float32x3,
              },
              wgpu_types::VertexAttribute {
                offset: size_of::<[f32; 3]>() as _,
                shader_location: 1,
                format: wgpu_types::VertexFormat::Float32x2,
              },
            ]
            .into(),
          }]
          .into(),
        },
        fragment: Some(wgpu_core::pipeline::FragmentState {
          stage: wgpu_core::pipeline::ProgrammableStageDescriptor {
            module: shader,
            entry_point: Some("fs_main".into()),
            constants: Default::default(),
            zero_initialize_workgroup_memory: false,
          },
          targets: vec![Some(wgpu_types::ColorTargetState {
            format: config.format,
            blend: Some(wgpu_types::BlendState::REPLACE),
            write_mask: wgpu_types::ColorWrites::ALL,
          })]
          .into(),
        }),
        multiview_mask: None,
        primitive: wgpu_types::PrimitiveState {
          topology: wgpu_types::PrimitiveTopology::TriangleList,
          strip_index_format: None,
          front_face: wgpu_types::FrontFace::Ccw,
          cull_mode: Some(wgpu_types::Face::Back),
          polygon_mode: wgpu_types::PolygonMode::Fill,
          unclipped_depth: false,
          conservative: false,
        },
        depth_stencil: None,
        multisample: wgpu_types::MultisampleState {
          count: 1,
          mask: !0,
          alpha_to_coverage_enabled: false,
        },
        cache: None,
      },
      None,
    );
    maybe_err_to_err(err)?;

    let vertex_buffer_data = bytemuck::cast_slice(VERTICES);
    let (vertex_buffer, err) = instance.device_create_buffer(
      device,
      &wgpu_core::resource::BufferDescriptor {
        label: None,
        usage: wgpu_types::BufferUsages::VERTEX
          | wgpu_types::BufferUsages::COPY_DST,
        size: {
          let unpadded_size =
            vertex_buffer_data.len() as wgpu_types::BufferAddress;
          let align_mask = wgpu_types::COPY_BUFFER_ALIGNMENT - 1;
          let padded_size = ((unpadded_size + align_mask) & !align_mask)
            .max(wgpu_types::COPY_BUFFER_ALIGNMENT);
          padded_size
        },
        mapped_at_creation: true,
      },
      None,
    );
    maybe_err_to_err(err)?;

    let (range_ptr, len) = instance
      .buffer_get_mapped_range(
        vertex_buffer,
        0,
        Some(size_of_val(VERTICES) as _),
      )
      .unwrap();
    unsafe {
      let slice = std::slice::from_raw_parts_mut(range_ptr.as_ptr(), len as _);
      slice.copy_from_slice(vertex_buffer_data);
    }
    instance.buffer_unmap(vertex_buffer).unwrap();

    let index_buffer_data = bytemuck::cast_slice(INDICES);
    let (index_buffer, err) = instance.device_create_buffer(
      device,
      &wgpu_core::resource::BufferDescriptor {
        label: None,
        usage: wgpu_types::BufferUsages::INDEX
          | wgpu_types::BufferUsages::COPY_DST,
        size: {
          let unpadded_size =
            index_buffer_data.len() as wgpu_types::BufferAddress;
          let align_mask = wgpu_types::COPY_BUFFER_ALIGNMENT - 1;
          let padded_size = ((unpadded_size + align_mask) & !align_mask)
            .max(wgpu_types::COPY_BUFFER_ALIGNMENT);
          padded_size
        },
        mapped_at_creation: true,
      },
      None,
    );
    maybe_err_to_err(err)?;

    let (range_ptr, len) = instance
      .buffer_get_mapped_range(index_buffer, 0, Some(size_of_val(INDICES) as _))
      .unwrap();
    unsafe {
      let slice = std::slice::from_raw_parts_mut(range_ptr.as_ptr(), len as _);
      slice.copy_from_slice(index_buffer_data);
    }
    instance.buffer_unmap(index_buffer).unwrap();

    Some(SurfaceBitmap {
      instance,
      device,
      queue,
      render_pipeline,
      vertex_buffer,
      index_buffer,
      bind_group_layout,
      sampler,
    })
  } else {
    None
  };

  let obj = deno_core::cppgc::make_cppgc_object(
    scope,
    ImageBitmapRenderingContext {
      alpha: settings.alpha,
      canvas,
      data,
      surface_only,
    },
  );
  Ok(v8::Global::new(scope, obj.cast()))
}

#[derive(WebIDL)]
#[webidl(dictionary)]
struct ImageBitmapRenderingContextSettings {
  #[webidl(default = true)]
  alpha: bool,
}

const SHADER: &str = r#"
// Vertex Shader

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) tex_coords: vec2<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
};

@vertex
fn vs_main(model: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.tex_coords = model.tex_coords;
    out.clip_position = vec4<f32>(model.position, 1.0);
    return out;
}

// Fragment Shader

@group(0) @binding(0)
var t_diffuse: texture_2d<f32>;
@group(0) @binding(1)
var s_diffuse: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return textureSample(t_diffuse, s_diffuse, in.tex_coords);
}
"#;

fn maybe_err_to_err<E>(err: Option<E>) -> Result<(), JsErrorBox>
where
  GPUError: From<E>,
{
  if let Some(err) = err {
    Err(JsErrorBox::from_err(GPUError::from(err)))
  } else {
    Ok(())
  }
}
