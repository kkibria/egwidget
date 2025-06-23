use pest::Position;
use pest::error::InputLocation;

use pest::Parser;
use pest::iterators::Pair;
use pest_derive::Parser;
use std::collections::HashMap;

#[allow(dead_code)]
#[derive(Debug)]
pub struct Widget {
    pub name: String,
    pub params: Vec<String>,
    pub body: Vec<WidgetInstance>,
}

#[allow(dead_code)]
#[derive(Debug)]
pub struct WidgetInstance {
    pub widget: String,
    pub args: HashMap<String, String>, // literal or {{…}} script
    pub children: Vec<WidgetInstance>,
}

#[derive(Parser)]
#[grammar = "wui.pest"] // relative to src/
pub struct WuiParser;

// Entry point: parse one file into a Template
pub fn parse_template(source: &str, name: &str) -> Result<Widget, pest::error::Error<Rule>> {
    let mut pairs = WuiParser::parse(Rule::file, source)?;
    let mut params = Vec::new();
    let mut body = Vec::new();

    for pair in pairs.next().unwrap().into_inner() {
        match pair.as_rule() {
            Rule::param_decl => {
                let mut inner = pair.into_inner();
                let ident = inner.next().unwrap().as_str().to_string();
                // ignore default for now
                params.push(ident);
            }
            Rule::widget_decl => {
                body.push(build_widget(pair));
            }
            Rule::breakpoints_decl
            | Rule::effect_decl
            | Rule::if_decl
            | Rule::for_decl
            | Rule::on_change_decl => {
                // TODO: handle these statements if you want
            }
            _ => {}
        }
    }

    Ok(Widget {
        name: name.to_string(),
        params,
        body,
    })
}

fn build_widget(pair: Pair<Rule>) -> WidgetInstance {
    assert_eq!(pair.as_rule(), Rule::widget_decl);
    let mut inner = pair.into_inner();
    // 1) widget name
    let widget = inner.next().unwrap().as_str().to_string();

    // 2) optional args
    let mut args = HashMap::new();
    let mut children = Vec::new();
    for section in inner {
        match section.as_rule() {
            Rule::arg_list => {
                for arg_pair in section.into_inner() {
                    let mut kv = arg_pair.into_inner();
                    let key = kv.next().unwrap().as_str().to_string();
                    let val = kv.next().unwrap().as_str().to_string();
                    args.insert(key, val);
                }
            }
            Rule::statement => {
                // nested statement: could be widget_decl, if_decl, for_decl, etc.
                let stmt = section.into_inner().next().unwrap();
                match stmt.as_rule() {
                    Rule::widget_decl => {
                        children.push(build_widget(stmt));
                    }
                    Rule::if_decl => {
                        // TODO: parse `If(...) { ... }` into a WidgetInstance
                    }
                    Rule::for_decl => {
                        // TODO: parse `For(...) { ... }`
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }

    WidgetInstance {
        widget,
        args,
        children,
    }
}

/// Print a user-friendly parse error for a `.wui` file.
pub fn print_parse_error(source: &str, err: pest::error::Error<Rule>, filename: &str) {
    // Compute (line, col) from the error’s location
    let (line, col) = match err.location {
        InputLocation::Pos(pos) => {
            // single position
            let p = Position::new(source, pos).unwrap();
            p.line_col()
        }
        InputLocation::Span((start, _end)) => {
            // span: point at the start
            let p = Position::new(source, start).unwrap();
            p.line_col()
        }
    };

    // 1) File and location
    eprintln!(
        "[ERROR] Parse error in {} at line {}, column {}:",
        filename, line, col
    );

    // 2) The Pest error message
    eprintln!("  {}", err.variant.message());

    // 3) Show the source line, if we can
    if let Some(src_line) = source.lines().nth(line - 1) {
        eprintln!("  {}", src_line);
        // 4) Draw a caret under the offending column
        let mut marker = String::new();
        for _ in 0..(col - 1) {
            // preserve tabs as tabs
            marker.push(' ');
        }
        marker.push('^');
        eprintln!("  {}", marker);
    }
}
