//! JSL schema representations.
//!
//! This module provides both an abstract ([`Schema`](struct.Schema.html)) and a
//! serializable/deserializable ([`SerdeSchema`](struct.SerdeSchema.html))
//! representation of JSL schemas.

use crate::errors::JslError;
use failure::{bail, Error};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet};

/// An abstract representation of a JSL schema.
///
/// This struct is meant for use by validators, code generators, or other
/// high-level processors of schemas. For serialization and deserialization of
/// schemas, instead use [`Serde`](struct.Serde.html).
#[derive(Clone, PartialEq, Debug)]
pub struct Schema {
    defs: Option<HashMap<String, Schema>>,
    form: Box<Form>,
    extra: HashMap<String, Value>,
}

impl Schema {
    /// Construct a new, root schema from a `Serde`.
    pub fn from_serde(mut serde_schema: Serde) -> Result<Self, Error> {
        let mut defs = HashMap::new();
        let serde_defs = serde_schema.defs;
        serde_schema.defs = None;

        for (name, sub_schema) in serde_defs.unwrap_or_default() {
            defs.insert(name, Self::_from_serde(sub_schema)?);
        }

        let mut schema = Self::_from_serde(serde_schema)?;
        schema.defs = Some(defs);

        Self::check_refs(&schema.defs.as_ref().unwrap(), &schema)?;
        for sub_schema in schema.defs.as_ref().unwrap().values() {
            Self::check_refs(&schema.defs.as_ref().unwrap(), &sub_schema)?;
        }

        Ok(schema)
    }

