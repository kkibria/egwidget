mod builder;
mod parser;

use std::path::PathBuf;
use std::sync::mpsc::{Receiver, channel};

use eframe::{App, egui};
use notify::{Config, Event, EventKind, RecursiveMode, Watcher};
use rfd::FileDialog;
use serde::Deserialize;
use std::collections::HashMap;

use notify::{Error, PollWatcher};
use std::time::Duration;

use builder::{WidgetDef, build_widget_tree};
use parser::{Template, parse_template, print_parse_error};

#[allow(dead_code)]
#[derive(Debug, Deserialize, Clone)]
struct InterfaceParam {
    name: String,
    #[serde(rename = "type")]
    ty: String,
    default: Option<toml::Value>,
}

#[allow(dead_code)]
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

// #[derive(Debug, Deserialize, Clone, Default)]
// struct WidgetDef {
//     widget_type: String,
//     #[serde(default)]
//     id: Option<String>,
//     #[serde(default)]
//     properties: toml::value::Table,
//     #[serde(default)]
//     children: Vec<WidgetDef>,
// }

struct TomlUiApp {
    watch_path: Option<PathBuf>,
    watcher: Option<PollWatcher>,
    watch_rx: Option<Receiver<Event>>,
    reload_rx: Option<Receiver<Event>>,
    templates: HashMap<String, Template>,
    roots: Vec<WidgetDef>,
}

impl TomlUiApp {
    pub fn new() -> Self {
        Self {
            templates: HashMap::new(),
            roots: Vec::new(),
            watch_path: None,
            watcher: None,
            watch_rx: None,
            reload_rx: None,
        }
    }

    /// Call when the user picks a folder.
    fn set_watch_path<F>(&mut self, path: PathBuf, mut on_event: F)
    where
        F: FnMut() + Send + 'static,
    {
        // remember the path
        self.watch_path = Some(path.clone());
        let (tx, rx) = channel();

        // Poll every 500ms (tweak as you like)
        let mut w: PollWatcher = PollWatcher::new(
            move |res: Result<Event, Error>| match res {
                Ok(evt) => {
                    println!("[DEBUG] watcher callback event: {:?}", evt.kind);
                    let _ = tx.send(evt);
                    on_event();
                }
                Err(err) => {
                    eprintln!("[DEBUG] watcher error: {:?}", err);
                }
            },
            Config::default().with_poll_interval(Duration::from_millis(500)),
        )
        .expect("failed to init PollWatcher");
        w.watch(&path, RecursiveMode::Recursive)
            .expect("failed to watch toml folder");

        self.watcher = Some(w);
        self.watch_rx = Some(rx);

        // initial load
        load_and_prepare(&path, &mut self.templates, &mut self.roots);
    }
}

impl App for TomlUiApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.watch_path.is_none() {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.heading("Widget Trees");
                ui.vertical_centered(|ui| {
                    ui.heading("No TOML folder selected");
                    if ui.button("Select folder…").clicked() {
                        // Using `rfd` crate for a native folder dialog:
                        if let Some(folder) = FileDialog::new()
                            .set_title("Choose your TOML folder")
                            .pick_folder()
                        {
                            let repaint = ctx.clone();
                            self.set_watch_path(folder, move || repaint.request_repaint());
                        }
                    }
                });
            });
            return; // skip the rest until a folder is chosen
        }

        // File events
        if let Some(rx) = &self.reload_rx {
            if let Ok(event) = rx.try_recv() {
                if let Some(dir) = &self.watch_path {
                    if matches!(event.kind, EventKind::Modify(_)) {
                        load_and_prepare(dir, &mut self.templates, &mut self.roots);
                        ctx.request_repaint();
                    }
                }
            }
        }

        // 1) Drain all pending events:
        let mut changed = false;
        if let Some(rx) = &self.watch_rx {
            while let Ok(evt) = rx.try_recv() {
                println!("[DEBUG] update loop got event: {:?}", evt.kind);
                changed = true;
            }
        }

        // 2) If anything arrived, reload:
        if changed {
            if let Some(path) = &self.watch_path {
                println!("[DEBUG] Detected file change, reloading TOMLs…");
                load_and_prepare(path, &mut self.templates, &mut self.roots);
                println!(
                    "[DEBUG] Reload complete: {} templates, {} roots",
                    self.templates.len(),
                    self.roots.len()
                );
            }
        }

        egui::SidePanel::left("tree").show(ctx, |ui| {
            let mut id_path = Vec::new();
            for (i, root) in self.roots.iter().enumerate() {
                id_path.clear();
                id_path.push(i);
                show_tree(ui, root, &mut id_path);
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
    templates: &mut HashMap<String, Template>,
    roots: &mut Vec<WidgetDef>,
) {
    println!("[DEBUG] Scanning directory: {:?}", dir);
    templates.clear();
    roots.clear();

    for entry in std::fs::read_dir(&dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().and_then(|e| e.to_str()) != Some("wui") {
            continue;
        }
        let stem = path.file_stem().unwrap().to_string_lossy().to_string();
        let text = std::fs::read_to_string(&path).unwrap();

        // Try to parse as a template
        match parse_template(&text, &stem) {
            Ok(tpl) => {
                println!("[DEBUG] Registered template: {}", tpl.name);
                templates.insert(tpl.name.clone(), tpl);
            }
            Err(err) => {
                print_parse_error(&text, err, &format!("{}.wui", stem));
                // skip this file
                continue;
            }
        }

        if let Some(root_def) = build_widget_tree(&templates, "main") {
            *roots = vec![root_def];
        }
    }
}

fn show_tree(ui: &mut egui::Ui, def: &WidgetDef, id_path: &mut Vec<usize>) {
    // 1) Compute the display label
    let label = def.id.as_deref().unwrap_or(&def.widget_type);

    // 2) Push a unique ID for this node based on its position in the tree
    //    e.g. [0,2,1] for root‐child‐grandchild path
    ui.push_id(id_path.clone(), |ui| {
        egui::CollapsingHeader::new(label)
            .default_open(true)
            .show(ui, |ui| {
                // 3) Recurse into children, pushing index as part of the path
                for (i, child) in def.children.iter().enumerate() {
                    id_path.push(i);
                    show_tree(ui, child, id_path);
                    id_path.pop();
                }
            });
    });
}

fn render_preview(ui: &mut egui::Ui, def: &WidgetDef) {
    match def.widget_type.as_str() {
        "Button" => {
            let text = def
                .args
                .get("text")
                .and_then(|v| Some(v.as_str()))
                .unwrap_or("Button");
            let _ = ui.button(text);
        }
        "Label" => {
            let text = def
                .args
                .get("text")
                .and_then(|v| Some(v.as_str()))
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
    let app = TomlUiApp::new();
    let native_options = eframe::NativeOptions::default();

    let _ = eframe::run_native(
        "wui egui Builder",
        native_options,
        Box::new(|_cc| Ok(Box::new(app))),
    );
}
