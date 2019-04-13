use crate::errors::*;
use crate::serde::SerdeSchema;
use serde_json::Value;
use std::collections::HashMap;
use url::Url;

#[derive(Debug, PartialEq)]
pub struct Registry {
    schemas: Vec<Schema>,
}

impl Registry {
    pub fn new() -> Registry {
        Registry {
            schemas: Vec::new(),
        }
    }

    pub fn register<I: IntoIterator<Item = SerdeSchema>>(
        &mut self,
        schemas: I,
    ) -> Result<Vec<Url>> {
        let initial_size = self.schemas.len();

        // To a first pass over all of the schemas.
        for schema in schemas {
            self.schemas.push(Self::first_pass(true, schema)?);
        }

        // With all of the schemas basically valid, let's ensure that all the
        // URIs resolve properly, and precompute the resolved URIs for faster
        // evaluation.
        for (index, schema) in self.schemas.iter_mut().enumerate() {
            let default_base = Url::parse(&format!("urn:jsl:auto:{}", initial_size + index)).unwrap();
            let base = schema
                .root_data
                .as_ref()
                .and_then(|root| root.id.as_ref())
                .unwrap_or(&default_base)
                .clone();

            for sub_schema in schema.root_data.as_mut().unwrap().defs.values_mut() {
                Self::second_pass(&base, sub_schema)?;
            }

            Self::second_pass(&base, schema)?;
        }

        Ok(vec![])
    }

    fn first_pass(is_root: bool, schema: SerdeSchema) -> Result<Schema> {
        let root_data = if is_root {
            let id = if let Some(id) = schema.id {
                Some(Url::parse(&id).chain_err(|| "failed to parse id")?)
            } else {
                None
            };

            let defs = if let Some(defs) = schema.defs {
                let mut first_pass_defs = HashMap::new();
                for (k, sub_schema) in defs {
                    first_pass_defs.insert(k, Self::first_pass(false, sub_schema)?);
                }

                first_pass_defs
            } else {
                HashMap::new()
            };

            Some(RootData { id, defs })
        } else {
            None
        };

        let mut form = SchemaForm::Empty;
        if let Some(rxf) = schema.rxf {
            form = SchemaForm::Ref {
                uri: rxf,
                resolved_uri: None,
            }
        }

        if let Some(typ) = schema.typ {
            if form != SchemaForm::Empty {
                return Ok(Err(ErrorKind::InvalidForm)?);
            }

            form = SchemaForm::Type(match typ.as_ref() {
                "null" => Ok(PrimitiveType::Null),
                "boolean" => Ok(PrimitiveType::Bool),
                "number" => Ok(PrimitiveType::Num),
                "string" => Ok(PrimitiveType::Str),
                _ => Err(ErrorKind::InvalidForm),
            }?);
        }

        if let Some(elems) = schema.elems {
            if form != SchemaForm::Empty {
                return Ok(Err(ErrorKind::InvalidForm)?);
            }

            form = SchemaForm::Elements(Box::new(Self::first_pass(false, *elems)?));
        }

        if schema.props.is_some() || schema.opt_props.is_some() {
            let mut required = HashMap::new();
            let mut optional = HashMap::new();

            for (name, sub_schema) in schema.props.unwrap_or_default() {
                required.insert(name, Self::first_pass(false, sub_schema)?);
            }

            for (name, sub_schema) in schema.opt_props.unwrap_or_default() {
                if required.contains_key(&name) {
                    return Ok(Err(ErrorKind::AmbiguousProperty)?);
                }

                optional.insert(name, Self::first_pass(false, sub_schema)?);
            }

            form = SchemaForm::Properties { required, optional };
        }

        if let Some(values) = schema.values {
            if form != SchemaForm::Empty {
                return Ok(Err(ErrorKind::InvalidForm)?);
            }

            form = SchemaForm::Values(Box::new(Self::first_pass(false, *values)?));
        }

        if let Some(discriminator) = schema.discriminator {
            if form != SchemaForm::Empty {
                return Ok(Err(ErrorKind::InvalidForm)?);
            }

            let mut mapping = HashMap::new();
            for (name, sub_schema) in discriminator.mapping {
                let parsed = Self::first_pass(false, sub_schema)?;
                match parsed.form {
                    SchemaForm::Properties {
                        ref required,
                        ref optional,
                    } => {
                        if required.contains_key(&discriminator.tag)
                            || optional.contains_key(&discriminator.tag)
                        {
                            return Ok(Err(ErrorKind::AmbiguousProperty)?);
                        }
                    }
                    _ => {
                        return Ok(Err(ErrorKind::AmbiguousProperty)?);
                    }
                }

                mapping.insert(name, parsed);
            }

            form = SchemaForm::Discriminator {
                tag: discriminator.tag,
                mapping,
            }
        }

        Ok(Schema {
            root_data,
            form,
            extra: schema.extra,
        })
    }

