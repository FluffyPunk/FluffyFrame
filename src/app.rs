//! App shell (original: `src/App.svelte`, `src/components/layout/*`).

use eframe::Frame;
use egui::{Margin, Ui};

use crate::components::{sidebar, status_bar, toasts};
use crate::ipc::Backend;
use crate::stores::{self, AppState};
use crate::theme;
use crate::views::{ViewName, Views};

pub struct WfHelperApp {
    backend: Backend,
    state: AppState,
    views: Views,
    current: ViewName,
    sidebar_collapsed: bool,
    item_modal: crate::components::item_modal::ItemModal,
}

impl WfHelperApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        egui_extras::install_image_loaders(&cc.egui_ctx);
        crate::image_loader::install(&cc.egui_ctx);
        crate::image_loader::install_thumbnails(&cc.egui_ctx);
        theme::install(&cc.egui_ctx);
        let settings = wfhelper_core::config::settings::Settings::load();
        crate::stores::apply_theme(&cc.egui_ctx, &settings);
        // WFHELPER_VIEW opens a specific view (handy for screenshots and debugging).
        let forced = std::env::var("WFHELPER_VIEW").ok().and_then(|v| ViewName::from_id(&v));
        let current = if let Some(v) = forced.filter(|_| settings.setup_complete) {
            v
        } else if settings.setup_complete {
            ViewName::from_id(&settings.landing_view).unwrap_or(ViewName::Inventory)
        } else {
            ViewName::Setup
        };
        let backend = Backend::start(cc.egui_ctx.clone(), settings);
        Self { backend, state: AppState::new(), views: Views::default(), current, sidebar_collapsed: false, item_modal: Default::default() }
    }
}

impl eframe::App for WfHelperApp {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut Frame) {
        crate::image_loader::release_evicted(ctx);
        while let Ok(event) = self.backend.events.try_recv() {
            self.state.apply(event, &self.backend);
        }
    }

    fn ui(&mut self, ui: &mut Ui, _frame: &mut Frame) {
        let settings = stores::settings(&self.backend);
        if self.current == ViewName::Setup && settings.setup_complete {
            self.current = ViewName::from_id(&settings.landing_view).unwrap_or(ViewName::Inventory);
        }
        if self.current != ViewName::Setup && !settings.setup_complete {
            self.current = ViewName::Setup;
        }

        let strip = egui::Frame::new().fill(theme::BG_DEEP()).inner_margin(Margin::symmetric(8, 4));
        egui::Panel::top("title").exact_size(30.0).frame(strip).show(ui, |ui| {
            status_bar::title_strip(ui, &self.state);
        });
        egui::Panel::bottom("status").exact_size(26.0).frame(strip).show(ui, |ui| {
            status_bar::show(ui, &self.state, &self.backend);
        });
        if self.current != ViewName::Setup {
            let width = if self.sidebar_collapsed { sidebar::COLLAPSED_WIDTH } else { sidebar::WIDTH };
            egui::Panel::left("sidebar")
                .resizable(false)
                .exact_size(width)
                .frame(egui::Frame::new().fill(theme::BG_SIDEBAR()).inner_margin(Margin::symmetric(8, 0)))
                .show(ui, |ui| {
                    sidebar::show(ui, &mut self.current, &settings.hidden_views, &mut self.sidebar_collapsed);
                });
        }
        let content = egui::Frame::new().fill(theme::BG_BASE()).inner_margin(Margin { left: 28, right: 20, top: 18, bottom: 8 });
        egui::CentralPanel::default().frame(content).show(ui, |ui| {
            if let Some(next) = self.views.show(self.current, ui, &mut self.state, &self.backend) {
                self.current = next;
            }
        });
        self.item_modal.show(ui.ctx(), &mut self.state, &self.backend);
        toasts::show(ui.ctx(), &mut self.state);
    }
}
