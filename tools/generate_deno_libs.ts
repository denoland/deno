#!/usr/bin/env -S deno run -A
// Copyright 2018-2026 the Deno authors. MIT license.
//
// Axis-2 interface-shape de-fork generator. See denoland/deno#36094 for the
// two-axis model and the correctness invariant.
//
// Deno ships forked web-platform lib declarations in cli/tsc/dts/lib.deno_*.d.ts
// that fully redefine types stock TypeScript already ships in lib.dom /
// lib.webworker. Two global `interface X { ... }` declarations merge silently
// under declaration merging IFF their members are compatible; a member whose
// *resolved type* or *modifiers* differ produces a hard TS2717 / TS2687 / TS2374
// / TS2428 error once `skipLibCheck` is off and the shapes co-load with `dom`.
//
// Axis 1 (global `declare var` bindings) is handled separately by
// tools/apply_web_globals_deferral.ts and is NOT touched here. This generator is
// the Axis-2 counterpart: it reconciles the small set of interface members that
// have *accidentally* drifted from stock (a literal narrowed to `number`, a
// property that lost its `?`, `any` where stock says `null`, ...) back to the
// stock member text, so the shapes merge cleanly. Members where Deno *genuinely*
// differs from stock on purpose (narrower WebTransport stream element types, the
// `ReadableStreamBYOBRequest.view` narrowing, `WebSocket.onerror` accepting
// `ErrorEvent`, ...) are left alone and listed under KEEP below.
//
// The transform derives the replacement text from the stock lib AST (so it stays
// in sync on TS bumps), splices the ORIGINAL file text at the member's span (so
// JSDoc, indentation and everything else are preserved byte-for-byte), applies
// edits back-to-front, and is idempotent (a member already equal to stock is
// skipped).
//
// deno-lint-ignore-file no-console

import { Project } from "jsr:@ts-morph/ts-morph@27.0.2";
import type {
  InterfaceDeclaration,
  SourceFile,
  TypeElementTypes,
} from "jsr:@ts-morph/ts-morph@27.0.2";

const dtsDir = new URL("../cli/tsc/dts/", import.meta.url).pathname;

// Deno interface members that have accidentally drifted from stock and are safe
// to realign (the replacement type references only literals / primitives / types
// Deno's own libs also declare, so the no-`dom` default config stays valid).
// `stock` selects which stock lib to copy the member text from.
interface Reconcile {
  file: string;
  iface: string;
  member: string;
  stock: "dom" | "webworker";
}
const RECONCILE: Reconcile[] = [
  // `window` on `RequestInit` is a vestigial slot; stock types it `null`, Deno
  // drifted to `any`.
  {
    file: "lib.deno_fetch.d.ts",
    iface: "RequestInit",
    member: "window",
    stock: "dom",
  },
  // The WebSocket ready-state constants are the literal runtime values; Deno
  // widened them to `number`.
  {
    file: "lib.deno_websocket.d.ts",
    iface: "WebSocket",
    member: "readyState",
    stock: "dom",
  },
  {
    file: "lib.deno_websocket.d.ts",
    iface: "WebSocket",
    member: "CONNECTING",
    stock: "dom",
  },
  {
    file: "lib.deno_websocket.d.ts",
    iface: "WebSocket",
    member: "OPEN",
    stock: "dom",
  },
  {
    file: "lib.deno_websocket.d.ts",
    iface: "WebSocket",
    member: "CLOSING",
    stock: "dom",
  },
  {
    file: "lib.deno_websocket.d.ts",
    iface: "WebSocket",
    member: "CLOSED",
    stock: "dom",
  },
  // `ignoreCase` is optional in stock; Deno dropped the `?`.
  {
    file: "lib.deno_url.d.ts",
    iface: "URLPatternOptions",
    member: "ignoreCase",
    stock: "dom",
  },
  // A `GPUBindGroupLayout` slot in the layout array may be `null` in stock.
  {
    file: "lib.deno_webgpu.d.ts",
    iface: "GPUPipelineLayoutDescriptor",
    member: "bindGroupLayouts",
    stock: "dom",
  },
  // `PredefinedColorSpace` is the stock alias for the same `"srgb" | "display-p3"`
  // union; Deno inlined the union.
  {
    file: "lib.deno_canvas.d.ts",
    iface: "GPUCanvasConfiguration",
    member: "colorSpace",
    stock: "dom",
  },
  // Init dictionaries carry no `readonly` in stock; Deno added it.
  {
    file: "lib.deno_web.d.ts",
    iface: "ImageDataSettings",
    member: "colorSpace",
    stock: "dom",
  },
  {
    file: "lib.deno_web.d.ts",
    iface: "ImageDataSettings",
    member: "pixelFormat",
    stock: "dom",
  },
  // `Window.navigator` is `readonly` in stock; `CSSRule.cssText` is writable.
  {
    file: "lib.deno.window.d.ts",
    iface: "Window",
    member: "navigator",
    stock: "dom",
  },
  {
    file: "lib.deno.unstable.d.ts",
    iface: "CSSRule",
    member: "cssText",
    stock: "dom",
  },
  // The stream read-done result's `value` is a required `T | undefined` in stock
  // (and in @types/node's node:stream/web); Deno drifted to an optional `T`,
  // which breaks assignability to Node's ReadableStream.
  {
    file: "lib.deno_web.d.ts",
    iface: "ReadableStreamReadDoneResult",
    member: "value",
    stock: "dom",
  },
];

