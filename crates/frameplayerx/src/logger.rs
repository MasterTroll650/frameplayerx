//! Minimal logger: stderr plus `~/.local/share/frameplayerx/frameplayerx.log`,
//! so logs survive when launched from Steam. Each start moves the last
//! run's log to `frameplayerx.log.old` (the one wanted after a hang or a
//! crash, once FramePlayerX has been started again). Lines carry the seconds
//! since start. Panics and fatal signals are logged too: nothing keeps
//! stderr when Steam starts FramePlayerX, so a crash left the log just
//! stopping.

use log::{LevelFilter, Log, Metadata, Record};
use std::io::Write;
use std::sync::Mutex;
use std::time::Instant;

struct Logger {
    file: Option<Mutex<std::fs::File>>,
    level: LevelFilter,
    start: Instant,
}

impl Log for Logger {
    fn enabled(&self, m: &Metadata) -> bool {
        m.level() <= self.level
    }
    fn log(&self, r: &Record) {
        if !self.enabled(r.metadata()) {
            return;
        }
        let line = format!(
            "{:9.3} [{:5}] {}: {}\n",
            self.start.elapsed().as_secs_f64(),
            r.level(),
            r.target(),
            r.args()
        );
        let _ = std::io::stderr().write_all(line.as_bytes());
        if let Some(f) = &self.file
            && let Ok(mut f) = f.lock()
        {
            let _ = f.write_all(line.as_bytes());
        }
    }
    fn flush(&self) {}
}

pub fn log_path() -> std::path::PathBuf {
    fp_core::dirs::data_dir().join("frameplayerx.log")
}

pub fn init(verbose: bool) {
    let _ = std::fs::create_dir_all(fp_core::dirs::data_dir());
    let path = log_path();
    let _ = std::fs::rename(&path, path.with_extension("log.old"));
    let file = std::fs::File::create(&path).ok();
    #[cfg(unix)]
    if let Some(f) = &file {
        fatal::watch(f);
    }
    let file = file.map(Mutex::new);
    let level = if verbose {
        LevelFilter::Debug
    } else {
        LevelFilter::Info
    };
    let logger = Box::leak(Box::new(Logger {
        file,
        level,
        start: Instant::now(),
    }));
    if log::set_logger(logger).is_ok() {
        log::set_max_level(level);
    }
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let thread = std::thread::current();
        log::error!(
            "thread {} {info}
{}",
            thread.name().unwrap_or("unnamed"),
            std::backtrace::Backtrace::force_capture()
        );
        default(info);
    }));
}

/// Notes a fatal signal (a crash in native code, an abort) in the log
/// before the process dies.
#[cfg(unix)]
mod fatal {
    use std::os::fd::AsRawFd;
    use std::sync::OnceLock;
    use std::sync::atomic::{AtomicI32, Ordering};

    const SIGNALS: [libc::c_int; 5] = [
        libc::SIGSEGV,
        libc::SIGBUS,
        libc::SIGILL,
        libc::SIGFPE,
        libc::SIGABRT,
    ];

    type Handler = extern "C" fn(libc::c_int, *mut libc::siginfo_t, *mut libc::c_void);

    static LOG_FD: AtomicI32 = AtomicI32::new(-1);
    /// What handled each signal before: Rust's own handler reports a
    /// stack overflow.
    static PREVIOUS: OnceLock<Vec<(libc::c_int, libc::sigaction)>> = OnceLock::new();

    pub fn watch(log: &std::fs::File) {
        LOG_FD.store(log.as_raw_fd(), Ordering::Relaxed);
        let mut previous = Vec::new();
        for sig in SIGNALS {
            // SAFETY: zeroed sigaction structs are valid, and the handler
            // only calls async-signal-safe functions.
            unsafe {
                let mut action: libc::sigaction = std::mem::zeroed();
                action.sa_sigaction = on_fatal as Handler as libc::sighandler_t;
                action.sa_flags = libc::SA_SIGINFO | libc::SA_ONSTACK;
                libc::sigemptyset(&mut action.sa_mask);
                let mut old: libc::sigaction = std::mem::zeroed();
                if libc::sigaction(sig, &action, &mut old) == 0 {
                    previous.push((sig, old));
                }
            }
        }
        let _ = PREVIOUS.set(previous);
    }

    extern "C" fn on_fatal(sig: libc::c_int, info: *mut libc::siginfo_t, ctx: *mut libc::c_void) {
        let fd = LOG_FD.load(Ordering::Relaxed);
        if fd >= 0 {
            let mut line = *b"          [FATAL] crashed with signal   
";
            let n = sig.clamp(0, 99) as u8;
            line[line.len() - 3] = b'0' + n / 10;
            line[line.len() - 2] = b'0' + n % 10;
            // SAFETY: write(2) is async-signal-safe and `line` outlives it.
            unsafe { libc::write(fd, line.as_ptr().cast(), line.len()) };
        }
        let previous = PREVIOUS
            .get()
            .and_then(|p| p.iter().find(|(s, _)| *s == sig))
            .map(|(_, a)| *a)
            .filter(|a| a.sa_sigaction != libc::SIG_DFL && a.sa_sigaction != libc::SIG_IGN);
        // SAFETY: the previous handler was installed for this signal, with
        // the signature its flags say; failing that, the default action
        // ends the process when the signal comes again.
        unsafe {
            match previous {
                Some(a) if a.sa_flags & libc::SA_SIGINFO != 0 => {
                    let handler: Handler = std::mem::transmute(a.sa_sigaction);
                    handler(sig, info, ctx);
                }
                Some(a) => {
                    let handler: extern "C" fn(libc::c_int) = std::mem::transmute(a.sa_sigaction);
                    handler(sig);
                }
                None => {
                    libc::signal(sig, libc::SIG_DFL);
                }
            }
        }
    }
}
