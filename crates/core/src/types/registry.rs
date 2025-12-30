//! Type registry for managing note type definitions.

use std::collections::HashMap;
use std::sync::Arc;

use super::definition::TypeDefinition;
use super::discovery::TypedefRepository;
use super::errors::TypedefError;
use crate::index::types::NoteType;

/// Registry of all known note types (built-in + custom).
///
/// The registry maintains:
/// - Custom type definitions loaded from Lua files
/// - Overrides for built-in types (when a Lua file matches a built-in name)
#[derive(Debug, Default)]
pub struct TypeRegistry {
    /// Custom type definitions loaded from Lua.
    custom_types: HashMap<String, Arc<TypeDefinition>>,

    /// Built-in type overrides (Lua files that extend built-in types).
    builtin_overrides: HashMap<NoteType, Arc<TypeDefinition>>,
}

impl TypeRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a registry from a TypedefRepository.
    pub fn from_repository(repo: &TypedefRepository) -> Result<Self, TypedefError> {
        let mut registry = Self::new();

        for info in repo.list_all() {
            let typedef = repo.load_typedef(&info.name)?;
            registry.register(typedef)?;
        }

        Ok(registry)
    }

    /// Register a type definition.
    pub fn register(&mut self, typedef: TypeDefinition) -> Result<(), TypedefError> {
        let name = typedef.name.clone();
        let typedef = Arc::new(typedef);

        // Check if it's a built-in type override
        if let Some(builtin) = Self::parse_builtin(&name) {
            self.builtin_overrides.insert(builtin, typedef);
            return Ok(());
        }

        // Otherwise, register as custom type
        if self.custom_types.contains_key(&name) {
            return Err(TypedefError::Duplicate(name));
        }
        self.custom_types.insert(name, typedef);
        Ok(())
    }

    /// Get a type definition by name.
    ///
    /// Returns custom types and built-in overrides.
    pub fn get(&self, name: &str) -> Option<Arc<TypeDefinition>> {
        // First check custom types
        if let Some(td) = self.custom_types.get(name) {
            return Some(Arc::clone(td));
        }

        // Then check built-in overrides
        Self::parse_builtin(name)
            .and_then(|builtin| self.builtin_overrides.get(&builtin).cloned())
    }

    /// Get override for a built-in type (if any).
    pub fn get_builtin_override(&self, note_type: NoteType) -> Option<Arc<TypeDefinition>> {
        self.builtin_overrides.get(&note_type).cloned()
    }

    /// Check if a type name is known (built-in or custom).
    pub fn is_known_type(&self, name: &str) -> bool {
        // Check custom types
        if self.custom_types.contains_key(name) {
            return true;
        }

        // Check if it's a valid built-in type
        Self::parse_builtin(name).is_some()
    }

    /// Check if there's a definition for a type (custom or override).
    pub fn has_definition(&self, name: &str) -> bool {
        self.get(name).is_some()
    }

    /// List all custom type names.
    pub fn list_custom_types(&self) -> Vec<&str> {
        self.custom_types.keys().map(|s| s.as_str()).collect()
    }

    /// List all overridden built-in types.
    pub fn list_overridden_builtins(&self) -> Vec<NoteType> {
        self.builtin_overrides.keys().copied().collect()
    }

    /// List all types (built-in names + custom names).
    pub fn list_all_types(&self) -> Vec<String> {
        let mut types: Vec<String> = vec![
            "daily".to_string(),
            "weekly".to_string(),
            "task".to_string(),
            "project".to_string(),
            "zettel".to_string(),
        ];
        types.extend(self.custom_types.keys().cloned());
        types.sort();
        types.dedup();
        types
    }

    /// Get the number of custom types registered.
    pub fn custom_type_count(&self) -> usize {
        self.custom_types.len()
    }

    /// Get the number of builtin overrides registered.
    pub fn override_count(&self) -> usize {
        self.builtin_overrides.len()
    }

    /// Parse a type name to a built-in NoteType, excluding None.
    fn parse_builtin(name: &str) -> Option<NoteType> {
        match name.to_lowercase().as_str() {
            "daily" => Some(NoteType::Daily),
            "weekly" => Some(NoteType::Weekly),
            "task" => Some(NoteType::Task),
            "project" => Some(NoteType::Project),
            "zettel" | "knowledge" => Some(NoteType::Zettel),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::schema::{FieldSchema, FieldType};

    fn make_typedef(name: &str) -> TypeDefinition {
        TypeDefinition::empty(name)
    }

    #[test]
    fn test_empty_registry() {
        let registry = TypeRegistry::new();
        assert_eq!(registry.custom_type_count(), 0);
        assert_eq!(registry.override_count(), 0);
        assert!(!registry.has_definition("anything"));
    }

    #[test]
    fn test_register_custom_type() {
        let mut registry = TypeRegistry::new();
        let typedef = make_typedef("meeting");

        registry.register(typedef).unwrap();

        assert_eq!(registry.custom_type_count(), 1);
        assert!(registry.has_definition("meeting"));
        assert!(registry.is_known_type("meeting"));
    }

    #[test]
    fn test_register_builtin_override() {
        let mut registry = TypeRegistry::new();
        let typedef = make_typedef("task");

        registry.register(typedef).unwrap();

        // Should be an override, not a custom type
        assert_eq!(registry.custom_type_count(), 0);
        assert_eq!(registry.override_count(), 1);
        assert!(registry.has_definition("task"));
        assert!(registry.get_builtin_override(NoteType::Task).is_some());
    }

    #[test]
    fn test_duplicate_custom_type() {
        let mut registry = TypeRegistry::new();
        registry.register(make_typedef("meeting")).unwrap();

        let result = registry.register(make_typedef("meeting"));
        assert!(matches!(result, Err(TypedefError::Duplicate(_))));
    }

    #[test]
    fn test_builtin_override_replaces() {
        let mut registry = TypeRegistry::new();

        let mut first = make_typedef("task");
        first.description = Some("First".to_string());
        registry.register(first).unwrap();

        let mut second = make_typedef("task");
        second.description = Some("Second".to_string());
        registry.register(second).unwrap();

        // Second should replace first (no duplicate error for overrides)
        let td = registry.get("task").unwrap();
        assert_eq!(td.description, Some("Second".to_string()));
    }

    #[test]
    fn test_is_known_type() {
        let registry = TypeRegistry::new();

        // Built-ins are always known
        assert!(registry.is_known_type("daily"));
        assert!(registry.is_known_type("weekly"));
        assert!(registry.is_known_type("task"));
        assert!(registry.is_known_type("project"));
        assert!(registry.is_known_type("zettel"));

        // Unknown types
        assert!(!registry.is_known_type("meeting"));
        assert!(!registry.is_known_type("custom"));
    }

    #[test]
    fn test_list_all_types() {
        let mut registry = TypeRegistry::new();
        registry.register(make_typedef("meeting")).unwrap();
        registry.register(make_typedef("agenda")).unwrap();

        let types = registry.list_all_types();

        // Should include all built-ins and custom types
        assert!(types.contains(&"daily".to_string()));
        assert!(types.contains(&"task".to_string()));
        assert!(types.contains(&"meeting".to_string()));
        assert!(types.contains(&"agenda".to_string()));
    }

    #[test]
    fn test_list_custom_types() {
        let mut registry = TypeRegistry::new();
        registry.register(make_typedef("meeting")).unwrap();
        registry.register(make_typedef("task")).unwrap(); // Override

        let custom = registry.list_custom_types();

        assert_eq!(custom.len(), 1);
        assert!(custom.contains(&"meeting"));
    }

    #[test]
    fn test_get_with_schema() {
        let mut registry = TypeRegistry::new();

        let mut typedef = make_typedef("meeting");
        typedef.schema.insert(
            "attendees".to_string(),
            FieldSchema {
                field_type: Some(FieldType::List),
                required: true,
                ..Default::default()
            },
        );

        registry.register(typedef).unwrap();

        let td = registry.get("meeting").unwrap();
        assert!(td.schema.contains_key("attendees"));
        assert!(td.schema.get("attendees").unwrap().required);
    }
}
