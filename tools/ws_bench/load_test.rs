// Copyright 2018-2026 the Deno authors. MIT license.
//
// Standalone WebSocket load generator for the ext/websocket fast-path
// benchmark. Same load shape as fastwebsockets PR #133's bench
// (`bench/load_test.c`) so before/after numbers compare against the
// fastwebsockets baseline directly.
//
// Each of `N` concurrent connections runs a tight echo loop: send one
// binary frame, read the echoed reply, repeat. After a 1s warmup we
// count completed round trips over `WINDOW` seconds.
//
// Build standalone with:
//   rustc -O tools/ws_bench/load_test.rs -o /tmp/ws_load_test \
//     --extern tokio=... (cumbersome — easier path below)
//
// Or, with the workspace toolchain already present, drop the file
// alongside a Cargo.toml that has `tokio` / `bytes` / `httparse` deps
// and `cargo run --release`. The repo's `tools/ws_bench/` does
// not have its own Cargo.toml because the benchmark is intentionally
// one-shot ad-hoc tooling — see the PR body for the exact invocation
// the maintainer used.
//
// To keep the file self-contained for inclusion in the PR as-is and
// re-runnable without bringing in dependencies, the protocol code
// (handshake + frame send / recv) is hand-rolled against the raw
// `TcpStream`. Only RFC 6455 binary frames; no fragmentation.

use std::env;
use std::io::IoSlice;
use std::sync::Arc;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;
use std::time::Duration;
use std::time::Instant;

use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;

fn build_upgrade_request(host: &str, key: &str) -> Vec<u8> {
  format!(
    "GET / HTTP/1.1\r\n\
     Host: {host}\r\n\
     Upgrade: websocket\r\n\
     Connection: Upgrade\r\n\
     Sec-WebSocket-Key: {key}\r\n\
     Sec-WebSocket-Version: 13\r\n\
     \r\n"
  )
  .into_bytes()
}

/// Read until two CRLFs — the end of the upgrade response headers.
async fn read_headers(stream: &mut TcpStream) -> std::io::Result<Vec<u8>> {
  let mut acc = Vec::with_capacity(512);
  let mut buf = [0u8; 256];
  loop {
    let n = stream.read(&mut buf).await?;
    if n == 0 {
      return Err(std::io::ErrorKind::UnexpectedEof.into());
    }
    acc.extend_from_slice(&buf[..n]);
    if acc.windows(4).any(|w| w == b"\r\n\r\n") {
      return Ok(acc);
    }
    if acc.len() > 8192 {
      return Err(std::io::ErrorKind::InvalidData.into());
    }
  }
}

/// Build a masked client-to-server binary frame for `payload`.
fn build_masked_frame(payload: &[u8]) -> Vec<u8> {
  let mut out = Vec::with_capacity(payload.len() + 14);
  out.push(0x82); // FIN | Binary
  let mask = [0x01u8, 0x02, 0x03, 0x04];
  if payload.len() < 126 {
    out.push(0x80 | payload.len() as u8);
  } else if payload.len() < 65536 {
    out.push(0x80 | 126);
    out.extend_from_slice(&(payload.len() as u16).to_be_bytes());
  } else {
    out.push(0x80 | 127);
    out.extend_from_slice(&(payload.len() as u64).to_be_bytes());
  }
  out.extend_from_slice(&mask);
  for (i, &b) in payload.iter().enumerate() {
    out.push(b ^ mask[i & 3]);
  }
  out
}

/// Drain one whole server-to-client (unmasked) frame from `buf`,
/// reading more bytes if needed.
async fn read_one_frame(
  stream: &mut TcpStream,
  buf: &mut Vec<u8>,
) -> std::io::Result<usize> {
  loop {
    if buf.len() >= 2 {
      let b1 = buf[1];
      let mut header_len = 2usize;
      let payload_len: usize = match b1 & 0x7f {
        126 => {
          if buf.len() < 4 {
            // need more
            grow_read(stream, buf).await?;
            continue;
          }
          header_len = 4;
          u16::from_be_bytes([buf[2], buf[3]]) as usize
        }
        127 => {
          if buf.len() < 10 {
            grow_read(stream, buf).await?;
            continue;
          }
          header_len = 10;
          u64::from_be_bytes([
            buf[2], buf[3], buf[4], buf[5], buf[6], buf[7], buf[8], buf[9],
          ]) as usize
        }
        other => other as usize,
      };
      let total = header_len + payload_len;
      if buf.len() < total {
        grow_read(stream, buf).await?;
        continue;
      }
      // Consume this one frame.
      buf.drain(..total);
      return Ok(payload_len);
    }
    grow_read(stream, buf).await?;
  }
}

