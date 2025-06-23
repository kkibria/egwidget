# Handling of the parse tree
Currently, we are specifying a watch folder. The watch folder is actually an
editable library. In the beginning Each "wui" files is parsed based on a pest
grammar and a object is created called widget (we have called it Template, but
moving forward we will call it Widget) and a Vec of widgets is returned. We
assume the top level widget is "main.wui". 

If we watch every single file in the watch folder it is possible that we may
have parsed more widgets than necessary, which we should avoid. To do that we
need to lazy load widgets, a. We need to specify what the top level widget is
instead of assuming "main.wui". b. We will parse the top level and then examine
it to find all the widget instances. That will dictate what additional widget
object we need. c. We will dump the parsed widgets as a binary file as well, and
when we need it we can compare timestamps and decide if we need to reparse it
again.

Once thats done, it will be added to the Vec of widgets. At this point we will
watch the watch folder and any "wui" file present in the Vec of widgets is
edited changed, then we will parse that and refresh the Vec of widgets.

# Using the Vec of widgets
We will have to create a synthetic widget which will serve the role of a test
rig. This will have be connected to our top level widget parameters in a
reactive way. The test rig fields will be shown on top of the widget render are
(The center area) in its own pane. We will build the tree (the left pane)
showing each "wui" widget as an expandable node. When expanded, children widgets
are shown. If we use any widget which is a builtin widget, then the node will
not expand. 

We must ensure that there is no recursion of "wei" widgets in the entire tree.
We must report this as a failure with details and abandon the tree.  

The widget display area will display the widgets. This will be done by
introspecting the list of widgets and performing the reactivity by linking all
the parameters to create another data structure where states of each instance
will be maintained. Using them we will simulate the behavior along with the
builtin widgets. 



# Preparing the rust version of the widget
Once satisfied with functionality and the looks, we will write a rust file that
is essentially a flattened list where all the instances are inlined and all the
reactive states are inlined and the rust file will have the same base name as
the top level widget and same behavior as the top level widget that can be used
with other rust egui project. 

# egui built in widgets 
Since egui may add more widgets in future, hardcoding built in widgets may
become a frequent maintenance issue. In order to mitigate that we will
distinguish between built in widget and "wui" widget. "wui" widgets will be
names will start with uppercase letter. Any widget name start with lowercase is
considered a builtin widget. As such, any widget name starting with uppercase
letter, to be searched in the watch folder and parsed. However, we must consider
platforms which uses case independent file names. We should compare file names
with widget names in a case independent way in our file watch.


# VScode integration
Everything we’ve built so far (the Pest grammar, the AST walker, the
error-printer with line/column & hints) slots straight into a Language Server,
so that VS Code (or any LSP-aware editor) can highlight syntax errors live as we
type.

Here’s a rough roadmap:

---

### 1. Pick an LSP framework in Rust

Two popular choices:

* **[`tower-lsp`](https://github.com/ebkalderon/tower-lsp)**: ergonomic,
  futures-based, built on Tokio.
* **[`lsp-server`](https://crates.io/crates/lsp-server)** +
  **[`lsp-types`](https://crates.io/crates/lsp-types)**: a bit more boilerplate
  but very flexible.

---

### 2. Wire up diagnostics on document change

In LSP implementation, we’ll handle the **`textDocument/didOpen`** and
**`textDocument/didChange`** notifications. On each update:

1. **Fetch** the newest document text.

2. **Call** `parser::parse_template(&text, &stem)` (or our unified `parse_wui`).

3. **On `Err(err)`**, use our `print_parse_error` logic—except instead of
   printing, **map** it into an LSP **`Diagnostic`**:

   ```rust
   Diagnostic {
     range: Range {
       start: Position { line: err_line-1, character: err_col-1 },
       end:   Position { line: err_line-1, character: err_col }, 
     },
     severity: Some(DiagnosticSeverity::ERROR),
     code: None,
     source: Some("wui-ls".into()),
     message: format!(
       "{} (expected {:?})",
       err.variant.message(),
       err.variant.expected()
     ),
     related_information: None,
   }
   ```

4. **Publish** those diagnostics back to the client via
   `client.publish_diagnostics(Url::parse(&uri)?, diagnostics, None).await;`.

If the parse succeeds, send back an empty diagnostics list to clear old errors.

---

### 3. Configure VS Code to use our server

We’ll need a minimal VS Code extension:

* **`package.json`** that declares a language “wui” with file-extension `.wui`.
* A **`client`** in JavaScript/TypeScript that spawns our Rust binary (the LSP
  server) and hooks up stdin/stdout.
* Associate **`languageId: "wui"`** so VS Code knows when to send events.

There are plenty of “Rust + tower-lsp” VS Code extension templates we can
borrow. Once installed, every time we edit a `.wui` file we’ll see red squiggles
at the exact line/column, and hover for the “Hint: …” message.

---

### 4. Optional: code completion & hover

With our AST we can also implement:

* **`textDocument/completion`** to suggest built-in widgets (`Frame`, `Label`,
  `If`, `For`, etc.) and any parsed template names.
* **`textDocument/hover`** to show a widget’s parameter list when we hover over
  its name.

Both just pull from our `Rule::ident` matches and our `templates` registry.

---

Once we’ve got that in place, our `.wui` files will feel every bit as
first-class as any mainstream language in VS Code—live errors, auto-complete,
hover docs, even go-to-definition for templates. Enjoy leveling up our WUI DSL!





