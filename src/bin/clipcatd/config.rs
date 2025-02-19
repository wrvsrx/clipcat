use std::{
    net::IpAddr,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use snafu::{ResultExt, Snafu};

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct Config {
    pub daemonize: bool,

    #[serde(skip_serializing, default = "Config::default_pid_file_path")]
    pub pid_file: PathBuf,

    #[serde(default = "Config::default_max_history")]
    pub max_history: usize,

    #[serde(default = "Config::default_history_file_path")]
    pub history_file_path: PathBuf,

    #[serde(
        default = "Config::default_log_level",
        with = "clipcat::display_from_str"
    )]
    pub log_level: tracing::Level,

    #[serde(default)]
    pub monitor: Monitor,

    pub grpc: Grpc,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct Monitor {
    pub load_current: bool,
    pub enable_clipboard: bool,
    pub enable_primary: bool,
    #[serde(default)]
    pub filter_min_size: usize,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct Grpc {
    pub host: IpAddr,
    pub port: u16,
}

impl Default for Config {
    fn default() -> Config {
        Config {
            daemonize: true,
            pid_file: Config::default_pid_file_path(),
            max_history: Config::default_max_history(),
            history_file_path: Config::default_history_file_path(),
            log_level: Config::default_log_level(),
            monitor: Default::default(),
            grpc: Default::default(),
        }
    }
}

impl Default for Monitor {
    fn default() -> Monitor {
        Monitor {
            load_current: true,
            enable_clipboard: true,
            enable_primary: true,
            filter_min_size: 0,
        }
    }
}

impl From<Monitor> for clipcat::ClipboardMonitorOptions {
    fn from(val: Monitor) -> Self {
        let Monitor {
            load_current,
            enable_clipboard,
            enable_primary,
            filter_min_size,
        } = val;
        clipcat::ClipboardMonitorOptions {
            load_current,
            enable_clipboard,
            enable_primary,
            filter_min_size,
        }
    }
}

impl Default for Grpc {
    fn default() -> Grpc {
        Grpc {
            host: clipcat::DEFAULT_GRPC_HOST
                .parse()
                .expect("Parse default gRPC host"),
            port: clipcat::DEFAULT_GRPC_PORT,
        }
    }
}

impl Config {
    #[inline]
    pub fn default_path() -> PathBuf {
        directories::BaseDirs::new()
            .expect("app_dirs")
            .config_dir()
            .join(clipcat::PROJECT_NAME)
            .join(clipcat::DAEMON_CONFIG_NAME)
    }

    #[inline]
    fn default_log_level() -> tracing::Level {
        tracing::Level::INFO
    }

    #[inline]
    pub fn default_history_file_path() -> PathBuf {
        directories::BaseDirs::new()
            .expect("app_dirs")
            .cache_dir()
            .join(clipcat::PROJECT_NAME)
            .join(clipcat::DAEMON_HISTORY_FILE_NAME)
    }

    #[inline]
    pub fn default_max_history() -> usize {
        50
    }

    #[inline]
    pub fn default_pid_file_path() -> PathBuf {
        let mut path = std::env::var("XDG_RUNTIME_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| std::env::temp_dir());
        path.push(format!("{}.pid", clipcat::DAEMON_PROGRAM_NAME));
        path
    }

    #[inline]
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Config, ConfigError> {
        let data = std::fs::read(&path).context(OpenConfigSnafu {
            filename: path.as_ref().to_path_buf(),
        })?;
        let mut config = toml::from_slice::<Config>(&data).context(ParseConfigSnafu {
            filename: path.as_ref().to_path_buf(),
        })?;

        if config.max_history == 0 {
            config.max_history = Self::default_max_history();
        }

        Ok(config)
    }
}

#[derive(Debug, Snafu)]
pub enum ConfigError {
    #[snafu(display("Could not open config from {}: {}", filename.display(), source))]
    OpenConfig {
        filename: PathBuf,
        source: std::io::Error,
    },

    #[snafu(display("Count not parse config from {}: {}", filename.display(), source))]
    ParseConfig {
        filename: PathBuf,
        source: toml::de::Error,
    },
}
