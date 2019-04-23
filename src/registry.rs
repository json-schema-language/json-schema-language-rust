//! Logic related to holding a collection of schemas together.

use crate::errors::JslError;
use crate::schema::{Form, Schema};
use failure::{bail, Error};
use std::collections::{HashMap, HashSet};
use url::Url;

/// Holds a collection of schemas, ensuring their mutual references are valid.
#[derive(Default)]
pub struct Registry {
    schemas: HashMap<Option<Url>, Schema>,
    missing_ids: Vec<Url>,
}

impl Registry {
    /// Construct a new, empty registry.
    pub fn new() -> Registry {
        Registry {
            schemas: HashMap::new(),
            missing_ids: Vec::new(),
        }
    }

    /// Add a schema to the registry, and return the IDs of the schemas still
    /// missing.
    ///
    /// Returns an error if the given schema is non-root, if there already
    /// exists a schema with the given ID in the registry, or if the given
    /// schema refers to a definition which is known not to exist.
    ///
    /// Because this method returns IDs that are still missing, you can call
    /// this function over multiple passes until all schemas are fetched. This
    /// crate does not presume how or whether you want to fetch schemas over the
    /// network.
    ///
    /// ```
    /// use serde_json::json;
    /// use jsl::{Registry, Schema, SerdeSchema, Validator, ValidationError};
    /// use failure::{Error, format_err};
    /// use url::Url;
    ///
    /// fn main() -> Result<(), Error> {
    ///     let initial_schema: SerdeSchema = serde_json::from_value(json!({
    ///         "properties": {
    ///             "users": {
    ///                 "elements": {
    ///                     "ref": "http://schemas.example.com/user.json"
    ///                 },
    ///             },
    ///             "next_page_token": { "type": "string" },
    ///         },
    ///     }))?;
    ///
    ///     let initial_schema = Schema::from_serde(initial_schema)?;
    ///
    ///     let mut registry = Registry::new();
    ///     let mut missing = registry.register(initial_schema)?;
    ///
    ///     // When this loop completes, all cross-references will be satisfied.
    ///     while !missing.is_empty() {
    ///         // Your fetch function could decide that an ID is untrusted, and
    ///         // refuse to fetch it.
    ///         let schema = fetch(&missing[0])?;
    ///         missing = registry.register(schema)?;
    ///     }
    ///
    ///     Ok(())
    /// }
    ///
    /// // This is just a demo implementation. It's up to you how this would
    /// // really work, but it's strongly recommended that you never simply
    /// // execute arbitrary schemas from the network.
    /// fn fetch(url: &Url) -> Result<Schema, Error> {
    ///     if url.as_str() != "http://schemas.example.com/user.json" {
    ///         return Err(format_err!("unexpected url"));
    ///     }
    ///
    ///     return Ok(Schema::from_serde(serde_json::from_value(json!({
    ///         "properties": {
    ///             "name": { "type": "string" },
    ///             "display_name": { "type": "string" },
    ///             "created_at": { "type": "string" },
    ///         }
    ///     }))?)?);
    /// }
    /// ```
    pub fn register(&mut self, schema: Schema) -> Result<&[Url], Error> {
        let id = if let Some(root_data) = schema.root_data() {
            root_data.id()
        } else {
            bail!(JslError::NonRoot);
        };

        self.schemas.insert(id.clone(), schema);

        let mut missing_ids = HashSet::new();
        for schema in self.schemas.values() {
            self.compute_missing_ids(&mut missing_ids, schema)?;
        }

        self.missing_ids = missing_ids.into_iter().collect();
        Ok(&self.missing_ids)
    }

    /// Gets the schema in this registry with the given ID.
    ///
    /// If no such schema exists in this registry, returns None.
    pub fn get(&self, id: &Option<Url>) -> Option<&Schema> {
        self.schemas.get(id)
    }

    /// Is this registry sealed?
    ///
    /// A registry being sealed doesn't mean that it's immutable. Rather, it
    /// means that there are no missing IDs in the registry. In other words, the
    /// registry is self-sufficient and consistent. Adding schemas to a sealed
    /// registry may or may not unseal it.
    ///
    /// This is just a convenience shorthand for `missing_ids().is_empty()`.
    pub fn is_sealed(&self) -> bool {
        self.missing_ids.is_empty()
    }

    /// Get the IDs missing from this registry.
    ///
    /// This is the same value as what [`register`](#method.register) returns.
    pub fn missing_ids(&self) -> &[Url] {
        &self.missing_ids
    }

