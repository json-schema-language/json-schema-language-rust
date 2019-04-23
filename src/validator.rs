//! Validate input data against schemas.
//!
//! This module contains logic related to *validation*, the process of taking a
//! piece of input data (called an "instance") and checking if it's valid
//! according to a schema.
//!
//! See the docs for [`Validator`](struct.Validator.html) for more.

use crate::registry::Registry;
use crate::vm::validate;
use failure::Error;
use json_pointer::JsonPointer;
use std::borrow::Cow;
use url::Url;

/// Validates instances against a registry of schemas.
pub struct Validator<'a> {
    config: Config,
    registry: &'a Registry,
}

impl<'a> Validator<'a> {
    /// Constructs a new validator using a registry and the default
    /// configuration.
    pub fn new(registry: &'a Registry) -> Self {
        Self::new_with_config(Config::default(), registry)
    }

    /// Constructs a new validator using a registry and configuration.
    pub fn new_with_config(config: Config, registry: &'a Registry) -> Self {
        Self { config, registry }
    }

    /// Validate an instance against the default schema the registry.
    ///
    /// See [`validate_by_uri`](#method.validate_by_uri) for possible error
    /// conditions.
    pub fn validate(
        &'a self,
        instance: &'a serde_json::Value,
    ) -> Result<Vec<ValidationError<'a>>, Error> {
        self.validate_by_id(&None, instance)
    }

    /// Validate an instance against the schema with the given URI.
    ///
    /// Returns an error if the registry is currently unsealed (see
    /// [`Registry::is_sealed`](../registry/struct.Registry.html#method.is_sealed)), or if
    /// the maximum reference depth is exceeded (see
    /// [`ValidatorConfig::max_depth`](struct.ValidatorConfig.html#method.max_depth)).
    ///
    /// The generated errors have the same lifetime as the inputted instance;
    /// this crate avoids copying data out of your inputted data.
    pub fn validate_by_id(
        &'a self,
        id: &'a Option<Url>,
        instance: &'a serde_json::Value,
    ) -> Result<Vec<ValidationError<'a>>, Error> {
        validate(
            self.config.max_errors,
            self.config.max_depth,
            self.registry,
            id,
            instance,
        )
    }
}

/// Configuration for how validation should proceed.
pub struct Config {
    max_errors: usize,
    max_depth: usize,
}

impl Config {
    /// Create a new, default `Config`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the maximum number of errors to produce before stopping validation.
    /// 0, the default value, indicates that all errors should be produced.
    ///
    /// If your use-case doesn't care about errors, and you just want to abort
    /// on the first error, you should set this value to 1.
    pub fn max_errors(&mut self, max_errors: usize) -> &mut Self {
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
    pub fn max_depth(&mut self, max_depth: usize) -> &mut Self {
        self.max_depth = max_depth;
        self
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
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
///
/// `ValidationError` uses `Cow` instead of `String` to store its components.
/// That's because this crate makes every effort to never copy data out of your
/// instances. However, some parts of error paths require allocation (such as
/// when the `usize` indices of an array are converted into `String`), and so
/// `Cow` is used.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ValidationError<'a> {
    instance_path: JsonPointer<Cow<'a, str>, Vec<Cow<'a, str>>>,
    schema_path: JsonPointer<Cow<'a, str>, Vec<Cow<'a, str>>>,
    schema_id: &'a Option<Url>,
}

impl<'a> ValidationError<'a> {
    pub fn new(
        instance_path: JsonPointer<Cow<'a, str>, Vec<Cow<'a, str>>>,
        schema_path: JsonPointer<Cow<'a, str>, Vec<Cow<'a, str>>>,
        schema_id: &'a Option<Url>,
    ) -> ValidationError<'a> {
        ValidationError {
            instance_path,
            schema_path,
            schema_id,
        }
    }

    /// A pointer into the part of the instance (input) which was rejected.
    pub fn instance_path(&self) -> &JsonPointer<Cow<'a, str>, Vec<Cow<'a, str>>> {
        &self.instance_path
    }

    /// A pointer into the part of the schema which rejected the instance.
    pub fn schema_path(&self) -> &JsonPointer<Cow<'a, str>, Vec<Cow<'a, str>>> {
        &self.schema_path
    }

    /// The ID of the schema which rejected the instance. If the schema
    /// doesn't have an ID, then this is None.
    pub fn schema_id(&self) -> &Option<Url> {
        &self.schema_id
    }
}
