/// A remote source configured for a project
pub struct ToduRemote {
    /// Remote type: `"github"` or `"jira"`
    pub remote_type: String,
    /// URL identifying the remote (repo URL for GitHub, base URL for Jira)
    pub url: String,
}
