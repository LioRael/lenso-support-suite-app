use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use lenso_app_plan::{
    CapabilityCardinality, CapabilityEndpointPlan, CapabilityRequirementPlan,
    authoring::{
        HostCatalog, HostDefaultPlugin, HostPluginRelease, HostSlot, PluginDescriptor,
        PluginInstanceId, PluginRootInstance, PluginRootResolutionError, PluginRootSnapshot,
        resolve_plugin_root,
    },
};
use lenso_native_adapter::NativePluginRegistry;
use serde_json::Value;

const SUITE_PLUGIN_IDS: [&str; 7] = [
    "lenso.customer-directory.postgres",
    "lenso.support-email.resend",
    "lenso.support-case.postgres",
    "lenso.support-attachment.postgres",
    "lenso.knowledge-base.postgres",
    "lenso.help-center.web",
    "lenso.support.web",
];
const HOST_FIXTURE_PLUGIN_ID: &str = "fixture.support-platform";
const CONTENT_VAULT_PLUGIN_ID: &str = "lenso.content-vault";
const SUPPORT_WEB_PLUGIN_ID: &str = "lenso.support.web";
const WEB_INGRESS_PLUGIN_ID: &str = "lenso.web-ingress";
const INSTANCE: &str = "default";
const AUTH_CREDENTIAL_AUDIENCES: [&str; 12] = [
    "lenso.http.endpoint@1:help.center.web.support.create",
    "lenso.http.endpoint@1:help.center.web.support.status",
    "lenso.http.endpoint@1:help.center.web.support.attachment.upload",
    "lenso.support-attachment@1:upload_and_attach",
    "lenso.support-case@1:add_message",
    "lenso.support-case@1:assign_case",
    "lenso.support-case@1:create_case",
    "lenso.support-case@1:get_case",
    "lenso.support-case@1:list_cases",
    "lenso.support-case@1:list_messages",
    "lenso.support-case@1:transition_case",
    "lenso.support-case@1:update_case",
];

fn plugin_root_ids() -> impl Iterator<Item = &'static str> {
    linked_plugin_ids().chain([WEB_INGRESS_PLUGIN_ID])
}

fn linked_plugin_ids() -> impl Iterator<Item = &'static str> {
    SUITE_PLUGIN_IDS
        .into_iter()
        .chain([CONTENT_VAULT_PLUGIN_ID])
}

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn host_slots() -> Vec<HostSlot> {
    vec![
        HostSlot::optional("customer-directory"),
        HostSlot::optional("support-case"),
        HostSlot::optional("support-attachment"),
        HostSlot::optional("content-vault"),
        HostSlot::optional("knowledge-base"),
        HostSlot::many("web"),
        HostSlot::one("http-ingress"),
        HostSlot::many("host-fixtures"),
    ]
}

fn descriptor_constants() -> BTreeMap<String, PluginDescriptor> {
    lenso_support_suite_app::suite_descriptor_json()
        .into_iter()
        .map(|json| {
            let descriptor = serde_json::from_str::<PluginDescriptor>(json)
                .expect("linked Plugin Descriptor must be valid");
            (descriptor.plugin_id().to_owned(), descriptor)
        })
        .collect()
}

