//! Renders the `compare` benchmark's criterion output as the pull request
//! comment the `Bench` workflow posts.
//!
//! The workflow runs the bench twice, back to back on one runner: first on the
//! base commit with `--save-baseline main`, then on the pull request with
//! `--baseline-lenient main`. Criterion leaves the numbers under its output
//! directory as JSON: each entry's `new` estimates, the saved baseline's
//! `main` estimates, and, where the two could be compared, the `change`
//! estimates between them. This tool reads those files and writes a Markdown
//! summary: what improved or regressed beyond criterion's noise threshold, and
//! every entry under a collapsed table.
//!
//! An entry counts as improved or regressed exactly when criterion's own
//! report would say so: the 95% confidence interval of the mean change must
//! clear the threshold on one side. The t-test's p-value is not among the
//! saved files, so the interval alone decides.
//!
//! Usage: `cargo run --example bench-summary -- [DIR] [BASELINE] [BASE]
//! [HEAD]`, where `DIR` defaults to `target/criterion`, `BASELINE` to `main`,
//! `BASE` to `main` and `HEAD` to `PR`. The labels are what the summary calls
//! the two sides; a label longer than eight characters is shortened to one.

use std::collections::BTreeMap;
use std::env;
use std::fmt::{self, Write as _};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use serde_json::Value;

/// The noise threshold, matching criterion's default: a change smaller than
/// this is not worth calling either way.
const NOISE_THRESHOLD: f64 = 0.01;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("bench-summary: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<()> {
    let mut args = env::args().skip(1);
    let dir = args.next().unwrap_or_else(|| "target/criterion".to_owned());
    let baseline = args.next().unwrap_or_else(|| "main".to_owned());
    let base = args.next().unwrap_or_else(|| "main".to_owned());
    let head = args.next().unwrap_or_else(|| "PR".to_owned());

    let entries = collect(Path::new(&dir), &baseline)?;
    if entries.is_empty() {
        return Err(format!("no criterion estimates under {dir}").into());
    }
    print!("{}", render(&entries, &baseline, &base, &head)?);
    Ok(())
}

/// One benchmark's estimates: the saved baseline, the pull request's numbers,
/// and the change between the two where criterion could compute one.
#[derive(Default)]
struct Entry {
    baseline: Option<f64>,
    new: Option<f64>,
    change: Option<Change>,
}

/// The mean change against the baseline, as a fraction, and the bounds of its
/// confidence interval.
struct Change {
    point: f64,
    lower: f64,
    upper: f64,
}

/// Reads every `estimates.json` under `root`, keyed by benchmark id — the
/// `group/function/value` path criterion files an entry under.
fn collect(root: &Path, baseline: &str) -> Result<BTreeMap<String, Entry>> {
    let mut entries: BTreeMap<String, Entry> = BTreeMap::new();
    for file in estimates_files(root)? {
        let Some((id, directory)) = split(&file, root) else {
            continue;
        };
        let entry = entries.entry(id).or_default();
        let value = load(&file)?;
        match directory.as_str() {
            "new" => entry.new = point(&value, "mean"),
            "change" => entry.change = change(&value),
            directory if directory == baseline => entry.baseline = point(&value, "mean"),
            _ => {}
        }
    }
    Ok(entries)
}

/// Every `estimates.json` below `root`, skipping criterion's `report` tree,
/// which holds only the HTML and SVG for a browser.
fn estimates_files(root: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    let mut directories = vec![root.to_path_buf()];
    while let Some(directory) = directories.pop() {
        for entry in fs::read_dir(&directory)? {
            let entry = entry?;
            let name = entry.file_name();
            if entry.file_type()?.is_dir() {
                if name != "report" {
                    directories.push(entry.path());
                }
            } else if name == "estimates.json" {
                files.push(entry.path());
            }
        }
    }
    Ok(files)
}

/// Splits `.../<id>/<directory>/estimates.json` into the benchmark id and the
/// directory the file sits in (`new`, `change`, or the baseline's name).
fn split(file: &Path, root: &Path) -> Option<(String, String)> {
    let relative = file.strip_prefix(root).ok()?;
    let mut parts: Vec<String> = relative
        .components()
        .map(|part| part.as_os_str().to_string_lossy().into_owned())
        .collect();
    if parts.pop()?.as_str() != "estimates.json" {
        return None;
    }
    let directory = parts.pop()?;
    if parts.is_empty() {
        return None;
    }
    Some((parts.join("/"), directory))
}

