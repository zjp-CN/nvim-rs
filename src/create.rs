use std::{
  self,
  future::Future,
  io::{Error, ErrorKind},
  path::Path,
  process::Stdio,
};

use crate::{
  callerror::{EnterError, LoopError},
  neovim::Neovim,
  runtime::{
    spawn, AsyncWrite, ChildStdin, Command, JoinHandle, Stdout,
    TcpStream,
  },
  Handler,
};

#[cfg(unix)]
use crate::runtime::{stdin, stdout, UnixStream};

/// Connect to nvim instance via tcp
pub async fn new_tcp<H, F>(
  host: &str,
  port: u16,
  handler: H,
  init: impl FnOnce(Neovim<TcpStream>) -> F,
) -> std::result::Result<(), Box<EnterError>>
where
  H: Handler<Writer =TcpStream> + Send + 'static,
  F: Future<Output = Result<(), Box<EnterError>>>,
{
  let stream = TcpStream::connect((host, port)).await.unwrap();
  let read = TcpStream::connect((host, port)).await.unwrap();
  let (nvim, io) = Neovim::<TcpStream>::new(stream, read, handler);

  run(nvim, spawn(io), init).await
}

#[cfg(unix)]
/// Connect to nvim instance via unix socket
pub async fn new_unix_socket<H, F, P: AsRef<Path> + Clone>(
  path: P,
  handler: H,
  init: impl FnOnce(Neovim<UnixStream>) -> F,
) -> std::result::Result<(), Box<EnterError>>
where
  H: Handler<Writer = UnixStream> + Send + 'static,
  F: Future<Output = Result<(), Box<EnterError>>>,
{
  let stream = UnixStream::connect(path.clone()).await.unwrap();
  let read = UnixStream::connect(path).await.unwrap();

  let (nvim, io) = Neovim::<UnixStream>::new(stream, read, handler);

  run(nvim, spawn(io), init).await
}

/// Connect to a Neovim instance that spawned this process over stdin/stdout.
pub async fn from_parent<H, F>(
  handler: H,
  init: impl FnOnce(Neovim<Stdout>) -> F,
) -> std::result::Result<(), Box<EnterError>>
where
  H: Handler<Writer = Stdout> + Send + 'static,
  F: Future<Output = Result<(), Box<EnterError>>>,
{
  let (nvim, io) = Neovim::<Stdout>::new(stdin(), stdout(), handler);

  run(nvim, spawn(io), init).await
}

/// Connect to a Neovim instance by spawning a new one.
pub async fn run_child<H, F>(
  handler: H,
  init: impl FnOnce(Neovim<ChildStdin>) -> F,
) -> std::result::Result<(), Box<EnterError>>
where
  H: Handler<Writer = ChildStdin> + Send + 'static,
  F: Future<Output = Result<(), Box<EnterError>>>,
{
  if cfg!(target_os = "windows") {
    run_child_path("nvim.exe", handler, init).await
  } else {
    run_child_path("nvim", handler, init).await
  }
}

/// Connect to a Neovim instance by spawning a new one from a given path
pub async fn run_child_path<H, F, S>(
  program: S,
  handler: H,
  init: impl FnOnce(Neovim<ChildStdin>) -> F,
) -> std::result::Result<(), Box<EnterError>>
where
  H: Handler<Writer = ChildStdin> + Send + 'static,
  F: Future<Output = Result<(), Box<EnterError>>>,
  S: AsRef<Path>,
{
  run_child_cmd(Command::new(program.as_ref()).arg("--embed"), handler,
  init).await
}

/// Connect to a Neovim instance by spawning a new one by running the given
/// command
///
/// stdin/stdout settings will be rewrited to `Stdio::piped()`
pub async fn run_child_cmd<H, F>(
  cmd: &mut Command,
  handler: H,
  init: impl FnOnce(Neovim<ChildStdin>) -> F,
) -> std::result::Result<(), Box<EnterError>>
where
  H: Handler<Writer = ChildStdin> + Send + 'static,
  F: Future<Output = Result<(), Box<EnterError>>>,
{
  let mut child = cmd
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .spawn()
    .unwrap();
  let stdout = child
    .stdout()
    .take()
    .ok_or_else(|| Error::new(ErrorKind::Other, "Can't open stdout"))
    .unwrap();
  let stdin = child
    .stdin()
    .take()
    .ok_or_else(|| Error::new(ErrorKind::Other, "Can't open stdin"))
    .unwrap();

  let (nvim, io) = Neovim::<ChildStdin>::new(stdout, stdin, handler);

  run(nvim, spawn(io), init).await
}

async fn run<W, F>(
  nvim: Neovim<W>,
  io_handle: JoinHandle<Result<(), Box<LoopError>>>,
  init: impl FnOnce(Neovim<W>) -> F,
) -> std::result::Result<(), Box<EnterError>>
where
  W: AsyncWrite + Send + Unpin + 'static,
  F: Future<Output = Result<(), Box<EnterError>>>,
{
  init(nvim).await?;
  match io_handle.await {
    // Result<Result<(), Box<LoopError>>, JoinErr>
    Ok(Ok(r)) => Ok(r),
    Ok(Err(e)) => Err(e)?,
    Err(e) => Err(e)?,
  }
}
