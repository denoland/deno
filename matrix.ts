// deno-fmt-ignore
const seq2d = [1, 0, 0, 1, 10, 20];
const dim2 = new Float32Array(seq2d);

Deno.bench("2d", () => {
  DOMMatrix.fromFloat32Array(dim2);
});

Deno.bench("2d-readonly", () => {
  DOMMatrixReadOnly.fromFloat32Array(dim2);
});

Deno.bench("2d-sequence", () => {
  new DOMMatrix(seq2d);
});

Deno.bench("2d-sequence-readonly", () => {
  new DOMMatrixReadOnly(seq2d);
});

// deno-fmt-ignore
const seq3d = [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 10, 20, 30, 1];
const dim3 = new Float32Array(seq3d);

Deno.bench("3d", () => {
  DOMMatrix.fromFloat32Array(dim3);
});

Deno.bench("3d-readonly", () => {
  DOMMatrixReadOnly.fromFloat32Array(dim3);
});

Deno.bench("3d-sequence", () => {
  new DOMMatrix(seq3d);
});

Deno.bench("3d-sequence-readonly", () => {
  new DOMMatrixReadOnly(seq3d);
});
