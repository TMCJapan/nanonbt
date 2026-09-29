//! Renders the `compare` suite's per-platform results as the pull request
//! comment the `Bench` workflow posts.
//!
//! Each platform job benches both revisions and uploads what it measured:
//! the two callgrind jobs upload the base and head trees of `summary.json`
//! files, the wasm job uploads the base and head `wasm.json` fuel counts, and
//! the Windows and macOS jobs upload a marker that says the smoke runner
//! walked every entry. This tool takes one `--callgrind NAME BASE HEAD` or
//! `--fuel NAME BASE HEAD` per counting platform and one `--smoke NAME` per
//! smoke platform, pairs each platform's base and head counts by benchmark
//! id, and writes one comment with a summary table over all platforms
//! followed by one section per counting platform.
//!
//! A count is exact on the platform that produced it — callgrind counts
//! instructions, wasmi counts wasm instructions as fuel — so a platform's two
//! sides compare without a threshold and without repeated passes. Counts from
//! different platforms are different instruction sets and are never compared
//! with each other; every section contrasts one platform's base and head
//! alone. The `pumpkin` target is the exception within every platform: its
//! compounds are `std` hash maps, the order serialization walks them follows
//! each process's random seed, and the seed moves those counts by as much as
//! ten percent between runs, so the report lists them for reference only.
//!
//! Every table stays compact enough that all platforms fit one comment: the
//! collapsed tables list every entry except the ones the two sides counted
//! equal — the platform line above still counts those — and show the pull
//! request's count and the change, not both counts (a changed entry already
//! carries both counts in the `Improved`/`Regressed` table above, and a new
//! or removed one has only one side to a count), and no number gets
//! thousands separators. If the whole comment would still outgrow GitHub's
//! size limit, the tool drops the count column from the collapsed tables
//! rather than lose any platform or entry, and fails if even that will not
//! fit.
//!
//! Usage: `cargo run --example bench-summary -- [OPTIONS]`, with
//!
//! - `--base-sha SHA` and `--head-sha SHA` to label the two sides (default
//!   `base` and `PR`),
//! - `--base-ref REF` to name the base branch (default `main`),
//! - `--run-url URL` to link the workflow run,
//! - `--title TITLE` for the comment heading (default `Benchmark results`),
//! - `--callgrind NAME BASE HEAD` for one callgrind platform, where `BASE`
//!   and `HEAD` are the trees under which `summary.json` files sit,
//! - `--fuel NAME BASE HEAD` for one wasm platform, where `BASE` and `HEAD`
//!   are `wasm.json` files,
//! - `--smoke-dir DIR` and one `--smoke NAME` per smoke platform, where
//!   `DIR/smoke-NAME/NAME.txt` is the smoke marker.
//!
//! A comparison whose two sides are both missing is reported as `did not
//! run`, so one failed platform still lets the others post; one side missing
//! alone is an error, as is a missing smoke marker for a name that was asked
//! for. A sha label longer than eight characters is shortened to one.

use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fmt::{self, Write as _};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use serde::Deserialize;
use serde_json::Value;

use nanonbt_bench::report::display_id;

/// The benchmark target the report does not compare. `pumpkin`'s compounds
/// are `std` hash maps, and serialization walks them in seed order, so its
/// counts move between runs; the report lists them for reference only.
const REFERENCE_TARGET: &str = "pumpkin";

/// The largest comment this tool writes to GitHub's 65,536-character limit,
/// with room for the sticky-comment action's wrapper around it.
const MAX_COMMENT_BYTES: usize = 60_000;

