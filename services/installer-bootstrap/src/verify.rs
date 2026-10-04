//! Product verification on the server (P15): V01–V09, V12b, V16.
//! D011_VERIFICATION_MATRIX. **Read-only**: it asks the services, the Core and
//! the proxy; it never starts, migrates or repairs anything to make an item
//! pass.

use ocinye_installer_contracts::core_output::{
    VerifyAdminBootstrap, VerifyEndpoint, VerifyInstance, VerifySchema,
};
use ocinye_installer_contracts::manifest::ReleaseManifest;
use ocinye_installer_contracts::plan::InstallationPlan;
use ocinye_installer_contracts::verification::{ItemStatus, VerificationId, VerificationItem};
use serde::Deserialize;

use crate::exec::{self, program};

/// The services that must be healthy (V01). `worker` has no healthcheck and
/// must be running.
pub const HEALTHY: [&str; 6] = [
    "core",
    "workspace",
    "proxy",
    "postgres",
    "redis",
    "object-store",
];

fn item(id: VerificationId, status: ItemStatus, evidence: &str) -> VerificationItem {
    VerificationItem {
        id,
        status,
        evidence: evidence.to_owned(),
    }
}

fn core_json(sub: &str) -> Option<String> {
    let o = exec::run(
        program::DOCKER,
        &[
            "exec",
            "ocinye-core-1",
            "/usr/local/bin/ocinye",
            sub,
            "--json",
        ],
        exec::secs(60),
    );
    o.ok().then(|| o.stdout.trim().to_owned())
}

#[derive(Deserialize)]
struct ReadyComponent {
    component: String,
    state: String,
}

#[derive(Deserialize)]
struct Ready {
    overall: String,
    contract_version: u32,
    components: Vec<ReadyComponent>,
}

/// Compare what the Core reports with the plan (V05/V06).
#[must_use]
pub fn instance_matches(plan: &InstallationPlan, got: &VerifyInstance) -> (bool, bool) {
    let c = &plan.configuration;
    let name = got.name == c.instance_name.as_str() && got.slug == c.instance_name.derived_slug();
    let want: Vec<&str> = c
        .distributions
        .as_slice()
        .iter()
        .map(|d| d.as_str())
        .collect();
    let mut have: Vec<&str> = got.distributions.iter().map(String::as_str).collect();
    let birth_ok = have.first() == want.first();
    have.sort_unstable();
    let mut want_sorted = want.clone();
    want_sorted.sort_unstable();
    (name, birth_ok && have == want_sorted)
}

/// Compare the endpoints with the plan (V12b).
#[must_use]
pub fn endpoints_match(plan: &InstallationPlan, got: &[VerifyEndpoint]) -> bool {
    let e = &plan.configuration.endpoints;
    let canonical_ok = got.iter().any(|g| {
        g.canonical
            && g.host == e.canonical.as_str()
            && g.distribution.is_none()
            && g.state == "active"
    });
    let bound_ok = e.bound.iter().all(|b| {
        got.iter().any(|g| {
            !g.canonical
                && g.host == b.host.as_str()
                && g.distribution.as_deref() == Some(b.distribution.as_str())
                && g.state == "active"
        })
    });
    canonical_ok && bound_ok && got.len() == e.bound.len() + 1
}

/// `curl --resolve host:443:127.0.0.1` → status code.
fn in_server_status(host: &str, path: &str) -> Option<u16> {
    let resolve = format!("{host}:443:127.0.0.1");
    let url = format!("https://{host}{path}");
    let o = exec::run(
        program::CURL,
        &[
            "-ks",
            "-o",
            "/dev/null",
            "-w",
            "%{http_code}",
            "--max-time",
            "20",
            "--resolve",
            &resolve,
            &url,
        ],
        exec::secs(30),
    );
    o.stdout.trim().parse().ok()
}

