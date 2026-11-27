//! Generate the port-status table and chart from areas/*.md.

use anyhow::{bail, ensure, Context, Result};
use std::{fs, path::Path};

const START: &str = "<!-- PORT-STATUS:START -->";
const END: &str = "<!-- PORT-STATUS:END -->";

pub(crate) enum Mode {
    Write,
    Check,
    Summary,
}

struct Area {
    id: String,
    name: String,
    order: i32,
    percent: u32,
    code: Vec<String>,
    scope: String,
}

pub(crate) fn run(root: &Path, mode: Mode) -> Result<()> {
    let areas = read_areas(root)?;
    ensure!(!areas.is_empty(), "areas/ has no area files");
    let total = (areas.iter().map(|a| a.percent).sum::<u32>() as f64 / areas.len() as f64).round()
        as u32;
    let body = table(&areas, total);
    let svg = chart(&areas, total);

    match mode {
        Mode::Summary => {
            println!("{body}");
            return Ok(());
        }
        Mode::Check => {
            let readme = fs::read_to_string(root.join("README.md"))?;
            let current = replace_block(&readme, &body)?;
            let expected_svg = fs::read_to_string(root.join("docs/port-status.svg")).unwrap_or_default();
            let mut stale = Vec::new();
            if current != readme {
                stale.push("README.md");
            }
            if expected_svg != svg {
                stale.push("docs/port-status.svg");
            }
            if !stale.is_empty() {
                bail!("Port status is stale: {}. Run the port-status command.", stale.join(", "));
            }
            println!("Port status is current ({total}%).");
        }
        Mode::Write => {
            let readme = fs::read_to_string(root.join("README.md"))?;
            fs::write(root.join("README.md"), replace_block(&readme, &body)?)?;
            fs::write(root.join("docs/port-status.svg"), svg)?;
            println!("Port status: {total}% across {} areas.", areas.len());
        }
    }
    Ok(())
}

fn read_areas(root: &Path) -> Result<Vec<Area>> {
    let mut areas = Vec::new();
    for entry in fs::read_dir(root.join("areas")).context("read areas/")? {
        let path = entry?.path();
        if path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        if path.file_name().and_then(|n| n.to_str()) == Some("README.md") {
            continue;
        }
        let text = fs::read_to_string(&path)?;
        if !text.trim_start().starts_with("---") {
            continue;
        }
        areas.push(parse_area(&path, &text)?);
    }
    areas.sort_by_key(|a| a.order);
    Ok(areas)
}

fn parse_area(path: &Path, text: &str) -> Result<Area> {
    let label = path.display();
    let lines: Vec<&str> = text.lines().collect();
    ensure!(lines.first() == Some(&"---"), "{label}: missing frontmatter");
    let close = lines
        .iter()
        .skip(1)
        .position(|l| l.trim_end() == "---")
        .map(|i| i + 1)
        .with_context(|| format!("{label}: unterminated frontmatter"))?;

    let mut id = None;
    let mut name = None;
    let mut order = None;
    let mut percent = None;
    let mut code = None;
    for line in &lines[1..close] {
        let Some((key, value)) = line.split_once(':') else { continue };
        let value = value.trim().to_owned();
        match key.trim() {
            "id" => id = Some(value),
            "name" => name = Some(value),
            "order" => order = Some(value.parse::<i32>()?),
            "percent" => percent = Some(value.parse::<u32>()?),
            "code" => {
                code = Some(
                    value
                        .split(',')
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                        .map(str::to_owned)
                        .collect::<Vec<_>>(),
                )
            }
            _ => {}
        }
    }

    let percent = percent.context(format!("{label}: missing percent"))?;
    ensure!(percent <= 100, "{label}: percent must be 0-100");

    let title = lines[close + 1..]
        .iter()
        .position(|l| l.starts_with("# "))
        .map(|i| i + close + 1)
        .with_context(|| format!("{label}: missing title"))?;
    let end = lines[title + 1..]
        .iter()
        .position(|l| l.starts_with("## "))
        .map(|i| i + title + 1)
        .unwrap_or(lines.len());
    let scope = lines[title + 1..end]
        .iter()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    ensure!(!scope.is_empty(), "{label}: missing scope paragraph");

    Ok(Area {
        id: id.context(format!("{label}: missing id"))?,
        name: name.context(format!("{label}: missing name"))?,
        order: order.context(format!("{label}: missing order"))?,
        percent,
        code: code.context(format!("{label}: missing code"))?,
        scope,
    })
}

fn table(areas: &[Area], total: u32) -> String {
    let mut out = String::new();
    out.push_str("## Port map\n\n");
    out.push_str(&format!(
        "**Overall: {total}%** — mean across {} areas. A handoff names one area; it includes its Rust rules, UI and desktop/browser/Android behaviour. Once the contract is stable, UI and cross-platform work run as two concurrent subagents.\n\n",
        areas.len()
    ));
    out.push_str("| Area | What it covers | Code home | Ported |\n|---|---|---|---|\n");
    for a in areas {
        let code = a
            .code
            .iter()
            .map(|c| format!("`{c}`"))
            .collect::<Vec<_>>()
            .join(", ");
        out.push_str(&format!(
            "| [{}](areas/{}.md) | {} | {} | {}% |\n",
            a.name,
            a.id,
            a.scope.replace('|', "\\|"),
            code,
            a.percent
        ));
    }
    out.push_str("\n![Port status](docs/port-status.svg)\n\n");
    out.push_str("**How the percentage works.** 0% is not started. 100% means every workflow in that area matches the internal tool on desktop, browser and Android — behaviour and UI, observed in the running app. Count verified user-visible behaviour only, never code, tests or scaffolding. When unsure, lower it. Change an area's `percent` in `areas/<id>.md`; the table and chart regenerate on commit.\n");
    out
}