/// The instruction counts one run of the suite holds, keyed by the benchmark
/// id the report shows.
type Run = BTreeMap<String, u64>;

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
    let options = parse_args()?;
    let mut platforms = Vec::new();
    for comparison in &options.comparisons {
        platforms.push(read_platform(comparison)?);
    }
    let smoke_dir = options.smoke_dir.as_deref();
    for name in &options.smokes {
        let dir = smoke_dir.expect("a --smoke-dir is required with --smoke");
        platforms.push(read_smoke(dir, name)?);
    }
    if platforms.is_empty() {
        return Err("nothing to report; pass --callgrind, --fuel or --smoke".into());
    }
    if platforms
        .iter()
        .all(|platform| matches!(platform, Platform::Missing { .. }))
    {
        return Err("no platform produced results".into());
    }

    let body = render(&options, &platforms, false)?;
    let body = if body.len() <= MAX_COMMENT_BYTES {
        body
    } else {
        let compact = render(&options, &platforms, true)?;
        if compact.len() > MAX_COMMENT_BYTES {
            return Err(format!(
                "the comment is {} bytes even with the count columns dropped; split it",
                compact.len()
            )
            .into());
        }
        compact
    };
    eprintln!("bench-summary: {} bytes", body.len());
    print!("{body}");
    Ok(())
}

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

/// What one platform measured, keyed by the benchmark id the report shows.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    /// Instructions, counted by callgrind.
    Callgrind,
    /// Wasm instructions, counted by wasmi as fuel.
    Fuel,
    /// Every entry run once, without counts.
    Smoke,
}

impl Kind {
    /// How the platform's table names the harness.
    fn harness(self) -> &'static str {
        match self {
            Kind::Callgrind => "callgrind instructions",
            Kind::Fuel => "wasmi fuel",
            Kind::Smoke => "build & run",
        }
    }
}

/// One counting platform: its name, its harness and the two runs to pair.
struct Comparison {
    name: String,
    kind: Kind,
    base: PathBuf,
    head: PathBuf,
}

/// The command line.
struct Options {
    base: String,
    head: String,
    base_ref: String,
    run_url: Option<String>,
    title: String,
    comparisons: Vec<Comparison>,
    smoke_dir: Option<PathBuf>,
    smokes: Vec<String>,
}

/// One side's `Run`, or `None` when the side never produced results.
fn read_side(path: &Path, kind: Kind) -> Result<Option<Run>> {
    if !path.exists() {
        return Ok(None);
    }
    let run = match kind {
        Kind::Callgrind => read_callgrind(path)?,
        Kind::Fuel => read_fuel(path)?,
        Kind::Smoke => unreachable!("smoke platforms do not read a run"),
    };
    if run.is_empty() {
        return Err(format!("{}: no results", path.display()).into());
    }
    Ok(Some(run))
}

/// One platform, whichever way the run went.
enum Platform {
    /// Both sides ran and their counts pair up.
    Counts {
        name: String,
        kind: Kind,
        base: Run,
        head: Run,
    },
    /// The smoke runner walked the registry, or did not.
    Smoke {
        name: String,
        entries: Option<usize>,
    },
    /// Neither side ran, so the section says so.
    Missing { name: String, kind: Kind },
}

/// Reads one platform's two sides, or reports the platform missing.
fn read_platform(comparison: &Comparison) -> Result<Platform> {
    let base = read_side(&comparison.base, comparison.kind)?;
    let head = read_side(&comparison.head, comparison.kind)?;
    match (base, head) {
        (None, None) => Ok(Platform::Missing {
            name: comparison.name.clone(),
            kind: comparison.kind,
        }),
        (Some(base), Some(head)) => Ok(Platform::Counts {
            name: comparison.name.clone(),
            kind: comparison.kind,
            base,
            head,
        }),
        _ => Err(format!(
            "{}: only one side produced results, which a pairing cannot compare",
            comparison.name
        )
        .into()),
    }
}

/// Reads the smoke marker `DIR/smoke-NAME/NAME.txt`.
///
/// The smoke runner prints `N entries ran` as its last line, so the marker is
/// also the entry count; a marker that is not that says its run failed.
fn read_smoke(dir: &Path, name: &str) -> Result<Platform> {
    let path = dir
        .join(format!("smoke-{name}"))
        .join(format!("{name}.txt"));
    if !path.exists() {
        return Ok(Platform::Smoke {
            name: name.to_owned(),
            entries: None,
        });
    }
    let text = fs::read_to_string(&path).map_err(|error| format!("{}: {error}", path.display()))?;
    let entries = text
        .lines()
        .rev()
        .find_map(|line| line.strip_suffix(" entries ran")?.trim().parse().ok())
        .ok_or_else(|| format!("{}: no `N entries ran` line", path.display()))?;
    Ok(Platform::Smoke {
        name: name.to_owned(),
        entries: Some(entries),
    })
}

