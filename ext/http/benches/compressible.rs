// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
use bencher::{benchmark_group, benchmark_main, Bencher};
use deno_http::compressible::is_content_compressible;

fn compressible_simple_hit(b: &mut Bencher) {
  b.iter(|| {
    is_content_compressible("text/plain");
  })
}

fn compressible_complex_hit(b: &mut Bencher) {
  b.iter(|| {
    is_content_compressible("text/PlAIn; charset=utf-8");
  })
}

fn compressible_simple_miss(b: &mut Bencher) {
  b.iter(|| {
    is_content_compressible("text/fake");
  })
}

fn compressible_complex_miss(b: &mut Bencher) {
  b.iter(|| {
    is_content_compressible("text/fake;charset=utf-8");
  })
}

benchmark_group!(
  benches,
  compressible_simple_hit,
  compressible_complex_hit,
  compressible_simple_miss,
  compressible_complex_miss,
);

benchmark_main!(benches);
