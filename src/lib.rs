//! `jsl` is a Rust implementation of [JSON Schema Language][jsl] ("JSL"), a
//! portable way to describe and validate the structure of JSON data.
//!
//! The documentation for this crate focuses on making JSON Schema Language work
//! with Rust. For information on JSON Schema Language in general, refer to the
//! [documentation on the JSL homepage][jsl-docs].
//!
//! # Validating data
//!
//! The most common use-case for this crate is checking that some JSON input is
//! really valid against a schema. Here's how you'd achieve that use-case:
//!
//! ```
//! use serde_json::json;
//! use jsl::{Schema, SerdeSchema, Validator, ValidationError};
//! use failure::Error;
//! use std::collections::HashSet;
//!
//! fn main() -> Result<(), Error> {
//!     let demo_schema_data = r#"
//!         {
//!             "properties": {
//!                 "name": { "type": "string" },
//!                 "age": { "type": "number" },
//!                 "phones": {
//!                     "elements": { "type": "string" }
//!                 }
//!             }
//!         }
//!     "#;
//!
//!     // The SerdeSchema type is a serde-friendly format for representing
//!     // schemas.
//!     let demo_schema: SerdeSchema = serde_json::from_str(demo_schema_data)?;
//!
//!     // The Schema type is a higher-level format that does more validity
//!     // checks.
//!     let demo_schema = Schema::from_serde(demo_schema).unwrap();
//!
//!     // Validator can quickly check if an instance satisfies some schema.
//!     // With the new_with_config constructor, you can configure how many
//!     // errors to return, and how to handle the possibility of a
//!     // circularly-defined schema.
//!     let validator = Validator::new();
//!     let input_ok = json!({
//!         "name": "John Doe",
//!         "age": 43,
//!         "phones": [
//!             "+44 1234567",
//!             "+44 2345678"
//!         ]
//!     });
//!
//!     let validation_errors_ok = validator.validate(&demo_schema, &input_ok)?;
//!     assert!(validation_errors_ok.is_empty());
//!
//!     let input_bad = json!({
//!         "age": "43",
//!         "phones": [
//!             "+44 1234567",
//!             442345678
//!         ]
//!     });
//!
//!     // Each ValidationError holds paths to the bad part of the input, as
//!     // well as the part of the schema which rejected it.
//!     //
//!     // For testing purposes, we'll sort the errors so that their order is
//!     // predictable.
//!     let mut validation_errors_bad = validator.validate(&demo_schema, &input_bad)?;
//!     validation_errors_bad.sort_by_key(|err| err.instance_path().to_string());
//!     assert_eq!(validation_errors_bad.len(), 3);
//!
//!     // "name" is required
//!     assert_eq!(validation_errors_bad[0].instance_path().to_string(), "");
//!     assert_eq!(validation_errors_bad[0].schema_path().to_string(), "/properties/name");
//!
//!     // "age" has the wrong type
//!     assert_eq!(validation_errors_bad[1].instance_path().to_string(), "/age");
//!     assert_eq!(validation_errors_bad[1].schema_path().to_string(), "/properties/age/type");
//!
//!     // "phones[1]" has the wrong type
//!     assert_eq!(validation_errors_bad[2].instance_path().to_string(), "/phones/1");
//!     assert_eq!(validation_errors_bad[2].schema_path().to_string(), "/properties/phones/elements/type");
//!
//!     Ok(())
//! }
//!
//! ```
//!
//! The [`ValidationError`](validator/struct.ValidationError.html) type that
//! [`Validator::validate`](validator/struct.Validator.html#method.validate)
//! produces contains two
//! [`json_pointer::JsonPointer`s](https://docs.rs/json-pointer/0.3.2/json_pointer/struct.JsonPointer.html).
//! These paths within these validation errors are standardized, and should be
//! understood by any implementation of JSL, not just this crate. All compliant
//! implementations of JSL produce the same paths, making them
//! platform-independent.
//!
//! # Writing tooling on top of JSL
//!
//! JSL was designed with the same principles that make JSON so useful: it's
//! easy to implement, and even easier to build on top of. If you're building
//! custom tooling on top of JSL, such as UI, documentation, or code generation,
//! this crate provides a [`Schema`](schema/struct.Schema.html) type for that
//! purpose.
//!
//! See the docs for [`Schema`](schema/struct.Schema.html) for more.
//!
//! [jsl]: http://json-schema-language.github.io
//!
//! [jsl-docs]: http://json-schema-language.github.io/docs

mod vm;

pub mod errors;
pub mod schema;
pub mod validator;

pub use crate::errors::JslError;
pub use crate::schema::{Form, Schema, Serde as SerdeSchema, Type};
pub use crate::validator::{Config, ValidationError, Validator};