/// Reads every `summary.json` under `root`, keyed by the benchmark id the
/// report shows.
fn read_callgrind(root: &Path) -> Result<Run> {
    let mut run: Run = BTreeMap::new();
    for file in summary_files(root)? {
        let value = load(&file)?;
        let (id, instructions) = entry(&value)
            .ok_or_else(|| format!("{}: not a complete iai-callgrind summary", file.display()))?;
        if run.insert(id.clone(), instructions).is_some() {
            return Err(format!("{}: duplicate benchmark id `{id}`", file.display()).into());
        }
    }
    Ok(run)
}

/// Reads the `wasm.json` the wasmi fuel runner writes.
fn read_fuel(path: &Path) -> Result<Run> {
    #[derive(Deserialize)]
    struct Report {
        entries: Vec<Entry>,
    }
    #[derive(Deserialize)]
    struct Entry {
        id: String,
        count: u64,
    }

    let bytes = fs::read(path).map_err(|error| format!("{}: {error}", path.display()))?;
    let report: Report =
        serde_json::from_slice(&bytes).map_err(|error| format!("{}: {error}", path.display()))?;
    let mut run: Run = BTreeMap::new();
    for entry in report.entries {
        if run.insert(entry.id.clone(), entry.count).is_some() {
            return Err(
                format!("{}: duplicate benchmark id `{}`", path.display(), entry.id).into(),
            );
        }
    }
    Ok(run)
}

/// Every `summary.json` below `root`.
fn summary_files(root: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    let mut directories = vec![root.to_path_buf()];
    while let Some(directory) = directories.pop() {
        for entry in fs::read_dir(&directory)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                directories.push(entry.path());
            } else if entry.file_name() == "summary.json" {
                files.push(entry.path());
            }
        }
    }
    Ok(files)
}

/// Loads one JSON file, naming it in the error.
fn load(path: &Path) -> Result<Value> {
    let bytes = fs::read(path).map_err(|error| format!("{}: {error}", path.display()))?;
    serde_json::from_slice(&bytes).map_err(|error| format!("{}: {error}", path.display()).into())
}

/// The report id and instruction count of one `summary.json`.
fn entry(value: &Value) -> Option<(String, u64)> {
    let function_name = value.get("function_name")?.as_str()?;
    let id = value.get("id")?.as_str()?;
    Some((display_id(function_name, id)?, instructions(value)?))
}

/// The instruction count of one run: the first profile's total `Ir`.
///
/// A fresh run files its count under `Left`, as the metric itself; a run that
/// found the previous output for the same benchmark binary files both under
/// `Both`, current first. Both are the current run's count, and neither is a
/// stored baseline, which `Right` alone would be.
fn instructions(value: &Value) -> Option<u64> {
    let metrics = value
        .get("profiles")?
        .as_array()?
        .iter()
        .find_map(|profile| profile.pointer("/summaries/total/summary/Callgrind/Ir/metrics"))?;
    ["Left", "Both"].iter().find_map(|side| {
        let side = metrics.get(*side)?;
        let current = match side.as_array() {
            Some(parts) => parts.first()?,
            None => side,
        };
        current.get("Int")?.as_u64()
    })
}

/// One benchmark across the two runs.
struct Entry {
    id: String,
    base: Option<u64>,
    head: Option<u64>,
}

impl Entry {
    /// Whether the entry's counts are stable enough to compare; when they are
    /// not, the report lists the entry for reference.
    fn comparable(&self) -> bool {
        target(&self.id) != Some(REFERENCE_TARGET)
    }

    /// What the two instruction counts say.
    fn verdict(&self) -> Option<Verdict> {
        if !self.comparable() {
            return None;
        }
        let (base, head) = (self.base?, self.head?);
        Some(if head < base {
            Verdict::Improved
        } else if head > base {
            Verdict::Regressed
        } else {
            Verdict::Unchanged
        })
    }

