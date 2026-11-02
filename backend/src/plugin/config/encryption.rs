//! AES-256-GCM encryption for sensitive plugin configuration values

use crate::core::app::error::ConfigurationErrorKind;
use crate::core::app::error::{Result, TingError};
use serde_json::Value;

/// Encrypt a value using AES-256-GCM
pub fn encrypt_value(encryption_key: &[u8; 32], value: &str) -> Result<String> {
    crate::core::security::crypto::encrypt(value, encryption_key)
}

/// Decrypt a value using AES-256-GCM
pub fn decrypt_value(encryption_key: &[u8; 32], encrypted: &str) -> Result<String> {
    crate::core::security::crypto::decrypt(encrypted, encryption_key)
}

/// Extract encrypted field paths from schema.
///
/// The canonical marker is `x-encrypted: true`, but the UI also treats
/// `encrypted: true` and `format: password|secret` as sensitive fields. Keep
/// the backend in lockstep so a password-looking field is never stored in
/// plain text because the manifest used a supported alias.
pub fn extract_encrypted_fields(schema: &Value) -> Vec<String> {
    let mut encrypted_fields = Vec::new();
    if let Some(properties) = schema.get("properties").and_then(|p| p.as_object()) {
        for (field_name, field_schema) in properties {
            if is_encrypted_field_schema(field_schema) {
                encrypted_fields.push(field_name.clone());
            }
        }
    }
    encrypted_fields
}

fn is_encrypted_field_schema(field_schema: &Value) -> bool {
    field_schema
        .get("x-encrypted")
        .and_then(Value::as_bool)
        .unwrap_or(false)
        || field_schema
            .get("encrypted")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        || matches!(
            field_schema
                .get("format")
                .and_then(Value::as_str)
                .map(str::trim)
                .map(str::to_ascii_lowercase)
                .as_deref(),
            Some("password" | "secret")
        )
}

/// Encrypt sensitive fields in a configuration value
pub fn encrypt_sensitive_fields(
    encryption_key: &[u8; 32],
    config: &Value,
    encrypted_fields: &[String],
) -> Result<Value> {
    if encrypted_fields.is_empty() {
        return Ok(config.clone());
    }

    let mut encrypted_config = config.clone();
    if let Some(obj) = encrypted_config.as_object_mut() {
        for field_name in encrypted_fields {
            if let Some(field_value) = obj.get(field_name) {
                let value_str = if field_value.is_string() {
                    field_value.as_str().unwrap().to_string()
                } else {
                    field_value.to_string()
                };
                let encrypted = encrypt_value(encryption_key, &value_str)?;
                obj.insert(
                    field_name.clone(),
                    Value::String(format!("encrypted:{}", encrypted)),
                );
            }
        }
    }

    Ok(encrypted_config)
}

/// Decrypt sensitive fields in a configuration value
pub fn decrypt_sensitive_fields(
    encryption_key: &[u8; 32],
    config: &Value,
    encrypted_fields: &[String],
) -> Result<Value> {
    if encrypted_fields.is_empty() {
        return Ok(config.clone());
    }

    let mut decrypted_config = config.clone();
    if let Some(obj) = decrypted_config.as_object_mut() {
        for field_name in encrypted_fields {
            if let Some(field_value) = obj.get(field_name)
                && let Some(encrypted_str) = field_value.as_str()
                && let Some(encrypted_data) = encrypted_str.strip_prefix("encrypted:")
            {
                let decrypted = decrypt_value(encryption_key, encrypted_data)?;
                obj.insert(field_name.clone(), Value::String(decrypted));
            }
        }
    }

    Ok(decrypted_config)
}

/// Validate a configuration against a JSON Schema
pub fn validate_config(schema: &Value, config: &Value) -> Result<()> {
    let mut validation_schema = schema.clone();
    normalize_schema_annotations(&mut validation_schema);
    let compiled_schema = jsonschema::JSONSchema::compile(&validation_schema).map_err(|e| {
        TingError::configuration(
            ConfigurationErrorKind::InvalidValue,
            "validate_config",
            format!("Invalid configuration schema: {e}"),
        )
    })?;

    if let Err(errors) = compiled_schema.validate(config) {
        let error_messages: Vec<String> = errors.map(|e| format!("{}", e)).collect();
        return Err(TingError::configuration(
            ConfigurationErrorKind::InvalidValue,
            "validate_config::configuration",
            format!(
                "Configuration validation failed: {}",
                error_messages.join(", ")
            ),
        ));
    }

    Ok(())
}

fn normalize_schema_annotations(schema: &mut Value) {
    let Some(object) = schema.as_object_mut() else {
        return;
    };

    // Only the validation copy needs standard string annotations. Keep the original
    // localized schema for forms, persistence and exports, and leave invalid types
    // untouched so JSON Schema meta-validation still rejects them.
    for keyword in ["title", "description"] {
        if let Some(annotation) = object.get_mut(keyword)
            && let Some(translations) = annotation.as_object()
            && !translations.is_empty()
            && translations.values().all(Value::is_string)
        {
            let text = translations
                .get("en")
                .or_else(|| translations.get("zh"))
                .or_else(|| translations.values().next())
                .cloned();
            if let Some(text) = text {
                *annotation = text;
            }
        }
    }

    // Follow schema positions only, never instance data in default/enum/const or
    // property-name maps themselves (a user field may be named "title").
    for keyword in [
        "properties",
        "patternProperties",
        "definitions",
        "$defs",
        "dependencies",
        "dependentSchemas",
    ] {
        if let Some(children) = object.get_mut(keyword).and_then(Value::as_object_mut) {
            for child in children.values_mut() {
                normalize_schema_annotations(child);
            }
        }
    }
    for keyword in [
        "additionalProperties",
        "additionalItems",
        "unevaluatedProperties",
        "unevaluatedItems",
        "propertyNames",
        "contains",
        "not",
        "if",
        "then",
        "else",
        "contentSchema",
    ] {
        if let Some(child) = object.get_mut(keyword) {
            normalize_schema_annotations(child);
        }
    }
    for keyword in ["allOf", "anyOf", "oneOf", "prefixItems"] {
        if let Some(children) = object.get_mut(keyword).and_then(Value::as_array_mut) {
            for child in children {
                normalize_schema_annotations(child);
            }
        }
    }
    if let Some(items) = object.get_mut("items") {
        match items {
            Value::Array(children) => {
                for child in children {
                    normalize_schema_annotations(child);
                }
            }
            _ => normalize_schema_annotations(items),
        }
    }
}
