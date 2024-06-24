use std::fmt::{self, Display, Formatter};

use crate::config::profile::{item_type::ProfileUid, profiles::IProfiles};

use super::ChainItem;

pub fn convert_uids_to_scripts(profiles: &IProfiles, uids: &[ProfileUid]) -> Vec<ChainItem> {
    uids.iter()
        .filter_map(|uid| profiles.get_item(uid).ok())
        .filter_map(<Option<ChainItem>>::from)
        .collect::<Vec<ChainItem>>()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EnhanceLogLevel {
    Info,  // print fn in script
    Warn,  // Something not recommended, it should be filtered by internal check after
    Error, // It should be interrupted by runner or internal check
}

impl Display for EnhanceLogLevel {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Info => write!(f, "INFO"),
            Self::Warn => write!(f, "WARN"),
            Self::Error => write!(f, "ERROR"),
        }
    }
}

impl From<EnhanceLogLevel> for &str {
    fn from(level: EnhanceLogLevel) -> Self {
        match level {
            EnhanceLogLevel::Info => "INFO",
            EnhanceLogLevel::Warn => "WARN",
            EnhanceLogLevel::Error => "ERROR",
        }
    }
}

impl From<EnhanceLogLevel> for String {
    fn from(level: EnhanceLogLevel) -> Self {
        match level {
            EnhanceLogLevel::Info => "INFO".to_string(),
            EnhanceLogLevel::Warn => "WARN".to_string(),
            EnhanceLogLevel::Error => "ERROR".to_string(),
        }
    }
}

impl From<&EnhanceLogLevel> for String {
    fn from(level: &EnhanceLogLevel) -> Self {
        match level {
            EnhanceLogLevel::Info => "INFO".to_string(),
            EnhanceLogLevel::Warn => "WARN".to_string(),
            EnhanceLogLevel::Error => "ERROR".to_string(),
        }
    }
}

impl From<&EnhanceLogLevel> for &str {
    fn from(level: &EnhanceLogLevel) -> Self {
        match level {
            EnhanceLogLevel::Info => "INFO",
            EnhanceLogLevel::Warn => "WARN",
            EnhanceLogLevel::Error => "ERROR",
        }
    }
}

pub type LogSpan = (EnhanceLogLevel, String);
pub trait LogSpanExt {
    #[allow(dead_code)]
    fn record(&self);
}

impl LogSpanExt for LogSpan {
    fn record(&self) {
        match self.0 {
            EnhanceLogLevel::Info => {
                tracing::info!("{}", self.1);
            }
            EnhanceLogLevel::Warn => {
                tracing::warn!("{}", self.1);
            }
            EnhanceLogLevel::Error => {
                tracing::error!("{}", self.1);
            }
        }
    }
}

pub type ResultLog = Vec<(EnhanceLogLevel, String)>;