    /// The change of the pull request against the base, as a fraction, or
    /// `None` when a side never ran the entry.
    fn change(&self) -> Option<f64> {
        Some(self.head? as f64 / self.base? as f64 - 1.0)
    }
}

/// What the two instruction counts say.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Verdict {
    /// Fewer instructions in the pull request.
    Improved,
    /// More instructions in the pull request.
    Regressed,
    /// The same number of instructions.
    Unchanged,
}

/// The target of a display id: the middle of `<kind>/<target>/<id>`.
fn target(id: &str) -> Option<&str> {
    let (_, rest) = id.split_once('/')?;
    Some(rest.split_once('/')?.0)
}

/// One platform's comparison, classified and in report order.
struct Classified {
    entries: Vec<Entry>,
    improved: Vec<usize>,
    regressed: Vec<usize>,
    unchanged: usize,
    reference: usize,
    new: usize,
    removed: usize,
}

impl Classified {
    /// Pairs two runs by benchmark id and classifies every entry.
    fn new(base: &Run, head: &Run) -> Self {
        let ids: BTreeSet<&str> = base.keys().chain(head.keys()).map(String::as_str).collect();
        let entries: Vec<Entry> = ids
            .into_iter()
            .map(|id| Entry {
                id: id.to_owned(),
                base: base.get(id).copied(),
                head: head.get(id).copied(),
            })
            .collect();

        let mut classified = Self {
            entries,
            improved: Vec::new(),
            regressed: Vec::new(),
            unchanged: 0,
            reference: 0,
            new: 0,
            removed: 0,
        };
        for (index, entry) in classified.entries.iter().enumerate() {
            match entry.verdict() {
                Some(Verdict::Improved) => classified.improved.push(index),
                Some(Verdict::Regressed) => classified.regressed.push(index),
                Some(Verdict::Unchanged) => classified.unchanged += 1,
                None if entry.base.is_some() && entry.head.is_none() => classified.removed += 1,
                None if entry.head.is_some() && entry.base.is_none() => classified.new += 1,
                None if !entry.comparable() => classified.reference += 1,
                None => {}
            }
        }
        // The tables lead with the biggest changes, which are the ones to
        // look at first.
        let change = |&index: &usize| classified.entries[index].change().unwrap_or_default();
        classified
            .improved
            .sort_by(|&a, &b| change(&a).total_cmp(&change(&b)));
        classified
            .regressed
            .sort_by(|&a, &b| change(&b).total_cmp(&change(&a)));
        classified
    }

    fn total(&self) -> usize {
        self.entries.len()
    }

    fn compared(&self) -> usize {
        self.improved.len() + self.regressed.len() + self.unchanged
    }

    /// The indices of the entries the collapsed tables list, in report order:
    /// every entry except the ones the two sides counted equal.
    fn listed(&self) -> Vec<usize> {
        self.entries
            .iter()
            .enumerate()
            .filter(|(_, entry)| entry.verdict() != Some(Verdict::Unchanged))
            .map(|(index, _)| index)
            .collect()
    }

    /// The entries of one class, in order.
    fn rows(&self, indices: &[usize]) -> Vec<&Entry> {
        indices.iter().map(|&index| &self.entries[index]).collect()
    }
}

