//! How to handle cleanup logic with access to the handler's data. See
//! src/examples/handler_drop.rs for documentation.
#![allow(unused)]
use nvim_rs::{
  compat::tokio::Compat, create::tokio as create, Handler, Neovim, Value,
};

use tokio::process::{ChildStdin, Command};

use async_trait::async_trait;

use std::{
  fs::File,
  io::Write,
  ops::Drop,
  sync::{Arc, Mutex},
};

const OUTPUT_FILE: &str = "handler_drop.txt";
const NVIMPATH: &str = "/usr/bin/nvim";

#[derive(Clone)]
struct NeovimHandler {
  buf: Arc<Mutex<Vec<String>>>,
}

#[async_trait]
impl Handler for NeovimHandler {
  type Writer = Compat<ChildStdin>;

  async fn handle_notify(
    &self,
    name: String,
    args: Vec<Value>,
    _req: Neovim<Compat<ChildStdin>>,
  ) {
    match dbg!(name.as_ref()) {
      "nvim_buf_lines_event" => {
        // This can be made more efficient by taking ownership appropriately,
        // but we skip this in this example
        for s in args[4]
          .as_array()
          .unwrap()
          .iter()
          .map(|s| s.as_str().unwrap())
        {
          self.buf.lock().unwrap().push(s.to_owned());
        }
      }
      _ => {}
    }
  }
}

impl Drop for NeovimHandler {
  fn drop(&mut self) {
    let mut file = File::create(OUTPUT_FILE).unwrap();

    for line in dbg!(&self.buf.lock().unwrap()).iter() {
      writeln!(file, "{}", line).unwrap();
    }
  }
}

#[tokio::main]
async fn main() {
  let handler = NeovimHandler {
    buf: Arc::new(Mutex::new(vec![])),
  };

  let (nvim, io_handle, child) = create::new_child_cmd(
    Command::new(NVIMPATH)
      .args(&["-u", "NONE", "--embed", "--headless"])
      .env("NVIM_LOG_FILE", "nvimlog"),
    handler,
  )
  .await
  .unwrap();

  let chan = nvim.get_api_info().await.unwrap()[0].as_i64().unwrap();

  let curbuf = nvim.get_current_buf().await.unwrap();
  if !curbuf.attach(false, vec![]).await.unwrap() {
    return;
  }
  curbuf
    .set_lines(0, 0, false, vec!["xyz".into(), "abc".into()])
    .await
    .unwrap();

  // Add some command calls
  // let _ = dbg!(nvim.command(&format!("echo \"hi {}\"", chan)).await);
  // let _ = dbg!(nvim.command("w! hi.txt").await);

  // Original
  let close = format!("call chanclose({})", chan);
  let _ = dbg!(nvim.command(&close).await);
  let _ = dbg!(io_handle.await);
}