    fn second_pass(base: &Url, schema: &mut Schema) -> Result<()> {
        match schema.form {
            SchemaForm::Ref {
                ref uri,
                ref mut resolved_uri,
            } => {
                println!("resolving uri: {:?} {:?}", base, uri);
                *resolved_uri = Some(base.join(uri).chain_err(|| "cannot resolve uri")?);
            }
            SchemaForm::Elements(ref mut elems) => {
                Self::second_pass(base, elems)?;
            }
            SchemaForm::Properties {
                ref mut required,
                ref mut optional,
            } => {
                for sub_schema in required.values_mut() {
                    Self::second_pass(base, sub_schema)?;
                }

                for sub_schema in optional.values_mut() {
                    Self::second_pass(base, sub_schema)?;
                }
            }
            SchemaForm::Values(ref mut values) => {
                Self::second_pass(base, values)?;
            }
            SchemaForm::Discriminator {
                ref mut mapping, ..
            } => {
                for sub_schema in mapping.values_mut() {
                    Self::second_pass(base, sub_schema)?;
                }
            }
            _ => {}
        }

        Ok(())
    }
}

#[derive(PartialEq, Debug, Clone)]
pub struct Schema {
    root_data: Option<RootData>,
    form: SchemaForm,
    extra: HashMap<String, Value>,
}

#[derive(PartialEq, Debug, Clone)]
pub struct RootData {
    id: Option<Url>,
    defs: HashMap<String, Schema>,
}

#[derive(PartialEq, Debug, Clone)]
pub enum SchemaForm {
    Empty,
    Ref {
        uri: String,
        resolved_uri: Option<Url>,
    },
    Type(PrimitiveType),
    Elements(Box<Schema>),
    Properties {
        required: HashMap<String, Schema>,
        optional: HashMap<String, Schema>,
    },
    Values(Box<Schema>),
    Discriminator {
        tag: String,
        mapping: HashMap<String, Schema>,
    },
}

#[derive(PartialEq, Debug, Clone)]
pub enum PrimitiveType {
    Null,
    Bool,
    Num,
    Str,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::serde::SerdeDiscriminator;

    #[test]
    fn first_pass_root() {
        assert_eq!(
            Registry::first_pass(
                true,
                SerdeSchema {
                    id: Some("http://example.com/foo".to_owned()),
                    defs: Some(
                        [("a".to_owned(), SerdeSchema::default())]
                            .iter()
                            .cloned()
                            .collect()
                    ),
                    ..SerdeSchema::default()
                }
            )
            .expect("failed to run first_pass"),
            Schema {
                root_data: Some(RootData {
                    id: Some(Url::parse("http://example.com/foo").unwrap()),
                    defs: [(
                        "a".to_owned(),
                        Schema {
                            root_data: None,
                            form: SchemaForm::Empty,
                            extra: HashMap::new(),
                        }
                    )]
                    .iter()
                    .cloned()
                    .collect(),
                }),
                form: SchemaForm::Empty,
                extra: HashMap::new(),
            },
        );
    }

    #[test]
    fn first_pass_ref_form() {
        assert_eq!(
            Registry::first_pass(
                true,
                SerdeSchema {
                    rxf: Some("http://example.com/bar".to_owned()),
                    ..SerdeSchema::default()
                }
            )
            .expect("failed to run first_pass"),
            Schema {
                root_data: Some(RootData {
                    id: None,
                    defs: HashMap::new(),
                }),
                form: SchemaForm::Ref {
                    uri: "http://example.com/bar".to_owned(),
                    resolved_uri: None,
                },
                extra: HashMap::new(),
            },
        );
    }