fn render(
    options: &Options,
    platforms: &[Platform],
    compact: bool,
) -> std::result::Result<String, fmt::Error> {
    let mut out = String::new();
    writeln!(out, "## {}\n", options.title)?;
    write!(
        out,
        "`{}` (this pull request) against `{}` (`{}`).",
        short_label(&options.head),
        short_label(&options.base),
        options.base_ref,
    )?;
    if let Some(url) = &options.run_url {
        write!(out, " [Workflow run]({url}).")?;
    }
    writeln!(
        out,
        "\n\nEvery counting platform compares its own base and pull request; a count is exact on \
         the platform that produced it, but counts from different architectures are different \
         instruction sets and are not comparable with each other. `{REFERENCE_TARGET}` entries \
         are listed for reference only: their compounds are `std` hash maps, so their counts move \
         between runs with the seed the map is built under.\n"
    )?;

    let mut classified: Vec<Option<Classified>> = Vec::with_capacity(platforms.len());
    for platform in platforms {
        classified.push(match platform {
            Platform::Counts { base, head, .. } => Some(Classified::new(base, head)),
            _ => None,
        });
    }

    writeln!(out, "### Platforms\n")?;
    writeln!(
        out,
        "| Platform | Harness | Entries | 🟢 | 🔴 | ⚪ | Other |"
    )?;
    writeln!(out, "| --- | --- | ---: | ---: | ---: | ---: | ---: |")?;
    for (platform, stats) in platforms.iter().zip(&classified) {
        match (platform, stats) {
            (Platform::Counts { name, kind, .. }, Some(stats)) => writeln!(
                out,
                "| {name} | {} | {} | {} | {} | {} | {} |",
                kind.harness(),
                stats.total(),
                stats.improved.len(),
                stats.regressed.len(),
                stats.unchanged,
                other_cell(stats),
            )?,
            (Platform::Smoke { name, entries }, _) => {
                let entries =
                    entries.map_or_else(|| "did not run".to_owned(), |n| format!("{n} run"));
                writeln!(
                    out,
                    "| {name} | {} | {entries} | — | — | — | — |",
                    Kind::Smoke.harness()
                )?;
            }
            (Platform::Missing { name, kind }, _) => {
                writeln!(
                    out,
                    "| {name} | {} | did not run | — | — | — | — |",
                    kind.harness()
                )?;
            }
            _ => unreachable!("only counting platforms classify"),
        }
    }
    writeln!(out)?;

    for (platform, stats) in platforms.iter().zip(&classified) {
        let (Platform::Counts { name, kind, .. }, Some(stats)) = (platform, stats) else {
            continue;
        };
        writeln!(out, "### {name}\n")?;
        match kind {
            Kind::Callgrind => writeln!(
                out,
                "Callgrind instruction counts: exact within this platform, so the two sides \
                 compare without a threshold; lower is better.\n"
            )?,
            Kind::Fuel => writeln!(
                out,
                "Wasmi fuel: the wasm instructions each entry runs, exact within this platform, \
                 so the two sides compare without a threshold; lower is better.\n"
            )?,
            Kind::Smoke => unreachable!("counting platforms only"),
        }

        write!(
            out,
            "**{} improved · {} regressed · {} unchanged** of {} entries compared",
            stats.improved.len(),
            stats.regressed.len(),
            stats.unchanged,
            stats.compared(),
        )?;
        let other = other_cell(stats);
        if other != "—" {
            write!(out, " — {other}")?;
        }
        writeln!(out, ".")?;

        for (title, indices) in [
            ("Improved", &stats.improved),
            ("Regressed", &stats.regressed),
        ] {
            if indices.is_empty() {
                continue;
            }
            writeln!(out, "\n#### {title}\n")?;
            changed_table(&mut out, &options.base_ref, &stats.rows(indices))?;
        }

        let listed = stats.listed();
        if !listed.is_empty() {
            writeln!(out, "\n<details>")?;
            if stats.unchanged == 0 {
                writeln!(out, "<summary>All {} entries</summary>\n", stats.total())?;
            } else {
                writeln!(
                    out,
                    "<summary>All {} entries · {} unchanged omitted</summary>\n",
                    stats.total(),
                    stats.unchanged
                )?;
            }
            let mut groups: BTreeMap<&str, Vec<usize>> = BTreeMap::new();
            for index in listed {
                let entry = &stats.entries[index];
                let group = entry
                    .id
                    .split_once('/')
                    .map_or(entry.id.as_str(), |(group, _)| group);
                groups.entry(group).or_default().push(index);
            }
            for (group, indices) in &groups {
                writeln!(out, "#### {group}\n")?;
                collapsed_table(&mut out, &stats.rows(indices), compact)?;
            }
            writeln!(out, "</details>\n")?;
        }
    }

    writeln!(
        out,
        "<sub>`iai-callgrind` counts instructions under valgrind, so the same code counts the \
         same instructions on every machine and every run; wasmi fuel is exact the same way, and \
         the comparison needs no noise threshold and no repeated passes.</sub>"
    )?;
    Ok(out)
}

