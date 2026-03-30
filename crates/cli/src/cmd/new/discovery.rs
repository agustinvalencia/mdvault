use color_eyre::eyre::{bail, Result, WrapErr};

use mdvault_core::config::types::ResolvedConfig;
use mdvault_core::frontmatter::parse as parse_frontmatter;
use mdvault_core::templates::engine::{render_string, resolve_template_output_path};
use mdvault_core::types::{discovery::load_typedef_from_file, TypeDefinition};
use std::collections::HashMap;
use std::path::PathBuf;

/// Extract note type from rendered content's frontmatter.
pub(super) fn extract_note_type(content: &str) -> Option<String> {
    let parsed = parse_frontmatter(content).ok()?;
    let fm = parsed.frontmatter?;

    if let Some(serde_yaml::Value::String(t)) = fm.fields.get("type") {
        return Some(t.clone());
    }
    None
}

/// Resolve output path from Lua typedef, or return an error.
pub(super) fn resolve_lua_output(
    lua_typedef: &Option<TypeDefinition>,
    cfg: &ResolvedConfig,
    render_ctx: &HashMap<String, String>,
) -> Result<PathBuf> {
    if let Some(ref typedef) = lua_typedef {
        if let Some(ref output_template) = typedef.output {
            render_output_path(output_template, cfg, render_ctx)
                .wrap_err("Failed to resolve Lua output path")
        } else {
            bail!("--output is required (neither template nor Lua script has output)");
        }
    } else {
        bail!("--output is required (template has no output in frontmatter)");
    }
}

/// Resolve the full output path chain: CLI > template FM > behaviour > Lua > error.
pub(super) fn resolve_output_path(
    args_output: Option<&PathBuf>,
    loaded_template: Option<&mdvault_core::templates::repository::LoadedTemplate>,
    note_type: Option<&mdvault_core::domain::NoteType>,
    creation_ctx: Option<&mdvault_core::domain::CreationContext>,
    lua_typedef: &Option<TypeDefinition>,
    cfg: &ResolvedConfig,
    render_ctx: &HashMap<String, String>,
) -> Result<PathBuf> {
    if let Some(out) = args_output {
        return Ok(out.clone());
    }

    if let Some(loaded) = loaded_template {
        match resolve_template_output_path(loaded, cfg, render_ctx) {
            Ok(Some(path)) => return Ok(path),
            Ok(None) => {
                if let (Some(nt), Some(ctx)) = (note_type, creation_ctx) {
                    match nt.behavior().output_path(ctx) {
                        Ok(path) => return Ok(path),
                        Err(_) => {
                            return resolve_lua_output(lua_typedef, cfg, render_ctx)
                        }
                    }
                } else {
                    return resolve_lua_output(lua_typedef, cfg, render_ctx);
                }
            }
            Err(e) => {
                bail!("Failed to resolve output path: {e}");
            }
        }
    }

    // No template — try behaviour output_path, then Lua
    if let (Some(nt), Some(ctx)) = (note_type, creation_ctx) {
        match nt.behavior().output_path(ctx) {
            Ok(path) => Ok(path),
            Err(_) => resolve_lua_output(lua_typedef, cfg, render_ctx),
        }
    } else {
        resolve_lua_output(lua_typedef, cfg, render_ctx)
    }
}

/// Load Lua typedef: from template frontmatter (if template has lua ref),
/// or from the type registry (for scaffolding path without template).
pub(super) fn resolve_lua_typedef(
    loaded_template: Option<&mdvault_core::templates::repository::LoadedTemplate>,
    type_registry: Option<&mdvault_core::types::TypeRegistry>,
    cfg: &ResolvedConfig,
    effective_name: &str,
) -> Option<TypeDefinition> {
    loaded_template
        .and_then(|loaded| loaded.frontmatter.as_ref())
        .and_then(|fm| fm.lua.as_ref())
        .and_then(|lua_path| {
            let lua_file = cfg.resolve_lua_path(lua_path);
            match load_typedef_from_file(&lua_file) {
                Ok(td) => Some(td),
                Err(e) => {
                    eprintln!("Warning: failed to load Lua script '{}': {}", lua_path, e);
                    None
                }
            }
        })
        .or_else(|| {
            type_registry
                .and_then(|reg| reg.get(effective_name).map(|arc| (*arc).clone()))
        })
}

/// Render an output path template with variable substitution.
fn render_output_path(
    template: &str,
    cfg: &ResolvedConfig,
    ctx: &HashMap<String, String>,
) -> Result<PathBuf> {
    let rendered =
        render_string(template, ctx).wrap_err("Failed to render output path template")?;

    let path = PathBuf::from(&rendered);
    if path.is_absolute() {
        Ok(path)
    } else {
        Ok(cfg.vault_root.join(path))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_note_type() {
        let content = "---\ntype: project\n---\nbody";
        assert_eq!(extract_note_type(content), Some("project".into()));

        let content_no_type = "---\ntitle: foo\n---\nbody";
        assert_eq!(extract_note_type(content_no_type), None);
    }
}
