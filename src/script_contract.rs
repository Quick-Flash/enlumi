//! Explicit output arity: arbitrary Rust loops/indices cannot be inferred safely.
pub fn output_count(source: &str) -> Result<usize, String> {
    let module = syn::parse_file(source).map_err(|error| syntax_diagnostics(source, error))?;
    let declarations: Vec<_> = module
        .items
        .iter()
        .filter_map(|item| match item {
            syn::Item::Const(item) if item.ident == "OUTPUTS" => Some(item),
            _ => None,
        })
        .collect();
    let [item] = declarations.as_slice() else {
        return Err(
            "Declare exactly one pub const OUTPUTS: usize = N; matching named output rows (1-16)."
                .into(),
        );
    };
    if !matches!(item.vis, syn::Visibility::Public(_))
        || !matches!(item.ty.as_ref(), syn::Type::Path(ty) if ty.path.is_ident("usize"))
        || !item.attrs.is_empty()
    {
        return Err("OUTPUTS must be an unconditional pub const OUTPUTS: usize = N;".into());
    }
    let syn::Expr::Lit(syn::ExprLit {
        lit: syn::Lit::Int(value),
        ..
    }) = item.expr.as_ref()
    else {
        return Err("OUTPUTS must be a literal number from 1 to 16".into());
    };
    let count = value
        .base10_parse::<usize>()
        .map_err(|_| "OUTPUTS must be a number from 1 to 16")?;
    if !(1..=16).contains(&count) {
        return Err("OUTPUTS must be a number from 1 to 16".into());
    }
    Ok(count)
}

fn syntax_diagnostics(source: &str, errors: syn::Error) -> String {
    let mut diagnostics = String::from("Rust compilation failed:\n");
    for error in errors {
        let position: proc_macro2::LineColumn = error.span().start();
        let line = position.line.max(1);
        let text = source.lines().nth(line - 1).unwrap_or("");
        let gutter = line.to_string().len();
        // Editor and parser count UTF-8 characters, not bytes. Expand tabs for caret alignment.
        let padding: usize = text
            .chars()
            .take(position.column)
            .map(|c| if c == '\t' { 4 } else { 1 })
            .sum();
        let context = if line > 1 {
            let previous = source
                .lines()
                .nth(line - 2)
                .unwrap_or("")
                .replace('\t', "    ");
            format!("{:>gutter$} | {previous}\n", line - 1)
        } else {
            String::new()
        };
        diagnostics.push_str(&format!(
            "error: {error}\n --> filter.rs:{line}:{}\n{:gutter$} |\n{context}{line} | {}\n{:gutter$} | {}^\n",
            position.column + 1, "", text.replace('\t', "    "), "", " ".repeat(padding),
        ));
    }
    diagnostics
}
