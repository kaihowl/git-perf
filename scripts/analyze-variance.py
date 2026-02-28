#!/usr/bin/env python3
# /// script
# requires-python = ">=3.10"
# dependencies = [
#     "pandas",
#     "matplotlib",
#     "scipy",
# ]
# ///
"""
Analyze cross-runner variance experiment results.

Usage:
    uv run scripts/analyze-variance.py <input_dir> <output_dir>

Input:  directory containing CSV files with columns:
        runner_id,cpu_model,cpu_count,os,run_number,instance,workload,rep_index,duration_ns

Output: summary tables (markdown), plots (PNG), verdict table
"""
import sys
import os
import glob
import math

import pandas as pd
import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt
import matplotlib.ticker as mticker
from scipy import stats


# ---------------------------------------------------------------------------
# Configuration
# ---------------------------------------------------------------------------
AGGREGATION_METHODS = ["min", "median", "mean", "max"]
DISPERSION_METHODS = ["stddev", "mad"]
SIGMAS = [3.0, 3.5, 5.0, 10.0]
WORKLOADS = ["sha256", "sort", "matrix", "noop"]

# Feasibility thresholds on inter-runner CoV (%)
THRESHOLDS = {"green": 10.0, "yellow": 20.0}


def load_data(input_dir: str) -> pd.DataFrame:
    files = glob.glob(os.path.join(input_dir, "**", "*.csv"), recursive=True)
    files += glob.glob(os.path.join(input_dir, "*.csv"))
    files = list(set(files))
    if not files:
        raise FileNotFoundError(f"No CSV files found in {input_dir}")

    dfs = []
    for f in files:
        try:
            df = pd.read_csv(f)
            dfs.append(df)
        except Exception as e:
            print(f"Warning: could not read {f}: {e}", file=sys.stderr)

    data = pd.concat(dfs, ignore_index=True)
    data["duration_ms"] = data["duration_ns"] / 1e6
    return data


def mad(series: pd.Series) -> float:
    """Median absolute deviation."""
    return (series - series.median()).abs().median()


def cov(series: pd.Series) -> float:
    """Coefficient of variation (%)."""
    m = series.mean()
    if m == 0:
        return float("nan")
    return series.std(ddof=1) / m * 100.0


def compute_runner_aggregates(data: pd.DataFrame) -> pd.DataFrame:
    """
    For each (runner_id, os, workload), aggregate the 30 reps
    using each aggregation method. Returns long-form DataFrame.
    """
    rows = []
    grouped = data.groupby(["runner_id", "os", "workload"])
    for (runner_id, os_name, workload), grp in grouped:
        for agg in AGGREGATION_METHODS:
            if agg == "min":
                val = grp["duration_ms"].min()
            elif agg == "median":
                val = grp["duration_ms"].median()
            elif agg == "mean":
                val = grp["duration_ms"].mean()
            elif agg == "max":
                val = grp["duration_ms"].max()
            rows.append({
                "runner_id": runner_id,
                "os": os_name,
                "workload": workload,
                "agg_method": agg,
                "value_ms": val,
                "cpu_model": grp["cpu_model"].iloc[0],
            })
    return pd.DataFrame(rows)


def within_runner_cov(data: pd.DataFrame) -> pd.DataFrame:
    """CoV of reps within each runner."""
    rows = []
    for (runner_id, os_name, workload), grp in data.groupby(["runner_id", "os", "workload"]):
        c = cov(grp["duration_ms"])
        rows.append({"runner_id": runner_id, "os": os_name, "workload": workload, "cov_pct": c})
    return pd.DataFrame(rows)


def between_runner_cov(agg_df: pd.DataFrame) -> pd.DataFrame:
    """CoV of per-runner aggregated values across runners."""
    rows = []
    for (os_name, workload, agg_method), grp in agg_df.groupby(["os", "workload", "agg_method"]):
        vals = grp["value_ms"].dropna()
        c = cov(vals)
        m = vals.median()
        std = vals.std(ddof=1)
        mad_val = mad(vals)
        rows.append({
            "os": os_name,
            "workload": workload,
            "agg_method": agg_method,
            "inter_cov_pct": c,
            "center_ms": m,
            "stddev_ms": std,
            "mad_ms": mad_val,
            "n_runners": len(vals),
        })
    return pd.DataFrame(rows)


