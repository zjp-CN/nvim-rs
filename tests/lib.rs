use nvim_rs::{create, DefaultHandler};

use tokio;

use std::{
  thread::sleep,
  time::{Duration, Instant},
};

use std::process::{Command, Stdio};
use std::fs;

use std::path::Path;
use tempdir::TempDir;

const NVIMPATH: &str = "neovim/build/bin/nvim";
const HOST: &str = "127.0.0.1";
const PORT: u16 = 6666;

#[test]
fn can_connect_via_tcp() {
  let mut rt = tokio::runtime::Runtime::new().unwrap();

  let listen = HOST.to_string() + ":" + &PORT.to_string();
  let stderrfile = fs::File::create("stderr.txt").unwrap();
  let stdoutfile = fs::File::create("stdout.txt").unwrap();

  let mut child = Command::new(NVIMPATH)
    .args(&["-u", "NONE", "--headless", "--listen", &listen])
    .stderr(Stdio::from(stderrfile))
    .stdout(Stdio::from(stdoutfile))
    .env("NVIM_LOG_FILE", "nvimlog_tcp")
    .spawn()
    .expect("Cannot start neovim");

  // wait at most 1 second for neovim to start and create the tcp socket
  let start = Instant::now();

  let (nvim, _io_handle) = loop {
    sleep(Duration::from_millis(100));

    let handler = DefaultHandler::new();
    if let Ok(r) = rt.block_on(create::new_tcp(&listen, handler)) {
      break r;
    } else {
      if Duration::from_secs(10) <= start.elapsed() {
        child.kill().unwrap();
        let log = fs::read_to_string("nvimlog_tcp")
                  .expect("Something went wrong reading the file");
        eprintln!("LOG: \n {}", log);
        let errors = fs::read_to_string("stderr.txt")
                  .expect("Something went wrong reading the errfile");
        eprintln!("ERRORS: \n {}", errors);
        let out = fs::read_to_string("stdout.txt")
                  .expect("Something went wrong reading the outfile");
        eprintln!("STDOUT: \n {}", out);
        panic!("Unable to connect to neovim via tcp at {}", listen);
      }
    }
  };

  let servername = rt
    .block_on(nvim.get_vvar("servername"))
    .expect("Error retrieving servername from neovim");

  assert_eq!(&listen, servername.as_str().unwrap());
}

#[tokio::test]
async fn can_connect_via_unix_socket() {
  let dir = TempDir::new("neovim-lib.test")
    .expect("Cannot create temporary directory for test.");

  let socket_path = dir.path().join("unix_socket");

  let _child = Command::new(NVIMPATH)
    .args(&["-u", "NONE", "--headless"])
    .env("NVIM_LISTEN_ADDRESS", &socket_path)
    .spawn()
    .expect("Cannot start neovim");

  // wait at most 1 second for neovim to start and create the socket
  {
    let start = Instant::now();
    let one_second = Duration::from_secs(1);
    loop {
      sleep(Duration::from_millis(100));

      if let Ok(_) = std::fs::metadata(&socket_path) {
        break;
      }

      if one_second <= start.elapsed() {
        panic!(format!("neovim socket not found at '{:?}'", &socket_path));
      }
    }
  }

  let handler = DefaultHandler::new();
  let (nvim, _io_handle) = create::new_unix_socket(&socket_path, handler)
    .await
    .expect(&format!(
      "Unable to connect to neovim's unix socket at {:?}",
      &socket_path
    ));

  let servername = nvim
    .get_vvar("servername")
    .await
    .expect("Error retrieving servername from neovim");

  let servername = servername.as_str().unwrap();

  assert_eq!(socket_path, Path::new(servername));
}
