use anyhow::{Context as _, Result};
use kube::config::{Context, Kubeconfig};
use std::path::PathBuf;

/// Discovers and loads the Kubernetes configuration from standard locations (KUBECONFIG or ~/.kube/config).
pub fn load_kube_config() -> Result<(Kubeconfig, PathBuf)> {
    let config_path = std::env::var_os("KUBECONFIG")
        .map(PathBuf::from)
        .or_else(|| {
            #[allow(deprecated)]
            std::env::home_dir().map(|h| h.join(".kube").join("config"))
        })
        .context("Could not find kubeconfig path. Please set KUBECONFIG or ensure HOME is set.")?;

    let config = Kubeconfig::read().context("Failed to read kubeconfig")?;

    Ok((config, config_path))
}

/// Serializes and saves the Kubernetes configuration back to the disk.
pub fn save_kube_config(config: &Kubeconfig, path: &PathBuf) -> Result<()> {
    let yaml_content =
        serde_yaml::to_string(config).context("failed to serialize kubeconfig to yaml")?;
    std::fs::write(path, yaml_content)
        .with_context(|| format!("failed to write to config file at {:?}", path))?;

    Ok(())
}

/// Returns the current namespace for a given context name.
pub fn current_namespace(config: &Kubeconfig, name: &str) -> Result<String> {
    let context_entry = config
        .contexts
        .iter()
        .find(|c| c.name == name)
        .context("Current context not found in config file")?;

    let ctx_struct = context_entry
        .context
        .as_ref()
        .context("Context data is missing/invalid")?;

    let current_namespace = ctx_struct
        .namespace
        .clone()
        .unwrap_or("default".to_string());

    Ok(current_namespace)
}

/// Returns a mutable reference to the internal Context structure for the specified context name.
pub fn get_mut_context<'a>(
    config: &'a mut Kubeconfig,
    current_context_name: &str,
) -> Result<&'a mut Context> {
    let context_entry = config
        .contexts
        .iter_mut()
        .find(|c| c.name == current_context_name)
        .context("Current context not found in config file")?;

    let ctx_struct = context_entry
        .context
        .as_mut()
        .context("This context is empty or invalid")?;

    Ok(ctx_struct)
}

/// Fetches the list of namespaces from the Kubernetes API and updates the local cache.
/// Supports an optional timeout, primarily for shell completion use cases.
pub async fn list_and_cache_namespaces(
    context_name: &str,
    timeout: Option<std::time::Duration>,
) -> Result<Vec<String>> {
    let client = kube::Client::try_default()
        .await
        .context("Failed to connect to Kubernetes cluster")?;
    let client_api: kube::Api<k8s_openapi::api::core::v1::Namespace> = kube::Api::all(client);

    let params = kube::api::ListParams::default();
    let list_future = client_api.list(&params);

    let ns_list = if let Some(duration) = timeout {
        tokio::time::timeout(duration, list_future)
            .await
            .map_err(|_| anyhow::anyhow!("Network timeout while fetching namespaces"))??
    } else {
        list_future.await.context("Failed to list namespaces")?
    };

    let ns_names: Vec<String> = ns_list
        .items
        .into_iter()
        .filter_map(|ns| ns.metadata.name)
        .collect();

    if ns_names.is_empty() {
        return Err(anyhow::anyhow!("No namespaces found in the cluster."));
    }

    // Update cache
    let _ = crate::cache::NamespaceCache::save(context_name, ns_names.clone());

    Ok(ns_names)
}

/// Provides shell completion candidates for Kubernetes namespaces.
/// Leverages the local cache for high performance and falls back to a timed network request.
pub fn complete_namespaces(
    current: &std::ffi::OsStr,
) -> Vec<clap_complete::engine::CompletionCandidate> {
    let (config, _) = match load_kube_config() {
        Ok(c) => c,
        Err(e) => {
            return vec![
                clap_complete::engine::CompletionCandidate::new("")
                    .help(Some(format!("Config Error: {}", e).into())),
            ];
        }
    };

    let current_context = match config.current_context.as_deref() {
        Some(c) => c,
        None => {
            return vec![
                clap_complete::engine::CompletionCandidate::new("")
                    .help(Some("Error: No active context set".into())),
            ];
        }
    };

    // Try loading from cache first
    if let Ok(Some(namespaces)) = crate::cache::NamespaceCache::load(current_context) {
        let current_str = current.to_string_lossy().to_lowercase();
        return namespaces
            .into_iter()
            .filter(|name| name.to_lowercase().contains(&current_str))
            .map(|name| {
                clap_complete::engine::CompletionCandidate::new(name).help(Some("Namespace".into()))
            })
            .collect();
    }

    let rt = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(_) => return vec![],
    };

    rt.block_on(async {
        let timeout_duration = std::time::Duration::from_millis(200);
        let ns_names =
            match list_and_cache_namespaces(current_context, Some(timeout_duration)).await {
                Ok(names) => names,
                Err(e) => {
                    let msg = if e.to_string().contains("Network timeout") {
                        "Error: Network timeout (200ms)".to_string()
                    } else {
                        format!("Error: {}", e)
                    };
                    return vec![
                        clap_complete::engine::CompletionCandidate::new("").help(Some(msg.into())),
                    ];
                }
            };

        let current_str = current.to_string_lossy().to_lowercase();
        ns_names
            .into_iter()
            .filter(|name| name.to_lowercase().contains(&current_str))
            .map(|name| {
                clap_complete::engine::CompletionCandidate::new(name).help(Some("Namespace".into()))
            })
            .collect()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use kube::config::{Context, NamedContext};

    #[test]
    fn test_current_namespace_default() {
        let mut config = Kubeconfig::default();
        config.contexts.push(NamedContext {
            name: "test-ctx".to_string(),
            context: Some(Context {
                cluster: "test-cluster".to_string(),
                user: Some("test-user".to_string()),
                namespace: None,
                extensions: None,
            }),
        });

        let ns = current_namespace(&config, "test-ctx").unwrap();
        assert_eq!(ns, "default");
    }

    #[test]
    fn test_current_namespace_custom() {
        let mut config = Kubeconfig::default();
        config.contexts.push(NamedContext {
            name: "test-ctx".to_string(),
            context: Some(Context {
                cluster: "test-cluster".to_string(),
                user: Some("test-user".to_string()),
                namespace: Some("custom-ns".to_string()),
                extensions: None,
            }),
        });

        let ns = current_namespace(&config, "test-ctx").unwrap();
        assert_eq!(ns, "custom-ns");
    }
}
