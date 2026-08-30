//! Linkage anchor for the reference Lenso Support App Host.
//!
//! This crate does not execute a Host. It makes the six real P0/P1 Plugin
//! factories and descriptors, plus their real Content Vault provider, reachable
//! to composition acceptance tests. A production Host must own the actual Host
//! Catalog, lifecycle, and routing.

use content_vault as _;
use lenso_customer_directory_postgres_plugin as _;
use lenso_knowledge_base_postgres_plugin as _;
use lenso_support_attachment_postgres_plugin as _;
use lenso_support_case_postgres_plugin as _;

/// The six real Plugin Descriptors linked into the reference Host acceptance binary.
pub fn suite_descriptor_json() -> [&'static str; 6] {
    lenso_support_email_resend::link();
    lenso_help_center_web::link();

    [
        lenso_customer_directory_postgres_plugin::PLUGIN_DESCRIPTOR_JSON,
        lenso_support_email_resend::PLUGIN_DESCRIPTOR_JSON,
        lenso_support_case_postgres_plugin::PLUGIN_DESCRIPTOR_JSON,
        lenso_support_attachment_postgres_plugin::PLUGIN_DESCRIPTOR_JSON,
        lenso_knowledge_base_postgres_plugin::PLUGIN_DESCRIPTOR_JSON,
        lenso_help_center_web::PLUGIN_DESCRIPTOR_JSON,
    ]
}

/// Forces all seven native Plugin crates to remain linked in a Host executable.
pub fn link() {
    let _ = content_vault::PLUGIN_ID;
    let _ = suite_descriptor_json();
}
