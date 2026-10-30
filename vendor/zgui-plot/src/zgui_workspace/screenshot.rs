use crate::zgui_svg::ScreenshotFormat;
use zgui::prelude::*;
use zgui::reactive::Callable;
use zgui_ui::prelude::*;

/// Shared plot/workspace screenshot menu. Host owns destination dialog and tasks.
#[component]
pub fn ScreenshotMenu(
    on_export: UnsyncCallback<ScreenshotFormat>,
    #[prop(optional)] on_copy: Option<UnsyncCallback<()>>,
    #[prop(into, default="Screenshot plot".into())] label: String,
) -> impl IntoView {
    install_stylesheet("plot-screenshot", include_str!("screenshot.css"));
    view! {
        DropdownMenu {
            DropdownMenuTrigger(size=ButtonSize::Xs,variant=ButtonVariant::Ghost,class="plot-screenshot",a11y:label=label) {
                vector(class="plot-screenshot-icon",prop:viewBox="0 0 20 20",prop:d="M2 5 H6 L7 3 H13 L14 5 H18 V17 H2 Z M14 10 A4 4 0 1 1 6 10 A4 4 0 1 1 14 10",a11y:hidden=true)
            }
            DropdownMenuContent(class="plot-screenshot-menu") {
                MenuItem(on_select=UnsyncCallback::new(move |()|on_export.run(ScreenshotFormat::Svg))) {"Save SVG"}
                if move ||cfg!(feature="raster") {
                    MenuItem(on_select=UnsyncCallback::new(move |()|on_export.run(ScreenshotFormat::Png))) {"Save PNG"}
                    if move ||on_copy.is_some() {
                        MenuItem(on_select=UnsyncCallback::new(move |()|{if let Some(copy)=on_copy {copy.run(())}})) {"Copy image"}
                    }
                }
            }
        }
    }
}
