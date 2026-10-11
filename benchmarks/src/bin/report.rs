//! Generates a visual benchmark report from criterion's raw estimates.
//!
//! Criterion already writes an HTML report per benchmark under
//! `target/criterion/report`, but its charts are per-benchmark and hard to
//! compare across libraries. This tool groups every criterion benchmark by
//! scenario, ranks the implementations by mean time, and renders bar charts:
//!
//! * an absolute chart (mean time per iteration), and
//! * a relative chart (`1.00x` is the fastest, `N.NNx` is how much slower the
//!   others are).
//!
//! Outputs `index.html`, one `.svg` per chart, and `report.md` next to the
//! criterion output. Run the benchmarks first:
//!
//! ```text
//! cargo bench -p nanonbt-bench --bench compare
//! cargo run -p nanonbt-bench --bin report
//! ```

// Chart axes and label positions intentionally go through `f64`.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss
)]

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use nanonbt_bench::meta;

/// One criterion benchmark measurement.
struct Entry {
    group: String,
    function: String,
    mean_ns: f64,
    change: Option<f64>,
}

/// A rendered bar.
struct Bar {
    label: String,
    /// Drives the bar length (time in ns, or slowdown ratio).
    width_value: f64,
    /// Text drawn at the end of the bar.
    value_text: String,
    color: &'static str,
}

fn main() {
    let mut criterion_dir = None;
    let mut out_dir = None;

    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--criterion-dir" => criterion_dir = args.next().map(PathBuf::from),
            "--out-dir" => out_dir = args.next().map(PathBuf::from),
            "--help" | "-h" => {
                println!(
                    "usage: report [--criterion-dir DIR] [--out-dir DIR]\n\
                     \n\
                     Generates target/nanonbt-bench-report/index.html and report.md from\n\
                     criterion's estimates in target/criterion."
                );
                return;
            }
            other => {
                eprintln!("unknown argument: {other}");
                std::process::exit(2);
            }
        }
    }

    let criterion_dir = criterion_dir.unwrap_or_else(find_criterion_dir);
    if !criterion_dir.is_dir() {
        eprintln!(
            "criterion output not found at {}; run `cargo bench -p nanonbt-bench --bench compare` first",
            criterion_dir.display()
        );
        std::process::exit(1);
    }

    let out_dir = out_dir.unwrap_or_else(|| {
        criterion_dir
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join("nanonbt-bench-report")
    });

    let mut entries = Vec::new();
    collect_entries(&criterion_dir, &mut entries);
    if entries.is_empty() {
        eprintln!(
            "no criterion estimates found under {}; run the benchmarks first",
            criterion_dir.display()
        );
        std::process::exit(1);
    }

    let mut lengths = BTreeMap::new();
    for entry in &entries {
        lengths
            .entry(entry.group.clone())
            .or_insert_with(|| meta::encoded_len(&entry.group));
    }

    if let Err(error) = fs::create_dir_all(&out_dir) {
        eprintln!("failed to create {}: {error}", out_dir.display());
        std::process::exit(1);
    }

    let html = render_html(&entries, &lengths);
    let markdown = render_markdown(&entries, &lengths);

    let html_path = out_dir.join("index.html");
    let md_path = out_dir.join("report.md");
    fs::write(&html_path, html).expect("write index.html");
    fs::write(&md_path, markdown).expect("write report.md");

    print_summary(&entries);
    println!();
    println!("HTML report:     {}", html_path.display());
    println!("Markdown report: {}", md_path.display());
    println!(
        "Criterion report: {}",
        criterion_dir.join("report/index.html").display()
    );
}

fn find_criterion_dir() -> PathBuf {
    if let Some(target_dir) = std::env::var_os("CARGO_TARGET_DIR") {
        let candidate = PathBuf::from(target_dir).join("criterion");
        if candidate.is_dir() {
            return candidate;
        }
    }

    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let mut dir = cwd.as_path();
    loop {
        let candidate = dir.join("target").join("criterion");
        if candidate.is_dir() {
            return candidate;
        }
        match dir.parent() {
            Some(parent) => dir = parent,
            None => break,
        }
    }
    cwd.join("target").join("criterion")
}

fn collect_entries(dir: &Path, out: &mut Vec<Entry>) {
    let Ok(read_dir) = fs::read_dir(dir) else {
        return;
    };
    for entry in read_dir.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        if path.file_name().is_some_and(|name| name == "new") {
            if let Some(entry) = read_measurement(&path) {
                out.push(entry);
            }
        } else {
            collect_entries(&path, out);
        }
    }
}