fn replace_block(text: &str, body: &str) -> Result<String> {
    let a = text.find(START).context("README.md: missing PORT-STATUS start")?;
    let b = text.find(END).context("README.md: missing PORT-STATUS end")?;
    ensure!(b > a, "README.md: PORT-STATUS markers are out of order");
    let mut out = String::with_capacity(text.len() + body.len());
    out.push_str(&text[..a + START.len()]);
    out.push('\n');
    out.push_str(body.trim_end());
    out.push('\n');
    out.push_str(&text[b..]);
    Ok(out)
}

fn chart(areas: &[Area], total: u32) -> String {
    const W: u32 = 980;
    const ROW: u32 = 26;
    const TOP: u32 = 96;
    const LABEL: u32 = 250;
    const BAR: u32 = 580;
    const RADIUS: f64 = 62.0;
    let height = TOP + ROW * areas.len() as u32 + 24;
    let circumference = 2.0 * std::f64::consts::PI * RADIUS;
    let dash = total as f64 / 100.0 * circumference;

    let mut bars = String::new();
    for (i, a) in areas.iter().enumerate() {
        let y = TOP + i as u32 * ROW;
        let width = (a.percent as f64 / 100.0 * BAR as f64).round() as u32;
        let color = if a.percent == 0 { "#c9d3e0" } else { "#4f8cff" };
        bars.push_str(&format!(
            concat!(
                "<text x=\"24\" y=\"{y}\" class=\"label\">{name}</text>",
                "<rect x=\"{label}\" y=\"{ty}\" width=\"{bar}\" height=\"12\" rx=\"6\" class=\"track\"/>",
                "<rect x=\"{label}\" y=\"{ty}\" width=\"{width}\" height=\"12\" rx=\"6\" fill=\"{color}\"/>",
                "<text x=\"{right}\" y=\"{y}\" text-anchor=\"end\" class=\"pct\">{percent}%</text>"
            ),
            y = y + 15,
            ty = y + 4,
            name = escape(&a.name),
            label = LABEL,
            bar = BAR,
            width = width,
            color = color,
            right = W - 24,
            percent = a.percent
        ));
    }

    let template = r#"<svg xmlns="http://www.w3.org/2000/svg" width="__W__" height="__H__" viewBox="0 0 __W__ __H__" role="img" aria-label="Port status: __TOTAL__% overall">
<style>
  .bg{fill:#ffffff}.label{font:13px -apple-system,Segoe UI,Roboto,sans-serif;fill:#1f2937}
  .pct{font:600 13px -apple-system,Segoe UI,Roboto,sans-serif;fill:#1f2937}
  .track{fill:#e8edf5}.title{font:600 18px -apple-system,Segoe UI,Roboto,sans-serif;fill:#111827}
  .sub{font:13px -apple-system,Segoe UI,Roboto,sans-serif;fill:#6b7280}
  .donut{fill:none;stroke:#e8edf5;stroke-width:16}.arc{fill:none;stroke:#4f8cff;stroke-width:16;stroke-linecap:round}
  .big{font:600 28px -apple-system,Segoe UI,Roboto,sans-serif;fill:#111827}
  @media (prefers-color-scheme: dark){
    .bg{fill:#0d1117}.label,.pct{fill:#e6edf3}.title,.big{fill:#f0f6fc}.sub{fill:#8b949e}
    .track{fill:#21262d}.donut{stroke:#22272e}
  }
</style>
<rect class="bg" width="__W__" height="__H__" rx="12"/>
<text x="24" y="34" class="title">Port status</text>
<text x="24" y="56" class="sub">Mean across __COUNT__ areas; verified behaviour and UI only.</text>
<g transform="translate(24,__DONUT_Y__)">
  <circle class="donut" cx="__R__" cy="__R__" r="__R__"/>
  <circle class="arc" cx="__R__" cy="__R__" r="__R__" stroke-dasharray="__DASH__ __REST__" transform="rotate(-90 __R__ __R__)"/>
  <text x="__R__" y="__BIG_Y__" text-anchor="middle" class="big">__TOTAL__%</text>
  <text x="__R__" y="__SUB_Y__" text-anchor="middle" class="sub">overall</text>
</g>
__BARS__
</svg>
"#;
    template
        .replace("__W__", &W.to_string())
        .replace("__H__", &height.to_string())
        .replace("__TOTAL__", &total.to_string())
        .replace("__COUNT__", &areas.len().to_string())
        .replace("__DONUT_Y__", &(TOP - 10).to_string())
        .replace("__R__", &RADIUS.to_string())
        .replace("__DASH__", &format!("{dash:.1}"))
        .replace("__REST__", &format!("{:.1}", circumference - dash))
        .replace("__BIG_Y__", &format!("{}", RADIUS.round() as u32 + 4))
        .replace("__SUB_Y__", &format!("{}", RADIUS.round() as u32 + 26))
        .replace("__BARS__", &bars)
}

fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
