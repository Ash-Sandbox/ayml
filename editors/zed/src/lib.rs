use zed_extension_api as zed;

struct AymlExtension;

impl zed::Extension for AymlExtension {
    fn new() -> Self {
        Self
    }

    fn language_server_command(
        &mut self,
        _language_server_id: &zed::LanguageServerId,
        worktree: &zed::Worktree,
    ) -> zed::Result<zed::Command> {
        let path = worktree
            .which("ayml-lsp")
            .ok_or_else(|| "ayml-lsp not found on PATH".to_string())?;
        Ok(zed::Command {
            command: path,
            args: vec![],
            env: Default::default(),
        })
    }
}

zed::register_extension!(AymlExtension);
