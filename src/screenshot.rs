use crate::state::{Pane, Workbench};
use plot::{
    zgui_svg::{ScreenshotFormat, WorkspaceSnapshot, capture_plot, capture_workspace_with_overlay},
    zgui_workspace::ScreenshotMenuProps,
};
use zgui::prelude::*;

#[component]
pub fn Screenshot(workbench: Workbench, #[prop(optional)] pane: Option<Pane>) -> impl IntoView {
    view! {ScreenshotMenu(label=if pane.is_some(){"Screenshot plot"}else{"Screenshot all docks"},on_export=UnsyncCallback::new(move |format|workbench.save_screenshot(pane,format)),on_copy=UnsyncCallback::new(move |()|workbench.copy_screenshot(pane)))}
}

impl Workbench {
    fn copy_screenshot(self, pane: Option<Pane>) {
        let svg = match self.capture_screenshot(pane) {
            Ok(svg) => svg,
            Err(error) => {
                self.status.set(error);
                return;
            }
        };
        let clipboard = self.screenshot_clipboard.get_value();
        self.task(async move {
            let result = blocking(move || clipboard.copy(&svg)).await;
            self.status.set(match result {
                Ok(()) => "Screenshot copied to clipboard".into(),
                Err(error) => error,
            });
        });
    }

    pub fn capture_screenshot(self, pane: Option<Pane>) -> Result<String, String> {
        if let Some(pane) = pane {
            capture_plot(pane.snapshot, true)
        } else {
            let slots = self.panes.with_untracked(|panes| {
                panes
                    .iter()
                    .map(|pane| WorkspaceSnapshot {
                        id: pane.id,
                        node: pane.dock.node,
                        snapshot: pane.snapshot,
                    })
                    .collect::<Vec<_>>()
            });
            capture_workspace_with_overlay(
                &slots,
                self.maximized.get_untracked(),
                &self.layout.get_untracked(),
                "#12181c",
                true,
            )
        }
    }
    fn save_screenshot(self, pane: Option<Pane>, format: ScreenshotFormat) {
        let svg = match self.capture_screenshot(pane) {
            Ok(svg) => svg,
            Err(error) => {
                self.status.set(error);
                return;
            }
        };
        let name = format!(
            "enlumi-{}.{}",
            pane.map_or_else(|| "docks".into(), |pane| format!("plot-{}", pane.id)),
            format.extension()
        );
        self.task(async move {
            let path = blocking(move || {
                rfd::FileDialog::new()
                    .add_filter(format.extension(), &[format.extension()])
                    .set_file_name(name)
                    .save_file()
            })
            .await;
            let Some(path) = path else {
                return;
            };
            let result = blocking(move || format.save(&svg, &path).map(|()| path)).await;
            self.status.set(match result {
                Ok(path) => format!("Screenshot saved: {}", path.display()),
                Err(error) => error,
            });
        });
    }
}
