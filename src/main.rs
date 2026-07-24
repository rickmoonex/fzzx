#[cfg(not(target_os = "macos"))]
compile_error!("fzzx only supports macOS");

mod app;

use std::env;
use std::ffi::OsString;
use std::fmt::Display;
use std::fs;
use std::io::{self, BufWriter, IsTerminal, Read, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use lexopt::prelude::*;
use nucleo_matcher::pattern::{AtomKind, CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config as MatcherConfig, Matcher, Utf32Str};

const HELP: &str = r#"A small, scriptable fuzzy picker for macOS

Usage: fzzx [OPTIONS]

Options:
  -d, --dmenu                    Fuzzel-compatible no-op
  -p, --prompt TEXT              Prompt shown before the query
  -l, --lines COUNT              Maximum visible results (1-8)
  -w, --width POINTS             Window width
  -f, --font FONT                Font family, optionally followed by :size=N
      --search TEXT              Initial query
      --index                    Print the original zero-based index
      --no-sort                  Keep matching entries in input order
      --config PATH              Use an explicit config file
      --no-config                Do not load a config file
      --background COLOR         Background color
      --text-color COLOR         Result text color
      --prompt-color COLOR       Prompt text color
      --prompt-background COLOR  Prompt block color
      --selection-color COLOR    Selected row color
      --selection-text-color COLOR
                                  Selected row text color
      --match-color COLOR        Matched character color
  -h, --help                     Print help
  -v, --version                  Print version

Colors accept RRGGBB or RRGGBBAA, with an optional leading '#'.
The default config is $XDG_CONFIG_HOME/fzzx/fzzx.ini or ~/.config/fzzx/fzzx.ini.
"#;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Color {
    pub(crate) red: f64,
    pub(crate) green: f64,
    pub(crate) blue: f64,
    pub(crate) alpha: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct FontSpec {
    pub(crate) family: String,
    pub(crate) size: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Options {
    pub(crate) prompt: String,
    pub(crate) lines: usize,
    pub(crate) width: f64,
    pub(crate) font: Option<FontSpec>,
    pub(crate) search: String,
    pub(crate) index: bool,
    pub(crate) sort: bool,
    pub(crate) background: Option<Color>,
    pub(crate) text_color: Option<Color>,
    pub(crate) prompt_color: Option<Color>,
    pub(crate) prompt_background: Option<Color>,
    pub(crate) selection_color: Option<Color>,
    pub(crate) selection_text_color: Option<Color>,
    pub(crate) match_color: Option<Color>,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            prompt: ">".into(),
            lines: 8,
            width: 640.0,
            font: None,
            search: String::new(),
            index: false,
            sort: true,
            background: None,
            text_color: None,
            prompt_color: None,
            prompt_background: None,
            selection_color: None,
            selection_text_color: None,
            match_color: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Match {
    pub(crate) original: usize,
    pub(crate) indices: Vec<u32>,
}

#[derive(Default)]
struct Overrides {
    prompt: Option<String>,
    lines: Option<usize>,
    width: Option<f64>,
    font: Option<String>,
    search: Option<String>,
    index: bool,
    no_sort: bool,
    config: Option<PathBuf>,
    no_config: bool,
    background: Option<String>,
    text_color: Option<String>,
    prompt_color: Option<String>,
    prompt_background: Option<String>,
    selection_color: Option<String>,
    selection_text_color: Option<String>,
    match_color: Option<String>,
}

#[allow(clippy::large_enum_variant)]
enum Action {
    Run(Overrides),
    Help,
    Version,
}

fn parse_args(args: impl IntoIterator<Item = OsString>) -> Result<Action, String> {
    let mut parser = lexopt::Parser::from_args(args);
    let mut overrides = Overrides::default();

    while let Some(arg) = parser.next().map_err(display)? {
        match arg {
            Short('d') | Long("dmenu") => {}
            Short('p') | Long("prompt") => overrides.prompt = Some(string_value(&mut parser)?),
            Short('l') | Long("lines") => overrides.lines = Some(value(&mut parser)?),
            Short('w') | Long("width") => overrides.width = Some(value(&mut parser)?),
            Short('f') | Long("font") => overrides.font = Some(string_value(&mut parser)?),
            Long("search") => overrides.search = Some(string_value(&mut parser)?),
            Long("index") => overrides.index = true,
            Long("no-sort") => overrides.no_sort = true,
            Long("config") => overrides.config = Some(parser.value().map_err(display)?.into()),
            Long("no-config") => overrides.no_config = true,
            Long("background") => overrides.background = Some(string_value(&mut parser)?),
            Long("text-color") => overrides.text_color = Some(string_value(&mut parser)?),
            Long("prompt-color") => overrides.prompt_color = Some(string_value(&mut parser)?),
            Long("prompt-background") => {
                overrides.prompt_background = Some(string_value(&mut parser)?)
            }
            Long("selection-color") => overrides.selection_color = Some(string_value(&mut parser)?),
            Long("selection-text-color") => {
                overrides.selection_text_color = Some(string_value(&mut parser)?)
            }
            Long("match-color") => overrides.match_color = Some(string_value(&mut parser)?),
            Short('h') | Long("help") => return Ok(Action::Help),
            Short('v') | Long("version") => return Ok(Action::Version),
            _ => return Err(arg.unexpected().to_string()),
        }
    }

    if overrides.config.is_some() && overrides.no_config {
        return Err("--config and --no-config cannot be used together".into());
    }
    Ok(Action::Run(overrides))
}

fn string_value(parser: &mut lexopt::Parser) -> Result<String, String> {
    parser.value().map_err(display)?.string().map_err(display)
}

fn value<T>(parser: &mut lexopt::Parser) -> Result<T, String>
where
    T: std::str::FromStr,
    T::Err: Display,
{
    string_value(parser)?.parse().map_err(display)
}

fn display(error: impl Display) -> String {
    error.to_string()
}

fn options(overrides: Overrides) -> Result<Options, String> {
    let mut options = Options::default();

    if !overrides.no_config {
        let explicit = overrides.config.is_some();
        let config_path = overrides.config.clone().or_else(default_config_path);
        if let Some(path) = config_path {
            match fs::read_to_string(&path) {
                Ok(contents) => parse_config(&contents, &path, &mut options)?,
                Err(error) if !explicit && error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => {
                    return Err(format!("failed to read {}: {error}", path.display()));
                }
            }
        }
    }

    if let Some(prompt) = overrides.prompt {
        options.prompt = prompt;
    }
    if let Some(lines) = overrides.lines {
        options.lines = lines;
    }
    if let Some(width) = overrides.width {
        options.width = width;
    }
    if let Some(font) = overrides.font {
        options.font = Some(parse_font(&font)?);
    }
    if let Some(search) = overrides.search {
        options.search = search;
    }
    options.index = overrides.index;
    options.sort = !overrides.no_sort;

    apply_color(&mut options.background, overrides.background, "background")?;
    apply_color(&mut options.text_color, overrides.text_color, "text-color")?;
    apply_color(
        &mut options.prompt_color,
        overrides.prompt_color,
        "prompt-color",
    )?;
    apply_color(
        &mut options.prompt_background,
        overrides.prompt_background,
        "prompt-background",
    )?;
    apply_color(
        &mut options.selection_color,
        overrides.selection_color,
        "selection-color",
    )?;
    apply_color(
        &mut options.selection_text_color,
        overrides.selection_text_color,
        "selection-text-color",
    )?;
    apply_color(
        &mut options.match_color,
        overrides.match_color,
        "match-color",
    )?;

    if !(1..=8).contains(&options.lines) {
        return Err("lines must be between 1 and 8".into());
    }
    if !options.width.is_finite() || options.width < 200.0 {
        return Err("width must be at least 200 points".into());
    }

    Ok(options)
}

fn apply_color(
    target: &mut Option<Color>,
    value: Option<String>,
    name: &str,
) -> Result<(), String> {
    if let Some(value) = value {
        *target = Some(parse_color(&value).map_err(|error| format!("invalid {name}: {error}"))?);
    }
    Ok(())
}

fn default_config_path() -> Option<PathBuf> {
    if let Some(path) = env::var_os("XDG_CONFIG_HOME") {
        return Some(PathBuf::from(path).join("fzzx/fzzx.ini"));
    }
    env::var_os("HOME").map(|home| PathBuf::from(home).join(".config/fzzx/fzzx.ini"))
}

fn parse_config(contents: &str, path: &Path, options: &mut Options) -> Result<(), String> {
    let mut section = "";

    for (index, raw_line) in contents.lines().enumerate() {
        let line_number = index + 1;
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            section = &line[1..line.len() - 1];
            if section != "main" && section != "colors" {
                return config_error(path, line_number, format!("unknown section [{section}]"));
            }
            continue;
        }

        let Some((key, value)) = line.split_once('=') else {
            return config_error(path, line_number, "expected key=value");
        };
        let key = key.trim();
        let value = value.trim();

        match (section, key) {
            ("main", "font") => options.font = Some(parse_font(value)?),
            ("main", "prompt") => options.prompt = value.to_owned(),
            ("main", "lines") => {
                options.lines = value
                    .parse()
                    .map_err(|error| format!("{}:{line_number}: {error}", path.display()))?
            }
            ("main", "width") => {
                options.width = value
                    .parse()
                    .map_err(|error| format!("{}:{line_number}: {error}", path.display()))?
            }
            ("colors", "background") => {
                options.background = Some(config_color(value, path, line_number)?)
            }
            ("colors", "text") => {
                options.text_color = Some(config_color(value, path, line_number)?)
            }
            ("colors", "prompt") => {
                options.prompt_color = Some(config_color(value, path, line_number)?)
            }
            ("colors", "prompt-background") => {
                options.prompt_background = Some(config_color(value, path, line_number)?)
            }
            ("colors", "selection") => {
                options.selection_color = Some(config_color(value, path, line_number)?)
            }
            ("colors", "selection-text") => {
                options.selection_text_color = Some(config_color(value, path, line_number)?)
            }
            ("colors", "match") => {
                options.match_color = Some(config_color(value, path, line_number)?)
            }
            ("", _) => return config_error(path, line_number, "key appears before a section"),
            _ => return config_error(path, line_number, format!("unknown key {key:?}")),
        }
    }
    Ok(())
}

fn config_color(value: &str, path: &Path, line: usize) -> Result<Color, String> {
    parse_color(value).map_err(|error| format!("{}:{line}: {error}", path.display()))
}

fn config_error<T>(path: &Path, line: usize, message: impl Display) -> Result<T, String> {
    Err(format!("{}:{line}: {message}", path.display()))
}

fn parse_font(value: &str) -> Result<FontSpec, String> {
    let value = value.trim();
    let (family, size) = match value.rsplit_once(":size=") {
        Some((family, size)) => {
            let size: f64 = size
                .parse()
                .map_err(|_| format!("invalid font size in {value:?}"))?;
            (family, size)
        }
        None => (value, 16.0),
    };
    if family.trim().is_empty() {
        return Err("font family cannot be empty".into());
    }
    if !size.is_finite() || !(6.0..=96.0).contains(&size) {
        return Err("font size must be between 6 and 96 points".into());
    }
    Ok(FontSpec {
        family: family.trim().into(),
        size,
    })
}

fn parse_color(value: &str) -> Result<Color, String> {
    let value = value.trim().strip_prefix('#').unwrap_or(value.trim());
    if value.len() != 6 && value.len() != 8 {
        return Err("expected RRGGBB or RRGGBBAA".into());
    }
    let byte = |offset| {
        u8::from_str_radix(&value[offset..offset + 2], 16)
            .map_err(|_| "color contains a non-hexadecimal digit".to_owned())
    };
    let alpha = if value.len() == 8 { byte(6)? } else { 255 };
    Ok(Color {
        red: f64::from(byte(0)?) / 255.0,
        green: f64::from(byte(2)?) / 255.0,
        blue: f64::from(byte(4)?) / 255.0,
        alpha: f64::from(alpha) / 255.0,
    })
}

pub(crate) fn filter_entries(entries: &[String], query: &str, sort: bool) -> Vec<Match> {
    let pattern = Pattern::new(
        query,
        CaseMatching::Ignore,
        Normalization::Smart,
        AtomKind::Fuzzy,
    );
    let mut matcher = Matcher::new(MatcherConfig::DEFAULT);
    let mut chars = Vec::new();
    let mut matches = Vec::with_capacity(entries.len());

    for (original, entry) in entries.iter().enumerate() {
        let mut indices = Vec::new();
        if let Some(score) =
            pattern.indices(Utf32Str::new(entry, &mut chars), &mut matcher, &mut indices)
        {
            indices.sort_unstable();
            indices.dedup();
            matches.push((Match { original, indices }, score));
        }
    }

    if sort && !query.is_empty() {
        matches.sort_by(|(left, left_score), (right, right_score)| {
            right_score
                .cmp(left_score)
                .then_with(|| left.original.cmp(&right.original))
        });
    }
    matches.into_iter().map(|(entry, _)| entry).collect()
}

fn read_entries() -> Result<Vec<String>, String> {
    let stdin = io::stdin();
    if stdin.is_terminal() {
        return Ok(Vec::new());
    }
    let mut input = String::new();
    stdin
        .lock()
        .read_to_string(&mut input)
        .map_err(|error| format!("failed to read UTF-8 choices from stdin: {error}"))?;
    Ok(input.lines().map(str::to_owned).collect())
}

fn run() -> Result<ExitCode, String> {
    match parse_args(env::args_os().skip(1))? {
        Action::Help => {
            print!("{HELP}");
            Ok(ExitCode::SUCCESS)
        }
        Action::Version => {
            println!("fzzx {}", env!("CARGO_PKG_VERSION"));
            Ok(ExitCode::SUCCESS)
        }
        Action::Run(overrides) => {
            let options = options(overrides)?;
            let entries = read_entries()?;
            if entries.is_empty() && options.index {
                return Err("--index requires choices from stdin".into());
            }

            let index_output = options.index;
            let Some(output) = app::run(options, &entries)? else {
                return Ok(ExitCode::from(1));
            };

            let mut stdout = BufWriter::new(io::stdout().lock());
            match output {
                app::Output::Selection(selected) if index_output => writeln!(stdout, "{selected}"),
                app::Output::Selection(selected) => writeln!(stdout, "{}", entries[selected]),
                app::Output::Input(input) => writeln!(stdout, "{input}"),
            }
            .map_err(|error| format!("failed to write selection: {error}"))?;
            Ok(ExitCode::SUCCESS)
        }
    }
}

fn main() -> ExitCode {
    match run() {
        Ok(code) => code,
        Err(error) => {
            eprintln!("fzzx: {error}");
            ExitCode::from(2)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_fuzzel_style_font() {
        assert_eq!(
            parse_font("JetBrainsMono Nerd Font Mono:size=17.5").unwrap(),
            FontSpec {
                family: "JetBrainsMono Nerd Font Mono".into(),
                size: 17.5,
            }
        );
    }

    #[test]
    fn parses_rgb_and_rgba_colors() {
        assert_eq!(
            parse_color("#ff8000").unwrap(),
            Color {
                red: 1.0,
                green: 128.0 / 255.0,
                blue: 0.0,
                alpha: 1.0,
            }
        );
        assert_eq!(parse_color("00000080").unwrap().alpha, 128.0 / 255.0);
    }

    #[test]
    fn config_uses_fuzzel_sections() {
        let mut options = Options::default();
        parse_config(
            "[main]\nfont=Symbols Nerd Font:size=18\nlines=7\nwidth=500\n\n[colors]\nbackground=101010ee\nprompt=ffffff\nprompt-background=333333\nmatch=ff0000\n",
            Path::new("test.ini"),
            &mut options,
        )
        .unwrap();

        assert_eq!(options.lines, 7);
        assert_eq!(options.width, 500.0);
        assert_eq!(options.font.unwrap().size, 18.0);
        assert_eq!(options.background.unwrap().alpha, 238.0 / 255.0);
        assert_eq!(options.prompt_color.unwrap().red, 1.0);
        assert_eq!(options.prompt_background.unwrap().red, 0.2);
        assert_eq!(options.match_color.unwrap().red, 1.0);
    }

    #[test]
    fn fuzzy_matches_preserve_original_indices() {
        let entries = vec![
            "Firefox".into(),
            "󰀻 Safari".into(),
            "System Settings".into(),
        ];
        let matches = filter_entries(&entries, "saf", true);

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].original, 1);
        assert_eq!(matches[0].indices, vec![2, 3, 4]);
    }

    #[test]
    fn no_sort_keeps_input_order() {
        let entries = vec!["zebra".into(), "alpha beta".into(), "albatross".into()];
        let matches = filter_entries(&entries, "a", false);

        assert_eq!(
            matches
                .iter()
                .map(|entry| entry.original)
                .collect::<Vec<_>>(),
            vec![0, 1, 2]
        );
    }

    #[test]
    fn cli_search_overrides_defaults() {
        let Action::Run(overrides) = parse_args(
            ["--no-config", "--search", "saf"]
                .into_iter()
                .map(OsString::from),
        )
        .unwrap() else {
            panic!("expected run action");
        };
        let options = options(overrides).unwrap();

        assert_eq!(options.search, "saf");
        assert_eq!(options.prompt, ">");
    }

    #[test]
    fn cli_colors_override_config_colors() {
        let path = env::temp_dir().join(format!("fzzx-test-{}.ini", std::process::id()));
        fs::write(
            &path,
            "[colors]\nbackground=111111\ntext=eeeeee\nselection=222222\n",
        )
        .unwrap();
        let Action::Run(overrides) = parse_args([
            OsString::from("--config"),
            path.clone().into_os_string(),
            OsString::from("--background"),
            OsString::from("abcdef"),
        ])
        .unwrap() else {
            panic!("expected run action");
        };
        let options = options(overrides).unwrap();
        fs::remove_file(path).unwrap();

        assert_eq!(options.background, Some(parse_color("abcdef").unwrap()));
        assert_eq!(options.text_color, Some(parse_color("eeeeee").unwrap()));
        assert_eq!(
            options.selection_color,
            Some(parse_color("222222").unwrap())
        );
    }

    #[test]
    fn visible_rows_are_capped_at_eight() {
        let Action::Run(overrides) = parse_args(
            ["--no-config", "--lines", "9"]
                .into_iter()
                .map(OsString::from),
        )
        .unwrap() else {
            panic!("expected run action");
        };

        assert_eq!(
            options(overrides).unwrap_err(),
            "lines must be between 1 and 8"
        );
        assert_eq!(Options::default().lines, 8);
    }
}
