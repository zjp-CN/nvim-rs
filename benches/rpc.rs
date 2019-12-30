use async_trait::async_trait;
use criterion::{criterion_group, criterion_main, Criterion};
use nvim_rs::{
  call_args, create,
  rpc::IntoVal,
  runtime::{ChildStdin, Command},
  Handler,
};

use tokio::runtime::Builder;

const NVIMPATH: &str = "neovim/build/bin/nvim";

struct NH {}

#[async_trait]
impl Handler for NH {
  type Writer = ChildStdin;
}

fn simple_requests(c: &mut Criterion) {

  c.bench_function("simple_requests", move |b| {
    let handler = NH {};

    let mut rt = Builder::new()
      .threaded_scheduler()
      .enable_io()
      .build()
      .unwrap();

    let _ = rt
      .block_on(create::run_child_cmd(
        Command::new(NVIMPATH).args(&["-u", "NONE", "--embed", "--headless"]),
        handler,
        |nvim| {
          async move {
            nvim.command("set noswapfile").await?;
            let res = b.iter(|| {
              let nvim = nvim.clone();
              let _curbuf = rt.block_on(async move {
                nvim.get_current_buf().await.expect("1");
              });
            });
            let _ = nvim.command("qa!").await;
            res
          }
        },
      ));
  });
}

fn request_file(c: &mut Criterion) {

  c.bench_function("simple_requests", move |b| {
    let handler = NH {};

    let mut rt = Builder::new()
      .threaded_scheduler()
      .enable_io()
      .build()
      .unwrap();

    let _ = rt
      .block_on(create::run_child_cmd(
        Command::new(NVIMPATH).args(&["-u", "NONE", "--embed", "--headless",
        "Cargo.lock"]),
        handler,
        |nvim| {
          async move {
            nvim.command("set noswapfile").await?;
            let res = b.iter(|| {
              let nvim = nvim.clone();
              let _ = rt.block_on(async move {
                // Using `call` is not recommended. It returns a
                // Result<Result<Value, Value, CallError>> that needs to be massaged
                // in a proper Result<Value, CallError> at least. That's what the API
                // is for, but for now we don't want to deal with getting a buffer
                // from the API
                nvim
                  .call("nvim_buf_get_lines", call_args![0i64, 0i64, -1i64, false])
                  .await?;
                });
            });
            let _ = nvim.command("qa!").await;
            res
          }
        },
      ));
  })
}

criterion_group!(name = requests; config = Criterion::default().without_plots(); targets = simple_requests, request_file);
criterion_main!(requests);