/// The `Other` cell of a summary table: everything that is not a verdict.
fn other_cell(stats: &Classified) -> String {
    let mut extras = Vec::new();
    if stats.new > 0 {
        extras.push(format!("{} new", stats.new));
    }
    if stats.removed > 0 {
        extras.push(format!("{} removed", stats.removed));
    }
    if stats.reference > 0 {
        extras.push(format!("{} reference", stats.reference));
    }
    if extras.is_empty() {
        "—".to_owned()
    } else {
        extras.join(", ")
    }
}

/// Writes one changed-entry table, header and all: both counts and the
/// change, because those are the entries whose counts differ.
fn changed_table(out: &mut String, base_ref: &str, rows: &[&Entry]) -> fmt::Result {
    writeln!(out, "| Benchmark | {base_ref} | PR | Change |")?;
    writeln!(out, "| --- | ---: | ---: | ---: |")?;
    for entry in rows {
        writeln!(
            out,
            "| `{}` | {} | {} | {} |",
            entry.id,
            count_cell(entry.base),
            count_cell(entry.head),
            status_cell(entry),
        )?;
    }
    out.push('\n');
    Ok(())
}

/// Writes one collapsed entry table: the pull request's count and the change.
///
/// An unchanged entry is never listed, a changed one already has both counts
/// in its `Improved` or `Regressed` table, and a new or removed one has only
/// one side to a count, so one count column carries every number the table
/// has to show and it stays small enough for all platforms to share one
/// comment. The compact form drops even that column when every platform
/// together would outgrow the comment.
fn collapsed_table(out: &mut String, rows: &[&Entry], compact: bool) -> fmt::Result {
    if compact {
        writeln!(out, "| Benchmark | Change |")?;
        writeln!(out, "| --- | --- |")?;
        for entry in rows {
            writeln!(out, "| `{}` | {} |", entry.id, status_cell(entry))?;
        }
    } else {
        writeln!(out, "| Benchmark | PR | Change |")?;
        writeln!(out, "| --- | ---: | --- |")?;
        for entry in rows {
            writeln!(
                out,
                "| `{}` | {} | {} |",
                entry.id,
                count_cell(entry.head.or(entry.base)),
                status_cell(entry),
            )?;
        }
    }
    out.push('\n');
    Ok(())
}

/// The entry's change cell: the verdict its two counts add up to, how the
/// entry stands out of the comparison, or that the entry is only listed for
/// reference.
fn status_cell(entry: &Entry) -> String {
    if let Some(verdict) = entry.verdict() {
        let change = entry.change().unwrap_or_default();
        return match verdict {
            Verdict::Improved => format!("🟢 {}", percent(change)),
            Verdict::Regressed => format!("🔴 {}", percent(change)),
            Verdict::Unchanged => "⚪".to_owned(),
        };
    }
    if entry.base.is_some() && entry.head.is_none() {
        return "removed".to_owned();
    }
    if entry.head.is_some() && entry.base.is_none() {
        return "new".to_owned();
    }
    if !entry.comparable() {
        return "not compared".to_owned();
    }
    "—".to_owned()
}