/// Run the server items, in order, calling `emit` for each.
#[allow(clippy::too_many_lines)]
pub fn run(
    plan: &InstallationPlan,
    manifest: &ReleaseManifest,
    emit: &mut dyn FnMut(&VerificationItem),
) -> Vec<VerificationItem> {
    let mut out = Vec::new();
    let mut push = |i: VerificationItem, out: &mut Vec<VerificationItem>| {
        emit(&i);
        out.push(i);
    };

    // V01 · services.
    let mut unhealthy = Vec::new();
    for svc in HEALTHY {
        let o = exec::run(
            program::DOCKER,
            &[
                "inspect",
                "-f",
                "{{if .State.Health}}{{.State.Health.Status}}{{else}}none{{end}}",
                &format!("ocinye-{svc}-1"),
            ],
            exec::secs(15),
        );
        if o.stdout.trim() != "healthy" {
            unhealthy.push(svc);
        }
    }
    let worker = exec::run(
        program::DOCKER,
        &["inspect", "-f", "{{.State.Running}}", "ocinye-worker-1"],
        exec::secs(15),
    );
    if worker.stdout.trim() != "true" {
        unhealthy.push("worker");
    }
    push(
        if unhealthy.is_empty() {
            item(VerificationId::V01, ItemStatus::Pass, "SERVICES_HEALTHY")
        } else {
            item(
                VerificationId::V01,
                ItemStatus::Fail,
                &format!("SERVICE_UNHEALTHY:{}", unhealthy.join(",")),
            )
        },
        &mut out,
    );

    // V02/V03 · database and schema.
    match core_json("verify-schema").and_then(|s| serde_json::from_str::<VerifySchema>(&s).ok()) {
        Some(s) => {
            push(
                item(VerificationId::V02, ItemStatus::Pass, "DATABASE_REACHABLE"),
                &mut out,
            );
            let ok = s.pending == 0
                && s.latest == manifest.migrations.latest
                && s.count == manifest.migrations.count;
            push(
                item(
                    VerificationId::V03,
                    if ok {
                        ItemStatus::Pass
                    } else {
                        ItemStatus::Fail
                    },
                    &format!("SCHEMA_{}_{}", s.latest, s.pending),
                ),
                &mut out,
            );
        }
        None => {
            push(
                item(
                    VerificationId::V02,
                    ItemStatus::Fail,
                    "DATABASE_UNREACHABLE",
                ),
                &mut out,
            );
            push(
                item(VerificationId::V03, ItemStatus::NotRun, "NOT_RUN"),
                &mut out,
            );
        }
    }

    // V04/V08 · readiness.
    let url = format!(
        "http://127.0.0.1:8080/ready?contract={}",
        manifest.compatibility.readiness_contract
    );
    let ready = exec::run(
        program::DOCKER,
        &[
            "exec",
            "ocinye-core-1",
            "curl",
            "-sS",
            "--max-time",
            "20",
            &url,
        ],
        exec::secs(30),
    );
    let parsed: Option<Ready> = serde_json::from_str(ready.stdout.trim()).ok();
    match &parsed {
        Some(r)
            if r.overall != "blocked"
                && r.contract_version == manifest.compatibility.readiness_contract =>
        {
            push(
                item(
                    VerificationId::V04,
                    ItemStatus::Pass,
                    &format!("READY_{}", r.overall.to_uppercase()),
                ),
                &mut out,
            );
        }
        Some(r) => push(
            item(
                VerificationId::V04,
                ItemStatus::Fail,
                &format!("READY_{}", r.overall.to_uppercase()),
            ),
            &mut out,
        ),
        None => push(
            item(VerificationId::V04, ItemStatus::Fail, "READY_UNREACHABLE"),
            &mut out,
        ),
    }

    // V05/V06/V07 · Instance, Distributions, registry — read from the Core.
    match core_json("verify-instance").and_then(|s| serde_json::from_str::<VerifyInstance>(&s).ok())
    {
        Some(got) => {
            let (name, dists) = instance_matches(plan, &got);
            push(
                item(
                    VerificationId::V05,
                    if name {
                        ItemStatus::Pass
                    } else {
                        ItemStatus::Fail
                    },
                    if name {
                        "INSTANCE_MATCHES"
                    } else {
                        "INSTANCE_DIFFERS"
                    },
                ),
                &mut out,
            );
            push(
                item(
                    VerificationId::V06,
                    if dists {
                        ItemStatus::Pass
                    } else {
                        ItemStatus::Fail
                    },
                    if dists {
                        "DISTRIBUTIONS_MATCH"
                    } else {
                        "DISTRIBUTIONS_DIFFER"
                    },
                ),
                &mut out,
            );
            let apps = got.applications.registered > 0
                && got.applications.active > 0
                && got.applications.essential_inactive == 0;
            push(
                item(
                    VerificationId::V07,
                    if apps {
                        ItemStatus::Pass
                    } else {
                        ItemStatus::Fail
                    },
                    &format!(
                        "APPLICATIONS_{}_{}",
                        got.applications.active, got.applications.registered
                    ),
                ),
                &mut out,
            );
        }
        None => {
            for id in [
                VerificationId::V05,
                VerificationId::V06,
                VerificationId::V07,
            ] {
                push(item(id, ItemStatus::Fail, "INSTANCE_UNREADABLE"), &mut out);
            }
        }
    }

    let storage_ok = parsed.as_ref().is_some_and(|r| {
        r.components
            .iter()
            .any(|c| c.component == "storage" && c.state == "available")
    });
    push(
        item(
            VerificationId::V08,
            if storage_ok {
                ItemStatus::Pass
            } else {
                ItemStatus::Fail
            },
            if storage_ok {
                "STORAGE_AVAILABLE"
            } else {
                "STORAGE_UNAVAILABLE"
            },
        ),
        &mut out,
    );

    // V09 · login surface, inside the server.
    let canonical = plan.configuration.endpoints.canonical.as_str();
    let root = in_server_status(canonical, "/");
    let login = in_server_status(canonical, "/login");
    push(
        item(
            VerificationId::V09,
            if root == Some(303) && login == Some(200) {
                ItemStatus::Pass
            } else {
                ItemStatus::Fail
            },
            &format!("HTTP_{}_{}", root.unwrap_or(0), login.unwrap_or(0)),
        ),
        &mut out,
    );

    // V12b · endpoints as the Core has them.
    let eps = core_json("verify-endpoints")
        .and_then(|s| serde_json::from_str::<Vec<VerifyEndpoint>>(&s).ok());
    let ok = eps.as_ref().is_some_and(|e| endpoints_match(plan, e));
    push(
        item(
            VerificationId::V12b,
            if ok {
                ItemStatus::Pass
            } else {
                ItemStatus::Fail
            },
            if ok {
                "ENDPOINTS_MATCH"
            } else {
                "ENDPOINTS_DIFFER"
            },
        ),
        &mut out,
    );

    // V16 · first access (informational).
    let adm = core_json("verify-admin-bootstrap")
        .and_then(|s| serde_json::from_str::<VerifyAdminBootstrap>(&s).ok());
    push(
        item(
            VerificationId::V16,
            if adm.is_some_and(|a| a.privileged_identity_exists) {
                ItemStatus::Pass
            } else {
                ItemStatus::Fail
            },
            match adm {
                Some(a) if a.temporary_credential_pending => "FIRST_ACCESS_PENDING",
                Some(_) => "FIRST_ACCESS_DONE",
                None => "ADMIN_UNREADABLE",
            },
        ),
        &mut out,
    );
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use ocinye_installer_contracts::core_output::VerifyApplications;

    fn plan() -> InstallationPlan {
        crate::tests::fixture_plan()
    }

    #[test]
    fn a_instancia_confere_pelo_nome_slug_e_distribuicoes() {
        let p = plan();
        let mut got = VerifyInstance {
            name: "Empresa Exemplo".into(),
            slug: "empresa-exemplo".into(),
            distributions: vec!["business".into(), "research".into()],
            applications: VerifyApplications {
                registered: 28,
                active: 20,
                essential_inactive: 0,
            },
        };
        assert_eq!(instance_matches(&p, &got), (true, true));
        got.distributions = vec!["research".into(), "business".into()];
        assert!(!instance_matches(&p, &got).1, "a de nascimento mudou");
        got.distributions = vec!["business".into()];
        assert!(!instance_matches(&p, &got).1, "uma Distribuição em falta");
        got.name = "Outra".into();
        assert!(!instance_matches(&p, &got).0);
    }

    #[test]
    fn os_pontos_conferem_um_a_um() {
        let p = plan();
        let ep = |h: &str, c: bool, d: Option<&str>| VerifyEndpoint {
            host: h.into(),
            canonical: c,
            distribution: d.map(str::to_owned),
            state: "active".into(),
        };
        let mut got = vec![
            ep("os.empresa.test", true, None),
            ep("business.empresa.test", false, Some("business")),
            ep("research.empresa.test", false, Some("research")),
        ];
        assert!(endpoints_match(&p, &got));
        got[2].distribution = Some("business".into());
        assert!(!endpoints_match(&p, &got));
        got.pop();
        assert!(!endpoints_match(&p, &got));
    }
}
