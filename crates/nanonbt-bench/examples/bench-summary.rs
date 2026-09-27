//! Renders the `compare` benchmark's criterion output as the pull request
//! comment the `Bench` workflow posts.
//!
//! The workflow benches both sides in two parallel jobs whose pass order is
//! mirrored: the base-first job runs the base commit and saves its numbers as
//! `base`, then runs the pull request and saves `head`; the head-first job
//! runs the two in the opposite order under the same names. Each job uploads
//! its criterion output, and this tool reads every entry's `base` and `head`
//! estimates from both and writes the Markdown summary.
//!
//! The mirrored order is what makes the numbers worth reading: anything that
//! follows the pass order — a clock ramping up, the page cache warming —
//! pushes one job's change up and the other's down, while a real change moves
//! both the same way. An entry counts as improved or regressed only when both
//! jobs clear criterion's ±1% noise threshold in the same direction; the two
//! jobs' numbers are always shown side by side, and a pair that disagrees is
//! called unstable rather than a change.
//!
//! Usage: `cargo run --example bench-summary -- <BASE_FIRST> <HEAD_FIRST>
//! [BASE] [HEAD] [BASE_BRANCH]`, where `BASE_FIRST` and `HEAD_FIRST` are the
//! two jobs' criterion directories, each holding every entry's `base` and
//! `head` estimates; `BASE` and `HEAD` are the shas the header names the two
//! sides by, defaulting to `base` and `PR`; and `BASE_BRANCH` labels the base
//! side in the tables, defaulting to `main`. A sha label longer than eight
//! characters is shortened to one.

use std::collections::{BTreeMap, BTreeSet};
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
    let (Some(base_first), Some(head_first)) = (args.next(), args.next()) else {
        return Err(
            "usage: bench-summary <BASE_FIRST> <HEAD_FIRST> [BASE] [HEAD] [BASE_BRANCH]".into(),
        );
    };
    let base = args.next().unwrap_or_else(|| "base".to_owned());
    let head = args.next().unwrap_or_else(|| "PR".to_owned());
    let base_branch = args.next().unwrap_or_else(|| "main".to_owned());

    let jobs = [
        read_job(Path::new(&base_first))?,
        read_job(Path::new(&head_first))?,
    ];
    if jobs[0].is_empty() || jobs[1].is_empty() {
        return Err(format!("no criterion estimates under {base_first} or {head_first}").into());
    }
    print!("{}", render(&jobs, &base, &head, &base_branch)?);
    Ok(())
}

/// One benchmark's estimates from one job: the base commit's mean time and
/// the pull request's, in nanoseconds.
#[derive(Clone, Copy, Default)]
struct Pair {
    base: Option<f64>,
    head: Option<f64>,
}

impl Pair {
    /// The job's change of the pull request against the base commit, as a
    /// fraction, or `None` when the job did not run both sides.
    fn change(self) -> Option<f64> {
        Some(self.head? / self.base? - 1.0)
    }
}

/// One benchmark across both jobs.
struct Entry<'a> {
    id: &'a str,
    sides: [Pair; 2],
}

impl Entry<'_> {
    /// The mean time of each side across the jobs that ran it.
    fn base(&self) -> Option<f64> {
        mean(self.sides.iter().filter_map(|side| side.base))
    }

    fn head(&self) -> Option<f64> {
        mean(self.sides.iter().filter_map(|side| side.head))
    }

    /// Each job's change of the pull request against the base commit.
    fn changes(&self) -> Vec<f64> {
        self.sides.iter().filter_map(|side| side.change()).collect()
    }

    /// What the jobs' changes add up to, or `None` when no job compared the
    /// entry's two sides.
    fn verdict(&self) -> Option<Verdict> {
        verdict(&self.changes())
    }
}