    #[test]
    fn first_pass_type_form() {
        assert_eq!(
            Registry::first_pass(
                true,
                SerdeSchema {
                    typ: Some("boolean".to_owned()),
                    ..SerdeSchema::default()
                }
            )
            .expect("failed to run first_pass"),
            Schema {
                root_data: Some(RootData {
                    id: None,
                    defs: HashMap::new(),
                }),
                form: SchemaForm::Type(PrimitiveType::Bool),
                extra: HashMap::new(),
            },
        );
    }

    #[test]
    fn first_pass_elems_form() {
        assert_eq!(
            Registry::first_pass(
                true,
                SerdeSchema {
                    elems: Some(Box::new(SerdeSchema::default())),
                    ..SerdeSchema::default()
                }
            )
            .expect("failed to run first_pass"),
            Schema {
                root_data: Some(RootData {
                    id: None,
                    defs: HashMap::new(),
                }),
                form: SchemaForm::Elements(Box::new(Schema {
                    root_data: None,
                    form: SchemaForm::Empty,
                    extra: HashMap::new(),
                })),
                extra: HashMap::new(),
            },
        );
    }

    #[test]
    fn first_pass_props_form() {
        assert_eq!(
            Registry::first_pass(
                true,
                SerdeSchema {
                    props: Some(
                        [("a".to_owned(), SerdeSchema::default())]
                            .iter()
                            .cloned()
                            .collect()
                    ),
                    opt_props: Some(
                        [("b".to_owned(), SerdeSchema::default())]
                            .iter()
                            .cloned()
                            .collect()
                    ),
                    ..SerdeSchema::default()
                }
            )
            .expect("failed to run first_pass"),
            Schema {
                root_data: Some(RootData {
                    id: None,
                    defs: HashMap::new(),
                }),
                form: SchemaForm::Properties {
                    required: [(
                        "a".to_owned(),
                        Schema {
                            root_data: None,
                            form: SchemaForm::Empty,
                            extra: HashMap::new(),
                        }
                    )]
                    .iter()
                    .cloned()
                    .collect(),
                    optional: [(
                        "b".to_owned(),
                        Schema {
                            root_data: None,
                            form: SchemaForm::Empty,
                            extra: HashMap::new(),
                        }
                    )]
                    .iter()
                    .cloned()
                    .collect(),
                },
                extra: HashMap::new(),
            },
        );
    }

    #[test]
    fn first_pass_values_form() {
        assert_eq!(
            Registry::first_pass(
                true,
                SerdeSchema {
                    values: Some(Box::new(SerdeSchema::default())),
                    ..SerdeSchema::default()
                }
            )
            .expect("failed to run first_pass"),
            Schema {
                root_data: Some(RootData {
                    id: None,
                    defs: HashMap::new(),
                }),
                form: SchemaForm::Values(Box::new(Schema {
                    root_data: None,
                    form: SchemaForm::Empty,
                    extra: HashMap::new(),
                })),
                extra: HashMap::new(),
            },
        );
    }

    #[test]
    fn first_pass_discriminator_form() {
        assert_eq!(
            Registry::first_pass(
                true,
                SerdeSchema {
                    discriminator: Some(SerdeDiscriminator {
                        tag: "foo".to_owned(),
                        mapping: [(
                            "a".to_owned(),
                            SerdeSchema {
                                props: Some(
                                    [("a".to_owned(), SerdeSchema::default())]
                                        .iter()
                                        .cloned()
                                        .collect(),
                                ),
                                ..SerdeSchema::default()
                            },
                        )]
                        .iter()
                        .cloned()
                        .collect(),
                    }),
                    ..SerdeSchema::default()
                }
            )
            .expect("failed to run first_pass"),
            Schema {
                root_data: Some(RootData {
                    id: None,
                    defs: HashMap::new(),
                }),
                form: SchemaForm::Discriminator {
                    tag: "foo".to_owned(),
                    mapping: [(
                        "a".to_owned(),
                        Schema {
                            root_data: None,
                            form: SchemaForm::Properties {
                                required: [(
                                    "a".to_owned(),
                                    Schema {
                                        root_data: None,
                                        form: SchemaForm::Empty,
                                        extra: HashMap::new(),
                                    }
                                )]
                                .iter()
                                .cloned()
                                .collect(),
                                optional: HashMap::new(),
                            },
                            extra: HashMap::new(),
                        }
                    )]
                    .iter()
                    .cloned()
                    .collect(),
                },
                extra: HashMap::new(),
            },
        );
    }

