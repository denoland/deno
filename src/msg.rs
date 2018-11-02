#![allow(unused_imports)]
#![allow(dead_code)]
use flatbuffers;
// TODO Replace DENO_BUILD_PATH with OUT_DIR. gn/ninja should generate into
// the same output directory as cargo uses.
include!(concat!(env!("DENO_BUILD_PATH"), "/gen/msg_generated.rs"));
