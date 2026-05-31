use crate::kube_config::current_namespace;
use crate::ui;
use anyhow::Result;
use kube::config::Kubeconfig;

pub fn run(config: Kubeconfig, current_context_name: String) -> Result<()> {
    let current_ns = current_namespace(&config, &current_context_name)?;
    ui::print_info(&format!("Current context: {:?}", current_context_name));
    ui::print_info(&format!("Current namespace: {:?}", current_ns));
    Ok(())
}
