// Copyright 2018-2025 the Deno authors. MIT license.

use criterion::*;
use deno_ast::MediaType;
use deno_ast::ModuleKind;
use deno_ast::ParseParams;
use deno_ast::SourceMapOption;
use deno_core::Extension;
use deno_core::JsRuntime;
use deno_core::JsRuntimeForSnapshot;
use deno_core::ModuleCodeString;
use deno_core::ModuleName;
use deno_core::RuntimeOptions;
use deno_core::SourceMapData;
use deno_error::JsErrorBox;
use std::rc::Rc;
use std::time::Duration;
use std::time::Instant;
use url::Url;

macro_rules! fake_extensions {
  ($which:ident, $($name:ident),+) => (
    {
      $(
        mod $name {
          deno_core::extension!(
            $name,
            ops = [ ops::$name ],
            esm_entry_point = concat!("ext:", stringify!($name), "/file.js"),
            esm = [ dir "benches/snapshot", "file.js", "file2.js" ]
          );

          mod ops {
            #[deno_core::op2(fast)]
            pub fn $name() {
            }
          }
        }
      )+

      vec![$($name::$name::init()),+]
    }
  );
}

fn make_extensions() -> Vec<Extension> {
  fake_extensions!(
    a0, b0, c0, d0, e0, f0, g0, h0, i0, j0, k0, l0, m0, n0, o0, p0, q0, r0, s0,
    t0, u0, v0, w0, x0, y0, z0, a1, b1, c1, d1, e1, f1, g1, h1, i1, j1, k1, l1,
    m1, n1, o1, p1, q1, r1, s1, t1, u1, v1, w1, x1, y1, z1, a2, b2, c2, d2, e2,
    f2, g2, h2, i2, j2, k2, l2, m2, n2, o2, p2, q2, r2, s2, t2, u2, v2, w2, x2,
    y2, z2
  )
}

pub fn maybe_transpile_source(
  specifier: ModuleName,
  source: ModuleCodeString,
) -> Result<(ModuleCodeString, Option<SourceMapData>), JsErrorBox> {
  let media_type = MediaType::TypeScript;

  let parsed = deno_ast::parse_module(ParseParams {
    specifier: Url::parse(&specifier).unwrap(),
    text: source.as_str().into(),
    media_type,
    capture_tokens: false,
    scope_analysis: false,
    maybe_syntax: None,
  })
  .map_err(JsErrorBox::from_err)?;
  let transpiled_source = parsed
    .transpile(
      &deno_ast::TranspileOptions {
        imports_not_used_as_values: deno_ast::ImportsNotUsedAsValues::Remove,
        ..Default::default()
      },
      &deno_ast::TranspileModuleOptions {
        module_kind: Some(ModuleKind::Esm),
      },
      &deno_ast::EmitOptions {
        source_map: SourceMapOption::Separate,
        inline_sources: true,
        ..Default::default()
      },
    )
    .map_err(JsErrorBox::from_err)?
    .into_source();
  Ok((
    transpiled_source.text.into(),
    transpiled_source.source_map.map(|s| s.into_bytes().into()),
  ))
}

fn bench_take_snapshot_empty(c: &mut Criterion) {
  c.bench_function("take snapshot (empty)", |b| {
    b.iter_custom(|iters| {
      let mut total = 0;
      for _ in 0..iters {
        let runtime = JsRuntimeForSnapshot::new(RuntimeOptions {
          startup_snapshot: None,
          ..Default::default()
        });
        let now = Instant::now();
        runtime.snapshot();
        total += now.elapsed().as_nanos();
      }
      Duration::from_nanos(total as _)
    });
  });
}

fn bench_take_snapshot(c: &mut Criterion) {
  fn inner(b: &mut Bencher, transpile: bool) {
    b.iter_custom(|iters| {
      let mut total = 0;
      for _ in 0..iters {
        let extensions = make_extensions();
        let runtime = JsRuntimeForSnapshot::new(RuntimeOptions {
          startup_snapshot: None,
          extension_transpiler: if transpile {
            Some(Rc::new(|specifier, source| {
              maybe_transpile_source(specifier, source)
            }))
          } else {
            None
          },
          extensions,
          ..Default::default()
        });
        let now = Instant::now();
        runtime.snapshot();
        total += now.elapsed().as_nanos();
      }
      Duration::from_nanos(total as _)
    });
  }

  let mut group = c.benchmark_group("take snapshot");
  group.bench_function("plain", |b| inner(b, false));
  group.bench_function("transpiled", |b| inner(b, true));
  group.finish();
}

fn bench_load_snapshot(c: &mut Criterion) {
  fn inner(b: &mut Bencher, transpile: bool) {
    let extensions = make_extensions();
    let runtime = JsRuntimeForSnapshot::new(RuntimeOptions {
      extensions,
      extension_transpiler: if transpile {
        Some(Rc::new(|specifier, source| {
          maybe_transpile_source(specifier, source)
        }))
      } else {
        None
      },
      startup_snapshot: None,
      ..Default::default()
    });
    let snapshot = runtime.snapshot();
    let snapshot = Box::leak(snapshot);

    b.iter_custom(|iters| {
      let mut total = 0;
      for _ in 0..iters {
        let now = Instant::now();
        let runtime = JsRuntime::new(RuntimeOptions {
          extensions: make_extensions(),
          startup_snapshot: Some(snapshot),
          ..Default::default()
        });
        total += now.elapsed().as_nanos();
        drop(runtime)
      }
      Duration::from_nanos(total as _)
    });
  }

  let mut group = c.benchmark_group("load snapshot");
  group.bench_function("plain", |b| inner(b, false));
  group.bench_function("transpiled", |b| inner(b, true));
  group.finish();
}

criterion_group!(
  name = benches;
  config = Criterion::default().sample_size(50);
  targets =
    bench_take_snapshot_empty,
    bench_take_snapshot,
    bench_load_snapshot,
);

criterion_main!(benches);
