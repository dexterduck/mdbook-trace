use std::{
    cell::{RefCell, RefMut},
    collections::{HashMap, HashSet},
    io,
    path::PathBuf,
};

use clap::{Parser, Subcommand};
use mdbook::book::{Book, Chapter};
use mdbook::errors::Error;
use mdbook::preprocess::{CmdPreprocessor, Preprocessor, PreprocessorContext};
use mdbook::BookItem;
use once_cell::sync::Lazy;
use regex::Regex;
use semver::{Version, VersionReq};
use serde::Deserialize;

#[derive(Debug, Clone, Parser)]
struct App {
    #[clap(subcommand)]
    pub cmd: Option<Command>,
}

#[derive(Debug, Clone, Subcommand)]
enum Command {
    Supports { renderer: String },
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct Config {
    /// Use fully qualified trace number as in-page footnote number.
    pub qualified_footnotes: bool,
    /// Add chapter numbers to each page title.
    pub chapter_numbers: bool,
    /// Insert a horizontal rule between the page body and the generated footnotes.
    pub footnote_divider: bool,
    pub parent_numbering: ParentNumbering,
    /// Heading to use for the first column of the trace table.
    pub record_heading: String,
    /// Heading to use for the second column of the trace table.
    pub trace_heading: String,
    /// Table of trace targets.
    pub targets: HashMap<String, TargetConfig>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            qualified_footnotes: false,
            footnote_divider: false,
            chapter_numbers: false,
            parent_numbering: ParentNumbering::Zero,
            record_heading: "Record".to_string(),
            trace_heading: "Traces".to_string(),
            targets: HashMap::default(),
        }
    }
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct TargetConfig {
    pub name: String,
}

/// ParentNumbering defines the trace numbering strategy for a page with subchapters.
#[derive(Debug, Clone, Copy, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ParentNumbering {
    /// Number traces as normal.
    /// This will result in traces with the same number as subchapters.
    /// (e.g. the first trace and first subchapter of chapter 1 will both be numbered 1.1)
    AllowDuplicates,
    /// Offset trace numbers from the last subchapter.
    /// (e.g. if chapter 1 has 2 subchapters, the first trace will be numbered 1.3)
    Offset,
    /// Insert a ".0" qualifier before traces in a page with subchapters.
    /// (e.g. if chapter 1 has 1 subchapter, the first trace will be 1.0.1).
    Zero,
}

fn main() {
    let args = App::parse();

    match args.cmd {
        Some(Command::Supports { renderer }) => handle_supports(renderer),
        None => {
            if let Err(e) = handle_preprocessing() {
                eprintln!("{}", e);
                std::process::exit(1);
            }
        }
    }
}

fn handle_preprocessing() -> Result<(), Error> {
    let (ctx, book) = CmdPreprocessor::parse_input(io::stdin())?;

    let config: Config = Config::deserialize(
        ctx.config
            .get("preprocessor.trace")
            .expect("key should exist if preprocessor is active")
            .to_owned(),
    )?;
    let pre = Traceable::new(config);

    let version = Version::parse(&ctx.mdbook_version)?;
    let req_version = VersionReq::parse(mdbook::MDBOOK_VERSION)?;
    if !req_version.matches(&version) {
        eprintln!(
            "Warning: The {} plugin was built against version {} of mdbook, \
             but we're being called from version {}",
            pre.name(),
            mdbook::MDBOOK_VERSION,
            ctx.mdbook_version,
        )
    }

    let processed = pre.run(&ctx, book)?;
    serde_json::to_writer(io::stdout(), &processed)?;

    Ok(())
}

fn handle_supports(renderer: impl AsRef<str>) {
    let pre = Traceable::default();
    if pre.supports_renderer(renderer.as_ref()) {
        std::process::exit(0);
    } else {
        std::process::exit(1);
    }
}

#[derive(Debug, Default)]
pub struct Traceable {
    config: Config,
    targets: RefCell<HashMap<String, Target>>,
}

impl Traceable {
    pub fn new(config: Config) -> Self {
        let targets = config
            .targets
            .iter()
            .map(|(k, v)| {
                let id = k.clone();
                let target = Target::new(&v.name);
                (id, target)
            })
            .collect::<HashMap<_, _>>();
        Self {
            config,
            targets: RefCell::new(targets),
        }
    }

