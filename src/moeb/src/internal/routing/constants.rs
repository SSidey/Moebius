/// GitHub App credentials compiled into the binary.
/// Populate these constants before building a release binary.
pub const MOEB_GITHUB_APP_ID: u64 = 4039184;
pub const MOEB_GITHUB_APP_INSTALLATION_ID: u64 = 139904405; // TODO: set installation ID
/// RSA private key PEM. Empty string disables routing (development default).
pub const MOEB_GITHUB_APP_PRIVATE_KEY_PEM: &str = "";
/// Default moeb repository slug. Override via config signal_routing.repo_slug.
pub const MOEB_GITHUB_REPO_SLUG: &str = "SSidey/Moebius";
