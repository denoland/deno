extension! {
  my_extension,
  deps = [ deno_core, deno_web ],
  parameters = [ P: Permissions ],
  ops = [ op_foo, op_bar::<P> ]
  esm = [ "my_script.js" ],
}