/// Loads one JSON file, naming it in the error.
fn load(path: &Path) -> Result<Value> {
    let bytes = fs::read(path).map_err(|error| format!("{}: {error}", path.display()))?;
    serde_json::from_slice(&bytes).map_err(|error| format!("{}: {error}", path.display()).into())
}

/// The point estimate of one statistic of an `estimates.json`.
fn point(value: &Value, statistic: &str) -> Option<f64> {
    value.get(statistic)?.get("point_estimate")?.as_f64()
}

/// The mean change of a `change/estimates.json`.
fn change(value: &Value) -> Option<Change> {
    let mean = value.get("mean")?;
    let interval = mean.get("confidence_interval")?;
    Some(Change {
        point: mean.get("point_estimate")?.as_f64()?,
        lower: interval.get("lower_bound")?.as_f64()?,
        upper: interval.get("upper_bound")?.as_f64()?,
    })
}

/// What a change clears the noise threshold as.
enum Verdict {
    Improved,
    /// Slower, and beyond noise.
    Regressed,
    WithinNoise,
}

fn verdict(change: &Change) -> Verdict {
    if change.lower < -NOISE_THRESHOLD && change.upper < -NOISE_THRESHOLD {
        Verdict::Improved
    } else if change.lower > NOISE_THRESHOLD && change.upper > NOISE_THRESHOLD {
        Verdict::Regressed
    } else {
        Verdict::WithinNoise
    }
}

fn render(
    entries: &BTreeMap<String, Entry>,
    baseline: &str,
    base: &str,
    head: &str,
) -> std::result::Result<String, fmt::Error> {
    let mut improved = Vec::new();
    let mut regressed = Vec::new();
    let mut within_noise = 0;
    let mut new = 0;
    let mut removed = 0;
    for (id, entry) in entries {
        match &entry.change {
            Some(change) => match verdict(change) {
                Verdict::Improved => improved.push((id, entry, change)),
                Verdict::Regressed => regressed.push((id, entry, change)),
                Verdict::WithinNoise => within_noise += 1,
            },
            None if entry.baseline.is_some() && entry.new.is_none() => removed += 1,
            None if entry.new.is_some() && entry.baseline.is_none() => new += 1,
            None => {}
        }
    }
    // The most improved first, and the most regressed first, so each table
    // leads with its news.
    improved.sort_by(|a, b| a.2.point.total_cmp(&b.2.point));
    regressed.sort_by(|a, b| b.2.point.total_cmp(&a.2.point));

    let baseline = short_label(baseline);
    let mut out = String::new();
    writeln!(out, "## Benchmark results\n")?;
    writeln!(
        out,
        "`{}` (this pull request) against `{}` ({baseline}), benchmarked back to back on one \
         runner. A negative change is faster.\n",
        short_label(head),
        short_label(base),
    )?;
    write!(
        out,
        "**{} improved · {} regressed · {within_noise} within noise** of {} entries compared",
        improved.len(),
        regressed.len(),
        improved.len() + regressed.len() + within_noise,
    )?;
    let mut extras = Vec::new();
    if new > 0 {
        extras.push(format!("{new} new"));
    }
    if removed > 0 {
        extras.push(format!("{removed} removed"));
    }
    if !extras.is_empty() {
        write!(out, " — {}", extras.join(", "))?;
    }
    writeln!(out, ".\n")?;

    if !improved.is_empty() {
        writeln!(out, "### Improved\n")?;
        table(&mut out, baseline, &improved)?;
    }
    if !regressed.is_empty() {
        writeln!(out, "### Regressed\n")?;
        table(&mut out, baseline, &regressed)?;
    }

    writeln!(out, "<details>")?;
    writeln!(out, "<summary>All {} entries</summary>\n", entries.len())?;
    let mut groups: BTreeMap<&str, Vec<(&String, &Entry)>> = BTreeMap::new();
    for (id, entry) in entries {
        let group = id.split_once('/').map_or(id.as_str(), |(group, _)| group);
        groups.entry(group).or_default().push((id, entry));
    }
    for (&group, rows) in &groups {
        writeln!(out, "#### {group}\n")?;
        writeln!(out, "| Benchmark | {baseline} | PR | Change |")?;
        writeln!(out, "| --- | ---: | ---: | ---: |")?;
        for &(id, entry) in rows {
            out.push_str(&row(id, entry));
        }
        writeln!(out)?;
    }
    writeln!(out, "</details>\n")?;
    writeln!(
        out,
        "<sub>An entry counts as improved or regressed when the 95% confidence interval of its \
         mean change clears criterion's ±1% noise threshold. Shared runners are noisy; repeat \
         before trusting a small change.</sub>",
    )?;
    Ok(out)
}

