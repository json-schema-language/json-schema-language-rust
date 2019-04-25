# jsl [![crates.io](https://img.shields.io/crates/v/jsl.svg)](https://crates.io/crates/jsl)

> Documentation on docs.rs: <https://docs.rs/jsl>

This crate is a Rust implementation of **JSON Schema Language**. You can use it
to:

1. Validate input data is valid against a schema,
2. Get a list of validation errors with that input data, or
3. Build your own custom tooling on top of JSON Schema Language.

## About JSON Schema Language

**JSON Schema Language ("JSL")** lets you define schemas for JSON data, or data
that's equivalent to JSON (such a subset of YAML, CBOR, BSON, etc.). Using those
schemas, you can:

1. Validate that inputted JSON data is correctly formatted
2. Document what kind of data you expect to recieve or produce
3. Generate code, documentation, or user interfaces automatically
4. Generate interoperable, detailed validation errors

JSON Schema Language is designed to make JSON more productive. For that reason,
it's super lightweight and easy to implement. It's designed to be intuitive and
easy to extend for your custom use-cases.

For more information, see: <https://json-schema-language.github.io>.

## Usage

The [detailed documentation on docs.rs](https://docs.rs/jsl) goes into more
detail, but at a high level here's how you use this crate to validate inputted
data:

```rust
use serde_json::json;
use jsl::{Registry, Schema, SerdeSchema, Validator, ValidationError};
use failure::Error;
use std::collections::HashSet;

fn main() -> Result<(), Error> {
    let demo_schema_data = r#"
        {
            "properties": {
                "name": { "type": "string" },
                "age": { "type": "number" },
                "phones": {
                    "elements": { "type": "string" }
                }
            }
        }
    "#;

    // The SerdeSchema type is a serde-friendly format for representing
    // schemas.
    let demo_schema: SerdeSchema = serde_json::from_str(demo_schema_data)?;

    // The Schema type is a higher-level format that does more validity
    // checks.
    let demo_schema = Schema::from_serde(demo_schema).unwrap();

    // A registry is a bundle of schemas that can cross-reference one
    // another. When you add a SerdeSchema to a Registry, the Registry will
    // return the URIs of all schemas still missing from the Registry.
    let mut registry = Registry::new();
    let missing_uris = registry.register(demo_schema)?;

    // Our schema doesn't use references, so we're not expecting any
    // dangling references to other schemas.
    assert!(missing_uris.is_empty());

    // Once you've registered all your schemas, you can efficiently begin
    // processing as many inputs as desired.
    let validator = Validator::new(&registry);
    let input_ok = json!({
        "name": "John Doe",
        "age": 43,
        "phones": [
            "+44 1234567",
            "+44 2345678"
        ]
    });

    let validation_errors_ok = validator.validate(&input_ok)?;
    assert!(validation_errors_ok.is_empty());

    let input_bad = json!({
        "age": "43",
        "phones": [
            "+44 1234567",
            442345678
        ]
    });

    // Each ValidationError holds paths to the bad part of the input, as
    // well as the part of the schema which rejected it.
    //
    // For testing purposes, we'll sort the errors so that their order is
    // predictable.
    let mut validation_errors_bad = validator.validate(&input_bad)?;
    validation_errors_bad.sort_by_key(|err| err.instance_path().to_string());
    assert_eq!(validation_errors_bad.len(), 3);

    // "name" is required
    assert_eq!(validation_errors_bad[0].instance_path().to_string(), "");
    assert_eq!(validation_errors_bad[0].schema_path().to_string(), "/properties/name");

    // "age" has the wrong type
    assert_eq!(validation_errors_bad[1].instance_path().to_string(), "/age");
    assert_eq!(validation_errors_bad[1].schema_path().to_string(), "/properties/age/type");

    // "phones[1]" has the wrong type
    assert_eq!(validation_errors_bad[2].instance_path().to_string(), "/phones/1");
    assert_eq!(validation_errors_bad[2].schema_path().to_string(), "/properties/phones/elements/type");

    Ok(())
}
```
