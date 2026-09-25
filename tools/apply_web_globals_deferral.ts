#!/usr/bin/env -S deno run -A
// Copyright 2018-2026 the Deno authors. MIT license.
//
// Rewrites Deno's web-platform global `declare var` declarations so they DEFER
// to lib.dom when present and let stock @types/node defer to them otherwise,
// using the same conditional-type mechanism @types/node already uses to coexist
// with lib.dom:
//
//   declare var Request: <DenoType>;
// becomes
//   declare var Request: typeof globalThis extends { document: any; Request: infer T } ? T : <DenoType>;
//
// `document` is a DOM-exclusive marker (neither Deno nor @types/node declare it),
// so when real lib.dom is loaded Deno re-exports lib.dom's type (precedence
// dom > deno); when it is absent Deno keeps its own type, and @types/node's
// `typeof globalThis extends { onmessage: any; Request: infer T }` probe finds
// Deno's declaration and defers to it (precedence deno > node).
//
// The set of names to transform is derived from the vendored @types/node so it
// stays in sync on `@types/node` bumps. This transform is behaviour-neutral
// under the current forked tsc (Deno files' globalThis has no `document`, so the
// conditional always yields the original type); it only matters once the fork's
// global split is removed. Markers (onmessage/onabort/ReportingObserver) are
// added separately in the composition libs.
//
// deno-lint-ignore-file no-console

import { Project } from "jsr:@ts-morph/ts-morph@27.0.2";

const dtsDir = new URL("../cli/tsc/dts/", import.meta.url).pathname;
const modRs = new URL("../cli/tsc/mod.rs", import.meta.url).pathname;

// 1. Derive the overlap set from TYPES_NODE_IGNORABLE_NAMES in mod.rs - the
// canonical, in-repo list of globals that overlap with @types/node and that
// Deno owns. (The vendored cli/tsc/dts/node copy is already stripped, so it
// can't be used as the source.) Over-inclusion is harmless: the deferral
// conditional falls through to the original type when no other declarer exists.
const probeNames = new Set<string>();
const rs = await Deno.readTextFile(modRs);
const listMatch =
  /TYPES_NODE_IGNORABLE_NAMES:\s*&\[&str\]\s*=\s*&\[([\s\S]*?)\];/.exec(rs);
if (!listMatch) {
  throw new Error("could not find TYPES_NODE_IGNORABLE_NAMES in mod.rs");
}
for (const m of listMatch[1].matchAll(/"([^"]+)"/g)) probeNames.add(m[1]);
console.log(
  `overlap names from mod.rs (${probeNames.size}):`,
  [...probeNames].sort().join(", "),
);

// Web-platform globals that overlap stock `lib.dom` but NOT @types/node, so
// they're absent from TYPES_NODE_IGNORABLE_NAMES and stayed un-deferred -
// colliding (TS2403 / TS2300) with lib.dom under a source-level `dom` co-load,
// unlike every other web global. Defer them the same way. These are all
// `declare var`s that both Deno's libs and the pinned tsgo's lib.dom declare
// (verified). Deliberately curated to the pure web-platform *types* (CSS, DOM
// geometry, canvas/image, WebGPU, WebTransport, ...); environment/identity
// globals Deno owns (console, crypto, self, window/Window, location, the
// on*-event handlers, web storage) are intentionally excluded - they need a
// separate own-vs-defer decision. See denoland/deno#36094.
const domOverlapNames = [
  // WebGPU (dom declares ~35 `declare var GPU*`)
  "GPUCanvasContext",
  "GPUError",
  "GPUInternalError",
  "GPUOutOfMemoryError",
  "GPUPipelineError",
  "GPUValidationError",
  // CSS Object Model
  "CSSRule",
  "CSSStyleSheet",
  // DOM geometry
  "DOMMatrix",
  "DOMMatrixReadOnly",
  "DOMPoint",
  "DOMPointReadOnly",
  "DOMQuad",
  "DOMRect",
  "DOMRectReadOnly",
  // Canvas / imaging
  "ImageBitmap",
  "ImageBitmapRenderingContext",
  "ImageData",
  "OffscreenCanvas",
  // File & events
  "FileReader",
  "ProgressEvent",
  "PromiseRejectionEvent",
  // Cache API
  "Cache",
  "CacheStorage",
  "caches",
  // WebTransport
  "WebTransport",
  "WebTransportBidirectionalStream",
  "WebTransportDatagramDuplexStream",
  "WebTransportError",
  // Misc web platform
  "Notification",
  "PermissionStatus",
  "Worker",
  // Web types split from `declare class` into interface + var by
  // tools/defork_classes.ts - their new `declare var` overlaps lib.dom's.
  "GPU",
  "GPUAdapter",
  "GPUAdapterInfo",
  "GPUBindGroup",
  "GPUBindGroupLayout",
  "GPUBuffer",
  "GPUCommandBuffer",
  "GPUCommandEncoder",
  "GPUCompilationInfo",
  "GPUCompilationMessage",
  "GPUComputePassEncoder",
  "GPUComputePipeline",
  "GPUDevice",
  "GPUPipelineLayout",
  "GPUQuerySet",
  "GPUQueue",
  "GPURenderBundle",
  "GPURenderBundleEncoder",
  "GPURenderPassEncoder",
  "GPURenderPipeline",
  "GPUSampler",
  "GPUShaderModule",
  "GPUSupportedFeatures",
  "GPUSupportedLimits",
  "GPUTexture",
  "GPUTextureView",
  "GPUUncapturedErrorEvent",
  "FocusEvent",
  "KeyboardEvent",
  "MouseEvent",
  "UIEvent",
  "WheelEvent",
];
for (const name of domOverlapNames) probeNames.add(name);

// 2. Locate the type-node span of each matching `declare var NAME` with the AST,
// then splice the ORIGINAL file text so only the type annotation changes -
// comments, formatting and everything else are preserved byte-for-byte.
const project = new Project({ useInMemoryFileSystem: false });
const libGlob = dtsDir + "lib.deno*.d.ts";
project.addSourceFilesAtPaths(libGlob);

const transformed: string[] = [];
const skipped: string[] = [];
for (const sf of project.getSourceFiles()) {
  const path = sf.getFilePath();
  let text = sf.getFullText();
  // Collect edits as [start, end, replacement], applied back-to-front.
  const edits: Array<[number, number, string]> = [];
  for (const stmt of sf.getVariableStatements()) {
    if (!stmt.hasDeclareKeyword()) continue;
    for (const decl of stmt.getDeclarations()) {
      const name = decl.getName();
      if (!probeNames.has(name)) continue;
      const typeNode = decl.getTypeNode();
      if (!typeNode) continue;
      const orig = typeNode.getText();
      if (orig.includes("infer T") && orig.includes("{ document")) {
        skipped.push(`${name} (already)`);
        continue;
      }
      edits.push([
        typeNode.getStart(),
        typeNode.getEnd(),
        `typeof globalThis extends { document: any; ${name}: infer T } ? T : ${orig}`,
      ]);
      transformed.push(`${sf.getBaseName()}:${name}`);
    }
  }
  if (edits.length === 0) continue;
  edits.sort((a, b) => b[0] - a[0]);
  for (const [start, end, repl] of edits) {
    text = text.slice(0, start) + repl + text.slice(end);
  }
  await Deno.writeTextFile(path, text);
}

console.log(`\ntransformed ${transformed.length}:`);
console.log(transformed.sort().join("\n"));
if (skipped.length) console.log(`\nskipped: ${skipped.join(", ")}`);
