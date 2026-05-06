use anyhow::Result;

pub fn run(shell: clap_complete::Shell) -> Result<()> {
    match shell {
        clap_complete::Shell::Zsh => {
            println!(r#"
# airis shell integration (zsh)
# Add this to your ~/.zshrc: source <(airis init-shell zsh)

_airis_prompt_precmd() {{
    local airis_status=$(airis status --short 2>/dev/null)
    if [[ -n "$airis_status" ]]; then
        # Use RPROMPT to show airis status on the right side
        RPROMPT="$airis_status"
    else
        RPROMPT=""
    fi
}}

# Add to precmd hooks
autoload -Uz add-zsh-hook
add-zsh-hook precmd _airis_prompt_precmd
"#);
        }
        clap_complete::Shell::Bash => {
            println!(r#"
# airis shell integration (bash)
# Add this to your ~/.bashrc: source <(airis init-shell bash)

_airis_prompt_command() {{
    local airis_status=$(airis status --short 2>/dev/null)
    # Bash doesn't have RPROMPT easily, so we can append to PS1 or print above
    # For now, let's just make it available or suggest custom integration
}}

# PROMPT_COMMAND="...; _airis_prompt_command"
"#);
        }
        _ => {
            eprintln!("Shell integration not yet implemented for {:?}", shell);
        }
    }
    Ok(())
}
