//! The performance overlay's system numbers: CPU, memory and this
//! process's GPU time, read from /proc and /sys twice a second, on a thread
//! of their own, while the overlay shows.

use fp_core::sync::lock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const INTERVAL: Duration = Duration::from_millis(500);

/// One reading, over the half second before it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Sample {
    /// The whole device's CPU, percent of all cores.
    pub cpu: Option<f32>,
    /// FramePlayerX's share, percent of all cores.
    pub app_cpu: Option<f32>,
    /// Memory in use (total less available) and the total, bytes.
    pub mem: Option<(u64, u64)>,
    /// FramePlayerX's resident memory, bytes.
    pub app_mem: Option<u64>,
    /// Share of the time the GPU spent on FramePlayerX's work, percent;
    /// None where the driver doesn't report it.
    pub gpu: Option<f32>,
    /// The GPU's clock, MHz.
    pub gpu_mhz: Option<u32>,
}

/// Samples while alive; the thread stops at its next reading once this is
/// dropped.
pub struct Monitor {
    latest: Arc<Mutex<Option<Sample>>>,
    stop: Arc<AtomicBool>,
}

impl Monitor {
    pub fn start() -> Monitor {
        let latest = Arc::new(Mutex::new(None));
        let stop = Arc::new(AtomicBool::new(false));
        let (out, quit) = (latest.clone(), stop.clone());
        let spawned = std::thread::Builder::new()
            .name("fp-perf".into())
            .spawn(move || {
                let mut prev = Counters::read();
                while !quit.load(Ordering::Relaxed) {
                    std::thread::sleep(INTERVAL);
                    let now = Counters::read();
                    *lock(&out) = Some(now.since(&prev));
                    prev = now;
                }
            });
        if let Err(e) = spawned {
            log::warn!("performance sampler: {e}");
        }
        Monitor { latest, stop }
    }

    /// The newest reading (none until the first half second has passed).
    pub fn latest(&self) -> Option<Sample> {
        *lock(&self.latest)
    }
}

impl Drop for Monitor {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

/// Cumulative counters at one moment; two give a [`Sample`].
struct Counters {
    at: Instant,
    /// Busy and total CPU time of all cores, in ticks, and the core count.
    cpu: Option<(u64, u64, usize)>,
    app_ticks: Option<u64>,
    gpu_ns: Option<u64>,
    mem: Option<(u64, u64)>,
    app_mem: Option<u64>,
    gpu_mhz: Option<u32>,
}

impl Counters {
    fn read() -> Counters {
        let file = |p: &str| std::fs::read_to_string(p).ok();
        Counters {
            at: Instant::now(),
            cpu: file("/proc/stat").as_deref().and_then(cpu_times),
            app_ticks: file("/proc/self/stat").as_deref().and_then(process_ticks),
            gpu_ns: gpu_time(),
            mem: file("/proc/meminfo").as_deref().and_then(meminfo),
            app_mem: file("/proc/self/status").as_deref().and_then(vm_rss),
            gpu_mhz: gpu_clock(),
        }
    }

