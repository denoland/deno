extension! {
  deno_io,
  deps = [ deno_web ],
  ops = [op_read_sync, op_write_sync],
  esm = [ "12_io.js" ],
  options = {
    stdio: Option<Stdio>,
  },
  middleware = |op| match op.name {
    "op_print" => op_print::decl(),
    _ => op,
  },
  state = |state, options| {},
}
