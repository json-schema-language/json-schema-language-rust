//! Validate input data against schemas.
//!
//! This module contains logic related to *validation*, the process of taking a
//! piece of input data (called an "instance") and checking if it's valid
//! according to a schema.
//!
//! See the docs for [`Validator`](struct.Validator.html) for more.

use crate::errors::JslError;
use crate::registry::Registry;
use json_pointer::JsonPointer;
use url::Url;

/// Validates instances against a registry of schemas.
pub struct Validator<'a> {
    config: ValidatorConfig,
    registry: &'a Registry,
}

impl<'a> Validator<'a> {
    /// Constructs a new validator using a registry and the default
    /// configuration.
    pub fn new(registry: &Registry) -> Validator {
        Self::new_with_config(ValidatorConfig::default(), registry)
    }

    /// Constructs a new validator using a registry and configuration.
    pub fn new_with_config(config: ValidatorConfig, registry: &Registry) -> Validator {
        Validator { config, registry }
    }

    /// Validate an instance against the default schema the registry.
    ///
    /// See [`validate_by_uri`](#method.validate_by_uri) for possible error
    /// conditions.
    pub fn validate(&self, instance: serde_json::Value) -> Result<Vec<ValidationError>, JslError> {
        self.validate_by_id(&None, instance)
    }

    /// Validate an instance against the schema with the given URI.
    pub fn validate_by_id(
        &self,
        id: &Option<Url>,
        instance: serde_json::Value,
    ) -> Result<Vec<ValidationError>, JslError> {
        Ok(vec![])
    }
}

/// Configuration for how validation should proceed.
pub struct ValidatorConfig {
    max_errors: usize,
    max_depth: usize,
}

impl ValidatorConfig {
    /// Create a new, default `ValidatorConfig`.
    pub fn new() -> ValidatorConfig {
        ValidatorConfig::default()
    }

    /// Sets the maximum number of errors to produce before stopping validation.
    /// 0, the default value, indicates that all errors should be produced.
    ///
    /// If your use-case doesn't care about errors, and you just want to abort
    /// on the first error, you should set this value to 1.
    pub fn max_errors(&mut self, max_errors: usize) -> &mut ValidatorConfig {
        self.max_errors = max_errors;
        self
    }

    /// Sets the maximum call depth before aborting evaluation. The default
    /// value is to follow 32 cross-references before aborting.
    ///
    /// When evaluation is aborted because of this maximum depth, validation
    /// *fails*. No validation errors are returned.
    ///
    /// This functionality exists to support detecting infinite loops in
    /// schemas, for example in circularly-defined schemas.
    pub fn max_depth(&mut self, max_depth: usize) -> &mut ValidatorConfig {
        self.max_depth = max_depth;
        self
    }
}

impl Default for ValidatorConfig {
    fn default() -> Self {
        ValidatorConfig {
            max_errors: 0,
            max_depth: 32,
        }
    }
}

/// Contains a single problem with an instance when evaluated against a schema.
///
/// Note that, despite its name, `ValidationError` is not an error in the usual
/// Rust sense. It is an ordinary struct, which happens to contain information
/// about why some data was unsatisfactory against a given schema.
pub struct ValidationError {
    instance_path: JsonPointer<String, Vec<String>>,
    schema_path: JsonPointer<String, Vec<String>>,
    schema_id: Option<Url>,
}

impl ValidationError {
    /// A pointer into the part of the instance (input) which was rejected.
    pub fn instance_path(&self) -> &JsonPointer<String, Vec<String>> {
        &self.instance_path
    }

    /// A pointer into the part of the schema which rejected the instance.
    pub fn schema_path(&self) -> &JsonPointer<String, Vec<String>> {
        &self.schema_path
    }

    /// The ID of the schema which rejected the instance. If the schema
    /// doesn't have an ID, then this is None.
    pub fn schema_id(&self) -> &Option<Url> {
        &self.schema_id
    }
}
