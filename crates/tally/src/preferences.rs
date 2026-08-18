//! App-wide preferences, stored separately from habit data so appearance
//! and calendar choices can evolve without changing the habit schema.

use chrono::Weekday;
use serde::{Deserialize, Serialize};

use crate::persist;

const KEY: &str = "settings/v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum WeekStart {
    #[default]
    Monday,
    Tuesday,
    Wednesday,
    Thursday,
    Friday,
    Saturday,
    Sunday,
}

impl WeekStart {
    pub const ALL: [Self; 7] = [
        Self::Monday,
        Self::Tuesday,
        Self::Wednesday,
        Self::Thursday,
        Self::Friday,
        Self::Saturday,
        Self::Sunday,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Monday => "Monday",
            Self::Tuesday => "Tuesday",
            Self::Wednesday => "Wednesday",
            Self::Thursday => "Thursday",
            Self::Friday => "Friday",
            Self::Saturday => "Saturday",
            Self::Sunday => "Sunday",
        }
    }

    pub const fn value(self) -> &'static str {
        match self {
            Self::Monday => "monday",
            Self::Tuesday => "tuesday",
            Self::Wednesday => "wednesday",
            Self::Thursday => "thursday",
            Self::Friday => "friday",
            Self::Saturday => "saturday",
            Self::Sunday => "sunday",
        }
    }

    pub fn from_value(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|day| day.value() == value)
    }

    pub const fn weekday(self) -> Weekday {
        match self {
            Self::Monday => Weekday::Mon,
            Self::Tuesday => Weekday::Tue,
            Self::Wednesday => Weekday::Wed,
            Self::Thursday => Weekday::Thu,
            Self::Friday => Weekday::Fri,
            Self::Saturday => Weekday::Sat,
            Self::Sunday => Weekday::Sun,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Preferences {
    pub dark_mode: bool,
    pub week_start: WeekStart,
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
        assert!(dark_only.dark_mode);
        assert_eq!(dark_only.week_start, WeekStart::Monday);
    }

    #[test]
    fn every_weekday_has_a_stable_form_value() {
        for day in WeekStart::ALL {
            assert_eq!(WeekStart::from_value(day.value()), Some(day));
        }
        assert_eq!(WeekStart::from_value("noday"), None);
    }
}
