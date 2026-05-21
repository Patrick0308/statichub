mod deploys;
mod serve;
mod auth;
mod projects;
mod management;
mod domains;

pub use deploys::{create_anonymous_deploy, DeployState};
pub use serve::serve_static_file;
pub use auth::{login_google, callback_google, auth_status, AuthState, PendingSession};
pub use projects::create_project_deploy;
pub use management::{list_projects, get_project_info, rollback_project};
pub use domains::{add_domain, list_domains, verify_domain, remove_domain};
