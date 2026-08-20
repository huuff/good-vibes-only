//! App-wide preferences, stored separately from habit data so appearance
//! and calendar choices can evolve without changing the habit schema.

use chrono::Weekday;
use serde::{Deserialize, Deserializer, Serialize};

use crate::persist;

const KEY: &str = "settings/v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
pub enum WeekStart {
    #[default]
    Monday,
    Sunday,
}

impl WeekStart {
    pub const ALL: [Self; 2] = [Self::Monday, Self::Sunday];

    pub const fn value(self) -> &'static str {
        match self {
            Self::Monday => "monday",
            Self::Sunday => "sunday",
        }
    }

    pub fn from_value(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|day| day.value() == value)
    }

    pub const fn weekday(self) -> Weekday {
        match self {
            Self::Monday => Weekday::Mon,
            Self::Sunday => Weekday::Sun,
        }
    }
}

impl<'de> Deserialize<'de> for WeekStart {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let stored = String::deserialize(deserializer)?;
        Ok(if stored == "Sunday" {
            Self::Sunday
        } else {
            // Older builds offered every weekday. Preserve the conventional
            // default when reading one of those retired values.
            Self::Monday
        })
    }
}

/// UI language. Options in the settings selector show their native name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Language {
    #[default]
    English,
    Spanish,
    French,
    German,
    Italian,
}

impl Language {
    pub const ALL: [Self; 5] = [
        Self::English,
        Self::Spanish,
        Self::French,
        Self::German,
        Self::Italian,
    ];

    /// The language's own name, shown untranslated in the selector.
    pub const fn native_name(self) -> &'static str {
        match self {
            Self::English => "English",
            Self::Spanish => "Español",
            Self::French => "Français",
            Self::German => "Deutsch",
            Self::Italian => "Italiano",
        }
    }

    pub const fn value(self) -> &'static str {
        match self {
            Self::English => "en",
            Self::Spanish => "es",
            Self::French => "fr",
            Self::German => "de",
            Self::Italian => "it",
        }
    }

    pub fn from_value(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|lang| lang.value() == value)
    }

    /// Locale for chrono's localized date formatting.
    pub const fn locale(self) -> chrono::Locale {
        match self {
            Self::English => chrono::Locale::en_GB,
            Self::Spanish => chrono::Locale::es_ES,
            Self::French => chrono::Locale::fr_FR,
            Self::German => chrono::Locale::de_DE,
            Self::Italian => chrono::Locale::it_IT,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Preferences {
    /// `None` follows the operating-system preference. Existing stored
    /// booleans deserialize as explicit overrides.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dark_mode: Option<bool>,
    pub week_start: WeekStart,
    pub language: Language,
}

impl Preferences {
    pub fn load() -> Self {
        persist::get(KEY).unwrap_or_default()
    }

    pub fn save(&self) {
        persist::set(KEY, self);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn old_or_partial_settings_keep_safe_defaults() {
        let empty: Preferences = serde_json::from_str("{}").unwrap();
        assert_eq!(empty, Preferences::default());

        let dark_only: Preferences = serde_json::from_str(r#"{"dark_mode":true}"#).unwrap();
        assert_eq!(dark_only.dark_mode, Some(true));
        assert_eq!(dark_only.week_start, WeekStart::Monday);
    }

    #[test]
    fn settings_without_a_theme_do_not_force_light_mode() {
        let settings: Preferences = serde_json::from_str("{}").unwrap();
        let stored = serde_json::to_value(settings).unwrap();

        assert_eq!(stored.get("dark_mode"), None);
    }

    #[test]
    fn only_monday_and_sunday_are_week_start_choices() {
        assert_eq!(
            WeekStart::ALL.as_slice(),
            &[WeekStart::Monday, WeekStart::Sunday]
        );
        for day in WeekStart::ALL {
            assert_eq!(WeekStart::from_value(day.value()), Some(day));
        }
        assert_eq!(WeekStart::from_value("thursday"), None);
    }

    #[test]
    fn legacy_midweek_start_is_migrated_to_monday() {
        let settings: Preferences = serde_json::from_str(r#"{"week_start":"Thursday"}"#).unwrap();

        assert_eq!(settings.week_start, WeekStart::Monday);
    }
}
