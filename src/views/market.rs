//! Market (original: `src/views/MarketView.svelte`, `src/components/market/*`).

use egui::{Align, FontId, Layout, Margin, RichText, Stroke, Ui, vec2};
use wfhelper_core::services::wfm_orders::OrderType;
use wfhelper_core::services::wfm_stats_price::price_key;

use crate::components::{ui as kit, widgets};
use crate::ipc::{Backend, MarketCmd, PriceCmd};
use crate::stores::{self, AppState};
use crate::theme::{self, *};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum Tab {
    #[default]
    Sell,
    Buy,
    Browse,
}

#[derive(Default)]
pub struct MarketView {
    tab: Tab,
    email: String,
    password: String,
    search: String,
    looked_up: Option<String>,
    edits: std::collections::HashMap<String, u32>,
    requested_orders: bool,
}

impl MarketView {
    pub fn ui(&mut self, ui: &mut Ui, state: &mut AppState, backend: &Backend) {
        ui.horizontal(|ui| {
            kit::page_title(ui, "Market", None);
            if let Some(s) = &state.session {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if kit::ghost_button(ui, "Sign out").clicked() {
                        backend.market(MarketCmd::SignOut);
                    }
                    if kit::ghost_button(ui, "⟳ Refresh").clicked() {
                        backend.market(MarketCmd::MyOrders);
                    }
                    ui.label(RichText::new(format!("Signed in as {}", s.ingame_name.as_deref().unwrap_or("?"))).color(TEXT_SECONDARY()));
                });
            }
        });
        kit::tabs(ui, &mut self.tab, &[(Tab::Sell, "Sell Orders".into()), (Tab::Buy, "Buy Orders".into()), (Tab::Browse, "Browse".into())]);
        match self.tab {
            Tab::Browse => self.browse(ui, state, backend),
            Tab::Sell | Tab::Buy => {
                if state.session.is_none() {
                    self.sign_in(ui, backend);
                } else {
                    if !self.requested_orders {
                        self.requested_orders = true;
                        backend.market(MarketCmd::MyOrders);
                    }
                    let kind = if self.tab == Tab::Sell { OrderType::Sell } else { OrderType::Buy };
                    self.orders(ui, state, backend, kind);
                }
            }
        }
    }

    fn sign_in(&mut self, ui: &mut Ui, backend: &Backend) {
        if self.email.is_empty()
            && let Some(e) = stores::settings(backend).wfm_email
        {
            self.email = e;
        }
        ui.add_space(12.0);
        ui.vertical_centered(|ui| {
            ui.set_max_width(560.0);
            kit::card(ui, |ui| {
                ui.set_width(530.0);
                ui.with_layout(Layout::top_down(Align::Min), |ui| {
                    ui.add(egui::Image::new(crate::assets::view_icon(crate::views::ViewName::Market)).tint(ACCENT()).fit_to_exact_size(vec2(30.0, 30.0)));
                    ui.label(RichText::new("Warframe.market").font(FontId::new(24.0, theme::display_bold())));
                    ui.label(RichText::new("Sign in with your email & password.").family(theme::body_strong()).size(13.0));
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing.x = 3.0;
                        ui.label(RichText::new("Steam/Discord users: add a password in your").size(13.0).color(TEXT_SECONDARY()));
                        ui.hyperlink_to("WFM account settings", "https://warframe.market/settings/account");
                        ui.label(RichText::new("first.").size(13.0).color(TEXT_SECONDARY()));
                    });
                    ui.add_space(10.0);
                    ui.label("Email");
                    ui.add(egui::TextEdit::singleline(&mut self.email).hint_text("you@example.com").desired_width(f32::INFINITY).margin(Margin::symmetric(10, 8)));
                    ui.add_space(6.0);
                    ui.label("Password");
                    let pw = ui.add(egui::TextEdit::singleline(&mut self.password).password(true).hint_text("········").desired_width(f32::INFINITY).margin(Margin::symmetric(10, 8)));
                    ui.add_space(10.0);
                    let enter = pw.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                    let clicked = ui
                        .add_sized(
                            [ui.available_width(), 36.0],
                            egui::Button::new(RichText::new("Sign In").family(theme::body_strong()).color(TEXT_ON_ACCENT())).fill(ACCENT()).stroke(Stroke::NONE),
                        )
                        .clicked();
                    if (clicked || enter) && !self.email.trim().is_empty() && !self.password.is_empty() {
                        let email = self.email.trim().to_string();
                        backend.market(MarketCmd::SignIn { email: email.clone(), password: std::mem::take(&mut self.password) });
                        stores::update_settings(backend, |s| s.wfm_email = Some(email));
                    }
                    ui.add_space(4.0);
                    ui.label(RichText::new("The password goes to warframe.market once and is never stored.").size(11.5).color(TEXT_MUTED()));
                });
            });
        });
    }

    fn orders(&mut self, ui: &mut Ui, state: &AppState, backend: &Backend, kind: OrderType) {
        let catalog = state.catalog.clone();
        let list: Vec<_> = state.my_orders.iter().filter(|o| o.order_type == kind).collect();
        if list.is_empty() {
            kit::empty(ui, "No orders", "List items from the Inventory, or with Bulk Sell.");
            return;
        }
        egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
            for o in list {
                let item = catalog.as_ref().and_then(|c| c.items.iter().find(|i| i.id == o.item_id));
                kit::card(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        kit::remote_image(ui, item.and_then(|i| i.thumb_url()).as_deref(), vec2(40.0, 40.0));
                        ui.vertical(|ui| {
                            let name = item.map(|i| i.name.clone()).unwrap_or_else(|| o.item_id.clone());
                            ui.label(RichText::new(match o.rank {
                                Some(r) => format!("{name}  (R{r})"),
                                None => name,
                            }).family(theme::body_strong()).size(15.0));
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(format!("Qty {}", o.quantity)).size(12.0).color(TEXT_SECONDARY()));
                                if let Some(i) = item {
                                    ui.label(RichText::new("48h avg").size(12.0).color(TEXT_MUTED()));
                                    kit::plat(ui, state.price(&i.slug, o.rank), 12.0);
                                }
                                if !o.visible {
                                    kit::pill(ui, "Hidden", WARNING());
                                }
                            });
                        });
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if kit::ghost_button(ui, "Remove").clicked() {
                                backend.market(MarketCmd::DeleteOrder(o.id.clone()));
                            }
                            let vis_label = if o.visible { "Hide" } else { "Show" };
                            if kit::ghost_button(ui, vis_label).clicked() {
                                backend.market(MarketCmd::UpdateOrder { id: o.id.clone(), platinum: o.platinum, quantity: o.quantity, visible: !o.visible });
                            }
                            let price = self.edits.entry(o.id.clone()).or_insert(o.platinum);
                            if *price != o.platinum && kit::primary_button(ui, " Save ").clicked() {
                                backend.market(MarketCmd::UpdateOrder { id: o.id.clone(), platinum: *price, quantity: o.quantity, visible: o.visible });
                            }
                            ui.add(egui::DragValue::new(price).range(1..=100_000).suffix(" p"));
                        });
                    });
                });
                ui.add_space(6.0);
            }
        });
    }

    fn browse(&mut self, ui: &mut Ui, state: &AppState, backend: &Backend) {
        let Some(catalog) = state.catalog.clone() else {
            ui.spinner();
            return;
        };
        kit::search(ui, &mut self.search, "Look up any item...", 360.0);
        if self.search.trim().len() >= 2 {
            ui.horizontal_wrapped(|ui| {
                for item in catalog.search(&self.search, 14) {
                    if kit::chip(ui, self.looked_up.as_deref() == Some(&item.slug), &item.name).clicked() {
                        let rank = item.max_rank.map(|_| 0);
                        self.looked_up = Some(item.slug.clone());
                        backend.market(MarketCmd::TopOrders { slug: item.slug.clone(), rank });
                        backend.prices(PriceCmd::Request(vec![(item.slug.clone(), rank)]));
                    }
                }
            });
        }
        ui.add_space(8.0);
        let Some(item) = self.looked_up.as_ref().and_then(|s| catalog.by_slug(s)) else {
            kit::empty(ui, "Item lookup", "Search the warframe.market catalog to see live orders.");
            return;
        };
        let rank = item.max_rank.map(|_| 0);
        kit::card(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                egui::Frame::new().fill(BG_WELL()).corner_radius(RADIUS_MD()).inner_margin(Margin::same(6)).show(ui, |ui| {
                    kit::remote_image(ui, item.thumb_url().as_deref(), vec2(64.0, 64.0));
                });
                ui.vertical(|ui| {
                    ui.label(RichText::new(&item.name).font(FontId::new(24.0, theme::display_bold())));
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("48h average").color(TEXT_MUTED()));
                        kit::plat(ui, state.price(&item.slug, rank), 14.0);
                        if item.ducats.is_some() {
                            kit::ducats(ui, item.ducats, 14.0);
                        }
                        widgets::open_link(ui, "Open on warframe.market", &item.market_url());
                    });
                });
            });
            ui.add_space(8.0);
            match state.top_orders.get(&price_key(&item.slug, rank)) {
                None => {
                    ui.spinner();
                }
                Some(orders) => {
                    ui.columns(2, |cols| {
                        for (ui, (title, list, color)) in cols.iter_mut().zip([("Sellers", &orders.sell, POSITIVE()), ("Buyers", &orders.buy, INFO())]) {
                            kit::caption(ui, title);
                            for o in list.iter().take(10) {
                                kit::raised(ui, |ui| {
                                    ui.set_width(ui.available_width());
                                    ui.horizontal(|ui| {
                                        ui.label(RichText::new(format!("{}p", o.platinum)).family(theme::body_strong()).color(color));
                                        ui.label(RichText::new(format!("×{}", o.quantity)).color(TEXT_MUTED()));
                                        ui.label(o.user_name.as_deref().unwrap_or("?"));
                                        ui.label(RichText::new(o.user_status.as_deref().unwrap_or("")).size(11.5).color(TEXT_MUTED()));
                                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                            if let Some(w) = o.whisper(&item.name)
                                                && ui.small_button("Copy /w").on_hover_text(&w).clicked()
                                            {
                                                ui.ctx().copy_text(w);
                                            }
                                        });
                                    });
                                });
                                ui.add_space(3.0);
                            }
                        }
                    });
                }
            }
        });
    }
}