fn linked_suite_releases() -> Vec<HostPluginRelease> {
    lenso_support_suite_app::link();
    let expected = descriptor_constants();
    assert_eq!(expected.len(), SUITE_PLUGIN_IDS.len());

    let linked = NativePluginRegistry::host_catalog(host_slots(), [])
        .expect("linked native descriptors must form a Host Catalog");
    let mut releases = linked
        .plugins()
        .iter()
        .filter(|release| {
            linked_plugin_ids().any(|plugin_id| plugin_id == release.descriptor().plugin_id())
        })
        .cloned()
        .collect::<Vec<_>>();
    releases.sort_by_key(|release| release.descriptor().plugin_id().to_owned());

    assert_eq!(releases.len(), linked_plugin_ids().count());
    assert_eq!(
        releases.len(),
        8,
        "the suite must link eight real providers"
    );
    for plugin_id in linked_plugin_ids() {
        let matches = releases
            .iter()
            .filter(|release| release.descriptor().plugin_id() == plugin_id)
            .collect::<Vec<_>>();
        assert_eq!(matches.len(), 1, "{plugin_id} must be linked exactly once");
        if let Some(expected_descriptor) = expected.get(plugin_id) {
            assert_eq!(matches[0].descriptor(), expected_descriptor);
        }
    }

    let content_vault = releases
        .iter()
        .find(|release| release.descriptor().plugin_id() == CONTENT_VAULT_PLUGIN_ID)
        .expect("Content Vault descriptor must be linked")
        .descriptor();
    assert_eq!(content_vault.root_slot(), "content-vault");
    assert!(
        content_vault
            .provided_capabilities()
            .iter()
            .any(|endpoint| {
                endpoint.capability_id() == "lenso.content-vault@1"
                    && endpoint.descriptor_version() == "1.0.0"
            })
    );
    assert_eq!(
        content_vault
            .required_capabilities()
            .iter()
            .map(lenso_app_plan::CapabilityRequirementPlan::capability_id)
            .collect::<BTreeSet<_>>(),
        BTreeSet::from(["lenso.secrets@1"])
    );

    let registry = NativePluginRegistry::new().with_linked_factories();
    for release in &releases {
        let package_id = release.descriptor().runtime_package_id();
        assert_eq!(
            registry
                .factories()
                .filter(|factory| factory.package_id() == package_id)
                .count(),
            1,
            "{package_id} must have exactly one native factory"
        );
    }
    releases
}

/// Test-Host-only infrastructure. These endpoints prove the business graph can
/// resolve, but deliberately do not claim that Auth, Secrets, Access, Search,
/// HTTP egress, or privacy orchestration is delivered by this repository.
fn host_fixture_descriptor(real: &[HostPluginRelease]) -> PluginDescriptor {
    let provided = real
        .iter()
        .flat_map(|release| release.descriptor().provided_capabilities())
        .map(|endpoint| {
            (
                endpoint.capability_id().to_owned(),
                endpoint.descriptor_version().to_owned(),
            )
        })
        .collect::<BTreeSet<_>>();

    let mut missing = BTreeMap::<String, String>::new();
    for requirement in real
        .iter()
        .flat_map(|release| release.descriptor().required_capabilities())
    {
        let key = (
            requirement.capability_id().to_owned(),
            requirement.descriptor_version().to_owned(),
        );
        if provided.contains(&key) {
            continue;
        }
        if let Some(existing) = missing.insert(key.0.clone(), key.1.clone()) {
            assert_eq!(
                existing, key.1,
                "fixture cannot hide conflicting Descriptor versions for {}",
                key.0
            );
        }
    }

    missing.into_iter().fold(
        PluginDescriptor::new(HOST_FIXTURE_PLUGIN_ID, "1.0.0", "host-fixtures"),
        |descriptor, (capability_id, descriptor_version)| {
            descriptor.with_capability(CapabilityEndpointPlan::new(
                capability_id,
                descriptor_version,
                ["host_fixture_only"],
            ))
        },
    )
}

/// Test-only stand-in for the existing Web Ingress package, which has not yet
/// adopted linked descriptor inventory. Its identity and configuration field
/// match the real package, but this fixture is not runtime delivery evidence.
fn web_ingress_fixture_descriptor() -> PluginDescriptor {
    PluginDescriptor::new(WEB_INGRESS_PLUGIN_ID, "0.3.6", "http-ingress")
        .with_configuration_schema(serde_json::json!({
            "type": "object",
            "required": ["max_request_body_bytes"],
            "properties": {
                "max_request_body_bytes": {
                    "type": "integer",
                    "minimum": 12_582_912
                }
            },
            "additionalProperties": false
        }))
        .with_requirement(CapabilityRequirementPlan::many(
            "lenso.http.endpoint@1",
            "1.1.0",
        ))
}

fn support_host() -> HostCatalog {
    let mut releases = linked_suite_releases();
    releases.push(HostPluginRelease::new(host_fixture_descriptor(&releases)));
    releases.push(HostPluginRelease::new(web_ingress_fixture_descriptor()));

    let defaults = linked_plugin_ids()
        .map(|plugin_id| HostDefaultPlugin::new(plugin_id, INSTANCE).disableable())
        .chain([
            HostDefaultPlugin::new(WEB_INGRESS_PLUGIN_ID, INSTANCE),
            HostDefaultPlugin::new(HOST_FIXTURE_PLUGIN_ID, INSTANCE),
        ]);
    HostCatalog::new(host_slots(), releases, defaults)
}

