//! The performance overlay (Settings › About › Diagnostics): frame time,
//! the video's frame rate and decoder, CPU, GPU and memory, in a small
//! panel low in your view.

use super::theme::{self, Weight};
use crate::perf::Sample;
use egui::{Color32, RichText};

/// What the overlay shows.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Overlay {
    /// Render-thread time per frame over the last second: typical and
    /// longest, ms.
    pub frame_ms: Option<(f32, f32)>,
    /// The display's refresh rate.
    pub display_hz: Option<f32>,
    /// The video's frames shown and dropped per second, and its decoder
    /// ("HW · HEVC 10-bit").
    pub video: Option<(f32, f32, String)>,
    pub sample: Option<Sample>,
}

/// Label, value, and whether the value is a warning sign: frames over the
/// display's time, or video frames dropped.
pub fn rows(o: &Overlay) -> Vec<(&'static str, String, bool)> {
    const NONE: &str = "–";
    let gb = |bytes: u64| bytes as f64 / (1u64 << 30) as f64;
    let mut rows = Vec::new();
    let budget = o.display_hz.filter(|hz| *hz > 0.0).map(|hz| 1000.0 / hz);
    rows.push(match o.frame_ms {
        Some((typical, longest)) => {
            let hz = o
                .display_hz
                .map_or(String::new(), |hz| format!(" · {hz:.0} Hz"));
            (
                "Frame",
                format!("{typical:.1} ms, longest {longest:.1}{hz}"),
                budget.is_some_and(|b| longest > b),
            )
        }
        None => ("Frame", NONE.into(), false),
    });
    if let Some((fps, dropped, decoder)) = &o.video {
        rows.push((
            "Video",
            format!("{fps:.1} fps · {dropped:.0} dropped/s"),
            *dropped > 0.0,
        ));
        rows.push(("Decoder", decoder.clone(), false));
    }
    let s = o.sample.unwrap_or_default();
    rows.push((
        "CPU",
        match (s.cpu, s.app_cpu) {
            (Some(all), Some(app)) => format!("{all:.0}% · app {app:.0}%"),
            (Some(all), None) => format!("{all:.0}%"),
            _ => NONE.into(),
        },
        false,
    ));
    rows.push((
        "GPU",
        match (s.gpu, s.gpu_mhz) {
            (Some(busy), Some(mhz)) => format!("{busy:.0}% · {mhz} MHz"),
            (Some(busy), None) => format!("{busy:.0}%"),
            (None, Some(mhz)) => format!("{mhz} MHz"),
            (None, None) => NONE.into(),
        },
        false,
    ));
    rows.push((
        "Memory",
        match (s.mem, s.app_mem) {
            (Some((used, total)), Some(app)) => format!(
                "{:.1} of {:.1} GB · app {:.1} GB",
                gb(used),
                gb(total),
                gb(app)
            ),
            (Some((used, total)), None) => format!("{:.1} of {:.1} GB", gb(used), gb(total)),
            _ => NONE.into(),
        },
        false,
    ));
    rows
}

pub fn overlay(ctx: &egui::Context, o: &Overlay) {
    let clear = egui::Frame::new().fill(Color32::TRANSPARENT);
    egui::CentralPanel::default().frame(clear).show(ctx, |ui| {
        egui::Frame::new()
            .fill(Color32::from_black_alpha(200))
            .corner_radius(12)
            .inner_margin(egui::Margin::symmetric(16, 12))
            .show(ui, |ui| {
                egui::Grid::new("perf")
                    .num_columns(2)
                    .spacing([16.0, 6.0])
                    .show(ui, |ui| {
                        for (label, value, warn) in rows(o) {
                            ui.label(
                                RichText::new(label)
                                    .font(theme::font(Weight::SemiBold, 24.0))
                                    .color(theme::TEXT_2),
                            );
                            ui.label(
                                RichText::new(value)
                                    .font(theme::font(Weight::Regular, 24.0))
                                    .color(if warn { theme::WARN } else { theme::TEXT }),
                            );
                            ui.end_row();
                        }
                    });
            });
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_say_what_is_known_and_flag_trouble() {
        let o = Overlay {
            frame_ms: Some((6.12, 12.4)),
            display_hz: Some(90.0),
            video: Some((59.94, 0.0, "HW · HEVC 10-bit".into())),
            sample: Some(Sample {
                cpu: Some(34.4),
                app_cpu: Some(21.0),
                mem: Some((5 << 30, 15 << 30)),
                app_mem: Some(3 << 29),
                gpu: Some(41.0),
                gpu_mhz: Some(680),
            }),
        };
        let r = rows(&o);
        assert_eq!(
            r[0],
            ("Frame", "6.1 ms, longest 12.4 · 90 Hz".into(), true),
            "over 11.1 ms"
        );
        assert_eq!(r[1], ("Video", "59.9 fps · 0 dropped/s".into(), false));
        assert_eq!(r[2], ("Decoder", "HW · HEVC 10-bit".into(), false));
        assert_eq!(r[3].1, "34% · app 21%");
        assert_eq!(r[4].1, "41% · 680 MHz");
        assert_eq!(r[5].1, "5.0 of 15.0 GB · app 1.5 GB");

        // Nothing known yet, no video: a dash rather than a made-up zero.
        let r = rows(&Overlay::default());
        assert_eq!(r.len(), 4);
        assert!(r.iter().all(|(_, v, warn)| v == "–" && !warn));
    }
}
