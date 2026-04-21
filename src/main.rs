use anyhow::Result;
use clap::{CommandFactory, Parser};
use colored::Colorize;

use airis_workspace::cli::{
    ClaudeCommands, Cli, Commands, DepsCommands, DocsCommands, GenerateCommands, GuardsCommands,
    HooksCommands, ManifestCommands, NetworkCommands, NewCommands, PolicyCommands, ShimCommands,
    TestLevel, ValidateCommands, WorkspaceCommands,
};
use airis_workspace::commands;

/// Get version string with dev suffix for non-release builds
fn get_version() -> String {
    let version = env!("CARGO_PKG_VERSION");
    let is_release = env!("IS_RELEASE");
    let git_hash = env!("GIT_HASH");

    if is_release == "true" {
        version.to_string()
    } else {
        format!("{}-dev (git: {})", version, git_hash)
    }
}

fn main() {
    // 1. Start background update check
    commands::upgrade::spawn_check();

    // 2. Run CLI
    let result = run_main();

    // 3. Print update notification
    commands::upgrade::print_notification();

    if let Err(e) = result {
        eprintln!("{}: {:?}", "Error".red().bold(), e);
        std::process::exit(1);
    }
}

fn run_main() -> Result<()> {
    // Setup miette for fancy errors
    miette::set_panic_hook();

    let cli = Cli::parse();

    // Handle version flag
    if cli.version {
        println!("airis {}", get_version());
        return Ok(());
    }

    // Require a command if not printing version
    let command = cli.command.unwrap_or_else(|| {
        Cli::command().print_help().unwrap();
        std::process::exit(0);
    });

    dispatch(command)
}

