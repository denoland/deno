// Input to the shader. The length of the array is determined by what buffer is bound.
//
// Out of bounds accesses
@group(0) @binding(0)
var<storage, read> input: array<f32>;
// Output of the shader.
@group(0) @binding(1)
var<storage, read_write> output: array<f32>;

// Ideal workgroup size depends on the hardware, the workload, and other factors. However, it should
// _generally_ be a multiple of 64. Common sizes are 64x1x1, 256x1x1; or 8x8x1, 16x16x1 for 2D workloads.
@compute @workgroup_size(64)
fn doubleMe(@builtin(global_invocation_id) global_id: vec3<u32>) {
    // While compute invocations are 3d, we're only using one dimension.
    let index = global_id.x;

    // Because we're using a workgroup size of 64, if the input size isn't a multiple of 64,
    // we will have some "extra" invocations. This is fine, but we should tell them to stop
    // to avoid out-of-bounds accesses.
    let array_length = arrayLength(&input);
    if (global_id.x >= array_length) {
        return;
    }

    // Do the multiply by two and write to the output.
    output[global_id.x] = input[global_id.x] * 2.0;
}
