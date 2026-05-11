use zed_extension_api as zed;

struct RvnExtension;

impl zed::Extension for RvnExtension {
    fn new() -> Self {
        Self
    }

    fn language_server_command(
        &mut self,
        _language_server_id: &zed::LanguageServerId,
        worktree: &zed::Worktree,
    ) -> zed::Result<zed::Command> {
        let command = worktree.which("rvn-lsp").unwrap_or_else(|| {
            let root = worktree.root_path();
            format!("{root}/target/debug/rvn-lsp")
        });

        Ok(zed::Command {
            command,
            args: Vec::new(),
            env: worktree.shell_env(),
        })
    }
}

zed::register_extension!(RvnExtension);
