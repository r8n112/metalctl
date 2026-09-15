//! Robot API credentials.

use std::env;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;
use serde::Deserialize;

use crate::error::{Error, Result};

/// Environment variable holding the Robot webservice username.
pub const USER_ENV: &str = "HETZNER_ROBOT_USER";
/// Environment variable holding the Robot webservice password.
pub const PASSWORD_ENV: &str = "HETZNER_ROBOT_PASSWORD";
/// Environment variable overriding the config file path.
pub const CONFIG_ENV: &str = "METALCTL_CONFIG";

/// HTTP Basic credentials for the Hetzner Robot API.
///
/// The password is never printed: [`Debug`](fmt::Debug) redacts it.
#[derive(Clone)]
pub struct Credentials {
    username: String,
    password: String,
}

impl fmt::Debug for Credentials {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Credentials")
            .field("username", &self.username)
            .field("password", &"<redacted>")
            .finish()
    }
}

impl Credentials {
    /// Creates credentials from a username and password.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidCredentials`] if either value is empty.
    pub fn new(username: impl Into<String>, password: impl Into<String>) -> Result<Self> {
        let username = username.into();
        let password = password.into();
        if username.is_empty() || password.is_empty() {
            return Err(Error::InvalidCredentials);
        }
        Ok(Self { username, password })
    }

    /// Reads credentials from the `HETZNER_ROBOT_USER` and
    /// `HETZNER_ROBOT_PASSWORD` environment variables.
    ///
    /// # Errors
    ///
    /// Returns [`Error::MissingCredentials`] if either variable is unset or
    /// empty, or [`Error::InvalidCredentials`] if a value is invalid.
    pub fn from_env() -> Result<Self> {
        let username = env::var(USER_ENV).unwrap_or_default();
        let password = env::var(PASSWORD_ENV).unwrap_or_default();
        if username.is_empty() || password.is_empty() {
            return Err(Error::MissingCredentials);
        }
        Self::new(username, password)
    }

    /// Resolves credentials from the first source that provides each value:
    /// CLI flags, then the environment, then a config file.
    ///
    /// # Errors
    ///
    /// Returns [`Error::MissingCredentials`] if either value is absent from
    /// every source, or [`Error::InvalidCredentials`] if a value is empty.
    pub fn resolve(sources: CredentialSources) -> Result<Self> {
        let config = sources.config.unwrap_or_default();
        let username = sources.flag_user.or(sources.env_user).or(config.user);
        let password = sources
            .flag_password
            .or(sources.env_password)
            .or(config.password);
        match (username, password) {
            (Some(username), Some(password)) => Self::new(username, password),
            _ => Err(Error::MissingCredentials),
        }
    }

    /// Returns the value for the HTTP `Authorization` header.
    #[must_use]
    pub fn authorization(&self) -> String {
        let raw = format!("{}:{}", self.username, self.password);
        format!("Basic {}", STANDARD.encode(raw))
    }
}

/// Credentials loaded from a config file.
///
/// The password is redacted in [`Debug`](fmt::Debug).
#[derive(Clone, Default)]
pub struct ConfigCredentials {
    /// Username, if present in the file.
    pub user: Option<String>,
    /// Password, if present in the file.
    pub password: Option<String>,
}

impl fmt::Debug for ConfigCredentials {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ConfigCredentials")
            .field("user", &self.user)
            .field("password", &redact(self.password.as_ref()))
            .finish()
    }
}

/// Credential values from every source, in precedence order.
///
/// [`Credentials::resolve`] takes the first non-empty value from `flag_*`, then
/// `env_*`, then `config`. Password fields are redacted in
/// [`Debug`](fmt::Debug).
#[derive(Clone, Default)]
pub struct CredentialSources {
    /// Username from a CLI flag.
    pub flag_user: Option<String>,
    /// Password from a CLI flag (for example read from `--password-file`).
    pub flag_password: Option<String>,
    /// Username from the environment.
    pub env_user: Option<String>,
    /// Password from the environment.
    pub env_password: Option<String>,
    /// Credentials loaded from a config file.
    pub config: Option<ConfigCredentials>,
}

