use anyhow::{Context as AnyhowContext, Result};
use clap::{CommandFactory, Parser, Subcommand};
use clap_complete::engine::{ArgValueCompleter, CompletionCandidate};
use clap_complete::env::CompleteEnv;
use console::{Term, style};
use dialoguer::{Confirm, FuzzySelect, theme::ColorfulTheme};
use k8s_openapi::api::core::v1::Namespace;
use kube::api::ListParams;
use kube::config::{Context, Kubeconfig};
use kube::{Api, Client};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "kctx")]
#[command(about = "A k8s context switcher", long_about = None)]
#[command(disable_help_subcommand = true)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    #[command(name = "context", about = "Change context", long_about = None)]
    ChangeContext {
        /// The context to switch to
        /// If omitted, opens up a interactive selection list
        #[arg(verbatim_doc_comment)]
        context_name: Option<String>,
    },
    #[command(name = "delete", about = "Delete context (won't delete user/cluster entry)", long_about = None)]
    Delete {
        /// The context to delete
        /// If omitted, opens up a interactive selection list
        #[arg(verbatim_doc_comment)]
        context_name: Option<String>,
    },
    #[command(name = "namespace", about = "Change namespace for the current context", long_about = None)]
    ChangeNamespace {
        /// The namespace to switch to
        /// If omitted, opens up a interactive selection list
        #[arg(verbatim_doc_comment, add = ArgValueCompleter::new(complete_namespaces))]
        namespace_name: Option<String>,
    },
    #[command(about = "Print current context and namespace information", long_about = None)]
    Info {},
    #[command(about = "Generate shell completion scripts", long_about = None)]
    Completion {
        #[arg(value_enum)]
        shell: clap_complete::Shell,
    },
}

fn main() -> Result<()> {
    CompleteEnv::with_factory(Cli::command)
        .bin("kctx")
        .complete();

    run_async_main()
}

#[tokio::main]
async fn run_async_main() -> Result<()> {
    let args = Cli::parse();
    let (mut config, config_path) = load_kube_config()?;
    let current_context_name = config
        .current_context
        .as_deref()
        .unwrap_or("none")
        .to_string();

    match &args.command {
        Commands::ChangeContext { context_name } => {
            let ctxs: Vec<String> = config.contexts.iter().map(|c| c.name.clone()).collect();

            let target_context = match context_name {
                Some(name) => {
                    if !ctxs.contains(name) {
                        anyhow::bail!("the context {name} does not exist in the kubeconfig.");
                    }

                    name.clone()
                }
                None => {
                    let default_index = ctxs
                        .iter()
                        .position(|c| c == &current_context_name)
                        .unwrap_or(0);

                    let selection = FuzzySelect::with_theme(&ColorfulTheme::default())
                        .with_prompt("Select kubernetes context")
                        .default(default_index)
                        .items(&ctxs)
                        .interact_opt()
                        .context("failed to read user selection")?;

                    match selection {
                        Some(index) => ctxs[index].clone(),
                        None => return Ok(()),
                    }
                }
            };

            config.current_context = Some(target_context.clone());
            save_kube_config(&config, &config_path)?;

            let prompt = format!("Switched context {:?}.", target_context);
            println!("{} {}", style("✔").green(), prompt,);
        }

        Commands::Delete { context_name } => {
            let ctxs: Vec<String> = config.contexts.iter().map(|c| c.name.clone()).collect();
            let target_delete_context = match context_name {
                Some(name) => {
                    if !ctxs.contains(name) {
                        anyhow::bail!("the context {name} does not exist in the kubeconfig.");
                    }

                    name.clone()
                }
                None => {
                    let default_index = ctxs
                        .iter()
                        .position(|c| c == &current_context_name)
                        .unwrap_or(0);

                    let selection = FuzzySelect::with_theme(&ColorfulTheme::default())
                        .with_prompt("Delete kubernetes context")
                        .default(default_index)
                        .items(&ctxs)
                        .interact_opt()
                        .context("failed to read user selection")?;

                    match selection {
                        Some(index) => ctxs[index].clone(),
                        None => {
                            return Ok(());
                        }
                    }
                }
            };

            let prompt = format!(
                "Are you sure you want to delete context '{}'?",
                target_delete_context
            );

            let confirmed = Confirm::with_theme(&ColorfulTheme::default())
                .with_prompt(prompt.as_str())
                .default(false)
                .report(false)
                .interact()
                .context("Failed to read confirmation")?;

            if confirmed {
                Term::stdout().clear_last_lines(1)?;
                println!(
                    "{} {} · {}",
                    style("✔").green(),
                    prompt,
                    style("yes").green()
                );

                config.contexts.retain(|c| c.name != target_delete_context);
                if current_context_name == target_delete_context {
                    config.current_context = None;
                }

                save_kube_config(&config, &config_path)?;
                println!("Deleted context {:?}", target_delete_context);
            } else {
                Term::stdout().clear_last_lines(1)?;
                println!("{} {} · {}", style("✘").red(), prompt, style("no").red());
            }
        }

        Commands::ChangeNamespace { namespace_name } => {
            if current_context_name == "none" {
                anyhow::bail!("No active context set, cannot change namespace");
            }

            let ctx = get_mut_context(&mut config, &current_context_name)?;
            let current_namespace = ctx.namespace.clone().unwrap_or("default".to_string());
            let client = Client::try_default()
                .await
                .context("Failed to connect to Kubernetes cluster")?;

            let api_client: Api<Namespace> = Api::all(client);

            let target_namespace = match namespace_name {
                Some(name) => {
                    if api_client.get(&name).await.is_err() {
                        anyhow::bail!("Namespace {name} does not exist.");
                    }
                    name.clone()
                }
                None => {
                    let ns_list = api_client.list(&ListParams::default()).await?;

                    let ns_names: Vec<String> = ns_list
                        .items
                        .into_iter()
                        .filter_map(|ns| ns.metadata.name)
                        .collect();

                    if ns_names.is_empty() {
                        anyhow::bail!("No namespaces found in the cluster.");
                    }

                    let default_index = ns_names
                        .iter()
                        .position(|n| n == &current_namespace)
                        .unwrap_or(0);

                    let selection = FuzzySelect::with_theme(&ColorfulTheme::default())
                        .with_prompt("Select namespace")
                        .default(default_index)
                        .items(&ns_names)
                        .interact_opt()
                        .context("failed to read user selection")?;

                    match selection {
                        Some(index) => ns_names[index].clone(),
                        None => return Ok(()),
                    }
                }
            };

            ctx.namespace = Some(target_namespace.clone());
            save_kube_config(&config, &config_path)?;

            let prompt1 = format!("Context {:?} modified.", current_context_name);
            let prompt2 = format!("Active namespace is {:?}", target_namespace);
            println!("{} {}", style("✔").green(), prompt1,);
            println!("{} {}", style("✔").green(), prompt2,);
        }

        Commands::Info {} => {
            let current_namespace = current_namespace(config, current_context_name.clone())?;
            let prompt_context = format!("Current context: {:?}", current_context_name);
            let prompt_namespace = format!("Current namespace: {:?}", current_namespace);
            println!("{} {}", style("›").cyan(), prompt_context);
            println!("{} {}", style("›").cyan(), prompt_namespace);
        }

        Commands::Completion { shell } => {
            unsafe {
                std::env::set_var("COMPLETE", shell.to_string());
            }

            CompleteEnv::with_factory(Cli::command)
                .bin("kctx")
                .complete();
        }
    }

    Ok(())
}

