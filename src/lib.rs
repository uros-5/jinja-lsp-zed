use std::fs;
use zed_extension_api::{self as zed, settings::LspSettings, Result};

// CREDITS: https://github.com/zed-extensions/typst/blob/main/src/typst.rs

struct JinjaLspExtension {
    cached_binary_path: Option<String>,
}

#[derive(Clone)]
struct JinjaLspBinary {
    path: String,
    args: Option<Vec<String>>,
    environment: Option<Vec<(String, String)>>,
}

impl JinjaLspExtension {
    fn language_server_binary(
        &mut self,
        language_server_id: &zed::LanguageServerId,
        worktree: &zed::Worktree,
    ) -> Result<JinjaLspBinary> {
        let binary_settings = LspSettings::for_worktree("jinja-lsp", worktree)
            .ok()
            .and_then(|lsp_settings| lsp_settings.binary);
        let binary_args = binary_settings
            .as_ref()
            .and_then(|settings| settings.arguments.clone());

        if let Some(path) = worktree.which("jinja-lsp") {
            let env = worktree.shell_env();
            return Ok(JinjaLspBinary {
                path,
                args: binary_args,
                environment: Some(env),
            });
        }

        if let Some(path) = &self.cached_binary_path {
            if fs::metadata(path).is_ok_and(|stat| stat.is_file()) {
                return Ok(JinjaLspBinary {
                    path: path.clone(),
                    args: binary_args,
                    environment: None,
                });
            }
        }

        zed::set_language_server_installation_status(
            language_server_id,
            &zed::LanguageServerInstallationStatus::CheckingForUpdate,
        );
        let release = zed::latest_github_release(
            "uros-5/jinja-lsp",
            zed::GithubReleaseOptions {
                require_assets: true,
                pre_release: false,
            },
        );

        let release = match release {
            Ok(r) => r,
            Err(e) => {
                if let Ok(path) = fs::read_to_string("latest-binary") {
                    if fs::metadata(&path).is_ok_and(|stat| stat.is_file()) {
                        return Ok(JinjaLspBinary {
                            path,
                            args: binary_args,
                            environment: None,
                        });
                    }
                }
                return Err(e);
            }
        };

        let (platform, arch) = zed::current_platform();
        let os = match platform {
            zed::Os::Mac => "apple-darwin",
            zed::Os::Linux => "unknown-linux-gnu",
            zed::Os::Windows => "pc-windows-msvc",
        };
        let arch_str = match arch {
            zed::Architecture::Aarch64 => "aarch64",
            zed::Architecture::X86 => "x86",
            zed::Architecture::X8664 => "x86_64",
        };
        let mut asset_name = format!("jinja-lsp-{arch_str}-{os}");

        if platform == zed::Os::Windows {
            asset_name = format!("{asset_name}.exe");
        }

        let asset = release
            .assets
            .iter()
            .find(|asset| asset.name == asset_name)
            .ok_or_else(|| format!("no asset found matching {:?}", asset_name))?;

        let version_dir = format!("jinja-lsp-{}", release.version);
        fs::create_dir_all(&version_dir).map_err(|e| format!("failed to create directory: {e}"))?;

        let binary_path = if cfg!(windows) {
            format!("{version_dir}/jinja-lsp.exe")
        } else {
            format!("{version_dir}/jinja-lsp")
        };

        if !fs::metadata(&binary_path).is_ok_and(|stat| stat.is_file()) {
            zed::set_language_server_installation_status(
                language_server_id,
                &zed::LanguageServerInstallationStatus::Downloading,
            );

            zed::download_file(
                &asset.download_url,
                &binary_path,
                zed::DownloadedFileType::Uncompressed,
            )
            .map_err(|e| format!("failed to download file: {e}"))?;

            zed::make_file_executable(&binary_path)?;

            let entries =
                fs::read_dir(".").map_err(|e| format!("failed to list working directory {e}"))?;
            for entry in entries {
                let entry = entry.map_err(|e| format!("failed to load directory entry {e}"))?;
                if entry.file_name().to_str() != Some(&version_dir) {
                    fs::remove_dir_all(entry.path()).ok();
                }
            }

            let _ = fs::write("latest-binary", &binary_path);
        }

        self.cached_binary_path = Some(binary_path.clone());
        Ok(JinjaLspBinary {
            path: binary_path,
            args: binary_args,
            environment: None,
        })
    }
}

impl zed::Extension for JinjaLspExtension {
    fn new() -> Self {
        Self {
            cached_binary_path: None,
        }
    }

    fn language_server_command(
        &mut self,
        language_server_id: &zed::LanguageServerId,
        worktree: &zed::Worktree,
    ) -> Result<zed::Command> {
        let binary = self.language_server_binary(language_server_id, worktree)?;

        Ok(zed::Command {
            command: binary.path,
            args: binary.args.unwrap_or_else(|| vec!["--stdio".to_string()]),
            env: binary.environment.unwrap_or_default(),
        })
    }

    fn language_server_initialization_options(
        &mut self,
        server_id: &zed_extension_api::LanguageServerId,
        worktree: &zed_extension_api::Worktree,
    ) -> Result<Option<zed_extension_api::serde_json::Value>> {
        let settings = LspSettings::for_worktree(server_id.as_ref(), worktree)
            .ok()
            .and_then(|lsp_settings| lsp_settings.initialization_options.clone())
            .unwrap_or_default();
        Ok(Some(settings))
    }

    fn language_server_workspace_configuration(
        &mut self,
        server_id: &zed_extension_api::LanguageServerId,
        worktree: &zed_extension_api::Worktree,
    ) -> Result<Option<zed_extension_api::serde_json::Value>> {
        let settings = LspSettings::for_worktree(server_id.as_ref(), worktree)
            .ok()
            .and_then(|lsp_settings| lsp_settings.settings.clone())
            .unwrap_or_default();
        Ok(Some(settings))
    }
}

zed::register_extension!(JinjaLspExtension);
