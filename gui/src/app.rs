use eframe::egui;
use std::{collections::HashMap, sync::mpsc::Receiver};

use crate::discovery::{self, BoardInfo};
use crate::link;
use crate::session::Session;

pub struct TractUi {
    boards: Vec<BoardInfo>,
    discovery: Receiver<Vec<BoardInfo>>,
    selected: Option<String>,
    sessions: HashMap<String, Session>,
}

impl TractUi {
    pub fn new(cc: &eframe::CreationContext) -> Self {
        Self {
            boards: Vec::new(),
            discovery: discovery::spawn(cc.egui_ctx.clone()),
            selected: None,
            sessions: HashMap::new(),
        }
    }
}

impl eframe::App for TractUi {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        if let Some(boards) = self.discovery.try_iter().last() {
            self.boards = boards;
        }

        for s in self.sessions.values_mut() {
            s.poll();
        }
        self.sessions.retain(|key, s| match s.closed() {
            Some(reason) => {
                eprintln!("{key}: disconnected ({reason})");
                false
            }
            None => true,
        });

        egui::Panel::top("menu").show(ui, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                ui.menu_button("File", |ui| {
                    egui::widgets::global_theme_preference_buttons(ui);
                    ui.separator();
                    if ui.button("Quit").clicked() {
                        ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                })
            });
        });

        egui::Panel::left("boards").show(ui, |ui| {
            ui.heading("TractCtrl");
            ui.separator();
            if self.boards.is_empty() {
                ui.label("No TractionHub found...");
            }
            for b in &self.boards {
                let key = b.key();
                let is_selected = self.selected.as_ref() == Some(&key);
                let connected = self.sessions.contains_key(&key);

                let text = format!("{} {}", if connected { "🟢" } else { "🔴" }, key);
                if ui.selectable_label(is_selected, text).clicked() {
                    self.selected = Some(key);
                }
                ui.small(&b.port);
                ui.separator();
            }
        });

        egui::CentralPanel::default().show(ui, |ui| {
            let Some(key) = self.selected.clone() else {
                ui.label("Select a board on the left");
                return;
            };

            match self.sessions.get_mut(&key) {
                Some(session) => session.ui(ui),
                None => {
                    ui.heading(&key);
                    if ui.button("Connect").clicked() {
                        if let Some(b) = self.boards.iter().find(|b| b.key() == key) {
                            let link = link::spawn(b.port.clone(), ui.ctx().clone());
                            self.sessions.insert(key, Session::new(link));
                        }
                    }
                }
            }
        });
    }
}