fn read_measurement(new_dir: &Path) -> Option<Entry> {
    let benchmark: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(new_dir.join("benchmark.json")).ok()?).ok()?;
    let group = benchmark["group_id"].as_str()?.to_owned();
    let function = benchmark["function_id"]
        .as_str()
        .unwrap_or("benchmark")
        .to_owned();

    let estimates: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(new_dir.join("estimates.json")).ok()?).ok()?;
    // Criterion stores point estimates in nanoseconds.
    let mean_ns = estimates["mean"]["point_estimate"].as_f64()?;

    let change = new_dir
        .parent()
        .map(|bench_dir| bench_dir.join("change/estimates.json"))
        .and_then(|path| fs::read_to_string(path).ok())
        .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
        .and_then(|json| json["mean"]["point_estimate"].as_f64());

    Some(Entry {
        group,
        function,
        mean_ns,
        change,
    })
}

fn library_color(function: &str) -> &'static str {
    if function.starts_with("nanonbt") {
        "#2e7d32"
    } else if function.starts_with("fastnbt") {
        "#1565c0"
    } else if function.starts_with("simdnbt") {
        "#ef6c00"
    } else {
        "#616161"
    }
}

fn format_time(ns: f64) -> String {
    if ns < 1_000.0 {
        format!("{ns:.1} ns")
    } else if ns < 1_000_000.0 {
        format!("{:.2} µs", ns / 1_000.0)
    } else if ns < 1_000_000_000.0 {
        format!("{:.2} ms", ns / 1_000_000.0)
    } else {
        format!("{:.2} s", ns / 1_000_000_000.0)
    }
}

fn format_throughput(bytes: usize, mean_ns: f64) -> String {
    let per_second = bytes as f64 / (mean_ns / 1e9);
    if per_second >= 1024.0 * 1024.0 * 1024.0 {
        format!("{:.2} GiB/s", per_second / (1024.0 * 1024.0 * 1024.0))
    } else {
        format!("{:.1} MiB/s", per_second / (1024.0 * 1024.0))
    }
}

fn format_change(change: f64) -> (String, &'static str) {
    let percent = change * 100.0;
    if percent < -0.5 {
        (format!("{percent:+.1}% faster"), "good")
    } else if percent > 0.5 {
        (format!("{percent:+.1}% slower"), "bad")
    } else {
        (format!("{percent:+.1}%"), "flat")
    }
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Renders a horizontal bar chart as an inline SVG.
fn bar_chart(rows: &[Bar]) -> String {
    const LABEL_WIDTH: f64 = 340.0;
    const CHART_WIDTH: f64 = 620.0;
    const VALUE_WIDTH: f64 = 150.0;
    const ROW_HEIGHT: f64 = 24.0;
    const BAR_HEIGHT: f64 = 14.0;

    let width = LABEL_WIDTH + CHART_WIDTH + VALUE_WIDTH;
    let height = (rows.len() as f64).mul_add(ROW_HEIGHT, 8.0);
    let max = rows
        .iter()
        .map(|row| row.width_value)
        .fold(0.0f64, f64::max)
        .max(f64::MIN_POSITIVE);

    let mut svg = String::new();
    let _ = write!(
        svg,
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width:.0}\" height=\"{height:.0}\" \
         viewBox=\"0 0 {width:.0} {height:.0}\" role=\"img\">"
    );
    for (index, row) in rows.iter().enumerate() {
        let y = (index as f64).mul_add(ROW_HEIGHT, 4.0);
        let bar_width = (row.width_value / max * CHART_WIDTH).max(2.0);
        let _ = write!(
            svg,
            "<text x=\"4\" y=\"{:.1}\" font-family=\"monospace\" font-size=\"12\" fill=\"#212121\">{}</text>",
            y + BAR_HEIGHT - 2.0,
            escape(&row.label)
        );
        let _ = write!(
            svg,
            "<rect x=\"{LABEL_WIDTH:.0}\" y=\"{y:.1}\" width=\"{bar_width:.1}\" height=\"{BAR_HEIGHT:.0}\" fill=\"{}\"/>",
            row.color
        );
        let _ = write!(
            svg,
            "<text x=\"{:.1}\" y=\"{:.1}\" font-family=\"monospace\" font-size=\"12\" fill=\"#212121\">{}</text>",
            LABEL_WIDTH + bar_width + 6.0,
            y + BAR_HEIGHT - 2.0,
            escape(&row.value_text)
        );
    }
    svg.push_str("</svg>");
    svg
}

