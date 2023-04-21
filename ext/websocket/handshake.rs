// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// Copyright 2023 Divy Srivastava <dj.srivastava23@gmail.com>
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use hyper::upgrade::Upgraded;
use hyper::Body;
use hyper::Request;
use hyper::Response;
use hyper::StatusCode;

use tokio::io::AsyncRead;
use tokio::io::AsyncWrite;
use tokio::task::spawn_local;

use std::error::Error;

pub async fn client<S>(
  request: Request<Body>,
  socket: S,
) -> Result<(Upgraded, Response<Body>), Box<dyn Error + Send + Sync>>
where
  S: AsyncRead + AsyncWrite + Send + Unpin + 'static,
{
  let (mut sender, conn) =
    hyper::client::conn::http1::handshake(socket).await?;
  spawn_local(async move {
    if let Err(e) = conn.await {
      eprintln!("Error polling connection: {}", e);
    }
  });

  let mut response = sender.send_request(request).await?;
  verify(&response)?;

  let upgraded = hyper::upgrade::on(&mut response).await?;
  Ok((upgraded, response))
}

// https://github.com/snapview/tungstenite-rs/blob/314feea3055a93e585882fb769854a912a7e6dae/src/handshake/client.rs#L189
fn verify(
  response: &Response<Body>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
  if response.status() != StatusCode::SWITCHING_PROTOCOLS {
    return Err("Invalid status code".into());
  }

  let headers = response.headers();

  if !headers
    .get("Upgrade")
    .and_then(|h| h.to_str().ok())
    .map(|h| h.eq_ignore_ascii_case("websocket"))
    .unwrap_or(false)
  {
    return Err("Invalid Upgrade header".into());
  }

  if !headers
    .get("Connection")
    .and_then(|h| h.to_str().ok())
    .map(|h| h.eq_ignore_ascii_case("Upgrade"))
    .unwrap_or(false)
  {
    return Err("Invalid Connection header".into());
  }

  Ok(())
}
