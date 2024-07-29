// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

// @ts-check
/// <reference path="../../core/lib.deno_core.d.ts" />
/// <reference path="../web/internal.d.ts" />
/// <reference path="../web/lib.deno_web.d.ts" />
/// <reference path="./lib.deno_webgpu.d.ts" />

import { core, primordials } from "ext:core/mod.js";
const {
  isDataView,
  isTypedArray,
} = core;
import {
  op_webgpu_buffer_get_map_async,
  op_webgpu_buffer_get_mapped_range,
  op_webgpu_buffer_unmap,
  op_webgpu_command_encoder_begin_compute_pass,
  op_webgpu_command_encoder_begin_render_pass,
  op_webgpu_command_encoder_clear_buffer,
  op_webgpu_command_encoder_copy_buffer_to_buffer,
  op_webgpu_command_encoder_copy_buffer_to_texture,
  op_webgpu_command_encoder_copy_texture_to_buffer,
  op_webgpu_command_encoder_copy_texture_to_texture,
  op_webgpu_command_encoder_finish,
  op_webgpu_command_encoder_insert_debug_marker,
  op_webgpu_command_encoder_pop_debug_group,
  op_webgpu_command_encoder_push_debug_group,
  op_webgpu_command_encoder_resolve_query_set,
  op_webgpu_command_encoder_write_timestamp,
  op_webgpu_compute_pass_dispatch_workgroups,
  op_webgpu_compute_pass_dispatch_workgroups_indirect,
  op_webgpu_compute_pass_end,
  op_webgpu_compute_pass_insert_debug_marker,
  op_webgpu_compute_pass_pop_debug_group,
  op_webgpu_compute_pass_push_debug_group,
  op_webgpu_compute_pass_set_bind_group,
  op_webgpu_compute_pass_set_pipeline,
  op_webgpu_compute_pipeline_get_bind_group_layout,
  op_webgpu_create_bind_group,
  op_webgpu_create_bind_group_layout,
  op_webgpu_create_buffer,
  op_webgpu_create_command_encoder,
  op_webgpu_create_compute_pipeline,
  op_webgpu_create_pipeline_layout,
  op_webgpu_create_query_set,
  op_webgpu_create_render_bundle_encoder,
  op_webgpu_create_render_pipeline,
  op_webgpu_create_sampler,
  op_webgpu_create_shader_module,
  op_webgpu_create_texture,
  op_webgpu_create_texture_view,
  op_webgpu_queue_submit,
  op_webgpu_render_bundle_encoder_draw,
  op_webgpu_render_bundle_encoder_draw_indexed,
  op_webgpu_render_bundle_encoder_draw_indirect,
  op_webgpu_render_bundle_encoder_finish,
  op_webgpu_render_bundle_encoder_insert_debug_marker,
  op_webgpu_render_bundle_encoder_pop_debug_group,
  op_webgpu_render_bundle_encoder_push_debug_group,
  op_webgpu_render_bundle_encoder_set_bind_group,
  op_webgpu_render_bundle_encoder_set_index_buffer,
  op_webgpu_render_bundle_encoder_set_pipeline,
  op_webgpu_render_bundle_encoder_set_vertex_buffer,
  op_webgpu_render_pass_begin_occlusion_query,
  op_webgpu_render_pass_draw,
  op_webgpu_render_pass_draw_indexed,
  op_webgpu_render_pass_draw_indexed_indirect,
  op_webgpu_render_pass_draw_indirect,
  op_webgpu_render_pass_end,
  op_webgpu_render_pass_end_occlusion_query,
  op_webgpu_render_pass_execute_bundles,
  op_webgpu_render_pass_insert_debug_marker,
  op_webgpu_render_pass_pop_debug_group,
  op_webgpu_render_pass_push_debug_group,
  op_webgpu_render_pass_set_bind_group,
  op_webgpu_render_pass_set_blend_constant,
  op_webgpu_render_pass_set_index_buffer,
  op_webgpu_render_pass_set_pipeline,
  op_webgpu_render_pass_set_scissor_rect,
  op_webgpu_render_pass_set_stencil_reference,
  op_webgpu_render_pass_set_vertex_buffer,
  op_webgpu_render_pass_set_viewport,
  op_webgpu_render_pipeline_get_bind_group_layout,
  op_webgpu_request_adapter,
  op_webgpu_request_adapter_info,
  op_webgpu_request_device,
  op_webgpu_write_buffer,
  op_webgpu_write_texture,
} from "ext:core/ops";
const {
  ArrayBuffer,
  ArrayBufferPrototypeGetByteLength,
  ArrayIsArray,
  ArrayPrototypeFindLast,
  ArrayPrototypeMap,
  ArrayPrototypePop,
  ArrayPrototypePush,
  DataViewPrototypeGetBuffer,
  Error,
  MathMax,
  ObjectDefineProperty,
  ObjectHasOwn,
  ObjectPrototypeIsPrototypeOf,
  Promise,
  PromiseReject,
  PromiseResolve,
  SafeArrayIterator,
  SafeSet,
  SafeWeakRef,
  SetPrototypeHas,
  Symbol,
  SymbolFor,
  SymbolIterator,
  TypeError,
  TypedArrayPrototypeGetBuffer,
  TypedArrayPrototypeGetSymbolToStringTag,
  Uint32Array,
  Uint8Array,
} = primordials;

import * as webidl from "ext:deno_webidl/00_webidl.js";
import {
  defineEventHandler,
  Event,
  EventTarget,
  setEventTargetData,
} from "ext:deno_web/02_event.js";
import { DOMException } from "ext:deno_web/01_dom_exception.js";
import { createFilteredInspectProxy } from "ext:deno_console/01_console.js";

const _rid = Symbol("[[rid]]");
const _size = Symbol("[[size]]");
const _usage = Symbol("[[usage]]");
const _state = Symbol("[[state]]");
const _mappingRange = Symbol("[[mapping_range]]");
const _mappedRanges = Symbol("[[mapped_ranges]]");
const _mapMode = Symbol("[[map_mode]]");
const _adapter = Symbol("[[adapter]]");
const _cleanup = Symbol("[[cleanup]]");
const _vendor = Symbol("[[vendor]]");
const _architecture = Symbol("[[architecture]]");
const _description = Symbol("[[description]]");
const _limits = Symbol("[[limits]]");
const _reason = Symbol("[[reason]]");
const _message = Symbol("[[message]]");
const _label = Symbol("[[label]]");
const _device = Symbol("[[device]]");
const _queue = Symbol("[[queue]]");
const _views = Symbol("[[views]]");
const _texture = Symbol("[[texture]]");
const _encoders = Symbol("[[encoders]]");
const _encoder = Symbol("[[encoder]]");
const _descriptor = Symbol("[[descriptor]]");
const _width = Symbol("[[width]]");
const _height = Symbol("[[height]]");
const _depthOrArrayLayers = Symbol("[[depthOrArrayLayers]]");
const _mipLevelCount = Symbol("[[mipLevelCount]]");
const _sampleCount = Symbol("[[sampleCount]]");
const _dimension = Symbol("[[dimension]]");
const _format = Symbol("[[format]]");
const _type = Symbol("[[type]]");
const _count = Symbol("[[count]]");

/**
 * @param {any} self
 * @param {string} prefix
 * @param {string} context
 * @returns {InnerGPUDevice & {rid: number}}
 */
function assertDevice(self, prefix, context) {
  const device = self[_device];
  const deviceRid = device?.rid;
  if (deviceRid === undefined) {
    throw new DOMException(
      `${prefix}: ${context} references an invalid or destroyed device.`,
      "OperationError",
    );
  }
  return device;
}

/**
 * @param {InnerGPUDevice} self
 * @param {any} resource
 * @param {{prefix: string, resourceContext: string, selfContext: string}} opts
 * @returns {InnerGPUDevice & {rid: number}}
 */
function assertDeviceMatch(
  self,
  resource,
  { prefix, resourceContext, selfContext },
) {
  const resourceDevice = assertDevice(resource, prefix, resourceContext);
  if (resourceDevice.rid !== self.rid) {
    throw new DOMException(
      `${prefix}: ${resourceContext} belongs to a different device than ${selfContext}.`,
      "OperationError",
    );
  }
  return { ...resourceDevice, rid: resourceDevice.rid };
}

/**
 * @param {any} self
 * @param {string} prefix
 * @param {string} context
 * @returns {number}
 */
function assertResource(self, prefix, context) {
  const rid = self[_rid];
  if (rid === undefined) {
    throw new DOMException(
      `${prefix}: ${context} an invalid or destroyed resource.`,
      "OperationError",
    );
  }
  return rid;
}

/**
 * @param {number[] | GPUExtent3DDict} data
 * @returns {GPUExtent3DDict}
 */
function normalizeGPUExtent3D(data) {
  if (ArrayIsArray(data)) {
    return {
      width: data[0],
      height: data[1] ?? 1,
      depthOrArrayLayers: data[2] ?? 1,
    };
  } else {
    return {
      width: data.width,
      height: data.height ?? 1,
      depthOrArrayLayers: data.depthOrArrayLayers ?? 1,
    };
  }
}

/**
 * @param {number[] | GPUOrigin3DDict} data
 * @returns {GPUOrigin3DDict}
 */
function normalizeGPUOrigin3D(data) {
  if (ArrayIsArray(data)) {
    return {
      x: data[0],
      y: data[1],
      z: data[2],
    };
  } else {
    return data;
  }
}

/**
 * @param {number[] | GPUColor} data
 * @returns {GPUColor}
 */
function normalizeGPUColor(data) {
  if (ArrayIsArray(data)) {
    return {
      r: data[0],
      g: data[1],
      b: data[2],
      a: data[3],
    };
  } else {
    return data;
  }
}

const illegalConstructorKey = Symbol("illegalConstructorKey");
class GPUError extends Error {
  constructor(key = null) {
    super();
    if (key !== illegalConstructorKey) {
      webidl.illegalConstructor();
    }
  }

  [_message];
  get message() {
    webidl.assertBranded(this, GPUErrorPrototype);
    return this[_message];
  }
}
const GPUErrorPrototype = GPUError.prototype;

class GPUValidationError extends GPUError {
  name = "GPUValidationError";
  /** @param {string} message */
  constructor(message) {
    const prefix = "Failed to construct 'GPUValidationError'";
    webidl.requiredArguments(arguments.length, 1, prefix);
    message = webidl.converters.DOMString(message, prefix, "Argument 1");
    super(illegalConstructorKey);
    this[webidl.brand] = webidl.brand;
    this[_message] = message;
  }
}

class GPUOutOfMemoryError extends GPUError {
  name = "GPUOutOfMemoryError";
  constructor(message) {
    const prefix = "Failed to construct 'GPUOutOfMemoryError'";
    webidl.requiredArguments(arguments.length, 1, prefix);
    message = webidl.converters.DOMString(message, prefix, "Argument 1");
    super(illegalConstructorKey);
    this[webidl.brand] = webidl.brand;
    this[_message] = message;
  }
}

class GPUInternalError extends GPUError {
  name = "GPUInternalError";
  constructor() {
    super(illegalConstructorKey);
    this[webidl.brand] = webidl.brand;
  }
}

class GPUUncapturedErrorEvent extends Event {
  #error;

  constructor(type, gpuUncapturedErrorEventInitDict) {
    super(type, gpuUncapturedErrorEventInitDict);
    this[webidl.brand] = webidl.brand;

    const prefix = "Failed to construct 'GPUUncapturedErrorEvent'";
    webidl.requiredArguments(arguments.length, 2, prefix);
    gpuUncapturedErrorEventInitDict = webidl.converters
      .GPUUncapturedErrorEventInit(
        gpuUncapturedErrorEventInitDict,
        prefix,
        "Argument 2",
      );

    this.#error = gpuUncapturedErrorEventInitDict.error;
  }

  get error() {
    webidl.assertBranded(this, GPUUncapturedErrorEventPrototype);
    return this.#error;
  }
}
const GPUUncapturedErrorEventPrototype = GPUUncapturedErrorEvent.prototype;

class GPU {
  [webidl.brand] = webidl.brand;

  constructor() {
    webidl.illegalConstructor();
  }

  /**
   * @param {GPURequestAdapterOptions} options
   */
  // deno-lint-ignore require-await
  async requestAdapter(options = { __proto__: null }) {
    webidl.assertBranded(this, GPUPrototype);
    options = webidl.converters.GPURequestAdapterOptions(
      options,
      "Failed to execute 'requestAdapter' on 'GPU'",
      "Argument 1",
    );

    const { err, ...data } = op_webgpu_request_adapter(
      options.powerPreference,
      options.forceFallbackAdapter,
    );

    if (err) {
      return null;
    } else {
      return createGPUAdapter(data);
    }
  }

  getPreferredCanvasFormat() {
    // Same as Gecko.
    //
    // https://github.com/mozilla/gecko-dev/blob/b75080bb8b11844d18cb5f9ac6e68a866ef8e243/dom/webgpu/Instance.h#L42-L47
    if (core.build.os == "android") {
      return "rgba8unorm";
    }
    return "bgra8unorm";
  }

  [SymbolFor("Deno.privateCustomInspect")](inspect, inspectOptions) {
    return `${this.constructor.name} ${inspect({}, inspectOptions)}`;
  }
}
const GPUPrototype = GPU.prototype;

/**
 * @typedef InnerGPUAdapter
 * @property {number} rid
 * @property {GPUSupportedFeatures} features
 * @property {GPUSupportedLimits} limits
 * @property {boolean} isFallbackAdapter
 */

/**
 * @param {InnerGPUAdapter} inner
 * @returns {GPUAdapter}
 */
function createGPUAdapter(inner) {
  /** @type {GPUAdapter} */
  const adapter = webidl.createBranded(GPUAdapter);
  adapter[_adapter] = {
    ...inner,
    features: createGPUSupportedFeatures(inner.features),
    limits: createGPUSupportedLimits(inner.limits),
  };
  return adapter;
}

const _invalid = Symbol("[[invalid]]");
class GPUAdapter {
  /** @type {InnerGPUAdapter} */
  [_adapter];
  /** @type {bool} */
  [_invalid];

  /** @returns {GPUSupportedFeatures} */
  get features() {
    webidl.assertBranded(this, GPUAdapterPrototype);
    return this[_adapter].features;
  }
  /** @returns {GPUSupportedLimits} */
  get limits() {
    webidl.assertBranded(this, GPUAdapterPrototype);
    return this[_adapter].limits;
  }
  /** @returns {boolean} */
  get isFallbackAdapter() {
    webidl.assertBranded(this, GPUAdapterPrototype);
    return this[_adapter].isFallbackAdapter;
  }

  constructor() {
    webidl.illegalConstructor();
  }

  /**
   * @param {GPUDeviceDescriptor} descriptor
   * @returns {Promise<GPUDevice>}
   */
  // deno-lint-ignore require-await
  async requestDevice(descriptor = { __proto__: null }) {
    webidl.assertBranded(this, GPUAdapterPrototype);
    const prefix = "Failed to execute 'requestDevice' on 'GPUAdapter'";
    descriptor = webidl.converters.GPUDeviceDescriptor(
      descriptor,
      prefix,
      "Argument 1",
    );
    const requiredFeatures = descriptor.requiredFeatures ?? [];
    for (let i = 0; i < requiredFeatures.length; ++i) {
      const feature = requiredFeatures[i];
      if (
        !SetPrototypeHas(this[_adapter].features[webidl.setlikeInner], feature)
      ) {
        throw new TypeError(
          `${prefix}: requiredFeatures must be a subset of the adapter features.`,
        );
      }
    }

    if (this[_invalid]) {
      throw new TypeError(
        "The adapter cannot be reused, as it has been invalidated by a device creation",
      );
    }

    const { rid, queueRid, features, limits } = op_webgpu_request_device(
      this[_adapter].rid,
      descriptor.label,
      requiredFeatures,
      descriptor.requiredLimits,
    );

    this[_invalid] = true;

    const inner = new InnerGPUDevice({
      rid,
      adapter: this,
      features: createGPUSupportedFeatures(features),
      limits: createGPUSupportedLimits(limits),
    });
    const queue = createGPUQueue(descriptor.label, inner, queueRid);
    inner.trackResource(queue);
    const device = createGPUDevice(
      descriptor.label,
      inner,
      queue,
    );
    inner.device = device;
    return device;
  }

  /**
   * @returns {Promise<GPUAdapterInfo>}
   */
  requestAdapterInfo() {
    webidl.assertBranded(this, GPUAdapterPrototype);

    if (this[_invalid]) {
      throw new TypeError(
        "The adapter cannot be reused, as it has been invalidated by a device creation",
      );
    }

    const {
      vendor,
      architecture,
      device,
      description,
    } = op_webgpu_request_adapter_info(this[_adapter].rid);

    const adapterInfo = webidl.createBranded(GPUAdapterInfo);
    adapterInfo[_vendor] = vendor;
    adapterInfo[_architecture] = architecture;
    adapterInfo[_device] = device;
    adapterInfo[_description] = description;
    return PromiseResolve(adapterInfo);
  }

  [SymbolFor("Deno.privateCustomInspect")](inspect, inspectOptions) {
    return inspect(
      createFilteredInspectProxy({
        object: this,
        evaluate: ObjectPrototypeIsPrototypeOf(GPUAdapterPrototype, this),
        keys: [
          "features",
          "limits",
          "isFallbackAdapter",
        ],
      }),
      inspectOptions,
    );
  }
}
const GPUAdapterPrototype = GPUAdapter.prototype;

class GPUAdapterInfo {
  /** @type {string} */
  [_vendor];
  /** @returns {string} */
  get vendor() {
    webidl.assertBranded(this, GPUAdapterInfoPrototype);
    return this[_vendor];
  }

  /** @type {string} */
  [_architecture];
  /** @returns {string} */
  get architecture() {
    webidl.assertBranded(this, GPUAdapterInfoPrototype);
    return this[_architecture];
  }

  /** @type {string} */
  [_device];
  /** @returns {string} */
  get device() {
    webidl.assertBranded(this, GPUAdapterInfoPrototype);
    return this[_device];
  }

  /** @type {string} */
  [_description];
  /** @returns {string} */
  get description() {
    webidl.assertBranded(this, GPUAdapterInfoPrototype);
    return this[_description];
  }

  [SymbolFor("Deno.privateCustomInspect")](inspect, inspectOptions) {
    return inspect(
      createFilteredInspectProxy({
        object: this,
        evaluate: ObjectPrototypeIsPrototypeOf(GPUAdapterInfoPrototype, this),
        keys: [
          "vendor",
          "architecture",
          "device",
          "description",
        ],
      }),
      inspectOptions,
    );
  }
}
const GPUAdapterInfoPrototype = GPUAdapterInfo.prototype;

function createGPUSupportedLimits(limits) {
  /** @type {GPUSupportedLimits} */
  const adapterFeatures = webidl.createBranded(GPUSupportedLimits);
  adapterFeatures[_limits] = limits;
  return adapterFeatures;
}

/**
 * @typedef InnerAdapterLimits
 * @property {number} maxTextureDimension1D
 * @property {number} maxTextureDimension2D
 * @property {number} maxTextureDimension3D
 * @property {number} maxTextureArrayLayers
 * @property {number} maxBindGroups
 * @property {number} maxDynamicUniformBuffersPerPipelineLayout
 * @property {number} maxDynamicStorageBuffersPerPipelineLayout
 * @property {number} maxSampledTexturesPerShaderStage
 * @property {number} maxSamplersPerShaderStage
 * @property {number} maxStorageBuffersPerShaderStage
 * @property {number} maxStorageTexturesPerShaderStage
 * @property {number} maxUniformBuffersPerShaderStage
 * @property {number} maxUniformBufferBindingSize
 * @property {number} maxStorageBufferBindingSize
 * @property {number} minUniformBufferOffsetAlignment
 * @property {number} minStorageBufferOffsetAlignment
 * @property {number} maxVertexBuffers
 * @property {number} maxVertexAttributes
 * @property {number} maxVertexBufferArrayStride
 * @property {number} maxInterStageShaderComponents
 * @property {number} maxComputeWorkgroupStorageSize
 * @property {number} maxComputeInvocationsPerWorkgroup
 * @property {number} maxComputeWorkgroupSizeX
 * @property {number} maxComputeWorkgroupSizeY
 * @property {number} maxComputeWorkgroupSizeZ
 * @property {number} maxComputeWorkgroupsPerDimension
 */

class GPUSupportedLimits {
  /** @type {InnerAdapterLimits} */
  [_limits];
  constructor() {
    webidl.illegalConstructor();
  }

  get maxTextureDimension1D() {
    webidl.assertBranded(this, GPUSupportedLimitsPrototype);
    return this[_limits].maxTextureDimension1D;
  }
  get maxTextureDimension2D() {
    webidl.assertBranded(this, GPUSupportedLimitsPrototype);
    return this[_limits].maxTextureDimension2D;
  }
  get maxTextureDimension3D() {
    webidl.assertBranded(this, GPUSupportedLimitsPrototype);
    return this[_limits].maxTextureDimension3D;
  }
  get maxTextureArrayLayers() {
    webidl.assertBranded(this, GPUSupportedLimitsPrototype);
    return this[_limits].maxTextureArrayLayers;
  }
  get maxBindGroups() {
    webidl.assertBranded(this, GPUSupportedLimitsPrototype);
    return this[_limits].maxBindGroups;
  }
  get maxBindingsPerBindGroup() {
    webidl.assertBranded(this, GPUSupportedLimitsPrototype);
    return this[_limits].maxBindingsPerBindGroup;
  }
  get maxBufferSize() {
    webidl.assertBranded(this, GPUSupportedLimitsPrototype);
    return this[_limits].maxBufferSize;
  }
  get maxDynamicUniformBuffersPerPipelineLayout() {
    webidl.assertBranded(this, GPUSupportedLimitsPrototype);
    return this[_limits].maxDynamicUniformBuffersPerPipelineLayout;
  }
  get maxDynamicStorageBuffersPerPipelineLayout() {
    webidl.assertBranded(this, GPUSupportedLimitsPrototype);
    return this[_limits].maxDynamicStorageBuffersPerPipelineLayout;
  }
  get maxSampledTexturesPerShaderStage() {
    webidl.assertBranded(this, GPUSupportedLimitsPrototype);
    return this[_limits].maxSampledTexturesPerShaderStage;
  }
  get maxSamplersPerShaderStage() {
    webidl.assertBranded(this, GPUSupportedLimitsPrototype);
    return this[_limits].maxSamplersPerShaderStage;
  }
  get maxStorageBuffersPerShaderStage() {
    webidl.assertBranded(this, GPUSupportedLimitsPrototype);
    return this[_limits].maxStorageBuffersPerShaderStage;
  }
  get maxStorageTexturesPerShaderStage() {
    webidl.assertBranded(this, GPUSupportedLimitsPrototype);
    return this[_limits].maxStorageTexturesPerShaderStage;
  }
  get maxUniformBuffersPerShaderStage() {
    webidl.assertBranded(this, GPUSupportedLimitsPrototype);
    return this[_limits].maxUniformBuffersPerShaderStage;
  }
  get maxUniformBufferBindingSize() {
    webidl.assertBranded(this, GPUSupportedLimitsPrototype);
    return this[_limits].maxUniformBufferBindingSize;
  }
  get maxStorageBufferBindingSize() {
    webidl.assertBranded(this, GPUSupportedLimitsPrototype);
    return this[_limits].maxStorageBufferBindingSize;
  }
  get minUniformBufferOffsetAlignment() {
    webidl.assertBranded(this, GPUSupportedLimitsPrototype);
    return this[_limits].minUniformBufferOffsetAlignment;
  }
  get minStorageBufferOffsetAlignment() {
    webidl.assertBranded(this, GPUSupportedLimitsPrototype);
    return this[_limits].minStorageBufferOffsetAlignment;
  }
  get maxVertexBuffers() {
    webidl.assertBranded(this, GPUSupportedLimitsPrototype);
    return this[_limits].maxVertexBuffers;
  }
  get maxVertexAttributes() {
    webidl.assertBranded(this, GPUSupportedLimitsPrototype);
    return this[_limits].maxVertexAttributes;
  }
  get maxVertexBufferArrayStride() {
    webidl.assertBranded(this, GPUSupportedLimitsPrototype);
    return this[_limits].maxVertexBufferArrayStride;
  }
  get maxInterStageShaderComponents() {
    webidl.assertBranded(this, GPUSupportedLimitsPrototype);
    return this[_limits].maxInterStageShaderComponents;
  }
  get maxColorAttachments() {
    webidl.assertBranded(this, GPUSupportedLimitsPrototype);
    return this[_limits].maxColorAttachments;
  }
  get maxColorAttachmentBytesPerSample() {
    webidl.assertBranded(this, GPUSupportedLimitsPrototype);
    return this[_limits].maxColorAttachmentBytesPerSample;
  }
  get maxComputeWorkgroupStorageSize() {
    webidl.assertBranded(this, GPUSupportedLimitsPrototype);
    return this[_limits].maxComputeWorkgroupStorageSize;
  }
  get maxComputeInvocationsPerWorkgroup() {
    webidl.assertBranded(this, GPUSupportedLimitsPrototype);
    return this[_limits].maxComputeInvocationsPerWorkgroup;
  }
  get maxComputeWorkgroupSizeX() {
    webidl.assertBranded(this, GPUSupportedLimitsPrototype);
    return this[_limits].maxComputeWorkgroupSizeX;
  }
  get maxComputeWorkgroupSizeY() {
    webidl.assertBranded(this, GPUSupportedLimitsPrototype);
    return this[_limits].maxComputeWorkgroupSizeY;
  }
  get maxComputeWorkgroupSizeZ() {
    webidl.assertBranded(this, GPUSupportedLimitsPrototype);
    return this[_limits].maxComputeWorkgroupSizeZ;
  }
  get maxComputeWorkgroupsPerDimension() {
    webidl.assertBranded(this, GPUSupportedLimitsPrototype);
    return this[_limits].maxComputeWorkgroupsPerDimension;
  }

  [SymbolFor("Deno.privateCustomInspect")](inspect, inspectOptions) {
    return inspect(
      createFilteredInspectProxy({
        object: this,
        evaluate: ObjectPrototypeIsPrototypeOf(
          GPUSupportedLimitsPrototype,
          this,
        ),
        keys: [
          "maxTextureDimension1D",
          "maxTextureDimension2D",
          "maxTextureDimension3D",
          "maxTextureArrayLayers",
          "maxBindGroups",
          "maxBindingsPerBindGroup",
          "maxBufferSize",
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
          "maxInterStageShaderComponents",
          "maxColorAttachments",
          "maxColorAttachmentBytesPerSample",
          "maxComputeWorkgroupStorageSize",
          "maxComputeInvocationsPerWorkgroup",
          "maxComputeWorkgroupSizeX",
          "maxComputeWorkgroupSizeY",
          "maxComputeWorkgroupSizeZ",
          "maxComputeWorkgroupsPerDimension",
        ],
      }),
      inspectOptions,
    );
  }
}
const GPUSupportedLimitsPrototype = GPUSupportedLimits.prototype;

function createGPUSupportedFeatures(features) {
  /** @type {GPUSupportedFeatures} */
  const supportedFeatures = webidl.createBranded(GPUSupportedFeatures);
  supportedFeatures[webidl.setlikeInner] = new SafeSet(features);
  webidl.setlike(
    supportedFeatures,
    GPUSupportedFeaturesPrototype,
    true,
  );
  return supportedFeatures;
}

class GPUSupportedFeatures {
  constructor() {
    webidl.illegalConstructor();
  }

  [SymbolFor("Deno.privateCustomInspect")](inspect, inspectOptions) {
    if (ObjectPrototypeIsPrototypeOf(GPUSupportedFeaturesPrototype, this)) {
      return `${this.constructor.name} ${
        // deno-lint-ignore prefer-primordials
        inspect([...this], inspectOptions)}`;
    } else {
      return `${this.constructor.name} ${inspect({}, inspectOptions)}`;
    }
  }
}
const GPUSupportedFeaturesPrototype = GPUSupportedFeatures.prototype;