fn support_web_host(provide_auth: bool, provide_support_case: bool) -> HostCatalog {
    let support_web = linked_suite_releases()
        .into_iter()
        .find(|release| release.descriptor().plugin_id() == SUPPORT_WEB_PLUGIN_ID)
        .expect("Support Web release must be linked");
    let fixture = [
        (provide_auth, "lenso.auth@1", "1.0.0"),
        (provide_support_case, "lenso.support-case@1", "1.0.0"),
    ]
    .into_iter()
    .filter(|(enabled, _, _)| *enabled)
    .fold(
        PluginDescriptor::new("fixture.support-web-platform", "1.0.0", "host-fixtures"),
        |descriptor, (_, capability_id, descriptor_version)| {
            descriptor.with_capability(CapabilityEndpointPlan::new(
                capability_id,
                descriptor_version,
                ["host_fixture_only"],
            ))
        },
    );

    HostCatalog::new(
        [HostSlot::many("web"), HostSlot::many("host-fixtures")],
        [support_web, HostPluginRelease::new(fixture)],
        [
            HostDefaultPlugin::new(SUPPORT_WEB_PLUGIN_ID, INSTANCE).disableable(),
            HostDefaultPlugin::new("fixture.support-web-platform", INSTANCE),
        ],
    )
}

fn support_web_root() -> PluginRootSnapshot {
    PluginRootSnapshot::new(
        [],
        [PluginRootInstance::new(SUPPORT_WEB_PLUGIN_ID, INSTANCE)
            .with_configuration(configuration(SUPPORT_WEB_PLUGIN_ID))],
        [],
    )
}

fn configuration(plugin_id: &str) -> Value {
    let path = repository_root()
        .join("plugins")
        .join(plugin_id)
        .join("default.toml");
    let source = fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
    let value = toml::from_str::<toml::Value>(&source)
        .unwrap_or_else(|error| panic!("parse {}: {error}", path.display()));
    serde_json::to_value(value).expect("TOML configuration must lower to JSON")
}

fn plugin_root(disabled: &[&str]) -> PluginRootSnapshot {
    let instances = plugin_root_ids().map(|plugin_id| {
        PluginRootInstance::new(plugin_id, INSTANCE).with_configuration(configuration(plugin_id))
    });
    let disabled = disabled
        .iter()
        .map(|plugin_id| PluginInstanceId::new(*plugin_id, INSTANCE));
    PluginRootSnapshot::new([], instances, disabled)
}

fn has_instance(plan: &lenso_app_plan::ResolvedAppPlan, plugin_id: &str) -> bool {
    let key = format!("{plugin_id}/{INSTANCE}");
    plan.plugin_instances()
        .iter()
        .any(|instance| instance.instance_key() == key)
}

fn binding_provider<'a>(
    plan: &'a lenso_app_plan::ResolvedAppPlan,
    consumer: &str,
    capability_id: &str,
) -> Option<&'a str> {
    plan.capability_bindings()
        .iter()
        .find(|binding| {
            binding.consumer_instance() == consumer && binding.capability_id() == capability_id
        })
        .map(lenso_app_plan::CapabilityBinding::provider_instance)
}

fn binding_providers<'a>(
    plan: &'a lenso_app_plan::ResolvedAppPlan,
    consumer: &str,
    capability_id: &str,
) -> BTreeSet<&'a str> {
    plan.capability_bindings()
        .iter()
        .filter(|binding| {
            binding.consumer_instance() == consumer && binding.capability_id() == capability_id
        })
        .map(lenso_app_plan::CapabilityBinding::provider_instance)
        .collect()
}

fn resolved_configuration(plan: &lenso_app_plan::ResolvedAppPlan, plugin_id: &str) -> Value {
    let instance_key = format!("{plugin_id}/{INSTANCE}");
    let instance = plan
        .plugin_instances()
        .iter()
        .find(|instance| instance.instance_key() == instance_key)
        .unwrap_or_else(|| panic!("{instance_key} must be present"));
    serde_json::from_str(instance.configuration()).expect("configuration must be JSON")
}