/// Reads every `base` and `head` estimate under `root`, keyed by benchmark id
/// — the `group/function/value` path criterion files an entry under.
fn read_job(root: &Path) -> Result<BTreeMap<String, Pair>> {
    let mut job: BTreeMap<String, Pair> = BTreeMap::new();
    for file in estimates_files(root)? {
        let Some((id, directory)) = split(&file, root) else {
            continue;
        };
        let side = job.entry(id).or_default();
        let value = load(&file)?;
        match directory.as_str() {
            "base" => side.base = point(&value, "mean"),
            "head" => side.head = point(&value, "mean"),
            _ => {}
        }
    }
    Ok(job)
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
/// directory the file sits in (`new`, `base`, or `head`).
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

/// What the jobs' changes agree on.
enum Verdict {
    Improved,
    /// Slower, and beyond noise.
    Regressed,
    /// The jobs disagree; neither one's change is trustworthy.
    Unstable,
    WithinNoise,
}

/// The verdict for one entry's per-job changes: every job must clear the
/// threshold in the same direction. Some jobs moving and others not, or two
/// jobs moving opposite ways, is a disagreement, not a change.
fn verdict(changes: &[f64]) -> Option<Verdict> {
    if changes.is_empty() {
        return None;
    }
    let improved = changes.iter().all(|change| *change <= -NOISE_THRESHOLD);
    let regressed = changes.iter().all(|change| *change >= NOISE_THRESHOLD);
    Some(if improved {
        Verdict::Improved
    } else if regressed {
        Verdict::Regressed
    } else if changes.iter().any(|change| change.abs() >= NOISE_THRESHOLD) {
        Verdict::Unstable
    } else {
        Verdict::WithinNoise
    })
}

fn render(
    jobs: &[BTreeMap<String, Pair>; 2],
    base: &str,
    head: &str,
    base_branch: &str,
) -> std::result::Result<String, fmt::Error> {
    let ids: BTreeSet<&str> = jobs
        .iter()
        .flat_map(|job| job.keys().map(String::as_str))
        .collect();
    let entries: Vec<Entry> = ids
        .into_iter()
        .map(|id| Entry {
            id,
            sides: [
                jobs[0].get(id).copied().unwrap_or_default(),
                jobs[1].get(id).copied().unwrap_or_default(),
            ],
        })
        .collect();

    let mut improved = Vec::new();
    let mut regressed = Vec::new();
    let mut unstable = Vec::new();
    let mut within_noise = 0;
    let mut new = 0;
    let mut removed = 0;
    for entry in &entries {
        match entry.verdict() {
            Some(Verdict::Improved) => improved.push(entry),
            Some(Verdict::Regressed) => regressed.push(entry),
            Some(Verdict::Unstable) => unstable.push(entry),
            Some(Verdict::WithinNoise) => within_noise += 1,
            None if entry.base().is_some() && entry.head().is_none() => removed += 1,
            None if entry.head().is_some() && entry.base().is_none() => new += 1,
            None => {}
        }
    }
    // Each table leads with its news. An entry is ordered by the number its
    // table shows: the change closest to zero, the one both jobs support.
    improved.sort_by(|a, b| closest(&a.changes()).total_cmp(&closest(&b.changes())));
    regressed.sort_by(|a, b| closest(&b.changes()).total_cmp(&closest(&a.changes())));
    unstable.sort_by(|a, b| widest(&b.changes()).total_cmp(&widest(&a.changes())));

    let mut out = String::new();
    writeln!(out, "## Benchmark results\n")?;
    writeln!(
        out,
        "`{}` (this pull request) against `{}` ({base_branch}). Each side was benched in two \
         jobs whose pass order is mirrored, and a change is listed only where both agree. A \
         negative change is faster.\n",
        short_label(head),
        short_label(base),
    )?;
    write!(
        out,
        "**{} improved · {} regressed · {} unstable · {within_noise} within noise** of {} entries compared",
        improved.len(),
        regressed.len(),
        unstable.len(),
        improved.len() + regressed.len() + unstable.len() + within_noise,
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

    for (title, rows) in [
        ("Improved", &improved),
        ("Regressed", &regressed),
        ("Unstable", &unstable),
    ] {
        if rows.is_empty() {
            continue;
        }
        writeln!(out, "### {title}\n")?;
        table(&mut out, base_branch, rows)?;
    }

    writeln!(out, "<details>")?;
    writeln!(out, "<summary>All {} entries</summary>\n", entries.len())?;
    let mut groups: BTreeMap<&str, Vec<&Entry>> = BTreeMap::new();
    for entry in &entries {
        let group = entry
            .id
            .split_once('/')
            .map_or(entry.id, |(group, _)| group);
        groups.entry(group).or_default().push(entry);
    }
    for (&group, rows) in &groups {
        writeln!(out, "#### {group}\n")?;
        table(&mut out, base_branch, rows)?;
    }
    writeln!(out, "</details>\n")?;
    writeln!(
        out,
        "<sub>`base first` benched the base commit and then the pull request; `head first` \
         benched them in the opposite order. An entry counts as improved or regressed only when \
         both jobs' changes clear criterion's ±1% noise threshold in the same direction — the \
         percentage is then the less extreme of the two — and a pair that disagrees is listed as \
         unstable. Shared runners are noisy; repeat before trusting a small change.</sub>",
    )?;
    Ok(out)
}

/// Writes one entry table, header and all.
fn table(out: &mut String, base_branch: &str, rows: &[&Entry]) -> fmt::Result {
    writeln!(
        out,
        "| Benchmark | {base_branch} | PR | Change | base first / head first |",
    )?;
    writeln!(out, "| --- | ---: | ---: | ---: | ---: |")?;
    for entry in rows {
        writeln!(
            out,
            "| `{}` | {} | {} | {} | {} |",
            entry.id,
            time_cell(entry.base()),
            time_cell(entry.head()),
            change_cell(entry),
            changes_cell(entry),
        )?;
    }
    out.push('\n');
    Ok(())
}

/// The entry's change cell: the verdict both jobs add up to, or how the entry
/// stands out of the comparison.
fn change_cell(entry: &Entry) -> String {
    match entry.verdict() {
        Some(Verdict::Unstable) => "⚠️ unstable".to_owned(),
        Some(verdict) => marked(verdict, closest(&entry.changes())),
        None if entry.base().is_some() && entry.head().is_none() => "removed".to_owned(),
        None if entry.head().is_some() && entry.base().is_none() => "new".to_owned(),
        None => "—".to_owned(),
    }
}

/// Each job's change, in the order the two jobs are named, with an em dash
/// where a job has no comparison.
fn changes_cell(entry: &Entry) -> String {
    entry
        .sides
        .iter()
        .map(|side| side.change().map_or_else(|| "—".to_owned(), percent))
        .collect::<Vec<_>>()
        .join(" / ")
}

/// A change with the mark and emphasis its verdict earns.
fn marked(verdict: Verdict, change: f64) -> String {
    let (mark, bold) = match verdict {
        Verdict::Improved => ("🟢", true),
        Verdict::Regressed => ("🔴", true),
        Verdict::Unstable => ("⚠️", false),
        Verdict::WithinNoise => ("⚪", false),
    };
    let percent = percent(change);
    if bold {
        format!("{mark} **{percent}**")
    } else {
        format!("{mark} {percent}")
    }
}

/// The mean of the values, or `None` when there are none.
fn mean(values: impl Iterator<Item = f64>) -> Option<f64> {
    let values: Vec<f64> = values.collect();
    if values.is_empty() {
        None
    } else {
        Some(values.iter().sum::<f64>() / values.len() as f64)
    }
}

/// The change closest to zero: the least extreme one, the number both jobs
/// support.
fn closest(changes: &[f64]) -> f64 {
    changes
        .iter()
        .copied()
        .min_by(|a, b| a.abs().total_cmp(&b.abs()))
        .unwrap_or_default()
}

/// The largest change magnitude a job saw; the unstable table leads with it.
fn widest(changes: &[f64]) -> f64 {
    changes
        .iter()
        .fold(0.0, |widest, change| widest.max(change.abs()))
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