/**
 * @param {string | undefined} reason
 * @param {string} message
 * @returns {GPUDeviceLostInfo}
 */
function createGPUDeviceLostInfo(reason, message) {
  /** @type {GPUDeviceLostInfo} */
  const deviceLostInfo = webidl.createBranded(GPUDeviceLostInfo);
  deviceLostInfo[_reason] = reason ?? "unknown";
  deviceLostInfo[_message] = message;
  return deviceLostInfo;
}

class GPUDeviceLostInfo {
  /** @type {string} */
  [_reason];
  /** @type {string} */
  [_message];

  constructor() {
    webidl.illegalConstructor();
  }

  get reason() {
    webidl.assertBranded(this, GPUDeviceLostInfoPrototype);
    return this[_reason];
  }
  get message() {
    webidl.assertBranded(this, GPUDeviceLostInfoPrototype);
    return this[_message];
  }

  [SymbolFor("Deno.privateCustomInspect")](inspect, inspectOptions) {
    return inspect(
      createFilteredInspectProxy({
        object: this,
        evaluate: ObjectPrototypeIsPrototypeOf(
          GPUDeviceLostInfoPrototype,
          this,
        ),
        keys: [
          "reason",
          "message",
        ],
      }),
      inspectOptions,
    );
  }
}

const GPUDeviceLostInfoPrototype = GPUDeviceLostInfo.prototype;

/**
 * @param {string} name
 * @param {any} type
 */
function GPUObjectBaseMixin(name, type) {
  type.prototype[_label] = null;
  ObjectDefineProperty(type.prototype, "label", {
    /**
     * @return {string | null}
     */
    get() {
      webidl.assertBranded(this, type.prototype);
      return this[_label];
    },
    /**
     * @param {string | null} label
     */
    set(label) {
      webidl.assertBranded(this, type.prototype);
      label = webidl.converters["UVString?"](
        label,
        `Failed to set 'label' on '${name}'`,
        "Argument 1",
      );
      this[_label] = label;
    },
  });
}

/**
 * @typedef ErrorScope
 * @property {string} filter
 * @property {GPUError[]} errors
 */

/**
 * @typedef InnerGPUDeviceOptions
 * @property {GPUAdapter} adapter
 * @property {number | undefined} rid
 * @property {GPUSupportedFeatures} features
 * @property {GPUSupportedLimits} limits
 * @property {GPUDevice} device
 */

class InnerGPUDevice {
  /** @type {GPUAdapter} */
  adapter;
  /** @type {number | undefined} */
  rid;
  /** @type {GPUSupportedFeatures} */
  features;
  /** @type {GPUSupportedLimits} */
  limits;
  /** @type {SafeWeakRef<any>[]} */
  resources;
  /** @type {boolean} */
  isLost;
  /** @type {Promise<GPUDeviceLostInfo>} */
  lost;
  /** @type {(info: GPUDeviceLostInfo) => void} */
  resolveLost;
  /** @type {ErrorScope[]} */
  errorScopeStack;
  /** @type {GPUDevice} */
  device;

  /**
   * @param {InnerGPUDeviceOptions} options
   */
  constructor(options) {
    this.adapter = options.adapter;
    this.rid = options.rid;
    this.features = options.features;
    this.limits = options.limits;
    this.resources = [];
    this.isLost = false;
    this.resolveLost = () => {};
    this.lost = new Promise((resolve) => {
      this.resolveLost = resolve;
    });
    this.errorScopeStack = [];
  }

  /** @param {any} resource */
  trackResource(resource) {
    ArrayPrototypePush(this.resources, new SafeWeakRef(resource));
  }

  // Ref: https://gpuweb.github.io/gpuweb/#abstract-opdef-dispatch-error
  /** @param {{ type: string, value: string | null } | undefined} error */
  pushError(error) {
    if (!error) {
      return;
    }

    let constructedError;
    switch (error.type) {
      case "lost":
        this.isLost = true;
        this.resolveLost(
          createGPUDeviceLostInfo(undefined, "device was lost"),
        );
        return;
      case "validation":
        constructedError = new GPUValidationError(
          error.value ?? "validation error",
        );
        break;
      case "out-of-memory":
        constructedError = new GPUOutOfMemoryError();
        break;
      case "internal":
        constructedError = new GPUInternalError();
        break;
    }

    if (this.isLost) {
      return;
    }

    const scope = ArrayPrototypeFindLast(
      this.errorScopeStack,
      ({ filter }) => filter === error.type,
    );
    if (scope) {
      ArrayPrototypePush(scope.errors, constructedError);
    } else {
      this.device.dispatchEvent(
        new GPUUncapturedErrorEvent("uncapturederror", {
          error: constructedError,
        }),
      );
    }
  }
}

/**
 * @param {string | null} label
 * @param {InnerGPUDevice} inner
 * @param {GPUQueue} queue
 * @returns {GPUDevice}
 */
function createGPUDevice(label, inner, queue) {
  /** @type {GPUDevice} */
  const device = webidl.createBranded(GPUDevice);
  device[_label] = label;
  device[_device] = inner;
  device[_queue] = queue;
  setEventTargetData(device);
  return device;
}

class GPUDevice extends EventTarget {
  /** @type {InnerGPUDevice} */
  [_device];

  /** @type {GPUQueue} */
  [_queue];

  [_cleanup]() {
    const device = this[_device];
    const resources = device.resources;
    while (resources.length > 0) {
      const resource = ArrayPrototypePop(resources)?.deref();
      if (resource) {
        resource[_cleanup]();
      }
    }
    const rid = device.rid;
    if (rid !== undefined) {
      core.close(rid);
      /** @type {number | undefined} */
      device.rid = undefined;
    }
  }

  get features() {
    webidl.assertBranded(this, GPUDevicePrototype);
    return this[_device].features;
  }
  get limits() {
    webidl.assertBranded(this, GPUDevicePrototype);
    return this[_device].limits;
  }
  get queue() {
    webidl.assertBranded(this, GPUDevicePrototype);
    return this[_queue];
  }

  constructor() {
    webidl.illegalConstructor();
    super();
  }

  destroy() {
    webidl.assertBranded(this, GPUDevicePrototype);
    this[_cleanup]();
  }

  /**
   * @param {GPUBufferDescriptor} descriptor
   * @returns {GPUBuffer}
   */
  createBuffer(descriptor) {
    webidl.assertBranded(this, GPUDevicePrototype);
    const prefix = "Failed to execute 'createBuffer' on 'GPUDevice'";
    webidl.requiredArguments(arguments.length, 1, prefix);
    descriptor = webidl.converters.GPUBufferDescriptor(
      descriptor,
      prefix,
      "Argument 1",
    );
    const device = assertDevice(this, prefix, "this");
    const { rid, err } = op_webgpu_create_buffer(
      device.rid,
      descriptor.label,
      descriptor.size,
      descriptor.usage,
      descriptor.mappedAtCreation,
    );
    device.pushError(err);
    /** @type {CreateGPUBufferOptions} */
    let options;
    if (descriptor.mappedAtCreation) {
      options = {
        mapping: new ArrayBuffer(descriptor.size),
        mappingRange: [0, descriptor.size],
        mappedRanges: [],
        state: "mapped at creation",
      };
    } else {
      options = {
        mapping: null,
        mappedRanges: null,
        mappingRange: null,
        state: "unmapped",
      };
    }
    const buffer = createGPUBuffer(
      descriptor.label,
      device,
      rid,
      descriptor.size,
      descriptor.usage,
      options,
    );
    device.trackResource(buffer);
    return buffer;
  }

  /**
   * @param {GPUTextureDescriptor} descriptor
   * @returns {GPUTexture}
   */
  createTexture(descriptor) {
    webidl.assertBranded(this, GPUDevicePrototype);
    const prefix = "Failed to execute 'createTexture' on 'GPUDevice'";
    webidl.requiredArguments(arguments.length, 1, prefix);
    descriptor = webidl.converters.GPUTextureDescriptor(
      descriptor,
      prefix,
      "Argument 1",
    );
    const device = assertDevice(this, prefix, "this");
    // assign normalized size to descriptor due to createGPUTexture needs it
    descriptor.size = normalizeGPUExtent3D(descriptor.size);
    const { rid, err } = op_webgpu_create_texture({
      deviceRid: device.rid,
      ...descriptor,
    });
    device.pushError(err);

    const texture = createGPUTexture(
      descriptor,
      device,
      rid,
    );
    device.trackResource(texture);
    return texture;
  }

  /**
   * @param {GPUSamplerDescriptor} descriptor
   * @returns {GPUSampler}
   */
  createSampler(descriptor = { __proto__: null }) {
    webidl.assertBranded(this, GPUDevicePrototype);
    const prefix = "Failed to execute 'createSampler' on 'GPUDevice'";
    descriptor = webidl.converters.GPUSamplerDescriptor(
      descriptor,
      prefix,
      "Argument 1",
    );
    const device = assertDevice(this, prefix, "this");
    const { rid, err } = op_webgpu_create_sampler({
      deviceRid: device.rid,
      ...descriptor,
    });
    device.pushError(err);

    const sampler = createGPUSampler(
      descriptor.label,
      device,
      rid,
    );
    device.trackResource(sampler);
    return sampler;
  }

  /**
   * @param {GPUBindGroupLayoutDescriptor} descriptor
   * @returns {GPUBindGroupLayout}
   */
  createBindGroupLayout(descriptor) {
    webidl.assertBranded(this, GPUDevicePrototype);
    const prefix = "Failed to execute 'createBindGroupLayout' on 'GPUDevice'";
    webidl.requiredArguments(arguments.length, 1, prefix);
    descriptor = webidl.converters.GPUBindGroupLayoutDescriptor(
      descriptor,
      prefix,
      "Argument 1",
    );
    const device = assertDevice(this, prefix, "this");
    for (let i = 0; i < descriptor.entries.length; ++i) {
      const entry = descriptor.entries[i];

      let j = 0;
      // deno-lint-ignore prefer-primordials
      if (entry.buffer) j++;
      if (entry.sampler) j++;
      if (entry.texture) j++;
      if (entry.storageTexture) j++;

      if (j !== 1) {
        throw new Error(); // TODO(@crowlKats): correct error
      }
    }

    const { rid, err } = op_webgpu_create_bind_group_layout(
      device.rid,
      descriptor.label,
      descriptor.entries,
    );
    device.pushError(err);

    const bindGroupLayout = createGPUBindGroupLayout(
      descriptor.label,
      device,
      rid,
    );
    device.trackResource(bindGroupLayout);
    return bindGroupLayout;
  }

  /**
   * @param {GPUPipelineLayoutDescriptor} descriptor
   * @returns {GPUPipelineLayout}
   */
  createPipelineLayout(descriptor) {
    webidl.assertBranded(this, GPUDevicePrototype);
    const prefix = "Failed to execute 'createPipelineLayout' on 'GPUDevice'";
    webidl.requiredArguments(arguments.length, 1, prefix);
    descriptor = webidl.converters.GPUPipelineLayoutDescriptor(
      descriptor,
      prefix,
      "Argument 1",
    );
    const device = assertDevice(this, prefix, "this");
    const bindGroupLayouts = ArrayPrototypeMap(
      descriptor.bindGroupLayouts,
      (layout, i) => {
        const context = `bind group layout ${i + 1}`;
        const rid = assertResource(layout, prefix, context);
        assertDeviceMatch(device, layout, {
          prefix,
          selfContext: "this",
          resourceContext: context,
        });
        return rid;
      },
    );
    const { rid, err } = op_webgpu_create_pipeline_layout(
      device.rid,
      descriptor.label,
      bindGroupLayouts,
    );
    device.pushError(err);

    const pipelineLayout = createGPUPipelineLayout(
      descriptor.label,
      device,
      rid,
    );
    device.trackResource(pipelineLayout);
    return pipelineLayout;
  }

  /**
   * @param {GPUBindGroupDescriptor} descriptor
   * @returns {GPUBindGroup}
   */
  createBindGroup(descriptor) {
    webidl.assertBranded(this, GPUDevicePrototype);
    const prefix = "Failed to execute 'createBindGroup' on 'GPUDevice'";
    webidl.requiredArguments(arguments.length, 1, prefix);
    descriptor = webidl.converters.GPUBindGroupDescriptor(
      descriptor,
      prefix,
      "Argument 1",
    );
    const device = assertDevice(this, prefix, "this");
    const layout = assertResource(descriptor.layout, prefix, "layout");
    assertDeviceMatch(device, descriptor.layout, {
      prefix,
      resourceContext: "layout",
      selfContext: "this",
    });
    const entries = ArrayPrototypeMap(descriptor.entries, (entry, i) => {
      const context = `entry ${i + 1}`;
      const resource = entry.resource;
      if (ObjectPrototypeIsPrototypeOf(GPUSamplerPrototype, resource)) {
        const rid = assertResource(resource, prefix, context);
        return {
          binding: entry.binding,
          kind: "GPUSampler",
          resource: rid,
        };
      } else if (
        ObjectPrototypeIsPrototypeOf(GPUTextureViewPrototype, resource)
      ) {
        const rid = assertResource(resource, prefix, context);
        assertResource(resource[_texture], prefix, context);
        return {
          binding: entry.binding,
          kind: "GPUTextureView",
          resource: rid,
        };
      } else {
        // deno-lint-ignore prefer-primordials
        const rid = assertResource(resource.buffer, prefix, context);
        return {
          binding: entry.binding,
          kind: "GPUBufferBinding",
          resource: rid,
          offset: entry.resource.offset,
          size: entry.resource.size,
        };
      }
    });

    const { rid, err } = op_webgpu_create_bind_group(
      device.rid,
      descriptor.label,
      layout,
      entries,
    );
    device.pushError(err);

    const bindGroup = createGPUBindGroup(
      descriptor.label,
      device,
      rid,
    );
    device.trackResource(bindGroup);
    return bindGroup;
  }

  /**
   * @param {GPUShaderModuleDescriptor} descriptor
   */
  createShaderModule(descriptor) {
    webidl.assertBranded(this, GPUDevicePrototype);
    const prefix = "Failed to execute 'createShaderModule' on 'GPUDevice'";
    webidl.requiredArguments(arguments.length, 1, prefix);
    descriptor = webidl.converters.GPUShaderModuleDescriptor(
      descriptor,
      prefix,
      "Argument 1",
    );
    const device = assertDevice(this, prefix, "this");
    const { rid, err } = op_webgpu_create_shader_module(
      device.rid,
      descriptor.label,
      descriptor.code,
    );
    device.pushError(err);

    const shaderModule = createGPUShaderModule(
      descriptor.label,
      device,
      rid,
    );
    device.trackResource(shaderModule);
    return shaderModule;
  }

  /**
   * @param {GPUComputePipelineDescriptor} descriptor
   * @returns {GPUComputePipeline}
   */
  createComputePipeline(descriptor) {
    webidl.assertBranded(this, GPUDevicePrototype);
    const prefix = "Failed to execute 'createComputePipeline' on 'GPUDevice'";
    webidl.requiredArguments(arguments.length, 1, prefix);
    descriptor = webidl.converters.GPUComputePipelineDescriptor(
      descriptor,
      prefix,
      "Argument 1",
    );
    const device = assertDevice(this, prefix, "this");
    let layout = descriptor.layout;
    if (typeof descriptor.layout !== "string") {
      const context = "layout";
      layout = assertResource(descriptor.layout, prefix, context);
      assertDeviceMatch(device, descriptor.layout, {
        prefix,
        resourceContext: context,
        selfContext: "this",
      });
    }
    const module = assertResource(
      descriptor.compute.module,
      prefix,
      "compute shader module",
    );
    assertDeviceMatch(device, descriptor.compute.module, {
      prefix,
      resourceContext: "compute shader module",
      selfContext: "this",
    });

    const { rid, err } = op_webgpu_create_compute_pipeline(
      device.rid,
      descriptor.label,
      layout,
      {
        module,
        entryPoint: descriptor.compute.entryPoint,
        constants: descriptor.compute.constants,
      },
    );
    device.pushError(err);

    const computePipeline = createGPUComputePipeline(
      descriptor.label,
      device,
      rid,
    );
    device.trackResource(computePipeline);
    return computePipeline;
  }

  /**
   * @param {GPURenderPipelineDescriptor} descriptor
   * @returns {GPURenderPipeline}
   */
  createRenderPipeline(descriptor) {
    webidl.assertBranded(this, GPUDevicePrototype);
    const prefix = "Failed to execute 'createRenderPipeline' on 'GPUDevice'";
    webidl.requiredArguments(arguments.length, 1, prefix);
    descriptor = webidl.converters.GPURenderPipelineDescriptor(
      descriptor,
      prefix,
      "Argument 1",
    );
    const device = assertDevice(this, prefix, "this");
    let layout = descriptor.layout;
    if (typeof descriptor.layout !== "string") {
      const context = "layout";
      layout = assertResource(descriptor.layout, prefix, context);
      assertDeviceMatch(device, descriptor.layout, {
        prefix,
        resourceContext: context,
        selfContext: "this",
      });
    }
    const module = assertResource(
      descriptor.vertex.module,
      prefix,
      "vertex shader module",
    );
    assertDeviceMatch(device, descriptor.vertex.module, {
      prefix,
      resourceContext: "vertex shader module",
      selfContext: "this",
    });
    let fragment = undefined;
    if (descriptor.fragment) {
      const module = assertResource(
        descriptor.fragment.module,
        prefix,
        "fragment shader module",
      );
      assertDeviceMatch(device, descriptor.fragment.module, {
        prefix,
        resourceContext: "fragment shader module",
        selfContext: "this",
      });
      fragment = {
        module,
        entryPoint: descriptor.fragment.entryPoint,
        targets: descriptor.fragment.targets,
      };
    }

    const { rid, err } = op_webgpu_create_render_pipeline({
      deviceRid: device.rid,
      label: descriptor.label,
      layout,
      vertex: {
        module,
        entryPoint: descriptor.vertex.entryPoint,
        buffers: descriptor.vertex.buffers,
      },
      primitive: descriptor.primitive,
      depthStencil: descriptor.depthStencil,
      multisample: descriptor.multisample,
      fragment,
    });
    device.pushError(err);

    const renderPipeline = createGPURenderPipeline(
      descriptor.label,
      device,
      rid,
    );
    device.trackResource(renderPipeline);
    return renderPipeline;
  }

  createComputePipelineAsync(descriptor) {
    // TODO(lucacasonato): this should be real async

    webidl.assertBranded(this, GPUDevicePrototype);
    const prefix =
      "Failed to execute 'createComputePipelineAsync' on 'GPUDevice'";
    webidl.requiredArguments(arguments.length, 1, prefix);
    descriptor = webidl.converters.GPUComputePipelineDescriptor(
      descriptor,
      prefix,
      "Argument 1",
    );
    const device = assertDevice(this, prefix, "this");
    let layout = descriptor.layout;
    if (typeof descriptor.layout !== "string") {
      const context = "layout";
      layout = assertResource(descriptor.layout, prefix, context);
      assertDeviceMatch(device, descriptor.layout, {
        prefix,
        resourceContext: context,
        selfContext: "this",
      });
    }
    const module = assertResource(
      descriptor.compute.module,
      prefix,
      "compute shader module",
    );
    assertDeviceMatch(device, descriptor.compute.module, {
      prefix,
      resourceContext: "compute shader module",
      selfContext: "this",
    });

    const { rid, err } = op_webgpu_create_compute_pipeline(
      device.rid,
      descriptor.label,
      layout,
      {
        module,
        entryPoint: descriptor.compute.entryPoint,
        constants: descriptor.compute.constants,
      },
    );
    device.pushError(err);
    if (err) {
      switch (err.type) {
        case "validation":
          return PromiseReject(
            new GPUPipelineError(err.value ?? "validation error", {
              reason: "validation",
            }),
          );
        case "internal":
          return PromiseReject(
            new GPUPipelineError("internal error", {
              reason: "validation",
            }),
          );
      }
    }

    const computePipeline = createGPUComputePipeline(
      descriptor.label,
      device,
      rid,
    );
    device.trackResource(computePipeline);
    return PromiseResolve(computePipeline);
  }

  createRenderPipelineAsync(descriptor) {
    // TODO(lucacasonato): this should be real async

    webidl.assertBranded(this, GPUDevicePrototype);
    const prefix =
      "Failed to execute 'createRenderPipelineAsync' on 'GPUDevice'";
    webidl.requiredArguments(arguments.length, 1, prefix);
    descriptor = webidl.converters.GPURenderPipelineDescriptor(
      descriptor,
      prefix,
      "Argument 1",
    );
    const device = assertDevice(this, prefix, "this");
    let layout = descriptor.layout;
    if (typeof descriptor.layout !== "string") {
      const context = "layout";
      layout = assertResource(descriptor.layout, prefix, context);
      assertDeviceMatch(device, descriptor.layout, {
        prefix,
        resourceContext: context,
        selfContext: "this",
      });
    }
    const module = assertResource(
      descriptor.vertex.module,
      prefix,
      "vertex shader module",
    );
    assertDeviceMatch(device, descriptor.vertex.module, {
      prefix,
      resourceContext: "vertex shader module",
      selfContext: "this",
    });
    let fragment = undefined;
    if (descriptor.fragment) {
      const module = assertResource(
        descriptor.fragment.module,
        prefix,
        "fragment shader module",
      );
      assertDeviceMatch(device, descriptor.fragment.module, {
        prefix,
        resourceContext: "fragment shader module",
        selfContext: "this",
      });
      fragment = {
        module,
        entryPoint: descriptor.fragment.entryPoint,
        targets: descriptor.fragment.targets,
      };
    }

    const { rid, err } = op_webgpu_create_render_pipeline({
      deviceRid: device.rid,
      label: descriptor.label,
      layout,
      vertex: {
        module,
        entryPoint: descriptor.vertex.entryPoint,
        buffers: descriptor.vertex.buffers,
      },
      primitive: descriptor.primitive,
      depthStencil: descriptor.depthStencil,
      multisample: descriptor.multisample,
      fragment,
    });
    device.pushError(err);
    if (err) {
      switch (err.type) {
        case "validation":
          return PromiseReject(
            new GPUPipelineError(err.value ?? "validation error", {
              reason: "validation",
            }),
          );
        case "internal":
          return PromiseReject(
            new GPUPipelineError("internal error", {
              reason: "validation",
            }),
          );
      }
    }

    const renderPipeline = createGPURenderPipeline(
      descriptor.label,
      device,
      rid,
    );
    device.trackResource(renderPipeline);
    return PromiseResolve(renderPipeline);
  }

  /**
   * @param {GPUCommandEncoderDescriptor} descriptor
   * @returns {GPUCommandEncoder}
   */
  createCommandEncoder(descriptor = { __proto__: null }) {
    webidl.assertBranded(this, GPUDevicePrototype);
    const prefix = "Failed to execute 'createCommandEncoder' on 'GPUDevice'";
    descriptor = webidl.converters.GPUCommandEncoderDescriptor(
      descriptor,
      prefix,
      "Argument 1",
    );
    const device = assertDevice(this, prefix, "this");
    const { rid, err } = op_webgpu_create_command_encoder(
      device.rid,
      descriptor.label,
    );
    device.pushError(err);

    const commandEncoder = createGPUCommandEncoder(
      descriptor.label,
      device,
      rid,
    );
    device.trackResource(commandEncoder);
    return commandEncoder;
  }

  /**
   * @param {GPURenderBundleEncoderDescriptor} descriptor
   * @returns {GPURenderBundleEncoder}
   */
  createRenderBundleEncoder(descriptor) {
    webidl.assertBranded(this, GPUDevicePrototype);
    const prefix =
      "Failed to execute 'createRenderBundleEncoder' on 'GPUDevice'";
    webidl.requiredArguments(arguments.length, 1, { prefix });
    descriptor = webidl.converters.GPURenderBundleEncoderDescriptor(
      descriptor,
      prefix,
      "Argument 1",
    );
    const device = assertDevice(this, prefix, "this");
    const { rid, err } = op_webgpu_create_render_bundle_encoder({
      deviceRid: device.rid,
      ...descriptor,
    });
    device.pushError(err);

    const renderBundleEncoder = createGPURenderBundleEncoder(
      descriptor.label,
      device,
      rid,
    );
    device.trackResource(renderBundleEncoder);
    return renderBundleEncoder;
  }

  /**
   * @param {GPUQuerySetDescriptor} descriptor
   * @returns {GPUQuerySet}
   */
  createQuerySet(descriptor) {
    webidl.assertBranded(this, GPUDevicePrototype);
    const prefix = "Failed to execute 'createQuerySet' on 'GPUDevice'";
    webidl.requiredArguments(arguments.length, 1, prefix);
    descriptor = webidl.converters.GPUQuerySetDescriptor(
      descriptor,
      prefix,
      "Argument 1",
    );
    const device = assertDevice(this, prefix, "this");
    const { rid, err } = op_webgpu_create_query_set({
      deviceRid: device.rid,
      ...descriptor,
    });
    device.pushError(err);

    const querySet = createGPUQuerySet(
      descriptor.label,
      device,
      rid,
      descriptor,
    );
    device.trackResource(querySet);
    return querySet;
  }

  get lost() {
    webidl.assertBranded(this, GPUDevicePrototype);
    const device = this[_device];
    if (!device) {
      return PromiseResolve(true);
    }
    if (device.rid === undefined) {
      return PromiseResolve(true);
    }
    return device.lost;
  }

  /**
   * @param {GPUErrorFilter} filter
   */
  pushErrorScope(filter) {
    webidl.assertBranded(this, GPUDevicePrototype);
    const prefix = "Failed to execute 'pushErrorScope' on 'GPUDevice'";
    webidl.requiredArguments(arguments.length, 1, prefix);
    filter = webidl.converters.GPUErrorFilter(filter, prefix, "Argument 1");
    const device = assertDevice(this, prefix, "this");
    ArrayPrototypePush(device.errorScopeStack, { filter, errors: [] });
  }

  /**
   * @returns {Promise<GPUError | null>}
   */
  // deno-lint-ignore require-await
  async popErrorScope() {
    webidl.assertBranded(this, GPUDevicePrototype);
    const prefix = "Failed to execute 'popErrorScope' on 'GPUDevice'";
    const device = assertDevice(this, prefix, "this");
    if (device.isLost) {
      throw new DOMException("Device has been lost.", "OperationError");
    }
    const scope = ArrayPrototypePop(device.errorScopeStack);
    if (!scope) {
      throw new DOMException(
        "There are no error scopes on the error scope stack.",
        "OperationError",
      );
    }
    return PromiseResolve(scope.errors[0] ?? null);
  }

  [SymbolFor("Deno.privateCustomInspect")](inspect, inspectOptions) {
    return inspect(
      createFilteredInspectProxy({
        object: this,
        evaluate: ObjectPrototypeIsPrototypeOf(GPUDevicePrototype, this),
        keys: [
          "features",
          "label",
          "limits",
          "lost",
          "queue",
          // TODO(lucacasonato): emit an UncapturedErrorEvent
          // "onuncapturederror"
        ],
      }),
      inspectOptions,
    );
  }
}
GPUObjectBaseMixin("GPUDevice", GPUDevice);
const GPUDevicePrototype = GPUDevice.prototype;
defineEventHandler(GPUDevice.prototype, "uncapturederror");

class GPUPipelineError extends DOMException {
  #reason;

  constructor(message = "", options = { __proto__: null }) {
    const prefix = "Failed to construct 'GPUPipelineError'";
    message = webidl.converters.DOMString(message, prefix, "Argument 1");
    options = webidl.converters.GPUPipelineErrorInit(
      options,
      prefix,
      "Argument 2",
    );
    super(message, "GPUPipelineError");

    this.#reason = options.reason;
  }

  get reason() {
    webidl.assertBranded(this, GPUPipelineErrorPrototype);
    return this.#reason;
  }
}
const GPUPipelineErrorPrototype = GPUPipelineError.prototype;