/// Dispatch a parsed CLI command to the appropriate handler.
fn dispatch(command: Commands) -> Result<()> {
    // (Existing dispatch logic... no changes needed here yet as it returns anyhow::Result)
    match command {
        Commands::Manifest { action } => {
            use commands::manifest_cmd::{self, ManifestAction};

            let manifest_action = match action {
                ManifestCommands::DevApps => ManifestAction::DevApps,
                ManifestCommands::Rule { name } => ManifestAction::Rule { name },
                ManifestCommands::Json => ManifestAction::Json,
            };

            manifest_cmd::run(manifest_action)?;
        }
        Commands::Claude { action } => match action {
            ClaudeCommands::Setup => commands::claude_setup::setup_global()?,
            ClaudeCommands::Status => commands::claude_setup::status()?,
            ClaudeCommands::Uninstall => commands::claude_setup::uninstall()?,
        },
        Commands::Guards { action } => match action {
            GuardsCommands::Install {
                global,
                preset,
                hooks,
            } => {
                if hooks {
                    eprintln!(
                        "{}: `airis guards install --hooks` is deprecated. Use `airis claude setup` instead.",
                        "warning".yellow().bold()
                    );
                    commands::claude_setup::setup_global()?;
                } else if global {
                    commands::guards::install_global(preset)?;
                } else {
                    commands::guards::install()?;
                }
            }
            GuardsCommands::CheckDocker => commands::guards::check_docker()?,
            GuardsCommands::Status { global, hooks } => {
                if hooks {
                    eprintln!(
                        "{}: `airis guards status --hooks` is deprecated. Use `airis claude status` instead.",
                        "warning".yellow().bold()
                    );
                    commands::claude_setup::status()?;
                } else if global {
                    commands::guards::status_global()?;
                } else {
                    commands::guards::status()?;
                }
            }
            GuardsCommands::Uninstall { global, hooks } => {
                if hooks {
                    eprintln!(
                        "{}: `airis guards uninstall --hooks` is deprecated. Use `airis claude uninstall` instead.",
                        "warning".yellow().bold()
                    );
                    commands::claude_setup::uninstall()?;
                } else if global {
                    commands::guards::uninstall_global()?;
                } else {
                    commands::guards::uninstall()?;
                }
            }
            GuardsCommands::Verify => commands::guards::verify_global()?,
            GuardsCommands::CheckAllow { cmd } => {
                commands::guards::check_allow(&cmd)?;
            }
        },
        Commands::Workspace(args) => match args.action {
            WorkspaceCommands::Uninstall => commands::workspace::uninstall()?,
        },
        Commands::Hooks { action } => match action {
            HooksCommands::Install => commands::hooks::install()?,
            HooksCommands::Uninstall => commands::hooks::uninstall()?,
        },
        Commands::Shim { action } => match action {
            ShimCommands::Install => commands::shim::install()?,
            ShimCommands::List => commands::shim::list()?,
            ShimCommands::Uninstall => commands::shim::uninstall()?,
            ShimCommands::Exec { cmd, args } => commands::shim::exec(&cmd, &args)?,
        },
        Commands::Docs { action } => match action {
            DocsCommands::Wrap { target } => commands::docs::wrap(&target)?,
            DocsCommands::Sync => commands::docs::sync()?,
            DocsCommands::List => commands::docs::list()?,
        },
        Commands::Validate { action, json } => {
            use commands::validate_cmd::{self, ValidateAction};

            let validate_action = match action {
                ValidateCommands::Manifest => ValidateAction::Manifest,
                ValidateCommands::Ports => ValidateAction::Ports,
                ValidateCommands::Networks => ValidateAction::Networks,
                ValidateCommands::Env => ValidateAction::Env,
                ValidateCommands::Dependencies => ValidateAction::Dependencies,
                ValidateCommands::Architecture => ValidateAction::Architecture,
                ValidateCommands::All => ValidateAction::All,
            };

            validate_cmd::run(validate_action, json)?;
        }
        Commands::Verify => commands::verify::run()?,
        Commands::Doctor {
            fix,
            truth,
            truth_json,
        } => {
            if truth || truth_json {
                commands::doctor::run_truth(truth_json)?;
            } else {
                commands::doctor::run(fix)?;
            }
        }
        Commands::Run { task, extra_args } => commands::run::run(&task, &extra_args)?,
        Commands::Up { extra_args } => commands::run::run("up", &extra_args)?,
        Commands::Install { extra_args } => commands::install::run(&extra_args)?,
        Commands::Down { extra_args } => commands::run::run("down", &extra_args)?,
        Commands::Shell { extra_args } => commands::run::run("shell", &extra_args)?,
        Commands::Test {
            scan,
            level,
            coverage_check,
            min_coverage,
            extra_args,
        } => {
            if scan {
                commands::test_scan::run()?;
            } else if let Some(lvl) = level {
                let task = match lvl {
                    TestLevel::Unit => "test:unit",
                    TestLevel::Integration => "test:integration",
                    TestLevel::E2e => "test:e2e",
                    TestLevel::Smoke => "test:smoke",
                };
                commands::run::run(task, &extra_args)?;
            } else if coverage_check {
                commands::run::run_test_coverage(min_coverage)?;
            } else {
                commands::run::run("test", &extra_args)?;
            }
        }
        Commands::Build {
            project,
            affected,
            base,
            head,
            docker,
            channel,
            targets,
            parallel,
            image,
            push,
            context_out,
            no_cache,
            remote_cache,
            prod,
            quick,
        } => {
            let opts = commands::build::DockerBuildOpts {
                channel,
                targets,
                parallel,
                image,
                push,
                context_out,
                no_cache,
                remote_cache,
            };

            if affected && docker {
                commands::build::build_affected_docker(&base, &head, &opts)?;
            } else if docker {
                let target = project.ok_or_else(|| {
                    anyhow::anyhow!("--docker requires a project path (e.g., apps/web)")
                })?;
                commands::build::build_docker(&target, &opts)?;
            } else if prod {
                let app_name = project
                    .as_deref()
                    .ok_or_else(|| anyhow::anyhow!("--prod requires a project path"))?;
                commands::run::run_build_prod(app_name)?;
            } else if quick {
                let app_name = project
                    .as_deref()
                    .ok_or_else(|| anyhow::anyhow!("--quick requires a project path"))?;
                commands::run::run_build_quick(app_name)?;
            } else {
                commands::run::run("build", &[])?;
            }
        }
        Commands::Clean {
            dry_run,
            purge,
            force,
            allow_anywhere,
            extra_args: _,
        } => {
            // dry_run is true by default, force overrides it
            let actual_dry_run = if force { false } else { dry_run };
            commands::clean::run(actual_dry_run, purge, allow_anywhere)?;
        }
        Commands::Bundle {
            project,
            output,
            k8s,
        } => {
            commands::bundle::run(&project, output.as_deref(), k8s)?;
        }
        Commands::Lint { extra_args } => commands::run::run("lint", &extra_args)?,
        Commands::Format { extra_args } => commands::run::run("format", &extra_args)?,
        Commands::Typecheck { extra_args } => commands::run::run("typecheck", &extra_args)?,
        Commands::Ps { extra_args } => {
            if extra_args.is_empty() {
                commands::run::run_ps()?;
            } else {
                commands::run::run("ps", &extra_args)?;
            }
        }
        Commands::Logs {
            service,
            follow,
            tail,
        } => commands::run::run_logs(service.as_deref(), follow, tail)?,
        Commands::Exec { service, cmd } => commands::run::run_exec(&service, &cmd)?,
        Commands::Restart { service } => commands::run::run_restart(service.as_deref())?,
        Commands::Network { action } => match action {
            NetworkCommands::Init => commands::network::init()?,
            NetworkCommands::Setup => commands::network::setup()?,
            NetworkCommands::List => commands::network::list()?,
            NetworkCommands::Remove => commands::network::remove()?,
        },
        Commands::New { template } => match template {
            NewCommands::Api { name, runtime } => {
                commands::new_cmd::run_with_runtime("api", &name, &runtime)?;
            }
            NewCommands::Web { name, runtime } => {
                commands::new_cmd::run_with_runtime("web", &name, &runtime)?;
            }
            NewCommands::Lib { name, runtime } => {
                commands::new_cmd::run_with_runtime("lib", &name, &runtime)?;
            }
            NewCommands::Edge { name } => {
                commands::new_cmd::run_with_runtime("edge", &name, "deno")?;
            }
            NewCommands::SupabaseTrigger { name } => {
                commands::new_cmd::run_with_runtime("supabase-trigger", &name, "plpgsql")?;
            }
            NewCommands::SupabaseRealtime { name } => {
                commands::new_cmd::run_with_runtime("supabase-realtime", &name, "deno")?;
            }
        },
        Commands::Affected { base, head } => {
            commands::affected::run(&base, &head)?;
        }
        Commands::Gen {
            dry_run,
            force,
            migrate,
        } => {
            commands::generate::run(dry_run, force, migrate)?;
        }
        Commands::Generate { action } => match action {
            GenerateCommands::Types {
                host,
                port,
                database,
                output,
            } => {
                commands::generate_types::run(&host, &port, &database, &output)?;
            }
        },
        Commands::BumpVersion {
            major,
            minor,
            patch,
            auto: _,
        } => {
            use commands::bump_version::{self, BumpMode};

            let mode = if major {
                BumpMode::Major
            } else if minor {
                BumpMode::Minor
            } else if patch {
                BumpMode::Patch
            } else {
                // Default to auto
                BumpMode::Auto
            };

            bump_version::run(mode)?;
        }
        Commands::Policy { action } => match action {
            PolicyCommands::Init => commands::policy::init()?,
            PolicyCommands::Check { project } => {
                commands::policy::check(project.as_deref())?;
            }
            PolicyCommands::Enforce { project } => {
                commands::policy::enforce(project.as_deref())?;
            }
        },
        Commands::Deps { action } => match action {
            DepsCommands::Tree => commands::deps::tree()?,
            DepsCommands::Json => commands::deps::json()?,
            DepsCommands::Show { package } => commands::deps::show(&package)?,
            DepsCommands::Check => commands::deps::check()?,
        },
        Commands::Diff { json, stat } => {
            use commands::diff::DiffFormat;
            let format = if json {
                DiffFormat::Json
            } else if stat {
                DiffFormat::Stat
            } else {
                DiffFormat::Unified
            };
            commands::diff::run(format)?;
        }
        Commands::Upgrade { check, version } => {
            if check {
                // Background check already happened, this just triggers immediate check/report
                commands::upgrade::spawn_check();
                println!("Check complete. If an update is found, it will be shown below.");
            } else {
                commands::upgrade::run(version)?;
            }
        }

        Commands::Completion { shell } => {
            commands::completion::run(shell)?;
        }
        Commands::Mcp => {
            commands::mcp::run()?;
        }
    }

    Ok(())
}
