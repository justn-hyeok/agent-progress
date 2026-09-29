use agent_progress::{
    dashboard, herdr,
    model::{self, Evidence, Plan, Session, Status, Task, Verification},
    store, ui,
};
use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::{path::PathBuf, process::ExitCode};
use uuid::Uuid;

#[derive(Parser)]
#[command(
    version,
    about = "공유 작업 계획 CLI/HUD · 체크율은 제품 완성도가 아닙니다"
)]
struct Cli {
    /// 계획 파일을 명시적으로 선택합니다. 최신 파일을 자동 선택하지 않습니다.
    #[arg(long, short, global = true)]
    file: Option<PathBuf>,
    /// 기록 주체의 이름. 본인 인증을 의미하지 않습니다.
    #[arg(long, global = true, default_value = "local")]
    actor: String,
    /// 읽었던 revision과 다르면 쓰기를 거부합니다.
    #[arg(long, global = true)]
    expect_revision: Option<u64>,
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Check native connection interfaces; --live also makes one authenticated plan-only request per agent.
    Compatibility {
        #[arg(long, value_parser=["codex","claude","opencode"])]
        agent: Vec<String>,
        #[arg(long)]
        live: bool,
        /// Explicit native model for one selected agent; uses that agent's existing authentication.
        #[arg(long, requires = "live")]
        model: Option<String>,
    },
    /// Optional shell integration for typing codex directly; backs up and preserves rc content.
    Shell {
        #[arg(value_parser=["preview","status","install","remove"],default_value="preview")]
        action: String,
        #[arg(long)]
        rc: Option<PathBuf>,
        #[arg(long, value_parser=["zsh","bash","fish"], default_value="zsh")]
        shell: String,
    },
    /// Project-local colors, placement and automatic Herdr observation.
    Config {
        #[command(subcommand)]
        action: PresentationAction,
    },
    /// Start an existing native CLI; observe Codex's frontend connection without disabling its shared server.
    Launch {
        #[arg(long,value_parser=["codex","claude","opencode"],default_value="codex")]
        agent: String,
        #[arg(last = true)]
        args: Vec<String>,
    },
    #[command(hide = true)]
    TerminalSource {
        #[arg(long)]
        slot: PathBuf,
    },
    #[command(hide = true)]
    TerminalFollow {
        #[arg(long)]
        slot: PathBuf,
        #[arg(long)]
        once: bool,
    },
    #[command(hide = true)]
    CodexClient {
        #[arg(long)]
        socket: PathBuf,
        #[arg(long)]
        backend: PathBuf,
        #[arg(long)]
        pane: String,
        #[arg(long)]
        owner: u32,
        #[arg(long)]
        root: PathBuf,
    },
    /// Native hook adapter. Reads a bounded JSON event on stdin; never injects model context.
    Bridge {
        #[arg(long,value_parser=["codex","claude","opencode"])]
        agent: String,
        #[arg(long)]
        root: Option<PathBuf>,
    },
    /// Passive project-local hooks; optional product MCP when ap.project.json exists. Preview reports management state.
    Connect {
        #[arg(value_parser=["preview","apply","remove"],default_value="preview")]
        action: String,
        #[arg(long)]
        allow_writes: bool,
        #[arg(long,value_parser=["codex","claude","opencode"],default_value="codex")]
        agent: String,
    },
    /// Import a portable product bundle into an explicitly named new directory.
    Import {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        into: PathBuf,
    },
    /// Browse explicitly selected saved products; does not reconnect live agents.
    Projects {
        #[arg(long,required=true,num_args=1..)]
        project: Vec<PathBuf>,
    },
    /// Read-only diagnostics; never selects another source or changes settings.
    Doctor {
        #[arg(long)]
        project: Option<PathBuf>,
        #[arg(long, conflicts_with = "rollout")]
        pane: Option<String>,
        #[arg(long)]
        rollout: Option<PathBuf>,
        /// Check the selected agent as required; other installed harnesses are optional.
        #[arg(long,value_parser=["codex","claude","opencode"])]
        agent: Option<String>,
        #[arg(long,value_parser=["zsh","bash","fish"])]
        shell: Option<String>,
        /// Read-only check of one shell file instead of default user setup.
        #[arg(long)]
        rc: Option<PathBuf>,
        /// Inspect this exact owned terminal slot; never select a newest launch.
        #[arg(long)]
        terminal_slot: Option<PathBuf>,
        /// Exit nonzero for unhealthy requested checks. Default remains report-only.
        #[arg(long)]
        strict: bool,
    },
    /// Local stdio MCP. Read-only unless writes are explicitly enabled.
    Mcp {
        #[arg(long)]
        project: Option<PathBuf>,
        #[arg(long)]
        allow_writes: bool,
    },
    /// 제품 전체 계획 조회·명시적 상태/근거 갱신. 항목 등록은 원본 계획에서 자동으로 합니다.
    Product {
        #[arg(long)]
        project: Option<PathBuf>,
        #[command(subcommand)]
        action: ProductAction,
    },
    /// 기존 에이전트 세션을 별도 진행 상황 창에서 자동으로 따라갑니다.
    Follow {
        #[arg(long, conflicts_with = "rollout")]
        pane: Option<String>,
        #[arg(long)]
        session: Option<Uuid>,
        #[arg(long)]
        rollout: Option<PathBuf>,
        #[arg(long)]
        codex_home: Option<PathBuf>,
        #[arg(long)]
        cache: Option<PathBuf>,
        #[arg(long)]
        project: Option<PathBuf>,
        #[arg(long)]
        once: bool,
    },
    /// 현재 에이전트 pane 아래에 진행 상황 창을 엽니다.
    Open {
        #[arg(long)]
        pane: Option<String>,
        #[arg(long)]
        reconnect: bool,
        #[arg(long, value_enum)]
        position: Option<agent_progress::settings::Position>,
    },
    Init {
        #[arg(long)]
        project: String,
        #[arg(long)]
        goal: String,
    },
    Add {
        title: String,
        #[arg(long, required = true)]
        criterion: Vec<String>,
        #[arg(long, value_enum, default_value = "reported")]
        require: Verification,
        #[arg(long)]
        depends_on: Vec<Uuid>,
        #[arg(long)]
        reason: String,
    },
    Status {
        id: Uuid,
        #[arg(value_enum)]
        status: Status,
        #[arg(long)]
        reason: String,
        #[arg(long)]
        next: Option<String>,
    },
    /// Record agent-reported status using the same evidence policy as live projection.
    Report {
        id: Uuid,
        #[arg(value_enum)]
        status: Status,
        #[arg(long)]
        reason: String,
        #[arg(long)]
        next: Option<String>,
    },
    Evidence {
        id: Uuid,
        #[arg(value_enum)]
        kind: Verification,
        reference: String,
    },
    /// 근거를 오래됨으로 표시하고 완료 작업을 검증 대기로 되돌립니다.
    Invalidate {
        id: Uuid,
        #[arg(long)]
        reason: String,
    },
    Bind {
        id: Uuid,
        #[arg(long)]
        agent: String,
        #[arg(long)]
        session: String,
    },
    Note {
        id: Uuid,
        text: String,
        #[arg(long)]
        next: Option<String>,
    },
    Show {
        #[arg(long)]
        json: bool,
    },
    Validate,
    History,
    /// 읽기 전용 HUD. q 또는 Ctrl-C로 종료합니다.
    Watch,
}

