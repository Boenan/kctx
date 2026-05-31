use crate::kube_config::save_kube_config;
use crate::ui;
use anyhow::{Result, anyhow};
use kube::config::Kubeconfig;
use std::path::PathBuf;

pub fn run(
    config: &mut Kubeconfig,
    config_path: &PathBuf,
    current_context_name: &str,
    context_name: Option<String>,
) -> Result<()> {
    let ctxs: Vec<String> = config.contexts.iter().map(|c| c.name.clone()).collect();
    let target_delete_context = match context_name {
        Some(name) => {
            if !ctxs.contains(&name) {
                return Err(anyhow!(
                    "the context {name} does not exist in the kubeconfig."
                ));
            }
            name
        }
        None => {
            let default_index = ctxs
                .iter()
                .position(|c| c == current_context_name)
                .unwrap_or(0);

            match ui::fuzzy_select("Delete kubernetes context", &ctxs, default_index)? {
                Some(ctx) => ctx,
                None => return Ok(()),
            }
        }
    };

    let prompt = format!(
        "Are you sure you want to delete context '{}'?",
        target_delete_context
    );

    let confirmed = ui::confirm(&prompt)?;
    ui::print_choice_confirmed(&prompt, confirmed)?;

    if confirmed {
        config.contexts.retain(|c| c.name != target_delete_context);
        if current_context_name == target_delete_context {
            config.current_context = None;
        }

        save_kube_config(config, config_path)?;
        println!("Deleted context {:?}", target_delete_context);
    }
    Ok(())
}