async fn grow_read(
  stream: &mut TcpStream,
  buf: &mut Vec<u8>,
) -> std::io::Result<()> {
  let mut chunk = [0u8; 16384];
  let n = stream.read(&mut chunk).await?;
  if n == 0 {
    return Err(std::io::ErrorKind::UnexpectedEof.into());
  }
  buf.extend_from_slice(&chunk[..n]);
  Ok(())
}

async fn run_client(
  addr: String,
  payload_size: usize,
  counter: Arc<AtomicU64>,
  stop: Arc<AtomicU64>,
) -> std::io::Result<()> {
  let mut stream = TcpStream::connect(&addr).await?;
  let _ = stream.set_nodelay(true);
  // Static key — server doesn't validate against any specific one, just
  // returns the matching Accept. fastwebsockets server is generous about
  // this.
  let req = build_upgrade_request(&addr, "dGhlIHNhbXBsZSBub25jZQ==");
  stream.write_all(&req).await?;
  let _hdrs = read_headers(&mut stream).await?;
  let payload = vec![0xABu8; payload_size];
  let masked = build_masked_frame(&payload);
  let mut rxbuf = Vec::with_capacity(payload_size + 16);
  let mut local = 0u64;
  loop {
    if stop.load(Ordering::Relaxed) != 0 {
      break;
    }
    stream.write_all(&masked).await?;
    let _ = read_one_frame(&mut stream, &mut rxbuf).await?;
    local += 1;
    if local % 64 == 0 {
      counter.fetch_add(64, Ordering::Relaxed);
      local = 0;
    }
  }
  counter.fetch_add(local, Ordering::Relaxed);
  Ok(())
}

#[tokio::main(flavor = "multi_thread", worker_threads = 4)]
async fn main() -> std::io::Result<()> {
  let mut args = env::args().skip(1);
  let conns: usize = args
    .next()
    .expect("usage: load_test <conns> <payload> [window_secs] [addr]")
    .parse()
    .unwrap();
  let payload: usize = args.next().unwrap().parse().unwrap();
  let window: u64 = args
    .next()
    .unwrap_or_else(|| "5".to_string())
    .parse()
    .unwrap();
  let addr = args.next().unwrap_or_else(|| "127.0.0.1:8080".to_string());
  let counter = Arc::new(AtomicU64::new(0));
  let stop = Arc::new(AtomicU64::new(0));
  let mut handles = Vec::with_capacity(conns);
  for _ in 0..conns {
    let c = counter.clone();
    let s = stop.clone();
    let a = addr.clone();
    handles.push(tokio::spawn(async move {
      let _ = run_client(a, payload, c, s).await;
    }));
  }
  // Warm up for 1s, then measure.
  tokio::time::sleep(Duration::from_secs(1)).await;
  counter.store(0, Ordering::Relaxed);
  let t0 = Instant::now();
  tokio::time::sleep(Duration::from_secs(window)).await;
  let total = counter.load(Ordering::Relaxed);
  let dt = t0.elapsed().as_secs_f64();
  stop.store(1, Ordering::Relaxed);
  let mps = total as f64 / dt;
  println!(
    "conns={conns} payload={payload} window={dt:.2}s msgs={total} mps={mps:.0}"
  );
  // Ignore client task results; they exit on stop or on TCP error.
  drop(handles);
  // IoSlice import keeps the bench binary self-contained even when
  // future versions use vectored sends; silence dead-code on stable.
  let _: Option<IoSlice<'_>> = None;
  Ok(())
}
