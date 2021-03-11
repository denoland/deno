[[builtin(vertex_index)]]
var<in> in_vertex_index: u32;
[[builtin(position)]]
var<out> out_pos: vec4<f32>;

[[stage(vertex)]]
fn vs_main() {
    var x: f32 = f32(i32(in_vertex_index) - 1);
    var y: f32 = f32(i32(in_vertex_index & 1) * 2 - 1);
    out_pos = vec4<f32>(x, y, 0.0, 1.0);
}

[[location(0)]]
var<out> out_color: vec4<f32>;

[[stage(fragment)]]
fn fs_main() {
    out_color = vec4<f32>(1.0, 0.0, 0.0, 1.0);
}
