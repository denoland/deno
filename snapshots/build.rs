mod build_runtime;
mod build_tsc;

fn main() {
  build_runtime::main();
  build_tsc::main();
}