#[test]
fn plugin_root_contains_only_one_typed_instance_file_per_configured_plugin() {
    let plugins = repository_root().join("plugins");
    let directories = fs::read_dir(&plugins)
        .expect("Plugin Root must exist")
        .map(|entry| entry.expect("Plugin Root entry must be readable"))
        .collect::<Vec<_>>();
    assert_eq!(directories.len(), plugin_root_ids().count());

    for plugin_id in plugin_root_ids() {
        let directory = plugins.join(plugin_id);
        assert!(directory.is_dir(), "missing {}", directory.display());
        let entries = fs::read_dir(&directory)
            .expect("Plugin directory must be readable")
            .map(|entry| entry.expect("Plugin entry must be readable").path())
            .collect::<Vec<_>>();
        assert_eq!(entries, [directory.join("default.toml")]);
        assert!(configuration(plugin_id).is_object());
    }

    assert!(!repository_root().join("lenso.app.json").exists());
}

#[test]
fn test_host_auth_policy_has_the_exact_requester_and_agent_calling_chain_audiences() {
    let path = repository_root()
        .join("tests")
        .join("fixtures")
        .join("auth-credential-policy.toml");
    let source = fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
    let value = toml::from_str::<toml::Value>(&source)
        .unwrap_or_else(|error| panic!("parse {}: {error}", path.display()));
    let audiences = value["credential_audiences"]
        .as_array()
        .expect("credential_audiences must be an array")
        .iter()
        .map(|audience| audience.as_str().expect("audience must be a string"))
        .collect::<BTreeSet<_>>();
    assert_eq!(audiences, BTreeSet::from(AUTH_CREDENTIAL_AUDIENCES));
}

