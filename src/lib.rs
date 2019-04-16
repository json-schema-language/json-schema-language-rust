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
//! use jsl::{Registry, SerdeSchema, Validator};
//!
//! let demo_schema_data = r#"
//!     {
//!         "properties": {
//!             "name": { "type": "string" },
//!             "age": { "type": "number" },
//!             "phones": {
//!                 "elements": { "type": "string" }
//!             }
//!         }
//!     }
//! "#;
//!
//! // The SerdeSchema type is a serde-friendly format for representing schemas.
//! let demo_schema: SerdeSchema = serde_json::from_str(demo_schema_data)?;
//!
//! // A registry is a bundle of schemas that can cross-reference one another.
//! // When you add a SerdeSchema to a Registry, the Registry will return the
//! // URIs of all schemas still missing from the Registry.
//! let mut registry = Registry::new();
//! let missing_uris = registry.register(&[demo_schema])?;
//!
//! // Our schema doesn't use references, so we're not expecting any dangling
//! // references to other schemas.
//! assert_eq!(missing_uris, vec![]);
//!
//! // Once you've registered all your schemas, you can efficiently begin
//! // processing as many inputs as desired.
//! let validator = Validator::new(&registry);
//! let validation_errors_ok = validator.validate(json!({
//!     "name": "John Doe",
//!     "age": 43,
//!     "phones": [
//!         "+44 1234567",
//!         "+44 2345678"
//!     ]
//! }));
//!
//! assert_eq!(validation_errors_ok, vec![]);
//!
//! let validation_errors_bad = validator.validate(json!({
//!     "age": "43",
//!     "phones": [
//!         "+44 1234567",
//!         442345678
//!     ]
//! }));
//!
//! // Each ValidationError holds paths to the bad part of the input, as well as
//! // the part of the schema which rejected it.
//! assert_eq!(validation_errors_bad, vec![
//!     // name is required, but was not given
//!     ValidationError::new("", "/properties/name"),
//!
//!     // age was a string, but should be a number
//!     ValidationError::new("/age", "/properties/age/type"),
//!
//!     // phones[1] was a number, but should be a string
//!     ValidationError::new("/phones/1", "/properties/phones/elements/type"),
//! ]);
//! ```
//!
//! The [`ValidationError`](ValidationError) type that
//! [`Validator::validate`](Validator::validate) produces contains two
//! [`json_pointer::JsonPointer`s](json_pointer::JsonPointer). These errors are
//! standardized, and should be understood by any implementation of JSL, not
//! just this crate.
//!
//! # Writing tooling on top of JSL
//!
//! JSL was designed with the same principles that make JSON so useful: it's
//! easy to implement, and even easier to build on top of. If you're building
//! custom tooling on top of JSL, such as UI, documentation, or code generation,
//! this crate provides a [`Schema`](struct.Schema.html) type for that purpose.
//!
//! See the docs for [`Schema`](struct.Schema.html) for more.
//!
//! [jsl]: http://json-schema-language.github.io
//!
//! [jsl-docs]: http://json-schema-language.github.io/docs

mod vm;

pub mod errors;
pub mod schema;
pub mod serde;

pub use crate::schema::Registry;
pub use crate::serde::SerdeSchema;