// Members where Deno intentionally differs from stock. Left as-is; recorded here
// so the divergence is documented and auditable rather than silently skipped.
// These stay incompatible with a co-loaded `dom` on purpose; the `has_dom`
// deno.ns-only defer (cli/tsc/tsconfig_gen.rs) keeps them from colliding.
const KEEP: Array<{ iface: string; member: string; why: string }> = [
  {
    iface: "ReadableStreamBYOBRequest",
    member: "view",
    why:
      "Deno narrows view to Uint8Array (ext/web #33477); stock allows any ArrayBufferView",
  },
  {
    iface: "WebSocket",
    member: "onerror",
    why:
      "Deno's WebSocket emits ErrorEvent; stock types the handler event as Event",
  },
  {
    iface: "WebTransport",
    member: "incomingBidirectionalStreams",
    why:
      "Deno types the element as WebTransportBidirectionalStream; stock uses any",
  },
  {
    iface: "WebTransport",
    member: "incomingUnidirectionalStreams",
    why: "Deno types the element as WebTransportReceiveStream; stock uses any",
  },
  {
    iface: "WebTransportBidirectionalStream",
    member: "readable",
    why: "Deno types the readable as WebTransportReceiveStream",
  },
  {
    iface: "WebTransportBidirectionalStream",
    member: "writable",
    why: "Deno types the writable as WebTransportSendStream",
  },
  {
    iface: "WebTransportDatagramDuplexStream",
    member: "readable",
    why: "Deno types the readable element narrowly",
  },
  {
    iface: "WebTransportDatagramDuplexStream",
    member: "writable",
    why: "Deno types the writable element narrowly",
  },
  {
    iface: "ImageBitmapRenderingContext",
    member: "canvas",
    why: "Deno has no HTMLCanvasElement, so canvas is OffscreenCanvas only",
  },
  {
    iface: "GPURenderPassColorAttachment",
    member: "view",
    why: "Deno's WebGPU narrows view to GPUTextureView",
  },
  {
    iface: "GPURenderPassColorAttachment",
    member: "resolveTarget",
    why: "Deno's WebGPU narrows resolveTarget to GPUTextureView",
  },
  {
    iface: "GPURenderPassDepthStencilAttachment",
    member: "view",
    why: "Deno's WebGPU narrows view to GPUTextureView",
  },
  {
    iface: "CSSStyleSheet",
    member: "cssRules",
    why: "stock's CSSRuleList type is dom-only; Deno uses readonly CSSRule[]",
  },
  {
    iface: "URLPatternResult",
    member: "inputs",
    why:
      "Deno uses a precise input tuple; @types/node owns this family when present",
  },
  {
    iface: "GPUCanvasContext",
    member: "canvas",
    why: "stock's HTMLCanvasElement is dom-only; Deno has only OffscreenCanvas",
  },
  {
    iface: "TextDecoderStream",
    member: "writable",
    why: "Deno accepts AllowSharedBufferSource; stock narrows to BufferSource",
  },
  {
    iface: "WebTransportHash",
    member: "algorithm",
    why:
      "Deno's runtime webidl treats this as optional; stock/spec marks it required",
  },
  {
    iface: "WebTransportHash",
    member: "value",
    why:
      "Deno's runtime webidl treats this as optional; stock/spec marks it required",
  },
];

