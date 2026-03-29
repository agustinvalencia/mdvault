use clap::Args;
use clap_complete::Shell;

#[derive(Debug, Args)]
#[command(after_help = "\
Examples:
  mdv completions bash > ~/.local/share/bash-completion/completions/mdv
  mdv completions zsh > ~/.zfunc/_mdv
  mdv completions fish > ~/.config/fish/completions/mdv.fish
")]
pub struct CompletionsArgs {
    /// Shell to generate completions for
    #[arg(value_enum)]
    pub shell: Shell,
}
