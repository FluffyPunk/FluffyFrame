//! Market (original: `src/views/MarketView.svelte`, `src/components/market/*`).

use egui::{Align, FontId, Layout, Margin, RichText, Stroke, Ui, vec2};
use fluffyframe_core::services::wfm_orders::OrderType;
use fluffyframe_core::services::wfm_presence as rivens_presence;
use fluffyframe_core::services::wfm_riven_search as rivens_wfm;
use fluffyframe_core::services::wfm_stats_price::price_key;

use crate::components::{ui as kit, widgets};
use crate::ipc::{Backend, MarketCmd, PresenceCmd, PriceCmd};
use crate::stores::{self, AppState};
use crate::theme::{self, *};
use crate::i18n::{tr, trf, trn};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum Tab {
    #[default]
    Sell,
    Buy,
    Contracts,
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
            kit::page_title(ui, tr("Market"), None);
            if let Some(s) = &state.session {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if kit::ghost_button(ui, tr("Sign out")).clicked() {
                        backend.market(MarketCmd::SignOut);
                    }
                    if kit::ghost_button(ui, tr("⟳ Refresh")).clicked() {
                        backend.market(MarketCmd::MyOrders);
                        backend.market(MarketCmd::MyAuctions);
                    }
                    let mut auto = stores::settings(backend).wfm_auto_status;
                    if ui
                        .checkbox(&mut auto, tr("In game while Warframe runs"))
                        .on_hover_text(tr("In game once you log in to Warframe, your status back when it closes, and Invisible when FluffyFrame quits"))
                        .changed()
                    {
                        stores::update_settings(backend, |s| s.wfm_auto_status = auto);
                    }
                    status_picker(ui, state.wfm_status, backend);
                    ui.label(RichText::new(trf("Signed in as {}", &[&s.ingame_name.as_deref().unwrap_or("?")])).color(TEXT_SECONDARY()));
                });
            }
        });
        let contracts = if state.session.is_some() { trf("Riven Contracts ({})", &[&state.my_auctions.len()]) } else { tr("Riven Contracts").into() };
        kit::tabs(
            ui,
            &mut self.tab,
            &[(Tab::Sell, tr(tr("Sell Orders")).into()), (Tab::Buy, tr(tr("Buy Orders")).into()), (Tab::Contracts, contracts), (Tab::Browse, tr(tr("Browse")).into())],
        );
        match self.tab {
            Tab::Browse => self.browse(ui, state, backend),
            Tab::Sell | Tab::Buy | Tab::Contracts => {
                if state.session.is_none() {
                    self.sign_in(ui, backend);
                } else {
                    if !self.requested_orders {
                        self.requested_orders = true;
                        backend.market(MarketCmd::MyOrders);
                        backend.market(MarketCmd::MyAuctions);
                    }
                    // Weapon names and images for the contracts.
                    if self.tab == Tab::Contracts && state.riven_weapons.is_none() && !state.riven_weapons_requested {
                        state.riven_weapons_requested = true;
                        backend.market(MarketCmd::RivenWeapons);
                    }
                    match self.tab {
                        Tab::Contracts => self.contracts(ui, state, backend),
                        Tab::Sell => self.orders(ui, state, backend, OrderType::Sell),
                        _ => self.orders(ui, state, backend, OrderType::Buy),
                    }
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
                    ui.label(RichText::new(tr("Warframe.market")).font(FontId::new(24.0, theme::display_bold())));
                    ui.label(RichText::new(tr("Sign in with your email & password.")).family(theme::body_strong()).size(13.0));
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing.x = 3.0;
                        ui.label(RichText::new(tr("Steam/Discord users: add a password in your")).size(13.0).color(TEXT_SECONDARY()));
                        ui.hyperlink_to(tr("WFM account settings"), "https://warframe.market/settings/account");
                        ui.label(RichText::new(tr("first.")).size(13.0).color(TEXT_SECONDARY()));
                    });
                    ui.add_space(10.0);
                    ui.label(tr("Email"));
                    ui.add(egui::TextEdit::singleline(&mut self.email).hint_text(tr("you@example.com")).desired_width(f32::INFINITY).margin(Margin::symmetric(10, 8)));
                    ui.add_space(6.0);
                    ui.label(tr("Password"));
                    let pw = ui.add(egui::TextEdit::singleline(&mut self.password).password(true).hint_text("········").desired_width(f32::INFINITY).margin(Margin::symmetric(10, 8)));
                    ui.add_space(10.0);
                    let enter = pw.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                    let clicked = ui
                        .add_sized(
                            [ui.available_width(), 36.0],
                            egui::Button::new(RichText::new(tr("Sign In")).family(theme::body_strong()).color(TEXT_ON_ACCENT())).fill(ACCENT()).stroke(Stroke::NONE),
                        )
                        .clicked();
                    if (clicked || enter) && !self.email.trim().is_empty() && !self.password.is_empty() {
                        let email = self.email.trim().to_string();
                        backend.market(MarketCmd::SignIn { email: email.clone(), password: std::mem::take(&mut self.password) });
                        stores::update_settings(backend, |s| s.wfm_email = Some(email));
                    }
                    ui.add_space(4.0);
                    ui.label(RichText::new(tr("The password goes to warframe.market once and is never stored.")).size(11.5).color(TEXT_MUTED()));
                });
            });
        });
    }

    fn orders(&mut self, ui: &mut Ui, state: &AppState, backend: &Backend, kind: OrderType) {
        let catalog = state.catalog.clone();
        let list: Vec<_> = state.my_orders.iter().filter(|o| o.order_type == kind).collect();
        if list.is_empty() {
            kit::empty(ui, tr("No orders"), tr("List items from the Inventory, or with Bulk Sell."));
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
                            let variant = o.variant().label();
                            ui.label(RichText::new(if variant.is_empty() { name } else { format!("{name}  ({variant})") }).family(theme::body_strong()).size(15.0));
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(trf("Qty {}", &[&o.quantity])).size(12.0).color(TEXT_SECONDARY()));
                                if let Some(i) = item {
                                    ui.label(RichText::new(tr("48h avg")).size(12.0).color(TEXT_MUTED()));
                                    kit::plat(ui, state.price(&i.slug, o.variant()), 12.0);
                                }
                                if !o.visible {
                                    kit::pill(ui, tr("Hidden"), WARNING());
                                }
                            });
                        });
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if kit::ghost_button(ui, tr("Remove")).clicked() {
                                backend.market(MarketCmd::DeleteOrder(o.id.clone()));
                            }
                            let vis_label = if o.visible { tr("Hide") } else { tr("Show") };
                            if kit::ghost_button(ui, vis_label).clicked() {
                                backend.market(MarketCmd::UpdateOrder { id: o.id.clone(), platinum: o.platinum, quantity: o.quantity, visible: !o.visible });
                            }
                            let price = self.edits.entry(o.id.clone()).or_insert(o.platinum);
                            if *price != o.platinum && kit::primary_button(ui, tr(" Save ")).clicked() {
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

    /// The user's open riven listings: price, visibility, remove.
    fn contracts(&mut self, ui: &mut Ui, state: &AppState, backend: &Backend) {
        if state.my_auctions.is_empty() {
            kit::empty(ui, tr("No riven contracts"), tr("List a riven from its detail window in Rivens."));
            return;
        }
        let labels: std::collections::HashMap<&str, String> = rivens_wfm::stat_options().into_iter().collect();
        egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
            for a in &state.my_auctions {
                let weapon = state.riven_weapons.as_ref().and_then(|w| w.weapons.iter().find(|x| x.slug == a.weapon_slug));
                let image = weapon.and_then(|w| w.game_ref.as_deref()).and_then(|g| state.db.as_ref()?.image_url(g));
                kit::card(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        kit::remote_image(ui, image.as_deref(), vec2(40.0, 40.0));
                        ui.vertical(|ui| {
                            let weapon_name = weapon.map_or(a.weapon_slug.as_str(), |w| w.name.as_str());
                            ui.label(RichText::new(format!("{weapon_name} {}", capitalize(&a.name))).family(theme::body_strong()).size(15.0));
                            ui.horizontal_wrapped(|ui| {
                                ui.spacing_mut().item_spacing.x = 10.0;
                                for (attr, value, positive) in &a.attributes {
                                    let label = labels.get(attr.as_str()).cloned().unwrap_or_else(|| attr.replace('_', " "));
                                    let color = if *positive { POSITIVE() } else { NEGATIVE() };
                                    ui.label(RichText::new(format!("{} {label}", attribute_value(attr, *value))).size(12.5).color(color));
                                }
                            });
                            ui.horizontal(|ui| {
                                if a.direct_sell {
                                    kit::pill(ui, tr("Direct sale"), INFO());
                                } else {
                                    kit::pill(ui, tr("Auction"), ACCENT());
                                    let bid = a.top_bid.map_or(tr("no bids").to_string(), |b| trf("top bid {} p", &[&b]));
                                    ui.label(RichText::new(bid).size(12.0).color(TEXT_SECONDARY()));
                                    if let Some(b) = a.buyout.filter(|_| !a.direct_sell) {
                                        ui.label(RichText::new(trf("buyout {} p", &[&b])).size(12.0).color(TEXT_SECONDARY()));
                                    }
                                }
                                let meta = trn("Rank {} · MR {} · {} reroll", "Rank {} · MR {} · {} rerolls", (a.rerolls) as u64, &[&a.mod_rank, &a.mastery, &a.rerolls]);
                                ui.label(RichText::new(meta).size(12.0).color(TEXT_MUTED()));
                                if !a.visible {
                                    kit::pill(ui, tr("Hidden"), WARNING());
                                }
                                if a.private {
                                    kit::pill(ui, tr("Private"), WARNING());
                                }
                            });
                        });
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if kit::ghost_button(ui, tr("Remove")).clicked() {
                                backend.market(MarketCmd::CloseAuction(a.id.clone()));
                            }
                            let current = a.price().unwrap_or(0);
                            let vis_label = if a.visible { tr("Hide") } else { tr("Show") };
                            if kit::ghost_button(ui, vis_label).clicked() {
                                backend.market(MarketCmd::UpdateAuction { auction: Box::new(a.clone()), price: current, visible: !a.visible });
                            }
                            if kit::ghost_button(ui, tr("Open")).on_hover_text(a.url()).clicked() {
                                ui.ctx().open_url(egui::OpenUrl::new_tab(a.url()));
                            }
                            // A direct sale's price, an auction's starting bid.
                            let base = if a.direct_sell { current } else { a.starting.unwrap_or(current) };
                            let price = self.edits.entry(a.id.clone()).or_insert(base);
                            if *price != base && kit::primary_button(ui, tr(" Save ")).clicked() {
                                backend.market(MarketCmd::UpdateAuction { auction: Box::new(a.clone()), price: *price, visible: a.visible });
                            }
                            ui.add(egui::DragValue::new(price).range(1..=100_000).suffix(" p"))
                                .on_hover_text(if a.direct_sell { tr("Price") } else { tr("Starting bid") });
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
        kit::search(ui, &mut self.search, tr("Look up any item..."), 360.0);
        if self.search.trim().len() >= 2 {
            ui.horizontal_wrapped(|ui| {
                for item in catalog.search(&self.search, 14) {
                    if kit::chip(ui, self.looked_up.as_deref() == Some(&item.slug), &item.name).clicked() {
                        let variant = item.default_variant();
                        self.looked_up = Some(item.slug.clone());
                        backend.market(MarketCmd::TopOrders { slug: item.slug.clone(), variant });
                        backend.prices(PriceCmd::Request(vec![(item.slug.clone(), variant)]));
                    }
                }
            });
        }
        ui.add_space(8.0);
        let Some(item) = self.looked_up.as_ref().and_then(|s| catalog.by_slug(s)) else {
            kit::empty(ui, tr("Item lookup"), tr("Search the warframe.market catalog to see live orders."));
            return;
        };
        let rank = item.default_variant();
        kit::card(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                egui::Frame::new().fill(BG_WELL()).corner_radius(RADIUS_MD()).inner_margin(Margin::same(6)).show(ui, |ui| {
                    kit::remote_image(ui, item.thumb_url().as_deref(), vec2(64.0, 64.0));
                });
                ui.vertical(|ui| {
                    ui.label(RichText::new(&item.name).font(FontId::new(24.0, theme::display_bold())));
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(tr("48h average")).color(TEXT_MUTED()));
                        kit::plat(ui, state.price(&item.slug, rank), 14.0);
                        if item.ducats.is_some() {
                            kit::ducats(ui, item.ducats, 14.0);
                        }
                        widgets::open_link(ui, tr("Open on warframe.market"), &item.market_url());
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
                        for (ui, (title, list, color)) in cols.iter_mut().zip([(tr("Sellers"), &orders.sell, POSITIVE()), (tr("Buyers"), &orders.buy, INFO())]) {
                            kit::caption(ui, title);
                            for o in list.iter().take(10) {
                                kit::raised(ui, |ui| {
                                    ui.set_width(ui.available_width());
                                    ui.horizontal(|ui| {
                                        ui.label(RichText::new(o.price_text()).family(theme::body_strong()).color(color))
                                            .on_hover_text(if o.per_trade > 1 { format!("{}p per {} ({:.2}p each)", o.platinum, o.per_trade, o.unit_price()) } else { String::new() });
                                        ui.label(RichText::new(format!("×{}", o.quantity)).color(TEXT_MUTED()));
                                        ui.label(o.user_name.as_deref().unwrap_or("?"));
                                        ui.label(RichText::new(o.user_status.as_deref().unwrap_or("")).size(11.5).color(TEXT_MUTED()));
                                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                            if let Some(w) = o.whisper(&item.name)
                                                && ui.small_button(tr("Copy /w")).on_hover_text(&w).clicked()
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

/// The warframe.market status, as a dropdown that sets it.
fn status_picker(ui: &mut Ui, current: Option<rivens_presence::WfmStatus>, backend: &Backend) {
    use crate::components::status_bar::wfm_status_color;
    let text = |s: Option<rivens_presence::WfmStatus>| {
        RichText::new(format!("● {}", s.map_or(tr("Status…"), |s| tr(s.label())))).color(wfm_status_color(s))
    };
    egui::ComboBox::from_id_salt("wfm-status").width(110.0).selected_text(text(current)).show_ui(ui, |ui| {
        for s in rivens_presence::WfmStatus::ALL {
            if ui.selectable_label(current == Some(s), text(Some(s))).clicked() && current != Some(s) {
                backend.presence(PresenceCmd::Set(s));
            }
        }
    });
}

/// "crita-visicron" → "Crita-visicron".
fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    c.next().map(|f| f.to_uppercase().chain(c).collect()).unwrap_or_default()
}

/// A warframe.market attribute value as the card prints it: faction damage as
/// a multiplier, flat stats without "%".
fn attribute_value(attr: &str, value: f64) -> String {
    let v = format!("{value:.2}");
    let v = v.trim_end_matches('0').trim_end_matches('.');
    if attr.starts_with("damage_vs_") || attr.starts_with("damage_to_") {
        return format!("x{v}");
    }
    let sign = if value >= 0.0 { "+" } else { "" };
    let unit = if matches!(attr, "punch_through" | "range" | "combo_duration") { "" } else { "%" };
    format!("{sign}{v}{unit}")
}