def compute_mde(between_df: pd.DataFrame) -> pd.DataFrame:
    """
    MDE% = sigma * dispersion / center * 100
    """
    rows = []
    for _, row in between_df.iterrows():
        center = row["center_ms"]
        if center == 0 or math.isnan(center):
            continue
        for disp_method in DISPERSION_METHODS:
            disp = row["stddev_ms"] if disp_method == "stddev" else row["mad_ms"]
            if math.isnan(disp):
                continue
            for sigma in SIGMAS:
                mde_pct = sigma * disp / center * 100.0
                rows.append({
                    "os": row["os"],
                    "workload": row["workload"],
                    "agg_method": row["agg_method"],
                    "disp_method": disp_method,
                    "sigma": sigma,
                    "mde_pct": mde_pct,
                    "inter_cov_pct": row["inter_cov_pct"],
                })
    return pd.DataFrame(rows)


def verdict(cov_pct: float) -> str:
    if math.isnan(cov_pct):
        return "N/A"
    if cov_pct < THRESHOLDS["green"]:
        return "GREEN"
    if cov_pct < THRESHOLDS["yellow"]:
        return "YELLOW"
    return "RED"


def write_summary_table(between_df: pd.DataFrame, output_dir: str) -> None:
    path = os.path.join(output_dir, "summary_inter_runner_cov.md")
    lines = ["# Inter-Runner CoV Summary\n",
             "Coefficient of Variation (%) of aggregated values across runners.\n",
             ""]

    for os_name in between_df["os"].unique():
        lines.append(f"## OS: {os_name}\n")
        subset = between_df[between_df["os"] == os_name]
        pivot = subset.pivot_table(
            index="workload", columns="agg_method", values="inter_cov_pct"
        )[AGGREGATION_METHODS]
        lines.append("| Workload | " + " | ".join(AGGREGATION_METHODS) + " | Verdict (min) |")
        lines.append("|" + "|".join(["---"] * (len(AGGREGATION_METHODS) + 2)) + "|")
        for wl in WORKLOADS:
            if wl not in pivot.index:
                continue
            row_vals = [f"{pivot.loc[wl, a]:.2f}%" if a in pivot.columns else "N/A"
                        for a in AGGREGATION_METHODS]
            best_cov = pivot.loc[wl].min()
            v = verdict(best_cov)
            lines.append(f"| {wl} | " + " | ".join(row_vals) + f" | {v} |")
        lines.append("")

    with open(path, "w") as f:
        f.write("\n".join(lines))
    print(f"Wrote {path}")


def plot_mde_heatmap(mde_df: pd.DataFrame, output_dir: str) -> None:
    for os_name in mde_df["os"].unique():
        subset = mde_df[(mde_df["os"] == os_name) & (mde_df["sigma"] == 3.5)]
        if subset.empty:
            continue

        subset = subset.copy()
        subset["config"] = subset["agg_method"] + "/" + subset["disp_method"]
        pivot = subset.pivot_table(index="workload", columns="config", values="mde_pct")

        fig, ax = plt.subplots(figsize=(max(8, len(pivot.columns) * 1.5), 4))
        im = ax.imshow(pivot.values, aspect="auto", cmap="RdYlGn_r", vmin=0, vmax=100)
        ax.set_xticks(range(len(pivot.columns)))
        ax.set_xticklabels(pivot.columns, rotation=45, ha="right")
        ax.set_yticks(range(len(pivot.index)))
        ax.set_yticklabels(pivot.index)
        for r in range(len(pivot.index)):
            for c in range(len(pivot.columns)):
                val = pivot.values[r, c]
                if not math.isnan(val):
                    ax.text(c, r, f"{val:.1f}%", ha="center", va="center", fontsize=8)
        plt.colorbar(im, ax=ax, label="MDE %")
        ax.set_title(f"MDE% at sigma=3.5 — {os_name}")
        ax.set_xlabel("agg_method / disp_method")
        ax.set_ylabel("Workload")
        fig.tight_layout()
        path = os.path.join(output_dir, f"mde_heatmap_{os_name}.png")
        fig.savefig(path, dpi=120)
        plt.close(fig)
        print(f"Wrote {path}")