fn ordered_groups(entries: &[Entry]) -> Vec<String> {
    let mut groups: Vec<String> = entries.iter().map(|entry| entry.group.clone()).collect();
    groups.sort();
    groups.dedup();

    let mut ordered = Vec::new();
    for scenario in meta::SCENARIOS {
        if groups.iter().any(|group| group == scenario.group) {
            ordered.push(scenario.group.to_owned());
        }
    }
    for group in groups {
        if !ordered.contains(&group) {
            ordered.push(group);
        }
    }
    ordered
}

fn render_html(entries: &[Entry], lengths: &BTreeMap<String, usize>) -> String {
    let groups = ordered_groups(entries);
    let mut html = String::new();
    html.push_str(
        "<!doctype html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n\
         <title>nanonbt benchmark report</title>\n\
         <style>\n\
         body { font-family: -apple-system, 'Segoe UI', Roboto, sans-serif; margin: 2rem auto; max-width: 1180px; padding: 0 1rem; color: #212121; }\n\
         h1 { margin-bottom: 0.2rem; }\n\
         h2 { margin-top: 2.2rem; border-bottom: 1px solid #e0e0e0; padding-bottom: 0.3rem; }\n\
         .description { color: #555; margin-top: 0; }\n\
         table { border-collapse: collapse; width: 100%; margin: 0.8rem 0 1.4rem; }\n\
         th, td { text-align: left; padding: 0.35rem 0.6rem; border-bottom: 1px solid #eee; font-size: 0.92rem; }\n\
         th { background: #f5f5f5; }\n\
         td.num, th.num { text-align: right; font-variant-numeric: tabular-nums; }\n\
         .good { color: #2e7d32; }\n\
         .bad { color: #c62828; }\n\
         .flat { color: #757575; }\n\
         .legend span { display: inline-block; margin-right: 1.2rem; font-size: 0.9rem; }\n\
         .legend i { display: inline-block; width: 12px; height: 12px; margin-right: 0.35rem; }\n\
         .note { color: #666; font-size: 0.88rem; }\n\
         </style>\n</head>\n<body>\n",
    );
    html.push_str("<h1>nanonbt benchmark report</h1>\n");
    let _ = writeln!(
        html,
        "<p class=\"note\">Generated from <code>{}</code>. Lower is better. \
         The relative chart uses <code>1.00x</code> for the fastest implementation; \
         larger numbers are slower. Reads include a full traversal/checksum of the payload.</p>",
        escape("target/criterion")
    );
    html.push_str(
        "<p class=\"legend\">\
         <span><i style=\"background:#2e7d32\"></i>nanonbt</span>\
         <span><i style=\"background:#1565c0\"></i>fastnbt</span>\
         <span><i style=\"background:#ef6c00\"></i>simdnbt</span></p>\n",
    );

    for group in &groups {
        let scenario = meta::SCENARIOS
            .iter()
            .find(|scenario| scenario.group == group);
        let mut group_entries: Vec<&Entry> = entries
            .iter()
            .filter(|entry| &entry.group == group)
            .collect();
        group_entries.sort_by(|a, b| a.mean_ns.partial_cmp(&b.mean_ns).unwrap());
        let fastest = group_entries
            .first()
            .map_or(1.0, |entry| entry.mean_ns.max(f64::MIN_POSITIVE));
        let bytes = lengths.get(group).copied().unwrap_or(0);

        let _ = writeln!(html, "<h2>{}</h2>", escape(group));
        if let Some(scenario) = scenario {
            let _ = writeln!(
                html,
                "<p class=\"description\">{} <span class=\"note\">(encoded size: {} bytes)</span></p>",
                escape(scenario.description),
                bytes
            );
        }

        let absolute_bars: Vec<Bar> = group_entries
            .iter()
            .map(|entry| Bar {
                label: entry.function.clone(),
                width_value: entry.mean_ns,
                value_text: format!(
                    "{}  {}",
                    format_time(entry.mean_ns),
                    format_throughput(bytes, entry.mean_ns)
                ),
                color: library_color(&entry.function),
            })
            .collect();
        let relative_bars: Vec<Bar> = group_entries
            .iter()
            .map(|entry| {
                let ratio = entry.mean_ns / fastest;
                Bar {
                    label: entry.function.clone(),
                    width_value: ratio,
                    value_text: format!("{ratio:.2}x"),
                    color: library_color(&entry.function),
                }
            })
            .collect();

        let _ = writeln!(
            html,
            "<h3>Mean time</h3>\n{}\n<h3>Relative to fastest</h3>\n{}",
            bar_chart(&absolute_bars),
            bar_chart(&relative_bars)
        );

        html.push_str(
            "<table>\n<tr><th>Benchmark</th><th class=\"num\">Mean</th>\
             <th class=\"num\">Throughput</th><th class=\"num\">vs fastest</th>\
             <th class=\"num\">vs previous run</th></tr>\n",
        );
        for entry in &group_entries {
            let ratio = entry.mean_ns / fastest;
            let change = entry
                .change
                .map_or_else(|| ("—".to_owned(), "flat"), format_change);
            let _ = writeln!(
                html,
                "<tr><td>{}</td><td class=\"num\">{}</td><td class=\"num\">{}</td>\
                 <td class=\"num\">{ratio:.2}x</td><td class=\"num {}\">{}</td></tr>",
                escape(&entry.function),
                format_time(entry.mean_ns),
                format_throughput(bytes, entry.mean_ns),
                change.1,
                change.0
            );
        }
        html.push_str("</table>\n");
    }

    html.push_str("</body>\n</html>\n");
    html
}

