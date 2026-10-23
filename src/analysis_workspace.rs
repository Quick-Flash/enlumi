use crate::copyable_text::CopyableTextProps;
use crate::{
    action_button::ActionButtonProps, analysis_source::AnalysisSourceProps, state::Workbench,
};
use zgui::prelude::*;
use zgui_ui::prelude::*;

#[component]
pub fn AnalysisWorkspace(workbench: Workbench) -> impl IntoView {
    view! {for current in move ||vec![workbench.view()],key=|view|view.id {
        ViewAnalysisWorkspace(workbench=workbench.for_view(current.id).expect("View exists"))
    }}
}

#[component]
fn ViewAnalysisWorkspace(workbench: Workbench) -> impl IntoView {
    install_stylesheet("enlumi-analysis", include_str!("analysis.css"));
    let ui = workbench.analysis();
    let sizes = RwSignal::new_local(Vec::<usize>::new());
    let sizing = RwSignal::new_local(false);
    let size_revision = RwSignal::new_local(0_u64);
    let available_sync = zgui::reactive::RenderEffect::new(move |_| {
        let job = workbench.analysis_size_job();
        let revision = size_revision.get_untracked().wrapping_add(1);
        size_revision.set(revision);
        sizes.set(Vec::new());
        sizing.set(true);
        workbench.task(async move {
            let available = blocking(job).await;
            if size_revision.try_get_untracked() == Some(revision) {
                sizes.set(available);
                sizing.set(false);
            }
        });
    });
    on_cleanup_local(move || drop(available_sync));
    let size_sync = zgui::reactive::RenderEffect::new(move |_| {
        let available = sizes.get();
        let requested = ui.size.get().parse::<usize>().unwrap_or(2048);
        if !available.contains(&requested)
            && let Some(size) = available
                .iter()
                .copied()
                .filter(|size| *size <= requested)
                .max()
                .or_else(|| available.first().copied())
        {
            ui.size.set(size.to_string());
        }
    });
    on_cleanup_local(move || drop(size_sync));
    view! {
        column(class = "analysis-workspace") {
            row(class = "analysis-heading") {
                text(class = "heading") {"Frequency analysis"}
                ActionButton(on:click=move |_|{ui.open.set(false);ui.hover.set(false);ui.picking_range.set(false);}) {"Close"}
            }
            row(class = "analysis-body") {
                column(class = "analysis-controls") {
                    text(class = "muted") {{move || ui.target.get().map_or_else(||"New analysis · drag signals onto a plot to create results".into(),|id|format!("Editing plot {id} · Update plot applies settings; choosing another type starts new analysis"))}}
                    row(class="analysis-context-actions") {
                        ActionButton(class="analysis-new",disabled=Signal::derive_local(move ||ui.busy.get()),on:click=move |_|workbench.new_analysis()) {"New analysis"}
                        ActionButton(class="analysis-edit-selected",class:setting-active=move ||ui.target.get()==Some(workbench.active().get()),disabled=Signal::derive_local(move ||{let active=workbench.active().get();ui.busy.get() ||ui.target.get()==Some(active)||workbench.find(active).is_none_or(|pane|pane.analysis.get().is_none())}),on:click=move |_|workbench.edit_analysis(workbench.pane())) {"Edit selected plot"}
                    }
                    row(class="analysis-modes",a11y:label="Analysis type") {
                        for mode in move || [("fft", "FFT amplitude"), ("psd", "PSD"), ("heatmap", "FFT vs field heatmap"), ("spectrogram", "Spectrogram")],key=|mode|mode.0 {
                            ActionButton(class="analysis-mode",class:setting-active=move ||ui.mode.get()==mode.0,on:click=move |_|workbench.select_analysis_mode(mode.0)) {{mode.1}}
                        }
                    }
                    text(class="muted") {"Signals · drag onto a plot to analyze"}
                    scroll(class="analysis-inputs",node_ref=ui.drop,class:analysis-drop-active=move ||ui.hover.get(),a11y:label="Draggable analysis signals") {
                        if move || ui.sources.with(Vec::is_empty) { text(class="analysis-drop-empty muted") {"Drop signal fields or profile views here"} }
                        for source in move ||ui.sources.get(),key=|source|source.clone() {
                            AnalysisSource(workbench=workbench,source=source)
                        }
                    }
                    if move || matches!(ui.mode.get().as_str(), "heatmap" | "spectrogram") {
                    Select(value = ui.size) {
                        SelectTrigger(label="FFT window size") {SelectValue()}
                        SelectContent {for size in move || sizes.get(),key=|size|*size {SelectItem(value=size.to_string()) {{size.to_string()}}}}
                    }
                    if move || sizing.get() { text(class="muted") {"Checking available windows…"} }
                    if move || !sizing.get() && sizes.get().is_empty() { text(class="muted") {"No usable FFT window in selected signals/range."} }
                    }
                    Select(value = ui.window) {
                        SelectTrigger(label="Window function") {SelectValue()}
                        SelectContent {
                            SelectItem(value="hann") {"Hann"}
                            SelectItem(value="hamming") {"Hamming"}
                            SelectItem(value="blackman") {"Blackman"}
                            SelectItem(value="rectangular") {"Rectangular"}
                        }
                    }
                    {{let dc = NodeRef::new();view! {
                        row(class="analysis-option") {
                            Checkbox(node_ref=dc,checked=Binding::controlled(Signal::derive_local(move ||if ui.remove_dc.get(){Checked::Yes}else{Checked::No}),move |value|ui.remove_dc.set(value==Checked::Yes)))
                            Label(control=dc) {"Remove DC (subtract mean)"}
                        }
                    }}}
                    if move || matches!(ui.mode.get().as_str(), "heatmap" | "spectrogram") {
                    Select(value = ui.overlap) {
                        SelectTrigger(label="Window overlap") {SelectValue()}
                        SelectContent {
                            SelectItem(value="0") {"0%"}
                            SelectItem(value="25") {"25%"}
                            SelectItem(value="50") {"50%"}
                            SelectItem(value="75") {"75%"}
                        }
                    }
                    }
                    text(class = "muted") {{move || {
                        let window = match ui.window.get().as_str() {"hamming"=>"Hamming","blackman"=>"Blackman","rectangular"=>"Rectangular",_=>"Hann"};
                        if matches!(ui.mode.get().as_str(), "heatmap" | "spectrogram") { format!("{} samples · {window} · {}% overlap", ui.size.get(), ui.overlap.get()) } else { format!("Longest valid run · {window} · no zero padding") }
                    }}}
                    text(class = "muted") {"Time range (log X seconds; blank = full signal)"}
                    row {Input(value=ui.start,placeholder="Start",a11y:label="Analysis start seconds") Input(value=ui.end,placeholder="End",a11y:label="Analysis end seconds")}
                    row(class="analysis-range-actions") {
                        ActionButton(class="analysis-pick-range",class:setting-active=move ||ui.picking_range.get(),on:click=move |_|{workbench.finish_geometry_editing();workbench.measurement.active.set(false);ui.picking_range.update(|picking|*picking = !*picking);}) {"Select on plot"}
                        ActionButton(class="analysis-visible-range",on:click=move |_|workbench.use_visible_analysis_range()) {"Use visible range"}
                        ActionButton(on:click=move |_|{ui.start.set(String::new());ui.end.set(String::new());ui.picking_range.set(false);}) {"Full signal"}
                    }
                    if move ||ui.picking_range.get() {text(class="muted",a11y:role=Role::Status) {"Drag across a time plot to select range. Escape cancels."}}
                    if move ||ui.mode.get()=="heatmap" {
                        text(class="muted") {"Conditioning field · mean value per FFT window"}
                        column(class="analysis-condition-drop",node_ref=ui.condition_drop,class:analysis-drop-active=move ||ui.condition_hover.get(),a11y:label="Drop heatmap conditioning field") {
                            text {{move || if ui.condition.get().is_empty() { "Drop throttle, RPM, or another field here".into() } else { workbench.field_name(&ui.condition.get()) }}}
                            ActionButton(disabled=Signal::derive_local(move ||ui.condition.get().is_empty()),on:click=move |_|{ui.cancel();ui.condition.set(String::new());}) {"Remove conditioning field"}
                        }
                        Select(value=ui.heatmap_bins) {
                            SelectTrigger(label="Heatmap conditioning bins") {SelectValue()}
                            SelectContent {
                                SelectItem(value="20") {"20 bins"}
                                SelectItem(value="40") {"40 bins"}
                                SelectItem(value="80") {"80 bins"}
                                SelectItem(value="160") {"160 bins"}
                                SelectItem(value="320") {"320 bins"}
                            }
                        }
                        text(class="muted") {"X = frequency; Y = conditioning field; color = PSD in dB. Drag signal from list onto a plot to create heatmap."}
                    }
                    if move ||ui.mode.get()=="spectrogram" {
                        text(class="muted") {"X = time; Y = frequency; color = PSD in dB. Time bin spacing follows FFT window size and overlap."}
                    }
                    row {
                        ActionButton(class="analysis-update",disabled=Signal::derive_local(move ||ui.target.get().is_none() ||ui.sources.with(Vec::is_empty)||ui.busy.get() || (matches!(ui.mode.get().as_str(), "heatmap" | "spectrogram") && sizes.get().is_empty())),on:click=move |_|workbench.run_analysis()) {"Update plot"}
                        ActionButton(disabled=Signal::derive_local(move ||!ui.busy.get()),on:click=move |_|{ui.cancel();ui.message.set("Cancelled".into());}) {"Cancel"}
                        ActionButton(on:click=move |_|ui.reset()) {"Clear"}
                    }
                    text(class="muted") {"Results open as dock tabs. Drag tabs above, below, or beside other plots; use Links to share axes and pointer. Results are snapshots."}
                    if move ||!ui.message.with(String::is_empty) {CopyableText(class="analysis-summary muted",label="Analysis status and errors",value=ui.message)}
                }

            }
        }
    }
}