def plot_box_plots(agg_df: pd.DataFrame, output_dir: str) -> None:
    for os_name in agg_df["os"].unique():
        subset = agg_df[agg_df["os"] == os_name]
        workloads_present = [w for w in WORKLOADS if w in subset["workload"].unique()]
        fig, axes = plt.subplots(1, len(workloads_present),
                                 figsize=(4 * len(workloads_present), 5))
        if len(workloads_present) == 1:
            axes = [axes]
        for ax, wl in zip(axes, workloads_present):
            data_by_agg = [subset[subset["agg_method"] == a]["value_ms"].dropna().values
                           for a in AGGREGATION_METHODS]
            ax.boxplot(data_by_agg, labels=AGGREGATION_METHODS, showfliers=True)
            ax.set_title(wl)
            ax.set_xlabel("Aggregation")
            ax.set_ylabel("Duration (ms)")
        fig.suptitle(f"Distribution of aggregated values per workload — {os_name}")
        fig.tight_layout()
        path = os.path.join(output_dir, f"boxplots_{os_name}.png")
        fig.savefig(path, dpi=120)
        plt.close(fig)
        print(f"Wrote {path}")


def plot_scatter_by_runner(agg_df: pd.DataFrame, output_dir: str) -> None:
    for os_name in agg_df["os"].unique():
        subset = agg_df[(agg_df["os"] == os_name) & (agg_df["agg_method"] == "median")]
        workloads_present = [w for w in WORKLOADS if w in subset["workload"].unique()]
        if not workloads_present:
            continue

        cpu_models = subset["cpu_model"].unique()
        color_map = {m: plt.cm.tab10(i % 10) for i, m in enumerate(cpu_models)}

        fig, axes = plt.subplots(1, len(workloads_present),
                                 figsize=(5 * len(workloads_present), 5))
        if len(workloads_present) == 1:
            axes = [axes]

        for ax, wl in zip(axes, workloads_present):
            wl_subset = subset[subset["workload"] == wl].reset_index(drop=True)
            for idx, row in wl_subset.iterrows():
                color = color_map.get(row["cpu_model"], "gray")
                ax.scatter(idx, row["value_ms"], color=color, s=30)
            ax.set_title(wl)
            ax.set_xlabel("Runner index")
            ax.set_ylabel("Median duration (ms)")

        # Legend (cpu model)
        handles = [plt.Line2D([0], [0], marker="o", color="w",
                               markerfacecolor=color_map[m], label=m, markersize=7)
                   for m in cpu_models]
        fig.legend(handles=handles, loc="lower center", ncol=2,
                   fontsize=7, title="CPU model", bbox_to_anchor=(0.5, -0.05))
        fig.suptitle(f"Median duration by runner — {os_name}")
        fig.tight_layout()
        path = os.path.join(output_dir, f"scatter_by_runner_{os_name}.png")
        fig.savefig(path, dpi=120, bbox_inches="tight")
        plt.close(fig)
        print(f"Wrote {path}")


def plot_cov_vs_reps(data: pd.DataFrame, output_dir: str) -> None:
    rep_counts = [1, 2, 5, 10, 15, 20, 30]
    for os_name in data["os"].unique():
        os_data = data[data["os"] == os_name]
        workloads_present = [w for w in WORKLOADS if w in os_data["workload"].unique()]
        fig, axes = plt.subplots(1, len(workloads_present),
                                 figsize=(4 * len(workloads_present), 5))
        if len(workloads_present) == 1:
            axes = [axes]

        for ax, wl in zip(axes, workloads_present):
            wl_data = os_data[os_data["workload"] == wl]
            for agg_method in ["min", "median", "mean"]:
                covs = []
                for k in rep_counts:
                    runner_aggs = []
                    for runner_id, grp in wl_data.groupby("runner_id"):
                        subset = grp.sort_values("rep_index").head(k)["duration_ms"]
                        if agg_method == "min":
                            runner_aggs.append(subset.min())
                        elif agg_method == "median":
                            runner_aggs.append(subset.median())
                        elif agg_method == "mean":
                            runner_aggs.append(subset.mean())
                    covs.append(cov(pd.Series(runner_aggs)))
                ax.plot(rep_counts, covs, marker="o", label=agg_method)
            ax.set_title(wl)
            ax.set_xlabel("Reps used")
            ax.set_ylabel("Inter-runner CoV (%)")
            ax.legend()
            ax.xaxis.set_major_locator(mticker.MaxNLocator(integer=True))

        fig.suptitle(f"CoV vs. repetitions — {os_name}")
        fig.tight_layout()
        path = os.path.join(output_dir, f"cov_vs_reps_{os_name}.png")
        fig.savefig(path, dpi=120)
        plt.close(fig)
        print(f"Wrote {path}")


