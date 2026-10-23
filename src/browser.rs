use crate::action_button::ActionButtonProps;
use crate::field_label::FieldLabelProps;
use crate::geometry_library::GeometryLibraryProps;
use crate::parameter_details::ParameterDetailsProps;
use crate::profile_library::ProfileLibraryProps;
use crate::tree_lines::TreeBranchProps;
use crate::{field_row::FieldRowProps, state::Workbench};
use crate::{sidebar_help::SidebarHelpProps, sidebar_search::SidebarSearchProps};
use zgui::prelude::*;

#[component]
pub fn Browser(workbench: Workbench) -> impl IntoView {
    let rows = crate::browser_fields::rows(workbench);
    let port = NodeRef::new();
    provide_local_context(crate::profile_preset_tree::ProfileViewport(port));
    let virtualized = zgui_ui::virtualize::Virtualize::new(
        port,
        Signal::derive_local(move || rows.with(Vec::len)),
        28.0,
        4,
    );
    let visible_fields = Memo::new(move |_| workbench.browser_fields.get());
    let resize = RwSignal::new_local(None::<(f32, f32)>);
    let analysis_unavailable = Signal::derive_local(move || {
        workbench.log.with(Option::is_none)
            || workbench.loading.get()
            || workbench.derived_busy.get()
            || workbench.profiles.busy.get()
            || workbench.analysis().busy.get()
    });
    let no_analysis_selection = Signal::derive_local(move || {
        workbench.selected_field.with(|source| {
            source.as_ref().is_none_or(|source| {
                source.starts_with("@setting/")
                    || source.starts_with("@analysis/")
                    || crate::profile::identity::Drag::parse(source).is_some_and(|drag| {
                        matches!(drag.item, crate::profile::identity::Item::Geometry(_))
                    })
            })
        })
    });
    let spectra_unavailable =
        Signal::derive_local(move || analysis_unavailable.get() || no_analysis_selection.get());
    view! {
        row(class = "browser", class:hidden = move || !workbench.sidebar_visible.get(), style:width = move || Some(format!("{}px", workbench.sidebar_width.get() + 3.0))) {
            column(class = "signal-sidebar") {
                row(class = "browser-heading") {
                    text(class="heading") {"Fields"}
                    ActionButton(class = "open-filters", node_ref = workbench.filter_button, class:setting-active = move || workbench.filters_open.get(), disabled = Signal::derive_local(move || workbench.log.get().is_none() || workbench.loading.get()), on:click = move |_| workbench.filters_open.update(|open| *open = !*open)) { "Filters" }
                    ActionButton(class = "collapse-fields", on:click = move |_| {workbench.expanded.set(std::collections::BTreeSet::from(["@raw".into()]));workbench.profiles.open.set(false);}) { "Collapse all" }
                }
                SidebarSearch(value = workbench.field_query, placeholder = "Search fields", label = "Search fields") {
                    SidebarHelp(label = "Field shortcuts") {
                        text(class="heading") { "Plot and filter fields" }
                        text { "Drag onto plot to add line. Enter plots focused field." }
                        text { "Ctrl-click toggles selection across Raw, Derived and Profiles. Shift-click selects range." }
                        text { "Search filters field entries; expand field sections to browse matches." }
                    }
                }
                column(class="analysis-selection-actions") {
                    row(class = "field-selection-actions") {
                        ActionButton(class="save-selection-profile",disabled=Signal::derive_local(move ||analysis_unavailable.get()||workbench.selected_field.with(Option::is_none)),on:click=move |_|workbench.save_selected_profile()) {"Save?"}
                        ActionButton(class="create-selection-fft",disabled=spectra_unavailable,on:click = move |_| workbench.create_selected_spectra("fft")) { "Create FFTs" }
                        ActionButton(class="create-selection-psd",disabled=spectra_unavailable,on:click = move |_| workbench.create_selected_spectra("psd")) { "Create PSDs" }
                        ActionButton(class = "analyze-selection",disabled=analysis_unavailable, on:click = move |_| {
                            if no_analysis_selection.get_untracked() {
                                workbench.analysis().open.set(true);
                                workbench.filters_open.set(false);
                            } else {workbench.analyze_selection();}
                        }) { "Configure" }
                        SidebarHelp(label="FFT, PSD and Configure help") {
                            text(class="heading") {"Analyze selected fields"}
                            text {"Create FFTs: show frequency amplitudes, one plot per selected field."}
                            text {"Create PSDs: show power per Hz, one plot per selected field."}
                            text {"Configure: open Analyze to adjust inputs, time range, window and DC removal before creating plots."}
                            text {"Select fields or profile views to enable FFT/PSD. Views use their member fields; folders organize profiles."}
                        }
                    }
                    text(class="analysis-selection-hint muted") {{move ||if workbench.log.with(Option::is_none){"Open a log to analyze fields."}else if no_analysis_selection.get(){"Select fields for FFT / PSD."}else{"Drag fields to plots or Analyze."}}}
                }
                GeometryLibrary(workbench = workbench)
                scroll(class="signal-list",node_ref=port,a11y:label="Fields and profiles") {
                    column(class="browser-field-rows",style:padding-top=move ||Some(format!("{}px",virtualized.window().lead)),style:padding-bottom=move ||Some(format!("{}px",virtualized.window().trail))) {
                        for index in move ||virtualized.indices(),key=|index|*index {
                            FieldRow(workbench=workbench,row=Memo::new(move |_|rows.with(|rows|rows.get(index).cloned())),visible_fields=visible_fields)
                        }
                    }
                    TreeBranch(open=workbench.profiles.open,name="Profiles".into(),count=0usize,depth=0,connectors=false,class="profiles-trigger browser-section")
                    column(class="profile-browser-pane",class:hidden=move ||!workbench.profiles.open.get()) {
                        ProfileLibrary(workbench=workbench)
                    }
                }
                ParameterDetails(workbench = workbench)
                row(class = "selected-field-row") {
                text(class = "selected-field") {{move || workbench.selected_field.get().map_or_else(|| "Select a field to edit labels".into(), |field| workbench.field_name(&field))}}
                    FieldLabel(workbench = workbench)
                }
                if move ||workbench.selected_fields.with(|fields|fields.len()>1) {
                    row(class="field-selection-actions") {
                        text(class="field-selection-count muted") {{move ||format!("{} selected",workbench.selected_fields.with(|fields|fields.len()))}}
                        ActionButton(a11y:label="Clear field selection",on:click=move |_|{workbench.selected_fields.set(Default::default());workbench.selected_field.set(None);workbench.selection_anchor.set(None);}) { "Clear" }
                    }
                }
                if move || workbench.log.get().is_some_and(|log| !log.warnings.is_empty()) {
                    text(class = "warnings") {{move || workbench.log.with(|log| log.as_ref().map_or_else(String::new, |log| format!("{} load warnings", log.warnings.len())))}}
                }
            }
            control(class = "sidebar-grip workspace-divider", tabindex = Focus::Sequential, a11y:label = "Resize field browser",
                on:pointer_down = move |event| { resize.set(Some((event.position.x.0, workbench.sidebar_width.get_untracked()))); event.capture_pointer(); },
                on:pointer_move = move |event| { if let Some((x, width)) = resize.get_untracked() { workbench.sidebar_width.set((width + event.position.x.0 - x).clamp(220.0, 480.0)); } },
                on:pointer_up = move |event| { resize.set(None); event.release_pointer(); },
                on:pointer_cancel = move |event| { resize.set(None); event.release_pointer(); },
                on:key_down = move |event| {
                    let change = match event.key { zgui::vocab::Key::Named(zgui::vocab::NamedKey::ArrowLeft) => -16.0, zgui::vocab::Key::Named(zgui::vocab::NamedKey::ArrowRight) => 16.0, _ => return };
                    workbench.sidebar_width.update(|width| *width = (*width + change).clamp(220.0, 480.0)); event.prevent_default(); event.stop_propagation();
                }) {}
        }
    }
}