#[derive(Subcommand)]
enum ProductAction {
    Show,
    /// Plain text recovery summary, suitable for terminal text readers.
    Summary,
    Revision {
        key: String,
    },
    Resume,
    Backup {
        #[arg(long)]
        output: PathBuf,
    },
    Export {
        #[arg(long)]
        output: PathBuf,
    },
    Restore {
        #[arg(long)]
        backup: PathBuf,
        #[arg(long)]
        expect_hash: Option<String>,
    },
    Status {
        key: String,
        #[arg(value_enum)]
        status: Status,
        #[arg(long)]
        reason: String,
    },
    Evidence {
        key: String,
        #[arg(value_enum)]
        kind: Verification,
        reference: String,
        #[arg(long)]
        revision: Option<String>,
    },
    Invalidate {
        key: String,
        #[arg(long)]
        reason: String,
    },
    Note {
        key: String,
        text: String,
        #[arg(long)]
        next: Option<String>,
    },
    /// Declare split/merge between existing stable IDs; never inherit completion.
    Replace {
        #[arg(long, required = true, num_args = 1..)]
        from: Vec<String>,
        #[arg(long, required = true, num_args = 1..)]
        into: Vec<String>,
        #[arg(long)]
        reason: String,
    },
}

#[derive(Subcommand)]
enum PresentationAction {
    Show,
    Reset,
    /// Create ui.yaml from current settings without replacing an existing YAML file.
    InitYaml,
    /// List built-in and user-defined palette names.
    Presets,
    Set(Box<PresentationOptions>),
}