/**
 * @param {string | null} label
 * @param {InnerGPUDevice} device
 * @param {number} rid
 * @returns {GPUQueue}
 */
function createGPUQueue(label, device, rid) {
  /** @type {GPUQueue} */
  const queue = webidl.createBranded(GPUQueue);
  queue[_label] = label;
  queue[_device] = device;
  queue[_rid] = rid;
  return queue;
}

class GPUQueue {
  /** @type {InnerGPUDevice} */
  [_device];
  /** @type {number} */
  [_rid];

  [_cleanup]() {
    const rid = this[_rid];
    if (rid !== undefined) {
      core.close(rid);
      /** @type {number | undefined} */
      this[_rid] = undefined;
    }
  }

  constructor() {
    webidl.illegalConstructor();
  }

  /**
   * @param {GPUCommandBuffer[]} commandBuffers
   */
  submit(commandBuffers) {
    webidl.assertBranded(this, GPUQueuePrototype);
    const prefix = "Failed to execute 'submit' on 'GPUQueue'";
    webidl.requiredArguments(arguments.length, 1, {
      prefix,
    });
    commandBuffers = webidl.converters["sequence<GPUCommandBuffer>"](
      commandBuffers,
      prefix,
      "Argument 1",
    );
    const device = assertDevice(this, prefix, "this");
    const commandBufferRids = ArrayPrototypeMap(
      commandBuffers,
      (buffer, i) => {
        const context = `command buffer ${i + 1}`;
        const rid = assertResource(buffer, prefix, context);
        assertDeviceMatch(device, buffer, {
          prefix,
          selfContext: "this",
          resourceContext: context,
        });
        return rid;
      },
    );
    const { err } = op_webgpu_queue_submit(this[_rid], commandBufferRids);
    for (let i = 0; i < commandBuffers.length; ++i) {
      commandBuffers[i][_rid] = undefined;
    }
    device.pushError(err);
  }

  onSubmittedWorkDone() {
    webidl.assertBranded(this, GPUQueuePrototype);
    return PromiseResolve();
  }

  /**
   * @param {GPUBuffer} buffer
   * @param {number} bufferOffset
   * @param {BufferSource} data
   * @param {number} [dataOffset]
   * @param {number} [size]
   */
  writeBuffer(buffer, bufferOffset, data, dataOffset = 0, size) {
    webidl.assertBranded(this, GPUQueuePrototype);
    const prefix = "Failed to execute 'writeBuffer' on 'GPUQueue'";
    webidl.requiredArguments(arguments.length, 3, prefix);
    buffer = webidl.converters["GPUBuffer"](buffer, prefix, "Argument 1");
    bufferOffset = webidl.converters["GPUSize64"](
      bufferOffset,
      prefix,
      "Argument 2",
    );
    data = webidl.converters.BufferSource(data, prefix, "Argument 3");
    dataOffset = webidl.converters["GPUSize64"](
      dataOffset,
      prefix,
      "Argument 4",
    );
    size = size === undefined
      ? undefined
      : webidl.converters.GPUSize64(size, prefix, "Argument 5");
    const device = assertDevice(this, prefix, "this");
    const bufferRid = assertResource(buffer, prefix, "Argument 1");
    assertDeviceMatch(device, buffer, {
      prefix,
      selfContext: "this",
      resourceContext: "Argument 1",
    });
    /** @type {ArrayBufferLike} */
    let abLike = data;
    if (isTypedArray(data)) {
      abLike = TypedArrayPrototypeGetBuffer(
        /** @type {Uint8Array} */ (data),
      );
    } else if (isDataView(data)) {
      abLike = DataViewPrototypeGetBuffer(/** @type {DataView} */ (data));
    }

    const { err } = op_webgpu_write_buffer(
      this[_rid],
      bufferRid,
      bufferOffset,
      dataOffset,
      size,
      new Uint8Array(abLike),
    );
    device.pushError(err);
  }

  /**
   * @param {GPUImageCopyTexture} destination
   * @param {BufferSource} data
   * @param {GPUImageDataLayout} dataLayout
   * @param {GPUExtent3D} size
   */
  writeTexture(destination, data, dataLayout, size) {
    webidl.assertBranded(this, GPUQueuePrototype);
    const prefix = "Failed to execute 'writeTexture' on 'GPUQueue'";
    webidl.requiredArguments(arguments.length, 4, prefix);
    destination = webidl.converters.GPUImageCopyTexture(
      destination,
      prefix,
      "Argument 1",
    );
    data = webidl.converters.BufferSource(data, prefix, "Argument 2");
    dataLayout = webidl.converters.GPUImageDataLayout(
      dataLayout,
      prefix,
      "Argument 3",
    );
    size = webidl.converters.GPUExtent3D(size, prefix, "Argument 4");
    const device = assertDevice(this, prefix, "this");
    const textureRid = assertResource(destination.texture, prefix, "texture");
    assertDeviceMatch(device, destination.texture, {
      prefix,
      selfContext: "this",
      resourceContext: "texture",
    });

    /** @type {ArrayBufferLike} */
    let abLike = data;
    if (isTypedArray(data)) {
      abLike = TypedArrayPrototypeGetBuffer(
        /** @type {Uint8Array} */ (data),
      );
    } else if (isDataView(data)) {
      abLike = DataViewPrototypeGetBuffer(/** @type {DataView} */ (data));
    }

    const { err } = op_webgpu_write_texture(
      this[_rid],
      {
        texture: textureRid,
        mipLevel: destination.mipLevel,
        origin: destination.origin
          ? normalizeGPUOrigin3D(destination.origin)
          : undefined,
        aspect: destination.aspect,
      },
      dataLayout,
      normalizeGPUExtent3D(size),
      new Uint8Array(abLike),
    );
    device.pushError(err);
  }

  [SymbolFor("Deno.privateCustomInspect")](inspect, inspectOptions) {
    return inspect(
      createFilteredInspectProxy({
        object: this,
        evaluate: ObjectPrototypeIsPrototypeOf(GPUQueuePrototype, this),
        keys: [
          "label",
        ],
      }),
      inspectOptions,
    );
  }
}
GPUObjectBaseMixin("GPUQueue", GPUQueue);
const GPUQueuePrototype = GPUQueue.prototype;

/**
 * @typedef CreateGPUBufferOptions
 * @property {ArrayBuffer | null} mapping
 * @property {number[] | null} mappingRange
 * @property {[ArrayBuffer, number, number][] | null} mappedRanges
 * @property {"mapped" | "mapped at creation" | "mapped pending" | "unmapped" | "destroy" } state
 */

/**
 * @param {string | null} label
 * @param {InnerGPUDevice} device
 * @param {number} rid
 * @param {number} size
 * @param {number} usage
 * @param {CreateGPUBufferOptions} options
 * @returns {GPUBuffer}
 */
function createGPUBuffer(label, device, rid, size, usage, options) {
  /** @type {GPUBuffer} */
  const buffer = webidl.createBranded(GPUBuffer);
  buffer[_label] = label;
  buffer[_device] = device;
  buffer[_rid] = rid;
  buffer[_size] = size;
  buffer[_usage] = usage;
  buffer[_mappingRange] = options.mappingRange;
  buffer[_mappedRanges] = options.mappedRanges;
  buffer[_state] = options.state;
  return buffer;
}

class GPUBuffer {
  /** @type {InnerGPUDevice} */
  [_device];
  /** @type {number} */
  [_rid];
  /** @type {number} */
  [_size];
  /** @type {number} */
  [_usage];
  /** @type {"mapped" | "mapped at creation" | "pending" | "unmapped" | "destroy"} */
  [_state];
  /** @type {[number, number] | null} */
  [_mappingRange];
  /** @type {[ArrayBuffer, number, number][] | null} */
  [_mappedRanges];
  /** @type {number} */
  [_mapMode];

  [_cleanup]() {
    const mappedRanges = this[_mappedRanges];
    if (mappedRanges) {
      while (mappedRanges.length > 0) {
        const mappedRange = ArrayPrototypePop(mappedRanges);
        if (mappedRange !== undefined) {
          core.close(mappedRange[1]);
        }
      }
    }
    const rid = this[_rid];
    if (rid !== undefined) {
      core.close(rid);
      /** @type {number | undefined} */
      this[_rid] = undefined;
    }
    this[_state] = "destroy";
  }

  constructor() {
    webidl.illegalConstructor();
  }

  get size() {
    webidl.assertBranded(this, GPUBufferPrototype);
    return this[_size];
  }

  get usage() {
    webidl.assertBranded(this, GPUBufferPrototype);
    return this[_usage];
  }

  get mapState() {
    webidl.assertBranded(this, GPUBufferPrototype);
    const state = this[_state];
    if (state === "mapped at creation") {
      return "mapped";
    } else {
      return state;
    }
  }

  /**
   * @param {number} mode
   * @param {number} offset
   * @param {number} [size]
   */
  async mapAsync(mode, offset = 0, size) {
    webidl.assertBranded(this, GPUBufferPrototype);
    const prefix = "Failed to execute 'mapAsync' on 'GPUBuffer'";
    webidl.requiredArguments(arguments.length, 1, prefix);
    mode = webidl.converters.GPUMapModeFlags(mode, prefix, "Argument 1");
    offset = webidl.converters.GPUSize64(offset, prefix, "Argument 2");
    size = size === undefined
      ? undefined
      : webidl.converters.GPUSize64(size, prefix, "Argument 3");
    const device = assertDevice(this, prefix, "this");
    const bufferRid = assertResource(this, prefix, "this");
    /** @type {number} */
    let rangeSize;
    if (size === undefined) {
      rangeSize = MathMax(0, this[_size] - offset);
    } else {
      rangeSize = this[_size];
    }
    if ((offset % 8) !== 0) {
      throw new DOMException(
        `${prefix}: offset must be a multiple of 8.`,
        "OperationError",
      );
    }
    if ((rangeSize % 4) !== 0) {
      throw new DOMException(
        `${prefix}: rangeSize must be a multiple of 4.`,
        "OperationError",
      );
    }
    if ((offset + rangeSize) > this[_size]) {
      throw new DOMException(
        `${prefix}: offset + rangeSize must be less than or equal to buffer size.`,
        "OperationError",
      );
    }
    if (this[_state] !== "unmapped") {
      throw new DOMException(
        `${prefix}: GPUBuffer is not currently unmapped.`,
        "OperationError",
      );
    }
    const readMode = (mode & 0x0001) === 0x0001;
    const writeMode = (mode & 0x0002) === 0x0002;
    if ((readMode && writeMode) || (!readMode && !writeMode)) {
      throw new DOMException(
        `${prefix}: exactly one of READ or WRITE map mode must be set.`,
        "OperationError",
      );
    }
    if (readMode && !((this[_usage] && 0x0001) === 0x0001)) {
      throw new DOMException(
        `${prefix}: READ map mode not valid because buffer does not have MAP_READ usage.`,
        "OperationError",
      );
    }
    if (writeMode && !((this[_usage] && 0x0002) === 0x0002)) {
      throw new DOMException(
        `${prefix}: WRITE map mode not valid because buffer does not have MAP_WRITE usage.`,
        "OperationError",
      );
    }

    this[_mapMode] = mode;
    this[_state] = "pending";
    const { err } = await op_webgpu_buffer_get_map_async(
      bufferRid,
      device.rid,
      mode,
      offset,
      rangeSize,
    );
    if (err) {
      device.pushError(err);
      throw new DOMException("validation error occurred", "OperationError");
    }
    this[_state] = "mapped";
    this[_mappingRange] = [offset, offset + rangeSize];
    /** @type {[ArrayBuffer, number, number][] | null} */
    this[_mappedRanges] = [];
  }

  /**
   * @param {number} offset
   * @param {number} size
   */
  getMappedRange(offset = 0, size) {
    webidl.assertBranded(this, GPUBufferPrototype);
    const prefix = "Failed to execute 'getMappedRange' on 'GPUBuffer'";
    offset = webidl.converters.GPUSize64(offset, prefix, "Argument 1");
    if (size !== undefined) {
      size = webidl.converters.GPUSize64(size, prefix, "Argument 2");
    }
    assertDevice(this, prefix, "this");
    const bufferRid = assertResource(this, prefix, "this");
    /** @type {number} */
    let rangeSize;
    if (size === undefined) {
      rangeSize = MathMax(0, this[_size] - offset);
    } else {
      rangeSize = size;
    }

    const mappedRanges = this[_mappedRanges];
    if (!mappedRanges) {
      throw new DOMException(`${prefix}: invalid state.`, "OperationError");
    }
    for (let i = 0; i < mappedRanges.length; ++i) {
      const { 0: buffer, 1: _rid, 2: start } = mappedRanges[i];
      // TODO(lucacasonato): is this logic correct?
      const end = start + ArrayBufferPrototypeGetByteLength(buffer);
      if (
        (start >= offset && start < (offset + rangeSize)) ||
        (end >= offset && end < (offset + rangeSize))
      ) {
        throw new DOMException(
          `${prefix}: requested buffer overlaps with another mapped range.`,
          "OperationError",
        );
      }
    }

    const buffer = new ArrayBuffer(rangeSize);
    const { rid } = op_webgpu_buffer_get_mapped_range(
      bufferRid,
      offset,
      size,
      new Uint8Array(buffer),
    );

    ArrayPrototypePush(mappedRanges, [buffer, rid, offset]);

    return buffer;
  }

  unmap() {
    webidl.assertBranded(this, GPUBufferPrototype);
    const prefix = "Failed to execute 'unmap' on 'GPUBuffer'";
    const device = assertDevice(this, prefix, "this");
    const bufferRid = assertResource(this, prefix, "this");
    if (this[_state] === "unmapped" || this[_state] === "destroyed") {
      throw new DOMException(
        `${prefix}: buffer is not ready to be unmapped.`,
        "OperationError",
      );
    }
    if (this[_state] === "pending") {
      // TODO(lucacasonato): this is not spec compliant.
      throw new DOMException(
        `${prefix}: can not unmap while mapping. This is a Deno limitation.`,
        "OperationError",
      );
    } else if (
      this[_state] === "mapped" || this[_state] === "mapped at creation"
    ) {
      /** @type {boolean} */
      let write = false;
      if (this[_state] === "mapped at creation") {
        write = true;
      } else if (this[_state] === "mapped") {
        const mapMode = this[_mapMode];
        if (mapMode === undefined) {
          throw new DOMException(
            `${prefix}: invalid state.`,
            "OperationError",
          );
        }
        if ((mapMode & 0x0002) === 0x0002) {
          write = true;
        }
      }

      const mappedRanges = this[_mappedRanges];
      if (!mappedRanges) {
        throw new DOMException(`${prefix}: invalid state.`, "OperationError");
      }
      for (let i = 0; i < mappedRanges.length; ++i) {
        const { 0: buffer, 1: mappedRid } = mappedRanges[i];
        const { err } = op_webgpu_buffer_unmap(
          bufferRid,
          mappedRid,
          ...new SafeArrayIterator(write ? [new Uint8Array(buffer)] : []),
        );
        device.pushError(err);
        if (err) return;
      }
      this[_mappingRange] = null;
      this[_mappedRanges] = null;
    }

    this[_state] = "unmapped";
  }

  destroy() {
    webidl.assertBranded(this, GPUBufferPrototype);
    this[_cleanup]();
  }

  [SymbolFor("Deno.privateCustomInspect")](inspect, inspectOptions) {
    return inspect(
      createFilteredInspectProxy({
        object: this,
        evaluate: ObjectPrototypeIsPrototypeOf(GPUBufferPrototype, this),
        keys: [
          "label",
          "mapState",
          "size",
          "usage",
        ],
      }),
      inspectOptions,
    );
  }
}
GPUObjectBaseMixin("GPUBuffer", GPUBuffer);
const GPUBufferPrototype = GPUBuffer.prototype;

class GPUBufferUsage {
  constructor() {
    webidl.illegalConstructor();
  }

  static get MAP_READ() {
    return 0x0001;
  }
  static get MAP_WRITE() {
    return 0x0002;
  }
  static get COPY_SRC() {
    return 0x0004;
  }
  static get COPY_DST() {
    return 0x0008;
  }
  static get INDEX() {
    return 0x0010;
  }
  static get VERTEX() {
    return 0x0020;
  }
  static get UNIFORM() {
    return 0x0040;
  }
  static get STORAGE() {
    return 0x0080;
  }
  static get INDIRECT() {
    return 0x0100;
  }
  static get QUERY_RESOLVE() {
    return 0x0200;
  }
}

class GPUMapMode {
  constructor() {
    webidl.illegalConstructor();
  }

  static get READ() {
    return 0x0001;
  }
  static get WRITE() {
    return 0x0002;
  }
}

/**
 * @param {GPUTextureDescriptor} descriptor
 * @param {InnerGPUDevice} device
 * @param {number} rid
 * @returns {GPUTexture}
 */
function createGPUTexture(descriptor, device, rid) {
  /** @type {GPUTexture} */
  const texture = webidl.createBranded(GPUTexture);
  texture[_label] = descriptor.label;
  texture[_device] = device;
  texture[_rid] = rid;
  texture[_views] = [];
  texture[_width] = descriptor.size.width;
  texture[_height] = descriptor.size.height;
  texture[_depthOrArrayLayers] = descriptor.size.depthOrArrayLayers;
  texture[_mipLevelCount] = descriptor.mipLevelCount;
  texture[_sampleCount] = descriptor.sampleCount;
  texture[_dimension] = descriptor.dimension;
  texture[_format] = descriptor.format;
  texture[_usage] = descriptor.usage;
  return texture;
}

class GPUTexture {
  /** @type {InnerGPUDevice} */
  [_device];
  /** @type {number | undefined} */
  [_rid];
  /** @type {SafeWeakRef<GPUTextureView>[]} */
  [_views];

  /** @type {number} */
  [_width];
  /** @type {number} */
  [_height];
  /** @type {number} */
  [_depthOrArrayLayers];
  /** @type {number} */
  [_mipLevelCount];
  /** @type {number} */
  [_sampleCount];
  /** @type {GPUTextureDimension} */
  [_dimension];
  /** @type {GPUTextureFormat} */
  [_format];
  /** @type {number} */
  [_usage];

  [_cleanup]() {
    const views = this[_views];
    while (views.length > 0) {
      const view = ArrayPrototypePop(views)?.deref();
      if (view) {
        view[_cleanup]();
      }
    }
    const rid = this[_rid];
    if (rid !== undefined) {
      core.close(rid);
      /** @type {number | undefined} */
      this[_rid] = undefined;
    }
  }

  constructor() {
    webidl.illegalConstructor();
  }

  /**
   * @param {GPUTextureViewDescriptor} descriptor
   */
  createView(descriptor = { __proto__: null }) {
    webidl.assertBranded(this, GPUTexturePrototype);
    const prefix = "Failed to execute 'createView' on 'GPUTexture'";
    webidl.requiredArguments(arguments.length, 0, prefix);
    descriptor = webidl.converters.GPUTextureViewDescriptor(
      descriptor,
      prefix,
      "Argument 1",
    );
    const device = assertDevice(this, prefix, "this");
    const textureRid = assertResource(this, prefix, "this");
    const { rid, err } = op_webgpu_create_texture_view({
      textureRid,
      ...descriptor,
    });
    device.pushError(err);

    const textureView = createGPUTextureView(
      descriptor.label,
      this,
      rid,
    );
    ArrayPrototypePush(this[_views], new SafeWeakRef(textureView));
    return textureView;
  }

  destroy() {
    webidl.assertBranded(this, GPUTexturePrototype);
    this[_cleanup]();
  }

  get width() {
    webidl.assertBranded(this, GPUTexturePrototype);
    return this[_width];
  }

  get height() {
    webidl.assertBranded(this, GPUTexturePrototype);
    return this[_height];
  }

  get depthOrArrayLayers() {
    webidl.assertBranded(this, GPUTexturePrototype);
    return this[_depthOrArrayLayers];
  }

  get mipLevelCount() {
    webidl.assertBranded(this, GPUTexturePrototype);
    return this[_mipLevelCount];
  }

  get sampleCount() {
    webidl.assertBranded(this, GPUTexturePrototype);
    return this[_sampleCount];
  }

  get dimension() {
    webidl.assertBranded(this, GPUTexturePrototype);
    return this[_dimension];
  }

  get format() {
    webidl.assertBranded(this, GPUTexturePrototype);
    return this[_format];
  }

  get usage() {
    webidl.assertBranded(this, GPUTexturePrototype);
    return this[_usage];
  }

  [SymbolFor("Deno.privateCustomInspect")](inspect, inspectOptions) {
    return inspect(
      createFilteredInspectProxy({
        object: this,
        evaluate: ObjectPrototypeIsPrototypeOf(GPUTexturePrototype, this),
        keys: [
          "label",
          "width",
          "height",
          "depthOrArrayLayers",
          "mipLevelCount",
          "sampleCount",
          "dimension",
          "format",
          "usage",
        ],
      }),
      inspectOptions,
    );
  }
}
GPUObjectBaseMixin("GPUTexture", GPUTexture);
const GPUTexturePrototype = GPUTexture.prototype;

class GPUTextureUsage {
  constructor() {
    webidl.illegalConstructor();
  }

  static get COPY_SRC() {
    return 0x01;
  }
  static get COPY_DST() {
    return 0x02;
  }
  static get TEXTURE_BINDING() {
    return 0x04;
  }
  static get STORAGE_BINDING() {
    return 0x08;
  }
  static get RENDER_ATTACHMENT() {
    return 0x10;
  }
}

/**
 * @param {string | null} label
 * @param {GPUTexture} texture
 * @param {number} rid
 * @returns {GPUTextureView}
 */
function createGPUTextureView(label, texture, rid) {
  /** @type {GPUTextureView} */
  const textureView = webidl.createBranded(GPUTextureView);
  textureView[_label] = label;
  textureView[_texture] = texture;
  textureView[_rid] = rid;
  return textureView;
}
class GPUTextureView {
  /** @type {GPUTexture} */
  [_texture];
  /** @type {number | undefined} */
  [_rid];

  [_cleanup]() {
    const rid = this[_rid];
    if (rid !== undefined) {
      core.close(rid);
      /** @type {number | undefined} */
      this[_rid] = undefined;
    }
  }

  constructor() {
    webidl.illegalConstructor();
  }

  [SymbolFor("Deno.privateCustomInspect")](inspect, inspectOptions) {
    return inspect(
      createFilteredInspectProxy({
        object: this,
        evaluate: ObjectPrototypeIsPrototypeOf(GPUTextureViewPrototype, this),
        keys: [
          "label",
        ],
      }),
      inspectOptions,
    );
  }
}
GPUObjectBaseMixin("GPUTextureView", GPUTextureView);
const GPUTextureViewPrototype = GPUTextureView.prototype;
/**
 * @param {string | null} label
 * @param {InnerGPUDevice} device
 * @param {number} rid
 * @returns {GPUSampler}
 */
function createGPUSampler(label, device, rid) {
  /** @type {GPUSampler} */
  const sampler = webidl.createBranded(GPUSampler);
  sampler[_label] = label;
  sampler[_device] = device;
  sampler[_rid] = rid;
  return sampler;
}
class GPUSampler {
  /** @type {InnerGPUDevice} */
  [_device];
  /** @type {number | undefined} */
  [_rid];

  [_cleanup]() {
    const rid = this[_rid];
    if (rid !== undefined) {
      core.close(rid);
      /** @type {number | undefined} */
      this[_rid] = undefined;
    }
  }

  constructor() {
    webidl.illegalConstructor();
  }

  [SymbolFor("Deno.privateCustomInspect")](inspect) {
    return `${this.constructor.name} ${
      inspect({
        label: this.label,
      })
    }`;
  }
}
GPUObjectBaseMixin("GPUSampler", GPUSampler);
const GPUSamplerPrototype = GPUSampler.prototype;
/**
 * @param {string | null} label
 * @param {InnerGPUDevice} device
 * @param {number} rid
 * @returns {GPUBindGroupLayout}
 */
function createGPUBindGroupLayout(label, device, rid) {
  /** @type {GPUBindGroupLayout} */
  const bindGroupLayout = webidl.createBranded(GPUBindGroupLayout);
  bindGroupLayout[_label] = label;
  bindGroupLayout[_device] = device;
  bindGroupLayout[_rid] = rid;
  return bindGroupLayout;
}
class GPUBindGroupLayout {
  /** @type {InnerGPUDevice} */
  [_device];
  /** @type {number | undefined} */
  [_rid];

  [_cleanup]() {
    const rid = this[_rid];
    if (rid !== undefined) {
      core.close(rid);
      /** @type {number | undefined} */
      this[_rid] = undefined;
    }
  }

  constructor() {
    webidl.illegalConstructor();
  }

  [SymbolFor("Deno.privateCustomInspect")](inspect, inspectOptions) {
    return inspect(
      createFilteredInspectProxy({
        object: this,
        evaluate: ObjectPrototypeIsPrototypeOf(
          GPUBindGroupLayoutPrototype,
          this,
        ),
        keys: [
          "label",
        ],
      }),
      inspectOptions,
    );
  }
}
GPUObjectBaseMixin("GPUBindGroupLayout", GPUBindGroupLayout);
const GPUBindGroupLayoutPrototype = GPUBindGroupLayout.prototype;
/**
 * @param {string | null} label
 * @param {InnerGPUDevice} device
 * @param {number} rid
 * @returns {GPUPipelineLayout}
 */
function createGPUPipelineLayout(label, device, rid) {
  /** @type {GPUPipelineLayout} */
  const pipelineLayout = webidl.createBranded(GPUPipelineLayout);
  pipelineLayout[_label] = label;
  pipelineLayout[_device] = device;
  pipelineLayout[_rid] = rid;
  return pipelineLayout;
}
class GPUPipelineLayout {
  /** @type {InnerGPUDevice} */
  [_device];
  /** @type {number | undefined} */
  [_rid];

  [_cleanup]() {
    const rid = this[_rid];
    if (rid !== undefined) {
      core.close(rid);
      /** @type {number | undefined} */
      this[_rid] = undefined;
    }
  }

  constructor() {
    webidl.illegalConstructor();
  }

  [SymbolFor("Deno.privateCustomInspect")](inspect, inspectOptions) {
    return inspect(
      createFilteredInspectProxy({
        object: this,
        evaluate: ObjectPrototypeIsPrototypeOf(
          GPUPipelineLayoutPrototype,
          this,
        ),
        keys: [
          "label",
        ],
      }),
      inspectOptions,
    );
  }
}
GPUObjectBaseMixin("GPUPipelineLayout", GPUPipelineLayout);
const GPUPipelineLayoutPrototype = GPUPipelineLayout.prototype;

/**
 * @param {string | null} label
 * @param {InnerGPUDevice} device
 * @param {number} rid
 * @returns {GPUBindGroup}
 */
function createGPUBindGroup(label, device, rid) {
  /** @type {GPUBindGroup} */
  const bindGroup = webidl.createBranded(GPUBindGroup);
  bindGroup[_label] = label;
  bindGroup[_device] = device;
  bindGroup[_rid] = rid;
  return bindGroup;
}
class GPUBindGroup {
  /** @type {InnerGPUDevice} */
  [_device];
  /** @type {number | undefined} */
  [_rid];

  [_cleanup]() {
    const rid = this[_rid];
    if (rid !== undefined) {
      core.close(rid);
      /** @type {number | undefined} */
      this[_rid] = undefined;
    }
  }

  constructor() {
    webidl.illegalConstructor();
  }

  [SymbolFor("Deno.privateCustomInspect")](inspect, inspectOptions) {
    return inspect(
      createFilteredInspectProxy({
        object: this,
        evaluate: ObjectPrototypeIsPrototypeOf(GPUBindGroupPrototype, this),
        keys: [
          "label",
        ],
      }),
      inspectOptions,
    );
  }
}
GPUObjectBaseMixin("GPUBindGroup", GPUBindGroup);
const GPUBindGroupPrototype = GPUBindGroup.prototype;
/**
 * @param {string | null} label
 * @param {InnerGPUDevice} device
 * @param {number} rid
 * @returns {GPUShaderModule}
 */
