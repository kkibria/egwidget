// src/builder.rs
use std::collections::HashMap;
use crate::parser::{Widget, WidgetInstance};
use serde::Deserialize;

/// A fully‐expanded widget ready for rendering.
#[derive(Debug, Deserialize, Clone, Default)]
// #[derive(Debug, Clone)]
pub struct WidgetDef {
    /// The name of the widget (e.g. "Frame", or a user template name).
    pub widget_type: String,

    /// Optional explicit ID (for naming in CollapsingHeaders, etc).
    pub id: Option<String>,

    /// Argument map: literal strings, numbers, or script blocks `{{…}}`.
    pub args: HashMap<String, String>,

    /// Child widgets in the layout tree.
    pub children: Vec<WidgetDef>,
}

/// Given a map of templates by name, and the root template name,
/// build a single expanded WidgetDef tree.
pub fn build_widget_tree(
    widgets: &HashMap<String, Widget>,
    root_name: &str,
) -> Option<WidgetDef> {
    widgets.get(root_name).map(|tpl| {
        // Start by instantiating the template with default params:
        let mut root = instantiate_widget(tpl);

        // Then recursively expand any nested template calls:
        expand_instances(&mut root, widgets);
        root
    })
}

/// Create a WidgetDef from a Template, filling all params with their default values.
/// (You can adapt this to override defaults from outside.)
fn instantiate_widget(tpl: &Widget) -> WidgetDef {
    // Build the initial children from the template body:
    let children = tpl
        .body
        .iter()
        .map(build_from_instance)
        .collect();

    WidgetDef {
        widget_type: tpl.name.clone(),
        id:          Some(tpl.name.clone()),
        args:        HashMap::new(), // no overrides at top level
        children,
    }
}

/// Turn a single WidgetInstance into a WidgetDef (without expanding templates).
fn build_from_instance(inst: &WidgetInstance) -> WidgetDef {
    let children = inst
        .children
        .iter()
        .map(build_from_instance)
        .collect();

    WidgetDef {
        widget_type: inst.widget.clone(),
        id:          inst.args.get("id").cloned(), // if user passed `id=…`
        args:        inst.args.clone(),
        children,
    }
}

/// Recursively walk the WidgetDef tree; whenever you see a widget_type
/// matching a template name, inline that template, merging args+children.
fn expand_instances(def: &mut WidgetDef, templates: &HashMap<String, Widget>) {
    if let Some(tpl) = templates.get(&def.widget_type) {
        // 1) Build a fresh base from the template
        let mut base = instantiate_widget(tpl);

        // 2) Merge def.args into base.args (overrides)
        base.args.extend(def.args.clone());

        // 3) Append any user‐provided children after the template’s own children
        base.children.extend(def.children.drain(..));

        // 4) Preserve any explicit id override, or keep base.id
        if let Some(own_id) = def.id.take() {
            base.id = Some(own_id);
        }

        // 5) Replace def in place
        *def = base;
    }

    // Recurse into children
    for child in &mut def.children {
        expand_instances(child, templates);
    }
}
