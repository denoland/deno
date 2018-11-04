// Copyright 2018 the Deno authors. All rights reserved. MIT license.
use libdeno::deno_buf;
extern "C" {
  pub static deno_snapshot: deno_buf;
}
