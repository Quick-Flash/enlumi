use std::rc::Rc;
use zgui::prelude::*;
use zgui_ui::prelude::*;

#[component]
pub fn PlotSettings(children: ChildrenFn) -> impl IntoView {
    let host = use_local_context::<RwSignal<Option<Rc<ChildrenFn>>, LocalStorage>>()
        .expect("PlotSettings requires WorkspacePlot's local settings host");
    let registration = Rc::new(children);
    host.set(Some(registration.clone()));
    on_cleanup_local(move || {
        if host
            .try_with_untracked(|current| {
                current
                    .as_ref()
                    .is_some_and(|current| Rc::ptr_eq(current, &registration))
            })
            .unwrap_or(false)
        {
            host.try_set(None);
        }
    });
    view! {}
}

#[component]
pub fn PlotSettingsButton(
    settings: RwSignal<Option<Rc<ChildrenFn>>, LocalStorage>,
    #[prop(optional)] children: Option<ChildrenFn>,
) -> impl IntoView {
    let extra = StoredValue::new_local(children);
    view! {
        row(class = "plot-settings-header") {
            if move || settings.try_get().flatten().is_some() {
            Popover {
                PopoverTrigger(size = ButtonSize::Xs, variant = ButtonVariant::Ghost, class = "plot-settings-trigger", a11y:label = "Open plot settings") { "Settings" }
                PopoverContent(class = "plot-settings-surface") {
                    column(class = "plot-settings-content") {
                        text(class = "settings-title") { "Plot settings" }
                        {move || settings.try_get().flatten().map(|children| children.view())}
                        {extra.with_value(|children| children.as_ref().map(|children| children.view()))}
                        row(class = "settings-footer") {
                            text(class = "trend-hint") { "Escape closes settings" }
                            PopoverClose(size = ButtonSize::Sm, variant = ButtonVariant::Outline) { "Done" }
                        }
                    }
                }
            }
            } else {
                Button(size = ButtonSize::Xs, variant = ButtonVariant::Ghost, class = "plot-settings-trigger", disabled = true, a11y:label = "Plot settings unavailable") { "Settings" }
            }
        }
    }
}

#[component]
pub fn ScatterFlipButton(
    config: RwSignal<crate::workspace::PlotConfig, LocalStorage>,
) -> impl IntoView {
    view! {
        if move || config.get().kind == crate::workspace::PlotKind::Scatter {
            Button(size = ButtonSize::Xs, disabled = Signal::derive_local(move || config.with(|config| config.x.is_empty() || config.y.is_empty())), a11y:label = "Swap scatter X and Y axes", on:click = move |_| {
                config.update(|config| std::mem::swap(&mut config.x, &mut config.y));
            }) { "Flip X/Y" }
        }
    }
}
