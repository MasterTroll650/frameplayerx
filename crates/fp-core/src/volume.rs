//! microSD cards and USB drives, known by their filesystem's ID.
//!
//! A card is mounted in a folder named after its label
//! (`/run/media/deck/SDCARD`), which two cards can share and which changes
//! with the label. Videos on a card are therefore located as
//! `volume:<id>/<path on the card>`, the ID being its filesystem's UUID or
//! serial number: [`stable`] makes such a location from a path while the
//! card is mounted, and [`resolve`] turns it back into one.

use crate::sync::{read, write};
use std::borrow::Cow;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

/// Where SteamOS (udisks) mounts microSD cards and USB drives:
/// `/run/media/deck/<label>` (older images: `/run/media/<device>`).
pub const REMOVABLE_ROOT: &str = "/run/media";

/// What locations on a card start with.
pub const SCHEME: &str = "volume:";

/// A mounted microSD card or USB drive.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Volume {
    pub mount: PathBuf,
    /// The kernel's device, e.g. `/dev/mmcblk0p1`.
    pub device: String,
    /// Its filesystem's UUID or serial number, when it has one.
    pub id: Option<String>,
}

impl Volume {
    /// The name it is mounted under, usually its label.
    pub fn name(&self) -> String {
        self.mount
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default()
    }
}

static MOUNTED: RwLock<Option<Vec<Volume>>> = RwLock::new(None);

/// The cards and drives mounted when last looked ([`refresh`]).
pub fn mounted() -> Vec<Volume> {
    if let Some(v) = read(&MOUNTED).as_ref() {
        return v.clone();
    }
    refresh()
}

/// Looks again which cards and drives are mounted.
pub fn refresh() -> Vec<Volume> {
    let mounts = std::fs::read_to_string("/proc/mounts").unwrap_or_default();
    let volumes = parse_mounts(&mounts, &disk_ids());
    *write(&MOUNTED) = Some(volumes.clone());
    volumes
}

/// The filesystem IDs of devices, from `/dev/disk/by-uuid`.
fn disk_ids() -> Vec<(String, String)> {
    let Ok(dir) = std::fs::read_dir("/dev/disk/by-uuid") else {
        return Vec::new();
    };
    dir.flatten()
        .filter_map(|e| {
            let id = e.file_name().to_str()?.to_string();
            let device = std::fs::canonicalize(e.path()).ok()?;
            Some((device.to_str()?.to_string(), id))
        })
        .collect()
}

/// The cards and drives in a `/proc/mounts` listing, with the IDs `ids`
/// (device, ID) gives their devices.
pub fn parse_mounts(mounts: &str, ids: &[(String, String)]) -> Vec<Volume> {
    let prefix = format!("{REMOVABLE_ROOT}/");
    let mut out: Vec<Volume> = mounts
        .lines()
        .filter_map(|l| {
            let mut fields = l.split_whitespace();
            let device = fields.next()?;
            let mount = unescape(fields.next()?);
            mount.starts_with(&prefix).then(|| Volume {
                id: ids
                    .iter()
                    .find(|(d, _)| d == device)
                    .map(|(_, id)| id.clone())
                    .filter(|id| valid_id(id)),
                device: device.to_string(),
                mount: PathBuf::from(mount),
            })
        })
        .collect();
    out.sort_by(|a, b| a.mount.cmp(&b.mount));
    out.dedup_by(|a, b| a.mount == b.mount);
    out
}

/// IDs are hex digits and dashes; anything else would make locations
/// ambiguous.
fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

/// `/proc/mounts` writes spaces and a few other bytes as `\ooo` octal.
fn unescape(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        let octal = b[i] == b'\\'
            && i + 3 < b.len()
            && b[i + 1..i + 4].iter().all(|c| (b'0'..=b'7').contains(c));
        if octal {
            out.push((b[i + 1] - b'0') * 64 + (b[i + 2] - b'0') * 8 + (b[i + 3] - b'0'));
            i += 4;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// `path` located by the card it is on, when it is on a mounted one with
/// an ID; anything else as it is.
pub fn stable(path: &str) -> Cow<'_, str> {
    if !path.starts_with(REMOVABLE_ROOT) {
        return Cow::Borrowed(path);
    }
    stable_in(&mounted(), path)
}

/// [`stable`] among `volumes`.
pub fn stable_in<'a>(volumes: &[Volume], path: &'a str) -> Cow<'a, str> {
    let p = Path::new(path);
    let innermost = volumes
        .iter()
        .filter(|v| p.starts_with(&v.mount))
        .max_by_key(|v| v.mount.as_os_str().len());
    let Some((v, id)) = innermost.and_then(|v| Some((v, v.id.as_deref()?))) else {
        return Cow::Borrowed(path);
    };
    match p.strip_prefix(&v.mount).ok().and_then(Path::to_str) {
        Some("") => Cow::Owned(format!("{SCHEME}{id}")),
        Some(rest) => Cow::Owned(format!("{SCHEME}{id}/{rest}")),
        None => Cow::Borrowed(path),
    }
}

/// A card location's card ID and path on the card (`""` for its top).
pub fn split(location: &str) -> Option<(&str, &str)> {
    let rest = location.strip_prefix(SCHEME)?;
    Some(match rest.split_once('/') {
        Some((id, path)) => (id, path.trim_start_matches('/')),
        None => (rest, ""),
    })
}

