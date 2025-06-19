// --- main copy.rs
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, channel};

use eframe::{App, egui};
use notify::{Config, Event, EventKind, RecursiveMode, Watcher};
use rfd::FileDialog;
use serde::Deserialize;
use std::collections::HashMap;
use walkdir::WalkDir;

use notify::{Error, PollWatcher};
use std::time::Duration;

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

struct TomlUiApp {
    watch_path: Option<PathBuf>,
    watcher: Option<PollWatcher>,
    watch_rx: Option<Receiver<Event>>,
    reload_rx: Option<Receiver<Event>>,
    templates: HashMap<String, (WidgetDef, Vec<String>)>,
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
    ///
    ///
    ///
    ///

    fn set_watch_path<F>(&mut self, path: PathBuf, mut on_event: F)
    where
        F: FnMut() + Send + 'static,
    {
        // fn set_watch_path(&mut self, path: PathBuf) {
        // remember the path
        self.watch_path = Some(path.clone());

        // channel for file events

        // create the watcher once
        // let mut w = RecommendedWatcher::new(
        //     move |res| {
        //         if let Ok(evt) = res {
        //             let _ = tx.send(evt);
        //         }
        //     },
        //     notify::Config::default(),
        // )
        // .expect("failed to init watcher");

        // w.watch(&path, RecursiveMode::Recursive)
        //     .expect("failed to watch folder");

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
        // Toolbar
        // egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {

        //     if ui.button("Select TOML Folder").clicked() {
        //         if let Some(dir) = FileDialog::new().pick_folder() {
        //             self.watch_path = Some(dir.clone());
        //             let (tx, rx) = channel::<Event>();
        //             let mut watcher: RecommendedWatcher = RecommendedWatcher::new(
        //                 move |res| {
        //                     if let Ok(event) = res {
        //                         let _ = tx.send(event);
        //                     }
        //                 },
        //                 Config::default(),
        //             )
        //             .unwrap();
        //             watcher.watch(&dir, RecursiveMode::Recursive).unwrap();
        //             self.reload_rx = Some(rx);
        //             load_and_prepare(&dir, &mut self.templates, &mut self.roots);
        //         }
        //     }
        // });

        if self.watch_path.is_none() {
            egui::CentralPanel::default().show(ctx, |ui| {
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

        // Tree view
        // egui::SidePanel::left("tree_panel").show(ctx, |ui| {
        //     ui.heading("Widget Trees");
        //     for (i, def) in self.roots.iter().enumerate() {
        //         let label = def.id.clone().unwrap_or_else(|| format!("Root {}", i));
        //         egui::CollapsingHeader::new(label)
        //             .default_open(true)
        //             .show(ui, |ui| {
        //                 show_tree(ui, def);
        //             });
        //     }
        // });

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
                    println!("[DEBUG] Read error: {:?}", e);
                    continue;
                }
            };

            let stem = entry
                .path()
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("");

            // Try parsing as template
            if let Ok(tpl) = toml::from_str::<TemplateDef>(&txt) {
                if !tpl.interface.is_empty() {
                    println!("[DEBUG] Registered template: {}", stem);
                    let params = tpl.interface.iter().map(|p| p.name.clone()).collect();
                    templates.insert(
                        stem.to_string(),
                        (
                            WidgetDef {
                                widget_type: tpl.widget_type,
                                id: tpl.id,
                                properties: tpl.properties,
                                children: tpl.children,
                            },
                            params,
                        ),
                    );
                    continue; // skip root parsing
                }
            }
            // Else try as root definition
            if let Ok(def) = toml::from_str::<WidgetDef>(&txt) {
                println!(
                    "[DEBUG] Registered root: {} (id={:?})",
                    def.widget_type, def.id
                );
                roots.push(def);
            } else {
                println!("[DEBUG] Failed to parse {} as template or root", stem);
            }
        }
    }

    println!("[DEBUG] Total templates: {}", templates.len());
    println!("[DEBUG] Total roots: {}", roots.len());

    for def in roots.iter_mut() {
        expand_templates(def, &templates);
    }
}

fn expand_templates(def: &mut WidgetDef, templates: &HashMap<String, (WidgetDef, Vec<String>)>) {
    if let Some((base, allowed)) = templates.get(&def.widget_type) {
        // shallow‐clone the base widget
        let mut merged = base.clone();

        // filter instance properties by allowed
        let props = std::mem::take(&mut def.properties);
        for (k, v) in props.into_iter().filter(|(k, _)| allowed.contains(k)) {
            merged.properties.insert(k, v);
        }

        // merge children
        let extra = std::mem::take(&mut def.children);
        merged.children.extend(extra);

        // replace
        *def = merged;
    }

    // recurse
    for child in &mut def.children {
        expand_templates(child, templates);
    }
}

// fn show_tree(ui: &mut egui::Ui, def: &WidgetDef) {
//     let label = def.id.as_deref().unwrap_or(&def.widget_type);
//     egui::CollapsingHeader::new(label)
//         .default_open(true)
//         .show(ui, |ui| {
//             for child in &def.children {
//                 show_tree(ui, child);
//             }
//         });
// }

// fn show_tree(ui: &mut egui::Ui, def: &WidgetDef) {
//     // Compose a unique id_source from the address of `def` or its label + a counter:
//     let id_src = def.id.as_deref().unwrap_or(&def.widget_type);
//     let unique = format!("{}_{:p}", id_src, def);

//     egui::CollapsingHeader::new(&def.id.clone().unwrap_or(id_src.to_string()))
//         .id_source(unique)
//         .default_open(true)
//         .show(ui, |ui| {
//             for child in &def.children {
//                 show_tree(ui, child);
//             }
//         });
// }

// fn show_tree(ui: &mut egui::Ui, def: &WidgetDef) {
//     let label = def.id.as_deref().unwrap_or(&def.widget_type);

//     // tack on a unique suffix after `##` so egui uses it as the internal ID:
//     let unique_label = format!("{label}##{:p}", def);

//     egui::CollapsingHeader::new(unique_label)
//         .default_open(true)
//         .show(ui, |ui| {
//             for child in &def.children {
//                 show_tree(ui, child);
//             }
//         });
// }

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
    let app = TomlUiApp::new();
    let native_options = eframe::NativeOptions::default();

    let _ = eframe::run_native(
        "TOML egui Builder",
        native_options,
        Box::new(|_cc| Ok(Box::new(app))),
    );
}