    fn compute_missing_ids(&self, out: &mut HashSet<Url>, schema: &Schema) -> Result<(), Error> {
        if let Some(root) = schema.root_data() {
            for def in root.definitions().values() {
                self.compute_missing_ids(out, def)?;
            }
        }

        match schema.form() {
            // Main case: checking references.
            Form::Ref(ref id, ref def) => {
                if let Some(refd_schema) = self.schemas.get(id) {
                    let refd_root_data = refd_schema
                        .root_data()
                        .as_ref()
                        .expect("unreachable: non-root schema in registry");

                    if let Some(def) = def {
                        if !refd_root_data.definitions().contains_key(def) {
                            bail!(JslError::NoSuchDefinition {
                                id: id.clone(),
                                definition: def.clone(),
                            });
                        }
                    }
                } else {
                    out.insert(
                        id.clone()
                            .expect("unreachable: non-resolving reference to anonymous schema"),
                    );
                }
            }

            // Recursive cases: discover all references.
            Form::Elements(ref schema) => self.compute_missing_ids(out, schema)?,
            Form::Properties(ref required, ref optional, _) => {
                for schema in required.values() {
                    self.compute_missing_ids(out, schema)?;
                }

                for schema in optional.values() {
                    self.compute_missing_ids(out, schema)?;
                }
            }
            Form::Values(ref schema) => self.compute_missing_ids(out, schema)?,
            Form::Discriminator(_, ref mapping) => {
                for schema in mapping.values() {
                    self.compute_missing_ids(out, schema)?;
                }
            }
            _ => {}
        };

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn register() {
        let mut registry = Registry::new();
        let missing1 = registry
            .register(
                Schema::from_serde(
                    serde_json::from_value(json!({
                        "id": "http://example.com/foo",
                        "definitions": {
                            "a": {
                                "ref": "",
                            },
                            "b": {
                                "ref": "#",
                            },
                            "c": {
                                "ref": "#c"
                            },
                            "d": {
                                "ref": "http://example.com/foo#d"
                            }
                        }
                    }))
                    .unwrap(),
                )
                .unwrap(),
            )
            .unwrap();

        assert_eq!(missing1, &[]);

        let missing2 = registry
            .register(
                Schema::from_serde(
                    serde_json::from_value(json!({
                        "id": "http://example.com/foo",
                        "definitions": {
                            "a": {
                                "ref": "/bar",
                            },
                            "b": {
                                "ref": "//foo.example.com",
                            },
                            "c": {
                                "ref": "/bar#c"
                            },
                            "d": {
                                "ref": "//foo.example.com#d"
                            }
                        }
                    }))
                    .unwrap(),
                )
                .unwrap(),
            )
            .unwrap();

        let mut missing2: Vec<_> = missing2.iter().cloned().collect();
        missing2.sort();
        assert_eq!(
            missing2,
            &[
                "http://example.com/bar".parse().unwrap(),
                "http://foo.example.com".parse().unwrap(),
            ]
        );

        let missing3 = registry
            .register(
                Schema::from_serde(
                    serde_json::from_value(json!({
                        "id": "http://example.com/bar",
                        "definitions": {
                            "c": {},
                        },
                    }))
                    .unwrap(),
                )
                .unwrap(),
            )
            .unwrap();

        assert_eq!(missing3, &["http://foo.example.com".parse().unwrap()]);

        let missing4 = registry
            .register(
                Schema::from_serde(
                    serde_json::from_value(json!({
                        "id": "http://foo.example.com",
                        "definitions": {
                            "d": {},
                        },
                    }))
                    .unwrap(),
                )
                .unwrap(),
            )
            .unwrap();

        assert_eq!(missing4, &[]);
        assert!(registry.is_sealed());

        let missing5 = registry
            .register(
                Schema::from_serde(
                    serde_json::from_value(json!({
                        "id": "http://bar.example.com",
                        "definitions": {
                            "a": {
                                "ref": "/1",
                            },
                            "b": {
                                "elements": { "ref": "/2" },
                            },
                            "c": {
                                "properties": {
                                    "a": { "ref": "/3" },
                                },
                                "optionalProperties": {
                                    "b": { "ref": "/4" },
                                },
                            },
                            "d": {
                                "values": { "ref": "/5" },
                            },
                            "e": {
                                "discriminator": {
                                    "tag": "foo",
                                    "mapping": {
                                        "a": {
                                            "properties": {
                                                "a": { "ref": "/6" },
                                            },
                                        },
                                    },
                                },
                            },
                        },
                    }))
                    .unwrap(),
                )
                .unwrap(),
            )
            .unwrap();

        let mut missing5: Vec<_> = missing5.iter().cloned().collect();
        missing5.sort();
        assert_eq!(
            missing5,
            &[
                "http://bar.example.com/1".parse().unwrap(),
                "http://bar.example.com/2".parse().unwrap(),
                "http://bar.example.com/3".parse().unwrap(),
                "http://bar.example.com/4".parse().unwrap(),
                "http://bar.example.com/5".parse().unwrap(),
                "http://bar.example.com/6".parse().unwrap(),
            ]
        );
    }
}