function createGPUShaderModule(label, device, rid) {
  /** @type {GPUShaderModule} */
  const bindGroup = webidl.createBranded(GPUShaderModule);
  bindGroup[_label] = label;
  bindGroup[_device] = device;
  bindGroup[_rid] = rid;
  return bindGroup;
}
class GPUShaderModule {
  /** @type {InnerGPUDevice} */
  [_device];
  /** @type {number | undefined} */
  [_rid];

  [_cleanup]() {
    const rid = this[_rid];
    if (rid !== undefined) {
      core.close(rid);
      /** @type {number | undefined} */
      this[_rid] = undefined;
    }
  }

  constructor() {
    webidl.illegalConstructor();
  }

  [SymbolFor("Deno.privateCustomInspect")](inspect, inspectOptions) {
    return inspect(
      createFilteredInspectProxy({
        object: this,
        evaluate: ObjectPrototypeIsPrototypeOf(GPUShaderModulePrototype, this),
        keys: [
          "label",
        ],
      }),
      inspectOptions,
    );
  }
}
GPUObjectBaseMixin("GPUShaderModule", GPUShaderModule);
const GPUShaderModulePrototype = GPUShaderModule.prototype;
class GPUShaderStage {
  constructor() {
    webidl.illegalConstructor();
  }

  static get VERTEX() {
    return 0x1;
  }

  static get FRAGMENT() {
    return 0x2;
  }

  static get COMPUTE() {
    return 0x4;
  }
}

/**
 * @param {string | null} label
 * @param {InnerGPUDevice} device
 * @param {number} rid
 * @returns {GPUComputePipeline}
 */
function createGPUComputePipeline(label, device, rid) {
  /** @type {GPUComputePipeline} */
  const pipeline = webidl.createBranded(GPUComputePipeline);
  pipeline[_label] = label;
  pipeline[_device] = device;
  pipeline[_rid] = rid;
  return pipeline;
}
class GPUComputePipeline {
  /** @type {InnerGPUDevice} */
  [_device];
  /** @type {number | undefined} */
  [_rid];

  [_cleanup]() {
    const rid = this[_rid];
    if (rid !== undefined) {
      core.close(rid);
      /** @type {number | undefined} */
      this[_rid] = undefined;
    }
  }

  constructor() {
    webidl.illegalConstructor();
  }

  /**
   * @param {number} index
   * @returns {GPUBindGroupLayout}
   */
  getBindGroupLayout(index) {
    webidl.assertBranded(this, GPUComputePipelinePrototype);
    const prefix =
      "Failed to execute 'getBindGroupLayout' on 'GPUComputePipeline'";
    webidl.requiredArguments(arguments.length, 1, { prefix });
    index = webidl.converters["unsigned long"](index, prefix, "Argument 1");
    const device = assertDevice(this, prefix, "this");
    const computePipelineRid = assertResource(this, prefix, "this");
    const { rid, label, err } =
      op_webgpu_compute_pipeline_get_bind_group_layout(
        computePipelineRid,
        index,
      );
    device.pushError(err);

    const bindGroupLayout = createGPUBindGroupLayout(
      label,
      device,
      rid,
    );
    device.trackResource(bindGroupLayout);
    return bindGroupLayout;
  }

  [SymbolFor("Deno.privateCustomInspect")](inspect, inspectOptions) {
    return inspect(
      createFilteredInspectProxy({
        object: this,
        evaluate: ObjectPrototypeIsPrototypeOf(
          GPUComputePipelinePrototype,
          this,
        ),
        keys: [
          "label",
        ],
      }),
      inspectOptions,
    );
  }
}
GPUObjectBaseMixin("GPUComputePipeline", GPUComputePipeline);
const GPUComputePipelinePrototype = GPUComputePipeline.prototype;

/**
 * @param {string | null} label
 * @param {InnerGPUDevice} device
 * @param {number} rid
 * @returns {GPURenderPipeline}
 */
function createGPURenderPipeline(label, device, rid) {
  /** @type {GPURenderPipeline} */
  const pipeline = webidl.createBranded(GPURenderPipeline);
  pipeline[_label] = label;
  pipeline[_device] = device;
  pipeline[_rid] = rid;
  return pipeline;
}
class GPURenderPipeline {
  /** @type {InnerGPUDevice} */
  [_device];
  /** @type {number | undefined} */
  [_rid];

  [_cleanup]() {
    const rid = this[_rid];
    if (rid !== undefined) {
      core.close(rid);
      /** @type {number | undefined} */
      this[_rid] = undefined;
    }
  }

  constructor() {
    webidl.illegalConstructor();
  }

  /**
   * @param {number} index
   */
  getBindGroupLayout(index) {
    webidl.assertBranded(this, GPURenderPipelinePrototype);
    const prefix =
      "Failed to execute 'getBindGroupLayout' on 'GPURenderPipeline'";
    webidl.requiredArguments(arguments.length, 1, prefix);
    index = webidl.converters["unsigned long"](index, prefix, "Argument 1");
    const device = assertDevice(this, prefix, "this");
    const renderPipelineRid = assertResource(this, prefix, "this");
    const { rid, label, err } = op_webgpu_render_pipeline_get_bind_group_layout(
      renderPipelineRid,
      index,
    );
    device.pushError(err);

    const bindGroupLayout = createGPUBindGroupLayout(
      label,
      device,
      rid,
    );
    device.trackResource(bindGroupLayout);
    return bindGroupLayout;
  }

  [SymbolFor("Deno.privateCustomInspect")](inspect, inspectOptions) {
    return inspect(
      createFilteredInspectProxy({
        object: this,
        evaluate: ObjectPrototypeIsPrototypeOf(
          GPURenderPipelinePrototype,
          this,
        ),
        keys: [
          "label",
        ],
      }),
      inspectOptions,
    );
  }
}
GPUObjectBaseMixin("GPURenderPipeline", GPURenderPipeline);
const GPURenderPipelinePrototype = GPURenderPipeline.prototype;

class GPUColorWrite {
  constructor() {
    webidl.illegalConstructor();
  }

  static get RED() {
    return 0x1;
  }
  static get GREEN() {
    return 0x2;
  }
  static get BLUE() {
    return 0x4;
  }
  static get ALPHA() {
    return 0x8;
  }
  static get ALL() {
    return 0xF;
  }
}

/**
 * @param {string | null} label
 * @param {InnerGPUDevice} device
 * @param {number} rid
 * @returns {GPUCommandEncoder}
 */
function createGPUCommandEncoder(label, device, rid) {
  /** @type {GPUCommandEncoder} */
  const encoder = webidl.createBranded(GPUCommandEncoder);
  encoder[_label] = label;
  encoder[_device] = device;
  encoder[_rid] = rid;
  encoder[_encoders] = [];
  return encoder;
}
class GPUCommandEncoder {
  /** @type {InnerGPUDevice} */
  [_device];
  /** @type {number | undefined} */
  [_rid];
  /** @type {SafeWeakRef<GPURenderPassEncoder | GPUComputePassEncoder>[]} */
  [_encoders];

  [_cleanup]() {
    const encoders = this[_encoders];
    while (encoders.length > 0) {
      const encoder = ArrayPrototypePop(encoders)?.deref();
      if (encoder) {
        encoder[_cleanup]();
      }
    }
    const rid = this[_rid];
    if (rid !== undefined) {
      core.close(rid);
      /** @type {number | undefined} */
      this[_rid] = undefined;
    }
  }

  constructor() {
    webidl.illegalConstructor();
  }

  /**
   * @param {GPURenderPassDescriptor} descriptor
   * @return {GPURenderPassEncoder}
   */
  beginRenderPass(descriptor) {
    webidl.assertBranded(this, GPUCommandEncoderPrototype);
    const prefix = "Failed to execute 'beginRenderPass' on 'GPUCommandEncoder'";
    webidl.requiredArguments(arguments.length, 1, prefix);
    descriptor = webidl.converters.GPURenderPassDescriptor(
      descriptor,
      prefix,
      "Argument 1",
    );
    const device = assertDevice(this, prefix, "this");
    const commandEncoderRid = assertResource(this, prefix, "this");

    if (this[_rid] === undefined) {
      throw new DOMException(
        "Failed to execute 'beginRenderPass' on 'GPUCommandEncoder': already consumed",
        "OperationError",
      );
    }

    let depthStencilAttachment;
    if (descriptor.depthStencilAttachment) {
      if (
        descriptor.depthStencilAttachment.depthLoadOp === "clear" &&
        !(ObjectHasOwn(descriptor.depthStencilAttachment, "depthClearValue"))
      ) {
        throw webidl.makeException(
          TypeError,
          '`depthClearValue` must be specified when `depthLoadOp` is "clear"',
          prefix,
          "Argument 1",
        );
      }

      const view = assertResource(
        descriptor.depthStencilAttachment.view,
        prefix,
        "texture view for depth stencil attachment",
      );
      assertDeviceMatch(
        device,
        descriptor.depthStencilAttachment.view[_texture],
        {
          prefix,
          resourceContext: "texture view for depth stencil attachment",
          selfContext: "this",
        },
      );

      depthStencilAttachment = {
        ...descriptor.depthStencilAttachment,
        view,
      };
    }
    const colorAttachments = ArrayPrototypeMap(
      descriptor.colorAttachments,
      (colorAttachment, i) => {
        const context = `color attachment ${i + 1}`;
        const view = assertResource(
          colorAttachment.view,
          prefix,
          `texture view for ${context}`,
        );
        assertResource(
          colorAttachment.view[_texture],
          prefix,
          `texture backing texture view for ${context}`,
        );
        assertDeviceMatch(
          device,
          colorAttachment.view[_texture],
          {
            prefix,
            resourceContext: `texture view for ${context}`,
            selfContext: "this",
          },
        );
        let resolveTarget;
        if (colorAttachment.resolveTarget) {
          resolveTarget = assertResource(
            colorAttachment.resolveTarget,
            prefix,
            `resolve target texture view for ${context}`,
          );
          assertResource(
            colorAttachment.resolveTarget[_texture],
            prefix,
            `texture backing resolve target texture view for ${context}`,
          );
          assertDeviceMatch(
            device,
            colorAttachment.resolveTarget[_texture],
            {
              prefix,
              resourceContext: `resolve target texture view for ${context}`,
              selfContext: "this",
            },
          );
        }
        return {
          view: view,
          resolveTarget,
          storeOp: colorAttachment.storeOp,
          loadOp: colorAttachment.loadOp,
          clearValue: normalizeGPUColor(colorAttachment.clearValue),
        };
      },
    );

    let occlusionQuerySet;

    if (descriptor.occlusionQuerySet) {
      occlusionQuerySet = assertResource(
        descriptor.occlusionQuerySet,
        prefix,
        "occlusionQuerySet",
      );
    }

    let timestampWrites = null;
    if (descriptor.timestampWrites) {
      const querySet = assertResource(
        descriptor.timestampWrites.querySet,
        prefix,
        "querySet",
      );

      timestampWrites = {
        querySet,
        beginningOfPassWriteIndex:
          descriptor.timestampWrites.beginningOfPassWriteIndex,
        endOfPassWriteIndex: descriptor.timestampWrites.endOfPassWriteIndex,
      };
    }

    const { rid } = op_webgpu_command_encoder_begin_render_pass(
      commandEncoderRid,
      descriptor.label,
      colorAttachments,
      depthStencilAttachment,
      occlusionQuerySet,
      timestampWrites,
    );

    const renderPassEncoder = createGPURenderPassEncoder(
      descriptor.label,
      this,
      rid,
    );
    ArrayPrototypePush(this[_encoders], new SafeWeakRef(renderPassEncoder));
    return renderPassEncoder;
  }

  /**
   * @param {GPUComputePassDescriptor} descriptor
   */
  beginComputePass(descriptor = { __proto__: null }) {
    webidl.assertBranded(this, GPUCommandEncoderPrototype);
    const prefix =
      "Failed to execute 'beginComputePass' on 'GPUCommandEncoder'";
    descriptor = webidl.converters.GPUComputePassDescriptor(
      descriptor,
      prefix,
      "Argument 1",
    );

    assertDevice(this, prefix, "this");
    const commandEncoderRid = assertResource(this, prefix, "this");

    let timestampWrites = null;
    if (descriptor.timestampWrites) {
      const querySet = assertResource(
        descriptor.timestampWrites.querySet,
        prefix,
        "querySet",
      );

      timestampWrites = {
        querySet,
        beginningOfPassWriteIndex:
          descriptor.timestampWrites.beginningOfPassWriteIndex,
        endOfPassWriteIndex: descriptor.timestampWrites.endOfPassWriteIndex,
      };
    }

    const { rid } = op_webgpu_command_encoder_begin_compute_pass(
      commandEncoderRid,
      descriptor.label,
      timestampWrites,
    );

    const computePassEncoder = createGPUComputePassEncoder(
      descriptor.label,
      this,
      rid,
    );
    ArrayPrototypePush(this[_encoders], new SafeWeakRef(computePassEncoder));
    return computePassEncoder;
  }

  /**
   * @param {GPUBuffer} source
   * @param {number} sourceOffset
   * @param {GPUBuffer} destination
   * @param {number} destinationOffset
   * @param {number} size
   */
  copyBufferToBuffer(
    source,
    sourceOffset,
    destination,
    destinationOffset,
    size,
  ) {
    webidl.assertBranded(this, GPUCommandEncoderPrototype);
    const prefix =
      "Failed to execute 'copyBufferToBuffer' on 'GPUCommandEncoder'";
    webidl.requiredArguments(arguments.length, 5, prefix);
    source = webidl.converters.GPUBuffer(source, prefix, "Argument 1");
    sourceOffset = webidl.converters.GPUSize64(
      sourceOffset,
      prefix,
      "Argument 2",
    );
    destination = webidl.converters.GPUBuffer(
      destination,
      prefix,
      "Argument 3",
    );
    destinationOffset = webidl.converters.GPUSize64(
      destinationOffset,
      prefix,
      "Argument 4",
    );
    size = webidl.converters.GPUSize64(size, prefix, "Argument 5");
    const device = assertDevice(this, prefix, "this");
    const commandEncoderRid = assertResource(this, prefix, "this");
    const sourceRid = assertResource(source, prefix, "Argument 1");
    assertDeviceMatch(device, source, {
      prefix,
      resourceContext: "Argument 1",
      selfContext: "this",
    });
    const destinationRid = assertResource(destination, prefix, "Argument 3");
    assertDeviceMatch(device, destination, {
      prefix,
      resourceContext: "Argument 3",
      selfContext: "this",
    });

    const { err } = op_webgpu_command_encoder_copy_buffer_to_buffer(
      commandEncoderRid,
      sourceRid,
      sourceOffset,
      destinationRid,
      destinationOffset,
      size,
    );
    device.pushError(err);
  }

  /**
   * @param {GPUImageCopyBuffer} source
   * @param {GPUImageCopyTexture} destination
   * @param {GPUExtent3D} copySize
   */
  copyBufferToTexture(source, destination, copySize) {
    webidl.assertBranded(this, GPUCommandEncoderPrototype);
    const prefix =
      "Failed to execute 'copyBufferToTexture' on 'GPUCommandEncoder'";
    webidl.requiredArguments(arguments.length, 3, prefix);
    source = webidl.converters.GPUImageCopyBuffer(source, prefix, "Argument 1");
    destination = webidl.converters.GPUImageCopyTexture(
      destination,
      prefix,
      "Argument 2",
    );
    copySize = webidl.converters.GPUExtent3D(copySize, prefix, "Argument 3");
    const device = assertDevice(this, prefix, "this");
    const commandEncoderRid = assertResource(this, prefix, "this");
    const sourceBufferRid = assertResource(
      // deno-lint-ignore prefer-primordials
      source.buffer,
      prefix,
      "source in Argument 1",
    );
    // deno-lint-ignore prefer-primordials
    assertDeviceMatch(device, source.buffer, {
      prefix,
      resourceContext: "source in Argument 1",
      selfContext: "this",
    });
    const destinationTextureRid = assertResource(
      destination.texture,
      prefix,
      "texture in Argument 2",
    );
    assertDeviceMatch(device, destination.texture, {
      prefix,
      resourceContext: "texture in Argument 2",
      selfContext: "this",
    });

    const { err } = op_webgpu_command_encoder_copy_buffer_to_texture(
      commandEncoderRid,
      {
        ...source,
        buffer: sourceBufferRid,
      },
      {
        texture: destinationTextureRid,
        mipLevel: destination.mipLevel,
        origin: destination.origin
          ? normalizeGPUOrigin3D(destination.origin)
          : undefined,
        aspect: destination.aspect,
      },
      normalizeGPUExtent3D(copySize),
    );
    device.pushError(err);
  }

  /**
   * @param {GPUImageCopyTexture} source
   * @param {GPUImageCopyBuffer} destination
   * @param {GPUExtent3D} copySize
   */
  copyTextureToBuffer(source, destination, copySize) {
    webidl.assertBranded(this, GPUCommandEncoderPrototype);
    const prefix =
      "Failed to execute 'copyTextureToBuffer' on 'GPUCommandEncoder'";
    webidl.requiredArguments(arguments.length, 3, prefix);
    source = webidl.converters.GPUImageCopyTexture(
      source,
      prefix,
      "Argument 1",
    );
    destination = webidl.converters.GPUImageCopyBuffer(
      destination,
      prefix,
      "Argument 2",
    );
    copySize = webidl.converters.GPUExtent3D(copySize, prefix, "Argument 3");
    const device = assertDevice(this, prefix, "this");
    const commandEncoderRid = assertResource(this, prefix, "this");
    const sourceTextureRid = assertResource(
      source.texture,
      prefix,
      "texture in Argument 1",
    );
    assertDeviceMatch(device, source.texture, {
      prefix,
      resourceContext: "texture in Argument 1",
      selfContext: "this",
    });
    const destinationBufferRid = assertResource(
      // deno-lint-ignore prefer-primordials
      destination.buffer,
      prefix,
      "buffer in Argument 2",
    );
    // deno-lint-ignore prefer-primordials
    assertDeviceMatch(device, destination.buffer, {
      prefix,
      resourceContext: "buffer in Argument 2",
      selfContext: "this",
    });
    const { err } = op_webgpu_command_encoder_copy_texture_to_buffer(
      commandEncoderRid,
      {
        texture: sourceTextureRid,
        mipLevel: source.mipLevel,
        origin: source.origin ? normalizeGPUOrigin3D(source.origin) : undefined,
        aspect: source.aspect,
      },
      {
        ...destination,
        buffer: destinationBufferRid,
      },
      normalizeGPUExtent3D(copySize),
    );
    device.pushError(err);
  }

  /**
   * @param {GPUImageCopyTexture} source
   * @param {GPUImageCopyTexture} destination
   * @param {GPUExtent3D} copySize
   */
  copyTextureToTexture(source, destination, copySize) {
    webidl.assertBranded(this, GPUCommandEncoderPrototype);
    const prefix =
      "Failed to execute 'copyTextureToTexture' on 'GPUCommandEncoder'";
    webidl.requiredArguments(arguments.length, 3, prefix);
    source = webidl.converters.GPUImageCopyTexture(
      source,
      prefix,
      "Argument 1",
    );
    destination = webidl.converters.GPUImageCopyTexture(
      destination,
      prefix,
      "Argument 2",
    );
    copySize = webidl.converters.GPUExtent3D(copySize, prefix, "Argument 3");
    const device = assertDevice(this, prefix, "this");
    const commandEncoderRid = assertResource(this, prefix, "this");
    const sourceTextureRid = assertResource(
      source.texture,
      prefix,
      "texture in Argument 1",
    );
    assertDeviceMatch(device, source.texture, {
      prefix,
      resourceContext: "texture in Argument 1",
      selfContext: "this",
    });
    const destinationTextureRid = assertResource(
      destination.texture,
      prefix,
      "texture in Argument 2",
    );
    assertDeviceMatch(device, destination.texture, {
      prefix,
      resourceContext: "texture in Argument 2",
      selfContext: "this",
    });
    const { err } = op_webgpu_command_encoder_copy_texture_to_texture(
      commandEncoderRid,
      {
        texture: sourceTextureRid,
        mipLevel: source.mipLevel,
        origin: source.origin ? normalizeGPUOrigin3D(source.origin) : undefined,
        aspect: source.aspect,
      },
      {
        texture: destinationTextureRid,
        mipLevel: destination.mipLevel,
        origin: destination.origin
          ? normalizeGPUOrigin3D(destination.origin)
          : undefined,
        aspect: source.aspect,
      },
      normalizeGPUExtent3D(copySize),
    );
    device.pushError(err);
  }

  /**
   * @param {GPUBuffer} buffer
   * @param {GPUSize64} offset
   * @param {GPUSize64} size
   */
  clearBuffer(buffer, offset = 0, size = undefined) {
    webidl.assertBranded(this, GPUCommandEncoderPrototype);
    const prefix = "Failed to execute 'clearBuffer' on 'GPUCommandEncoder'";
    webidl.requiredArguments(arguments.length, 3, prefix);
    buffer = webidl.converters.GPUBuffer(buffer, prefix, "Argument 1");
    offset = webidl.converters.GPUSize64(offset, prefix, "Argument 2");
    size = webidl.converters.GPUSize64(size, prefix, "Argument 3");
    const device = assertDevice(this, prefix, "this");
    const commandEncoderRid = assertResource(this, prefix, "this");
    const bufferRid = assertResource(buffer, prefix, "Argument 1");
    const { err } = op_webgpu_command_encoder_clear_buffer(
      commandEncoderRid,
      bufferRid,
      offset,
      size,
    );
    device.pushError(err);
  }

  /**
   * @param {string} groupLabel
   */
  pushDebugGroup(groupLabel) {
    webidl.assertBranded(this, GPUCommandEncoderPrototype);
    const prefix = "Failed to execute 'pushDebugGroup' on 'GPUCommandEncoder'";
    webidl.requiredArguments(arguments.length, 1, prefix);
    groupLabel = webidl.converters.USVString(groupLabel, prefix, "Argument 1");
    const device = assertDevice(this, prefix, "this");
    const commandEncoderRid = assertResource(this, prefix, "this");
    const { err } = op_webgpu_command_encoder_push_debug_group(
      commandEncoderRid,
      groupLabel,
    );
    device.pushError(err);
  }

  popDebugGroup() {
    webidl.assertBranded(this, GPUCommandEncoderPrototype);
    const prefix = "Failed to execute 'popDebugGroup' on 'GPUCommandEncoder'";
    const device = assertDevice(this, prefix, "this");
    const commandEncoderRid = assertResource(this, prefix, "this");
    const { err } = op_webgpu_command_encoder_pop_debug_group(
      commandEncoderRid,
    );
    device.pushError(err);
  }

  /**
   * @param {string} markerLabel
   */
  insertDebugMarker(markerLabel) {
    webidl.assertBranded(this, GPUCommandEncoderPrototype);
    const prefix =
      "Failed to execute 'insertDebugMarker' on 'GPUCommandEncoder'";
    webidl.requiredArguments(arguments.length, 1, prefix);
    markerLabel = webidl.converters.USVString(
      markerLabel,
      prefix,
      "Argument 1",
    );
    const device = assertDevice(this, prefix, "this");
    const commandEncoderRid = assertResource(this, prefix, "this");
    const { err } = op_webgpu_command_encoder_insert_debug_marker(
      commandEncoderRid,
      markerLabel,
    );
    device.pushError(err);
  }

  /**
   * @param {GPUQuerySet} querySet
   * @param {number} queryIndex
   */
  writeTimestamp(querySet, queryIndex) {
    webidl.assertBranded(this, GPUCommandEncoderPrototype);
    const prefix = "Failed to execute 'writeTimestamp' on 'GPUCommandEncoder'";
    webidl.requiredArguments(arguments.length, 2, prefix);
    querySet = webidl.converters.GPUQuerySet(querySet, prefix, "Argument 1");
    queryIndex = webidl.converters.GPUSize32(queryIndex, prefix, "Argument 2");
    const device = assertDevice(this, prefix, "this");
    const commandEncoderRid = assertResource(this, prefix, "this");
    const querySetRid = assertResource(querySet, prefix, "Argument 1");
    assertDeviceMatch(device, querySet, {
      prefix,
      resourceContext: "Argument 1",
      selfContext: "this",
    });
    const { err } = op_webgpu_command_encoder_write_timestamp(
      commandEncoderRid,
      querySetRid,
      queryIndex,
    );
    device.pushError(err);
  }

  /**
   * @param {GPUQuerySet} querySet
   * @param {number} firstQuery
   * @param {number} queryCount
   * @param {GPUBuffer} destination
   * @param {number} destinationOffset
   */
  resolveQuerySet(
    querySet,
    firstQuery,
    queryCount,
    destination,
    destinationOffset,
  ) {
    webidl.assertBranded(this, GPUCommandEncoderPrototype);
    const prefix = "Failed to execute 'resolveQuerySet' on 'GPUCommandEncoder'";
    webidl.requiredArguments(arguments.length, 5, { prefix });
    querySet = webidl.converters.GPUQuerySet(querySet, prefix, "Argument 1");
    firstQuery = webidl.converters.GPUSize32(firstQuery, prefix, "Argument 2");
    queryCount = webidl.converters.GPUSize32(queryCount, prefix, "Argument 3");
    destination = webidl.converters.GPUBuffer(
      destination,
      prefix,
      "Argument 4",
    );
    destinationOffset = webidl.converters.GPUSize64(
      destinationOffset,
      prefix,
      "Argument 5",
    );
    const device = assertDevice(this, prefix, "this");
    const commandEncoderRid = assertResource(this, prefix, "this");
    const querySetRid = assertResource(querySet, prefix, "Argument 1");
    assertDeviceMatch(device, querySet, {
      prefix,
      resourceContext: "Argument 1",
      selfContext: "this",
    });
    const destinationRid = assertResource(destination, prefix, "Argument 3");
    assertDeviceMatch(device, destination, {
      prefix,
      resourceContext: "Argument 3",
      selfContext: "this",
    });
    const { err } = op_webgpu_command_encoder_resolve_query_set(
      commandEncoderRid,
      querySetRid,
      firstQuery,
      queryCount,
      destinationRid,
      destinationOffset,
    );
    device.pushError(err);
  }

  /**
   * @param {GPUCommandBufferDescriptor} descriptor
   * @returns {GPUCommandBuffer}
   */
  finish(descriptor = { __proto__: null }) {
    webidl.assertBranded(this, GPUCommandEncoderPrototype);
    const prefix = "Failed to execute 'finish' on 'GPUCommandEncoder'";
    descriptor = webidl.converters.GPUCommandBufferDescriptor(
      descriptor,
      prefix,
      "Argument 1",
    );
    const device = assertDevice(this, prefix, "this");
    const commandEncoderRid = assertResource(this, prefix, "this");
    const { rid, err } = op_webgpu_command_encoder_finish(
      commandEncoderRid,
      descriptor.label,
    );
    device.pushError(err);
    /** @type {number | undefined} */
    this[_rid] = undefined;

    const commandBuffer = createGPUCommandBuffer(
      descriptor.label,
      device,
      rid,
    );
    device.trackResource(commandBuffer);
    return commandBuffer;
  }

  [SymbolFor("Deno.privateCustomInspect")](inspect, inspectOptions) {
    return inspect(
      createFilteredInspectProxy({
        object: this,
        evaluate: ObjectPrototypeIsPrototypeOf(
          GPUCommandEncoderPrototype,
          this,
        ),
        keys: [
          "label",
        ],
      }),
      inspectOptions,
    );
  }
}
GPUObjectBaseMixin("GPUCommandEncoder", GPUCommandEncoder);
const GPUCommandEncoderPrototype = GPUCommandEncoder.prototype;

/**
 * @param {string | null} label
 * @param {GPUCommandEncoder} encoder
 * @param {number} rid
 * @returns {GPURenderPassEncoder}
 */
