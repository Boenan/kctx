use anyhow::Result;
use clap::{CommandFactory, Parser, Subcommand};
use clap_complete::env::CompleteEnv;
use kctx::{cmd, init, kube_config};

#[derive(Parser)]
#[command(name = "kctx")]
#[command(version)]
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
        #[arg(
            verbatim_doc_comment,
            add = clap_complete::engine::ArgValueCompleter::new(kube_config::complete_namespaces)
        )]
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
    init::init_crypto();

    CompleteEnv::with_factory(Cli::command)
        .bin("kctx")
        .complete();

    run_async_main()
}

#[tokio::main(flavor = "current_thread")]
async fn run_async_main() -> Result<()> {
    let args = Cli::parse();
    let (mut config, config_path) = kube_config::load_kube_config()?;
    let current_context_name = config
        .current_context
        .as_deref()
        .unwrap_or("none")
        .to_string();

    match args.command {
        Commands::ChangeContext { context_name } => {
            cmd::context::run(
                &mut config,
                &config_path,
                &current_context_name,
                context_name,
            )?;
        }

        Commands::Delete { context_name } => {
            cmd::delete::run(
                &mut config,
                &config_path,
                &current_context_name,
                context_name,
            )?;
        }

        Commands::ChangeNamespace { namespace_name } => {
            cmd::namespace::run(config, config_path, &current_context_name, namespace_name).await?;
        }

        Commands::Info {} => {
            cmd::info::run(config, current_context_name)?;
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
