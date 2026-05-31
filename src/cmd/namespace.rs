use crate::kube_config::{get_mut_context, save_kube_config};
use crate::ui;
use anyhow::{Context as _, Result, anyhow};
use k8s_openapi::api::core::v1::Namespace;
use kube::config::Kubeconfig;
use kube::{Api, Client};
use std::path::PathBuf;

pub async fn run(
    mut config: Kubeconfig,
    config_path: PathBuf,
    current_context_name: &str,
    namespace_name: Option<String>,
) -> Result<()> {
    if current_context_name == "none" {
        return Err(anyhow!("No active context set, cannot change namespace"));
    }

    let ctx = get_mut_context(&mut config, current_context_name)?;
    let current_namespace = ctx.namespace.clone().unwrap_or("default".to_string());

    let target_namespace = match namespace_name {
        Some(name) => {
            let client = Client::try_default()
                .await
                .context("Failed to connect to Kubernetes cluster")?;
            let api_client: Api<Namespace> = Api::all(client);

            if api_client.get(&name).await.is_err() {
                return Err(anyhow!("Namespace {name} does not exist."));
            }
            name
        }
        None => {
            let ns_names =
                crate::kube_config::list_and_cache_namespaces(current_context_name, None).await?;

            let default_index = ns_names
                .iter()
                .position(|n| n == &current_namespace)
                .unwrap_or(0);

            match ui::fuzzy_select("Select namespace", &ns_names, default_index)? {
                Some(ns) => ns,
                None => return Ok(()),
            }
        }
    };

    ctx.namespace = Some(target_namespace.clone());
    save_kube_config(&config, &config_path)?;

    ui::print_success(&format!("Context {:?} modified.", current_context_name));
    ui::print_success(&format!("Active namespace is {:?}", target_namespace));
    Ok(())
}