    fn _from_serde(serde_schema: Serde) -> Result<Self, Error> {
        let mut form = Form::Empty;

        if let Some(rxf) = serde_schema.rxf {
            form = Form::Ref(rxf);
        }

        if let Some(typ) = serde_schema.typ {
            if form != Form::Empty {
                bail!(JslError::InvalidForm);
            }

            form = Form::Type(match typ.as_ref() {
                "boolean" => Type::Boolean,
                "number" => Type::Number,
                "string" => Type::String,
                "timestamp" => Type::Timestamp,
                _ => bail!(JslError::InvalidForm),
            });
        }

        if let Some(enm) = serde_schema.enm {
            if form != Form::Empty {
                bail!(JslError::InvalidForm);
            }

            let mut values = HashSet::new();
            for val in enm {
                if values.contains(&val) {
                    bail!(JslError::InvalidForm);
                } else {
                    values.insert(val);
                }
            }

            if values.is_empty() {
                bail!(JslError::InvalidForm);
            }

            form = Form::Enum(values);
        }

        if let Some(elements) = serde_schema.elems {
            if form != Form::Empty {
                bail!(JslError::InvalidForm);
            }

            form = Form::Elements(Self::_from_serde(*elements)?);
        }

        if serde_schema.props.is_some() || serde_schema.opt_props.is_some() {
            if form != Form::Empty {
                bail!(JslError::InvalidForm);
            }

            let has_required = serde_schema.props.is_some();

            let mut required = HashMap::new();
            for (name, sub_schema) in serde_schema.props.unwrap_or_default() {
                required.insert(name, Self::_from_serde(sub_schema)?);
            }

            let mut optional = HashMap::new();
            for (name, sub_schema) in serde_schema.opt_props.unwrap_or_default() {
                if required.contains_key(&name) {
                    bail!(JslError::AmbiguousProperty { property: name });
                }

                optional.insert(name, Self::_from_serde(sub_schema)?);
            }

            form = Form::Properties(required, optional, has_required);
        }

        if let Some(values) = serde_schema.values {
            if form != Form::Empty {
                bail!(JslError::InvalidForm);
            }

            form = Form::Values(Self::_from_serde(*values)?);
        }

        if let Some(discriminator) = serde_schema.discriminator {
            if form != Form::Empty {
                bail!(JslError::InvalidForm);
            }

            let mut mapping = HashMap::new();
            for (name, sub_schema) in discriminator.mapping {
                let sub_schema = Self::_from_serde(sub_schema)?;
                match sub_schema.form.as_ref() {
                    Form::Properties(required, optional, _) => {
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

        Ok(Self {
            defs: None,
            form: Box::new(form),
            extra: serde_schema.extra,
        })
    }

    fn check_refs(defs: &HashMap<String, Schema>, schema: &Schema) -> Result<(), Error> {
        match schema.form() {
            Form::Ref(ref def) => {
                if !defs.contains_key(def) {
                    bail!(JslError::NoSuchDefinition {
                        definition: def.clone()
                    })
                }
            }
            Form::Elements(ref schema) => {
                Self::check_refs(defs, schema)?;
            }
            Form::Properties(ref required, ref optional, _) => {
                for schema in required.values() {
                    Self::check_refs(defs, schema)?;
                }

                for schema in optional.values() {
                    Self::check_refs(defs, schema)?;
                }
            }
            Form::Values(ref schema) => {
                Self::check_refs(defs, schema)?;
            }
            Form::Discriminator(_, ref mapping) => {
                for schema in mapping.values() {
                    Self::check_refs(defs, schema)?;
                }
            }
            _ => {}
        };

        Ok(())
    }

    /// Is this schema a root schema?
    ///
    /// Under the hood, this is entirely equivalent to checking whether
    /// `defs().is_some()`.
    pub fn is_root(&self) -> bool {
        self.defs.is_some()
    }

    /// Get the definitions associated with this schema.
    ///
    /// If this schema is non-root, this returns None.
    pub fn definitions(&self) -> &Option<HashMap<String, Schema>> {
        &self.defs
    }

    /// Get the form of the schema.
    pub fn form(&self) -> &Form {
        &self.form
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
    /// schema does. The contained string is the name of the definition of the
    /// referred-to schema -- it is an index into the `defs` of the root schema.
    Ref(String),

    /// The type form.
    ///
    /// This schema asserts that the data is one of the primitive types.
    Type(Type),

    /// The enum form.
    ///
    /// This schema asserts that the data is a string, and that it is one of a
    /// set of values.
    Enum(HashSet<String>),

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
    ///
    /// The final property indicates whether `properties` exists on the schema.
    /// This allows implementations to distinguish the case of an empty
    /// `properties` field from an omitted one. This is necessary for tooling
    /// which wants to link to a particular part of a schema in JSON form.
    Properties(HashMap<String, Schema>, HashMap<String, Schema>, bool),

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
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Type {
    /// The "true" or "false" JSON values.
    Boolean,

    /// Any JSON number.
    ///
    /// Note that JSON only has one kind of number, and JSON numbers may have a
    /// decimal part.
    Number,

    /// Any JSON string.
    String,

    /// A string encoding an RFC3339 timestamp.
    Timestamp,
}

/// A serialization/deserialization-friendly representation of a JSL schema.
///
/// This struct is meant for use with the `serde` crate. It is excellent for
/// parsing from various data formats, but does not enforce all the semantic
/// rules about how schemas must be formed. For that, consider converting
/// instances of `Serde` into [`Schema`](struct.Schema.html) using
/// [`Schema::from_serde`](struct.Schema.html#method.from_serde).
#[derive(Debug, PartialEq, Deserialize, Serialize, Default, Clone)]
pub struct Serde {
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "definitions")]
    pub defs: Option<HashMap<String, Serde>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "ref")]
    pub rxf: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "type")]
    pub typ: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "enum")]
    pub enm: Option<Vec<String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "elements")]
    pub elems: Option<Box<Serde>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "properties")]
    pub props: Option<HashMap<String, Serde>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "optionalProperties")]
    pub opt_props: Option<HashMap<String, Serde>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub values: Option<Box<Serde>>,

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
    pub mapping: HashMap<String, Serde>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn roundtrip_json() {
        let data = r#"{
  "definitions": {
    "a": {}
  },
  "ref": "http://example.com/bar",
  "type": "foo",
  "enum": [
    "FOO",
    "BAR"
  ],
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

        let parsed: Serde = serde_json::from_str(data).expect("failed to parse json");
        assert_eq!(
            parsed,
            Serde {
                rxf: Some("http://example.com/bar".to_owned()),
                defs: Some(
                    [("a".to_owned(), Serde::default())]
                        .iter()
                        .cloned()
                        .collect()
                ),
                typ: Some("foo".to_owned()),
                enm: Some(vec!["FOO".to_owned(), "BAR".to_owned()]),
                elems: Some(Box::new(Serde::default())),
                props: Some(
                    [("a".to_owned(), Serde::default())]
                        .iter()
                        .cloned()
                        .collect()
                ),
                opt_props: Some(
                    [("a".to_owned(), Serde::default())]
                        .iter()
                        .cloned()
                        .collect()
                ),
                values: Some(Box::new(Serde::default())),
                discriminator: Some(SerdeDiscriminator {
                    tag: "foo".to_owned(),
                    mapping: [("a".to_owned(), Serde::default())]
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
                    "definitions": {
                        "a": { "type": "boolean" }
                    }
                }))
                .unwrap()
            )
            .unwrap(),
            Schema {
                defs: Some(
                    [(
                        "a".to_owned(),
                        Schema {
                            defs: None,
                            form: Box::new(Form::Type(Type::Boolean)),
                            extra: HashMap::new(),
                        },
                    )]
                    .iter()
                    .cloned()
                    .collect()
                ),
                form: Box::new(Form::Empty),
                extra: HashMap::new(),
            }
        );
    }

    #[test]
    fn from_serde_empty() {
        assert_eq!(
            Schema::from_serde(serde_json::from_value(json!({})).unwrap()).unwrap(),
            Schema {
                defs: Some(HashMap::new()),
                form: Box::new(Form::Empty),
                extra: HashMap::new(),
            }
        );
    }

    #[test]
    fn from_serde_extra() {
        assert_eq!(
            Schema::from_serde(serde_json::from_value(json!({ "foo": "bar" })).unwrap()).unwrap(),
            Schema {
                defs: Some(HashMap::new()),
                form: Box::new(Form::Empty),
                extra: serde_json::from_value(json!({ "foo": "bar" })).unwrap(),
            }
        );
    }

    #[test]
    fn from_serde_ref() {
        assert_eq!(
            Schema::from_serde(
                serde_json::from_value(json!({
                    "definitions": {
                        "a": { "type": "boolean" }
                    },
                    "ref": "a",
                }))
                .unwrap()
            )
            .unwrap(),
            Schema {
                defs: Some(
                    [(
                        "a".to_owned(),
                        Schema {
                            defs: None,
                            form: Box::new(Form::Type(Type::Boolean)),
                            extra: HashMap::new(),
                        },
                    )]
                    .iter()
                    .cloned()
                    .collect()
                ),
                form: Box::new(Form::Ref("a".to_owned())),
                extra: HashMap::new(),
            }
        );

        assert!(Schema::from_serde(
            serde_json::from_value(json!({
                "definitions": {
                    "a": { "type": "boolean" }
                },
                "ref": "",
            }))
            .unwrap()
        )
        .is_err());
    }

    #[test]
    fn from_serde_type() {
        assert_eq!(
            Schema::from_serde(
                serde_json::from_value(json!({
                    "type": "boolean",
                }))
                .unwrap()
            )
            .unwrap(),
            Schema {
                defs: Some(HashMap::new()),
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
                defs: Some(HashMap::new()),
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
                defs: Some(HashMap::new()),
                form: Box::new(Form::Type(Type::String)),
                extra: HashMap::new(),
            },
        );

        assert_eq!(
            Schema::from_serde(
                serde_json::from_value(json!({
                    "type": "timestamp",
                }))
                .unwrap()
            )
            .unwrap(),
            Schema {
                defs: Some(HashMap::new()),
                form: Box::new(Form::Type(Type::Timestamp)),
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
    fn from_serde_enum() {
        assert_eq!(
            Schema::from_serde(
                serde_json::from_value(json!({
                    "enum": ["FOO", "BAR"],
                }))
                .unwrap()
            )
            .unwrap(),
            Schema {
                defs: Some(HashMap::new()),
                form: Box::new(Form::Enum(
                    vec!["FOO".to_owned(), "BAR".to_owned()]
                        .iter()
                        .cloned()
                        .collect()
                )),
                extra: HashMap::new(),
            },
        );

        assert!(Schema::from_serde(
            serde_json::from_value(json!({
                "enum": [],
            }))
            .unwrap()
        )
        .is_err());

        assert!(Schema::from_serde(
            serde_json::from_value(json!({
                "enum": ["FOO", "FOO"],
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
                        "type": "boolean",
                    },
                }))
                .unwrap()
            )
            .unwrap(),
            Schema {
                defs: Some(HashMap::new()),
                form: Box::new(Form::Elements(Schema {
                    defs: None,
                    form: Box::new(Form::Type(Type::Boolean)),
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
                        "a": { "type": "boolean" },
                    },
                    "optionalProperties": {
                        "b": { "type": "boolean" },
                    },
                }))
                .unwrap()
            )
            .unwrap(),
            Schema {
                defs: Some(HashMap::new()),
                form: Box::new(Form::Properties(
                    [(
                        "a".to_owned(),
                        Schema {
                            defs: None,
                            form: Box::new(Form::Type(Type::Boolean)),
                            extra: HashMap::new(),
                        }
                    )]
                    .iter()
                    .cloned()
                    .collect(),
                    [(
                        "b".to_owned(),
                        Schema {
                            defs: None,
                            form: Box::new(Form::Type(Type::Boolean)),
                            extra: HashMap::new(),
                        }
                    )]
                    .iter()
                    .cloned()
                    .collect(),
                    true,
                )),
                extra: HashMap::new(),
            }
        );

        assert_eq!(
            Schema::from_serde(
                serde_json::from_value(json!({
                    "optionalProperties": {
                        "b": { "type": "boolean" },
                    },
                }))
                .unwrap()
            )
            .unwrap(),
            Schema {
                defs: Some(HashMap::new()),
                form: Box::new(Form::Properties(
                    HashMap::new(),
                    [(
                        "b".to_owned(),
                        Schema {
                            defs: None,
                            form: Box::new(Form::Type(Type::Boolean)),
                            extra: HashMap::new(),
                        }
                    )]
                    .iter()
                    .cloned()
                    .collect(),
                    false,
                )),
                extra: HashMap::new(),
            }
        );

        assert!(Schema::from_serde(
            serde_json::from_value(json!({
                "properties": {
                    "a": { "type": "boolean" },
                },
                "optionalProperties": {
                    "a": { "type": "boolean" },
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
                        "type": "boolean",
                    },
                }))
                .unwrap()
            )
            .unwrap(),
            Schema {
                defs: Some(HashMap::new()),
                form: Box::new(Form::Values(Schema {
                    defs: None,
                    form: Box::new(Form::Type(Type::Boolean)),
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
                defs: Some(HashMap::new()),
                form: Box::new(Form::Discriminator(
                    "foo".to_owned(),
                    [
                        (
                            "a".to_owned(),
                            Schema {
                                defs: None,
                                form: Box::new(Form::Properties(
                                    HashMap::new(),
                                    HashMap::new(),
                                    true
                                )),
                                extra: HashMap::new(),
                            }
                        ),
                        (
                            "b".to_owned(),
                            Schema {
                                defs: None,
                                form: Box::new(Form::Properties(
                                    HashMap::new(),
                                    HashMap::new(),
                                    true
                                )),
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
                        "a": { "type": "boolean" },
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
                                "foo": { "type": "boolean" },
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
