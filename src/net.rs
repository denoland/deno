use errors::DenoResult;
use hyper;
use hyper::rt::{Future, Stream};
use hyper::{Client, Uri};
use hyper_rustls;
use tokio::runtime::current_thread::Runtime;

// The CodeFetch message is used to load HTTP javascript resources and expects a
// synchronous response, this utility method supports that.
pub fn fetch_sync_string(url: &Uri) -> DenoResult<String> {
  let https = hyper_rustls::HttpsConnector::new(4);
  let client: Client<_, hyper::Body> = Client::builder().build(https);

  // TODO Use Deno's RT
  let mut rt = Runtime::new().unwrap();

  let body = rt.block_on(
    client
      .get(url.clone())
      .and_then(|response| response.into_body().concat2()),
  )?;
  Ok(String::from_utf8(body.to_vec()).unwrap())
}

#[test]
fn test_fetch_sync_string() {
  // Relies on external http server. See tools/http_server.pyfetch_sync_string
  let p = fetch_sync_string(&"http://localhost:4545/package.json".parse::<hyper::Uri>().unwrap()).unwrap();
  println!("package.json len {}", p.len());
  assert!(p.len() > 1);
}