function createGPURenderPassEncoder(label, encoder, rid) {
  /** @type {GPURenderPassEncoder} */
  const passEncoder = webidl.createBranded(GPURenderPassEncoder);
  passEncoder[_label] = label;
  passEncoder[_encoder] = encoder;
  passEncoder[_rid] = rid;
  return passEncoder;
}

class GPURenderPassEncoder {
  /** @type {GPUCommandEncoder} */
  [_encoder];
  /** @type {number | undefined} */
  [_rid];

  [_cleanup]() {
    const rid = this[_rid];
    if (rid !== undefined) {
      core.close(rid);
      /** @type {number | undefined} */
      this[_rid] = undefined;
    }
  }

  constructor() {
    webidl.illegalConstructor();
  }

  /**
   * @param {number} x
   * @param {number} y
   * @param {number} width
   * @param {number} height
   * @param {number} minDepth
   * @param {number} maxDepth
   */
  setViewport(x, y, width, height, minDepth, maxDepth) {
    webidl.assertBranded(this, GPURenderPassEncoderPrototype);
    const prefix = "Failed to execute 'setViewport' on 'GPUComputePassEncoder'";
    webidl.requiredArguments(arguments.length, 6, { prefix });
    x = webidl.converters.float(x, prefix, "Argument 1");
    y = webidl.converters.float(y, prefix, "Argument 2");
    width = webidl.converters.float(width, prefix, "Argument 3");
    height = webidl.converters.float(height, prefix, "Argument 4");
    minDepth = webidl.converters.float(minDepth, prefix, "Argument 5");
    maxDepth = webidl.converters.float(maxDepth, prefix, "Argument 6");
    assertDevice(this[_encoder], prefix, "encoder referenced by this");
    assertResource(this[_encoder], prefix, "encoder referenced by this");
    const renderPassRid = assertResource(this, prefix, "this");
    op_webgpu_render_pass_set_viewport({
      renderPassRid,
      x,
      y,
      width,
      height,
      minDepth,
      maxDepth,
    });
  }

  /**
   * @param {number} x
   * @param {number} y
   * @param {number} width
   * @param {number} height
   */
  setScissorRect(x, y, width, height) {
    webidl.assertBranded(this, GPURenderPassEncoderPrototype);
    const prefix =
      "Failed to execute 'setScissorRect' on 'GPUComputePassEncoder'";
    webidl.requiredArguments(arguments.length, 4, prefix);
    x = webidl.converters.GPUIntegerCoordinate(x, prefix, "Argument 1");
    y = webidl.converters.GPUIntegerCoordinate(y, prefix, "Argument 2");
    width = webidl.converters.GPUIntegerCoordinate(width, prefix, "Argument 3");
    height = webidl.converters.GPUIntegerCoordinate(
      height,
      prefix,
      "Argument 4",
    );
    assertDevice(this[_encoder], prefix, "encoder referenced by this");
    assertResource(this[_encoder], prefix, "encoder referenced by this");
    const renderPassRid = assertResource(this, prefix, "this");
    op_webgpu_render_pass_set_scissor_rect(
      renderPassRid,
      x,
      y,
      width,
      height,
    );
  }

  /**
   * @param {GPUColor} color
   */
  setBlendConstant(color) {
    webidl.assertBranded(this, GPURenderPassEncoderPrototype);
    const prefix =
      "Failed to execute 'setBlendConstant' on 'GPUComputePassEncoder'";
    webidl.requiredArguments(arguments.length, 1, prefix);
    color = webidl.converters.GPUColor(color, prefix, "Argument 1");
    assertDevice(this[_encoder], prefix, "encoder referenced by this");
    assertResource(this[_encoder], prefix, "encoder referenced by this");
    const renderPassRid = assertResource(this, prefix, "this");
    op_webgpu_render_pass_set_blend_constant(
      renderPassRid,
      normalizeGPUColor(color),
    );
  }

  /**
   * @param {number} reference
   */
  setStencilReference(reference) {
    webidl.assertBranded(this, GPURenderPassEncoderPrototype);
    const prefix =
      "Failed to execute 'setStencilReference' on 'GPUComputePassEncoder'";
    webidl.requiredArguments(arguments.length, 1, prefix);
    reference = webidl.converters.GPUStencilValue(
      reference,
      prefix,
      "Argument 1",
    );
    assertDevice(this[_encoder], prefix, "encoder referenced by this");
    assertResource(this[_encoder], prefix, "encoder referenced by this");
    const renderPassRid = assertResource(this, prefix, "this");
    op_webgpu_render_pass_set_stencil_reference(
      renderPassRid,
      reference,
    );
  }

  /**
   * @param {number} queryIndex
   */
  beginOcclusionQuery(queryIndex) {
    webidl.assertBranded(this, GPURenderPassEncoderPrototype);
    const prefix =
      "Failed to execute 'beginOcclusionQuery' on 'GPUComputePassEncoder'";
    webidl.requiredArguments(arguments.length, 1, prefix);
    queryIndex = webidl.converters.GPUSize32(queryIndex, prefix, "Argument 1");
    assertDevice(this[_encoder], prefix, "encoder referenced by this");
    assertResource(this[_encoder], prefix, "encoder referenced by this");
    const renderPassRid = assertResource(this, prefix, "this");
    op_webgpu_render_pass_begin_occlusion_query(
      renderPassRid,
      queryIndex,
    );
  }

  endOcclusionQuery() {
    webidl.assertBranded(this, GPURenderPassEncoderPrototype);
    const prefix =
      "Failed to execute 'endOcclusionQuery' on 'GPUComputePassEncoder'";
    assertDevice(this[_encoder], prefix, "encoder referenced by this");
    assertResource(this[_encoder], prefix, "encoder referenced by this");
    const renderPassRid = assertResource(this, prefix, "this");
    op_webgpu_render_pass_end_occlusion_query(renderPassRid);
  }

  /**
   * @param {GPURenderBundle[]} bundles
   */
  executeBundles(bundles) {
    webidl.assertBranded(this, GPURenderPassEncoderPrototype);
    const prefix =
      "Failed to execute 'executeBundles' on 'GPURenderPassEncoder'";
    webidl.requiredArguments(arguments.length, 1, prefix);
    bundles = webidl.converters["sequence<GPURenderBundle>"](
      bundles,
      prefix,
      "Argument 1",
    );
    const device = assertDevice(
      this[_encoder],
      prefix,
      "encoder referenced by this",
    );
    assertResource(this[_encoder], prefix, "encoder referenced by this");
    const renderPassRid = assertResource(this, prefix, "this");
    const bundleRids = ArrayPrototypeMap(bundles, (bundle, i) => {
      const context = `bundle ${i + 1}`;
      const rid = assertResource(bundle, prefix, context);
      assertDeviceMatch(device, bundle, {
        prefix,
        resourceContext: context,
        selfContext: "this",
      });
      return rid;
    });
    op_webgpu_render_pass_execute_bundles(renderPassRid, bundleRids);
  }

  end() {
    webidl.assertBranded(this, GPURenderPassEncoderPrototype);
    const prefix = "Failed to execute 'end' on 'GPURenderPassEncoder'";
    const device = assertDevice(
      this[_encoder],
      prefix,
      "encoder referenced by this",
    );
    const commandEncoderRid = assertResource(
      this[_encoder],
      prefix,
      "encoder referenced by this",
    );
    const renderPassRid = assertResource(this, prefix, "this");
    const { err } = op_webgpu_render_pass_end(
      commandEncoderRid,
      renderPassRid,
    );
    device.pushError(err);
    this[_rid] = undefined;
  }

  // TODO(lucacasonato): has an overload
  setBindGroup(
    index,
    bindGroup,
    dynamicOffsetsData,
    dynamicOffsetsDataStart,
    dynamicOffsetsDataLength,
  ) {
    webidl.assertBranded(this, GPURenderPassEncoderPrototype);
    const prefix = "Failed to execute 'setBindGroup' on 'GPURenderPassEncoder'";
    const device = assertDevice(
      this[_encoder],
      prefix,
      "encoder referenced by this",
    );
    assertResource(this[_encoder], prefix, "encoder referenced by this");
    const renderPassRid = assertResource(this, prefix, "this");
    const bindGroupRid = assertResource(bindGroup, prefix, "Argument 2");
    assertDeviceMatch(device, bindGroup, {
      prefix,
      resourceContext: "Argument 2",
      selfContext: "this",
    });
    if (
      TypedArrayPrototypeGetSymbolToStringTag(dynamicOffsetsData) !==
        "Uint32Array"
    ) {
      dynamicOffsetsData = new Uint32Array(dynamicOffsetsData ?? []);
      dynamicOffsetsDataStart = 0;
      dynamicOffsetsDataLength = dynamicOffsetsData.length;
    }
    op_webgpu_render_pass_set_bind_group(
      renderPassRid,
      index,
      bindGroupRid,
      dynamicOffsetsData,
      dynamicOffsetsDataStart,
      dynamicOffsetsDataLength,
    );
  }

  /**
   * @param {string} groupLabel
   */
  pushDebugGroup(groupLabel) {
    webidl.assertBranded(this, GPURenderPassEncoderPrototype);
    const prefix =
      "Failed to execute 'pushDebugGroup' on 'GPURenderPassEncoder'";
    webidl.requiredArguments(arguments.length, 1, prefix);
    groupLabel = webidl.converters.USVString(groupLabel, prefix, "Argument 1");
    assertDevice(this[_encoder], prefix, "encoder referenced by this");
    assertResource(this[_encoder], prefix, "encoder referenced by this");
    const renderPassRid = assertResource(this, prefix, "this");
    op_webgpu_render_pass_push_debug_group(renderPassRid, groupLabel);
  }

  popDebugGroup() {
    webidl.assertBranded(this, GPURenderPassEncoderPrototype);
    const prefix =
      "Failed to execute 'popDebugGroup' on 'GPURenderPassEncoder'";
    assertDevice(this[_encoder], prefix, "encoder referenced by this");
    assertResource(this[_encoder], prefix, "encoder referenced by this");
    const renderPassRid = assertResource(this, prefix, "this");
    op_webgpu_render_pass_pop_debug_group(renderPassRid);
  }

  /**
   * @param {string} markerLabel
   */
  insertDebugMarker(markerLabel) {
    webidl.assertBranded(this, GPURenderPassEncoderPrototype);
    const prefix =
      "Failed to execute 'insertDebugMarker' on 'GPURenderPassEncoder'";
    webidl.requiredArguments(arguments.length, 1, prefix);
    markerLabel = webidl.converters.USVString(
      markerLabel,
      prefix,
      "Argument 1",
    );
    assertDevice(this[_encoder], prefix, "encoder referenced by this");
    assertResource(this[_encoder], prefix, "encoder referenced by this");
    const renderPassRid = assertResource(this, prefix, "this");
    op_webgpu_render_pass_insert_debug_marker(renderPassRid, markerLabel);
  }

  /**
   * @param {GPURenderPipeline} pipeline
   */
  setPipeline(pipeline) {
    webidl.assertBranded(this, GPURenderPassEncoderPrototype);
    const prefix = "Failed to execute 'setPipeline' on 'GPURenderPassEncoder'";
    webidl.requiredArguments(arguments.length, 1, prefix);
    pipeline = webidl.converters.GPURenderPipeline(
      pipeline,
      prefix,
      "Argument 1",
    );
    const device = assertDevice(
      this[_encoder],
      prefix,
      "encoder referenced by this",
    );
    assertResource(this[_encoder], prefix, "encoder referenced by this");
    const renderPassRid = assertResource(this, prefix, "this");
    const pipelineRid = assertResource(pipeline, prefix, "Argument 1");
    assertDeviceMatch(device, pipeline, {
      prefix,
      resourceContext: "Argument 1",
      selfContext: "this",
    });
    op_webgpu_render_pass_set_pipeline(renderPassRid, pipelineRid);
  }

  /**
   * @param {GPUBuffer} buffer
   * @param {GPUIndexFormat} indexFormat
   * @param {number} offset
   * @param {number} size
   */
  setIndexBuffer(buffer, indexFormat, offset = 0, size) {
    webidl.assertBranded(this, GPURenderPassEncoderPrototype);
    const prefix =
      "Failed to execute 'setIndexBuffer' on 'GPURenderPassEncoder'";
    webidl.requiredArguments(arguments.length, 2, prefix);
    buffer = webidl.converters.GPUBuffer(buffer, prefix, "Argument 1");
    indexFormat = webidl.converters.GPUIndexFormat(
      indexFormat,
      prefix,
      "Argument 2",
    );
    offset = webidl.converters.GPUSize64(offset, prefix, "Argument 3");
    if (size !== undefined) {
      size = webidl.converters.GPUSize64(size, prefix, "Argument 4");
    }
    const device = assertDevice(
      this[_encoder],
      prefix,
      "encoder referenced by this",
    );
    assertResource(this[_encoder], prefix, "encoder referenced by this");
    const renderPassRid = assertResource(this, prefix, "this");
    const bufferRid = assertResource(buffer, prefix, "Argument 1");
    assertDeviceMatch(device, buffer, {
      prefix,
      resourceContext: "Argument 1",
      selfContext: "this",
    });
    op_webgpu_render_pass_set_index_buffer(
      renderPassRid,
      bufferRid,
      indexFormat,
      offset,
      size,
    );
  }

  /**
   * @param {number} slot
   * @param {GPUBuffer} buffer
   * @param {number} offset
   * @param {number} size
   */
  setVertexBuffer(slot, buffer, offset = 0, size) {
    webidl.assertBranded(this, GPURenderPassEncoderPrototype);
    const prefix =
      "Failed to execute 'setVertexBuffer' on 'GPURenderPassEncoder'";
    webidl.requiredArguments(arguments.length, 2, prefix);
    slot = webidl.converters.GPUSize32(slot, prefix, "Argument 1");
    buffer = webidl.converters.GPUBuffer(buffer, prefix, "Argument 2");
    offset = webidl.converters.GPUSize64(offset, prefix, "Argument 3");
    if (size !== undefined) {
      size = webidl.converters.GPUSize64(size, prefix, "Argument 4");
    }
    const device = assertDevice(
      this[_encoder],
      prefix,
      "encoder referenced by this",
    );
    assertResource(this[_encoder], prefix, "encoder referenced by this");
    const renderPassRid = assertResource(this, prefix, "this");
    const bufferRid = assertResource(buffer, prefix, "Argument 2");
    assertDeviceMatch(device, buffer, {
      prefix,
      resourceContext: "Argument 2",
      selfContext: "this",
    });
    op_webgpu_render_pass_set_vertex_buffer(
      renderPassRid,
      slot,
      bufferRid,
      offset,
      size,
    );
  }

  /**
   * @param {number} vertexCount
   * @param {number} instanceCount
   * @param {number} firstVertex
   * @param {number} firstInstance
   */
  draw(vertexCount, instanceCount = 1, firstVertex = 0, firstInstance = 0) {
    webidl.assertBranded(this, GPURenderPassEncoderPrototype);
    const prefix = "Failed to execute 'draw' on 'GPURenderPassEncoder'";
    webidl.requiredArguments(arguments.length, 1, prefix);
    vertexCount = webidl.converters.GPUSize32(
      vertexCount,
      prefix,
      "Argument 1",
    );
    instanceCount = webidl.converters.GPUSize32(
      instanceCount,
      prefix,
      "Argument 2",
    );
    firstVertex = webidl.converters.GPUSize32(
      firstVertex,
      prefix,
      "Argument 3",
    );
    firstInstance = webidl.converters.GPUSize32(
      firstInstance,
      prefix,
      "Argument 4",
    );
    assertDevice(this[_encoder], prefix, "encoder referenced by this");
    assertResource(this[_encoder], prefix, "encoder referenced by this");
    const renderPassRid = assertResource(this, prefix, "this");
    op_webgpu_render_pass_draw(
      renderPassRid,
      vertexCount,
      instanceCount,
      firstVertex,
      firstInstance,
    );
  }

  /**
   * @param {number} indexCount
   * @param {number} instanceCount
   * @param {number} firstIndex
   * @param {number} baseVertex
   * @param {number} firstInstance
   */
  drawIndexed(
    indexCount,
    instanceCount = 1,
    firstIndex = 0,
    baseVertex = 0,
    firstInstance = 0,
  ) {
    webidl.assertBranded(this, GPURenderPassEncoderPrototype);
    const prefix = "Failed to execute 'drawIndexed' on 'GPURenderPassEncoder'";
    webidl.requiredArguments(arguments.length, 1, prefix);
    indexCount = webidl.converters.GPUSize32(indexCount, prefix, "Argument 1");
    instanceCount = webidl.converters.GPUSize32(
      instanceCount,
      prefix,
      "Argument 2",
    );
    firstIndex = webidl.converters.GPUSize32(firstIndex, prefix, "Argument 3");
    baseVertex = webidl.converters.GPUSignedOffset32(
      baseVertex,
      prefix,
      "Argument 4",
    );
    firstInstance = webidl.converters.GPUSize32(
      firstInstance,
      prefix,
      "Argument 5",
    );
    assertDevice(this[_encoder], prefix, "encoder referenced by this");
    assertResource(this[_encoder], prefix, "encoder referenced by this");
    const renderPassRid = assertResource(this, prefix, "this");
    op_webgpu_render_pass_draw_indexed(
      renderPassRid,
      indexCount,
      instanceCount,
      firstIndex,
      baseVertex,
      firstInstance,
    );
  }

  /**
   * @param {GPUBuffer} indirectBuffer
   * @param {number} indirectOffset
   */
  drawIndirect(indirectBuffer, indirectOffset) {
    webidl.assertBranded(this, GPURenderPassEncoderPrototype);
    const prefix = "Failed to execute 'drawIndirect' on 'GPURenderPassEncoder'";
    webidl.requiredArguments(arguments.length, 2, prefix);
    indirectBuffer = webidl.converters.GPUBuffer(
      indirectBuffer,
      prefix,
      "Argument 1",
    );
    indirectOffset = webidl.converters.GPUSize64(
      indirectOffset,
      prefix,
      "Argument 2",
    );
    const device = assertDevice(
      this[_encoder],
      prefix,
      "encoder referenced by this",
    );
    assertResource(this[_encoder], prefix, "encoder referenced by this");
    const renderPassRid = assertResource(this, prefix, "this");
    const indirectBufferRid = assertResource(
      indirectBuffer,
      prefix,
      "Argument 1",
    );
    assertDeviceMatch(device, indirectBuffer, {
      prefix,
      resourceContext: "Argument 1",
      selfContext: "this",
    });
    op_webgpu_render_pass_draw_indirect(
      renderPassRid,
      indirectBufferRid,
      indirectOffset,
    );
  }

  /**
   * @param {GPUBuffer} indirectBuffer
   * @param {number} indirectOffset
   */
  drawIndexedIndirect(indirectBuffer, indirectOffset) {
    webidl.assertBranded(this, GPURenderPassEncoderPrototype);
    const prefix =
      "Failed to execute 'drawIndexedIndirect' on 'GPURenderPassEncoder'";
    webidl.requiredArguments(arguments.length, 2, prefix);
    indirectBuffer = webidl.converters.GPUBuffer(
      indirectBuffer,
      prefix,
      "Argument 1",
    );
    indirectOffset = webidl.converters.GPUSize64(
      indirectOffset,
      prefix,
      "Argument 2",
    );
    const device = assertDevice(
      this[_encoder],
      prefix,
      "encoder referenced by this",
    );
    assertResource(this[_encoder], prefix, "encoder referenced by this");
    const renderPassRid = assertResource(this, prefix, "this");
    const indirectBufferRid = assertResource(
      indirectBuffer,
      prefix,
      "Argument 1",
    );
    assertDeviceMatch(device, indirectBuffer, {
      prefix,
      resourceContext: "Argument 1",
      selfContext: "this",
    });
    op_webgpu_render_pass_draw_indexed_indirect(
      renderPassRid,
      indirectBufferRid,
      indirectOffset,
    );
  }

  [SymbolFor("Deno.privateCustomInspect")](inspect, inspectOptions) {
    return inspect(
      createFilteredInspectProxy({
        object: this,
        evaluate: ObjectPrototypeIsPrototypeOf(
          GPURenderPassEncoderPrototype,
          this,
        ),
        keys: [
          "label",
        ],
      }),
      inspectOptions,
    );
  }
}
GPUObjectBaseMixin("GPURenderPassEncoder", GPURenderPassEncoder);
const GPURenderPassEncoderPrototype = GPURenderPassEncoder.prototype;

/**
 * @param {string | null} label
 * @param {GPUCommandEncoder} encoder
 * @param {number} rid
 * @returns {GPUComputePassEncoder}
 */
function createGPUComputePassEncoder(label, encoder, rid) {
  /** @type {GPUComputePassEncoder} */
  const computePassEncoder = webidl.createBranded(GPUComputePassEncoder);
  computePassEncoder[_label] = label;
  computePassEncoder[_encoder] = encoder;
  computePassEncoder[_rid] = rid;
  return computePassEncoder;
}

class GPUComputePassEncoder {
  /** @type {GPUCommandEncoder} */
  [_encoder];

  /** @type {number | undefined} */
  [_rid];

  [_cleanup]() {
    const rid = this[_rid];
    if (rid !== undefined) {
      core.close(rid);
      /** @type {number | undefined} */
      this[_rid] = undefined;
    }
  }

  constructor() {
    webidl.illegalConstructor();
  }

  /**
   * @param {GPUComputePipeline} pipeline
   */
  setPipeline(pipeline) {
    webidl.assertBranded(this, GPUComputePassEncoderPrototype);
    const prefix = "Failed to execute 'setPipeline' on 'GPUComputePassEncoder'";
    webidl.requiredArguments(arguments.length, 1, prefix);
    pipeline = webidl.converters.GPUComputePipeline(
      pipeline,
      prefix,
      "Argument 1",
    );
    const device = assertDevice(
      this[_encoder],
      prefix,
      "encoder referenced by this",
    );
    assertResource(this[_encoder], prefix, "encoder referenced by this");
    const computePassRid = assertResource(this, prefix, "this");
    const pipelineRid = assertResource(pipeline, prefix, "Argument 1");
    assertDeviceMatch(device, pipeline, {
      prefix,
      resourceContext: "Argument 1",
      selfContext: "this",
    });
    op_webgpu_compute_pass_set_pipeline(computePassRid, pipelineRid);
  }

  /**
   * @param {number} workgroupCountX
   * @param {number} workgroupCountY
   * @param {number} workgroupCountZ
   */
  dispatchWorkgroups(
    workgroupCountX,
    workgroupCountY = 1,
    workgroupCountZ = 1,
  ) {
    webidl.assertBranded(this, GPUComputePassEncoderPrototype);
    const prefix =
      "Failed to execute 'dispatchWorkgroups' on 'GPUComputePassEncoder'";
    webidl.requiredArguments(arguments.length, 1, prefix);
    workgroupCountX = webidl.converters.GPUSize32(
      workgroupCountX,
      prefix,
      "Argument 1",
    );
    workgroupCountY = webidl.converters.GPUSize32(
      workgroupCountY,
      prefix,
      "Argument 2",
    );
    workgroupCountZ = webidl.converters.GPUSize32(
      workgroupCountZ,
      prefix,
      "Argument 3",
    );
    assertDevice(this[_encoder], prefix, "encoder referenced by this");
    assertResource(this[_encoder], prefix, "encoder referenced by this");
    const computePassRid = assertResource(this, prefix, "this");
    op_webgpu_compute_pass_dispatch_workgroups(
      computePassRid,
      workgroupCountX,
      workgroupCountY,
      workgroupCountZ,
    );
  }

  /**
   * @param {GPUBuffer} indirectBuffer
   * @param {number} indirectOffset
   */
  dispatchWorkgroupsIndirect(indirectBuffer, indirectOffset) {
    webidl.assertBranded(this, GPUComputePassEncoderPrototype);
    const prefix =
      "Failed to execute 'dispatchWorkgroupsIndirect' on 'GPUComputePassEncoder'";
    webidl.requiredArguments(arguments.length, 2, prefix);
    indirectBuffer = webidl.converters.GPUBuffer(
      indirectBuffer,
      prefix,
      "Argument 1",
    );
    indirectOffset = webidl.converters.GPUSize64(
      indirectOffset,
      prefix,
      "Argument 2",
    );
    const device = assertDevice(
      this[_encoder],
      prefix,
      "encoder referenced by this",
    );
    assertResource(this[_encoder], prefix, "encoder referenced by this");
    const computePassRid = assertResource(this, prefix, "this");
    const indirectBufferRid = assertResource(
      indirectBuffer,
      prefix,
      "Argument 1",
    );
    assertDeviceMatch(device, indirectBuffer, {
      prefix,
      resourceContext: "Argument 1",
      selfContext: "this",
    });
    op_webgpu_compute_pass_dispatch_workgroups_indirect(
      computePassRid,
      indirectBufferRid,
      indirectOffset,
    );
  }

  end() {
    webidl.assertBranded(this, GPUComputePassEncoderPrototype);
    const prefix = "Failed to execute 'end' on 'GPUComputePassEncoder'";
    const device = assertDevice(
      this[_encoder],
      prefix,
      "encoder referenced by this",
    );
    const commandEncoderRid = assertResource(
      this[_encoder],
      prefix,
      "encoder referenced by this",
    );
    const computePassRid = assertResource(this, prefix, "this");
    const { err } = op_webgpu_compute_pass_end(
      commandEncoderRid,
      computePassRid,
    );
    device.pushError(err);
    this[_rid] = undefined;
  }

  // TODO(lucacasonato): has an overload
  setBindGroup(
    index,
    bindGroup,
    dynamicOffsetsData,
    dynamicOffsetsDataStart,
    dynamicOffsetsDataLength,
  ) {
    webidl.assertBranded(this, GPUComputePassEncoderPrototype);
    const prefix =
      "Failed to execute 'setBindGroup' on 'GPUComputePassEncoder'";
    const device = assertDevice(
      this[_encoder],
      prefix,
      "encoder referenced by this",
    );
    assertResource(this[_encoder], prefix, "encoder referenced by this");
    const computePassRid = assertResource(this, prefix, "this");
    const bindGroupRid = assertResource(bindGroup, prefix, "Argument 2");
    assertDeviceMatch(device, bindGroup, {
      prefix,
      resourceContext: "Argument 2",
      selfContext: "this",
    });
    if (
      TypedArrayPrototypeGetSymbolToStringTag(dynamicOffsetsData) !==
        "Uint32Array"
    ) {
      dynamicOffsetsData = new Uint32Array(dynamicOffsetsData ?? []);
      dynamicOffsetsDataStart = 0;
      dynamicOffsetsDataLength = dynamicOffsetsData.length;
    }
    op_webgpu_compute_pass_set_bind_group(
      computePassRid,
      index,
      bindGroupRid,
      dynamicOffsetsData,
      dynamicOffsetsDataStart,
      dynamicOffsetsDataLength,
    );
  }

  /**
   * @param {string} groupLabel
   */
  pushDebugGroup(groupLabel) {
    webidl.assertBranded(this, GPUComputePassEncoderPrototype);
    const prefix =
      "Failed to execute 'pushDebugGroup' on 'GPUComputePassEncoder'";
    webidl.requiredArguments(arguments.length, 1, prefix);
    groupLabel = webidl.converters.USVString(groupLabel, prefix, "Argument 1");
    assertDevice(this[_encoder], prefix, "encoder referenced by this");
    assertResource(this[_encoder], prefix, "encoder referenced by this");
    const computePassRid = assertResource(this, prefix, "this");
    op_webgpu_compute_pass_push_debug_group(computePassRid, groupLabel);
  }

  popDebugGroup() {
    webidl.assertBranded(this, GPUComputePassEncoderPrototype);
    const prefix =
      "Failed to execute 'popDebugGroup' on 'GPUComputePassEncoder'";
    assertDevice(this[_encoder], prefix, "encoder referenced by this");
    assertResource(this[_encoder], prefix, "encoder referenced by this");
    const computePassRid = assertResource(this, prefix, "this");
    op_webgpu_compute_pass_pop_debug_group(computePassRid);
  }