impl fmt::Debug for CredentialSources {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CredentialSources")
            .field("flag_user", &self.flag_user)
            .field("flag_password", &redact(self.flag_password.as_ref()))
            .field("env_user", &self.env_user)
            .field("env_password", &redact(self.env_password.as_ref()))
            .field("config", &self.config)
            .finish()
    }
}

fn redact(value: Option<&String>) -> Option<&'static str> {
    value.map(|_| "<redacted>")
}

/// Returns the config file path: `$METALCTL_CONFIG`, else
/// `$XDG_CONFIG_HOME/metalctl/config.toml`, else
/// `~/.config/metalctl/config.toml`, or `None` if no home is known.
#[must_use]
pub fn default_config_path() -> Option<PathBuf> {
    if let Some(path) = env::var_os(CONFIG_ENV) {
        return Some(PathBuf::from(path));
    }
    let base = env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))?;
    Some(base.join("metalctl").join("config.toml"))
}

/// Loads credentials from `path`, or `Ok(None)` if the file is absent.
///
/// The format is a minimal `key = value` file supporting `user` and `password`;
/// `#` starts a comment and surrounding quotes are optional. On Unix the file
/// must not be accessible by group or others (for example `chmod 600`), or
/// [`Error::Config`] is returned.
///
/// # Errors
///
/// Returns [`Error::Config`] on I/O errors, insecure permissions, or parse
/// errors.
pub fn load_config(path: &Path) -> Result<Option<ConfigCredentials>> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(Error::Config(format!(
                "failed to read {}: {error}",
                path.display()
            )));
        }
    };
    check_config_permissions(path)?;
    Ok(Some(parse_config(&text)?))
}

#[cfg(unix)]
fn check_config_permissions(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mode = fs::metadata(path)
        .map_err(|error| Error::Config(format!("failed to stat {}: {error}", path.display())))?
        .permissions()
        .mode();
    if mode & 0o077 != 0 {
        return Err(Error::Config(format!(
            "{} is accessible by other users (mode {:03o}); run `chmod 600`",
            path.display(),
            mode & 0o777
        )));
    }
    Ok(())
}

#[cfg(not(unix))]
fn check_config_permissions(_path: &Path) -> Result<()> {
    Ok(())
}

/// Config file schema.
///
/// Only `user` and `password` are accepted; `deny_unknown_fields` rejects
/// typos and unknown keys instead of silently ignoring them.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ConfigFile {
    user: Option<String>,
    password: Option<String>,
}

