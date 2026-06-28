/// Origin of a todo item
#[derive(Clone, Copy)]
pub enum ToduSource {
    /// Local tasks without an associated remote source
    Local,
    /// Remote origin tasks (jira, github)
    Remote,
}

impl ToduSource {
    /// Parses a source string from the database; always returns `Local` until remote sources are added.
    pub(crate) fn from_str(_s: &str) -> Self {
        Self::Local
    }

    /// Returns the string label used for database storage.
    pub fn label(&self) -> &str {
        match self {
            Self::Local => "local",
            Self::Remote => "remote",
        }
    }
}