fn load_kube_config() -> Result<(Kubeconfig, PathBuf)> {
    let home_dir = dirs::home_dir().context("Could not find home directory")?;
    let config_path = home_dir.join(".kube").join("config");
    let yaml_content = std::fs::read_to_string(&config_path)
        .context(format!("Failed to read config file at {:?}", config_path))?;
    let config = Kubeconfig::from_yaml(&yaml_content).context("failed to parse kubeconfig yaml")?;

    Ok((config, config_path))
}

fn save_kube_config(config: &Kubeconfig, path: &PathBuf) -> Result<()> {
    let yaml_content =
        serde_yaml::to_string(config).context("failed to serialize kubeconfig to yaml")?;
    std::fs::write(path, yaml_content)
        .context(format!("failed to write to config file at {:?}", path))?;

    Ok(())
}

fn current_namespace(config: Kubeconfig, name: String) -> Result<String> {
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

/// Helper to get a mutable reference to the active Kubernetes context
fn get_mut_context<'a>(
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

fn complete_namespaces(current: &std::ffi::OsStr) -> Vec<CompletionCandidate> {
    let rt = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(_) => return vec![],
    };

    rt.block_on(async {
        let client = match Client::try_default().await {
            Ok(client) => client,
            Err(_) => return vec![],
        };
        let client_api: Api<Namespace> = Api::all(client);
        let ns_list = match client_api.list(&ListParams::default()).await {
            Ok(list) => list,
            Err(_) => return vec![],
        };

        let current_str = current.to_string_lossy().to_lowercase();
        ns_list
            .items
            .into_iter()
            .filter_map(|ns| ns.metadata.name)
            .filter(|name| name.to_lowercase().contains(&current_str))
            // FIX APPLIED HERE for the Option<StyledStr> requirement:
            .map(|name| CompletionCandidate::new(name).help(Some("Namespace".into())))
            .collect()
    })
}
