//! JSL schema representations.
//!
//! This module provides both an abstract ([`Schema`](struct.Schema.html)) and a
//! serializable/deserializable ([`SerdeSchema`](struct.SerdeSchema.html))
//! representation of JSL schemas.

use crate::errors::JslError;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use url::Url;

/// An abstract representation of a JSL schema.
///
/// This struct is meant for use by validators, code generators, or other
/// high-level processors of schemas. For serialization and deserialization of
/// schemas, instead use [`SerdeSchema`](struct.SerdeSchema.html). `Schema` and
/// `SerdeSchema` can be converted between each other within the context of a
/// `Registry`.
pub struct Schema {
    root: Option<RootData>,
    form: Box<Form>,
    extra: HashMap<String, Value>,
}

impl Schema {
    /// Construct a new, root schema from a SerdeSchema.
    pub fn from_serde(serde_schema: SerdeSchema) -> Result<Schema, JslError> {
        Err(JslError::InvalidForm)
    }

    /// Is this schema a root schema?
    ///
    /// Under the hood, this is entirely equivalent to checking whether
    /// `root_data().is_some()`.
    pub fn is_root(&self) -> bool {
        self.root.is_some()
    }

    /// Get the root data associated with this schema.
    ///
    /// If this schema is non-root, this returns None.
    pub fn root_data(&self) -> &Option<RootData> {
        &self.root
    }

    /// Same as [`root_data`](#method.root_data), but takes a mutable reference.
    pub fn root_data_mut(&mut self) -> &mut Option<RootData> {
        &mut self.root
    }

    /// Same as [`root_data`](#method.root_data), but moves ownership.
    pub fn root_data_root(self) -> Option<RootData> {
        self.root
    }

    /// Get the form of the schema.
    pub fn form(&self) -> &Form {
        &self.form
    }

    /// Same as [`form`](#method.form), but takes a mutable reference.
    pub fn form_mut(&mut self) -> &mut Form {
        &mut self.form
    }

    /// Same as [`form`](#method.form), but moves ownership.
    pub fn into_form(self) -> Form {
        *self.form
    }

    /// Get the extra data on the schema.
    ///
    /// Extra data here refers to key-value pairs on a schema which were present
    /// on the data, but the keys were not any of the keywords in JSL.
    ///
    /// This data is useful if you're implementing custom functionality on top
    /// of JSL.
    pub fn extra(&self) -> &HashMap<String, Value> {
        &self.extra
    }

    /// Same as [`extra`](#method.extra), but takes a mutable reference.
    pub fn extra_mut(&mut self) -> &mut HashMap<String, Value> {
        &mut self.extra
    }

    /// Same as [`extra`](#method.extra), but moves ownership.
    pub fn into_extra(self) -> HashMap<String, Value> {
        self.extra
    }
}

/// Data relevant only for root schemas.
pub struct RootData {
    id: Option<Url>,
    defs: HashMap<String, Schema>,
}

impl RootData {
    /// Is this schema anonymous?
    ///
    /// A schema is anonymous when it lacks an `id` keyword. This function is
    /// just convenience for `id().is_none()`.
    pub fn is_anonymous(&self) -> bool {
        self.id.is_none()
    }

    /// Get the id of the schema.
    pub fn id(&self) -> &Option<Url> {
        &self.id
    }

    /// Same as [`id`](#method.id), but takes a mutable reference.
    pub fn id_mut(&mut self) -> &mut Option<Url> {
        &mut self.id
    }

    /// Same as [`id`](#method.id), but moves ownership.
    pub fn into_id(self) -> Option<Url> {
        self.id
    }

    /// Get the definitions of the schema.
    pub fn definitions(&self) -> &HashMap<String, Schema> {
        &self.defs
    }

    /// Same as [`definitions`](#method.definitions), but takes a mutable
    /// reference.
    pub fn definitions_mut(&mut self) -> &mut HashMap<String, Schema> {
        &mut self.defs
    }

    /// Same as [`definitions`](#method.definitions), but moves ownership.
    pub fn into_definitions(self) -> HashMap<String, Schema> {
        self.defs
    }
}

/// The various forms which a schema may take on, and their respective data.
pub enum Form {
    /// The empty form.
    ///
    /// This schema accepts all data.
    Empty,

    /// The ref form.
    ///
    /// This schema refers to another schema, and does whatever that other
    /// schema does.
    ///
    /// The first parameter is the URI of the root schema that the referred-to
    /// schema belongs to. It is None if the schema referred to lacks an ID.
    ///
    /// The second parameter is the definition of the referred-to schema. If the
    /// reference isn't to any definition at all (i.e. the reference is to a
    /// root schema, not a definition), then it is None.
    Ref(Option<Url>, Option<String>),

    /// The type form.
    ///
    /// This schema asserts that the data is one of the primitive types.
    Type(Type),