  /**
   * @param {string} markerLabel
   */
  insertDebugMarker(markerLabel) {
    webidl.assertBranded(this, GPUComputePassEncoderPrototype);
    const prefix =
      "Failed to execute 'insertDebugMarker' on 'GPUComputePassEncoder'";
    webidl.requiredArguments(arguments.length, 1, prefix);
    markerLabel = webidl.converters.USVString(
      markerLabel,
      prefix,
      "Argument 1",
    );
    assertDevice(this[_encoder], prefix, "encoder referenced by this");
    assertResource(this[_encoder], prefix, "encoder referenced by this");
    const computePassRid = assertResource(this, prefix, "this");
    op_webgpu_compute_pass_insert_debug_marker(
      computePassRid,
      markerLabel,
    );
  }

  [SymbolFor("Deno.privateCustomInspect")](inspect, inspectOptions) {
    return inspect(
      createFilteredInspectProxy({
        object: this,
        evaluate: ObjectPrototypeIsPrototypeOf(
          GPUComputePassEncoderPrototype,
          this,
        ),
        keys: [
          "label",
        ],
      }),
      inspectOptions,
    );
  }
}
GPUObjectBaseMixin("GPUComputePassEncoder", GPUComputePassEncoder);
const GPUComputePassEncoderPrototype = GPUComputePassEncoder.prototype;

/**
 * @param {string | null} label
 * @param {InnerGPUDevice} device
 * @param {number} rid
 * @returns {GPUCommandBuffer}
 */
function createGPUCommandBuffer(label, device, rid) {
  /** @type {GPUCommandBuffer} */
  const commandBuffer = webidl.createBranded(GPUCommandBuffer);
  commandBuffer[_label] = label;
  commandBuffer[_device] = device;
  commandBuffer[_rid] = rid;
  return commandBuffer;
}

class GPUCommandBuffer {
  /** @type {InnerGPUDevice} */
  [_device];
  /** @type {number | undefined} */
  [_rid];

  [_cleanup]() {
    const rid = this[_rid];
    if (rid !== undefined) {
      core.close(rid);
      /** @type {number | undefined} */
      this[_rid] = undefined;
    }
  }

  constructor() {
    webidl.illegalConstructor();
  }

  [SymbolFor("Deno.privateCustomInspect")](inspect, inspectOptions) {
    return inspect(
      createFilteredInspectProxy({
        object: this,
        evaluate: ObjectPrototypeIsPrototypeOf(GPUCommandBufferPrototype, this),
        keys: [
          "label",
        ],
      }),
      inspectOptions,
    );
  }
}
GPUObjectBaseMixin("GPUCommandBuffer", GPUCommandBuffer);
const GPUCommandBufferPrototype = GPUCommandBuffer.prototype;

/**
 * @param {string | null} label
 * @param {InnerGPUDevice} device
 * @param {number} rid
 * @returns {GPURenderBundleEncoder}
 */
function createGPURenderBundleEncoder(label, device, rid) {
  /** @type {GPURenderBundleEncoder} */
  const bundleEncoder = webidl.createBranded(GPURenderBundleEncoder);
  bundleEncoder[_label] = label;
  bundleEncoder[_device] = device;
  bundleEncoder[_rid] = rid;
  return bundleEncoder;
}

class GPURenderBundleEncoder {
  /** @type {InnerGPUDevice} */
  [_device];
  /** @type {number | undefined} */
  [_rid];

  [_cleanup]() {
    const rid = this[_rid];
    if (rid !== undefined) {
      core.close(rid);
      /** @type {number | undefined} */
      this[_rid] = undefined;
    }
  }

  constructor() {
    webidl.illegalConstructor();
  }

  /**
   * @param {GPURenderBundleDescriptor} descriptor
   */
  finish(descriptor = { __proto__: null }) {
    webidl.assertBranded(this, GPURenderBundleEncoderPrototype);
    const prefix = "Failed to execute 'finish' on 'GPURenderBundleEncoder'";
    descriptor = webidl.converters.GPURenderBundleDescriptor(
      descriptor,
      prefix,
      "Argument 1",
    );
    const device = assertDevice(this, prefix, "this");
    const renderBundleEncoderRid = assertResource(this, prefix, "this");
    const { rid, err } = op_webgpu_render_bundle_encoder_finish(
      renderBundleEncoderRid,
      descriptor.label,
    );
    device.pushError(err);
    this[_rid] = undefined;

    const renderBundle = createGPURenderBundle(
      descriptor.label,
      device,
      rid,
    );
    device.trackResource(renderBundle);
    return renderBundle;
  }

  // TODO(lucacasonato): has an overload
  setBindGroup(
    index,
    bindGroup,
    dynamicOffsetsData,
    dynamicOffsetsDataStart,
    dynamicOffsetsDataLength,
  ) {
    webidl.assertBranded(this, GPURenderBundleEncoderPrototype);
    const prefix =
      "Failed to execute 'setBindGroup' on 'GPURenderBundleEncoder'";
    const device = assertDevice(this, prefix, "this");
    const renderBundleEncoderRid = assertResource(this, prefix, "this");
    const bindGroupRid = assertResource(bindGroup, prefix, "Argument 2");
    assertDeviceMatch(device, bindGroup, {
      prefix,
      resourceContext: "Argument 2",
      selfContext: "this",
    });
    if (
      TypedArrayPrototypeGetSymbolToStringTag(dynamicOffsetsData) !==
        "Uint32Array"
    ) {
      dynamicOffsetsData = new Uint32Array(dynamicOffsetsData ?? []);
      dynamicOffsetsDataStart = 0;
      dynamicOffsetsDataLength = dynamicOffsetsData.length;
    }
    op_webgpu_render_bundle_encoder_set_bind_group(
      renderBundleEncoderRid,
      index,
      bindGroupRid,
      dynamicOffsetsData,
      dynamicOffsetsDataStart,
      dynamicOffsetsDataLength,
    );
  }

  /**
   * @param {string} groupLabel
   */
  pushDebugGroup(groupLabel) {
    webidl.assertBranded(this, GPURenderBundleEncoderPrototype);
    const prefix =
      "Failed to execute 'pushDebugGroup' on 'GPURenderBundleEncoder'";
    webidl.requiredArguments(arguments.length, 1, prefix);
    groupLabel = webidl.converters.USVString(groupLabel, prefix, "Argument 1");
    assertDevice(this, prefix, "this");
    const renderBundleEncoderRid = assertResource(this, prefix, "this");
    op_webgpu_render_bundle_encoder_push_debug_group(
      renderBundleEncoderRid,
      groupLabel,
    );
  }

  popDebugGroup() {
    webidl.assertBranded(this, GPURenderBundleEncoderPrototype);
    const prefix =
      "Failed to execute 'popDebugGroup' on 'GPURenderBundleEncoder'";
    assertDevice(this, prefix, "this");
    const renderBundleEncoderRid = assertResource(this, prefix, "this");
    op_webgpu_render_bundle_encoder_pop_debug_group(
      renderBundleEncoderRid,
    );
  }

  /**
   * @param {string} markerLabel
   */
  insertDebugMarker(markerLabel) {
    webidl.assertBranded(this, GPURenderBundleEncoderPrototype);
    const prefix =
      "Failed to execute 'insertDebugMarker' on 'GPURenderBundleEncoder'";
    webidl.requiredArguments(arguments.length, 1, prefix);
    markerLabel = webidl.converters.USVString(
      markerLabel,
      prefix,
      "Argument 1",
    );
    assertDevice(this, prefix, "this");
    const renderBundleEncoderRid = assertResource(this, prefix, "this");
    op_webgpu_render_bundle_encoder_insert_debug_marker(
      renderBundleEncoderRid,
      markerLabel,
    );
  }

  /**
   * @param {GPURenderPipeline} pipeline
   */
  setPipeline(pipeline) {
    webidl.assertBranded(this, GPURenderBundleEncoderPrototype);
    const prefix =
      "Failed to execute 'setPipeline' on 'GPURenderBundleEncoder'";
    webidl.requiredArguments(arguments.length, 1, prefix);
    pipeline = webidl.converters.GPURenderPipeline(
      pipeline,
      prefix,
      "Argument 1",
    );
    const device = assertDevice(this, prefix, "this");
    const renderBundleEncoderRid = assertResource(this, prefix, "this");
    const pipelineRid = assertResource(pipeline, prefix, "Argument 1");
    assertDeviceMatch(device, pipeline, {
      prefix,
      resourceContext: "Argument 1",
      selfContext: "this",
    });
    op_webgpu_render_bundle_encoder_set_pipeline(
      renderBundleEncoderRid,
      pipelineRid,
    );
  }

  /**
   * @param {GPUBuffer} buffer
   * @param {GPUIndexFormat} indexFormat
   * @param {number} offset
   * @param {number} size
   */
  setIndexBuffer(buffer, indexFormat, offset = 0, size = 0) {
    webidl.assertBranded(this, GPURenderBundleEncoderPrototype);
    const prefix =
      "Failed to execute 'setIndexBuffer' on 'GPURenderBundleEncoder'";
    webidl.requiredArguments(arguments.length, 2, prefix);
    buffer = webidl.converters.GPUBuffer(buffer, prefix, "Argument 1");
    indexFormat = webidl.converters.GPUIndexFormat(
      indexFormat,
      prefix,
      "Argument 2",
    );
    offset = webidl.converters.GPUSize64(offset, prefix, "Argument 3");
    size = webidl.converters.GPUSize64(size, prefix, "Argument 4");
    const device = assertDevice(this, prefix, "this");
    const renderBundleEncoderRid = assertResource(this, prefix, "this");
    const bufferRid = assertResource(buffer, prefix, "Argument 1");
    assertDeviceMatch(device, buffer, {
      prefix,
      resourceContext: "Argument 1",
      selfContext: "this",
    });
    op_webgpu_render_bundle_encoder_set_index_buffer(
      renderBundleEncoderRid,
      bufferRid,
      indexFormat,
      offset,
      size,
    );
  }

  /**
   * @param {number} slot
   * @param {GPUBuffer} buffer
   * @param {number} offset
   * @param {number} size
   */
  setVertexBuffer(slot, buffer, offset = 0, size) {
    webidl.assertBranded(this, GPURenderBundleEncoderPrototype);
    const prefix =
      "Failed to execute 'setVertexBuffer' on 'GPURenderBundleEncoder'";
    webidl.requiredArguments(arguments.length, 2, prefix);
    slot = webidl.converters.GPUSize32(slot, prefix, "Argument 1");
    buffer = webidl.converters.GPUBuffer(buffer, prefix, "Argument 2");
    offset = webidl.converters.GPUSize64(offset, prefix, "Argument 3");
    if (size !== undefined) {
      size = webidl.converters.GPUSize64(size, prefix, "Argument 4");
    }
    const device = assertDevice(this, prefix, "this");
    const renderBundleEncoderRid = assertResource(this, prefix, "this");
    const bufferRid = assertResource(buffer, prefix, "Argument 2");
    assertDeviceMatch(device, buffer, {
      prefix,
      resourceContext: "Argument 2",
      selfContext: "this",
    });
    op_webgpu_render_bundle_encoder_set_vertex_buffer(
      renderBundleEncoderRid,
      slot,
      bufferRid,
      offset,
      size,
    );
  }

  /**
   * @param {number} vertexCount
   * @param {number} instanceCount
   * @param {number} firstVertex
   * @param {number} firstInstance
   */
  draw(vertexCount, instanceCount = 1, firstVertex = 0, firstInstance = 0) {
    webidl.assertBranded(this, GPURenderBundleEncoderPrototype);
    const prefix = "Failed to execute 'draw' on 'GPURenderBundleEncoder'";
    webidl.requiredArguments(arguments.length, 1, prefix);
    vertexCount = webidl.converters.GPUSize32(
      vertexCount,
      prefix,
      "Argument 1",
    );
    instanceCount = webidl.converters.GPUSize32(
      instanceCount,
      prefix,
      "Argument 2",
    );
    firstVertex = webidl.converters.GPUSize32(
      firstVertex,
      prefix,
      "Argument 3",
    );
    firstInstance = webidl.converters.GPUSize32(
      firstInstance,
      prefix,
      "Argument 4",
    );
    assertDevice(this, prefix, "this");
    const renderBundleEncoderRid = assertResource(this, prefix, "this");
    op_webgpu_render_bundle_encoder_draw(
      renderBundleEncoderRid,
      vertexCount,
      instanceCount,
      firstVertex,
      firstInstance,
    );
  }

  /**
   * @param {number} indexCount
   * @param {number} instanceCount
   * @param {number} firstIndex
   * @param {number} baseVertex
   * @param {number} firstInstance
   */
  drawIndexed(
    indexCount,
    instanceCount = 1,
    firstIndex = 0,
    baseVertex = 0,
    firstInstance = 0,
  ) {
    webidl.assertBranded(this, GPURenderBundleEncoderPrototype);
    const prefix =
      "Failed to execute 'drawIndexed' on 'GPURenderBundleEncoder'";
    webidl.requiredArguments(arguments.length, 1, prefix);
    indexCount = webidl.converters.GPUSize32(indexCount, prefix, "Argument 1");
    instanceCount = webidl.converters.GPUSize32(
      instanceCount,
      prefix,
      "Argument 2",
    );
    firstIndex = webidl.converters.GPUSize32(firstIndex, prefix, "Argument 3");
    baseVertex = webidl.converters.GPUSignedOffset32(
      baseVertex,
      prefix,
      "Argument 4",
    );
    firstInstance = webidl.converters.GPUSize32(
      firstInstance,
      prefix,
      "Argument 5",
    );
    assertDevice(this, prefix, "this");
    const renderBundleEncoderRid = assertResource(this, prefix, "this");
    op_webgpu_render_bundle_encoder_draw_indexed(
      renderBundleEncoderRid,
      indexCount,
      instanceCount,
      firstIndex,
      baseVertex,
      firstInstance,
    );
  }

  /**
   * @param {GPUBuffer} indirectBuffer
   * @param {number} indirectOffset
   */
  drawIndirect(indirectBuffer, indirectOffset) {
    webidl.assertBranded(this, GPURenderBundleEncoderPrototype);
    const prefix =
      "Failed to execute 'drawIndirect' on 'GPURenderBundleEncoder'";
    webidl.requiredArguments(arguments.length, 2, prefix);
    indirectBuffer = webidl.converters.GPUBuffer(
      indirectBuffer,
      prefix,
      "Argument 1",
    );
    indirectOffset = webidl.converters.GPUSize64(
      indirectOffset,
      prefix,
      "Argument 2",
    );
    const device = assertDevice(this, prefix, "this");
    const renderBundleEncoderRid = assertResource(this, prefix, "this");
    const indirectBufferRid = assertResource(
      indirectBuffer,
      prefix,
      "Argument 1",
    );
    assertDeviceMatch(device, indirectBuffer, {
      prefix,
      resourceContext: "Argument 1",
      selfContext: "this",
    });
    op_webgpu_render_bundle_encoder_draw_indirect(
      renderBundleEncoderRid,
      indirectBufferRid,
      indirectOffset,
    );
  }

  [SymbolFor("Deno.privateCustomInspect")](inspect, inspectOptions) {
    return inspect(
      createFilteredInspectProxy({
        object: this,
        evaluate: ObjectPrototypeIsPrototypeOf(
          GPURenderBundleEncoderPrototype,
          this,
        ),
        keys: [
          "label",
        ],
      }),
      inspectOptions,
    );
  }
}
GPUObjectBaseMixin("GPURenderBundleEncoder", GPURenderBundleEncoder);
const GPURenderBundleEncoderPrototype = GPURenderBundleEncoder.prototype;

/**
 * @param {string | null} label
 * @param {InnerGPUDevice} device
 * @param {number} rid
 * @returns {GPURenderBundle}
 */
function createGPURenderBundle(label, device, rid) {
  /** @type {GPURenderBundle} */
  const bundle = webidl.createBranded(GPURenderBundle);
  bundle[_label] = label;
  bundle[_device] = device;
  bundle[_rid] = rid;
  return bundle;
}

class GPURenderBundle {
  /** @type {InnerGPUDevice} */
  [_device];
  /** @type {number | undefined} */
  [_rid];

  [_cleanup]() {
    const rid = this[_rid];
    if (rid !== undefined) {
      core.close(rid);
      /** @type {number | undefined} */
      this[_rid] = undefined;
    }
  }

  constructor() {
    webidl.illegalConstructor();
  }

  [SymbolFor("Deno.privateCustomInspect")](inspect, inspectOptions) {
    return inspect(
      createFilteredInspectProxy({
        object: this,
        evaluate: ObjectPrototypeIsPrototypeOf(GPURenderBundlePrototype, this),
        keys: [
          "label",
        ],
      }),
      inspectOptions,
    );
  }
}
GPUObjectBaseMixin("GPURenderBundle", GPURenderBundle);
const GPURenderBundlePrototype = GPURenderBundle.prototype;
/**
 * @param {string | null} label
 * @param {InnerGPUDevice} device
 * @param {number} rid
 * @returns {GPUQuerySet}
 */
function createGPUQuerySet(label, device, rid, descriptor) {
  /** @type {GPUQuerySet} */
  const queue = webidl.createBranded(GPUQuerySet);
  queue[_label] = label;
  queue[_device] = device;
  queue[_rid] = rid;
  queue[_descriptor] = descriptor;
  return queue;
}

class GPUQuerySet {
  /** @type {InnerGPUDevice} */
  [_device];
  /** @type {number | undefined} */
  [_rid];
  /** @type {GPUQuerySetDescriptor} */
  [_descriptor];
  /** @type {GPUQueryType} */
  [_type];
  /** @type {number} */
  [_count];

  [_cleanup]() {
    const rid = this[_rid];
    if (rid !== undefined) {
      core.close(rid);
      /** @type {number | undefined} */
      this[_rid] = undefined;
    }
  }

  constructor() {
    webidl.illegalConstructor();
  }

  destroy() {
    webidl.assertBranded(this, GPUQuerySetPrototype);
    this[_cleanup]();
  }

  get type() {
    webidl.assertBranded(this, GPUQuerySetPrototype);
    return this[_type]();
  }

  get count() {
    webidl.assertBranded(this, GPUQuerySetPrototype);
    return this[_count]();
  }

  [SymbolFor("Deno.privateCustomInspect")](inspect, inspectOptions) {
    return inspect(
      createFilteredInspectProxy({
        object: this,
        evaluate: ObjectPrototypeIsPrototypeOf(GPUQuerySetPrototype, this),
        keys: [
          "label",
          "type",
          "count",
        ],
      }),
      inspectOptions,
    );
  }
}
GPUObjectBaseMixin("GPUQuerySet", GPUQuerySet);
const GPUQuerySetPrototype = GPUQuerySet.prototype;

// Converters

// This needs to be initialized after all of the base classes are implemented,
// otherwise their converters might not be available yet.
// DICTIONARY: GPUObjectDescriptorBase
const dictMembersGPUObjectDescriptorBase = [
  { key: "label", converter: webidl.converters["USVString"], defaultValue: "" },
];
webidl.converters["GPUObjectDescriptorBase"] = webidl
  .createDictionaryConverter(
    "GPUObjectDescriptorBase",
    dictMembersGPUObjectDescriptorBase,
  );

// INTERFACE: GPUSupportedLimits
webidl.converters.GPUSupportedLimits = webidl.createInterfaceConverter(
  "GPUSupportedLimits",
  GPUSupportedLimits.prototype,
);

// INTERFACE: GPUSupportedFeatures
webidl.converters.GPUSupportedFeatures = webidl.createInterfaceConverter(
  "GPUSupportedFeatures",
  GPUSupportedFeatures.prototype,
);

// INTERFACE: GPU
webidl.converters.GPU = webidl.createInterfaceConverter("GPU", GPU.prototype);

// ENUM: GPUPowerPreference
webidl.converters["GPUPowerPreference"] = webidl.createEnumConverter(
  "GPUPowerPreference",
  [
    "low-power",
    "high-performance",
  ],
);

// DICTIONARY: GPURequestAdapterOptions
const dictMembersGPURequestAdapterOptions = [
  {
    key: "powerPreference",
    converter: webidl.converters["GPUPowerPreference"],
  },
  {
    key: "forceFallbackAdapter",
    converter: webidl.converters.boolean,
    defaultValue: false,
  },
];
webidl.converters["GPURequestAdapterOptions"] = webidl
  .createDictionaryConverter(
    "GPURequestAdapterOptions",
    dictMembersGPURequestAdapterOptions,
  );

// INTERFACE: GPUAdapter
webidl.converters.GPUAdapter = webidl.createInterfaceConverter(
  "GPUAdapter",
  GPUAdapter.prototype,
);

// ENUM: GPUFeatureName
webidl.converters["GPUFeatureName"] = webidl.createEnumConverter(
  "GPUFeatureName",
  [
    // api
    "depth-clip-control",
    "timestamp-query",
    "indirect-first-instance",
    // shader
    "shader-f16",
    // texture formats
    "depth32float-stencil8",
    "texture-compression-bc",
    "texture-compression-etc2",
    "texture-compression-astc",
    "rg11b10ufloat-renderable",
    "bgra8unorm-storage",
    "float32-filterable",

    // extended from spec

    // texture formats
    "texture-format-16-bit-norm",
    "texture-compression-astc-hdr",
    "texture-adapter-specific-format-features",
    // api
    //"pipeline-statistics-query",
    "timestamp-query-inside-passes",
    "mappable-primary-buffers",
    "texture-binding-array",
    "buffer-binding-array",
    "storage-resource-binding-array",
    "sampled-texture-and-storage-buffer-array-non-uniform-indexing",
    "uniform-buffer-and-storage-texture-array-non-uniform-indexing",
    "partially-bound-binding-array",
    "multi-draw-indirect",
    "multi-draw-indirect-count",
    "push-constants",
    "address-mode-clamp-to-zero",
    "address-mode-clamp-to-border",
    "polygon-mode-line",
    "polygon-mode-point",
    "conservative-rasterization",
    "vertex-writable-storage",
    "clear-texture",
    "spirv-shader-passthrough",
    "multiview",
    "vertex-attribute-64-bit",
    // shader
    "shader-f64",
    "shader-i16",
    "shader-primitive-index",
    "shader-early-depth-test",
  ],
);

// DICTIONARY: GPUPipelineErrorInit
webidl.converters["GPUPipelineErrorInit"] = webidl.createDictionaryConverter(
  "GPUPipelineErrorInit",
  [
    {
      key: "reason",
      converter: webidl.converters.GPUPipelineErrorReason,
      required: true,
    },
  ],
);

// ENUM: GPUPipelineErrorReason
webidl.converters["GPUPipelineErrorReason"] = webidl.createEnumConverter(
  "GPUPipelineErrorReason",
  [
    "validation",
    "internal",
  ],
);

// TYPEDEF: GPUSize32
webidl.converters["GPUSize32"] = (V, opts) =>
  webidl.converters["unsigned long"](V, { ...opts, enforceRange: true });

// TYPEDEF: GPUSize64
webidl.converters["GPUSize64"] = (V, opts) =>
  webidl.converters["unsigned long long"](V, { ...opts, enforceRange: true });

// DICTIONARY: GPUDeviceDescriptor
const dictMembersGPUDeviceDescriptor = [
  {
    key: "requiredFeatures",
    converter: webidl.createSequenceConverter(
      webidl.converters["GPUFeatureName"],
    ),
    get defaultValue() {
      return [];
    },
  },
  {
    key: "requiredLimits",
    converter: webidl.createRecordConverter(
      webidl.converters["DOMString"],
      webidl.converters["GPUSize64"],
    ),
  },
];
webidl.converters["GPUDeviceDescriptor"] = webidl.createDictionaryConverter(
  "GPUDeviceDescriptor",
  dictMembersGPUObjectDescriptorBase,
  dictMembersGPUDeviceDescriptor,
);

// INTERFACE: GPUDevice
webidl.converters.GPUDevice = webidl.createInterfaceConverter(
  "GPUDevice",
  GPUDevice.prototype,
);

// INTERFACE: GPUBuffer
webidl.converters.GPUBuffer = webidl.createInterfaceConverter(
  "GPUBuffer",
  GPUBuffer.prototype,
);

// TYPEDEF: GPUBufferUsageFlags
webidl.converters["GPUBufferUsageFlags"] = (V, opts) =>
  webidl.converters["unsigned long"](V, { ...opts, enforceRange: true });

// DICTIONARY: GPUBufferDescriptor
const dictMembersGPUBufferDescriptor = [
  { key: "size", converter: webidl.converters["GPUSize64"], required: true },
  {
    key: "usage",
    converter: webidl.converters["GPUBufferUsageFlags"],
    required: true,
  },
  {
    key: "mappedAtCreation",
    converter: webidl.converters["boolean"],
    defaultValue: false,
  },
];
webidl.converters["GPUBufferDescriptor"] = webidl.createDictionaryConverter(
  "GPUBufferDescriptor",
  dictMembersGPUObjectDescriptorBase,
  dictMembersGPUBufferDescriptor,
);

// INTERFACE: GPUBufferUsage
webidl.converters.GPUBufferUsage = webidl.createInterfaceConverter(
  "GPUBufferUsage",
  GPUBufferUsage.prototype,
);

// TYPEDEF: GPUMapModeFlags
webidl.converters["GPUMapModeFlags"] = (V, opts) =>
  webidl.converters["unsigned long"](V, { ...opts, enforceRange: true });

// INTERFACE: GPUMapMode
webidl.converters.GPUMapMode = webidl.createInterfaceConverter(
  "GPUMapMode",
  GPUMapMode.prototype,
);

// INTERFACE: GPUTexture
webidl.converters.GPUTexture = webidl.createInterfaceConverter(
  "GPUTexture",
  GPUTexture.prototype,
);

// TYPEDEF: GPUIntegerCoordinate
webidl.converters["GPUIntegerCoordinate"] = (V, opts) =>
  webidl.converters["unsigned long"](V, { ...opts, enforceRange: true });
webidl.converters["sequence<GPUIntegerCoordinate>"] = webidl
  .createSequenceConverter(webidl.converters["GPUIntegerCoordinate"]);

// DICTIONARY: GPUExtent3DDict
const dictMembersGPUExtent3DDict = [
  {
    key: "width",
    converter: webidl.converters["GPUIntegerCoordinate"],
    required: true,
  },
  {
    key: "height",
    converter: webidl.converters["GPUIntegerCoordinate"],
    defaultValue: 1,
  },
  {
    key: "depthOrArrayLayers",
    converter: webidl.converters["GPUIntegerCoordinate"],
    defaultValue: 1,
  },
];
webidl.converters["GPUExtent3DDict"] = webidl.createDictionaryConverter(
  "GPUExtent3DDict",
  dictMembersGPUExtent3DDict,
);

// TYPEDEF: GPUExtent3D
webidl.converters["GPUExtent3D"] = (V, opts) => {
  // Union for (sequence<GPUIntegerCoordinate> or GPUExtent3DDict)
  if (V === null || V === undefined) {
    return webidl.converters["GPUExtent3DDict"](V, opts);
  }
  if (typeof V === "object") {
    const method = V[SymbolIterator];
    if (method !== undefined) {
      // validate length of GPUExtent3D
      const min = 1;
      const max = 3;
      if (V.length < min || V.length > max) {
        throw webidl.makeException(
          TypeError,
          `A sequence of number used as a GPUExtent3D must have between ${min} and ${max} elements.`,
          opts,
        );
      }
      return webidl.converters["sequence<GPUIntegerCoordinate>"](V, opts);
    }
    return webidl.converters["GPUExtent3DDict"](V, opts);
  }
  throw webidl.makeException(
    TypeError,
    "can not be converted to sequence<GPUIntegerCoordinate> or GPUExtent3DDict.",
    opts,
  );
};

// ENUM: GPUTextureDimension
webidl.converters["GPUTextureDimension"] = webidl.createEnumConverter(
  "GPUTextureDimension",
  [
    "1d",
    "2d",
    "3d",
  ],
);

