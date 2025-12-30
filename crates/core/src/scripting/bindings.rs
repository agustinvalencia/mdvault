//! Lua bindings for mdvault functionality.
//!
//! This module provides the `mdv` global table with bindings to
//! mdvault's date math and template rendering engines.

use mlua::{Function, Lua, Result as LuaResult, Table, Value};
use std::collections::HashMap;

use crate::templates::engine::{RenderContext, render_string};
use crate::vars::datemath::{evaluate_date_expr, is_date_expr, parse_date_expr};

/// Register the `mdv` global table with all bindings.
///
/// After calling this function, Lua scripts can use:
/// - `mdv.date(expr, format?)` - Evaluate date math expressions
/// - `mdv.render(template, context)` - Render templates with variables
/// - `mdv.is_date_expr(str)` - Check if string is a date expression
pub fn register_mdv_table(lua: &Lua) -> LuaResult<()> {
    let mdv = lua.create_table()?;

    mdv.set("date", create_date_fn(lua)?)?;
    mdv.set("render", create_render_fn(lua)?)?;
    mdv.set("is_date_expr", create_is_date_expr_fn(lua)?)?;

    lua.globals().set("mdv", mdv)?;
    Ok(())
}

/// Create the `mdv.date(expr, format?)` function.
///
/// # Examples (in Lua)
///
/// ```lua
/// mdv.date("today")           -- "2025-12-29"
/// mdv.date("today + 7d")      -- "2026-01-05"
/// mdv.date("today", "%B %d")  -- "December 29"
/// mdv.date("now", "%H:%M")    -- "14:30"
/// ```
fn create_date_fn(lua: &Lua) -> LuaResult<Function> {
    lua.create_function(|_, args: (String, Option<String>)| {
        let (expr, format_override) = args;

        // Parse the expression
        let mut parsed =
            parse_date_expr(&expr).map_err(|e| mlua::Error::runtime(e.to_string()))?;

        // Override format if provided as second argument
        if let Some(fmt) = format_override {
            parsed.format = Some(fmt);
        }

        Ok(evaluate_date_expr(&parsed))
    })
}

/// Create the `mdv.render(template, context)` function.
///
/// # Examples (in Lua)
///
/// ```lua
/// mdv.render("Hello {{name}}", { name = "World" })  -- "Hello World"
/// mdv.render("Count: {{n}}", { n = 42 })            -- "Count: 42"
/// ```
fn create_render_fn(lua: &Lua) -> LuaResult<Function> {
    lua.create_function(|_, args: (String, Table)| {
        let (template, ctx_table) = args;

        // Convert Lua table to RenderContext (HashMap<String, String>)
        let mut ctx: RenderContext = HashMap::new();

        for pair in ctx_table.pairs::<String, Value>() {
            let (key, value) = pair?;
            let str_value = lua_value_to_string(&key, value)?;
            ctx.insert(key, str_value);
        }

        render_string(&template, &ctx).map_err(|e| mlua::Error::runtime(e.to_string()))
    })
}

/// Create the `mdv.is_date_expr(str)` function.
///
/// # Examples (in Lua)
///
/// ```lua
/// mdv.is_date_expr("today + 1d")  -- true
/// mdv.is_date_expr("hello")       -- false
/// ```
fn create_is_date_expr_fn(lua: &Lua) -> LuaResult<Function> {
    lua.create_function(|_, s: String| Ok(is_date_expr(&s)))
}

/// Convert a Lua value to a string for use in template context.
fn lua_value_to_string(key: &str, value: Value) -> LuaResult<String> {
    match value {
        Value::String(s) => Ok(s.to_str()?.to_string()),
        Value::Integer(i) => Ok(i.to_string()),
        Value::Number(n) => Ok(n.to_string()),
        Value::Boolean(b) => Ok(b.to_string()),
        Value::Nil => Ok(String::new()),
        _ => Err(mlua::Error::runtime(format!(
            "context value for '{}' must be string, number, boolean, or nil",
            key
        ))),
    }
}
