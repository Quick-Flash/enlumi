use crate::{
    action_button::ActionButtonProps,
    profile::identity::{Drag, Item, ProfileKey},
    profile_library::ProfileFile,
    profile_output_row::ProfileOutputRowProps,
    state::Workbench,
};
use zgui::prelude::*;

#[component]
pub fn ProfileOutputs(
    workbench: Workbench,
    file: StoredValue<ProfileFile, LocalStorage>,
    query: Signal<String, LocalStorage>,
) -> impl IntoView {
    let key = Memo::new(move |_| {
        ProfileKey::new(
            file.with_value(|f| f.path.clone()),
            workbench.profiles.flight.get(),
        )
    });
    let instance = Signal::derive_local(move || {
        workbench
            .profiles
            .instances
            .with(|all| all.get(&key.get()).cloned())
    });
    let details = RwSignal::new_local(false);
    let sections = file.with_value(|file| {
        vec![
            (
                "Views",
                file.views
                    .iter()
                    .map(|(name, count)| (Item::View(name.clone()), name.clone(), *count))
                    .collect::<Vec<_>>(),
            ),
            (
                "Fields",
                file.fields
                    .iter()
                    .map(|(id, name)| (Item::Field(format!("${id}")), name.clone(), 0))
                    .collect(),
            ),
            (
                "Geometry",
                file.geometry
                    .iter()
                    .map(|(id, name)| (Item::Geometry(id.clone()), name.clone(), 0))
                    .collect(),
            ),
        ]
    });
    let resource_query = Signal::derive_local(move || {
        let query = query.get().trim().to_lowercase();
        if file.with_value(|file| {
            file.name.to_lowercase().contains(&query)
                || file.path.to_string_lossy().to_lowercase().contains(&query)
        }) {
            String::new()
        } else {
            query
        }
    });
    view! {
        column(class="profile-outputs") {
            row(class="profile-summary") {
                text(class="profile-status muted") {{move ||if workbench.profiles.loading_profile.get().as_ref()==Some(&key.get()){"Preparing…".into()}else{instance.with(|p|match p {
                    Some(p) if p.diagnostics.is_empty()=>"Ready".into(),
                    Some(p)=>format!("Partial · {} skipped",p.diagnostics.len()),
                    None=>if file.with_value(|f|f.compatible.is_ok()){"Available".into()}else{"Unavailable".into()},
                })}}}
                ActionButton(class="profile-details-toggle",a11y:label="Profile match and skipped output details",on:click=move |_|details.update(|open|*open = !*open)) {{move ||if details.get(){"Hide details"}else{"Details"}}}
            }
            text(class="warnings") {{move ||workbench.profiles.failures.with(|all|all.get(&key.get()).cloned().unwrap_or_default())}}
            if move ||details.get() {
                text(class="profile-diagnostics muted") {{move ||file.with_value(|file|file.matched.clone())}}
                text(class="profile-diagnostics muted") {{move ||instance.with(|p|p.as_ref().map(|p|p.diagnostics.iter().map(|d|format!("{}{}: {}{}",d.resource,d.source.as_ref().map(|s|format!(" [{s}]")).unwrap_or_default(),d.message,if d.dependents.is_empty(){String::new()}else{format!("; also {}",d.dependents.join(", "))})).collect::<Vec<_>>().join("\n")).unwrap_or_default())}}
            }
            for section in move ||sections.clone(),key=|section|section.0 {
                {{let label=section.0;
                let entries=StoredValue::new_local(section.1);
                let open=RwSignal::new_local(label=="Views");
                let matching=Memo::new(move |_| {
                    let query=resource_query.get();
                    entries.with_value(|entries|entries.iter().filter(|(_,name,_)|name.to_lowercase().contains(&query)).cloned().collect::<Vec<_>>())
                });
                view! {
                    if move ||!matching.with(Vec::is_empty) {
                        column(class="profile-section") {
                            ActionButton(class="profile-output-section",{..Attrs::new().a11y_from(A11yBinding::new(Role::Button).label(format!("Toggle profile {label}")).expanded(move ||open.get()||!resource_query.with(String::is_empty)))},on:click=move |_|open.update(|open|*open = !*open)) {
                                {move ||format!("{} {label} ({})",if open.get()||!resource_query.with(String::is_empty){"▾"}else{"▸"},matching.with(Vec::len))}
                            }
                            if move ||open.get()||!resource_query.with(String::is_empty) {
                                for entry in move ||matching.get(),key=|entry|entry.0.clone() {
                                    ProfileOutputRow(workbench=workbench,file=file,drag=Signal::derive_local(move ||Drag{profile:key.get(),item:entry.0.clone()}),name=entry.1,count=entry.2)
                                }
                            }
                        }
                    }
                }}}
            }
        }
    }
}
