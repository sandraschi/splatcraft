//! Run a tool briefly and capture its output (`--help`, ffprobe).

use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use crate::error::{Result, TrainError};

const MAX_CAPTURE: u64 = 1024 * 1024;

pub trait Probe {
    /// Run `program args` and return stdout plus stderr, within `max_secs`.
    fn run_capture(&self, program: &Path, args: &[&str], max_secs: u64) -> Result<String>;
}

pub struct SystemProbe;

#[cfg(windows)]
fn no_window(cmd: &mut Command) {
    use std::os::windows::process::CommandExt;
    cmd.creation_flags(0x0800_0000);
}

#[cfg(not(windows))]
fn no_window(_cmd: &mut Command) {}

impl Probe for SystemProbe {
    fn run_capture(&self, program: &Path, args: &[&str], max_secs: u64) -> Result<String> {
        let tmp = std::env::temp_dir().join(format!(
            "splatcraft-probe-{}-{}.txt",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_nanos())
        ));
        let out = std::fs::File::create(&tmp)?;
        let err = out.try_clone()?;
        let mut cmd = Command::new(program);
        cmd.args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::from(out))
            .stderr(Stdio::from(err));
        no_window(&mut cmd);
        let mut child = cmd
            .spawn()
            .map_err(|e| TrainError::Io(format!("cannot start {}: {e}", program.display())))?;
        let start = Instant::now();
        let finished = loop {
            if child.try_wait()?.is_some() {
                break true;
            }
            if start.elapsed() > Duration::from_secs(max_secs) {
                let _ = child.kill();
                let _ = child.wait();
                break false;
            }
            std::thread::sleep(Duration::from_millis(50));
        };
        let text = read_capped(&tmp);
        let _ = std::fs::remove_file(&tmp);
        if !finished {
            return Err(TrainError::Io(format!(
                "{} did not finish within {max_secs}s",
                program.display()
            )));
        }
        Ok(text)
    }
}

fn read_capped(path: &Path) -> String {
    use std::io::Read;
    let mut buf = Vec::new();
    if let Ok(f) = std::fs::File::open(path) {
        let _ = f.take(MAX_CAPTURE).read_to_end(&mut buf);
    }
    String::from_utf8_lossy(&buf).into_owned()
}

#[cfg(test)]
pub mod fake {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use std::cell::RefCell;

    /// Canned output keyed by (program file name, first argument).
    #[derive(Default)]
    pub struct FakeProbe {
        pub replies: Vec<(String, String, String)>,
        pub calls: RefCell<Vec<String>>,
    }

    impl FakeProbe {
        pub fn reply(mut self, prog: &str, first_arg: &str, text: &str) -> Self {
            self.replies
                .push((prog.into(), first_arg.into(), text.into()));
            self
        }
    }

    impl Probe for FakeProbe {
        fn run_capture(&self, program: &Path, args: &[&str], _max: u64) -> Result<String> {
            let name = program
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_string();
            let first = args.first().copied().unwrap_or("");
            self.calls
                .borrow_mut()
                .push(format!("{name} {}", args.join(" ")));
            self.replies
                .iter()
                .find(|(p, a, _)| *p == name && (a == first || a == "*"))
                .map(|(_, _, t)| t.clone())
                .ok_or_else(|| TrainError::Io(format!("no fake reply for {name} {first}")))
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    #[test]
    fn captures_output_of_a_real_process() {
        // The running test binary lists its tests and exits 0.
        let exe = std::env::current_exe().unwrap();
        let out = SystemProbe.run_capture(&exe, &["--list"], 30).unwrap();
        assert!(out.contains("captures_output_of_a_real_process"), "{out}");
    }

    #[test]
    fn missing_program_is_an_error() {
        let r = SystemProbe.run_capture(Path::new("/no/such/program-xyz"), &[], 5);
        assert!(r.is_err());
    }
}
