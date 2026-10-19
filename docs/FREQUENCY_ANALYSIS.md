# Frequency analysis

Select fields or profile views in sidebar, then click **Create FFTs** or
**Create PSDs**. Each selected signal opens its own spectral plot; profile groups
expand into their member signals. No time plot or extra Run click needed.
Choose **Configure** to adjust settings before running. **One plot per signal**
creates separate plots; turn it off to overlay signals with matching known units.

Open **Analyze** from toolbar, or use plot header's **Analyze** to copy visible
plot signals and current X range into analysis workspace. Existing time plots
remain intact. Toolbar Analyze starts with full signal range unless bounds were
previously entered. Time bounds use same log X coordinates as time plots.

Drag raw fields, derived fields, profile fields, or whole profile views into
workspace. A view contributes its available member signals, not their sum or
concatenation. Duplicate sources are removed. Missing profiles prepare in sequence;
failed profiles remain diagnostic entries while usable selections continue.

Expand arrow beside profile view to see individual available member fields.
Each member supports same selection and dragging as other profile outputs.
This works for literal view sources as well as named profile aliases.

Input checkboxes control which signals run. Remove button removes an input.
Enabled signals compute sequentially in one cancellable batch; same known unit overlays curves, while
different or unknown units produce separate plots. Metadata labels never convert
values: use converted profile/filter outputs when physical units are needed.

## Modes

- **FFT amplitude:** one-sided peak-calibrated amplitude from one transform over
  all uniformly resampled samples in selected contiguous range. Transform length
  follows data, including non-power-of-two and odd lengths. No zero padding.
  Values retain input unit.
- **PSD:** one-sided, window-energy-normalized power density over full selected
  contiguous range, using same arbitrary transform length. Values have
  input-unit squared per Hz. Integrating PSD estimates windowed signal variance.
- **FFT vs field heatmap:** frequency on X, window-mean conditioning value on Y,
  mean PSD per conditioning bin as color. Add throttle, RPM, or another numeric
  field to inputs and choose it as conditioner. Uncheck it if its own spectrum
  is unwanted. Conditioner does not need same sample rate as analyzed signal,
  but must cover complete analyzed windows without gaps. Choose 20, 40, 80, 160,
  or 320 equal conditioning bins; each populated bin averages its contributing
  windows. Empty or zero-power bins are blank. Color is dB relative to 1
  input-unit²/Hz, with per-plot auto color range shown above plot.
- **FFT over time (spectrogram):** time on X, frequency on Y, PSD in dB as color.
  Each complete FFT window creates one time bin; window size sets frequency
  resolution and overlap sets time spacing. Windows with missing samples stay
  blank. Color range auto-scales per signal.

Choose window function and optional time bounds, then **Run**. Hann is default.
FFT size and overlap (0%, 25%, 50%, or 75%) appear only for heatmaps and
spectrograms. Size choices require complete windows in every enabled input and
must fit frequency-map memory limits. Mean removal stays enabled per transform. Results list measured sample rate, frequency resolution,
accepted windows, and skipped windows. Plots support normal pan/zoom.
**Cancel** stops active calculation. Results are snapshots; rerun after changing
filters, metadata, inputs, or analysis settings.

Analyze opens a resizable settings panel beside docking workspace. **Run** creates
result tabs inside same docking workspace as normal plots. Drag tabs to place
spectra above, below, or beside other plots; rename, duplicate, maximize, and close
with normal tab controls. Closing Analyze keeps result plots.

Each result has **Links** controls for X/Y bounds and pointer. Spectrograms use
same time group as normal plots. FFT, PSD, and field heatmaps use a frequency X
group, preventing accidental links between seconds and Hz. Bounds and pointer
links start disabled on new analysis plots; enable intended links explicitly.

## Sampling and scaling

Median positive timestamp difference locates nominal cadence; averaging intervals
within 10% of median refines sample interval against timestamp quantization. Values linearly
resample onto that interval to handle timestamp jitter. Interpolation never
extrapolates, bridges gaps above 1.5 nominal sample intervals, or crosses a
nonfinite endpoint. Heatmap/spectrogram windows containing missing samples are skipped. Full-range
FFT/PSD reports gaps and asks for a contiguous range instead of dropping samples. No samples
are replaced with zero to hide gaps. Strongly irregular sampling is unsuitable
for interpreting high-frequency content through this interpolation method.

Each complete window has its mean removed. The selected window function is
periodic Hann, Hamming, Blackman, or rectangular. Map window overlap is selectable. For N
samples, sample rate fs, and window w:

- Frequency bin k is `k * fs / N`, for k from 0 through floor(N/2).
  Odd transforms end below Nyquist; even transforms include Nyquist.
- FFT amplitude is `factor * abs(FFT) / sum(w)` per window.
- PSD is `factor * abs(FFT)^2 / (fs * sum(w^2))` per window.
- `factor = 2` for interior positive-frequency bins, 1 for DC and Nyquist.
- FFT/PSD has one full-range transform. Maps average window PSDs within bins.

Maps use complete windows; trailing partial map windows are omitted. Full-range
FFT/PSD uses every sample in selected contiguous range.
Short intervals, static parameters without sufficient samples, unavailable
derived fields, and conditioners without varying values report errors per signal.
Analysis is bounded to four million resampled points per signal and eight million
frequency-map cells. Narrow time range, reduce heatmap bins, or increase FFT size
when limits are exceeded.

Regression checks cover odd-length amplitude scaling, PSD integrated power,
selected time ranges, gap handling, docking, and links across matching X domains.

## References

- [BlackBox_CSV_Render](https://github.com/nerdCopter/BlackBox_CSV_Render) provides
  workflow inspiration for spectra, PSD, and condition-dependent frequency views.
- [SciPy Welch documentation](https://docs.scipy.org/doc/scipy/reference/generated/scipy.signal.welch.html)
  describes density scaling, one-sided bins, and Hann overlap convention.
- [RustFFT](https://docs.rs/rustfft/6.4.1/rustfft/) supplies forward transforms.
  Enlumi applies normalization explicitly. No analyzer code was copied from
  BlackBox_CSV_Render.

FFT/PSD plots share normal plot legend and line settings: color, width, line
pattern, markers, visibility, removal, and drag ordering. Long legends scroll.
Spectral lines stay separate from time-domain lines and other analysis modes.

FFT/PSD always render Steps pre. Connection controls are omitted. Use
**Analysis settings** in plot header or **FFT / PSD settings** in legend line
settings to restore that plot's window and time range. **Update plot** recomputes
in place, retaining line colors and other appearance settings.
