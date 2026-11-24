//! Toast stack (original: `ToastHost.svelte`).

use std::time::Duration;

use egui::{Align2, Context, RichText};

use crate::ipc::ToastKind;
use crate::stores::AppState;
use crate::theme;

const SHOW_FOR: Duration = Duration::from_secs(6);

pub fn show(ctx: &Context, state: &mut AppState) {
    state.toasts.retain(|t| t.at.elapsed() < SHOW_FOR);
    if state.toasts.is_empty() {
        return;
    }
    egui::Area::new(egui::Id::unique("wfhelper-toasts"))
        .anchor(Align2::RIGHT_BOTTOM, [-16.0, -36.0])
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            let mut dismiss = None;
            for (i, t) in state.toasts.iter().enumerate() {
                let color = match t.kind {
                    ToastKind::Info => theme::INFO(),
                    ToastKind::Success => theme::POSITIVE(),
                    ToastKind::Error => theme::NEGATIVE(),
                };
                egui::Frame::popup(ui.style()).stroke((1.0, color)).show(ui, |ui| {
                    ui.set_max_width(360.0);
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("●").color(color));
                        ui.label(&t.text);
                        if ui.small_button("✕").clicked() {
                            dismiss = Some(i);
                        }
                    });
                });
                ui.add_space(4.0);
            }
            if let Some(i) = dismiss {
                state.toasts.remove(i);
            }
        });
    ctx.request_repaint_after(Duration::from_millis(500));
}
