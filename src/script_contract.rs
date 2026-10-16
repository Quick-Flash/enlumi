//! Explicit output arity: arbitrary Rust loops/indices cannot be inferred safely.
pub fn output_count(source: &str) -> Result<usize, String> {
    let module = syn::parse_file(source)
        .map_err(|error| format!("Rust compilation failed: filter.rs: syntax error: {error}"))?;
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