    fn target(&self, target: impl AsRef<str>) -> Result<RefMut<'_, Target>, Error> {
        let targets = self.targets.borrow_mut();
        if targets.contains_key(target.as_ref()) {
            Ok(RefMut::map(targets, |t| {
                t.get_mut(target.as_ref()).unwrap()
            }))
        } else {
            Err(anyhow::anyhow!(
                "no target defined with id '{}'.",
                target.as_ref()
            ))
        }
    }

    fn number_headings(&self, chapter: &mut Chapter) {
        if let Some(number) = &chapter.number {
            chapter.content = HEADING_RE
                .replace(&chapter.content, |caps: &regex::Captures| {
                    let title = caps.name("title").unwrap().as_str();
                    format!("\n# {} {}\n\n", number, title)
                })
                .to_string();
        }
    }

    fn generate_traces(&self, chapter: &mut Chapter) -> Result<(), Error> {
        let mut footnotes = vec![];
        let mut count = 0;
        let mut res = Ok(());

        let content = TRACE_RE
            .replace_all(&chapter.content, |caps: &regex::Captures| {
                count += 1;
                let target = caps.name("target").unwrap().as_str();
                let record = caps.name("record").unwrap().as_str();

                let mut target = match self.target(target) {
                    Ok(t) => t,
                    Err(e) => {
                        res = Err(e);
                        return String::new();
                    }
                };

                let mut number = chapter.number.clone().unwrap_or_default().0;
                match self.config.parent_numbering {
                    ParentNumbering::Zero => {
                        if !chapter.sub_items.is_empty() {
                            number.push(0);
                        }
                        number.push(count);
                    }
                    ParentNumbering::AllowDuplicates => number.push(count),
                    ParentNumbering::Offset => {
                        number.push(count + (chapter.sub_items.len() as u32));
                    }
                };

                let trace = Trace::new(
                    chapter.path.clone(),
                    number,
                    self.config.qualified_footnotes,
                );
                target.add_trace(record, trace.clone());

                let (link, note) = target.footnote(&trace).unwrap();
                let anchor = trace.anchor();
                footnotes.push(note);
                format!("{anchor}{link}")
            })
            .to_string();

        let footer = footnotes.join("\n\n");

        chapter.content = if footer.is_empty() {
            content
        } else if self.config.footnote_divider {
            vec![content, footer].join("\n\n---\n\n")
        } else {
            vec![content, footer].join("\n\n")
        };

        res
    }

    fn generate_tables(&self, chapter: &mut Chapter) -> Result<(), Error> {
        let mut res = Ok(());
        chapter.content = MATRIX_RE
            .replace_all(&chapter.content, |caps: &regex::Captures| {
                let target = caps.name("target").unwrap().as_str();
                match self.target(target) {
                    Ok(t) => t.matrix(&self.config.record_heading, &self.config.trace_heading),
                    Err(e) => {
                        res = Err(e);
                        String::new()
                    }
                }
            })
            .to_string();
        res
    }
}

impl Preprocessor for Traceable {
    fn name(&self) -> &str {
        "trace-preprocessor"
    }

    fn run(&self, _ctx: &PreprocessorContext, mut book: Book) -> Result<Book, Error> {
        let mut res = Ok(());

        book.for_each_mut(|item| {
            if let BookItem::Chapter(chapter) = item {
                if self.config.chapter_numbers {
                    self.number_headings(chapter);
                }
                if let Err(e) = self.generate_traces(chapter) {
                    res = Err(e);
                }
            }
        });
        book.for_each_mut(|item| {
            if let BookItem::Chapter(chapter) = item {
                if let Err(e) = self.generate_tables(chapter) {
                    res = Err(e)
                }
            }
        });
        res?;
        Ok(book)
    }

    fn supports_renderer(&self, renderer: &str) -> bool {
        renderer != "not-supported"
    }
}

#[derive(Debug, Clone)]
struct Target {
    pub name: String,
    pub records: HashMap<String, Record>,
}

impl Target {
    pub fn new(name: impl AsRef<str>) -> Self {
        Self {
            name: name.as_ref().to_string(),
            records: HashMap::default(),
        }
    }