    fn since(&self, prev: &Counters) -> Sample {
        let secs = self.at.duration_since(prev.at).as_secs_f64().max(1e-3);
        let pct = |part: f64, whole: f64| (whole > 0.0).then(|| (100.0 * part / whole) as f32);
        let cpu = match (self.cpu, prev.cpu) {
            (Some((b, t, _)), Some((pb, pt, _))) => {
                pct(b.saturating_sub(pb) as f64, t.saturating_sub(pt) as f64)
            }
            _ => None,
        };
        let cores = self.cpu.map_or(1, |c| c.2.max(1)) as f64;
        let app_cpu = match (self.app_ticks, prev.app_ticks) {
            (Some(a), Some(pa)) => pct(
                a.saturating_sub(pa) as f64 / ticks_per_second(),
                secs * cores,
            ),
            _ => None,
        };
        let gpu = match (self.gpu_ns, prev.gpu_ns) {
            (Some(g), Some(pg)) => {
                pct(g.saturating_sub(pg) as f64, secs * 1e9).map(|p| p.min(100.0))
            }
            _ => None,
        };
        Sample {
            cpu,
            app_cpu,
            mem: self
                .mem
                .map(|(total, available)| (total.saturating_sub(available), total)),
            app_mem: self.app_mem,
            gpu,
            gpu_mhz: self.gpu_mhz,
        }
    }
}

fn ticks_per_second() -> f64 {
    // SAFETY: sysconf only reads a system constant.
    let t = unsafe { libc::sysconf(libc::_SC_CLK_TCK) };
    if t > 0 { t as f64 } else { 100.0 }
}

/// From /proc/stat: busy and total time of all cores (ticks) and how many
/// cores there are. Waiting for I/O counts as idle; guest time is already
/// in user time.
fn cpu_times(stat: &str) -> Option<(u64, u64, usize)> {
    let first = stat.lines().next()?.strip_prefix("cpu ")?;
    let v: Vec<u64> = first
        .split_whitespace()
        .take(8)
        .map(|f| f.parse().ok())
        .collect::<Option<_>>()?;
    let (idle, iowait) = (*v.get(3)?, *v.get(4)?);
    let total: u64 = v.iter().sum();
    let cores = stat
        .lines()
        .filter(|l| {
            l.strip_prefix("cpu")
                .is_some_and(|r| r.starts_with(|c: char| c.is_ascii_digit()))
        })
        .count();
    Some((total - idle - iowait, total, cores))
}

/// From /proc/self/stat: user and system time, in ticks. The process name
/// can hold spaces and parentheses, so fields count from its last ')'.
fn process_ticks(stat: &str) -> Option<u64> {
    let fields: Vec<&str> = stat.rsplit_once(')')?.1.split_whitespace().collect();
    let utime: u64 = fields.get(11)?.parse().ok()?;
    let stime: u64 = fields.get(12)?.parse().ok()?;
    Some(utime + stime)
}

/// A `Key:   123 kB` value from /proc/meminfo or /proc/self/status, bytes.
fn kb_field(text: &str, key: &str) -> Option<u64> {
    let line = text.lines().find(|l| l.starts_with(key))?;
    let kb: u64 = line[key.len()..]
        .trim_start_matches(':')
        .split_whitespace()
        .next()?
        .parse()
        .ok()?;
    Some(kb * 1024)
}

/// Total and available memory, bytes.
fn meminfo(text: &str) -> Option<(u64, u64)> {
    Some((kb_field(text, "MemTotal")?, kb_field(text, "MemAvailable")?))
}

fn vm_rss(status: &str) -> Option<u64> {
    kb_field(status, "VmRSS")
}

/// A DRM client's id and its GPU time over all engines (ns), from the
/// fdinfo of a file open on a /dev/dri node.
fn drm_engine_time(fdinfo: &str) -> Option<(u64, u64)> {
    let mut id = None;
    let mut ns = None;
    for line in fdinfo.lines() {
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let value = value.trim();
        if key == "drm-client-id" {
            id = value.parse().ok();
        } else if key.starts_with("drm-engine-")
            && let Some(n) = value
                .strip_suffix(" ns")
                .and_then(|n| n.parse::<u64>().ok())
        {
            *ns.get_or_insert(0) += n;
        }
    }
    Some((id?, ns?))
}

/// This process's GPU time over all its DRM clients (ns): each open
/// /dev/dri file reports its client's, and several files can share one.
fn gpu_time() -> Option<u64> {
    let mut seen = Vec::new();
    let mut total = None;
    for entry in std::fs::read_dir("/proc/self/fd").ok()?.flatten() {
        let on_dri =
            std::fs::read_link(entry.path()).is_ok_and(|target| target.starts_with("/dev/dri/"));
        if !on_dri {
            continue;
        }
        let info = std::fs::read_to_string(format!(
            "/proc/self/fdinfo/{}",
            entry.file_name().to_string_lossy()
        ));
        if let Some((id, ns)) = info.ok().as_deref().and_then(drm_engine_time)
            && !seen.contains(&id)
        {
            seen.push(id);
            *total.get_or_insert(0) += ns;
        }
    }
    total
}

/// The GPU's clock (MHz) from its devfreq node, the one named for a GPU.
fn gpu_clock() -> Option<u32> {
    std::fs::read_dir("/sys/class/devfreq")
        .ok()?
        .flatten()
        .find(|e| e.file_name().to_string_lossy().contains("gpu"))
        .and_then(|e| std::fs::read_to_string(e.path().join("cur_freq")).ok())
        .and_then(|hz| hz.trim().parse::<u64>().ok())
        .map(|hz| (hz / 1_000_000) as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    // As read on a Steam Frame (SteamOS 0.4.5).
    const STAT: &str = "cpu  400447 132664 196222 7662770 1308 77230 21967 0 0 0\n\
                        cpu0 41641 17281 24359 952174 79 21152 4003 0 0 0\n\
                        cpu1 41118 16882 24181 964064 68 10200 1308 0 0 0\n\
                        intr 1234 5 6\n";
    const SELF_STAT: &str = "46731 (fp perf) (x)) R 46728 46728 46728 0 -1 4194304 68 0 0 0 \
                             250 75 0 0 20 0 1 0 1071185 2527232 255";
    const MEMINFO: &str = "MemTotal:       16038044 kB\nMemFree:         9109772 kB\n\
                           MemAvailable:   11404720 kB\n";
    const FDINFO: &str = "pos:\t0\nflags:\t02400002\nmnt_id:\t30\nino:\t250\n\
                          drm-driver:\tmsm\ndrm-client-id:\t281\n\
                          drm-engine-gpu:\t1500000 ns\ndrm-cycles-gpu:\t0\n\
                          drm-maxfreq-gpu:\t903000000 Hz\ndrm-total-memory:\t0\n";

    #[test]
    fn parses_what_the_kernel_writes() {
        let total = 400447 + 132664 + 196222 + 7662770 + 1308 + 77230 + 21967;
        assert_eq!(cpu_times(STAT), Some((total - 7662770 - 1308, total, 2)));
        assert_eq!(process_ticks(SELF_STAT), Some(325), "name with ') ' in it");
        assert_eq!(
            meminfo(MEMINFO),
            Some((16_038_044 * 1024, 11_404_720 * 1024))
        );
        assert_eq!(vm_rss("Name:\tx\nVmRSS:\t    1368 kB\n"), Some(1368 * 1024));
        assert_eq!(drm_engine_time(FDINFO), Some((281, 1_500_000)));
        assert_eq!(
            drm_engine_time("pos:\t0\nflags:\t0\n"),
            None,
            "not a DRM file"
        );
        assert_eq!(cpu_times("garbage"), None);
    }

    #[test]
    fn readings_become_percentages() {
        let at = Instant::now();
        let prev = Counters {
            at,
            cpu: Some((100, 1000, 8)),
            app_ticks: Some(0),
            gpu_ns: Some(0),
            mem: Some((16 << 30, 12 << 30)),
            app_mem: Some(1 << 30),
            gpu_mhz: Some(500),
        };
        let now = Counters {
            at: at + Duration::from_millis(500),
            cpu: Some((400, 2000, 8)),
            app_ticks: Some(20),
            gpu_ns: Some(250_000_000),
            mem: Some((16 << 30, 10 << 30)),
            app_mem: Some(2 << 30),
            gpu_mhz: Some(680),
        };
        let s = now.since(&prev);
        assert_eq!(s.cpu, Some(30.0));
        // 20 ticks of one core in 0.5 s of 8 cores (5% at 100 ticks a second).
        let expected = (100.0 * 20.0 / ticks_per_second() / (0.5 * 8.0)) as f32;
        let app = s.app_cpu.unwrap();
        assert!((app - expected).abs() < 0.01, "{app} vs {expected}");
        assert_eq!(s.gpu, Some(50.0));
        assert_eq!(s.mem, Some((6 << 30, 16 << 30)));
        assert_eq!((s.app_mem, s.gpu_mhz), (Some(2 << 30), Some(680)));
        // A counter that can't be read gives no number, not a zero.
        let blind = Counters {
            gpu_ns: None,
            ..now
        };
        assert_eq!(blind.since(&prev).gpu, None);
    }

    #[test]
    fn reads_this_machine() {
        let a = Counters::read();
        std::thread::sleep(Duration::from_millis(50));
        let s = Counters::read().since(&a);
        if cfg!(target_os = "linux") {
            assert!(s.cpu.is_some() && s.app_cpu.is_some());
            assert!(
                s.mem
                    .is_some_and(|(used, total)| used <= total && total > 0)
            );
            assert!(s.app_mem.is_some_and(|m| m > 0));
        }
    }
}
