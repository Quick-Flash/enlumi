"""Compare two release replay executables with identical local inputs."""

import argparse
import csv
import hashlib
import json
import math
import os
from pathlib import Path
import statistics
import subprocess


def percentile(values, fraction=0.95):
    return sorted(values)[max(0, math.ceil(len(values) * fraction) - 1)]


def compare_images(directory):
    comparisons = {}
    for path in sorted((directory / "baseline-images").glob("*.ppm")):
        other = directory / "candidate-images" / path.name
        def read(image):
            with image.open("rb") as file:
                assert file.readline() == b"P6\n"
                width, height = map(int, file.readline().split())
                assert file.readline() == b"255\n"
                pixels = file.read()
                assert len(pixels) == width * height * 3
                return width, height, pixels
        width, height, before = read(path)
        other_width, other_height, after = read(other)
        assert (width, height) == (other_width, other_height)
        # The top toolbar contains the continuously changing FPS readout.
        differences = [abs(a - b) for a, b in zip(before[width * 60 * 3:], after[width * 60 * 3:])]
        comparisons[path.stem] = {
            "excluded_top_rows": 60, "max_channel_difference": max(differences),
            "mean_channel_difference": statistics.mean(differences),
            "channels_differing_by_more_than_one": sum(d > 1 for d in differences),
        }
    return comparisons


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("baseline", "candidate", "log", "profile", "out"):
        parser.add_argument(f"--{name}", type=Path, required=True)
    parser.add_argument("--runs", type=int, default=3)
    args = parser.parse_args()
    if args.runs < 1:
        parser.error("--runs must be positive")
    args.out.mkdir(parents=True, exist_ok=True)
    env = os.environ.copy()
    for name in ("ZGUI_LATENCY", "ZGUI_COALESCE_DAMAGE", "ENLUMI_PERF_LOG",
                 "ENLUMI_REPLAY_SCREENSHOTS"):
        env.pop(name, None)
    env.update(ENLUMI_REPLAY_LOG=str(args.log.resolve()),
               ENLUMI_REPLAY_PROFILE=str(args.profile.resolve()),
               ENLUMI_REPLAY_GPU="1", ENLUMI_REPLAY_FULL="1")
    results = {"baseline": {}, "candidate": {}}
    order = []
    for run in range(1, args.runs + 1):
        labels = ("baseline", "candidate") if run % 2 else ("candidate", "baseline")
        for label in labels:
            name = f"{label}-{run}"
            order.append(name)
            output = args.out / f"{name}.csv"
            env["ENLUMI_REPLAY_OUT"] = str(output.resolve())
            if run == 1:
                env["ENLUMI_REPLAY_SCREENSHOTS"] = str((args.out / f"{label}-images").resolve())
            else:
                env.pop("ENLUMI_REPLAY_SCREENSHOTS", None)
            print(f"Running {name}", flush=True)
            with (args.out / f"{name}.log").open("w", encoding="utf-8") as log:
                subprocess.run([str(getattr(args, label).resolve()),
                                "replay_attitude_drag", "--ignored", "--nocapture",
                                "--test-threads=1"], env=env, stdout=log,
                               stderr=subprocess.STDOUT, check=True)
            with output.open(newline="") as file:
                cases = {}
                for row in csv.DictReader(file):
                    cases.setdefault(row["case"], []).append(row)
            for case, rows in cases.items():
                summary = {"frames": len(rows)}
                for metric in ("frame_ms", "paint_submit_ms", "gpu_wait_ms"):
                    values = [float(row[metric]) for row in rows]
                    summary[f"{metric}_p95"] = percentile(values)
                    summary[f"{metric}_mean"] = statistics.mean(values)
                summary["draw_calls_mean"] = statistics.mean(float(row["draw_calls"]) for row in rows)
                summary["uploaded_bytes_mean"] = statistics.mean(float(row["uploaded_bytes"]) for row in rows)
                results[label].setdefault(case, []).append(summary)
    comparison = {}
    for case in results["baseline"]:
        comparison[case] = {}
        for metric in results["baseline"][case][0]:
            before = [row[metric] for row in results["baseline"][case]]
            after = [row[metric] for row in results["candidate"][case]]
            baseline, candidate = statistics.median(before), statistics.median(after)
            comparison[case][metric] = {
                "baseline_median": baseline, "candidate_median": candidate,
                "change_percent": (candidate / baseline - 1) * 100 if baseline else None,
                "baseline_range": [min(before), max(before)],
                "candidate_range": [min(after), max(after)],
            }
    def digest(path):
        with path.open("rb") as file:
            return hashlib.file_digest(file, "sha256").hexdigest()
    report = {"order": order, "input_sha256": {"log": digest(args.log), "profile": digest(args.profile)},
              "executable_sha256": {label: digest(getattr(args, label)) for label in results},
              "runs": results, "comparison": comparison,
              "image_comparison": compare_images(args.out)}
    (args.out / "summary.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    for case, metrics in comparison.items():
        metric = metrics["frame_ms_p95"]
        print(f"{case}: {metric['baseline_median']:.2f} -> "
              f"{metric['candidate_median']:.2f} ms ({metric['change_percent']:+.1f}%)")


if __name__ == "__main__":
    main()
