/// Origin of a todo item
#[derive(Clone, PartialEq, Eq)]
pub enum ToduSource {
    /// Local tasks without an associated remote source
    Local,
    /// Task imported from a GitHub repository; holds the repo URL (e.g. `https://github.com/org/repo`)
    #[cfg(feature = "remote")]
    GitHub(String),
    /// Task imported from a Jira instance; holds the instance base URL (e.g. `https://org.atlassian.net`)
    #[cfg(feature = "remote")]
    Jira(String),
}

impl ToduSource {
    /// Parses a source string from the database
    pub(crate) fn from_str(s: &str) -> Self {
        #[cfg(feature = "remote")]
        if let Some(url) = s.strip_prefix("github:") {
            return Self::GitHub(url.to_owned());
        }
        #[cfg(feature = "remote")]
        if let Some(url) = s.strip_prefix("jira:") {
            return Self::Jira(url.to_owned());
        }
        Self::Local
    }

    /// Returns the string label used for database storage
    pub fn label(&self) -> String {
        match self {
            Self::Local => "local".to_owned(),
            #[cfg(feature = "remote")]
            Self::GitHub(url) => format!("github:{url}"),
            #[cfg(feature = "remote")]
            Self::Jira(url) => format!("jira:{url}"),
        }
    }

    /// Returns a short human-readable name without the URL
    pub fn short_label(&self) -> &str {
        match self {
            Self::Local => "local",
            #[cfg(feature = "remote")]
            Self::GitHub(_) => "github",
            #[cfg(feature = "remote")]
            Self::Jira(_) => "jira",
        }
    }
}
