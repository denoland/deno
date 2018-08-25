use errors::DenoResult;
use hyper::rt::{Future, Stream};
use hyper::{Body, Client, Uri};
use tokio::runtime::current_thread::Runtime;
use hyper_tls::HttpsConnector;

// The CodeFetch message is used to load HTTP javascript resources and expects a
// synchronous response, this utility method supports that.
pub fn fetch_sync_string(module_name: &str) -> DenoResult<String> {
  let url = module_name.parse::<Uri>().unwrap();
  let https = HttpsConnector::new(4).unwrap();
  let client = Client::builder()
    .build::<_, Body>(https);

  // TODO Use Deno's RT
  let mut rt = Runtime::new().unwrap();

  let body = rt.block_on(
    client
      .get(url)
      .and_then(|response| response.into_body().concat2()),
  )?;
  Ok(String::from_utf8(body.to_vec()).unwrap())
}

#[test]
fn test_fetch_sync_string() {
  // Relies on external http server. See tools/http_server.py
  let p = fetch_sync_string("http://localhost:4545/package.json").unwrap();
  println!("package.json len {}", p.len());
  assert!(p.len() > 1);
}
