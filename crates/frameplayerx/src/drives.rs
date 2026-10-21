//! microSD cards and USB drives: which are mounted, and mounting those
//! SteamOS leaves alone.

use std::path::PathBuf;

pub use fp_core::volume::REMOVABLE_ROOT;

/// Mounted microSD cards and USB drives, as last looked
/// ([`refresh_mounts`]).
pub fn removable_mounts() -> Vec<PathBuf> {
    fp_core::volume::mounted()
        .into_iter()
        .map(|v| v.mount)
        .collect()
}

/// Looks again which microSD cards and USB drives are mounted.
pub fn refresh_mounts() -> Vec<PathBuf> {
    fp_core::volume::refresh()
        .into_iter()
        .map(|v| v.mount)
        .collect()
}

/// A drive partition SteamOS left unmounted that FramePlayerX mounts itself.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnmountedDrive {
    /// Kernel name, e.g. `mmcblk0p1`.
    pub device: String,
    pub fs_type: String,
}

/// Whether FramePlayerX should mount this partition. The Frame's SteamOS
/// automounts only ext4 microSD cards (Steam libraries need symlinks) and
/// no USB drives at all (`sd*` also names its internal OS slots), so a card
/// formatted on Windows is never mounted. `sys_path` is the resolved
/// `/sys/class/block/<device>` link; USB drives are told apart from the
/// internal UFS disks by it.
pub fn should_mount(device: &str, sys_path: &str, fs_type: &str) -> bool {
    const FOREIGN: [&str; 4] = ["exfat", "vfat", "ntfs", "ntfs3"];
    if device.starts_with("mmcblk") {
        // ext4 cards are SteamOS's: it checks them before mounting.
        FOREIGN.contains(&fs_type)
    } else {
        sys_path.contains("/usb") && (FOREIGN.contains(&fs_type) || fs_type == "ext4")
    }
}

/// `ID_FS_TYPE` from a udev database record (`/run/udev/data/b<maj>:<min>`).
pub fn udev_fs_type(record: &str) -> Option<&str> {
    record
        .lines()
        .find_map(|l| l.strip_prefix("E:ID_FS_TYPE="))
        .filter(|t| !t.is_empty())
}

/// microSD cards and USB drives with a readable filesystem that nothing
/// has mounted.
pub fn unmounted_drives() -> Vec<UnmountedDrive> {
    let mounted = std::fs::read_to_string("/proc/mounts").unwrap_or_default();
    let mounted: Vec<&str> = mounted
        .lines()
        .filter_map(|l| l.split_whitespace().next())
        .collect();
    let Ok(dir) = std::fs::read_dir("/sys/class/block") else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for e in dir.flatten() {
        let device = e.file_name().to_string_lossy().into_owned();
        if !device.starts_with("mmcblk") && !device.starts_with("sd") {
            continue;
        }
        let sys_path = std::fs::canonicalize(e.path())
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default();
        if !device.starts_with("mmcblk") && !sys_path.contains("/usb") {
            continue;
        }
        let Ok(devnum) = std::fs::read_to_string(e.path().join("dev")) else {
            continue;
        };
        let Ok(record) = std::fs::read_to_string(format!("/run/udev/data/b{}", devnum.trim()))
        else {
            continue;
        };
        let Some(fs_type) = udev_fs_type(&record) else {
            continue;
        };
        if should_mount(&device, &sys_path, fs_type)
            && !mounted.contains(&format!("/dev/{device}").as_str())
        {
            out.push(UnmountedDrive {
                fs_type: fs_type.to_string(),
                device,
            });
        }
    }
    out.sort_by(|a, b| a.device.cmp(&b.device));
    out
}

/// Mounts a drive through udisks, as SteamOS does, under
/// `/run/media/<user>/<label>`. The Frame's polkit rules let the logged-in
/// user do this without a password. Blocking: call from a job thread.
pub fn mount_drive(drive: &UnmountedDrive) -> Result<PathBuf, String> {
    let out = std::process::Command::new("udisksctl")
        .args(["mount", "--no-user-interaction", "-o", "noatime", "-b"])
        .arg(format!("/dev/{}", drive.device))
        .output()
        .map_err(|e| format!("udisksctl: {e}"))?;
    let text = String::from_utf8_lossy(&out.stdout);
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        return Err(err.trim().to_string());
    }
    // "Mounted /dev/mmcblk0p1 at /run/media/steamos/SDCARD"
    text.trim()
        .split_once(" at ")
        .map(|(_, p)| PathBuf::from(p.trim_end_matches('.')))
        .ok_or_else(|| format!("unexpected udisksctl output: {}", text.trim()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mounts_what_steamos_leaves_alone() {
        let card = "/sys/devices/platform/soc@0/8804000.mmc/mmc_host/mmc0/mmc0:d555/block/mmcblk0/mmcblk0p1";
        let usb = "/sys/devices/platform/soc@0/a600000.usb/xhci-hcd.1.auto/usb1/1-1/1-1:1.0/host0/target0:0:0/0:0:0:0/block/sde/sde1";
        let ufs =
            "/sys/devices/platform/soc@0/1d84000.ufshc/host0/target0:0:0/0:0:0:0/block/sda/sda8";
        assert!(should_mount("mmcblk0p1", card, "exfat"));
        assert!(should_mount("mmcblk0p1", card, "vfat"));
        assert!(!should_mount("mmcblk0p1", card, "ext4"));
        assert!(should_mount("sde1", usb, "exfat"));
        assert!(should_mount("sde1", usb, "ext4"));
        assert!(!should_mount("sda8", ufs, "ext4"));
        assert!(!should_mount("sda1", ufs, "vfat"));
        assert!(!should_mount("sde1", usb, "swap"));
    }

    #[test]
    fn fs_type_from_udev_record() {
        let r = "S:disk/by-label/SDCARD\nE:ID_FS_LABEL=SDCARD\nE:ID_FS_TYPE=exfat\nE:ID_FS_USAGE=filesystem\n";
        assert_eq!(udev_fs_type(r), Some("exfat"));
        assert_eq!(udev_fs_type("E:ID_FS_TYPE=\n"), None);
        assert_eq!(udev_fs_type("E:ID_PART_TABLE_TYPE=dos\n"), None);
    }
}