    pub fn add_trace(&mut self, record: impl AsRef<str>, trace: Trace) {
        let record = record.as_ref().to_string();
        self.records
            .entry(record.clone())
            .or_insert_with(|| Record::new(record))
            .add_trace(trace);
    }

    pub fn matrix(
        &self,
        record_heading: impl AsRef<str>,
        trace_heading: impl AsRef<str>,
    ) -> String {
        let mut rows = vec![
            format!(
                "| {} | {} |",
                record_heading.as_ref(),
                trace_heading.as_ref()
            ),
            "|--------|--------|".to_string(),
        ];
        let mut records = self.records.values().cloned().collect::<Vec<_>>();
        records.sort_by(|a, b| a.name.cmp(&b.name));
        for record in records {
            rows.push(format!(
                "| {} | {} |",
                record.name,
                record.references().join(", ")
            ));
        }
        rows.join("\n")
    }

    pub fn footnote(&self, trace: &Trace) -> Option<(String, String)> {
        for (_, record) in self.records.iter() {
            if record.traces.contains(trace) {
                let link = trace.reference();
                let note = format!("{} {} {}", trace.footnote(), self.name, record.name);
                return Some((link, note));
            }
        }
        None
    }
}

#[derive(Debug, Clone, PartialEq)]
struct Record {
    pub name: String,
    pub traces: HashSet<Trace>,
}

impl Record {
    pub fn new(name: impl AsRef<str>) -> Self {
        Self {
            name: name.as_ref().to_string(),
            traces: HashSet::new(),
        }
    }

    pub fn add_trace(&mut self, trace: Trace) {
        self.traces.insert(trace);
    }

    pub fn references(&self) -> Vec<String> {
        self.traces.iter().map(|trace| trace.link()).collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct Trace {
    pub path: Option<PathBuf>,
    pub number: Vec<u32>,
    pub qualified: bool,
}

impl Trace {
    pub fn new(
        path: impl Into<Option<PathBuf>>,
        number: impl IntoIterator<Item = u32>,
        qualified: bool,
    ) -> Self {
        Self {
            path: path.into(),
            number: number.into_iter().collect(),
            qualified,
        }
    }

    fn number(&self, sep: impl AsRef<str>, qualified: bool) -> String {
        if qualified {
            self.number
                .iter()
                .map(|n| n.to_string())
                .collect::<Vec<_>>()
                .join(sep.as_ref())
        } else {
            self.number.last().unwrap().to_string()
        }
    }

    pub fn footnote(&self) -> String {
        format!(
            "<a name=\"note{}\"></a><sup>{}</sup>",
            self.number("_", true),
            self.number(".", self.qualified)
        )
    }

    pub fn reference(&self) -> String {
        format!(
            "<a href=\"#note{}\"><sup>{}</sup></a>",
            self.number("_", true),
            self.number(".", self.qualified)
        )
    }

    pub fn anchor(&self) -> String {
        format!("<a name=\"trace{}\"></a>", self.number("_", true))
    }

    pub fn link(&self) -> String {
        if let Some(path) = &self.path {
            format!(
                "[{}]({}#trace{})",
                self.number(".", true),
                path.display(),
                self.number("_", true),
            )
        } else {
            self.number(".", true)
        }
    }
}

/// Regex that captures a command in one of the following forms:
///   - `{{#trace <target>:<record>}}`
///   - `{{#tr <target>:<record>}}`
static TRACE_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?s)\{\{#(?:trace|tr)\s+(?P<target>[a-zA-Z0-9_\-]+):\s*(?P<record>.*?)\s*\}\}")
        .unwrap()
});
/// Regex that captures a command in one of the following forms:
///   - `{{#tracematrix <target> }}`
///   - `{{#trace_matrix <target> }}`
static MATRIX_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?s)\{\{#(?:tracematrix|trace_matrix)\s+(?P<target>[a-zA-Z0-9_\-]+)\s*\}\}")
        .unwrap()
});
/// Regex that captures a top-level markdown heading `# <Text>`
static HEADING_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"(^|\n)#\s+(?P<title>.*?)\n").unwrap());
