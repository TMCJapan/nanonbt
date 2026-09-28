//! Renders the `compare` benchmark's iai-callgrind summaries as the pull
//! request comment the `Bench` workflow posts.
//!
//! The workflow benches both revisions in one job: it checks the base commit
//! out, lays the pull request's `nanonbt-bench` over the base tree and runs
//! the suite, keeps those summaries aside, checks the pull request out and
//! runs the same suite again. Each side's `summary.json` files hold one
//! instruction count per entry, and this tool pairs the two counts by
//! benchmark id.
//!
//! `iai-callgrind` counts instructions under callgrind, so a count does not
//! depend on the machine or on the run and the two sides compare as exact
//! numbers: no threshold, no repeated passes, no noise. The `pumpkin` target
//! is the exception: its compounds are `std` hash maps, the order
//! serialization walks them follows each process's random seed, and the seed
//! moves the counts of those entries by as much as ten percent between runs.
//! The report lists them for reference only instead of reading that move as
//! a change.
//!
//! Usage: `cargo run --example bench-summary -- <BASE> <HEAD> [BASE_SHA]
//! [HEAD_SHA] [BASE_BRANCH]`, where `BASE` and `HEAD` are the two trees of
//! `summary.json` files; `BASE_SHA` and `HEAD_SHA` are the shas the header
//! names the two sides by, defaulting to `base` and `PR`; and `BASE_BRANCH`
//! labels the base side in the tables, defaulting to `main`. A sha label
//! longer than eight characters is shortened to one.

use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fmt::{self, Write as _};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use serde_json::Value;

/// The benchmark target the report does not compare. `pumpkin`'s compounds
/// are `std` hash maps, and serialization walks them in seed order, so its
/// counts move between runs; the report lists them for reference only.
const REFERENCE_TARGET: &str = "pumpkin";

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
    let mut args = env::args().skip(1);
    let (Some(base_dir), Some(head_dir)) = (args.next(), args.next()) else {
        return Err(
            "usage: bench-summary <BASE> <HEAD> [BASE_SHA] [HEAD_SHA] [BASE_BRANCH]".into(),
        );
    };
    let base = args.next().unwrap_or_else(|| "base".to_owned());
    let head = args.next().unwrap_or_else(|| "PR".to_owned());
    let base_branch = args.next().unwrap_or_else(|| "main".to_owned());

    let runs = [
        read_run(Path::new(&base_dir))?,
        read_run(Path::new(&head_dir))?,
    ];
    if runs[0].is_empty() || runs[1].is_empty() {
        return Err(format!("no summary.json files under {base_dir} or {head_dir}").into());
    }
    print!("{}", render(&runs, &base, &head, &base_branch)?);
    Ok(())
}

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

