//! JSL schema representations.
//!
//! This module provides both an abstract ([`Schema`](struct.Schema.html)) and a
//! serializable/deserializable ([`SerdeSchema`](struct.SerdeSchema.html))
//! representation of JSL schemas.

use crate::errors::JslError;
use failure::{bail, Error};
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
#[derive(Clone, PartialEq, Debug)]
pub struct Schema {
    root: Option<RootData>,
    form: Box<Form>,
    extra: HashMap<String, Value>,
}

impl Schema {
    /// Construct a new, non-root, empty-form schema without any extra data.
    pub fn new_empty() -> Schema {
        Schema {
            root: None,
            form: Box::new(Form::Empty),
            extra: HashMap::new(),
        }
    }

    /// Construct a new, root schema from a SerdeSchema.
    pub fn from_serde(serde_schema: SerdeSchema) -> Result<Schema, Error> {
        let base = if let Some(ref id) = serde_schema.id {
            Some(id.parse()?)
        } else {
            None
        };

        Self::_from_serde(&base, true, serde_schema)
    }

    fn _from_serde(
        base: &Option<Url>,
        root: bool,
        serde_schema: SerdeSchema,
    ) -> Result<Schema, Error> {
        let root = if root {
            let id = if let Some(id) = serde_schema.id {
                Some(Url::parse(&id)?)
            } else {
                None
            };

            let defs = if let Some(defs) = serde_schema.defs {
                let mut out = HashMap::new();
                for (name, sub_schema) in defs {
                    out.insert(name, Self::_from_serde(base, false, sub_schema)?);
                }

                out
            } else {
                HashMap::new()
            };

            Some(RootData { id, defs })
        } else {
            None
        };

        let mut form = Form::Empty;

        if let Some(rxf) = serde_schema.rxf {
            let (uri, def) = if let Some(ref base) = base {
                let mut resolved = base.join(&rxf)?;
                let frag = resolved.fragment().map(|f| f.to_owned());
                resolved.set_fragment(None);

                (Some(resolved), frag)
            } else {
                // There is no base URI. Either the reference is intra-document
                // (just a fragment, and thus is empty or starts with "#"), or
                // it can be parsed as an absolute URI.
                if rxf.is_empty() || rxf == "#" {
                    (None, None)
                } else if rxf.starts_with("#") {
                    (None, Some(rxf[1..].to_owned()))
                } else {
                    let mut resolved: Url = rxf.parse()?;
                    let frag = resolved.fragment().map(|f| f.to_owned());
                    resolved.set_fragment(None);

                    (Some(resolved), frag)
                }
            };

            form = Form::Ref(uri, def)
        }

        if let Some(typ) = serde_schema.typ {
            if form != Form::Empty {
                bail!(JslError::InvalidForm);
            }

            form = Form::Type(match typ.as_ref() {
                "null" => Type::Null,
                "boolean" => Type::Boolean,
                "number" => Type::Number,
                "string" => Type::String,
                _ => bail!(JslError::InvalidForm),
            });
        }

        if let Some(elements) = serde_schema.elems {
            if form != Form::Empty {
                bail!(JslError::InvalidForm);
            }

            form = Form::Elements(Self::_from_serde(base, false, *elements)?);
        }

        if serde_schema.props.is_some() || serde_schema.opt_props.is_some() {
            if form != Form::Empty {
                bail!(JslError::InvalidForm);
            }

            let mut required = HashMap::new();
            for (name, sub_schema) in serde_schema.props.unwrap_or_default() {
                required.insert(name, Self::_from_serde(base, false, sub_schema)?);
            }

            let mut optional = HashMap::new();
            for (name, sub_schema) in serde_schema.opt_props.unwrap_or_default() {
                if required.contains_key(&name) {
                    bail!(JslError::AmbiguousProperty { property: name });
                }

                optional.insert(name, Self::_from_serde(base, false, sub_schema)?);
            }

            form = Form::Properties(required, optional);
        }

        if let Some(values) = serde_schema.values {
            if form != Form::Empty {
                bail!(JslError::InvalidForm);
            }

            form = Form::Values(Self::_from_serde(base, false, *values)?);
        }

        if let Some(discriminator) = serde_schema.discriminator {
            if form != Form::Empty {
                bail!(JslError::InvalidForm);
            }

            let mut mapping = HashMap::new();
            for (name, sub_schema) in discriminator.mapping {
                let sub_schema = Self::_from_serde(base, false, sub_schema)?;
                match sub_schema.form.as_ref() {
                    Form::Properties(required, optional) => {
                        if required.contains_key(&discriminator.tag)
                            || optional.contains_key(&discriminator.tag)
                        {
                            bail!(JslError::AmbiguousProperty {
                                property: discriminator.tag,
                            });
                        }
                    }
                    _ => bail!(JslError::InvalidForm),
                };

                mapping.insert(name, sub_schema);
            }

            form = Form::Discriminator(discriminator.tag, mapping);
        }

        Ok(Schema {
            root,
            form: Box::new(form),
            extra: HashMap::new(),
        })
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
#[derive(Clone, Debug, PartialEq)]
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
#[derive(Clone, Debug, PartialEq)]
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
#[derive(Clone, Debug, PartialEq, Eq)]
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

    #[test]
    fn from_serde_root() {
        assert_eq!(
            Schema::from_serde(
                serde_json::from_value(json!({
                    "id": "http://example.com/foo",
                    "definitions": {
                        "a": { "type": "null" }
                    }
                }))
                .unwrap()
            )
            .unwrap(),
            Schema {
                root: Some(RootData {
                    id: Some("http://example.com/foo".parse().unwrap()),
                    defs: [(
                        "a".to_owned(),
                        Schema {
                            root: None,
                            form: Box::new(Form::Type(Type::Null)),
                            extra: HashMap::new(),
                        },
                    )]
                    .iter()
                    .cloned()
                    .collect(),
                }),
                form: Box::new(Form::Empty),
                extra: HashMap::new()
            }
        );
    }

    #[test]
    fn from_serde_empty() {
        assert_eq!(
            Schema::from_serde(serde_json::from_value(json!({})).unwrap()).unwrap(),
            Schema {
                root: Some(RootData {
                    id: None,
                    defs: HashMap::new(),
                }),
                form: Box::new(Form::Empty),
                extra: HashMap::new(),
            }
        );
    }

    #[test]
    fn from_serde_ref() {
        assert_eq!(
            Schema::from_serde(
                serde_json::from_value(json!({
                    "ref": ""
                }))
                .unwrap()
            )
            .unwrap(),
            Schema {
                root: Some(RootData {
                    id: None,
                    defs: HashMap::new(),
                }),
                form: Box::new(Form::Ref(None, None)),
                extra: HashMap::new(),
            }
        );

        assert_eq!(
            Schema::from_serde(
                serde_json::from_value(json!({
                    "ref": "#"
                }))
                .unwrap()
            )
            .unwrap(),
            Schema {
                root: Some(RootData {
                    id: None,
                    defs: HashMap::new(),
                }),
                form: Box::new(Form::Ref(None, None)),
                extra: HashMap::new(),
            }
        );

        assert_eq!(
            Schema::from_serde(
                serde_json::from_value(json!({
                    "id": "http://example.com/foo",
                    "ref": ""
                }))
                .unwrap()
            )
            .unwrap(),
            Schema {
                root: Some(RootData {
                    id: Some("http://example.com/foo".parse().unwrap()),
                    defs: HashMap::new(),
                }),
                form: Box::new(Form::Ref(
                    Some("http://example.com/foo".parse().unwrap()),
                    None
                )),
                extra: HashMap::new(),
            }
        );

        assert_eq!(
            Schema::from_serde(
                serde_json::from_value(json!({
                    "id": "http://example.com/foo",
                    "ref": "/bar"
                }))
                .unwrap()
            )
            .unwrap(),
            Schema {
                root: Some(RootData {
                    id: Some("http://example.com/foo".parse().unwrap()),
                    defs: HashMap::new(),
                }),
                form: Box::new(Form::Ref(
                    Some("http://example.com/bar".parse().unwrap()),
                    None
                )),
                extra: HashMap::new(),
            }
        );

        assert_eq!(
            Schema::from_serde(
                serde_json::from_value(json!({
                    "id": "http://example.com/foo",
                    "ref": "#asdf"
                }))
                .unwrap()
            )
            .unwrap(),
            Schema {
                root: Some(RootData {
                    id: Some("http://example.com/foo".parse().unwrap()),
                    defs: HashMap::new(),
                }),
                form: Box::new(Form::Ref(
                    Some("http://example.com/foo".parse().unwrap()),
                    Some("asdf".to_owned()),
                )),
                extra: HashMap::new(),
            }
        );

        assert_eq!(
            Schema::from_serde(
                serde_json::from_value(json!({
                    "id": "http://example.com/foo",
                    "ref": "/bar#asdf"
                }))
                .unwrap()
            )
            .unwrap(),
            Schema {
                root: Some(RootData {
                    id: Some("http://example.com/foo".parse().unwrap()),
                    defs: HashMap::new(),
                }),
                form: Box::new(Form::Ref(
                    Some("http://example.com/bar".parse().unwrap()),
                    Some("asdf".to_owned()),
                )),
                extra: HashMap::new(),
            }
        );
    }

    #[test]
    fn from_serde_type() {
        assert_eq!(
            Schema::from_serde(
                serde_json::from_value(json!({
                    "type": "null",
                }))
                .unwrap()
            )
            .unwrap(),
            Schema {
                root: Some(RootData {
                    id: None,
                    defs: HashMap::new(),
                }),
                form: Box::new(Form::Type(Type::Null)),
                extra: HashMap::new(),
            },
        );

        assert_eq!(
            Schema::from_serde(
                serde_json::from_value(json!({
                    "type": "boolean",
                }))
                .unwrap()
            )
            .unwrap(),
            Schema {
                root: Some(RootData {
                    id: None,
                    defs: HashMap::new(),
                }),
                form: Box::new(Form::Type(Type::Boolean)),
                extra: HashMap::new(),
            },
        );

        assert_eq!(
            Schema::from_serde(
                serde_json::from_value(json!({
                    "type": "number",
                }))
                .unwrap()
            )
            .unwrap(),
            Schema {
                root: Some(RootData {
                    id: None,
                    defs: HashMap::new(),
                }),
                form: Box::new(Form::Type(Type::Number)),
                extra: HashMap::new(),
            },
        );

        assert_eq!(
            Schema::from_serde(
                serde_json::from_value(json!({
                    "type": "string",
                }))
                .unwrap()
            )
            .unwrap(),
            Schema {
                root: Some(RootData {
                    id: None,
                    defs: HashMap::new(),
                }),
                form: Box::new(Form::Type(Type::String)),
                extra: HashMap::new(),
            },
        );

        assert!(Schema::from_serde(
            serde_json::from_value(json!({
                "type": "nonsense",
            }))
            .unwrap()
        )
        .is_err());
    }

    #[test]
    fn from_serde_elements() {
        assert_eq!(
            Schema::from_serde(
                serde_json::from_value(json!({
                    "elements": {
                        "type": "null",
                    },
                }))
                .unwrap()
            )
            .unwrap(),
            Schema {
                root: Some(RootData {
                    id: None,
                    defs: HashMap::new(),
                }),
                form: Box::new(Form::Elements(Schema {
                    root: None,
                    form: Box::new(Form::Type(Type::Null)),
                    extra: HashMap::new(),
                })),
                extra: HashMap::new(),
            }
        );
    }

    #[test]
    fn from_serde_properties() {
        assert_eq!(
            Schema::from_serde(
                serde_json::from_value(json!({
                    "properties": {
                        "a": { "type": "null" },
                    },
                    "optionalProperties": {
                        "b": { "type": "null" },
                    },
                }))
                .unwrap()
            )
            .unwrap(),
            Schema {
                root: Some(RootData {
                    id: None,
                    defs: HashMap::new(),
                }),
                form: Box::new(Form::Properties(
                    [(
                        "a".to_owned(),
                        Schema {
                            root: None,
                            form: Box::new(Form::Type(Type::Null)),
                            extra: HashMap::new(),
                        }
                    )]
                    .iter()
                    .cloned()
                    .collect(),
                    [(
                        "b".to_owned(),
                        Schema {
                            root: None,
                            form: Box::new(Form::Type(Type::Null)),
                            extra: HashMap::new(),
                        }
                    )]
                    .iter()
                    .cloned()
                    .collect(),
                )),
                extra: HashMap::new(),
            }
        );

        assert!(Schema::from_serde(
            serde_json::from_value(json!({
                "properties": {
                    "a": { "type": "null" },
                },
                "optionalProperties": {
                    "a": { "type": "null" },
                },
            }))
            .unwrap()
        )
        .is_err());
    }

    #[test]
    fn from_serde_values() {
        assert_eq!(
            Schema::from_serde(
                serde_json::from_value(json!({
                    "values": {
                        "type": "null",
                    },
                }))
                .unwrap()
            )
            .unwrap(),
            Schema {
                root: Some(RootData {
                    id: None,
                    defs: HashMap::new(),
                }),
                form: Box::new(Form::Values(Schema {
                    root: None,
                    form: Box::new(Form::Type(Type::Null)),
                    extra: HashMap::new(),
                })),
                extra: HashMap::new(),
            }
        );
    }

    #[test]
    fn from_serde_discriminator() {
        assert_eq!(
            Schema::from_serde(
                serde_json::from_value(json!({
                    "discriminator": {
                        "tag": "foo",
                        "mapping": {
                            "a": { "properties": {} },
                            "b": { "properties": {} },
                        },
                    },
                }))
                .unwrap()
            )
            .unwrap(),
            Schema {
                root: Some(RootData {
                    id: None,
                    defs: HashMap::new(),
                }),
                form: Box::new(Form::Discriminator(
                    "foo".to_owned(),
                    [
                        (
                            "a".to_owned(),
                            Schema {
                                root: None,
                                form: Box::new(Form::Properties(HashMap::new(), HashMap::new())),
                                extra: HashMap::new(),
                            }
                        ),
                        (
                            "b".to_owned(),
                            Schema {
                                root: None,
                                form: Box::new(Form::Properties(HashMap::new(), HashMap::new())),
                                extra: HashMap::new(),
                            }
                        )
                    ]
                    .iter()
                    .cloned()
                    .collect(),
                )),
                extra: HashMap::new(),
            }
        );

        assert!(Schema::from_serde(
            serde_json::from_value(json!({
                "discriminator": {
                    "tag": "foo",
                    "mapping": {
                        "a": { "type": "null" },
                    }
                },
            }))
            .unwrap()
        )
        .is_err());

        assert!(Schema::from_serde(
            serde_json::from_value(json!({
                "discriminator": {
                    "tag": "foo",
                    "mapping": {
                        "a": {
                            "properties": {
                                "foo": { "type": "null" },
                            },
                        },
                    },
                },
            }))
            .unwrap()
        )
        .is_err());
    }
}