function normSig(text: string): string {
  return text.replace(/\s+/g, " ").replace(/;+\s*$/, "").trim();
}

function findMember(
  iface: InterfaceDeclaration,
  name: string,
): TypeElementTypes | undefined {
  return iface.getMembers().find((m) => {
    // deno-lint-ignore no-explicit-any
    const n = (m as any).getName?.();
    return n === name;
  });
}

const project = new Project({ useInMemoryFileSystem: false });

// Stock `lib.dom` / `lib.webworker` come from the pinned tsgo (the same compiler
// `deno check` runs), not from Deno's own `cli/tsc/dts/`. Resolve its `lib/`
// directory from `DENO_TSC_BIN` (`<lib>/tsc`), which `tools/download_tsc.ts`
// sets, so the reconciliation always tracks the version we ship against.
const denoTscBin = Deno.env.get("DENO_TSC_BIN");
if (!denoTscBin) {
  throw new Error(
    "DENO_TSC_BIN is not set. Run:\n" +
      "  export DENO_TSC_BIN=$(deno run -A tools/download_tsc.ts)\n" +
      "then re-run this generator.",
  );
}
const stockDir = denoTscBin.slice(0, denoTscBin.lastIndexOf("/") + 1);

// Stock member lookup.
const stockFiles: Record<string, SourceFile> = {
  dom: project.addSourceFileAtPath(stockDir + "lib.dom.d.ts"),
  webworker: project.addSourceFileAtPath(stockDir + "lib.webworker.d.ts"),
};

function stockMemberText(
  lib: "dom" | "webworker",
  iface: string,
  member: string,
): string {
  const sf = stockFiles[lib];
  const decl = sf.getInterface(iface);
  if (!decl) throw new Error(`stock ${lib} has no interface ${iface}`);
  const m = findMember(decl, member);
  if (!m) throw new Error(`stock ${lib} ${iface} has no member ${member}`);
  return m.getText();
}

const reconciled: string[] = [];
const skipped: string[] = [];

// Group reconcile entries by file so each file is parsed and written once.
const byFile = new Map<string, Reconcile[]>();
for (const r of RECONCILE) {
  const arr = byFile.get(r.file);
  if (arr) arr.push(r);
  else byFile.set(r.file, [r]);
}

for (const [file, entries] of byFile) {
  const sf = project.addSourceFileAtPath(dtsDir + file);
  let text = sf.getFullText();
  const edits: Array<[number, number, string]> = [];
  for (const entry of entries) {
    const iface = sf.getInterface(entry.iface);
    if (!iface) throw new Error(`${file} has no interface ${entry.iface}`);
    const member = findMember(iface, entry.member);
    if (!member) {
      throw new Error(`${file} ${entry.iface} has no member ${entry.member}`);
    }
    const stockText = stockMemberText(entry.stock, entry.iface, entry.member);
    if (normSig(member.getText()) === normSig(stockText)) {
      skipped.push(`${entry.iface}.${entry.member} (already stock)`);
      continue;
    }
    edits.push([member.getStart(), member.getEnd(), stockText]);
    reconciled.push(`${file}:${entry.iface}.${entry.member}`);
  }
  if (edits.length === 0) continue;
  edits.sort((a, b) => b[0] - a[0]);
  for (const [start, end, repl] of edits) {
    text = text.slice(0, start) + repl + text.slice(end);
  }
  await Deno.writeTextFile(dtsDir + file, text);
}

console.log(`reconciled ${reconciled.length} member(s) toward stock:`);
console.log(reconciled.sort().map((s) => `  ${s}`).join("\n"));
if (skipped.length) {
  console.log(`\nskipped ${skipped.length} (idempotent):`);
  console.log(skipped.sort().map((s) => `  ${s}`).join("\n"));
}
console.log(`\nkept ${KEEP.length} intentional Deno divergence(s):`);
for (const k of KEEP) console.log(`  ${k.iface}.${k.member} - ${k.why}`);
