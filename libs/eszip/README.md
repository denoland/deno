# eszip

The eszip format lets you losslessly serialize an ECMAScript module graph
(represented by [`deno_graph::ModuleGraph`][module_graph]) into a single compact
file.

The eszip file format is designed to be compact and streaming capable. This
allows for efficient loading of large ECMAScript module graphs.

https://eszip-viewer.deno.dev/ is a tool for inspecting eszip files.

[module_graph]: https://docs.rs/deno_graph/latest/deno_graph/struct.ModuleGraph.html

## Examples

### Creating an eszip

```shell
cargo run --example eszip_builder https://deno.land/std/http/file_server.ts file_server.eszip2
```

### Viewing the contents of an eszip

```shell
cargo run --example eszip_viewer file_server.eszip2
```

### Loading the eszip into V8

```shell
cargo run --example eszip_load file_server.eszip2 https://deno.land/std/http/file_server.ts
```

## File format

The file format looks as follows:

```
Eszip:
| Magic (8) | Header size (4) | Header (n) | Header hash (32) | Sources size (4) | Sources (n) | SourceMaps size (4) | SourceMaps (n) |

Header:
( | Specifier size (4) | Specifier (n) | Entry type (1) | Entry (n) | )*

Entry (redirect):
| Specifier size (4) | Specifier (n) |

Entry (module):
| Source offset (4) | Source size (4) | SourceMap offset (4) | SourceMap size (4) | Module type (1) |

Sources:
( | Source (n) | Hash (32) | )*

SourceMaps:
( | SourceMap (n) | Hash (32) | )*
```

There is one optimization for empty source / source map entries. If both the
offset and size are set to 0, no entry and no hash is present in the data
sections for that module.

## Development

When opening a PR make sure to rebuild Wasm by running:

```
deno task build
```