// ENUM: GPUTextureFormat
webidl.converters["GPUTextureFormat"] = webidl.createEnumConverter(
  "GPUTextureFormat",
  [
    "r8unorm",
    "r8snorm",
    "r8uint",
    "r8sint",
    "r16uint",
    "r16sint",
    "r16float",
    "rg8unorm",
    "rg8snorm",
    "rg8uint",
    "rg8sint",
    "r32uint",
    "r32sint",
    "r32float",
    "rg16uint",
    "rg16sint",
    "rg16float",
    "rgba8unorm",
    "rgba8unorm-srgb",
    "rgba8snorm",
    "rgba8uint",
    "rgba8sint",
    "bgra8unorm",
    "bgra8unorm-srgb",
    "rgb9e5ufloat",
    "rgb10a2uint",
    "rgb10a2unorm",
    "rg11b10ufloat",
    "rg32uint",
    "rg32sint",
    "rg32float",
    "rgba16uint",
    "rgba16sint",
    "rgba16float",
    "rgba32uint",
    "rgba32sint",
    "rgba32float",
    "stencil8",
    "depth16unorm",
    "depth24plus",
    "depth24plus-stencil8",
    "depth32float",
    "depth32float-stencil8",
    "bc1-rgba-unorm",
    "bc1-rgba-unorm-srgb",
    "bc2-rgba-unorm",
    "bc2-rgba-unorm-srgb",
    "bc3-rgba-unorm",
    "bc3-rgba-unorm-srgb",
    "bc4-r-unorm",
    "bc4-r-snorm",
    "bc5-rg-unorm",
    "bc5-rg-snorm",
    "bc6h-rgb-ufloat",
    "bc6h-rgb-float",
    "bc7-rgba-unorm",
    "bc7-rgba-unorm-srgb",
    "etc2-rgb8unorm",
    "etc2-rgb8unorm-srgb",
    "etc2-rgb8a1unorm",
    "etc2-rgb8a1unorm-srgb",
    "etc2-rgba8unorm",
    "etc2-rgba8unorm-srgb",
    "eac-r11unorm",
    "eac-r11snorm",
    "eac-rg11unorm",
    "eac-rg11snorm",
    "astc-4x4-unorm",
    "astc-4x4-unorm-srgb",
    "astc-5x4-unorm",
    "astc-5x4-unorm-srgb",
    "astc-5x5-unorm",
    "astc-5x5-unorm-srgb",
    "astc-6x5-unorm",
    "astc-6x5-unorm-srgb",
    "astc-6x6-unorm",
    "astc-6x6-unorm-srgb",
    "astc-8x5-unorm",
    "astc-8x5-unorm-srgb",
    "astc-8x6-unorm",
    "astc-8x6-unorm-srgb",
    "astc-8x8-unorm",
    "astc-8x8-unorm-srgb",
    "astc-10x5-unorm",
    "astc-10x5-unorm-srgb",
    "astc-10x6-unorm",
    "astc-10x6-unorm-srgb",
    "astc-10x8-unorm",
    "astc-10x8-unorm-srgb",
    "astc-10x10-unorm",
    "astc-10x10-unorm-srgb",
    "astc-12x10-unorm",
    "astc-12x10-unorm-srgb",
    "astc-12x12-unorm",
    "astc-12x12-unorm-srgb",
  ],
);

// TYPEDEF: GPUTextureUsageFlags
webidl.converters["GPUTextureUsageFlags"] = (V, opts) =>
  webidl.converters["unsigned long"](V, { ...opts, enforceRange: true });

// DICTIONARY: GPUTextureDescriptor
const dictMembersGPUTextureDescriptor = [
  {
    key: "size",
    converter: webidl.converters["GPUExtent3D"],
    required: true,
  },
  {
    key: "mipLevelCount",
    converter: webidl.converters["GPUIntegerCoordinate"],
    defaultValue: 1,
  },
  {
    key: "sampleCount",
    converter: webidl.converters["GPUSize32"],
    defaultValue: 1,
  },
  {
    key: "dimension",
    converter: webidl.converters["GPUTextureDimension"],
    defaultValue: "2d",
  },
  {
    key: "format",
    converter: webidl.converters["GPUTextureFormat"],
    required: true,
  },
  {
    key: "usage",
    converter: webidl.converters["GPUTextureUsageFlags"],
    required: true,
  },
  {
    key: "viewFormats",
    converter: webidl.createSequenceConverter(
      webidl.converters["GPUTextureFormat"],
    ),
    get defaultValue() {
      return [];
    },
  },
];
webidl.converters["GPUTextureDescriptor"] = webidl.createDictionaryConverter(
  "GPUTextureDescriptor",
  dictMembersGPUObjectDescriptorBase,
  dictMembersGPUTextureDescriptor,
);

// INTERFACE: GPUTextureUsage
webidl.converters.GPUTextureUsage = webidl.createInterfaceConverter(
  "GPUTextureUsage",
  GPUTextureUsage.prototype,
);

// INTERFACE: GPUTextureView
webidl.converters.GPUTextureView = webidl.createInterfaceConverter(
  "GPUTextureView",
  GPUTextureView.prototype,
);

// ENUM: GPUTextureViewDimension
webidl.converters["GPUTextureViewDimension"] = webidl.createEnumConverter(
  "GPUTextureViewDimension",
  [
    "1d",
    "2d",
    "2d-array",
    "cube",
    "cube-array",
    "3d",
  ],
);

// ENUM: GPUTextureAspect
webidl.converters["GPUTextureAspect"] = webidl.createEnumConverter(
  "GPUTextureAspect",
  [
    "all",
    "stencil-only",
    "depth-only",
  ],
);

// DICTIONARY: GPUTextureViewDescriptor
const dictMembersGPUTextureViewDescriptor = [
  { key: "format", converter: webidl.converters["GPUTextureFormat"] },
  {
    key: "dimension",
    converter: webidl.converters["GPUTextureViewDimension"],
  },
  {
    key: "aspect",
    converter: webidl.converters["GPUTextureAspect"],
    defaultValue: "all",
  },
  {
    key: "baseMipLevel",
    converter: webidl.converters["GPUIntegerCoordinate"],
    defaultValue: 0,
  },
  {
    key: "mipLevelCount",
    converter: webidl.converters["GPUIntegerCoordinate"],
  },
  {
    key: "baseArrayLayer",
    converter: webidl.converters["GPUIntegerCoordinate"],
    defaultValue: 0,
  },
  {
    key: "arrayLayerCount",
    converter: webidl.converters["GPUIntegerCoordinate"],
  },
];
webidl.converters["GPUTextureViewDescriptor"] = webidl
  .createDictionaryConverter(
    "GPUTextureViewDescriptor",
    dictMembersGPUObjectDescriptorBase,
    dictMembersGPUTextureViewDescriptor,
  );

// INTERFACE: GPUSampler
webidl.converters.GPUSampler = webidl.createInterfaceConverter(
  "GPUSampler",
  GPUSampler.prototype,
);

// ENUM: GPUAddressMode
webidl.converters["GPUAddressMode"] = webidl.createEnumConverter(
  "GPUAddressMode",
  [
    "clamp-to-edge",
    "repeat",
    "mirror-repeat",
  ],
);

// ENUM: GPUFilterMode
webidl.converters["GPUFilterMode"] = webidl.createEnumConverter(
  "GPUFilterMode",
  [
    "nearest",
    "linear",
  ],
);

// ENUM: GPUMipmapFilterMode
webidl.converters["GPUMipmapFilterMode"] = webidl.createEnumConverter(
  "GPUMipmapFilterMode",
  [
    "nearest",
    "linear",
  ],
);

// ENUM: GPUCompareFunction
webidl.converters["GPUCompareFunction"] = webidl.createEnumConverter(
  "GPUCompareFunction",
  [
    "never",
    "less",
    "equal",
    "less-equal",
    "greater",
    "not-equal",
    "greater-equal",
    "always",
  ],
);

// DICTIONARY: GPUSamplerDescriptor
const dictMembersGPUSamplerDescriptor = [
  {
    key: "addressModeU",
    converter: webidl.converters["GPUAddressMode"],
    defaultValue: "clamp-to-edge",
  },
  {
    key: "addressModeV",
    converter: webidl.converters["GPUAddressMode"],
    defaultValue: "clamp-to-edge",
  },
  {
    key: "addressModeW",
    converter: webidl.converters["GPUAddressMode"],
    defaultValue: "clamp-to-edge",
  },
  {
    key: "magFilter",
    converter: webidl.converters["GPUFilterMode"],
    defaultValue: "nearest",
  },
  {
    key: "minFilter",
    converter: webidl.converters["GPUFilterMode"],
    defaultValue: "nearest",
  },
  {
    key: "mipmapFilter",
    converter: webidl.converters["GPUMipmapFilterMode"],
    defaultValue: "nearest",
  },
  {
    key: "lodMinClamp",
    converter: webidl.converters["float"],
    defaultValue: 0,
  },
  {
    key: "lodMaxClamp",
    converter: webidl.converters["float"],
    defaultValue: 0xffffffff,
  },
  { key: "compare", converter: webidl.converters["GPUCompareFunction"] },
  {
    key: "maxAnisotropy",
    converter: (V, opts) =>
      webidl.converters["unsigned short"](V, { ...opts, clamp: true }),
    defaultValue: 1,
  },
];
webidl.converters["GPUSamplerDescriptor"] = webidl.createDictionaryConverter(
  "GPUSamplerDescriptor",
  dictMembersGPUObjectDescriptorBase,
  dictMembersGPUSamplerDescriptor,
);

// INTERFACE: GPUBindGroupLayout
webidl.converters.GPUBindGroupLayout = webidl.createInterfaceConverter(
  "GPUBindGroupLayout",
  GPUBindGroupLayout.prototype,
);

// TYPEDEF: GPUIndex32
webidl.converters["GPUIndex32"] = (V, opts) =>
  webidl.converters["unsigned long"](V, { ...opts, enforceRange: true });

// TYPEDEF: GPUShaderStageFlags
webidl.converters["GPUShaderStageFlags"] = (V, opts) =>
  webidl.converters["unsigned long"](V, { ...opts, enforceRange: true });

// ENUM: GPUBufferBindingType
webidl.converters["GPUBufferBindingType"] = webidl.createEnumConverter(
  "GPUBufferBindingType",
  [
    "uniform",
    "storage",
    "read-only-storage",
  ],
);

// DICTIONARY: GPUBufferBindingLayout
const dictMembersGPUBufferBindingLayout = [
  {
    key: "type",
    converter: webidl.converters["GPUBufferBindingType"],
    defaultValue: "uniform",
  },
  {
    key: "hasDynamicOffset",
    converter: webidl.converters["boolean"],
    defaultValue: false,
  },
  {
    key: "minBindingSize",
    converter: webidl.converters["GPUSize64"],
    defaultValue: 0,
  },
];
webidl.converters["GPUBufferBindingLayout"] = webidl
  .createDictionaryConverter(
    "GPUBufferBindingLayout",
    dictMembersGPUBufferBindingLayout,
  );

// ENUM: GPUSamplerBindingType
webidl.converters["GPUSamplerBindingType"] = webidl.createEnumConverter(
  "GPUSamplerBindingType",
  [
    "filtering",
    "non-filtering",
    "comparison",
  ],
);

// DICTIONARY: GPUSamplerBindingLayout
const dictMembersGPUSamplerBindingLayout = [
  {
    key: "type",
    converter: webidl.converters["GPUSamplerBindingType"],
    defaultValue: "filtering",
  },
];
webidl.converters["GPUSamplerBindingLayout"] = webidl
  .createDictionaryConverter(
    "GPUSamplerBindingLayout",
    dictMembersGPUSamplerBindingLayout,
  );

// ENUM: GPUTextureSampleType
webidl.converters["GPUTextureSampleType"] = webidl.createEnumConverter(
  "GPUTextureSampleType",
  [
    "float",
    "unfilterable-float",
    "depth",
    "sint",
    "uint",
  ],
);

// DICTIONARY: GPUTextureBindingLayout
const dictMembersGPUTextureBindingLayout = [
  {
    key: "sampleType",
    converter: webidl.converters["GPUTextureSampleType"],
    defaultValue: "float",
  },
  {
    key: "viewDimension",
    converter: webidl.converters["GPUTextureViewDimension"],
    defaultValue: "2d",
  },
  {
    key: "multisampled",
    converter: webidl.converters["boolean"],
    defaultValue: false,
  },
];
webidl.converters["GPUTextureBindingLayout"] = webidl
  .createDictionaryConverter(
    "GPUTextureBindingLayout",
    dictMembersGPUTextureBindingLayout,
  );

// ENUM: GPUStorageTextureAccess
webidl.converters["GPUStorageTextureAccess"] = webidl.createEnumConverter(
  "GPUStorageTextureAccess",
  [
    "write-only",
    "read-only",
    "read-write",
  ],
);

// DICTIONARY: GPUStorageTextureBindingLayout
const dictMembersGPUStorageTextureBindingLayout = [
  {
    key: "access",
    converter: webidl.converters["GPUStorageTextureAccess"],
    defaultValue: "write-only",
  },
  {
    key: "format",
    converter: webidl.converters["GPUTextureFormat"],
    required: true,
  },
  {
    key: "viewDimension",
    converter: webidl.converters["GPUTextureViewDimension"],
    defaultValue: "2d",
  },
];
webidl.converters["GPUStorageTextureBindingLayout"] = webidl
  .createDictionaryConverter(
    "GPUStorageTextureBindingLayout",
    dictMembersGPUStorageTextureBindingLayout,
  );

// DICTIONARY: GPUBindGroupLayoutEntry
const dictMembersGPUBindGroupLayoutEntry = [
  {
    key: "binding",
    converter: webidl.converters["GPUIndex32"],
    required: true,
  },
  {
    key: "visibility",
    converter: webidl.converters["GPUShaderStageFlags"],
    required: true,
  },
  { key: "buffer", converter: webidl.converters["GPUBufferBindingLayout"] },
  { key: "sampler", converter: webidl.converters["GPUSamplerBindingLayout"] },
  { key: "texture", converter: webidl.converters["GPUTextureBindingLayout"] },
  {
    key: "storageTexture",
    converter: webidl.converters["GPUStorageTextureBindingLayout"],
  },
];
webidl.converters["GPUBindGroupLayoutEntry"] = webidl
  .createDictionaryConverter(
    "GPUBindGroupLayoutEntry",
    dictMembersGPUBindGroupLayoutEntry,
  );

// DICTIONARY: GPUBindGroupLayoutDescriptor
const dictMembersGPUBindGroupLayoutDescriptor = [
  {
    key: "entries",
    converter: webidl.createSequenceConverter(
      webidl.converters["GPUBindGroupLayoutEntry"],
    ),
    required: true,
  },
];
webidl.converters["GPUBindGroupLayoutDescriptor"] = webidl
  .createDictionaryConverter(
    "GPUBindGroupLayoutDescriptor",
    dictMembersGPUObjectDescriptorBase,
    dictMembersGPUBindGroupLayoutDescriptor,
  );

// INTERFACE: GPUShaderStage
webidl.converters.GPUShaderStage = webidl.createInterfaceConverter(
  "GPUShaderStage",
  GPUShaderStage.prototype,
);

// INTERFACE: GPUBindGroup
webidl.converters.GPUBindGroup = webidl.createInterfaceConverter(
  "GPUBindGroup",
  GPUBindGroup.prototype,
);

// DICTIONARY: GPUBufferBinding
const dictMembersGPUBufferBinding = [
  {
    key: "buffer",
    converter: webidl.converters["GPUBuffer"],
    required: true,
  },
  {
    key: "offset",
    converter: webidl.converters["GPUSize64"],
    defaultValue: 0,
  },
  { key: "size", converter: webidl.converters["GPUSize64"] },
];
webidl.converters["GPUBufferBinding"] = webidl.createDictionaryConverter(
  "GPUBufferBinding",
  dictMembersGPUBufferBinding,
);

// TYPEDEF: GPUBindingResource
webidl.converters["GPUBindingResource"] =
  webidl.converters.any /** put union here! **/;

// DICTIONARY: GPUBindGroupEntry
const dictMembersGPUBindGroupEntry = [
  {
    key: "binding",
    converter: webidl.converters["GPUIndex32"],
    required: true,
  },
  {
    key: "resource",
    converter: webidl.converters["GPUBindingResource"],
    required: true,
  },
];
webidl.converters["GPUBindGroupEntry"] = webidl.createDictionaryConverter(
  "GPUBindGroupEntry",
  dictMembersGPUBindGroupEntry,
);

// DICTIONARY: GPUBindGroupDescriptor
const dictMembersGPUBindGroupDescriptor = [
  {
    key: "layout",
    converter: webidl.converters["GPUBindGroupLayout"],
    required: true,
  },
  {
    key: "entries",
    converter: webidl.createSequenceConverter(
      webidl.converters["GPUBindGroupEntry"],
    ),
    required: true,
  },
];
webidl.converters["GPUBindGroupDescriptor"] = webidl
  .createDictionaryConverter(
    "GPUBindGroupDescriptor",
    dictMembersGPUObjectDescriptorBase,
    dictMembersGPUBindGroupDescriptor,
  );

// INTERFACE: GPUPipelineLayout
webidl.converters.GPUPipelineLayout = webidl.createInterfaceConverter(
  "GPUPipelineLayout",
  GPUPipelineLayout.prototype,
);

// DICTIONARY: GPUPipelineLayoutDescriptor
const dictMembersGPUPipelineLayoutDescriptor = [
  {
    key: "bindGroupLayouts",
    converter: webidl.createSequenceConverter(
      webidl.converters["GPUBindGroupLayout"],
    ),
    required: true,
  },
];
webidl.converters["GPUPipelineLayoutDescriptor"] = webidl
  .createDictionaryConverter(
    "GPUPipelineLayoutDescriptor",
    dictMembersGPUObjectDescriptorBase,
    dictMembersGPUPipelineLayoutDescriptor,
  );

// INTERFACE: GPUShaderModule
webidl.converters.GPUShaderModule = webidl.createInterfaceConverter(
  "GPUShaderModule",
  GPUShaderModule.prototype,
);

// DICTIONARY: GPUShaderModuleDescriptor
const dictMembersGPUShaderModuleDescriptor = [
  {
    key: "code",
    converter: webidl.converters["DOMString"],
    required: true,
  },
];
webidl.converters["GPUShaderModuleDescriptor"] = webidl
  .createDictionaryConverter(
    "GPUShaderModuleDescriptor",
    dictMembersGPUObjectDescriptorBase,
    dictMembersGPUShaderModuleDescriptor,
  );

// // ENUM: GPUCompilationMessageType
// webidl.converters["GPUCompilationMessageType"] = webidl.createEnumConverter(
//   "GPUCompilationMessageType",
//   [
//     "error",
//     "warning",
//     "info",
//   ],
// );

// // INTERFACE: GPUCompilationMessage
// webidl.converters.GPUCompilationMessage = webidl.createInterfaceConverter(
//   "GPUCompilationMessage",
//   GPUCompilationMessage.prototype,
// );

// // INTERFACE: GPUCompilationInfo
// webidl.converters.GPUCompilationInfo = webidl.createInterfaceConverter(
//   "GPUCompilationInfo",
//   GPUCompilationInfo.prototype,
// );

webidl.converters["GPUAutoLayoutMode"] = webidl.createEnumConverter(
  "GPUAutoLayoutMode",
  [
    "auto",
  ],
);

webidl.converters["GPUPipelineLayout or GPUAutoLayoutMode"] = (V, opts) => {
  if (typeof V === "object") {
    return webidl.converters["GPUPipelineLayout"](V, opts);
  }
  return webidl.converters["GPUAutoLayoutMode"](V, opts);
};

// DICTIONARY: GPUPipelineDescriptorBase
const dictMembersGPUPipelineDescriptorBase = [
  {
    key: "layout",
    converter: webidl.converters["GPUPipelineLayout or GPUAutoLayoutMode"],
    required: true,
  },
];
webidl.converters["GPUPipelineDescriptorBase"] = webidl
  .createDictionaryConverter(
    "GPUPipelineDescriptorBase",
    dictMembersGPUObjectDescriptorBase,
    dictMembersGPUPipelineDescriptorBase,
  );

// TYPEDEF: GPUPipelineConstantValue
webidl.converters.GPUPipelineConstantValue = webidl.converters.double;

webidl.converters["record<USVString, GPUPipelineConstantValue>"] = webidl
  .createRecordConverter(
    webidl.converters.USVString,
    webidl.converters.GPUPipelineConstantValue,
  );

// DICTIONARY: GPUProgrammableStage
const dictMembersGPUProgrammableStage = [
  {
    key: "module",
    converter: webidl.converters["GPUShaderModule"],
    required: true,
  },
  {
    key: "entryPoint",
    converter: webidl.converters["USVString"],
  },
  {
    key: "constants",
    converter: webidl.converters["record<USVString, GPUPipelineConstantValue>"],
  },
];
webidl.converters["GPUProgrammableStage"] = webidl.createDictionaryConverter(
  "GPUProgrammableStage",
  dictMembersGPUProgrammableStage,
);

// INTERFACE: GPUComputePipeline
webidl.converters.GPUComputePipeline = webidl.createInterfaceConverter(
  "GPUComputePipeline",
  GPUComputePipeline.prototype,
);

// DICTIONARY: GPUComputePipelineDescriptor
const dictMembersGPUComputePipelineDescriptor = [
  {
    key: "compute",
    converter: webidl.converters["GPUProgrammableStage"],
    required: true,
  },
];
webidl.converters["GPUComputePipelineDescriptor"] = webidl
  .createDictionaryConverter(
    "GPUComputePipelineDescriptor",
    dictMembersGPUObjectDescriptorBase,
    dictMembersGPUPipelineDescriptorBase,
    dictMembersGPUComputePipelineDescriptor,
  );

// INTERFACE: GPURenderPipeline
webidl.converters.GPURenderPipeline = webidl.createInterfaceConverter(
  "GPURenderPipeline",
  GPURenderPipeline.prototype,
);

// ENUM: GPUVertexStepMode
webidl.converters["GPUVertexStepMode"] = webidl.createEnumConverter(
  "GPUVertexStepMode",
  [
    "vertex",
    "instance",
  ],
);

// ENUM: GPUVertexFormat
webidl.converters["GPUVertexFormat"] = webidl.createEnumConverter(
  "GPUVertexFormat",
  [
    "uint8x2",
    "uint8x4",
    "sint8x2",
    "sint8x4",
    "unorm8x2",
    "unorm8x4",
    "snorm8x2",
    "snorm8x4",
    "uint16x2",
    "uint16x4",
    "sint16x2",
    "sint16x4",
    "unorm16x2",
    "unorm16x4",
    "snorm16x2",
    "snorm16x4",
    "float16x2",
    "float16x4",
    "float32",
    "float32x2",
    "float32x3",
    "float32x4",
    "uint32",
    "uint32x2",
    "uint32x3",
    "uint32x4",
    "sint32",
    "sint32x2",
    "sint32x3",
    "sint32x4",
    "unorm10-10-10-2",
  ],
);

// DICTIONARY: GPUVertexAttribute
const dictMembersGPUVertexAttribute = [
  {
    key: "format",
    converter: webidl.converters["GPUVertexFormat"],
    required: true,
  },
  {
    key: "offset",
    converter: webidl.converters["GPUSize64"],
    required: true,
  },
  {
    key: "shaderLocation",
    converter: webidl.converters["GPUIndex32"],
    required: true,
  },
];
webidl.converters["GPUVertexAttribute"] = webidl.createDictionaryConverter(
  "GPUVertexAttribute",
  dictMembersGPUVertexAttribute,
);

// DICTIONARY: GPUVertexBufferLayout
const dictMembersGPUVertexBufferLayout = [
  {
    key: "arrayStride",
    converter: webidl.converters["GPUSize64"],
    required: true,
  },
  {
    key: "stepMode",
    converter: webidl.converters["GPUVertexStepMode"],
    defaultValue: "vertex",
  },
  {
    key: "attributes",
    converter: webidl.createSequenceConverter(
      webidl.converters["GPUVertexAttribute"],
    ),
    required: true,
  },
];
webidl.converters["GPUVertexBufferLayout"] = webidl.createDictionaryConverter(
  "GPUVertexBufferLayout",
  dictMembersGPUVertexBufferLayout,
);

// DICTIONARY: GPUVertexState
const dictMembersGPUVertexState = [
  {
    key: "buffers",
    converter: webidl.createSequenceConverter(
      webidl.createNullableConverter(
        webidl.converters["GPUVertexBufferLayout"],
      ),
    ),
    get defaultValue() {
      return [];
    },
  },
];
webidl.converters["GPUVertexState"] = webidl.createDictionaryConverter(
  "GPUVertexState",
  dictMembersGPUProgrammableStage,
  dictMembersGPUVertexState,
);

// ENUM: GPUPrimitiveTopology
webidl.converters["GPUPrimitiveTopology"] = webidl.createEnumConverter(
  "GPUPrimitiveTopology",
  [
    "point-list",
    "line-list",
    "line-strip",
    "triangle-list",
    "triangle-strip",
  ],
);

// ENUM: GPUIndexFormat
webidl.converters["GPUIndexFormat"] = webidl.createEnumConverter(
  "GPUIndexFormat",
  [
    "uint16",
    "uint32",
  ],
);

// ENUM: GPUFrontFace
webidl.converters["GPUFrontFace"] = webidl.createEnumConverter(
  "GPUFrontFace",
  [
    "ccw",
    "cw",
  ],
);

// ENUM: GPUCullMode
webidl.converters["GPUCullMode"] = webidl.createEnumConverter("GPUCullMode", [
  "none",
  "front",
  "back",
]);

// DICTIONARY: GPUPrimitiveState
const dictMembersGPUPrimitiveState = [
  {
    key: "topology",
    converter: webidl.converters["GPUPrimitiveTopology"],
    defaultValue: "triangle-list",
  },
  { key: "stripIndexFormat", converter: webidl.converters["GPUIndexFormat"] },
  {
    key: "frontFace",
    converter: webidl.converters["GPUFrontFace"],
    defaultValue: "ccw",
  },
  {
    key: "cullMode",
    converter: webidl.converters["GPUCullMode"],
    defaultValue: "none",
  },
  {
    key: "unclippedDepth",
    converter: webidl.converters["boolean"],
    defaultValue: false,
  },
];
webidl.converters["GPUPrimitiveState"] = webidl.createDictionaryConverter(
  "GPUPrimitiveState",
  dictMembersGPUPrimitiveState,
);

// ENUM: GPUStencilOperation
webidl.converters["GPUStencilOperation"] = webidl.createEnumConverter(
  "GPUStencilOperation",
  [
    "keep",
    "zero",
    "replace",
    "invert",
    "increment-clamp",
    "decrement-clamp",
    "increment-wrap",
    "decrement-wrap",
  ],
);

// DICTIONARY: GPUStencilFaceState
const dictMembersGPUStencilFaceState = [
  {
    key: "compare",
    converter: webidl.converters["GPUCompareFunction"],
    defaultValue: "always",
  },
  {
    key: "failOp",
    converter: webidl.converters["GPUStencilOperation"],
    defaultValue: "keep",
  },
  {
    key: "depthFailOp",
    converter: webidl.converters["GPUStencilOperation"],
    defaultValue: "keep",
  },
  {
    key: "passOp",
    converter: webidl.converters["GPUStencilOperation"],
    defaultValue: "keep",
  },
];
webidl.converters["GPUStencilFaceState"] = webidl.createDictionaryConverter(
  "GPUStencilFaceState",
  dictMembersGPUStencilFaceState,
);

// TYPEDEF: GPUStencilValue
webidl.converters["GPUStencilValue"] = (V, opts) =>
  webidl.converters["unsigned long"](V, { ...opts, enforceRange: true });

// TYPEDEF: GPUDepthBias
webidl.converters["GPUDepthBias"] = (V, opts) =>
  webidl.converters["long"](V, { ...opts, enforceRange: true });

