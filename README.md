Everything we’ve built so far (the Pest grammar, the AST walker, the error-printer with line/column & hints) slots straight into a Language Server, so that VS Code (or any LSP-aware editor) can highlight syntax errors live as we type.

Here’s a rough roadmap:

---

### 1. Pick an LSP framework in Rust

Two popular choices:

* **[`tower-lsp`](https://github.com/ebkalderon/tower-lsp)**: ergonomic, futures-based, built on Tokio.
* **[`lsp-server`](https://crates.io/crates/lsp-server)** + **[`lsp-types`](https://crates.io/crates/lsp-types)**: a bit more boilerplate but very flexible.

---

### 2. Wire up diagnostics on document change

In LSP implementation, we’ll handle the **`textDocument/didOpen`** and **`textDocument/didChange`** notifications. On each update:

1. **Fetch** the newest document text.

2. **Call** `parser::parse_template(&text, &stem)` (or our unified `parse_wui`).

3. **On `Err(err)`**, use our `print_parse_error` logic—except instead of printing, **map** it into an LSP **`Diagnostic`**:

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
* A **`client`** in JavaScript/TypeScript that spawns our Rust binary (the LSP server) and hooks up stdin/stdout.
* Associate **`languageId: "wui"`** so VS Code knows when to send events.

There are plenty of “Rust + tower-lsp” VS Code extension templates we can borrow. Once installed, every time we edit a `.wui` file we’ll see red squiggles at the exact line/column, and hover for the “Hint: …” message.

---

### 4. Optional: code completion & hover

With our AST we can also implement:

* **`textDocument/completion`** to suggest built-in widgets (`Frame`, `Label`, `If`, `For`, etc.) and any parsed template names.
* **`textDocument/hover`** to show a widget’s parameter list when we hover over its name.

Both just pull from our `Rule::ident` matches and our `templates` registry.

---

Once we’ve got that in place, our `.wui` files will feel every bit as first-class as any mainstream language in VS Code—live errors, auto-complete, hover docs, even go-to-definition for templates. Enjoy leveling up our WUI DSL!
