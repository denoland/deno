// Type-level checks that Deno.UnsafeWindowSurface.getContext narrows per
// context id, mirroring the OffscreenCanvas.getContext overloads.

function assertType<T>(_value: T): void {}

declare const surface: Deno.UnsafeWindowSurface;

assertType<GPUCanvasContext | null>(surface.getContext("webgpu"));
assertType<ImageBitmapRenderingContext | null>(
  surface.getContext("bitmaprenderer"),
);
declare const contextId: OffscreenRenderingContextId;
assertType<OffscreenRenderingContext | null>(surface.getContext(contextId));

// The constructor objects have construct signatures, so instanceof narrows.
declare const context: unknown;
if (context instanceof GPUCanvasContext) {
  assertType<GPUCanvasContext>(context);
}
if (context instanceof ImageBitmapRenderingContext) {
  assertType<ImageBitmapRenderingContext>(context);
}