// DICTIONARY: GPUDepthStencilState
const dictMembersGPUDepthStencilState = [
  {
    key: "format",
    converter: webidl.converters["GPUTextureFormat"],
    required: true,
  },
  {
    key: "depthWriteEnabled",
    converter: webidl.converters["boolean"],
    required: true,
  },
  {
    key: "depthCompare",
    converter: webidl.converters["GPUCompareFunction"],
    required: true,
  },
  {
    key: "stencilFront",
    converter: webidl.converters["GPUStencilFaceState"],
    get defaultValue() {
      return {};
    },
  },
  {
    key: "stencilBack",
    converter: webidl.converters["GPUStencilFaceState"],
    get defaultValue() {
      return {};
    },
  },
  {
    key: "stencilReadMask",
    converter: webidl.converters["GPUStencilValue"],
    defaultValue: 0xFFFFFFFF,
  },
  {
    key: "stencilWriteMask",
    converter: webidl.converters["GPUStencilValue"],
    defaultValue: 0xFFFFFFFF,
  },
  {
    key: "depthBias",
    converter: webidl.converters["GPUDepthBias"],
    defaultValue: 0,
  },
  {
    key: "depthBiasSlopeScale",
    converter: webidl.converters["float"],
    defaultValue: 0,
  },
  {
    key: "depthBiasClamp",
    converter: webidl.converters["float"],
    defaultValue: 0,
  },
];
webidl.converters["GPUDepthStencilState"] = webidl.createDictionaryConverter(
  "GPUDepthStencilState",
  dictMembersGPUDepthStencilState,
);

// TYPEDEF: GPUSampleMask
webidl.converters["GPUSampleMask"] = (V, opts) =>
  webidl.converters["unsigned long"](V, { ...opts, enforceRange: true });

// DICTIONARY: GPUMultisampleState
const dictMembersGPUMultisampleState = [
  {
    key: "count",
    converter: webidl.converters["GPUSize32"],
    defaultValue: 1,
  },
  {
    key: "mask",
    converter: webidl.converters["GPUSampleMask"],
    defaultValue: 0xFFFFFFFF,
  },
  {
    key: "alphaToCoverageEnabled",
    converter: webidl.converters["boolean"],
    defaultValue: false,
  },
];
webidl.converters["GPUMultisampleState"] = webidl.createDictionaryConverter(
  "GPUMultisampleState",
  dictMembersGPUMultisampleState,
);

// ENUM: GPUBlendFactor
webidl.converters["GPUBlendFactor"] = webidl.createEnumConverter(
  "GPUBlendFactor",
  [
    "zero",
    "one",
    "src",
    "one-minus-src",
    "src-alpha",
    "one-minus-src-alpha",
    "dst",
    "one-minus-dst",
    "dst-alpha",
    "one-minus-dst-alpha",
    "src-alpha-saturated",
    "constant",
    "one-minus-constant",
  ],
);

// ENUM: GPUBlendOperation
webidl.converters["GPUBlendOperation"] = webidl.createEnumConverter(
  "GPUBlendOperation",
  [
    "add",
    "subtract",
    "reverse-subtract",
    "min",
    "max",
  ],
);

// DICTIONARY: GPUBlendComponent
const dictMembersGPUBlendComponent = [
  {
    key: "srcFactor",
    converter: webidl.converters["GPUBlendFactor"],
    defaultValue: "one",
  },
  {
    key: "dstFactor",
    converter: webidl.converters["GPUBlendFactor"],
    defaultValue: "zero",
  },
  {
    key: "operation",
    converter: webidl.converters["GPUBlendOperation"],
    defaultValue: "add",
  },
];
webidl.converters["GPUBlendComponent"] = webidl.createDictionaryConverter(
  "GPUBlendComponent",
  dictMembersGPUBlendComponent,
);

// DICTIONARY: GPUBlendState
const dictMembersGPUBlendState = [
  {
    key: "color",
    converter: webidl.converters["GPUBlendComponent"],
    required: true,
  },
  {
    key: "alpha",
    converter: webidl.converters["GPUBlendComponent"],
    required: true,
  },
];
webidl.converters["GPUBlendState"] = webidl.createDictionaryConverter(
  "GPUBlendState",
  dictMembersGPUBlendState,
);

// TYPEDEF: GPUColorWriteFlags
webidl.converters["GPUColorWriteFlags"] = (V, opts) =>
  webidl.converters["unsigned long"](V, { ...opts, enforceRange: true });

// DICTIONARY: GPUColorTargetState
const dictMembersGPUColorTargetState = [
  {
    key: "format",
    converter: webidl.converters["GPUTextureFormat"],
    required: true,
  },
  { key: "blend", converter: webidl.converters["GPUBlendState"] },
  {
    key: "writeMask",
    converter: webidl.converters["GPUColorWriteFlags"],
    defaultValue: 0xF,
  },
];
webidl.converters["GPUColorTargetState"] = webidl.createDictionaryConverter(
  "GPUColorTargetState",
  dictMembersGPUColorTargetState,
);

// DICTIONARY: GPUFragmentState
const dictMembersGPUFragmentState = [
  {
    key: "targets",
    converter: webidl.createSequenceConverter(
      webidl.createNullableConverter(
        webidl.converters["GPUColorTargetState"],
      ),
    ),
    required: true,
  },
];
webidl.converters["GPUFragmentState"] = webidl.createDictionaryConverter(
  "GPUFragmentState",
  dictMembersGPUProgrammableStage,
  dictMembersGPUFragmentState,
);

// DICTIONARY: GPURenderPipelineDescriptor
const dictMembersGPURenderPipelineDescriptor = [
  {
    key: "vertex",
    converter: webidl.converters["GPUVertexState"],
    required: true,
  },
  {
    key: "primitive",
    converter: webidl.converters["GPUPrimitiveState"],
    get defaultValue() {
      return {};
    },
  },
  {
    key: "depthStencil",
    converter: webidl.converters["GPUDepthStencilState"],
  },
  {
    key: "multisample",
    converter: webidl.converters["GPUMultisampleState"],
    get defaultValue() {
      return {};
    },
  },
  { key: "fragment", converter: webidl.converters["GPUFragmentState"] },
];
webidl.converters["GPURenderPipelineDescriptor"] = webidl
  .createDictionaryConverter(
    "GPURenderPipelineDescriptor",
    dictMembersGPUObjectDescriptorBase,
    dictMembersGPUPipelineDescriptorBase,
    dictMembersGPURenderPipelineDescriptor,
  );

// INTERFACE: GPUColorWrite
webidl.converters.GPUColorWrite = webidl.createInterfaceConverter(
  "GPUColorWrite",
  GPUColorWrite.prototype,
);

// INTERFACE: GPUCommandBuffer
webidl.converters.GPUCommandBuffer = webidl.createInterfaceConverter(
  "GPUCommandBuffer",
  GPUCommandBuffer.prototype,
);
webidl.converters["sequence<GPUCommandBuffer>"] = webidl
  .createSequenceConverter(webidl.converters["GPUCommandBuffer"]);

// DICTIONARY: GPUCommandBufferDescriptor
const dictMembersGPUCommandBufferDescriptor = [];
webidl.converters["GPUCommandBufferDescriptor"] = webidl
  .createDictionaryConverter(
    "GPUCommandBufferDescriptor",
    dictMembersGPUObjectDescriptorBase,
    dictMembersGPUCommandBufferDescriptor,
  );

// INTERFACE: GPUCommandEncoder
webidl.converters.GPUCommandEncoder = webidl.createInterfaceConverter(
  "GPUCommandEncoder",
  GPUCommandEncoder.prototype,
);

// DICTIONARY: GPUCommandEncoderDescriptor
const dictMembersGPUCommandEncoderDescriptor = [];
webidl.converters["GPUCommandEncoderDescriptor"] = webidl
  .createDictionaryConverter(
    "GPUCommandEncoderDescriptor",
    dictMembersGPUObjectDescriptorBase,
    dictMembersGPUCommandEncoderDescriptor,
  );

// DICTIONARY: GPUImageDataLayout
const dictMembersGPUImageDataLayout = [
  {
    key: "offset",
    converter: webidl.converters["GPUSize64"],
    defaultValue: 0,
  },
  { key: "bytesPerRow", converter: webidl.converters["GPUSize32"] },
  { key: "rowsPerImage", converter: webidl.converters["GPUSize32"] },
];
webidl.converters["GPUImageDataLayout"] = webidl.createDictionaryConverter(
  "GPUImageDataLayout",
  dictMembersGPUImageDataLayout,
);

// DICTIONARY: GPUImageCopyBuffer
const dictMembersGPUImageCopyBuffer = [
  {
    key: "buffer",
    converter: webidl.converters["GPUBuffer"],
    required: true,
  },
];
webidl.converters["GPUImageCopyBuffer"] = webidl.createDictionaryConverter(
  "GPUImageCopyBuffer",
  dictMembersGPUImageDataLayout,
  dictMembersGPUImageCopyBuffer,
);

// DICTIONARY: GPUOrigin3DDict
const dictMembersGPUOrigin3DDict = [
  {
    key: "x",
    converter: webidl.converters["GPUIntegerCoordinate"],
    defaultValue: 0,
  },
  {
    key: "y",
    converter: webidl.converters["GPUIntegerCoordinate"],
    defaultValue: 0,
  },
  {
    key: "z",
    converter: webidl.converters["GPUIntegerCoordinate"],
    defaultValue: 0,
  },
];
webidl.converters["GPUOrigin3DDict"] = webidl.createDictionaryConverter(
  "GPUOrigin3DDict",
  dictMembersGPUOrigin3DDict,
);

// TYPEDEF: GPUOrigin3D
webidl.converters["GPUOrigin3D"] = (V, opts) => {
  // Union for (sequence<GPUIntegerCoordinate> or GPUOrigin3DDict)
  if (V === null || V === undefined) {
    return webidl.converters["GPUOrigin3DDict"](V, opts);
  }
  if (typeof V === "object") {
    const method = V[SymbolIterator];
    if (method !== undefined) {
      // validate length of GPUOrigin3D
      const length = 3;
      if (V.length > length) {
        throw webidl.makeException(
          TypeError,
          `A sequence of number used as a GPUOrigin3D must have at most ${length} elements.`,
          opts,
        );
      }
      return webidl.converters["sequence<GPUIntegerCoordinate>"](V, opts);
    }
    return webidl.converters["GPUOrigin3DDict"](V, opts);
  }
  throw webidl.makeException(
    TypeError,
    "can not be converted to sequence<GPUIntegerCoordinate> or GPUOrigin3DDict.",
    opts,
  );
};

// DICTIONARY: GPUImageCopyTexture
const dictMembersGPUImageCopyTexture = [
  {
    key: "texture",
    converter: webidl.converters["GPUTexture"],
    required: true,
  },
  {
    key: "mipLevel",
    converter: webidl.converters["GPUIntegerCoordinate"],
    defaultValue: 0,
  },
  {
    key: "origin",
    converter: webidl.converters["GPUOrigin3D"],
    get defaultValue() {
      return {};
    },
  },
  {
    key: "aspect",
    converter: webidl.converters["GPUTextureAspect"],
    defaultValue: "all",
  },
];
webidl.converters["GPUImageCopyTexture"] = webidl.createDictionaryConverter(
  "GPUImageCopyTexture",
  dictMembersGPUImageCopyTexture,
);

// DICTIONARY: GPUOrigin2DDict
const dictMembersGPUOrigin2DDict = [
  {
    key: "x",
    converter: webidl.converters["GPUIntegerCoordinate"],
    defaultValue: 0,
  },
  {
    key: "y",
    converter: webidl.converters["GPUIntegerCoordinate"],
    defaultValue: 0,
  },
];
webidl.converters["GPUOrigin2DDict"] = webidl.createDictionaryConverter(
  "GPUOrigin2DDict",
  dictMembersGPUOrigin2DDict,
);

// TYPEDEF: GPUOrigin2D
webidl.converters["GPUOrigin2D"] = (V, opts) => {
  // Union for (sequence<GPUIntegerCoordinate> or GPUOrigin2DDict)
  if (V === null || V === undefined) {
    return webidl.converters["GPUOrigin2DDict"](V, opts);
  }
  if (typeof V === "object") {
    const method = V[SymbolIterator];
    if (method !== undefined) {
      // validate length of GPUOrigin2D
      const length = 2;
      if (V.length > length) {
        throw webidl.makeException(
          TypeError,
          `A sequence of number used as a GPUOrigin2D must have at most ${length} elements.`,
          opts,
        );
      }
      return webidl.converters["sequence<GPUIntegerCoordinate>"](V, opts);
    }
    return webidl.converters["GPUOrigin2DDict"](V, opts);
  }
  throw webidl.makeException(
    TypeError,
    "can not be converted to sequence<GPUIntegerCoordinate> or GPUOrigin2DDict.",
    opts,
  );
};

// INTERFACE: GPUComputePassEncoder
webidl.converters.GPUComputePassEncoder = webidl.createInterfaceConverter(
  "GPUComputePassEncoder",
  GPUComputePassEncoder.prototype,
);

// DICTIONARY: GPUComputePassTimestampWrites
webidl.converters["GPUComputePassTimestampWrites"] = webidl
  .createDictionaryConverter(
    "GPUComputePassTimestampWrites",
    [
      {
        key: "querySet",
        converter: webidl.converters["GPUQuerySet"],
        required: true,
      },
      {
        key: "beginningOfPassWriteIndex",
        converter: webidl.converters["GPUSize32"],
      },
      {
        key: "endOfPassWriteIndex",
        converter: webidl.converters["GPUSize32"],
      },
    ],
  );

// DICTIONARY: GPUComputePassDescriptor
const dictMembersGPUComputePassDescriptor = [
  {
    key: "timestampWrites",
    converter: webidl.converters["GPUComputePassTimestampWrites"],
  },
];
webidl.converters["GPUComputePassDescriptor"] = webidl
  .createDictionaryConverter(
    "GPUComputePassDescriptor",
    dictMembersGPUObjectDescriptorBase,
    dictMembersGPUComputePassDescriptor,
  );

// INTERFACE: GPURenderPassEncoder
webidl.converters.GPURenderPassEncoder = webidl.createInterfaceConverter(
  "GPURenderPassEncoder",
  GPURenderPassEncoder.prototype,
);

// ENUM: GPULoadOp
webidl.converters["GPULoadOp"] = webidl.createEnumConverter("GPULoadOp", [
  "load",
  "clear",
]);

// DICTIONARY: GPUColorDict
const dictMembersGPUColorDict = [
  { key: "r", converter: webidl.converters["double"], required: true },
  { key: "g", converter: webidl.converters["double"], required: true },
  { key: "b", converter: webidl.converters["double"], required: true },
  { key: "a", converter: webidl.converters["double"], required: true },
];
webidl.converters["GPUColorDict"] = webidl.createDictionaryConverter(
  "GPUColorDict",
  dictMembersGPUColorDict,
);

// TYPEDEF: GPUColor
webidl.converters["GPUColor"] = (V, opts) => {
  // Union for (sequence<double> or GPUColorDict)
  if (V === null || V === undefined) {
    return webidl.converters["GPUColorDict"](V, opts);
  }
  if (typeof V === "object") {
    const method = V[SymbolIterator];
    if (method !== undefined) {
      // validate length of GPUColor
      const length = 4;
      if (V.length !== length) {
        throw webidl.makeException(
          TypeError,
          `A sequence of number used as a GPUColor must have exactly ${length} elements.`,
          opts,
        );
      }
      return webidl.converters["sequence<double>"](V, opts);
    }
    return webidl.converters["GPUColorDict"](V, opts);
  }
  throw webidl.makeException(
    TypeError,
    "can not be converted to sequence<double> or GPUColorDict.",
    opts,
  );
};

// ENUM: GPUStoreOp
webidl.converters["GPUStoreOp"] = webidl.createEnumConverter("GPUStoreOp", [
  "store",
  "discard",
]);

// DICTIONARY: GPURenderPassColorAttachment
const dictMembersGPURenderPassColorAttachment = [
  {
    key: "view",
    converter: webidl.converters["GPUTextureView"],
    required: true,
  },
  { key: "resolveTarget", converter: webidl.converters["GPUTextureView"] },
  {
    key: "clearValue",
    converter: webidl.converters["GPUColor"],
  },
  {
    key: "loadOp",
    converter: webidl.converters["GPULoadOp"],
    required: true,
  },
  {
    key: "storeOp",
    converter: webidl.converters["GPUStoreOp"],
    required: true,
  },
];
webidl.converters["GPURenderPassColorAttachment"] = webidl
  .createDictionaryConverter(
    "GPURenderPassColorAttachment",
    dictMembersGPURenderPassColorAttachment,
  );

// DICTIONARY: GPURenderPassDepthStencilAttachment
const dictMembersGPURenderPassDepthStencilAttachment = [
  {
    key: "view",
    converter: webidl.converters["GPUTextureView"],
    required: true,
  },
  {
    key: "depthClearValue",
    converter: webidl.converters["float"],
  },
  {
    key: "depthLoadOp",
    converter: webidl.converters["GPULoadOp"],
  },
  {
    key: "depthStoreOp",
    converter: webidl.converters["GPUStoreOp"],
  },
  {
    key: "depthReadOnly",
    converter: webidl.converters["boolean"],
    defaultValue: false,
  },
  {
    key: "stencilClearValue",
    converter: webidl.converters["GPUStencilValue"],
    defaultValue: 0,
  },
  {
    key: "stencilLoadOp",
    converter: webidl.converters["GPULoadOp"],
  },
  {
    key: "stencilStoreOp",
    converter: webidl.converters["GPUStoreOp"],
  },
  {
    key: "stencilReadOnly",
    converter: webidl.converters["boolean"],
    defaultValue: false,
  },
];
webidl.converters["GPURenderPassDepthStencilAttachment"] = webidl
  .createDictionaryConverter(
    "GPURenderPassDepthStencilAttachment",
    dictMembersGPURenderPassDepthStencilAttachment,
  );

// INTERFACE: GPUQuerySet
webidl.converters.GPUQuerySet = webidl.createInterfaceConverter(
  "GPUQuerySet",
  GPUQuerySet.prototype,
);

// DICTIONARY: GPURenderPassTimestampWrites
webidl.converters["GPURenderPassTimestampWrites"] = webidl
  .createDictionaryConverter(
    "GPURenderPassTimestampWrites",
    [
      {
        key: "querySet",
        converter: webidl.converters["GPUQuerySet"],
        required: true,
      },
      {
        key: "beginningOfPassWriteIndex",
        converter: webidl.converters["GPUSize32"],
      },
      {
        key: "endOfPassWriteIndex",
        converter: webidl.converters["GPUSize32"],
      },
    ],
  );

// DICTIONARY: GPURenderPassDescriptor
const dictMembersGPURenderPassDescriptor = [
  {
    key: "colorAttachments",
    converter: webidl.createSequenceConverter(
      webidl.createNullableConverter(
        webidl.converters["GPURenderPassColorAttachment"],
      ),
    ),
    required: true,
  },
  {
    key: "depthStencilAttachment",
    converter: webidl.converters["GPURenderPassDepthStencilAttachment"],
  },
  {
    key: "occlusionQuerySet",
    converter: webidl.converters["GPUQuerySet"],
  },
  {
    key: "timestampWrites",
    converter: webidl.converters["GPURenderPassTimestampWrites"],
  },
];
webidl.converters["GPURenderPassDescriptor"] = webidl
  .createDictionaryConverter(
    "GPURenderPassDescriptor",
    dictMembersGPUObjectDescriptorBase,
    dictMembersGPURenderPassDescriptor,
  );

// INTERFACE: GPURenderBundle
webidl.converters.GPURenderBundle = webidl.createInterfaceConverter(
  "GPURenderBundle",
  GPURenderBundle.prototype,
);
webidl.converters["sequence<GPURenderBundle>"] = webidl
  .createSequenceConverter(webidl.converters["GPURenderBundle"]);

// DICTIONARY: GPURenderBundleDescriptor
const dictMembersGPURenderBundleDescriptor = [];
webidl.converters["GPURenderBundleDescriptor"] = webidl
  .createDictionaryConverter(
    "GPURenderBundleDescriptor",
    dictMembersGPUObjectDescriptorBase,
    dictMembersGPURenderBundleDescriptor,
  );

// INTERFACE: GPURenderBundleEncoder
webidl.converters.GPURenderBundleEncoder = webidl.createInterfaceConverter(
  "GPURenderBundleEncoder",
  GPURenderBundleEncoder.prototype,
);

// DICTIONARY: GPURenderPassLayout
const dictMembersGPURenderPassLayout = [
  {
    key: "colorFormats",
    converter: webidl.createSequenceConverter(
      webidl.createNullableConverter(webidl.converters["GPUTextureFormat"]),
    ),
    required: true,
  },
  {
    key: "depthStencilFormat",
    converter: webidl.converters["GPUTextureFormat"],
  },
  {
    key: "sampleCount",
    converter: webidl.converters["GPUSize32"],
    defaultValue: 1,
  },
];
webidl.converters["GPURenderPassLayout"] = webidl
  .createDictionaryConverter(
    "GPURenderPassLayout",
    dictMembersGPUObjectDescriptorBase,
    dictMembersGPURenderPassLayout,
  );

// DICTIONARY: GPURenderBundleEncoderDescriptor
const dictMembersGPURenderBundleEncoderDescriptor = [
  {
    key: "depthReadOnly",
    converter: webidl.converters.boolean,
    defaultValue: false,
  },
  {
    key: "stencilReadOnly",
    converter: webidl.converters.boolean,
    defaultValue: false,
  },
];
webidl.converters["GPURenderBundleEncoderDescriptor"] = webidl
  .createDictionaryConverter(
    "GPURenderBundleEncoderDescriptor",
    dictMembersGPUObjectDescriptorBase,
    dictMembersGPURenderPassLayout,
    dictMembersGPURenderBundleEncoderDescriptor,
  );

// INTERFACE: GPUQueue
webidl.converters.GPUQueue = webidl.createInterfaceConverter(
  "GPUQueue",
  GPUQueue.prototype,
);

// ENUM: GPUQueryType
webidl.converters["GPUQueryType"] = webidl.createEnumConverter(
  "GPUQueryType",
  [
    "occlusion",
    "timestamp",
  ],
);

// DICTIONARY: GPUQuerySetDescriptor
const dictMembersGPUQuerySetDescriptor = [
  {
    key: "type",
    converter: webidl.converters["GPUQueryType"],
    required: true,
  },
  { key: "count", converter: webidl.converters["GPUSize32"], required: true },
  {
    key: "pipelineStatistics",
    converter: webidl.createSequenceConverter(
      webidl.converters["GPUPipelineStatisticName"],
    ),
    get defaultValue() {
      return [];
    },
  },
];
webidl.converters["GPUQuerySetDescriptor"] = webidl.createDictionaryConverter(
  "GPUQuerySetDescriptor",
  dictMembersGPUObjectDescriptorBase,
  dictMembersGPUQuerySetDescriptor,
);

// ENUM: GPUDeviceLostReason
webidl.converters["GPUDeviceLostReason"] = webidl.createEnumConverter(
  "GPUDeviceLostReason",
  [
    "destroyed",
  ],
);

// // INTERFACE: GPUDeviceLostInfo
// webidl.converters.GPUDeviceLostInfo = webidl.createInterfaceConverter(
//   "GPUDeviceLostInfo",
//   GPUDeviceLostInfo.prototype,
// );

// ENUM: GPUErrorFilter
webidl.converters["GPUErrorFilter"] = webidl.createEnumConverter(
  "GPUErrorFilter",
  [
    "out-of-memory",
    "validation",
    "internal",
  ],
);

// INTERFACE: GPUOutOfMemoryError
webidl.converters.GPUOutOfMemoryError = webidl.createInterfaceConverter(
  "GPUOutOfMemoryError",
  GPUOutOfMemoryError.prototype,
);

// INTERFACE: GPUValidationError
webidl.converters.GPUValidationError = webidl.createInterfaceConverter(
  "GPUValidationError",
  GPUValidationError.prototype,
);

// TYPEDEF: GPUError
webidl.converters["GPUError"] = webidl.converters.any /** put union here! **/;

// // INTERFACE: GPUUncapturedErrorEvent
// webidl.converters.GPUUncapturedErrorEvent = webidl.createInterfaceConverter(
//   "GPUUncapturedErrorEvent",
//   GPUUncapturedErrorEvent.prototype,
// );

// DICTIONARY: GPUUncapturedErrorEventInit
const dictMembersGPUUncapturedErrorEventInit = [
  { key: "error", converter: webidl.converters["GPUError"], required: true },
];
webidl.converters["GPUUncapturedErrorEventInit"] = webidl
  .createDictionaryConverter(
    "GPUUncapturedErrorEventInit",
    // dictMembersEventInit,
    dictMembersGPUUncapturedErrorEventInit,
  );

// TYPEDEF: GPUBufferDynamicOffset
webidl.converters["GPUBufferDynamicOffset"] = (V, opts) =>
  webidl.converters["unsigned long"](V, { ...opts, enforceRange: true });

// TYPEDEF: GPUSignedOffset32
webidl.converters["GPUSignedOffset32"] = (V, opts) =>
  webidl.converters["long"](V, { ...opts, enforceRange: true });

// TYPEDEF: GPUFlagsConstant
webidl.converters["GPUFlagsConstant"] = webidl.converters["unsigned long"];

// ENUM: GPUCanvasAlphaMode
webidl.converters["GPUCanvasAlphaMode"] = webidl.createEnumConverter(
  "GPUCanvasAlphaMode",
  [
    "opaque",
    "premultiplied",
  ],
);

// NON-SPEC: ENUM: GPUPresentMode
webidl.converters["GPUPresentMode"] = webidl.createEnumConverter(
  "GPUPresentMode",
  [
    "autoVsync",
    "autoNoVsync",
    "fifo",
    "fifoRelaxed",
    "immediate",
    "mailbox",
  ],
);

// DICT: GPUCanvasConfiguration
const dictMembersGPUCanvasConfiguration = [
  { key: "device", converter: webidl.converters.GPUDevice, required: true },
  {
    key: "format",
    converter: webidl.converters.GPUTextureFormat,
    required: true,
  },
  {
    key: "usage",
    converter: webidl.converters["GPUTextureUsageFlags"],
    defaultValue: GPUTextureUsage.RENDER_ATTACHMENT,
  },
  {
    key: "alphaMode",
    converter: webidl.converters["GPUCanvasAlphaMode"],
    defaultValue: "opaque",
  },

  // Extended from spec
  {
    key: "presentMode",
    converter: webidl.converters["GPUPresentMode"],
  },
  {
    key: "viewFormats",
    converter: webidl.createSequenceConverter(
      webidl.converters["GPUTextureFormat"],
    ),
    get defaultValue() {
      return [];
    },
  },
];
webidl.converters["GPUCanvasConfiguration"] = webidl
  .createDictionaryConverter(
    "GPUCanvasConfiguration",
    dictMembersGPUCanvasConfiguration,
  );

const gpu = webidl.createBranded(GPU);
export {
  _device,
  assertDevice,
  createGPUTexture,
  GPU,
  gpu,
  GPUAdapter,
  GPUAdapterInfo,
  GPUBindGroup,
  GPUBindGroupLayout,
  GPUBuffer,
  GPUBufferUsage,
  GPUColorWrite,
  GPUCommandBuffer,
  GPUCommandEncoder,
  GPUComputePassEncoder,
  GPUComputePipeline,
  GPUDevice,
  GPUDeviceLostInfo,
  GPUError,
  GPUInternalError,
  GPUMapMode,
  GPUOutOfMemoryError,
  GPUPipelineLayout,
  GPUQuerySet,
  GPUQueue,
  GPURenderBundle,
  GPURenderBundleEncoder,
  GPURenderPassEncoder,
  GPURenderPipeline,
  GPUSampler,
  GPUShaderModule,
  GPUShaderStage,
  GPUSupportedFeatures,
  GPUSupportedLimits,
  GPUTexture,
  GPUTextureUsage,
  GPUTextureView,
  GPUUncapturedErrorEvent,
  GPUValidationError,
};
