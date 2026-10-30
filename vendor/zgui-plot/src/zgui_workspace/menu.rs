use super::ScatterFlipButtonProps;
use crate::workspace::{PlotConfig, PlotKind};
use zgui::prelude::*;
use zgui::reactive::{Callable, UnsyncCallback};
use zgui_ui::prelude::*;

/// Application factories and selection hooks for generic workspace commands.
#[derive(Clone, Copy)]
pub struct PlotCommands {
    pub select: UnsyncCallback<u64>,
    pub duplicate: UnsyncCallback<()>,
    pub split: UnsyncCallback<bool>,
    pub move_plot: UnsyncCallback<bool>,
    pub can_move: UnsyncCallback<bool, bool>,
    pub maximize: UnsyncCallback<()>,
}

/// Plot kind, duplicate, split, reorder, maximize, and axis swap controls.
#[component]
pub fn PlotMenu(
    id: u64,
    config: RwSignal<PlotConfig, LocalStorage>,
    maximized: RwSignal<Option<u64>, LocalStorage>,
    commands: PlotCommands,
) -> impl IntoView {
    view! {
        Popover {
            PopoverTrigger(size = ButtonSize::Xs, variant = ButtonVariant::Ghost) { "Plot" }
            PopoverContent(class = "plot-settings-surface") {
                for kind in move || vec![PlotKind::Trend, PlotKind::Histogram, PlotKind::Scatter], key = |kind| *kind {
                    Button(size = ButtonSize::Sm, class:setting-active = move || config.get().kind == kind,
                        on:click = move |_| {
                            commands.select.run(id);
                            if config.get_untracked().kind != kind { config.update(|config| config.kind = kind); }
                        }) {{format!("View: {kind:?}")}}
                }
                Button(size = ButtonSize::Sm, on:click = move |_| commands.duplicate.run(())) { "Duplicate plot" }
                row(class = "plot-toolbar") {
                    for vertical in move || [false, true], key = |vertical| *vertical {
                        Button(size = ButtonSize::Sm, on:click = move |_| commands.split.run(vertical)) {{if vertical { "Split below" } else { "Split right" }}}
                    }
                }
                row(class = "plot-toolbar") {
                    for forward in move || [false, true], key = |forward| *forward {
                        Button(size = ButtonSize::Sm,
                            disabled = Signal::derive_local(move || !commands.can_move.run(forward)),
                            on:click = move |_| commands.move_plot.run(forward)) {{if forward { "Move next" } else { "Move previous" }}}
                    }
                }
            }
        }
        Button(size = ButtonSize::Xs, on:click = move |_| commands.maximize.run(())) {{move || if maximized.get() == Some(id) { "Restore" } else { "Maximize" }}}
        ScatterFlipButton(config = config)
    }
}