fn render_markdown(entries: &[Entry], lengths: &BTreeMap<String, usize>) -> String {
    let groups = ordered_groups(entries);
    let mut markdown = String::new();
    markdown.push_str("# nanonbt benchmark report\n\n");
    markdown
        .push_str("Lower is better. Reads include a full traversal/checksum of the payload.\n\n");

    for group in &groups {
        let scenario = meta::SCENARIOS
            .iter()
            .find(|scenario| scenario.group == group);
        let mut group_entries: Vec<&Entry> = entries
            .iter()
            .filter(|entry| &entry.group == group)
            .collect();
        group_entries.sort_by(|a, b| a.mean_ns.partial_cmp(&b.mean_ns).unwrap());
        let fastest = group_entries
            .first()
            .map_or(1.0, |entry| entry.mean_ns.max(f64::MIN_POSITIVE));
        let bytes = lengths.get(group).copied().unwrap_or(0);

        let _ = write!(markdown, "## {group}\n\n");
        if let Some(scenario) = scenario {
            let _ = writeln!(markdown, "{}\n", scenario.description);
        }
        let _ = writeln!(
            markdown,
            "| Benchmark | Mean | Throughput | vs fastest | vs previous |\n|---|---:|---:|---:|---:|"
        );
        for entry in &group_entries {
            let ratio = entry.mean_ns / fastest;
            let _ = writeln!(
                markdown,
                "| `{}` | {} | {} | {:.2}x | {} |",
                entry.function,
                format_time(entry.mean_ns),
                format_throughput(bytes, entry.mean_ns),
                ratio,
                entry
                    .change
                    .map_or_else(|| "—".to_owned(), |change| format_change(change).0),
            );
        }
        markdown.push('\n');
    }
    markdown
}

fn print_summary(entries: &[Entry]) {
    let groups = ordered_groups(entries);
    println!("Fastest implementation per scenario:");
    for group in &groups {
        let mut group_entries: Vec<&Entry> = entries
            .iter()
            .filter(|entry| &entry.group == group)
            .collect();
        group_entries.sort_by(|a, b| a.mean_ns.partial_cmp(&b.mean_ns).unwrap());
        let Some(fastest) = group_entries.first() else {
            continue;
        };
        let best_nanonbt = group_entries
            .iter()
            .find(|entry| entry.function.starts_with("nanonbt"));
        let nanonbt_note = best_nanonbt.map_or_else(
            || "nanonbt: n/a".to_owned(),
            |entry| {
                let ratio = entry.mean_ns / fastest.mean_ns;
                format!("nanonbt best: {} ({ratio:.2}x)", entry.function)
            },
        );
        println!(
            "  {group:<12} {:>10}  {:<24} {nanonbt_note}",
            format_time(fastest.mean_ns),
            fastest.function
        );
    }
}