/// Where a location is now: a card's in the folder the card is mounted
/// in, `None` while it is not; anything else as it is.
pub fn resolve(location: &str) -> Option<Cow<'_, str>> {
    if !location.starts_with(SCHEME) {
        return Some(Cow::Borrowed(location));
    }
    resolve_in(&mounted(), location)
}

/// [`resolve`] among `volumes`.
pub fn resolve_in<'a>(volumes: &[Volume], location: &'a str) -> Option<Cow<'a, str>> {
    let Some((id, path)) = split(location) else {
        return Some(Cow::Borrowed(location));
    };
    if path.split('/').any(|c| c == "..") {
        return None;
    }
    let v = volumes.iter().find(|v| v.id.as_deref() == Some(id))?;
    let mut p = v.mount.clone();
    if !path.is_empty() {
        p.push(path);
    }
    Some(Cow::Owned(p.to_string_lossy().into_owned()))
}

/// Whether the card a location is on is mounted (always true for other
/// locations).
pub fn present(location: &str) -> bool {
    resolve(location).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn card(mount: &str, id: Option<&str>) -> Volume {
        Volume {
            mount: PathBuf::from(mount),
            device: "/dev/mmcblk0p1".into(),
            id: id.map(str::to_string),
        }
    }

    #[test]
    fn mounts_with_their_ids() {
        let m = "/dev/nvme0n1p8 /home ext4 rw 0 0\n\
                 /dev/mmcblk0p1 /run/media/deck/SD\\040Card ext4 rw,nosuid 0 0\n\
                 /dev/sda1 /run/media/deck/USB exfat rw 0 0\n\
                 /dev/mmcblk0p1 /run/media/mmcblk0p1 ext4 rw 0 0\n\
                 tmpfs /run/user/1000 tmpfs rw 0 0\n";
        let ids = [
            ("/dev/mmcblk0p1".to_string(), "1a2b-3C4D".to_string()),
            ("/dev/sda1".to_string(), "bad/id".to_string()),
        ];
        let v = parse_mounts(m, &ids);
        let got: Vec<(&str, Option<&str>)> = v
            .iter()
            .map(|v| (v.mount.to_str().unwrap(), v.id.as_deref()))
            .collect();
        assert_eq!(
            got,
            [
                ("/run/media/deck/SD Card", Some("1a2b-3C4D")),
                ("/run/media/deck/USB", None),
                ("/run/media/mmcblk0p1", Some("1a2b-3C4D")),
            ]
        );
        assert_eq!(v[0].name(), "SD Card");
    }

    #[test]
    fn cards_with_the_same_label_stay_apart() {
        let a = [card("/run/media/deck/SDCARD", Some("AAAA-0001"))];
        let b = [card("/run/media/deck/SDCARD", Some("BBBB-0002"))];
        let path = "/run/media/deck/SDCARD/Clips/a.mp4";
        let (la, lb) = (stable_in(&a, path), stable_in(&b, path));
        assert_eq!(la, "volume:AAAA-0001/Clips/a.mp4");
        assert_eq!(lb, "volume:BBBB-0002/Clips/a.mp4");
        assert_eq!(resolve_in(&a, &la).as_deref(), Some(path));
        assert_eq!(resolve_in(&b, &la), None, "card A is out");
    }

    #[test]
    fn a_card_is_found_wherever_it_is_mounted() {
        let then = [card("/run/media/deck/SDCARD", Some("AAAA-0001"))];
        let now = [
            card("/run/media/deck/SDCARD", None),
            card("/run/media/deck/SDCARD1", Some("AAAA-0001")),
        ];
        let loc = stable_in(&then, "/run/media/deck/SDCARD").into_owned();
        assert_eq!(loc, "volume:AAAA-0001");
        assert_eq!(
            resolve_in(&now, &loc).as_deref(),
            Some("/run/media/deck/SDCARD1")
        );
        // Not SDCARD's: the mount is matched by whole folder names.
        assert_eq!(
            stable_in(&now, "/run/media/deck/SDCARD1/x.mp4"),
            "volume:AAAA-0001/x.mp4"
        );
        assert_eq!(
            stable_in(&now, "/run/media/deck/SDCARD/x.mp4"),
            "/run/media/deck/SDCARD/x.mp4",
            "no ID: kept as a path"
        );
    }

    #[test]
    fn other_locations_pass_through() {
        let v = [card("/run/media/deck/SDCARD", Some("AAAA-0001"))];
        for l in ["/home/deck/Videos/a.mp4", "smb://nas/v/a.mp4", "/run/media"] {
            assert_eq!(stable_in(&v, l), l);
            assert_eq!(resolve_in(&v, l).as_deref(), Some(l));
        }
        assert_eq!(split("volume:AAAA-0001"), Some(("AAAA-0001", "")));
        assert_eq!(split("volume:AAAA-0001//a/b"), Some(("AAAA-0001", "a/b")));
        assert_eq!(
            resolve_in(&v, "volume:AAAA-0001//etc").as_deref(),
            Some("/run/media/deck/SDCARD/etc"),
            "never outside the card"
        );
        assert_eq!(resolve_in(&v, "volume:AAAA-0001/a/../../../etc"), None);
    }
}
