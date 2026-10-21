//! Up next (the queue, opened from the control bar), and the buttons that
//! queue a video or add it to a playlist wherever one is shown.

use super::page;
use super::widgets::{self, Kind};
use super::{Action, Picking, View, icons};
use crate::playback::OpenRequest;

/// A video in Up next as the page shows it.
pub struct Row {
    pub title: String,
    pub facts: String,
    pub thumb: Option<String>,
}

/// Play next, Add to queue and Add to playlist for `req`.
pub fn buttons(ui: &mut egui::Ui, v: &mut View, req: &OpenRequest) {
    if widgets::button(ui, Some(icons::SKIP_FORWARD), "Play next", Kind::Secondary).clicked() {
        v.actions.push(Action::Queue {
            req: req.clone(),
            next: true,
        });
    }
    if widgets::button(ui, Some(icons::QUEUE), "Add to queue", Kind::Secondary).clicked() {
        v.actions.push(Action::Queue {
            req: req.clone(),
            next: false,
        });
    }
    if widgets::button(
        ui,
        Some(icons::PLAYLIST),
        "Add to playlist",
        Kind::Secondary,
    )
    .clicked()
    {
        v.state.picking = Some(Picking {
            reqs: vec![req.clone()],
            title: req.shown_title(),
        });
    }
}

/// How a queued video shows: as the library has it, else as its source
/// listed it.
fn row(v: &View, req: &OpenRequest) -> Row {
    let location = fp_core::volume::stable(&req.location);
    if let Ok(Some(r)) = v.services.library.get_by_location(&location) {
        return Row {
            title: widgets::display_title(&r.title),
            facts: super::library::facts(&r, v.settings.default_format),
            thumb: super::library::thumb_key(&r),
        };
    }
    let e = req.entry.as_ref();
    let mut facts = Vec::new();
    if let Some(d) = e.and_then(|e| e.duration) {
        facts.push(super::fmt_time(d));
    }
    if let Some(f) = e.and_then(|e| e.format) {
        facts.push(widgets::format_short(&f));
    }
    Row {
        title: widgets::display_title(&req.shown_title()),
        facts: facts.join("  ·  "),
        thumb: e.and_then(|e| e.thumbnail_url.clone()),
    }
}

/// The Up next page; Back returns to the video.
pub fn page(ui: &mut egui::Ui, v: &mut View) {
    let q = v.queue;
    if v.state
        .up_next
        .as_ref()
        .is_none_or(|(ver, _)| *ver != q.version)
    {
        let rows = q.items.iter().map(|r| row(v, r)).collect();
        v.state.up_next = Some((q.version, rows));
    }
    let n = q.items.len();
    let subtitle = match n {
        0 => "Nothing queued: Play next and Add to queue are on a video's details".to_string(),
        _ if q.plays_through => format!("{} of {n} · plays through to the end", q.pos + 1),
        _ => format!("{} of {n} · X and Y step through the list", q.pos + 1),
    };
    let back = page::header(ui, "Up next", Some(&subtitle), |ui| {
        if n > 1
            && widgets::button_sized(ui, Some(icons::TRASH), "Clear", Kind::Secondary, 44.0)
                .clicked()
        {
            v.actions.push(Action::QueueClear);
        }
        if n > 0
            && widgets::button_sized(
                ui,
                Some(icons::PLAYLIST),
                "Save as playlist",
                Kind::Secondary,
                44.0,
            )
            .clicked()
        {
            v.state.picking = Some(Picking {
                reqs: q.items.clone(),
                title: "Up next".into(),
            });
        }
    });
    if back {
        if v.playback.is_some() {
            v.actions.push(Action::ShowBrowser(false));
        } else {
            v.state.screen = super::Screen::Home;
        }
    }
    let Some((_, rows)) = v.state.up_next.take() else {
        return;
    };
    page::scroll(ui, |ui| {
        ui.add_space(4.0);
        for (i, r) in rows.iter().enumerate() {
            let facts = if i == q.pos && v.playback.is_some() {
                format!("Now playing  ·  {}", r.facts)
            } else {
                r.facts.clone()
            };
            let resp = page::list_row(
                ui,
                v.thumbs,
                r.thumb.as_deref(),
                &r.title,
                &facts,
                i == q.pos,
                (i == 0, i + 1 == n),
            );
            if resp.open {
                v.actions.push(Action::QueueJump(i));
            } else if resp.up {
                v.actions.push(Action::QueueMove(i, i - 1));
            } else if resp.down {
                v.actions.push(Action::QueueMove(i, i + 1));
            } else if resp.remove {
                v.actions.push(Action::QueueRemove(i));
            }
            ui.add_space(4.0);
        }
    });
    v.state.up_next = Some((q.version, rows));
}
