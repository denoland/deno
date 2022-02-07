const lib = Deno.core.dlopen(
  "./node_modules/@parcel/transformer-js/parcel-swc.darwin-arm64.node",
);

const { code } = lib.transform({
  filename: "main.js",
  code: Deno.core.encode("const x = 1;"),
  module_id: "1",
  project_root: ".",
  replace_env: false,
  inline_fs: false,
  insert_node_globals: false,
  is_browser: true,
  is_worker: false,
  env: {},
  is_type_script: false,
  is_jsx: false,
  // jsx_pragma: undefined,
  // jsx_pragma_frag: config?.pragmaFrag,
  automatic_jsx_runtime: false,
  // jsx_import_source: config?.jsxImportSource,
  is_development: false,
  react_refresh: false,
  decorators: false,
  targets: null,
  source_maps: false,
  scope_hoist: false,
  source_type: "Module",
  supports_module_workers: false,
  is_library: true,
  is_esm_output: false,
  trace_bailouts: true,
});

console.log(Deno.core.decode(code));
