// --- main.rs
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, channel};

use eframe::{App, egui};
use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use rfd::FileDialog;
use serde::Deserialize;
use std::collections::HashMap;
use walkdir::WalkDir;

#[allow(dead_code)]
#[derive(Debug, Deserialize, Clone)]
struct InterfaceParam {
    name: String,
    #[serde(rename = "type")]
    ty: String,
    default: Option<toml::Value>,
}

#[derive(Debug, Deserialize, Clone)]
struct TemplateDef {
    #[serde(default)]
    interface: Vec<InterfaceParam>,
    widget_type: String,
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    properties: toml::value::Table,
    #[serde(default)]
    children: Vec<WidgetDef>,
}

#[derive(Debug, Deserialize, Clone, Default)]
struct WidgetDef {
    widget_type: String,
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    properties: toml::value::Table,
    #[serde(default)]
    children: Vec<WidgetDef>,
}

#[derive(Default)]
struct TomlUiApp {
    watch_dir: Option<PathBuf>,
    reload_rx: Option<Receiver<Event>>,
    templates: HashMap<String, (WidgetDef, Vec<String>)>,
    roots: Vec<WidgetDef>,
}

impl App for TomlUiApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Toolbar
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            if ui.button("Select TOML Folder").clicked() {
                if let Some(dir) = FileDialog::new().pick_folder() {
                    self.watch_dir = Some(dir.clone());
                    let (tx, rx) = channel::<Event>();
                    let mut watcher: RecommendedWatcher = RecommendedWatcher::new(
                        move |res| {
                            if let Ok(event) = res {
                                let _ = tx.send(event);
                            }
                        },
                        Config::default(),
                    )
                    .unwrap();
                    watcher.watch(&dir, RecursiveMode::Recursive).unwrap();
                    self.reload_rx = Some(rx);
                    load_and_prepare(&dir, &mut self.templates, &mut self.roots);
                }
            }
        });

        // File events
        if let Some(rx) = &self.reload_rx {
            if let Ok(event) = rx.try_recv() {
                if let Some(dir) = &self.watch_dir {
                    if matches!(event.kind, EventKind::Modify(_)) {
                        load_and_prepare(dir, &mut self.templates, &mut self.roots);
                        ctx.request_repaint();
                    }
                }
            }
        }

        // Tree view
        egui::SidePanel::left("tree_panel").show(ctx, |ui| {
            ui.heading("Widget Trees");
            for (i, def) in self.roots.iter().enumerate() {
                let label = def.id.clone().unwrap_or_else(|| format!("Root {}", i));
                egui::CollapsingHeader::new(label)
                    .default_open(true)
                    .show(ui, |ui| {
                        show_tree(ui, def);
                    });
            }
        });

        // Preview
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Preview");
            for def in &self.roots {
                render_preview(ui, def);
            }
        });
    }
}

fn load_and_prepare(
    dir: &PathBuf,
    templates: &mut HashMap<String, (WidgetDef, Vec<String>)>,
    roots: &mut Vec<WidgetDef>,
) {
    println!("[DEBUG] Scanning directory: {:?}", dir);
    templates.clear();
    roots.clear();

    for entry in WalkDir::new(dir).into_iter().filter_map(Result::ok) {
        if entry.path().extension().and_then(|s| s.to_str()) == Some("toml") {
            println!("[DEBUG] Found TOML file: {:?}", entry.path());
            let txt = match std::fs::read_to_string(entry.path()) {
                Ok(s) => s,
                Err(e) => {
                    println!("[DEBUG] Read error {:?}", e);
                    continue;
                }
            };

            let stem = entry
                .path()
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("");

            // Try template first
            if let Ok(tpl) = toml::from_str::<TemplateDef>(&txt) {
                println!("[DEBUG] Registered template: {}", stem);
                let params = tpl.interface.iter().map(|p| p.name.clone()).collect();
                let base = WidgetDef {
                    widget_type: tpl.widget_type,
                    id: tpl.id,
                    properties: tpl.properties,
                    children: tpl.children,
                };
                templates.insert(stem.to_string(), (base, params));
            } else if let Ok(def) = toml::from_str::<WidgetDef>(&txt) {
                println!(
                    "[DEBUG] Registered root: {} (id={:?})",
                    def.widget_type, def.id
                );
                roots.push(def);
            }
        }
    }

    println!("[DEBUG] Total templates: {}", templates.len());
    println!("[DEBUG] Total roots: {}", roots.len());

    for def in roots.iter_mut() {
        expand_templates(def, templates);
    }
}

fn expand_templates(def: &mut WidgetDef, templates: &HashMap<String, (WidgetDef, Vec<String>)>) {
    if let Some((tpl, allowed)) = templates.get(&def.widget_type) {
        let props = std::mem::take(&mut def.properties);
        let filtered: toml::value::Table = props
            .into_iter()
            .filter(|(k, _)| allowed.contains(k))
            .collect();
        let mut expanded = tpl.clone();
        for (k, v) in filtered {
            expanded.properties.insert(k, v);
        }
        expanded.children.extend(def.children.drain(..));
        *def = expanded;
    }
    for child in def.children.iter_mut() {
        expand_templates(child, templates);
    }
}

fn show_tree(ui: &mut egui::Ui, def: &WidgetDef) {
    let label = def.id.as_deref().unwrap_or(&def.widget_type);
    egui::CollapsingHeader::new(label).show(ui, |ui| {
        for child in &def.children {
            show_tree(ui, child);
        }
    });
}

fn render_preview(ui: &mut egui::Ui, def: &WidgetDef) {
    match def.widget_type.as_str() {
        "Button" => {
            let text = def
                .properties
                .get("text")
                .and_then(|v| v.as_str())
                .unwrap_or("Button");
            let _ = ui.button(text);
        }
        "Label" => {
            let text = def
                .properties
                .get("text")
                .and_then(|v| v.as_str())
                .unwrap_or("Label");
            ui.label(text);
        }
        "Vertical" => {
            ui.vertical(|ui| {
                for c in &def.children {
                    render_preview(ui, c);
                }
            });
        }
        "Frame" => {
            egui::Frame::group(ui.style()).show(ui, |ui| {
                for c in &def.children {
                    render_preview(ui, c);
                }
            });
        }
        "Separator" => {
            ui.separator();
        }
        other => {
            ui.label(format!("Unknown widget: {}", other));
        }
    }
}

fn main() {
    let app = TomlUiApp::default();
    let native_options = eframe::NativeOptions::default();

    let _ = eframe::run_native(
        "TOML egui Builder",
        native_options,
        Box::new(|_cc| Ok(Box::new(app))),
    );
}