def write_verdict_table(between_df: pd.DataFrame, output_dir: str) -> None:
    path = os.path.join(output_dir, "verdict.md")
    lines = ["# Verdict Table\n",
             "Based on best (min) inter-runner CoV across aggregation methods.\n",
             "",
             "| OS | Workload | Best CoV (%) | Best Agg | Verdict |",
             "|---|---|---|---|---|"]

    for os_name in between_df["os"].unique():
        subset = between_df[between_df["os"] == os_name]
        for wl in WORKLOADS:
            wl_subset = subset[subset["workload"] == wl]
            if wl_subset.empty:
                continue
            best_row = wl_subset.loc[wl_subset["inter_cov_pct"].idxmin()]
            v = verdict(best_row["inter_cov_pct"])
            lines.append(
                f"| {os_name} | {wl} | {best_row['inter_cov_pct']:.2f}% "
                f"| {best_row['agg_method']} | {v} |"
            )

    with open(path, "w") as f:
        f.write("\n".join(lines))
    print(f"Wrote {path}")


def write_within_runner_summary(within_df: pd.DataFrame, output_dir: str) -> None:
    path = os.path.join(output_dir, "within_runner_cov.md")
    lines = ["# Within-Runner CoV Summary\n",
             "Median and IQR of per-runner CoV (%) across all runners.\n",
             "",
             "| OS | Workload | Median CoV (%) | IQR |",
             "|---|---|---|---|"]

    for os_name in within_df["os"].unique():
        subset = within_df[within_df["os"] == os_name]
        for wl in WORKLOADS:
            wl_subset = subset[subset["workload"] == wl]["cov_pct"].dropna()
            if wl_subset.empty:
                continue
            median = wl_subset.median()
            q1, q3 = wl_subset.quantile(0.25), wl_subset.quantile(0.75)
            lines.append(f"| {os_name} | {wl} | {median:.2f}% | [{q1:.2f}%, {q3:.2f}%] |")

    with open(path, "w") as f:
        f.write("\n".join(lines))
    print(f"Wrote {path}")


def main() -> None:
    if len(sys.argv) < 3:
        print(f"Usage: {sys.argv[0]} <input_dir> <output_dir>", file=sys.stderr)
        sys.exit(1)

    input_dir = sys.argv[1]
    output_dir = sys.argv[2]
    os.makedirs(output_dir, exist_ok=True)

    print(f"Loading data from {input_dir} ...")
    data = load_data(input_dir)
    print(f"  Loaded {len(data):,} rows from {data['runner_id'].nunique()} unique runners.")
    print(f"  OS: {data['os'].unique().tolist()}")
    print(f"  Workloads: {data['workload'].unique().tolist()}")

    print("\nComputing runner aggregates ...")
    agg_df = compute_runner_aggregates(data)

    print("Computing within-runner CoV ...")
    within_df = within_runner_cov(data)

    print("Computing between-runner CoV ...")
    between_df = between_runner_cov(agg_df)

    print("Computing MDE ...")
    mde_df = compute_mde(between_df)

    print("\nWriting outputs ...")
    write_within_runner_summary(within_df, output_dir)
    write_summary_table(between_df, output_dir)
    write_verdict_table(between_df, output_dir)

    plot_mde_heatmap(mde_df, output_dir)
    plot_box_plots(agg_df, output_dir)
    plot_scatter_by_runner(agg_df, output_dir)
    plot_cov_vs_reps(data, output_dir)

    print("\nDone.")


if __name__ == "__main__":
    main()
