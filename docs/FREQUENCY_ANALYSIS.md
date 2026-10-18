# Frequency analysis

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
Up to 16 enabled signals run together; same known unit overlays curves, while
different or unknown units produce separate plots. Metadata labels never convert
values: use converted profile/filter outputs when physical units are needed.

## Modes

- **FFT amplitude:** one-sided peak-calibrated amplitude for each Hann window,
  combined using RMS across windows. Values retain input unit. This is averaged
  windowed amplitude, not one full-record complex transform or phase response.
- **Welch PSD:** mean of one-sided, window-energy-normalized periodograms.
  Values have input-unit squared per Hz. Integrating PSD over frequency estimates
  windowed signal variance; it is different from amplitude squared per bin.
- **FFT vs field heatmap:** frequency on X, window-mean conditioning value on Y,
  mean PSD per conditioning bin as color. Add throttle, RPM, or another numeric
  field to inputs and choose it as conditioner. Uncheck it if its own spectrum
  is unwanted. Conditioner does not need same sample rate as analyzed signal,
  but must cover complete analyzed windows without gaps. Heatmaps have 40 equal
  conditioning bins; each populated bin averages its own contributing windows.
  Empty or zero-power bins are blank. Color is dB relative to 1 input-unit²/Hz,
  with per-plot auto color range shown above plot. Hover reads frequency, Y, dB.

Choose window size, optional time bounds, then **Run**. Results list measured
sample rate, frequency resolution, accepted windows, and skipped windows.
Plots support normal pan/zoom. **Cancel** stops active calculation. Results are
snapshots; rerun after changing filters, metadata, inputs, or analysis settings.

## Sampling and scaling

Median positive timestamp difference locates nominal cadence; averaging intervals
within 10% of median refines sample interval against timestamp quantization. Values linearly
resample onto that interval to handle timestamp jitter. Interpolation never
extrapolates, bridges gaps above 1.5 nominal sample intervals, or crosses a
nonfinite endpoint. Windows containing missing samples are skipped. No samples
are replaced with zero to hide gaps. Strongly irregular sampling is unsuitable
for interpreting high-frequency content through this interpolation method.

Each complete window has its mean removed and periodic Hann weighting applied;
overlap is 50%. For N samples, sample rate fs, and window w:

- Frequency bin k is `k * fs / N`; maximum is Nyquist `fs / 2`.
- FFT amplitude is `factor * abs(FFT) / sum(w)` per window.
- PSD is `factor * abs(FFT)^2 / (fs * sum(w^2))` per window.
- `factor = 2` for interior positive-frequency bins, 1 for DC and Nyquist.
- Output FFT is square root of mean squared window amplitudes; output PSD is
  arithmetic mean of window PSDs.

Only complete windows contribute; trailing partial windows are omitted.
Short intervals, static parameters without sufficient samples, unavailable
derived fields, and conditioners without varying values report errors per signal.
Analysis is bounded to four million resampled points per signal and eight million
temporary heatmap spectrum cells. Narrow time range when limits are exceeded.

Numerical implementation and native UI compile/lint checks are complete. Numerical
reference comparisons and graphical interaction checks have not been run.

## References

- [BlackBox_CSV_Render](https://github.com/nerdCopter/BlackBox_CSV_Render) provides
  workflow inspiration for spectra, PSD, and condition-dependent frequency views.
- [SciPy Welch documentation](https://docs.scipy.org/doc/scipy/reference/generated/scipy.signal.welch.html)
  describes density scaling, one-sided bins, and Hann overlap convention.
- [RustFFT](https://docs.rs/rustfft/6.4.1/rustfft/) supplies forward transforms.
  Enlumi applies normalization explicitly. No analyzer code was copied from
  BlackBox_CSV_Render.