/// Reads every `summary.json` under `root`, keyed by the benchmark id the
/// report shows.
fn read_run(root: &Path) -> Result<Run> {
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

/// The report id of one entry.
///
/// Every entry is named `<kind>_<target>_<id>`, and the report writes that id
/// `<kind>/<target>/<id>` with `-` for the underscores of the target and the
/// id: `parse_nanonbt_serde_short_names` with the bench id `short_names` is
/// `parse/nanonbt-serde/short-names`. A name that does not end in the id its
/// `#[bench]` attribute carries fits none of the report's rows, so it is not
/// a summary this tool can place.
fn display_id(function_name: &str, id: &str) -> Option<String> {
    let prefix = function_name.strip_suffix(id)?.strip_suffix('_')?;
    let (kind, target) = prefix.split_once('_')?;
    Some(format!(
        "{kind}/{}/{}",
        target.replace('_', "-"),
        id.replace('_', "-")
    ))
}

/// The target of a display id: the middle of `<kind>/<target>/<id>`.
fn target(id: &str) -> Option<&str> {
    let (_, rest) = id.split_once('/')?;
    Some(rest.split_once('/')?.0)
}

/// The instruction count of one run: the first profile's total `Ir`.
///
/// `Left` is the measured side; a summary saved against an iai-callgrind
/// baseline would carry the baseline's counts in `Right`, which this tool
/// never consults.
fn instructions(value: &Value) -> Option<u64> {
    value
        .get("profiles")?
        .as_array()?
        .iter()
        .find_map(|profile| {
            profile
                .pointer("/summaries/total/summary/Callgrind/Ir/metrics/Left/Int")?
                .as_u64()
        })
}

/// One benchmark across the two runs.
struct Entry {
    id: String,
    base: Option<u64>,
    head: Option<u64>,
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

impl Entry {
    /// Whether the entry's counts are stable enough to compare; when they are
    /// not, the report lists the entry for reference.
    fn comparable(&self) -> bool {
        target(&self.id) != Some(REFERENCE_TARGET)
    }

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

fn render(
    runs: &[Run; 2],
    base: &str,
    head: &str,
    base_branch: &str,
) -> std::result::Result<String, fmt::Error> {
    let ids: BTreeSet<&str> = runs
        .iter()
        .flat_map(|run| run.keys().map(String::as_str))
        .collect();
    let entries: Vec<Entry> = ids
        .into_iter()
        .map(|id| Entry {
            id: id.to_owned(),
            base: runs[0].get(id).copied(),
            head: runs[1].get(id).copied(),
        })
        .collect();

    let mut improved = Vec::new();
    let mut regressed = Vec::new();
    let mut unchanged = 0;
    let mut reference = 0;
    let mut new = 0;
    let mut removed = 0;
    for entry in &entries {
        match entry.verdict() {
            Some(Verdict::Improved) => improved.push(entry),
            Some(Verdict::Regressed) => regressed.push(entry),
            Some(Verdict::Unchanged) => unchanged += 1,
            None if entry.base.is_some() && entry.head.is_none() => removed += 1,
            None if entry.head.is_some() && entry.base.is_none() => new += 1,
            None if !entry.comparable() => reference += 1,
            None => {}
        }
    }
    // The tables lead with the biggest changes, which are the ones to look
    // at first.
    improved.sort_by(|a, b| {
        a.change()
            .unwrap_or_default()
            .total_cmp(&b.change().unwrap_or_default())
    });
    regressed.sort_by(|a, b| {
        b.change()
            .unwrap_or_default()
            .total_cmp(&a.change().unwrap_or_default())
    });

    let mut out = String::new();
    writeln!(out, "## Benchmark results\n")?;
    writeln!(
        out,
        "`{}` (this pull request) against `{}` ({base_branch}). Both sides ran the same suite \
         under callgrind, which counts instructions, so the counts are exact and a negative \
         change is fewer instructions.\n",
        short_label(head),
        short_label(base),
    )?;
    write!(
        out,
        "**{} improved · {} regressed · {unchanged} unchanged** of {} entries compared",
        improved.len(),
        regressed.len(),
        improved.len() + regressed.len() + unchanged,
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
    writeln!(out, ".")?;
    if reference > 0 {
        writeln!(
            out,
            "\n{reference} `{REFERENCE_TARGET}` entries are listed for reference only: their \
             compounds are `std` hash maps, so their counts move between runs with the seed the \
             map is built under."
        )?;
    }
    writeln!(out)?;

    for (title, rows) in [("Improved", &improved), ("Regressed", &regressed)] {
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
            .map_or(entry.id.as_str(), |(group, _)| group);
        groups.entry(group).or_default().push(entry);
    }
    for (&group, rows) in &groups {
        writeln!(out, "#### {group}\n")?;
        table(&mut out, base_branch, rows)?;
    }
    writeln!(out, "</details>\n")?;
    writeln!(
        out,
        "<sub>`iai-callgrind` counts instructions under valgrind, so the same code counts the \
         same instructions on every machine and every run; the comparison needs no noise \
         threshold and no repeated passes.</sub>",
    )?;
    Ok(out)
}

/// Writes one entry table, header and all.
fn table(out: &mut String, base_branch: &str, rows: &[&Entry]) -> fmt::Result {
    writeln!(out, "| Benchmark | {base_branch} | PR | Change |")?;
    writeln!(out, "| --- | ---: | ---: | ---: |")?;
    for entry in rows {
        writeln!(
            out,
            "| `{}` | {} | {} | {} |",
            entry.id,
            count_cell(entry.base),
            count_cell(entry.head),
            change_cell(entry),
        )?;
    }
    out.push('\n');
    Ok(())
}

/// The entry's change cell: the verdict its two counts add up to, how the
/// entry stands out of the comparison, or that the entry is only listed for
/// reference.
fn change_cell(entry: &Entry) -> String {
    if let Some(verdict) = entry.verdict() {
        return marked(verdict, entry.change().unwrap_or_default());
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

/// A change with the mark and emphasis its verdict earns.
fn marked(verdict: Verdict, change: f64) -> String {
    match verdict {
        Verdict::Improved => format!("🟢 **{}**", percent(change)),
        Verdict::Regressed => format!("🔴 **{}**", percent(change)),
        Verdict::Unchanged => "⚪ same".to_owned(),
    }
}

/// An instruction count, or an em dash when the side never ran the entry.
fn count_cell(instructions: Option<u64>) -> String {
    instructions.map_or_else(|| "—".to_owned(), count)
}

/// An instruction count with thousands separators.
fn count(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, digit) in digits.char_indices() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            out.push(',');
        }
        out.push(digit);
    }
    out
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
