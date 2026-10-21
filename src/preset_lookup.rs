use crate::{
    preset::{PlotPreset, ViewPreset},
    profile::Profile,
};
pub fn is_nested_plot(name: &str) -> bool {
    name.starts_with("@workspace/") || name.starts_with("@workspace-set/")
}

pub fn view_key(workspace: &str, index: usize) -> String {
    format!("@workspace-view/{workspace}/{index}")
}
pub fn plot_key(view: &str, index: usize) -> String {
    view.strip_prefix("@workspace-view/").map_or_else(
        || format!("@workspace/{view}/{index}"),
        |view| format!("@workspace-set/{view}/{index}"),
    )
}
pub fn view<'a>(profile: &'a Profile, name: &str) -> Option<&'a ViewPreset> {
    if let Some((workspace, index)) = name
        .strip_prefix("@workspace-view/")
        .and_then(|name| name.rsplit_once('/'))
    {
        return profile
            .workspace_presets
            .iter()
            .find(|item| item.name == workspace)?
            .views
            .get(index.parse::<usize>().ok()?);
    }
    profile.view_presets.iter().find(|item| item.name == name)
}
pub fn plot<'a>(profile: &'a Profile, name: &str) -> Option<&'a PlotPreset> {
    profile
        .plot_presets
        .iter()
        .find(|item| item.name == name)
        .or_else(|| {
            profile.view_presets.iter().find_map(|view| {
                view.plots
                    .iter()
                    .enumerate()
                    .find_map(|(index, plot)| (plot_key(&view.name, index) == name).then_some(plot))
            })
        })
        .or_else(|| {
            profile.workspace_presets.iter().find_map(|workspace| {
                workspace
                    .views
                    .iter()
                    .enumerate()
                    .find_map(|(view_index, view)| {
                        view.plots.iter().enumerate().find_map(|(index, plot)| {
                            (plot_key(&view_key(&workspace.name, view_index), index) == name)
                                .then_some(plot)
                        })
                    })
            })
        })
}
