// Copyright 2018-2026 the Deno authors. MIT license.
//
// Compares snapshotting ES modules vs plain scripts.
// 126 extensions x 2 files = 252 files per variant, ~25+14 KiB each.
//
// Run: cargo bench -p deno_core --bench esm_vs_script

use std::time::Duration;
use std::time::Instant;

use criterion::*;
use deno_core::Extension;
use deno_core::JsRuntime;
use deno_core::JsRuntimeForSnapshot;
use deno_core::RuntimeOptions;

macro_rules! fake_esm_extensions {
  ($which:ident, $($name:ident),+) => (
    {
      $(
        mod $name {
          deno_core::extension!(
            $name,
            ops = [ ops::$name ],
            esm_entry_point = concat!("ext:", stringify!($name), "/esm_file.js"),
            esm = [ dir "benches/snapshot", "esm_file.js", "esm_file2.js" ]
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

macro_rules! fake_script_extensions {
  ($which:ident, $($name:ident),+) => (
    {
      $(
        mod $name {
          deno_core::extension!(
            $name,
            ops = [ ops::$name ],
            js = [ dir "benches/snapshot", "script_file.js", "script_file2.js" ]
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

// 126 extensions x 2 files = 252 ESM files
fn make_esm_extensions() -> Vec<Extension> {
  fake_esm_extensions!(
    ea0, eb0, ec0, ed0, ee0, ef0, eg0, eh0, ei0, ej0, ek0, el0, em0, en0, eo0,
    ep0, eq0, er0, es0, et0, eu0, ev0, ew0, ex0, ey0, ez0, ea1, eb1, ec1, ed1,
    ee1, ef1, eg1, eh1, ei1, ej1, ek1, el1, em1, en1, eo1, ep1, eq1, er1, es1,
    et1, eu1, ev1, ew1, ex1, ey1, ez1, ea2, eb2, ec2, ed2, ee2, ef2, eg2, eh2,
    ei2, ej2, ek2, el2, em2, en2, eo2, ep2, eq2, er2, es2, et2, eu2, ev2, ew2,
    ex2, ey2, ez2, ea3, eb3, ec3, ed3, ee3, ef3, eg3, eh3, ei3, ej3, ek3, el3,
    em3, en3, eo3, ep3, eq3, er3, es3, et3, eu3, ev3, ew3, ex3, ey3, ez3, ea4,
    eb4, ec4, ed4, ee4, ef4, eg4, eh4, ei4, ej4, ek4, el4, em4, en4, eo4, ep4,
    eq4, er4, es4, et4, eu4, ev4, ew4, ex4, ey4
  )
}

// 126 extensions x 2 files = 252 script files
fn make_script_extensions() -> Vec<Extension> {
  fake_script_extensions!(
    sa0, sb0, sc0, sd0, se0, sf0, sg0, sh0, si0, sj0, sk0, sl0, sm0, sn0, so0,
    sp0, sq0, sr0, ss0, st0, su0, sv0, sw0, sx0, sy0, sz0, sa1, sb1, sc1, sd1,
    se1, sf1, sg1, sh1, si1, sj1, sk1, sl1, sm1, sn1, so1, sp1, sq1, sr1, ss1,
    st1, su1, sv1, sw1, sx1, sy1, sz1, sa2, sb2, sc2, sd2, se2, sf2, sg2, sh2,
    si2, sj2, sk2, sl2, sm2, sn2, so2, sp2, sq2, sr2, ss2, st2, su2, sv2, sw2,
    sx2, sy2, sz2, sa3, sb3, sc3, sd3, se3, sf3, sg3, sh3, si3, sj3, sk3, sl3,
    sm3, sn3, so3, sp3, sq3, sr3, ss3, st3, su3, sv3, sw3, sx3, sy3, sz3, sa4,
    sb4, sc4, sd4, se4, sf4, sg4, sh4, si4, sj4, sk4, sl4, sm4, sn4, so4, sp4,
    sq4, sr4, ss4, st4, su4, sv4, sw4, sx4, sy4
  )
}

fn bench_take_snapshot(c: &mut Criterion) {
  let mut group = c.benchmark_group("esm vs script: take snapshot");
  group.sample_size(20);

  group.bench_function("esm", |b| {
    b.iter_custom(|iters| {
      let mut total = 0;
      for _ in 0..iters {
        let extensions = make_esm_extensions();
        let runtime = JsRuntimeForSnapshot::new(RuntimeOptions {
          startup_snapshot: None,
          extensions,
          ..Default::default()
        });
        let now = Instant::now();
        runtime.snapshot();
        total += now.elapsed().as_nanos();
      }
      Duration::from_nanos(total as _)
    });
  });

  group.bench_function("script", |b| {
    b.iter_custom(|iters| {
      let mut total = 0;
      for _ in 0..iters {
        let extensions = make_script_extensions();
        let runtime = JsRuntimeForSnapshot::new(RuntimeOptions {
          startup_snapshot: None,
          extensions,
          ..Default::default()
        });
        let now = Instant::now();
        runtime.snapshot();
        total += now.elapsed().as_nanos();
      }
      Duration::from_nanos(total as _)
    });
  });

  group.finish();
}

fn bench_load_snapshot(c: &mut Criterion) {
  // Create both snapshots and report sizes, but benchmark sequentially
  // to avoid leaking both simultaneously (V8 memory pressure).
  let esm_snapshot_box = {
    let extensions = make_esm_extensions();
    let runtime = JsRuntimeForSnapshot::new(RuntimeOptions {
      startup_snapshot: None,
      extensions,
      ..Default::default()
    });
    runtime.snapshot()
  };

  let script_snapshot_box = {
    let extensions = make_script_extensions();
    let runtime = JsRuntimeForSnapshot::new(RuntimeOptions {
      startup_snapshot: None,
      extensions,
      ..Default::default()
    });
    runtime.snapshot()
  };

  eprintln!();
  eprintln!(
    "=== Snapshot sizes (126 ext x 2 files = 252 files, ~25+14 KiB each) ==="
  );
  eprintln!(
    "  ESM:    {} bytes ({:.2} MiB)",
    esm_snapshot_box.len(),
    esm_snapshot_box.len() as f64 / 1024.0 / 1024.0
  );
  eprintln!(
    "  Script: {} bytes ({:.2} MiB)",
    script_snapshot_box.len(),
    script_snapshot_box.len() as f64 / 1024.0 / 1024.0
  );
  eprintln!(
    "  Delta:  {} bytes ({:+.1}%)",
    esm_snapshot_box.len() as isize - script_snapshot_box.len() as isize,
    (esm_snapshot_box.len() as f64 / script_snapshot_box.len() as f64 - 1.0)
      * 100.0
  );
  eprintln!(
    "========================================================================="
  );
  eprintln!();

  // Benchmark ESM load first, then free it before benchmarking script load.
  let esm_snapshot: &'static [u8] = Box::leak(esm_snapshot_box);

  {
    let mut group = c.benchmark_group("esm vs script: load snapshot");
    group.sample_size(10);
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(5));

    group.bench_function("esm", |b| {
      b.iter_custom(|iters| {
        let mut total = 0;
        for _ in 0..iters {
          let now = Instant::now();
          let runtime = JsRuntime::new(RuntimeOptions {
            startup_snapshot: Some(esm_snapshot),
            extensions: make_esm_extensions(),
            ..Default::default()
          });
          total += now.elapsed().as_nanos();
          drop(runtime);
        }
        Duration::from_nanos(total as _)
      });
    });

    group.finish();
  }

  // SAFETY: group.finish() consumed the group and dropped all closures that
  // captured esm_snapshot. No references remain.
  unsafe {
    drop(Box::from_raw(esm_snapshot as *const [u8] as *mut [u8]));
  }

  let script_snapshot: &'static [u8] = Box::leak(script_snapshot_box);

  {
    let mut group = c.benchmark_group("esm vs script: load snapshot");
    group.sample_size(10);
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(5));

    group.bench_function("script", |b| {
      b.iter_custom(|iters| {
        let mut total = 0;
        for _ in 0..iters {
          let now = Instant::now();
          let runtime = JsRuntime::new(RuntimeOptions {
            startup_snapshot: Some(script_snapshot),
            extensions: make_script_extensions(),
            ..Default::default()
          });
          total += now.elapsed().as_nanos();
          drop(runtime);
        }
        Duration::from_nanos(total as _)
      });
    });

    group.finish();
  }
}

criterion_group!(
  name = benches;
  config = Criterion::default();
  targets =
    bench_take_snapshot,
    bench_load_snapshot,
);

criterion_main!(benches);
