//! Translations: Fluent bundles per language with the UI's strings and every
//! game's. A message missing in the chosen language falls back to English,
//! then to its id.

use archi_storage::Timestamp;
use fluent_bundle::{FluentArgs, FluentBundle, FluentResource, FluentValue};
use unic_langid::LanguageIdentifier;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
  Es,
  En,
}

impl Language {
  pub fn system() -> Self {
    match sys_locale::get_locale() {
      Some(locale) if locale.to_ascii_lowercase().starts_with("es") => Self::Es,
      _ => Self::En,
    }
  }

  fn code(self) -> &'static str {
    match self {
      Self::Es => "es",
      Self::En => "en",
    }
  }

  fn sources(self) -> [&'static str; 3] {
    match self {
      Self::Es => [
        include_str!("../locales/es.ftl"),
        include_str!("../../../games/fortnite/locales/es.ftl"),
        include_str!("../../../games/platform/locales/es.ftl"),
      ],
      Self::En => [
        include_str!("../locales/en.ftl"),
        include_str!("../../../games/fortnite/locales/en.ftl"),
        include_str!("../../../games/platform/locales/en.ftl"),
      ],
    }
  }
}

/// What the user picked in Settings; stored as a setting.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum LanguageChoice {
  #[default]
  System,
  Fixed(Language),
}

impl LanguageChoice {
  pub const SETTING: &str = "ui.language";

  pub fn from_code(code: &str) -> Self {
    match code {
      "es" => Self::Fixed(Language::Es),
      "en" => Self::Fixed(Language::En),
      _ => Self::System,
    }
  }

  pub fn code(self) -> &'static str {
    match self {
      Self::System => "system",
      Self::Fixed(language) => language.code(),
    }
  }

  pub fn resolve(self) -> Language {
    match self {
      Self::System => Language::system(),
      Self::Fixed(language) => language,
    }
  }
}

pub struct I18n {
  language: Language,
  current: FluentBundle<FluentResource>,
  english: FluentBundle<FluentResource>,
}

impl I18n {
  pub fn new(language: Language) -> Self {
    Self { language, current: bundle(language), english: bundle(Language::En) }
  }

  pub fn set_language(&mut self, language: Language) {
    if language != self.language {
      *self = Self::new(language);
    }
  }

  pub fn tr(&self, id: &str) -> String {
    self.format(id, None)
  }

  /// `args` are `(name, value)` pairs for the message's `{ $name }`s.
  pub fn tr_args<'a>(&self, id: &str, args: &[(&'a str, FluentValue<'a>)]) -> String {
    let mut fluent_args = FluentArgs::new();
    for (name, value) in args {
      fluent_args.set(*name, value.clone());
    }
    self.format(id, Some(&fluent_args))
  }

  /// "just now", "5 min ago", … up to a week, then the (UTC) date.
  pub fn when(&self, at: Timestamp, now: Timestamp) -> String {
    let elapsed = (now - at).max(0);
    match elapsed {
      0..60 => self.tr("ui-time-just-now"),
      60..3600 => self.tr_args("ui-time-minutes", &[("n", (elapsed / 60).into())]),
      3600..86_400 => self.tr_args("ui-time-hours", &[("n", (elapsed / 3600).into())]),
      86_400..604_800 => self.tr_args("ui-time-days", &[("n", (elapsed / 86_400).into())]),
      _ => self.tr_args("ui-time-date", &[("date", utc_date(at).into())]),
    }
  }

  fn format(&self, id: &str, args: Option<&FluentArgs>) -> String {
    for bundle in [&self.current, &self.english] {
      if let Some(pattern) = bundle.get_message(id).and_then(|message| message.value()) {
        let mut errors = Vec::new();
        return bundle.format_pattern(pattern, args, &mut errors).into_owned();
      }
    }
    id.to_owned()
  }
}

