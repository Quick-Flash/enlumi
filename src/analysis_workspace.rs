use crate::{action_button::ActionButtonProps, analysis_plot::AnalysisPlotProps, state::Workbench};
use zgui::prelude::*;
use zgui_ui::prelude::*;

#[component]
pub fn AnalysisWorkspace(workbench: Workbench) -> impl IntoView {
    install_stylesheet("enlumi-analysis", include_str!("analysis.css"));
    let ui = workbench.analysis;
    view! {
        column(class = "analysis-workspace", node_ref = ui.drop, class:analysis-drop-active = move || ui.hover.get()) {
            row(class = "analysis-heading") {
                text(class = "heading") {"Frequency analysis"}
                ActionButton(on:click=move |_|{ui.open.set(false);ui.hover.set(false);}) {"Back to plots"}
            }
            row(class = "analysis-body") {
                column(class = "analysis-controls") {
                    text(class = "muted") {"Drag fields or profile views here. Enable individual signals below."}
                    Select(value = ui.mode) {
                        SelectTrigger(label="Analysis type") {SelectValue()}
                        SelectContent {
                            SelectItem(value="fft") {"FFT amplitude"}
                            SelectItem(value="psd") {"Welch PSD"}
                            SelectItem(value="heatmap") {"FFT vs field heatmap"}
                            SelectItem(value="spectrogram") {"FFT over time (spectrogram)"}
                        }
                    }
                    Select(value = ui.size) {
                        SelectTrigger(label="FFT window size") {SelectValue()}
                        SelectContent {for size in ||[128,256,512,1024,2048,4096,8192,16384],key=|size|*size {SelectItem(value=size.to_string()) {{size.to_string()}}}}
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
                    Select(value = ui.overlap) {
                        SelectTrigger(label="Window overlap") {SelectValue()}
                        SelectContent {
                            SelectItem(value="0") {"0%"}
                            SelectItem(value="25") {"25%"}
                            SelectItem(value="50") {"50%"}
                            SelectItem(value="75") {"75%"}
                        }
                    }
                    text(class = "muted") {{move || {
                        let window = match ui.window.get().as_str() {"hamming"=>"Hamming","blackman"=>"Blackman","rectangular"=>"Rectangular",_=>"Hann"};
                        format!("{}-sample {window} window · {}% overlap · remove mean per window", ui.size.get(), ui.overlap.get())
                    }}}
                    text(class = "muted") {"Time range (log X seconds; blank = full signal)"}
                    row {Input(value=ui.start,placeholder="Start",a11y:label="Analysis start seconds") Input(value=ui.end,placeholder="End",a11y:label="Analysis end seconds")}
                    if move ||ui.mode.get()=="heatmap" {
                        text(class="muted") {"Conditioning field · mean value per FFT window"}
                        Select(value=ui.condition) {
                            SelectTrigger(label="Heatmap conditioning field") {SelectValue(placeholder="Choose input field")}
                            SelectContent {
                                for source in move ||ui.sources.get(),key=|source|source.clone() {SelectItem(value=source.clone()) {{workbench.field_name(&source)}}}
                            }
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
                        text(class="muted") {"X = frequency; Y = conditioning field; color = PSD in dB. Add throttle/RPM as input, then choose it here."}
                    }
                    if move ||ui.mode.get()=="spectrogram" {
                        text(class="muted") {"X = time; Y = frequency; color = PSD in dB. Time bin spacing follows FFT window size and overlap."}
                    }
                    row {
                        ActionButton(disabled=Signal::derive_local(move ||ui.busy.get()),on:click=move |_|workbench.run_analysis()) {"Run"}
                        ActionButton(disabled=Signal::derive_local(move ||!ui.busy.get()),on:click=move |_|{ui.cancel();ui.message.set("Cancelled".into());}) {"Cancel"}
                        ActionButton(on:click=move |_|ui.reset()) {"Clear"}
                    }
                    text(class="muted") {"Same-unit signals overlay; different or unknown units get separate plots. Results are snapshots: rerun after changes."}
                    scroll(class="analysis-inputs",a11y:label="Individual analysis signals") {
                        for source in move ||ui.sources.get(),key=|source|source.clone() {
                            {{let source=StoredValue::new_local(source);let control=NodeRef::new();view! {
                                row(class="analysis-input") {
                                    Checkbox(node_ref=control,checked=Binding::controlled(Signal::derive_local(move ||if ui.disabled.with(|ids|ids.contains(&source.get_value())){Checked::No}else{Checked::Yes}),move |value|{ui.disabled.update(|ids|{if value==Checked::Yes{ids.remove(&source.get_value());}else{ids.insert(source.get_value());}});}))
                                    Label(control=control) {{move ||workbench.field_name(&source.get_value())}}
                                    ActionButton(icon=true,a11y:label="Remove analysis input",on:click=move |_|{let id=source.get_value();ui.cancel();ui.sources.update(|all|all.retain(|source|source!=&id));ui.disabled.update(|all|{all.remove(&id);});if ui.condition.get_untracked()==id{ui.condition.set(String::new());}}) {"×"}
                                }
                            }}}
                        }
                    }
                }
                scroll(class="analysis-results",a11y:label="Spectral results") {
                    text(class="analysis-summary muted",a11y:role=Role::Status) {{move ||ui.message.get()}}
                    for result in move ||ui.results.get(),key=|result|(result.id,result.title.clone(),result.units.clone()) {AnalysisPlot(result=result)}
                }
            }
        }
    }
}
