use std::fs::File;
use std::io::{self, Read};
use std::path::Path;

use glib::{
    child_watch_add, current_dir, environ, shell_parse_argv, spawn_async_with_pipes,
    spawn_check_exit_status, Error, Pid, SourceId, SpawnFlags,
};
use gtk::glib;
use log::{error, log, warn, Level};

pub fn spawn_hook(hook: &str) -> Result<SourceId, Error> {
    let args = shell_parse_argv(hook)?;
    let args = args.iter().map(Path::new).collect::<Vec<_>>();
    let env = environ();
    let env = env.iter().map(Path::new).collect::<Vec<_>>();
    let (pid, _, mut stdout, mut stderr): (Pid, File, File, File) = spawn_async_with_pipes(
        current_dir(),
        &args,
        &env,
        SpawnFlags::SEARCH_PATH | SpawnFlags::DO_NOT_REAP_CHILD,
        None,
    )?;
    Ok(child_watch_add(pid, move |_, status| log_hook_status(status, &mut stdout, &mut stderr)))
}

fn log_hook_status(status: i32, stdout: &mut File, stderr: &mut File) {
    match spawn_check_exit_status(status) {
        Ok(()) => log_stream(stdout, Level::Info),
        Err(e) => {
            warn!("{e}");
            log_stream(stderr, Level::Warn);
        }
    }
}

fn log_stream(stream: &mut File, lvl: Level) {
    match read_stream(stream) {
        Ok(stream) => {
            if !stream.is_empty() {
                log!(lvl, "{}", stream);
            }
        }
        Err(e) => error!("Failed read stream: {e}"),
    }
}

fn read_stream(stream: &mut File) -> io::Result<String> {
    let mut buf = String::new();
    stream.read_to_string(&mut buf).map(|_| Ok(buf.trim_end_matches('\n').to_owned()))?
}
