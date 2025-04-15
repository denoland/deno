// only test purpose
override value: f32;

@vertex
fn vs_main(@builtin(vertex_index) in_vertex_index: u32) -> @builtin(position) vec4<f32> {
    // only test purpose
    _ = value;

    let x = f32(i32(in_vertex_index) - 1);
    let y = f32(i32(in_vertex_index & 1u) * 2 - 1);
    return vec4<f32>(x, y, 0.0, 1.0);
}

@fragment
fn fs_main() -> @location(0) vec4<f32> {
    // only test purpose
    _ = value;

    return vec4<f32>(1.0, 0.0, 0.0, 1.0);
}