fn bundle(language: Language) -> FluentBundle<FluentResource> {
  let locale: LanguageIdentifier = language.code().parse().expect("valid language code");
  let mut bundle = FluentBundle::new(vec![locale]);
  // Unicode isolation marks around arguments render as boxes in egui.
  bundle.set_use_isolating(false);
  for source in language.sources() {
    // The files are bundled and tested, so a parse error is a bug.
    let resource = FluentResource::try_new(source.to_owned()).expect("bundled Fluent file parses");
    bundle.add_resource(resource).expect("no duplicate Fluent ids");
  }
  bundle
}

/// `YYYY-MM-DD` of a Unix timestamp, in UTC (days-to-civil algorithm).
fn utc_date(at: Timestamp) -> String {
  let z = at.div_euclid(86_400) + 719_468;
  let era = z.div_euclid(146_097);
  let doe = z - era * 146_097;
  let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
  let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
  let mp = (5 * doy + 2) / 153;
  let day = doy - (153 * mp + 2) / 5 + 1;
  let month = if mp < 10 { mp + 3 } else { mp - 9 };
  let year = yoe + era * 400 + i64::from(month <= 2);
  format!("{year:04}-{month:02}-{day:02}")
}

#[cfg(test)]
mod tests {
  use std::collections::BTreeSet;
  use std::fs;
  use std::path::Path;

  use super::*;

  fn ids(source: &str) -> BTreeSet<String> {
    source
      .lines()
      .filter(|line| line.starts_with(|c: char| c.is_ascii_alphabetic()))
      .filter_map(|line| line.split_once('=').map(|(id, _)| id.trim().to_owned()))
      .collect()
  }

  /// Every `"ui-…"` literal in the app's source.
  fn used_ids() -> BTreeSet<String> {
    let mut used = BTreeSet::new();
    let mut dirs = vec![Path::new(env!("CARGO_MANIFEST_DIR")).join("src")];
    while let Some(dir) = dirs.pop() {
      for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
          dirs.push(path);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
          let source = fs::read_to_string(&path).unwrap();
          for (start, _) in source.match_indices("\"ui-") {
            let id: String = source[start + 1..].chars().take_while(|&c| c.is_ascii_alphanumeric() || c == '-' || c == '_').collect();
            if !id.ends_with('-') {
              used.insert(id);
            }
          }
        }
      }
    }
    used
  }

  #[test]
  fn ui_strings_exist_in_both_languages() {
    let es = ids(include_str!("../locales/es.ftl"));
    let en = ids(include_str!("../locales/en.ftl"));
    assert_eq!(es, en, "es.ftl and en.ftl must define the same ids");
    let missing: Vec<_> = used_ids().into_iter().filter(|id| !es.contains(id)).collect();
    assert!(missing.is_empty(), "used but not translated: {missing:?}");
  }

  #[test]
  fn formats_with_arguments_plurals_and_fallback() {
    let es = I18n::new(Language::Es);
    assert_eq!(es.tr_args("ui-level", &[("level", 7.into())]), "Nivel 7");
    assert_eq!(es.tr("fortnite-first_win-name"), "Primera corona");
    assert_eq!(es.tr_args("ui-time-days", &[("n", 1.into())]), "hace 1 día");
    assert_eq!(es.tr_args("ui-time-days", &[("n", 3.into())]), "hace 3 días");
    assert_eq!(es.tr("no-such-id"), "no-such-id");
    assert_eq!(I18n::new(Language::En).tr("ui-nav-profile"), "Profile");
  }

  #[test]
  fn relative_times_then_dates() {
    let en = I18n::new(Language::En);
    let now = 1_793_652_036;
    assert_eq!(en.when(now - 10, now), "just now");
    assert_eq!(en.when(now - 300, now), "5 min ago");
    assert_eq!(en.when(now - 7200, now), "2 h ago");
    assert_eq!(en.when(now - 30 * 86_400, now), "on 2026-10-03");
  }

  #[test]
  fn utc_dates() {
    assert_eq!(utc_date(0), "1970-01-01");
    assert_eq!(utc_date(951_782_400), "2000-02-29");
    assert_eq!(utc_date(1_793_652_036), "2026-11-02");
  }
}
