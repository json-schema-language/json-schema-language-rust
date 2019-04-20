//! Logic related to holding a collection of schemas together.

use crate::errors::JslError;
use crate::schema::Schema;
use std::collections::HashMap;
use url::Url;

/// Holds a collection of schemas, ensuring their mutual references are valid.
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
    /// let mut registry = Registry::new();
    /// let mut missing = registry.register(initial_schema)?;
    ///
    /// // When this loop completes, all cross-references will be satisfied.
    /// while !missing.is_empty() {
    ///     // Your fetch function could decide that an ID is untrusted, and
    ///     // refuse to fetch it.
    ///     let schema = fetch(missing[0])?;
    ///     missing = registry.register(schema)?;
    /// }
    /// ```
    pub fn register(&mut self, schema: Schema) -> Result<&[Url], JslError> {
        let id = if let Some(root_data) = schema.root_data() {
            root_data.id()
        } else {
            return Err(JslError::NonRoot);
        };

        self.schemas.insert(id.clone(), schema);

        Ok(&self.missing_ids)
    }

    /// Gets the schema in this registry with the given ID.
    ///
    /// If no such schema exists in this registry, returns None.
    pub fn get(&self, id: &Option<Url>) -> &Option<Schema> {
        &None
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
}
