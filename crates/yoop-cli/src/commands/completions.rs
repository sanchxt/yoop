//! Shell completions generation and installation.

use std::io;
use std::path::{Path, PathBuf};
use std::{env, fs};

use anyhow::{bail, Context, Result};
use clap::CommandFactory;
use clap_complete::{generate, Shell};

use super::{Cli, CompletionsAction, ShellType};

impl From<ShellType> for Shell {
    fn from(shell: ShellType) -> Self {
        match shell {
            ShellType::Bash => Self::Bash,
            ShellType::Zsh => Self::Zsh,
            ShellType::Fish => Self::Fish,
            ShellType::PowerShell => Self::PowerShell,
            ShellType::Elvish => Self::Elvish,
        }
    }
}

/// Run the completions command.
pub fn run(action: CompletionsAction) -> Result<()> {
    match action {
        CompletionsAction::Install { shell } => install(shell),
        CompletionsAction::Uninstall { shell } => uninstall(shell),
        CompletionsAction::Generate { shell } => {
            generate_to_stdout(shell);
            Ok(())
        }
    }
}

/// Generate completions and print to stdout.
fn generate_to_stdout(shell: ShellType) {
    let mut cmd = Cli::command();
    generate(Shell::from(shell), &mut cmd, "yoop", &mut io::stdout());
}

/// Generate completions as a string.
fn generate_completions(shell: ShellType) -> String {
    let mut cmd = Cli::command();
    let mut buf = Vec::new();
    generate(Shell::from(shell), &mut cmd, "yoop", &mut buf);
    String::from_utf8(buf).expect("completions should be valid UTF-8")
}

/// Detect the user's shell from environment.
fn detect_shell() -> Result<ShellType> {
    let shell_path = env::var("SHELL").context(
        "Could not detect shell from $SHELL environment variable.\n\
         Use --shell to specify your shell manually.",
    )?;

    let shell_name = shell_path
        .rsplit('/')
        .next()
        .unwrap_or(&shell_path)
        .to_lowercase();

    match shell_name.as_str() {
        "bash" => Ok(ShellType::Bash),
        "zsh" => Ok(ShellType::Zsh),
        "fish" => Ok(ShellType::Fish),
        "pwsh" | "powershell" => Ok(ShellType::PowerShell),
        "elvish" => Ok(ShellType::Elvish),
        other => bail!(
            "Unknown shell: {other}\n\
             Supported shells: bash, zsh, fish, powershell, elvish\n\
             Use --shell to specify your shell manually."
        ),
    }
}

/// Get the home directory.
fn home_dir() -> Result<PathBuf> {
    dirs_path().context("Could not determine home directory")
}

fn dirs_path() -> Option<PathBuf> {
    env::var_os("HOME")
        .filter(|h| !h.is_empty())
        .map(PathBuf::from)
}

/// Get the completions file path for a shell.
fn get_completions_path(shell: ShellType) -> Result<PathBuf> {
    let home = home_dir()?;

    let path = match shell {
        ShellType::Bash => {
            let xdg_data =
                env::var("XDG_DATA_HOME").map_or_else(|_| home.join(".local/share"), PathBuf::from);
            xdg_data.join("bash-completion/completions/yoop")
        }
        ShellType::Zsh => {
            let xdg_data =
                env::var("XDG_DATA_HOME").map_or_else(|_| home.join(".local/share"), PathBuf::from);
            xdg_data.join("zsh/site-functions/_yoop")
        }
        ShellType::Fish => {
            let xdg_config =
                env::var("XDG_CONFIG_HOME").map_or_else(|_| home.join(".config"), PathBuf::from);
            xdg_config.join("fish/completions/yoop.fish")
        }
        ShellType::PowerShell => {
            let documents = home.join("Documents");
            if cfg!(windows) {
                documents.join("PowerShell/Modules/YoopCompletion/YoopCompletion.psm1")
            } else {
                home.join(".config/powershell/Microsoft.PowerShell_profile.d/yoop.ps1")
            }
        }
        ShellType::Elvish => home.join(".elvish/lib/yoop.elv"),
    };

    Ok(path)
}

/// Install completions for the detected or specified shell.
fn install(shell_override: Option<ShellType>) -> Result<()> {
    let shell = match shell_override {
        Some(s) => s,
        None => detect_shell()?,
    };

    let path = get_completions_path(shell)?;
    let completions = generate_completions(shell);

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create directory: {}", parent.display()))?;
    }

    fs::write(&path, &completions)
        .with_context(|| format!("Failed to write completions to: {}", path.display()))?;

    println!("✓ Installed {shell:?} completions to: {}", path.display());
    print_post_install_instructions(shell, &path);

    Ok(())
}

/// Uninstall completions for the detected or specified shell.
fn uninstall(shell_override: Option<ShellType>) -> Result<()> {
    let shell = match shell_override {
        Some(s) => s,
        None => detect_shell()?,
    };

    let path = get_completions_path(shell)?;

    if path.exists() {
        fs::remove_file(&path).with_context(|| format!("Failed to remove: {}", path.display()))?;
        println!("✓ Removed {shell:?} completions from: {}", path.display());
    } else {
        println!("No completions file found at: {}", path.display());
    }

    Ok(())
}

/// Print shell-specific post-installation instructions.
fn print_post_install_instructions(shell: ShellType, path: &Path) {
    println!();
    match shell {
        ShellType::Bash => {
            println!("To enable completions, restart your shell or run:");
            println!("  source {}", path.display());
        }
        ShellType::Zsh => {
            println!("To enable completions, add this to your ~/.zshrc (if not already present):");
            println!("  fpath=(~/.local/share/zsh/site-functions $fpath)");
            println!("  autoload -Uz compinit && compinit");
            println!();
            println!("Then restart your shell or run: exec zsh");
        }
        ShellType::Fish => {
            println!("Completions will be available in new shell sessions.");
            println!("Or run: source {}", path.display());
        }
        ShellType::PowerShell => {
            println!("To enable completions, add this to your PowerShell profile:");
            println!("  Import-Module {}", path.display());
            println!();
            println!("Then restart PowerShell.");
        }
        ShellType::Elvish => {
            println!("To enable completions, add this to your ~/.elvish/rc.elv:");
            println!("  use yoop");
            println!();
            println!("Then restart Elvish.");
        }
    }
}