#[test]
fn full_support_graph_resolves_real_descriptors_and_unique_bindings() {
    let resolved = resolve_plugin_root(&support_host(), &plugin_root(&[]))
        .expect("complete Support Plugin Root must resolve");
    let plan = resolved.plan();
    plan.validate()
        .expect("resolved Plan must be internally valid");

    for plugin_id in linked_plugin_ids() {
        assert!(has_instance(plan, plugin_id));
    }
    assert!(has_instance(plan, WEB_INGRESS_PLUGIN_ID));
    assert!(has_instance(plan, HOST_FIXTURE_PLUGIN_ID));

    let binding_keys = plan
        .capability_bindings()
        .iter()
        .map(|binding| {
            (
                binding.consumer_instance(),
                binding.capability_id(),
                binding.descriptor_version(),
                binding.provider_instance(),
            )
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(binding_keys.len(), plan.capability_bindings().len());

    for consumer in plan.plugin_instances() {
        for requirement in consumer.required_capabilities() {
            let matching_bindings = plan
                .capability_bindings()
                .iter()
                .filter(|binding| {
                    binding.consumer_instance() == consumer.instance_key()
                        && binding.capability_id() == requirement.capability_id()
                        && binding.descriptor_version() == requirement.descriptor_version()
                })
                .count();
            let matching_providers = plan
                .plugin_instances()
                .iter()
                .filter(|provider| {
                    provider.provided_capabilities().iter().any(|endpoint| {
                        endpoint.capability_id() == requirement.capability_id()
                            && endpoint.descriptor_version() == requirement.descriptor_version()
                    })
                })
                .count();
            match requirement.cardinality() {
                CapabilityCardinality::One => assert_eq!(matching_bindings, 1),
                CapabilityCardinality::Optional => assert!(matching_bindings <= 1),
                CapabilityCardinality::Many => {
                    assert_eq!(matching_bindings, matching_providers);
                }
            }
        }
    }

    assert_eq!(
        binding_provider(
            plan,
            "lenso.support-email.resend/default",
            "lenso.customer-directory@1",
        ),
        Some("lenso.customer-directory.postgres/default")
    );
    assert_eq!(
        binding_provider(
            plan,
            "lenso.support-email.resend/default",
            "lenso.support-intake@1",
        ),
        Some("lenso.support-case.postgres/default")
    );
    assert_eq!(
        binding_provider(
            plan,
            "lenso.help-center.web/default",
            "lenso.knowledge-base@1",
        ),
        Some("lenso.knowledge-base.postgres/default")
    );
    assert_eq!(
        binding_provider(
            plan,
            "lenso.help-center.web/default",
            "lenso.support-attachment@1",
        ),
        Some("lenso.support-attachment.postgres/default")
    );
    assert_eq!(
        binding_provider(
            plan,
            "lenso.support-attachment.postgres/default",
            "lenso.support-case-authorization@1",
        ),
        Some("lenso.support-case.postgres/default")
    );
    assert_eq!(
        binding_provider(plan, "lenso.support.web/default", "lenso.support-case@1",),
        Some("lenso.support-case.postgres/default")
    );
    assert_eq!(
        binding_provider(plan, "lenso.support.web/default", "lenso.auth@1"),
        Some("fixture.support-platform/default")
    );

    let customer_config = resolved_configuration(plan, "lenso.customer-directory.postgres");
    assert_eq!(customer_config["schema"], "support_customer");
    assert_eq!(
        customer_config["database_url_secret"],
        "support/postgres/database-url"
    );
    assert_eq!(
        customer_config["resolve_callers"],
        serde_json::json!(["lenso.support-email.resend/default"])
    );
    assert_eq!(
        customer_config["admin_callers"],
        serde_json::json!(["support.admin/default"])
    );
    assert_eq!(
        customer_config["export_callers"],
        serde_json::json!(["privacy.export/default"])
    );
    assert_eq!(
        customer_config["retention_callers"],
        serde_json::json!(["privacy.retention/default"])
    );
    assert_eq!(customer_config["max_export_bytes"], 8 * 1024 * 1024);

    let case_config = resolved_configuration(plan, "lenso.support-case.postgres");
    assert_eq!(case_config["schema"], "support_case");
    assert_eq!(
        case_config["database_url_secret"],
        "support/postgres/database-url"
    );
    assert_eq!(
        case_config["business_callers"],
        serde_json::json!(["lenso.support.web/default"])
    );
    assert_eq!(
        case_config["intake_callers"],
        serde_json::json!([
            "lenso.help-center.web/default",
            "lenso.support-email.resend/default"
        ])
    );
    assert_eq!(
        case_config["resource_callers"],
        serde_json::json!(["lenso.support-attachment.postgres/default"])
    );
    assert_eq!(
        case_config["export_callers"],
        serde_json::json!(["privacy.export/default"])
    );
    assert_eq!(
        case_config["retention_callers"],
        serde_json::json!(["privacy.retention/default"])
    );
    assert_eq!(case_config["max_export_bytes"], 8 * 1024 * 1024);

    let attachment_config = resolved_configuration(plan, "lenso.support-attachment.postgres");
    assert_eq!(attachment_config["schema"], "support_attachment");
    assert_eq!(
        attachment_config["database_url_secret"],
        "support/postgres/database-url"
    );
    assert_eq!(
        attachment_config["business_callers"],
        serde_json::json!(["lenso.help-center.web/default"])
    );
    assert_eq!(
        binding_provider(
            plan,
            "lenso.support-attachment.postgres/default",
            "lenso.content-vault@1",
        ),
        Some("lenso.content-vault/default")
    );

    let content_vault_config = resolved_configuration(plan, CONTENT_VAULT_PLUGIN_ID);
    assert_eq!(
        content_vault_config["database_url_secret"],
        "support/content-vault/database-url"
    );
    assert_eq!(
        content_vault_config["maintenance_callers"],
        serde_json::json!(["content.maintenance/default"])
    );
    assert_eq!(content_vault_config["s3_bucket"], "support-content-vault");
    assert_eq!(content_vault_config["s3_region"], "us-east-1");
    assert_eq!(
        content_vault_config["s3_access_key_id_secret"],
        "support/content-vault/s3-access-key-id"
    );
    assert_eq!(
        content_vault_config["s3_secret_access_key_secret"],
        "support/content-vault/s3-secret-access-key"
    );
    assert_eq!(content_vault_config["quarantine_grace_seconds"], 900);
    assert_eq!(content_vault_config["sweep_batch_limit"], 100);
    assert_eq!(content_vault_config["stream_channel_capacity"], 4);

    let knowledge_config = resolved_configuration(plan, "lenso.knowledge-base.postgres");
    assert_eq!(knowledge_config["schema"], "support_knowledge_base");
    assert_eq!(
        knowledge_config["database_url_secret"],
        "support/postgres/database-url"
    );
    assert_eq!(
        knowledge_config["business_callers"],
        serde_json::json!(["support.admin/default"])
    );
    assert_eq!(
        knowledge_config["public_read_grants"],
        serde_json::json!([{
            "caller_instance": "lenso.help-center.web/default",
            "organization_id": "org_support_demo"
        }])
    );

    let resend_config = resolved_configuration(plan, "lenso.support-email.resend");
    let help_config = resolved_configuration(plan, "lenso.help-center.web");
    assert_eq!(
        resend_config["organization_id"],
        help_config["organization_id"]
    );
    assert_eq!(
        resend_config["organization_id"],
        knowledge_config["public_read_grants"][0]["organization_id"]
    );
    assert_eq!(
        resend_config["webhook_secret_reference"],
        "support/resend/webhook-signing-secret"
    );
    assert_eq!(
        resend_config["api_key_secret_reference"],
        "support/resend/api-key"
    );
    assert_eq!(
        resend_config["reply_token_secret_reference"],
        "support/resend/reply-token"
    );
    assert_eq!(
        resend_config["recipient_addresses"],
        serde_json::json!(["support@example.com"])
    );
    assert_eq!(resend_config["max_webhook_age_seconds"], 300);
    assert_eq!(
        [
            &resend_config["webhook_secret_reference"],
            &resend_config["api_key_secret_reference"],
            &resend_config["reply_token_secret_reference"],
        ]
        .into_iter()
        .map(|reference| reference
            .as_str()
            .expect("secret reference must be a string"))
        .collect::<BTreeSet<_>>()
        .len(),
        3
    );

    let ingress_config = resolved_configuration(plan, WEB_INGRESS_PLUGIN_ID);
    assert_eq!(ingress_config["max_request_body_bytes"], 12 * 1024 * 1024);
    let ingress_providers =
        binding_providers(plan, "lenso.web-ingress/default", "lenso.http.endpoint@1");
    assert_eq!(
        ingress_providers,
        BTreeSet::from([
            "lenso.help-center.web/default",
            "lenso.support-email.resend/default",
            "lenso.support.web/default",
        ])
    );

    let target_configs = [
        customer_config,
        case_config,
        attachment_config,
        knowledge_config,
        help_config,
    ];
    assert!(target_configs.iter().all(|config| {
        config["auth_issuer"] == "support-auth"
            && config["auth_assertion_public_key"] == "11qYAYKxCrfVS_7TyWQHOg7hcvPapiMlrwIaaPcHURo"
    }));
}

#[test]
fn disabling_resend_preserves_the_help_center_path() {
    let resolved = resolve_plugin_root(
        &support_host(),
        &plugin_root(&["lenso.support-email.resend"]),
    )
    .expect("Help Center path must not depend on Resend");
    let plan = resolved.plan();
    assert!(!has_instance(plan, "lenso.support-email.resend"));
    assert!(has_instance(plan, "lenso.help-center.web"));
    assert_eq!(
        binding_provider(
            plan,
            "lenso.help-center.web/default",
            "lenso.support-intake@1"
        ),
        Some("lenso.support-case.postgres/default")
    );
    assert_eq!(
        binding_providers(plan, "lenso.web-ingress/default", "lenso.http.endpoint@1"),
        BTreeSet::from(["lenso.help-center.web/default", "lenso.support.web/default",])
    );
}

#[test]
fn disabling_help_center_preserves_the_email_path() {
    let resolved = resolve_plugin_root(&support_host(), &plugin_root(&["lenso.help-center.web"]))
        .expect("email path must not depend on Help Center");
    let plan = resolved.plan();
    assert!(!has_instance(plan, "lenso.help-center.web"));
    assert!(has_instance(plan, "lenso.support-email.resend"));
    assert_eq!(
        binding_provider(
            plan,
            "lenso.support-email.resend/default",
            "lenso.support-intake@1"
        ),
        Some("lenso.support-case.postgres/default")
    );
    assert_eq!(
        binding_providers(plan, "lenso.web-ingress/default", "lenso.http.endpoint@1"),
        BTreeSet::from([
            "lenso.support-email.resend/default",
            "lenso.support.web/default",
        ])
    );
}

#[test]
fn disabling_support_web_only_removes_the_agent_surface() {
    let resolved = resolve_plugin_root(&support_host(), &plugin_root(&[SUPPORT_WEB_PLUGIN_ID]))
        .expect("requester intake and email intake must not depend on Support Web");
    let plan = resolved.plan();

    assert!(!has_instance(plan, SUPPORT_WEB_PLUGIN_ID));
    for plugin_id in linked_plugin_ids().filter(|plugin_id| *plugin_id != SUPPORT_WEB_PLUGIN_ID) {
        assert!(has_instance(plan, plugin_id));
    }
    assert_eq!(
        binding_provider(
            plan,
            "lenso.help-center.web/default",
            "lenso.support-intake@1",
        ),
        Some("lenso.support-case.postgres/default")
    );
    assert_eq!(
        binding_provider(
            plan,
            "lenso.support-email.resend/default",
            "lenso.support-intake@1",
        ),
        Some("lenso.support-case.postgres/default")
    );
    assert_eq!(
        binding_providers(plan, "lenso.web-ingress/default", "lenso.http.endpoint@1"),
        BTreeSet::from([
            "lenso.help-center.web/default",
            "lenso.support-email.resend/default",
        ])
    );
}

#[test]
fn support_web_fails_closed_without_support_case() {
    let error = resolve_plugin_root(&support_web_host(true, false), &support_web_root())
        .expect_err("Support Web requires exactly one Support Case provider");
    assert_eq!(
        error,
        PluginRootResolutionError::MissingCapability {
            consumer: PluginInstanceId::new(SUPPORT_WEB_PLUGIN_ID, INSTANCE),
            capability_id: "lenso.support-case@1".to_owned(),
            descriptor_version: "1.0.0".to_owned(),
        }
    );
}

#[test]
fn support_web_fails_closed_without_auth() {
    let error = resolve_plugin_root(&support_web_host(false, true), &support_web_root())
        .expect_err("Support Web requires exactly one Auth provider");
    assert_eq!(
        error,
        PluginRootResolutionError::MissingCapability {
            consumer: PluginInstanceId::new(SUPPORT_WEB_PLUGIN_ID, INSTANCE),
            capability_id: "lenso.auth@1".to_owned(),
            descriptor_version: "1.0.0".to_owned(),
        }
    );
}

#[test]
fn disabling_customer_directory_blocks_resend_resolution() {
    let error = resolve_plugin_root(
        &support_host(),
        &plugin_root(&["lenso.customer-directory.postgres"]),
    )
    .expect_err("Resend requires the Customer Directory provider");
    assert_eq!(
        error,
        PluginRootResolutionError::MissingCapability {
            consumer: PluginInstanceId::new("lenso.support-email.resend", INSTANCE),
            capability_id: "lenso.customer-directory@1".to_owned(),
            descriptor_version: "1.0.0".to_owned(),
        }
    );
}

#[test]
fn disabling_support_case_blocks_attachment_resolution() {
    let error = resolve_plugin_root(
        &support_host(),
        &plugin_root(&["lenso.support-case.postgres"]),
    )
    .expect_err("Support Attachment requires Support Case authorization");
    assert_eq!(
        error,
        PluginRootResolutionError::MissingCapability {
            consumer: PluginInstanceId::new("lenso.support-attachment.postgres", INSTANCE),
            capability_id: "lenso.support-case-authorization@1".to_owned(),
            descriptor_version: "1.0.0".to_owned(),
        }
    );
}

#[test]
fn disabling_support_case_blocks_resend_after_other_dependents_are_disabled() {
    let error = resolve_plugin_root(
        &support_host(),
        &plugin_root(&[
            "lenso.support-case.postgres",
            "lenso.help-center.web",
            SUPPORT_WEB_PLUGIN_ID,
            "lenso.support-attachment.postgres",
        ]),
    )
    .expect_err("Resend requires the Support Case intake provider");
    assert_eq!(
        error,
        PluginRootResolutionError::MissingCapability {
            consumer: PluginInstanceId::new("lenso.support-email.resend", INSTANCE),
            capability_id: "lenso.support-intake@1".to_owned(),
            descriptor_version: "1.0.0".to_owned(),
        }
    );
}

#[test]
fn dependent_plugin_groups_can_be_disabled_without_falling_back_to_host_defaults() {
    for disabled in [
        vec![
            "lenso.support-email.resend",
            "lenso.customer-directory.postgres",
        ],
        vec!["lenso.help-center.web", "lenso.knowledge-base.postgres"],
        vec![
            "lenso.help-center.web",
            "lenso.support-attachment.postgres",
            CONTENT_VAULT_PLUGIN_ID,
        ],
        vec![
            "lenso.support-email.resend",
            "lenso.help-center.web",
            SUPPORT_WEB_PLUGIN_ID,
            "lenso.support-attachment.postgres",
            "lenso.support-case.postgres",
        ],
    ] {
        resolve_plugin_root(&support_host(), &plugin_root(&disabled))
            .expect("a provider and all of its dependent default Instances can be disabled");
    }
}

fn assert_help_center_dependency_failure(plugin_id: &str, capability_id: &str) {
    let error = resolve_plugin_root(&support_host(), &plugin_root(&[plugin_id]))
        .expect_err("removing a required Help Center provider must fail closed");
    assert_eq!(
        error,
        PluginRootResolutionError::MissingCapability {
            consumer: PluginInstanceId::new("lenso.help-center.web", INSTANCE),
            capability_id: capability_id.to_owned(),
            descriptor_version: if capability_id == "lenso.support-attachment@1" {
                "1.1.0".to_owned()
            } else {
                "1.0.0".to_owned()
            },
        }
    );
}

#[test]
fn disabling_knowledge_base_blocks_help_center_resolution() {
    assert_help_center_dependency_failure(
        "lenso.knowledge-base.postgres",
        "lenso.knowledge-base@1",
    );
}

#[test]
fn disabling_attachment_blocks_help_center_resolution() {
    assert_help_center_dependency_failure(
        "lenso.support-attachment.postgres",
        "lenso.support-attachment@1",
    );
}

#[test]
fn disabling_content_vault_blocks_attachment_resolution() {
    let error = resolve_plugin_root(&support_host(), &plugin_root(&[CONTENT_VAULT_PLUGIN_ID]))
        .expect_err("Support Attachment requires the real Content Vault provider");
    assert_eq!(
        error,
        PluginRootResolutionError::MissingCapability {
            consumer: PluginInstanceId::new("lenso.support-attachment.postgres", INSTANCE),
            capability_id: "lenso.content-vault@1".to_owned(),
            descriptor_version: "1.0.0".to_owned(),
        }
    );
}

#[test]
fn host_fixture_supplies_only_platform_dependencies_missing_from_the_suite() {
    let releases = linked_suite_releases();
    let fixture = host_fixture_descriptor(&releases);
    let fixture_capabilities = fixture
        .provided_capabilities()
        .iter()
        .map(lenso_app_plan::CapabilityEndpointPlan::capability_id)
        .collect::<BTreeSet<_>>();

    assert_eq!(
        fixture_capabilities,
        BTreeSet::from([
            "lenso.access-control@1",
            "lenso.auth@1",
            "lenso.http.client@1",
            "lenso.organization-membership@1",
            "lenso.search-index@1",
            "lenso.search@1",
            "lenso.secrets@1",
        ])
    );
    for business_capability in [
        "lenso.content-vault@1",
        "lenso.customer-directory@1",
        "lenso.knowledge-base@1",
        "lenso.support-attachment@1",
        "lenso.support-case@1",
        "lenso.support-case-authorization@1",
        "lenso.support-intake@1",
    ] {
        assert!(!fixture_capabilities.contains(business_capability));
    }
}

#[test]
fn plugin_root_paths_are_relative_only_to_the_app_repository() {
    for plugin_id in plugin_root_ids() {
        let path = Path::new("plugins").join(plugin_id).join("default.toml");
        assert!(repository_root().join(path).is_file());
    }
}