    #[test]
    fn amibguous_props_form() {
        let err = Registry::first_pass(
            true,
            SerdeSchema {
                props: Some(
                    [("a".to_owned(), SerdeSchema::default())]
                        .iter()
                        .cloned()
                        .collect(),
                ),
                opt_props: Some(
                    [("a".to_owned(), SerdeSchema::default())]
                        .iter()
                        .cloned()
                        .collect(),
                ),
                ..SerdeSchema::default()
            },
        )
        .expect_err("no error for ambiguous schema");

        match err {
            Error(ErrorKind::AmbiguousProperty, _) => {}
            _ => panic!("wrong error produced"),
        };
    }

    #[test]
    fn ambiguous_discriminator_form_by_non_props() {
        let err = Registry::first_pass(
            true,
            SerdeSchema {
                discriminator: Some(SerdeDiscriminator {
                    tag: "foo".to_owned(),
                    mapping: [("a".to_owned(), SerdeSchema::default())]
                        .iter()
                        .cloned()
                        .collect(),
                }),
                ..SerdeSchema::default()
            },
        )
        .expect_err("no error for ambiguous schema");

        match err {
            Error(ErrorKind::AmbiguousProperty, _) => {}
            _ => panic!("wrong error produced"),
        };
    }

    #[test]
    fn ambiguous_discriminator_form_by_props() {
        let err = Registry::first_pass(
            true,
            SerdeSchema {
                discriminator: Some(SerdeDiscriminator {
                    tag: "foo".to_owned(),
                    mapping: [(
                        "a".to_owned(),
                        SerdeSchema {
                            props: Some(
                                [("foo".to_owned(), SerdeSchema::default())]
                                    .iter()
                                    .cloned()
                                    .collect(),
                            ),
                            ..SerdeSchema::default()
                        },
                    )]
                    .iter()
                    .cloned()
                    .collect(),
                }),
                ..SerdeSchema::default()
            },
        )
        .expect_err("no error for ambiguous schema");

        match err {
            Error(ErrorKind::AmbiguousProperty, _) => {}
            _ => panic!("wrong error produced"),
        };
    }

    #[test]
    fn resolve_refs() {
        let mut registry = Registry::new();
        assert_eq!(
            registry
                .register(vec![
                    SerdeSchema {
                        id: Some("http://example.com/foo".to_owned()),
                        defs: Some(
                            [("a".to_owned(), SerdeSchema::default())]
                                .iter()
                                .cloned()
                                .collect()
                        ),
                        rxf: Some("#a".to_owned()),
                        ..SerdeSchema::default()
                    },
                    SerdeSchema {
                        id: None,
                        defs: Some(
                            [("a".to_owned(), SerdeSchema{
                                rxf: Some("#a".to_owned()),
                                ..SerdeSchema::default()
                            })]
                                .iter()
                                .cloned()
                                .collect()
                        ),
                        rxf: Some("http://example.com/foo#a".to_owned()),
                        ..SerdeSchema::default()
                    }
                ])
                .expect("error while registering schema"),
            vec![]
        );

        assert_eq!(
            registry,
            Registry {
                schemas: vec![
                    Schema {
                        root_data: Some(RootData {
                            id: Some(Url::parse("http://example.com/foo").unwrap()),
                            defs: [(
                                "a".to_owned(),
                                Schema {
                                    root_data: None,
                                    form: SchemaForm::Empty,
                                    extra: HashMap::new(),
                                }
                            ),
                            ]
                            .iter()
                            .cloned()
                            .collect(),
                        }),
                        form: SchemaForm::Ref {
                            uri: "#a".to_owned(),
                            resolved_uri: Some(Url::parse("http://example.com/foo#a").unwrap()),
                        },
                        extra: HashMap::new(),
                    },
                    Schema {
                        root_data: Some(RootData {
                            id: None,
                            defs: [("a".to_owned(), Schema{
                                root_data: None,
                                form: SchemaForm::Ref {
                                    uri: "#a".to_owned(),
                                    resolved_uri: Some(Url::parse("urn:jsl:auto:1#a").unwrap()),
                                },
                                    extra: HashMap::new(),
                            })].iter().cloned().collect(),
                        }),
                        form: SchemaForm::Ref {
                            uri: "http://example.com/foo#a".to_owned(),
                            resolved_uri: Some(Url::parse("http://example.com/foo#a").unwrap()),
                        },
                        extra: HashMap::new(),
                    },
                ]
            }
        );
    }
}
