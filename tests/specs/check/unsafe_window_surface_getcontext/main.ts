// Type-level checks that Deno.UnsafeWindowSurface.getContext narrows per
// context id, mirroring the OffscreenCanvas.getContext overloads. Strict
// equality (not assignability) is used so the exact return types - and the
// overload order, which is the easiest thing to break later - are pinned.

type Equals<A, B> = (<T>() => T extends A ? 1 : 2) extends
  (<T>() => T extends B ? 1 : 2) ? true : false;
type AssertTrue<T extends true> = T;

declare const surface: Deno.UnsafeWindowSurface;

const webgpu = surface.getContext("webgpu");
type _Webgpu = AssertTrue<Equals<typeof webgpu, GPUCanvasContext | null>>;

const bitmaprenderer = surface.getContext("bitmaprenderer");
type _Bitmaprenderer = AssertTrue<
  Equals<typeof bitmaprenderer, ImageBitmapRenderingContext | null>
>;

declare const contextId: OffscreenRenderingContextId;
const dynamic = surface.getContext(contextId);
type _Dynamic = AssertTrue<
  Equals<typeof dynamic, OffscreenRenderingContext | null>
>;

// Not implemented by Deno; getContext returns null for these context ids.
const unimplemented = surface.getContext("2d");
type _Unimplemented = AssertTrue<Equals<typeof unimplemented, null>>;

// The constructor objects have construct signatures, so instanceof narrows.
declare const context: unknown;
if (context instanceof GPUCanvasContext) {
  type _Gpu = AssertTrue<Equals<typeof context, GPUCanvasContext>>;
}
if (context instanceof ImageBitmapRenderingContext) {
  type _Bitmap = AssertTrue<
    Equals<typeof context, ImageBitmapRenderingContext>
  >;
}
