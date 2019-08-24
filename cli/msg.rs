// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
#![allow(dead_code)]
#![cfg_attr(feature = "cargo-clippy", allow(clippy::all, clippy::pedantic))]
use flatbuffers;

// GN_OUT_DIR is set either by build.rs (for the Cargo build), or by
// build_extra/rust/run.py (for the GN+Ninja build).
include!(concat!(env!("GN_OUT_DIR"), "/gen/cli/msg_generated.rs"));
