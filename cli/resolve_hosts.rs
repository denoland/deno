/// Expands "bare port" paths (eg. ":8080") into full paths with hosts. It
/// expands to such paths into 3 paths with following hosts: `0.0.0.0:port`,
/// `127.0.0.1:port` and `localhost:port`.
pub fn resolve_hosts(paths: Vec<String>) -> Vec<String> {
  let mut out: Vec<String> = vec![];
  for host_and_port in paths.iter() {
    let parts = host_and_port.split(':').collect::<Vec<&str>>();

    match parts.len() {
      // host only
      1 => {
        out.push(host_and_port.to_owned());
      }
      // host and port (NOTE: host might be empty string)
      2 => {
        let host = parts[0];
        let port = parts[1];

        if !host.is_empty() {
          out.push(host_and_port.to_owned());
          continue;
        }

        // we got bare port, let's add default hosts
        for host in ["0.0.0.0", "127.0.0.1", "localhost"].iter() {
          out.push(format!("{}:{}", host, port));
        }
      }
      _ => panic!("Bad host:port pair: {}", host_and_port),
    }
  }

  out
}