/// An instruction count, or an em dash when the side never ran the entry.
fn count_cell(instructions: Option<u64>) -> String {
    instructions.map_or_else(|| "—".to_owned(), |count| count.to_string())
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

/// Reads one option's value, naming the option when it is missing.
fn value(args: &mut impl Iterator<Item = String>, flag: &str) -> Result<String> {
    args.next()
        .ok_or_else(|| format!("{flag} needs a value").into())
}

/// Parses the command line.
fn parse_args() -> Result<Options> {
    const USAGE: &str = "usage: bench-summary [--base-sha SHA] [--head-sha SHA] [--base-ref REF] \
                         [--run-url URL] [--title TITLE] \
                         [--callgrind NAME BASE HEAD]... [--fuel NAME BASE HEAD]... \
                         [--smoke-dir DIR] [--smoke NAME]...";

    let mut options = Options {
        base: "base".to_owned(),
        head: "PR".to_owned(),
        base_ref: "main".to_owned(),
        run_url: None,
        title: "Benchmark results".to_owned(),
        comparisons: Vec::new(),
        smoke_dir: None,
        smokes: Vec::new(),
    };

    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--base-sha" => options.base = value(&mut args, "--base-sha")?,
            "--head-sha" => options.head = value(&mut args, "--head-sha")?,
            "--base-ref" => options.base_ref = value(&mut args, "--base-ref")?,
            "--run-url" => options.run_url = Some(value(&mut args, "--run-url")?),
            "--title" => options.title = value(&mut args, "--title")?,
            "--callgrind" | "--fuel" => {
                let kind = if arg == "--callgrind" {
                    Kind::Callgrind
                } else {
                    Kind::Fuel
                };
                let name = value(&mut args, &arg)?;
                let base = value(&mut args, &arg)?;
                let head = value(&mut args, &arg)?;
                options.comparisons.push(Comparison {
                    name,
                    kind,
                    base: base.into(),
                    head: head.into(),
                });
            }
            "--smoke-dir" => options.smoke_dir = Some(value(&mut args, "--smoke-dir")?.into()),
            "--smoke" => options.smokes.push(value(&mut args, "--smoke")?),
            "--help" | "-h" => return Err(USAGE.into()),
            other => return Err(format!("unknown argument `{other}`\n{USAGE}").into()),
        }
    }
    if !options.smokes.is_empty() && options.smoke_dir.is_none() {
        return Err("--smoke needs a --smoke-dir\nusage: ...".into());
    }
    Ok(options)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_each_entry() {
        let base: Run = [
            ("parse/nanonbt-serde/small".to_owned(), 100),
            ("parse/fastnbt/small".to_owned(), 200),
            ("skip/nanonbt-serde/chunk".to_owned(), 300),
            ("write/pumpkin/small".to_owned(), 400),
            ("parse/gone/chunk".to_owned(), 500),
        ]
        .into();
        let head: Run = [
            ("parse/nanonbt-serde/small".to_owned(), 90),
            ("parse/fastnbt/small".to_owned(), 220),
            ("skip/nanonbt-serde/chunk".to_owned(), 300),
            ("write/pumpkin/small".to_owned(), 401),
            ("parse/new/chunk".to_owned(), 1),
        ]
        .into();
        let stats = Classified::new(&base, &head);
        assert_eq!(stats.total(), 6);
        assert_eq!(stats.improved.len(), 1);
        assert_eq!(stats.regressed.len(), 1);
        assert_eq!(stats.unchanged, 1);
        assert_eq!(stats.reference, 1);
        assert_eq!(stats.new, 1);
        assert_eq!(stats.removed, 1);
        assert_eq!(
            stats.rows(&stats.improved)[0].id,
            "parse/nanonbt-serde/small"
        );
        assert_eq!(stats.rows(&stats.regressed)[0].id, "parse/fastnbt/small");
    }

    #[test]
    fn reads_a_left_summary() {
        let value = serde_json::json!({
            "profiles": [{ "summaries": { "total": { "summary": { "Callgrind": { "Ir": {
                "metrics": { "Left": { "Int": 3835 } }
            } } } } } }]
        });
        assert_eq!(instructions(&value), Some(3835));
    }

    #[test]
    fn reads_a_both_summary_current_side() {
        let value = serde_json::json!({
            "profiles": [{ "summaries": { "total": { "summary": { "Callgrind": { "Ir": {
                "metrics": { "Both": [{ "Int": 3835 }, { "Int": 3962 }] }
            } } } } } }]
        });
        assert_eq!(instructions(&value), Some(3835));
    }

    #[test]
    fn renders_a_small_comment() {
        let body = render(&small_options(), &small_platforms(), false).unwrap();
        assert!(body.contains("## Benchmark results"));
        assert!(body.contains("01234567"));
        assert!(body.contains("fedcba98"));
        assert!(body.contains("| linux-x86_64 | callgrind instructions | 1 | 1 | 0 | 0 | — |"));
        assert!(body.contains("| `parse/nanonbt-serde/small` | 100 | 90 | 🟢 -10.0% |"));
        assert!(body.contains("| `parse/nanonbt-serde/small` | 90 | 🟢 -10.0% |"));
    }

    #[test]
    fn compacts_the_collapsed_tables() {
        let body = render(&small_options(), &small_platforms(), true).unwrap();
        // The changed table keeps both counts; the collapsed table drops its
        // count column so every platform fits one comment.
        assert!(body.contains("| `parse/nanonbt-serde/small` | 100 | 90 | 🟢 -10.0% |"));
        assert!(body.contains("| `parse/nanonbt-serde/small` | 🟢 -10.0% |"));
        assert!(!body.contains("| `parse/nanonbt-serde/small` | 90 | 🟢 -10.0% |"));
    }

    #[test]
    fn leaves_unchanged_entries_out_of_the_collapsed_tables() {
        let body = render(
            &small_options(),
            &small_platforms_with_an_unchanged_entry(),
            false,
        )
        .unwrap();
        // The unchanged entry is still counted above, but its row is left
        // out; the reference entry, which no verdict covers, keeps its row.
        assert!(body.contains(
            "**1 improved · 0 regressed · 1 unchanged** of 2 entries compared — 1 reference."
        ));
        assert!(body.contains("<summary>All 3 entries · 1 unchanged omitted</summary>"));
        assert!(body.contains("| `parse/nanonbt-serde/small` | 90 | 🟢 -10.0% |"));
        assert!(body.contains("| `parse/pumpkin/small` | 401 | not compared |"));
        assert!(!body.contains("parse/nanonbt-serde/chunk"));
    }

    #[test]
    fn drops_the_collapsed_tables_when_no_entry_is_listed() {
        let run: Run = [("parse/nanonbt-serde/small".to_owned(), 100)].into();
        let platforms = [Platform::Counts {
            name: "linux-x86_64".to_owned(),
            kind: Kind::Callgrind,
            base: run.clone(),
            head: run,
        }];
        let body = render(&small_options(), &platforms, false).unwrap();
        assert!(body.contains("**0 improved · 0 regressed · 1 unchanged** of 1 entries compared."));
        assert!(!body.contains("<details>"));
    }

    fn small_options() -> Options {
        Options {
            base: "0123456789abcdef".to_owned(),
            head: "fedcba9876543210".to_owned(),
            base_ref: "main".to_owned(),
            run_url: Some("https://example.invalid/run/1".to_owned()),
            title: "Benchmark results".to_owned(),
            comparisons: Vec::new(),
            smoke_dir: None,
            smokes: Vec::new(),
        }
    }

    fn small_platforms() -> [Platform; 1] {
        let base: Run = [("parse/nanonbt-serde/small".to_owned(), 100)].into();
        let head: Run = [("parse/nanonbt-serde/small".to_owned(), 90)].into();
        [Platform::Counts {
            name: "linux-x86_64".to_owned(),
            kind: Kind::Callgrind,
            base,
            head,
        }]
    }

    fn small_platforms_with_an_unchanged_entry() -> [Platform; 1] {
        let base: Run = [
            ("parse/nanonbt-serde/small".to_owned(), 100),
            ("parse/nanonbt-serde/chunk".to_owned(), 50),
            ("parse/pumpkin/small".to_owned(), 400),
        ]
        .into();
        let head: Run = [
            ("parse/nanonbt-serde/small".to_owned(), 90),
            ("parse/nanonbt-serde/chunk".to_owned(), 50),
            ("parse/pumpkin/small".to_owned(), 401),
        ]
        .into();
        [Platform::Counts {
            name: "linux-x86_64".to_owned(),
            kind: Kind::Callgrind,
            base,
            head,
        }]
    }
}