    /// The elements form.
    ///
    /// This schema asserts that the instance is an array, and that every
    /// element of the array matches a given schema.
    Elements(Schema),

    /// The properties form.
    ///
    /// This schema asserts that the instance is an object, and that the
    /// properties all satisfy their respective schemas.
    ///
    /// The first map is the set of required properties and their schemas. The
    /// second map is the set of optional properties and their schemas.
    Properties(HashMap<String, Schema>, HashMap<String, Schema>),

    /// The values form.
    ///
    /// This schema asserts that the instance is an object, and that all the
    /// values in the object all satisfy the same schema.
    Values(Schema),

    /// The discriminator form.
    ///
    /// This schema asserts that the instance is an object, and that it has a
    /// "tag" property. The value of that tag must be one of the expected
    /// "mapping" keys, and the corresponding mapping value is a schema that the
    /// instance is expected to satisfy.
    ///
    /// The first parameter is the name of the tag property. The second
    /// parameter is the mapping from tag values to their corresponding schemas.
    Discriminator(String, HashMap<String, Schema>),
}

/// The values that the "type" keyword may check for.
///
/// In a certain sense, you can consider these types to be JSON's "primitive"
/// types, with the remaining two types, arrays and objects, being the "complex"
/// types covered by other keywords.
pub enum Type {
    /// The "null" JSON value.
    Null,

    /// The "true" or "false" JSON values.
    Boolean,

    /// Any JSON number.
    ///
    /// Note that JSON only has one kind of number, and JSON numbers may have a
    /// decimal part.
    Number,

    /// Any JSON string.
    String,
}

/// A serialization/deserialization-friendly representation of a JSL schema.
///
/// This struct is meant for use with the `serde` crate. It is excellent for
/// parsing from various data formats, but does not enforce all the semantic
/// rules about how schemas must be formed. For that, consider converting
/// instances of `SerdeSchema` into [`Schema`](struct.Schema.html).
#[derive(Debug, PartialEq, Deserialize, Serialize, Default, Clone)]
pub struct SerdeSchema {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "definitions")]
    pub defs: Option<HashMap<String, SerdeSchema>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "ref")]
    pub rxf: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "type")]
    pub typ: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "elements")]
    pub elems: Option<Box<SerdeSchema>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "properties")]
    pub props: Option<HashMap<String, SerdeSchema>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "optionalProperties")]
    pub opt_props: Option<HashMap<String, SerdeSchema>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub values: Option<Box<SerdeSchema>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub discriminator: Option<SerdeDiscriminator>,

    #[serde(skip_serializing_if = "HashMap::is_empty")]
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

/// A serialization/deserialization-friendly representation of a JSL
/// discriminator.
///
/// This struct is useful mostly in the context of
/// [`SerdeSchema`](struct.SerdeSchema.html).
#[derive(Debug, PartialEq, Deserialize, Serialize, Default, Clone)]
pub struct SerdeDiscriminator {
    #[serde(rename = "tag")]
    pub tag: String,
    pub mapping: HashMap<String, SerdeSchema>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn roundtrip_json() {
        let data = r#"{
  "id": "http://example.com/foo",
  "definitions": {
    "a": {}
  },
  "ref": "http://example.com/bar",
  "type": "foo",
  "elements": {},
  "properties": {
    "a": {}
  },
  "optionalProperties": {
    "a": {}
  },
  "values": {},
  "discriminator": {
    "tag": "foo",
    "mapping": {
      "a": {}
    }
  },
  "extra": "foo"
}"#;

        let parsed: SerdeSchema = serde_json::from_str(data).expect("failed to parse json");
        assert_eq!(
            parsed,
            SerdeSchema {
                id: Some("http://example.com/foo".to_owned()),
                rxf: Some("http://example.com/bar".to_owned()),
                defs: Some(
                    [("a".to_owned(), SerdeSchema::default())]
                        .iter()
                        .cloned()
                        .collect()
                ),
                typ: Some("foo".to_owned()),
                elems: Some(Box::new(SerdeSchema::default())),
                props: Some(
                    [("a".to_owned(), SerdeSchema::default())]
                        .iter()
                        .cloned()
                        .collect()
                ),
                opt_props: Some(
                    [("a".to_owned(), SerdeSchema::default())]
                        .iter()
                        .cloned()
                        .collect()
                ),
                values: Some(Box::new(SerdeSchema::default())),
                discriminator: Some(SerdeDiscriminator {
                    tag: "foo".to_owned(),
                    mapping: [("a".to_owned(), SerdeSchema::default())]
                        .iter()
                        .cloned()
                        .collect(),
                }),
                extra: [("extra".to_owned(), json!("foo"))]
                    .iter()
                    .cloned()
                    .collect(),
            }
        );

        let round_trip = serde_json::to_string_pretty(&parsed).expect("failed to serialize json");
        assert_eq!(round_trip, data);
    }
}
