//! The page offered after a crash: what a report contains and where it
//! goes, the report itself on request, and the choice to send it.

use super::theme::{self, Weight};
use super::widgets::{self, Kind};
use super::{Action, View, icons};
use egui::RichText;

pub struct CrashPrompt {
    pub report: crate::crash::Report,
    /// The whole report is on the page.
    pub shown: bool,
    pub sending: bool,
    /// Why it could not be sent.
    pub error: Option<String>,
}

impl CrashPrompt {
    pub fn new(report: crate::crash::Report) -> CrashPrompt {
        CrashPrompt {
            report,
            shown: false,
            sending: false,
            error: None,
        }
    }
}

const CONTAINS: [&str; 3] = [
    "The versions of FramePlayerX and of the headset's system software.",
    "What went wrong, and FramePlayerX's log of that session: what it was doing, \
     such as the formats and decoders it used and the errors it met.",
    "Not the names of your videos or files, your folders, your servers' addresses \
     or any password: the report has placeholders such as <video 1> in their place.",
];

const GOES: [&str; 2] = [
    "Only to FramePlayerX's developers, as a private issue on GitHub that only they can read.",
    "No account, name or anything else that identifies you goes with it. Your network \
     address reaches their server with the report, as with any website, but is not kept.",
];

pub fn prompt(ui: &mut egui::Ui, v: &mut View) {
    let actions = &mut *v.actions;
    let Some(c) = v.state.crash.as_mut() else {
        return;
    };
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.add_space(18.0);
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(icons::WARNING_CIRCLE)
                        .font(theme::icon(34.0))
                        .color(theme::WARN),
                );
                ui.label(
                    RichText::new("FramePlayerX stopped unexpectedly")
                        .font(theme::font(Weight::Bold, 32.0))
                        .color(theme::TEXT),
                );
            });
            ui.add_space(8.0);
            ui.label(
                RichText::new(
                    "It crashed the last time it ran. A crash report tells its developers \
                     what went wrong so that they can fix it. Sending one is up to you: \
                     nothing is sent unless you choose Send report.",
                )
                .font(theme::font(Weight::Regular, 18.0))
                .color(theme::TEXT_2),
            );
            ui.add_space(16.0);
            list(ui, "What a report contains", &CONTAINS);
            list(ui, "Where it goes", &GOES);
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 10.0;
                if c.sending {
                    ui.spinner();
                    ui.label(RichText::new("Sending…").color(theme::TEXT_2));
                    return;
                }
                if widgets::button(ui, Some(icons::CHECK), "Send report", Kind::Primary).clicked() {
                    actions.push(Action::SendCrashReport);
                }
                let label = if c.shown {
                    "Hide the report"
                } else {
                    "Show the report"
                };
                if widgets::button(ui, Some(icons::EYE), label, Kind::Secondary).clicked() {
                    c.shown = !c.shown;
                }
                if widgets::button(ui, None, "Don't send", Kind::Ghost).clicked() {
                    actions.push(Action::DismissCrashReport);
                }
            });
            if let Some(e) = &c.error {
                ui.add_space(6.0);
                ui.label(
                    RichText::new(format!(
                        "It couldn't be sent: {e}. Try again, or don't send it."
                    ))
                    .color(theme::ERROR),
                );
            }
            if c.shown {
                ui.add_space(14.0);
                widgets::section_label(ui, "The report, exactly as it would be sent");
                egui::Frame::new()
                    .fill(theme::SURFACE_2)
                    .corner_radius(12)
                    .inner_margin(egui::Margin::same(16))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.label(
                            RichText::new(&c.report.text)
                                .font(egui::FontId::monospace(13.0))
                                .color(theme::TEXT_2),
                        );
                    });
            }
            ui.add_space(24.0);
        });
}

fn list(ui: &mut egui::Ui, title: &str, items: &[&str]) {
    widgets::section_label(ui, title);
    for item in items {
        ui.horizontal_top(|ui| {
            ui.label(
                RichText::new("•")
                    .font(theme::font(Weight::Bold, 18.0))
                    .color(theme::ACCENT),
            );
            ui.add(
                egui::Label::new(
                    RichText::new(*item)
                        .font(theme::font(Weight::Regular, 17.0))
                        .color(theme::TEXT),
                )
                .wrap(),
            );
        });
    }
    ui.add_space(12.0);
}
