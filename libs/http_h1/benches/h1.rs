// Copyright 2018-2026 the Deno authors. MIT license.

use criterion::Criterion;
use criterion::black_box;
use criterion::criterion_group;
use criterion::criterion_main;
use deno_http_h1::Header;
use deno_http_h1::MAX_HEADERS;
use deno_http_h1::ResponseHeader;
use deno_http_h1::Version;
use deno_http_h1::append_chunk;
use deno_http_h1::append_chunked_end;
use deno_http_h1::parse_request_head;
use deno_http_h1::write_response_head;

fn bench_parse(c: &mut Criterion) {
  c.bench_function("parse_static_get_head", |b| {
    b.iter(|| {
      let mut headers = [Header::EMPTY; MAX_HEADERS];
      let request = parse_request_head(
        black_box(
          b"GET / HTTP/1.1\r\nHost: localhost:8000\r\nUser-Agent: oha/1.0\r\nAccept: */*\r\n\r\n",
        ),
        black_box(&mut headers),
      )
      .unwrap()
      .unwrap();
      black_box(request.consumed);
    });
  });

  c.bench_function("parse_post_content_length_head", |b| {
    b.iter(|| {
      let mut headers = [Header::EMPTY; MAX_HEADERS];
      let request = parse_request_head(
        black_box(
          b"POST /echo HTTP/1.1\r\nHost: localhost:8000\r\nContent-Length: 13\r\nContent-Type: text/plain\r\n\r\nHello, World!",
        ),
        black_box(&mut headers),
      )
      .unwrap()
      .unwrap();
      black_box(request.body_kind);
    });
  });
}

fn bench_write(c: &mut Criterion) {
  let headers = [Header {
    name: b"content-type",
    value: b"text/plain;charset=UTF-8",
  }];
  c.bench_function("write_static_hello_response_head", |b| {
    let mut out = Vec::with_capacity(256);
    b.iter(|| {
      write_response_head(
        black_box(&mut out),
        ResponseHeader {
          version: Version::Http11,
          status: 200,
          reason: b"OK",
          headers: black_box(&headers),
          content_length: Some(13),
          keep_alive: true,
        },
      );
      black_box(out.len());
    });
  });

  c.bench_function("write_chunked_body_parts", |b| {
    let trailers = [Header {
      name: b"x-sig",
      value: b"abc",
    }];
    let mut out = Vec::with_capacity(256);
    b.iter(|| {
      out.clear();
      append_chunk(black_box(&mut out), black_box(b"Hello, World!"));
      append_chunked_end(black_box(&mut out), black_box(&trailers));
      black_box(out.len());
    });
  });
}

criterion_group!(benches, bench_parse, bench_write);
criterion_main!(benches);