#[derive(clap::Args)]
struct PresentationOptions {
    #[arg(long)]
    preset: Option<String>,
    #[arg(long)]
    brightness: Option<f64>,
    #[arg(long, value_enum)]
    position: Option<agent_progress::settings::Position>,
    #[arg(long)]
    auto_open: Option<bool>,
    #[arg(long)]
    track: Option<String>,
    #[arg(long)]
    fill: Option<String>,
    #[arg(long)]
    accent: Option<String>,
    #[arg(long)]
    text: Option<String>,
    #[arg(long)]
    muted: Option<String>,
    #[arg(long)]
    metadata: Option<String>,
    #[arg(long)]
    warning: Option<String>,
}

fn run(cli: Cli) -> Result<()> {
    let command = cli.command.unwrap_or(Command::Follow {
        pane: None,
        session: None,
        rollout: None,
        codex_home: None,
        cache: None,
        project: None,
        once: false,
    });
    let command = match command {
        Command::Compatibility { agent, live, model } => {
            let agents = if agent.is_empty() {
                vec!["codex".into(), "claude".into(), "opencode".into()]
            } else {
                agent
            };
            anyhow::ensure!(
                model.is_none() || agents.len() == 1,
                "--model requires one --agent"
            );
            let report =
                agent_progress::compatibility::report_model(&agents, live, model.as_deref());
            println!("{}", serde_json::to_string_pretty(&report)?);
            anyhow::ensure!(
                report["passed"] == true,
                "native compatibility check failed"
            );
            return Ok(());
        }
        Command::Shell { action, rc, shell } => {
            println!(
                "{}",
                serde_json::to_string_pretty(&agent_progress::shell::manage_selected(
                    &action,
                    &shell,
                    rc.as_deref()
                )?)?
            );
            return Ok(());
        }
        Command::Config { action } => {
            let cwd = std::env::current_dir()?;
            let project = agent_progress::project::Project::discover(&cwd)?;
            let root = project.as_ref().map(|p| p.root()).unwrap_or(&cwd);
            let mut settings = if matches!(&action, PresentationAction::Reset) {
                Default::default()
            } else {
                agent_progress::settings::load(root)?
            };
            let mut changed = true;
            match action {
                PresentationAction::Show => changed = false,
                PresentationAction::InitYaml => {
                    agent_progress::settings::init_yaml(root)?;
                    println!("{}", agent_progress::settings::yaml_path(root).display());
                    return Ok(());
                }
                PresentationAction::Presets => {
                    println!(
                        "{}",
                        serde_json::json!({"built_in":["signal","forest","ocean","amber"],"custom":settings.presets.keys().collect::<Vec<_>>()})
                    );
                    return Ok(());
                }
                PresentationAction::Reset => settings = Default::default(),
                PresentationAction::Set(options) => {
                    let PresentationOptions {
                        preset,
                        brightness,
                        position,
                        auto_open,
                        track,
                        fill,
                        accent,
                        text,
                        muted,
                        metadata,
                        warning,
                    } = *options;
                    if let Some(preset) = preset {
                        settings.preset = preset;
                        settings.theme = Default::default();
                    }
                    if let Some(value) = brightness {
                        settings.background_brightness = value;
                    }
                    if let Some(value) = position {
                        settings.position = value;
                    }
                    if let Some(value) = auto_open {
                        settings.auto_open = value;
                    }
                    for (target, value) in [
                        (&mut settings.theme.track, track),
                        (&mut settings.theme.fill, fill),
                        (&mut settings.theme.accent, accent),
                        (&mut settings.theme.text, text),
                        (&mut settings.theme.muted, muted),
                        (&mut settings.theme.metadata, metadata),
                        (&mut settings.theme.warning, warning),
                    ] {
                        if let Some(value) = value {
                            agent_progress::settings::color(&value)?;
                            *target = Some(value.to_ascii_uppercase());
                        }
                    }
                }
            }
            if changed {
                agent_progress::settings::save(root, &settings)?;
            }
            println!("{}", serde_json::to_string_pretty(&settings)?);
            return Ok(());
        }
        Command::Launch { agent, args } => {
            use std::io::IsTerminal;
            use std::os::unix::process::CommandExt;
            agent_progress::terminal::claim()?;
            if agent_progress::terminal::interactive_args(&agent, &args)
                && (std::env::var("HERDR_ENV").as_deref() == Ok("1")
                    || std::env::var_os("AP_TERMINAL_SLOT").is_some()
                    || std::io::stdin().is_terminal())
            {
                agent_progress::compatibility::on_launch(&agent);
            }
            if agent_progress::terminal::maybe_launch(&agent, &args)? {
                return Ok(());
            }
            if agent == "codex" {
                return agent_progress::codex_client::launch(args);
            }
            let mut command = std::process::Command::new(&agent);
            if std::env::var_os("AP_AUTO_OPEN").is_none() {
                command.env("AP_AUTO_OPEN", "1");
            }
            command.args(args);
            return Err(command.exec().into());
        }
        Command::TerminalSource { slot } => return agent_progress::terminal::source(&slot),
        Command::TerminalFollow { slot, once } => {
            return agent_progress::terminal::follow(&slot, once);
        }
        Command::CodexClient {
            socket,
            backend,
            pane,
            owner,
            root,
        } => {
            agent_progress::codex_client::serve(&socket, &backend, &pane, owner, &root)?;
            return Ok(());
        }
        Command::Bridge { agent, root } => {
            let event = agent_progress::bridge::read_input(std::io::stdin().lock())?;
            let registered = agent_progress::bridge::ingest(&agent, &event, root.as_deref())?;
            let source_root = event["cwd"]
                .as_str()
                .map(PathBuf::from)
                .context("hook cwd missing")?;
            let project = agent_progress::project::Project::discover(&source_root)?;
            let settings = agent_progress::settings::load(
                project.as_ref().map(|p| p.root()).unwrap_or(&source_root),
            )
            .unwrap_or_else(|_| {
                eprintln!("progress presentation unavailable: invalid settings; hook continues");
                agent_progress::settings::Settings {
                    auto_open: false,
                    ..Default::default()
                }
            });
            if settings.auto_open
                && std::env::var("AP_AUTO_OPEN").as_deref() != Ok("0")
                && registered["registered_pane"] == true
            {
                // Launcher opt-in: open only a read-only observer after exact source ownership was proved.
                // Diagnostics stay local; observing must never block or steer an agent turn.
                if let Err(error) = herdr::open_quiet(std::env::var("HERDR_PANE_ID").ok(), true) {
                    eprintln!("progress window unavailable: {error}");
                }
            }
            println!(
                "{}",
                serde_json::json!({"continue":true,"suppressOutput":true})
            );
            return Ok(());
        }
        Command::Connect {
            action,
            allow_writes,
            agent,
        } => {
            println!(
                "{}",
                serde_json::to_string_pretty(&agent_progress::connection::manage_agent(
                    &std::env::current_dir()?,
                    &action,
                    allow_writes,
                    &agent
                )?)?
            );
            return Ok(());
        }
        Command::Import { input, into } => {
            let p = agent_progress::project::Project::import_bundle(&input, &into)?;
            println!("{}", serde_json::to_string_pretty(&p.resume()?)?);
            return Ok(());
        }
        Command::Projects { project } => return dashboard::watch_projects(project),
        Command::Doctor {
            project,
            pane,
            rollout,
            agent,
            shell,
            rc,
            terminal_slot,
            strict,
        } => {
            let report = agent_progress::doctor::inspect_options(agent_progress::doctor::Options {
                project,
                pane,
                rollout,
                agent,
                shell,
                rc,
                terminal_slot,
            });
            println!("{}", serde_json::to_string_pretty(&report)?);
            anyhow::ensure!(
                !strict || report["healthy"] == true,
                "doctor found unhealthy requested checks"
            );
            return Ok(());
        }
        Command::Mcp {
            project,
            allow_writes,
        } => {
            let p = if let Some(path) = project {
                agent_progress::project::Project::open(&path)?
            } else {
                agent_progress::project::Project::discover(&std::env::current_dir()?)?
                    .context("no ap.project.json")?
            };
            return agent_progress::mcp::serve(
                p,
                allow_writes,
                std::io::stdin().lock(),
                std::io::stdout().lock(),
            );
        }
        Command::Product { project, action } => {
            anyhow::ensure!(
                cli.file.is_none() && cli.expect_revision.is_none(),
                "--file/--expect-revision are file-mode options; use --project for product commands"
            );
            let p = if let Some(path) = project {
                agent_progress::project::Project::open(&path)?
            } else {
                agent_progress::project::Project::discover(&std::env::current_dir()?)?
                    .context("no ap.project.json")?
            };
            let plan = match action {
                ProductAction::Revision { key } => {
                    println!(
                        "{}",
                        serde_json::json!({"key":key,"code_revision":p.code_revision(&key)?})
                    );
                    return Ok(());
                }
                ProductAction::Summary => {
                    let summary = p.resume()?;
                    println!("목표: {}", summary["goal"].as_str().unwrap_or("미정"));
                    let counts = &summary["counts"];
                    println!(
                        "체크 항목: {} / {} 완료 (제품 완성도나 남은 시간이 아님)",
                        counts[0], counts[1]
                    );
                    if let Some(last) = summary["last_completed"]["title"].as_str() {
                        println!("마지막 완료: {last}");
                    }
                    if let Some(blocked) = summary["blocked"].as_array() {
                        for task in blocked {
                            println!(
                                "막힘: {} — {}. 필요한 행동: {}",
                                task["title"].as_str().unwrap_or(""),
                                task["blocker"].as_str().unwrap_or(""),
                                task["next_action"].as_str().unwrap_or("")
                            );
                        }
                    }
                    if let Some(next) = summary["next"]["title"].as_str() {
                        println!("다음 작업: {next}");
                    }
                    println!("저장 위치: {}", p.state_path().display());
                    return Ok(());
                }
                ProductAction::Resume => {
                    println!("{}", serde_json::to_string_pretty(&p.resume()?)?);
                    return Ok(());
                }
                ProductAction::Backup { output } => {
                    p.backup(&output)?;
                    println!("{}", output.display());
                    return Ok(());
                }
                ProductAction::Export { output } => {
                    p.export(&output)?;
                    println!("{}", output.display());
                    return Ok(());
                }
                ProductAction::Restore {
                    backup,
                    expect_hash,
                } => {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&p.restore(
                            &backup,
                            expect_hash.as_deref(),
                            &cli.actor
                        )?)?
                    );
                    return Ok(());
                }
                ProductAction::Show => p.plan()?,
                ProductAction::Status {
                    key,
                    status,
                    reason,
                } => p.status(&key, status, &cli.actor, &reason)?,
                ProductAction::Evidence {
                    key,
                    kind,
                    reference,
                    revision,
                } => p.evidence_at(&key, kind, &cli.actor, &reference, revision.as_deref())?,
                ProductAction::Invalidate { key, reason } => {
                    p.invalidate(&key, &cli.actor, &reason)?
                }
                ProductAction::Note { key, text, next } => p.note(&key, &cli.actor, &text, next)?,
                ProductAction::Replace { from, into, reason } => {
                    p.replace(&from, &into, &cli.actor, &reason)?
                }
            };
            println!("{}", serde_json::to_string_pretty(&plan)?);
            return Ok(());
        }
        Command::Open {
            pane,
            reconnect,
            position,
        } => return herdr::open_position(pane, reconnect, position),
        Command::Follow {
            pane,
            session,
            rollout,
            codex_home,
            cache,
            project,
            once,
        } => {
            return dashboard::follow(dashboard::Options {
                pane,
                session,
                rollout,
                home: codex_home,
                once,
                cache,
                project,
            });
        }
        command => command,
    };
    let file = cli
        .file
        .as_deref()
        .context("--file is required; select a plan explicitly")?;
    let actor = &cli.actor;
    match command {
        Command::Init { project, goal } => {
            let plan = Plan::new(project, goal, actor)?;
            store::create(file, &plan)?;
            println!("{} revision={}", plan.id, plan.revision);
        }
        Command::Show { json } => {
            let plan = store::read(file)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&plan)?);
            } else {
                println!("{}", ui::summary(&plan));
            }
        }
        Command::Validate => {
            let plan = store::read(file)?;
            println!(
                "valid schema={} revision={} tasks={}",
                plan.schema_version,
                plan.revision,
                plan.tasks.len()
            );
        }
        Command::History => {
            for event in store::read(file)?.history {
                println!(
                    "r{} {} [{}] {} {} — {}",
                    event.revision,
                    event.at,
                    event.actor,
                    event.action,
                    event.task.map(|id| id.to_string()).unwrap_or_default(),
                    event.reason
                );
            }
        }
        Command::Watch => ui::watch(file)?,
        command => {
            let mut changed = None;
            let plan = store::update(file, cli.expect_revision, |plan| {
                let (action, id, reason) = match command {
                    Command::Add {
                        title,
                        criterion,
                        require,
                        depends_on,
                        reason,
                    } => {
                        let task = Task::new(title, criterion, require, depends_on);
                        let id = task.id;
                        plan.tasks.push(task);
                        ("add".to_owned(), id, reason)
                    }
                    Command::Status {
                        id,
                        status,
                        reason,
                        next,
                    } => {
                        plan.set_status(id, status, &reason, next)?;
                        (format!("status:{status:?}"), id, reason)
                    }
                    Command::Report {
                        id,
                        status,
                        reason,
                        next,
                    } => {
                        let task = plan.task_mut(id)?;
                        task.report(status, actor, &reason);
                        if status == Status::Blocked {
                            task.blocker = Some(reason.clone());
                        }
                        if let Some(next) = next {
                            task.next_action = next;
                        }
                        plan.hold_dependencies();
                        (format!("report:{status:?}"), id, reason)
                    }
                    Command::Evidence {
                        id,
                        kind,
                        reference,
                    } => {
                        model::nonempty(&reference, "evidence reference")?;
                        plan.task_mut(id)?.evidence.push(Evidence {
                            kind,
                            reference: reference.clone(),
                            actor: actor.clone(),
                            recorded_at: model::now(),
                            stale: false,
                            code_revision: None,
                        });
                        (format!("evidence:{kind:?}"), id, reference)
                    }
                    Command::Invalidate { id, reason } => {
                        // Include dependent tasks so completed work cannot retain invalid prerequisites.
                        plan.invalidate(id, &reason)?;
                        ("invalidate".into(), id, reason)
                    }
                    Command::Bind { id, agent, session } => {
                        plan.task_mut(id)?.session = Some(Session {
                            agent: agent.clone(),
                            id: session.clone(),
                        });
                        ("bind".into(), id, format!("{agent} / {session}"))
                    }
                    Command::Note { id, text, next } => {
                        let task = plan.task_mut(id)?;
                        if let Some(next) = next {
                            task.next_action = next;
                        }
                        task.notes.push(text.clone());
                        ("note".into(), id, text)
                    }
                    _ => unreachable!(),
                };
                changed = Some(id);
                plan.record(actor, &action, Some(id), &reason)
            })?;
            println!(
                "{} revision={}",
                changed.context("no changed task")?,
                plan.revision
            );
        }
    }
    Ok(())
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("ap: {error:#}");
            ExitCode::FAILURE
        }
    }
}