fn parse_config(text: &str) -> Result<ConfigCredentials> {
    let file: ConfigFile =
        toml::from_str(text).map_err(|error| Error::Config(format!("invalid config: {error}")))?;
    Ok(ConfigCredentials {
        user: file.user,
        password: file.password,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_empty_username() {
        assert!(matches!(
            Credentials::new("", "secret"),
            Err(Error::InvalidCredentials)
        ));
    }

    #[test]
    fn rejects_empty_password() {
        assert!(matches!(
            Credentials::new("user", ""),
            Err(Error::InvalidCredentials)
        ));
    }

    #[test]
    fn builds_basic_authorization_header() {
        // base64("user:pass") == "dXNlcjpwYXNz"
        let credentials = Credentials::new("user", "pass").unwrap();
        assert_eq!(credentials.authorization(), "Basic dXNlcjpwYXNz");
    }

    #[test]
    fn debug_redacts_password() {
        let credentials = Credentials::new("user", "s3cr3t").unwrap();
        let rendered = format!("{credentials:?}");
        assert!(rendered.contains("<redacted>"));
        assert!(!rendered.contains("s3cr3t"));
    }

    fn sources(
        flag: Option<(&str, &str)>,
        env: Option<(&str, &str)>,
        config: Option<(&str, &str)>,
    ) -> CredentialSources {
        CredentialSources {
            flag_user: flag.map(|(user, _)| user.to_string()),
            flag_password: flag.map(|(_, password)| password.to_string()),
            env_user: env.map(|(user, _)| user.to_string()),
            env_password: env.map(|(_, password)| password.to_string()),
            config: config.map(|(user, password)| ConfigCredentials {
                user: Some(user.to_string()),
                password: Some(password.to_string()),
            }),
        }
    }

    #[test]
    fn resolve_prefers_flags_over_env_and_config() {
        let resolved = Credentials::resolve(sources(
            Some(("flag-user", "flag-pass")),
            Some(("env-user", "env-pass")),
            Some(("cfg-user", "cfg-pass")),
        ))
        .unwrap();
        assert_eq!(
            resolved.authorization(),
            Credentials::new("flag-user", "flag-pass")
                .unwrap()
                .authorization()
        );
    }

    #[test]
    fn resolve_falls_back_to_env() {
        let resolved =
            Credentials::resolve(sources(None, Some(("env-user", "env-pass")), None)).unwrap();
        assert_eq!(
            resolved.authorization(),
            Credentials::new("env-user", "env-pass")
                .unwrap()
                .authorization()
        );
    }

    #[test]
    fn resolve_uses_config_last_and_mixes_sources_per_value() {
        // user from a flag, password from the config file.
        let mixed = Credentials::resolve(CredentialSources {
            flag_user: Some("flag-user".to_string()),
            config: Some(ConfigCredentials {
                user: Some("cfg-user".to_string()),
                password: Some("cfg-pass".to_string()),
            }),
            ..CredentialSources::default()
        })
        .unwrap();
        assert_eq!(
            mixed.authorization(),
            Credentials::new("flag-user", "cfg-pass")
                .unwrap()
                .authorization()
        );
    }

    #[test]
    fn resolve_errors_when_absent() {
        assert!(matches!(
            Credentials::resolve(sources(None, None, None)),
            Err(Error::MissingCredentials)
        ));
    }

    #[test]
    fn debug_redacts_passwords_in_sources() {
        let rendered = format!(
            "{:?}",
            sources(
                Some(("flag-user", "FLAG-SECRET")),
                Some(("env-user", "ENV-SECRET")),
                Some(("cfg-user", "CFG-SECRET")),
            )
        );
        assert!(rendered.contains("<redacted>"));
        assert!(!rendered.contains("FLAG-SECRET"));
        assert!(!rendered.contains("ENV-SECRET"));
        assert!(!rendered.contains("CFG-SECRET"));
    }

    #[test]
    fn parses_user_and_password() {
        let config = parse_config("# comment\nuser = \"alice\"\npassword = \"secret\"\n").unwrap();
        assert_eq!(config.user.as_deref(), Some("alice"));
        assert_eq!(config.password.as_deref(), Some("secret"));
    }

    #[test]
    fn rejects_unknown_config_keys() {
        assert!(matches!(
            parse_config("token = \"x\"\n"),
            Err(Error::Config(_))
        ));
    }

    #[test]
    fn rejects_malformed_toml() {
        // Missing value.
        assert!(matches!(parse_config("user =\n"), Err(Error::Config(_))));
        // Not TOML at all.
        assert!(matches!(
            parse_config("this is not toml\n"),
            Err(Error::Config(_))
        ));
    }

    fn temp_config_path() -> PathBuf {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = env::temp_dir().join(format!("metalctl-test-{}-{unique}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        dir.join("config.toml")
    }

    #[test]
    fn load_config_returns_none_when_absent() {
        assert!(load_config(&temp_config_path()).unwrap().is_none());
    }

    #[cfg(unix)]
    #[test]
    fn load_config_enforces_owner_only_permissions() {
        use std::io::Write as _;
        use std::os::unix::fs::PermissionsExt as _;

        let path = temp_config_path();
        let mut file = fs::File::create(&path).unwrap();
        writeln!(file, "user = \"u\"").unwrap();
        writeln!(file, "password = \"p\"").unwrap();
        drop(file);

        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
        assert!(matches!(load_config(&path), Err(Error::Config(_))));

        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        let loaded = load_config(&path).unwrap().unwrap();
        assert_eq!(loaded.user.as_deref(), Some("u"));
        assert_eq!(loaded.password.as_deref(), Some("p"));

        let _ = fs::remove_file(&path);
    }
}
