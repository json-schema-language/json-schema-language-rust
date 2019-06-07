//! Validate input data against schemas.
//!
//! This module contains logic related to *validation*, the process of taking a
//! piece of input data (called an "instance") and checking if it's valid
//! according to a schema.
//!
//! See the docs for [`Validator`](struct.Validator.html) for more.

use crate::schema::Schema;
use crate::vm::validate;
use failure::Error;
use json_pointer::JsonPointer;
use std::borrow::Cow;

/// Validates instances against schemas.
#[derive(Debug, Default, Eq, PartialEq, Clone, Hash)]
pub struct Validator {
    config: Config,
}

impl Validator {
    /// Constructs a new validator using the default configuration.
    pub fn new() -> Self {
        Self::new_with_config(Config::default())
    }

    /// Constructs a new validator using a configuration.
    pub fn new_with_config(config: Config) -> Self {
        Self { config }
    }

    /// Validate an instance against a schema.
    ///
    /// The generated validation errors have the same lifetime as the inputted
    /// instance; this crate avoids copying data out of your inputted data.
    /// Despite having "Error" in their name, they are not Rust errors. A list
    /// of validation errors is the _successful_ result of running `validate`.
    ///
    /// Returns an error if if the maximum reference depth is exceeded (see
    /// [`ValidatorConfig::max_depth`](struct.ValidatorConfig.html#method.max_depth)).
    pub fn validate<'a>(
        &self,
        schema: &'a Schema,
        instance: &'a serde_json::Value,
    ) -> Result<Vec<ValidationError<'a>>, Error> {
        validate(
            self.config.max_errors,
            self.config.max_depth,
            self.config.strict_instance_semantics,
            schema,
            instance,
        )
    }
}

/// Configuration for how validation should proceed.
#[derive(Debug, Eq, PartialEq, Clone, Hash)]
pub struct Config {
    max_errors: usize,
    max_depth: usize,
    strict_instance_semantics: bool,
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

    /// Sets whether to use strict instance semantics. The default is to not use
    /// strict instance semantics.
    ///
    /// Essentially, strict instance semantics determines whether it's ok for an
    /// instance (input) to have properties not mentioned in a schema. When
    /// using strict instance semantics, these undeclared properties will be
    /// considered erroneuous. In non-strict instance semantics, these
    /// properties are simply ignored.
    pub fn strict_instance_semantics(&mut self, strict_instance_semantics: bool) -> &mut Self {
        self.strict_instance_semantics = strict_instance_semantics;
        self
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            max_errors: 0,
            max_depth: 32,
            strict_instance_semantics: false,
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
}

impl<'a> ValidationError<'a> {
    pub fn new(
        instance_path: JsonPointer<Cow<'a, str>, Vec<Cow<'a, str>>>,
        schema_path: JsonPointer<Cow<'a, str>, Vec<Cow<'a, str>>>,
    ) -> ValidationError<'a> {
        ValidationError {
            instance_path,
            schema_path,
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
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::schema::Schema;
    use serde_json::json;

    #[test]
    fn infinite_loop() -> Result<(), Error> {
        let validator = Validator::new();
        assert!(validator
            .validate(
                &Schema::from_serde(serde_json::from_value(json!({
                    "definitions": {
                        "a": { "ref": "a" },
                    },
                    "ref": "a",
                }))?)?,
                &json!({})
            )
            .is_err());

        Ok(())
    }

    #[test]
    fn max_errors() -> Result<(), Error> {
        let mut config = Config::new();
        config.max_errors(3);

        let validator = Validator::new_with_config(config);
        assert_eq!(
            validator
                .validate(
                    &Schema::from_serde(serde_json::from_value(json!({
                        "elements": { "type": "string" },
                    }))?)?,
                    &json!([null, null, null, null, null,])
                )
                .unwrap()
                .len(),
            3
        );

        Ok(())
    }
}
