use crate::{profile_library::ProfileLibraryProps, state::Workbench};
use zgui::prelude::*;
use zgui_ui::prelude::*;

#[component]
pub fn ProfilePicker(workbench: Workbench) -> impl IntoView {
    install_stylesheet("enlumi-profiles", include_str!("profile.css"));
    view! {
        Dialog {
            DialogTrigger(size = ButtonSize::Xs, variant = ButtonVariant::Ghost, class = "app-action profiles-trigger") { "Profiles" }
            DialogContent(class = "profile-dialog") {
                DialogHeader {
                    DialogTitle { "Log profiles" }
                    DialogDescription { "Load names, filters, geometry, and optional plot views from a JSON profile." }
                }
                ProfileLibrary(workbench = workbench)
                DialogFooter { DialogClose { "Close" } }
            }
        }
    }
}