/// Writes one change table, header and all.
fn table(out: &mut String, baseline: &str, rows: &[(&String, &Entry, &Change)]) -> fmt::Result {
    writeln!(out, "| Benchmark | {baseline} | PR | Change |")?;
    writeln!(out, "| --- | ---: | ---: | ---: |")?;
    for &(id, entry, change) in rows {
        writeln!(
            out,
            "| `{id}` | {} | {} | {} |",
            time_cell(entry.baseline),
            time_cell(entry.new),
            marked(change),
        )?;
    }
    out.push('\n');
    Ok(())
}

/// One row of the collapsed table: every entry, whether it compared or is new
/// or gone.
fn row(id: &str, entry: &Entry) -> String {
    let change = entry.change.as_ref().map_or_else(
        || match (entry.baseline.is_some(), entry.new.is_some()) {
            (true, false) => "removed".to_owned(),
            (false, true) => "new".to_owned(),
            _ => "—".to_owned(),
        },
        marked,
    );
    format!(
        "| `{id}` | {} | {} | {change} |\n",
        time_cell(entry.baseline),
        time_cell(entry.new),
    )
}

/// A change with the mark and emphasis its verdict earns.
fn marked(change: &Change) -> String {
    let (mark, bold) = match verdict(change) {
        Verdict::Improved => ("🟢", true),
        Verdict::Regressed => ("🔴", true),
        Verdict::WithinNoise => ("⚪", false),
    };
    let percent = percent(change.point);
    if bold {
        format!("{mark} **{percent}**")
    } else {
        format!("{mark} {percent}")
    }
}

/// A time, in the unit that keeps it readable, or an em dash when the side
/// never ran the entry.
fn time_cell(ns: Option<f64>) -> String {
    ns.map_or_else(|| "—".to_owned(), format_time)
}

/// Formats a time in nanoseconds the way criterion's own report does.
fn format_time(ns: f64) -> String {
    if ns < 1.0 {
        format!("{} ps", short(ns * 1e3))
    } else if ns < 1e3 {
        format!("{} ns", short(ns))
    } else if ns < 1e6 {
        format!("{} µs", short(ns / 1e3))
    } else if ns < 1e9 {
        format!("{} ms", short(ns / 1e6))
    } else {
        format!("{} s", short(ns / 1e9))
    }
}

/// Five significant digits, as compact as criterion prints them: one decimal
/// is too coarse to compare two rows whose difference is a few percent.
fn short(n: f64) -> String {
    if n < 10.0 {
        format!("{n:.4}")
    } else if n < 100.0 {
        format!("{n:.3}")
    } else if n < 1000.0 {
        format!("{n:.2}")
    } else if n < 10000.0 {
        format!("{n:.1}")
    } else {
        format!("{n:.0}")
    }
}

/// A change as a signed percentage; a value that would round to a signed zero
/// is just `0.0%`.
fn percent(ratio: f64) -> String {
    let value = ratio * 100.0;
    if value.abs() < 0.05 {
        "0.0%".to_owned()
    } else {
        format!("{value:+.1}%")
    }
}

/// The first eight characters of a label — enough of a sha to recognize,
/// where a whole one would dominate every table header.
fn short_label(label: &str) -> &str {
    match label.char_indices().nth(8) {
        Some((end, _)) => &label[..end],
        None => label,
    }
}
