use crate::{http_util::{fetch_once, FetchOnceResult}, version, ErrBox};
use crate::futures::FutureExt;
use regex::Regex;
use reqwest::{redirect::Policy, Client};
use std::future::{Future};
use std::env::current_exe;
use std::pin::Pin;
use url::Url;

lazy_static! {
  static ref LATEST_VERSION_URL: String =
    "https://github.com/denoland/deno/releases/latest".to_string();
  static ref EXEC_DOWNLOAD_URL: String =
    "https://github.com/denoland/deno/releases/download/v".to_string();
  static ref REGEX_STRING: String = r#"v([^\?]+)?""#.to_string();
}


// TODO: docs
pub async fn exec_upgrade() -> Result<(), ErrBox> {
  println!("{:?}", current_exe()); // TODO: use current_exe to set path to copy over exec
  let client = Client::builder().redirect(Policy::none()).build()?;
  println!("Checking for latest version.");
  let body = client
    .get(Url::parse(&LATEST_VERSION_URL)?)
    .send()
    .await?
    .text()
    .await?;
  let checked_version = find_version(&body)?;
  if is_latest_version_greater(&version::DENO.to_string(), &checked_version) {
    println!("Deno found greater version.\nDeno is upgrading to version {}", &checked_version);
    let exec = download_exec(&compose_url_to_exec(&checked_version)?, client).await?;
    println!("{:?}", &exec.len());
  } else {
    println!("Local deno version {} is the greatest one", &version::DENO);
  }
  Ok(())
}

fn download_exec(
  url: &Url,
  client: Client,
) -> Pin<Box<dyn Future<Output = Result<Vec<u8>, ErrBox>>>> {
  let url = url.clone();
  let fut = async move {
    match fetch_once(client.clone(), &url, None).await? {
      FetchOnceResult::Code(source, _) => Ok(source),
      FetchOnceResult::NotModified => Ok(Vec::new()),
      FetchOnceResult::Redirect(_url, _) => download_exec(&_url, client).await,
    }
  };
  fut.boxed_local()
}

fn compose_url_to_exec(version: &String) -> Result<Url, ErrBox> {
  let mut url_str = EXEC_DOWNLOAD_URL.clone();
  url_str.push_str(&format!("{}/", version));
  if cfg!(target_os = "windows") {
    url_str.push_str("deno_win_x64.zip");
  } else if cfg!(target_os = "macos") {
    url_str.push_str("deno_osx_x64.gz");
  } else {
    url_str.push_str("deno_linux_x64.gz");
  }
  let url = Url::parse(&url_str[..])?;
  Ok(url)
}

fn find_version(text: &String) -> Result<String, ErrBox> {
  let re = Regex::new(&REGEX_STRING)?;
  if let Some(_mat) = re.find(text) {
    let mat = _mat.as_str();
    return Ok(mat[1..mat.len() - 1].to_string());
  }
  let e = std::io::Error::new(
    std::io::ErrorKind::Other,
    "Cannot read latest tag version".to_string(),
  );
  Err(ErrBox::from(e))
}

fn is_latest_version_greater(old_v: &String, new_v: &String) -> bool {
  let mut power = 4;
  let (mut old_v_num, mut new_v_num) = (0, 0);
  old_v
    .split(".")
    .into_iter()
    .zip(new_v.split(".").into_iter())
    .for_each(|(old, new)| {
      old_v_num += old.parse::<i32>().unwrap() * (10_f32.powi(power) as i32);
      new_v_num += new.parse::<i32>().unwrap() * (10_f32.powi(power) as i32);
      power -= 2;
    });
  old_v_num < new_v_num
}
