use crate::errors::*;
use crate::serde::SerdeSchema;
use crate::vm::validate;
use json_pointer::JsonPointer;
use serde_json::Value;
use std::collections::HashMap;
use url::Url;

#[derive(Debug, PartialEq)]
pub struct Registry {
    schemas: HashMap<Option<Url>, Schema>,
}

// pub struct ValidationResult {
//     pub failures: Vec<ValidationFailure>,
// }

pub struct ValidationFailure {
    pub instance_path: JsonPointer<String, Vec<String>>,
    pub schema_path: JsonPointer<String, Vec<String>>,
    pub schema_uri: Option<Url>,
}

impl Registry {
    pub fn new() -> Registry {
        Registry {
            schemas: HashMap::new(),
        }
    }

    pub fn validate(&self, instance: Value) -> Vec<ValidationFailure> {
        Vec::new()
    }

    pub fn register<I: IntoIterator<Item = SerdeSchema>>(
        &mut self,
        schemas: I,
    ) -> Result<Vec<Url>> {
        let initial_size = self.schemas.len();

        // To a first pass over all of the schemas.
        for schema in schemas {
            let schema = Self::first_pass(true, schema)?;
            self.schemas
                .insert(schema.root_data.as_ref().unwrap().id.clone(), schema);
        }

        // With all of the schemas basically valid, let's ensure that all the
        // URIs resolve properly, and precompute the resolved URIs for faster
        // evaluation.
        for (_, schema) in self.schemas.values_mut().enumerate() {
            // let default_base = Url::parse(&format!("urn:jsl:auto:{}", initial_size)).unwrap();
            // let base = schema.root_data.and_then(|root| root.id);
            let base = if let Some(ref root) = schema.root_data {
                root.id.clone()
            } else {
                None
            };

            // .unwrap_or(&default_base)
            // .clone();

            for sub_schema in schema.root_data.as_mut().unwrap().defs.values_mut() {
                Self::second_pass(base.as_ref(), sub_schema)?;
            }

            Self::second_pass(base.as_ref(), schema)?;
        }

        let mut missing_uris = Vec::new();
        for schema in self.schemas.values() {
            Self::third_pass(&mut missing_uris, &self.schemas, schema, schema)?;
        }

        Ok(missing_uris)
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
                resolved_schema_id: None,
                resolved_schema_def: None,
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

    fn second_pass(base: Option<&Url>, schema: &mut Schema) -> Result<()> {
        match schema.form {
            SchemaForm::Ref {
                ref uri,
                ref mut resolved_schema_id,
                ref mut resolved_schema_def,
            } => {
                // The url crate does not handle parsing relative references. We
                // therefore handle valid cases of relative references directly
                // here.
                if uri == "" || uri == "#" {
                    *resolved_schema_id = None; // indicates "same-document"
                    *resolved_schema_def = None; // indicates "root"
                } else if uri.starts_with("#") {
                    *resolved_schema_id = None; // indicates "same-document"
                    *resolved_schema_def = Some(uri[1..].to_owned());
                } else {
                    let mut resolved = if let Some(base) = base {
                        base.join(uri).chain_err(|| "cannot resolve uri")?
                    } else {
                        Url::parse(uri).chain_err(|| "cannot resolve uri")?
                    };

                    *resolved_schema_def = resolved.fragment().and_then(|f| {
                        if f == "" {
                            None
                        } else {
                            Some(f.to_owned())
                        }
                    });
                    resolved.set_fragment(None);
                    *resolved_schema_id = Some(resolved);
                }
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

    fn third_pass(
        missing_uris: &mut Vec<Url>,
        schemas: &HashMap<Option<Url>, Schema>,
        root_schema: &Schema,
        schema: &Schema,
    ) -> Result<()> {
        match schema.form {
            SchemaForm::Ref {
                ref resolved_schema_id,
                ref resolved_schema_def,
                ref uri,
                // ref resolved_uri,
                // ref uri,
            } => {
                let resolved_schema = if let Some(id) = resolved_schema_id {
                    if let Some(s) = schemas.get(resolved_schema_id) {
                        s
                    } else {
                        missing_uris.push(id.clone());
                        return Ok(());
                    }
                } else {
                    schema
                };

                if let Some(def) = resolved_schema_def {
                    if !resolved_schema
                        .root_data
                        .as_ref()
                        .unwrap()
                        .defs
                        .contains_key(def)
                    {
                        return Ok(Err(ErrorKind::NoSuchDefinition)?);
                    }
                }

                // let ref_uri = resolved_uri.as_ref().unwrap();
                // let ref_uri_frag = ref_uri.fragment();

                // let mut uri_absolute = ref_uri.clone();
                // uri_absolute.set_fragment(None);

                // // This is a janky way to detect URIs that are intra-document --
                // // i.e., just a fragment. These references always resolve to a
                // // root schema, even if the root schema lacks an ID.
                // //
                // // The case of a schema referring to itself using its "public"
                // // ID is handled by the case below.
                // if uri.starts_with("#") {
                //     println!("URI is intra!");

                //     if let Some(frag) = ref_uri_frag {
                //         if !frag.is_empty()
                //             && !root_schema
                //                 .root_data
                //                 .as_ref()
                //                 .unwrap()
                //                 .defs
                //                 .contains_key(frag)
                //         {
                //             missing_uris.push(ref_uri.clone());
                //         }
                //     }
                // } else {
                //     let mut found = false;
                //     for schema in schemas.values() {
                //         if let Some(id) = schema.root_data.as_ref().unwrap().id.as_ref() {
                //             if id == &uri_absolute {
                //                 found = true;

                //                 if let Some(frag) = ref_uri_frag {
                //                     if !frag.is_empty()
                //                         && !schema
                //                             .root_data
                //                             .as_ref()
                //                             .unwrap()
                //                             .defs
                //                             .contains_key(frag)
                //                     {
                //                         missing_uris.push(ref_uri.clone());
                //                     }
                //                 }
                //             }
                //         }
                //     }

                //     if !found {
                //         missing_uris.push(ref_uri.clone());
                //     }
                // }
            }
            SchemaForm::Elements(ref elems) => {
                Self::third_pass(missing_uris, schemas, root_schema, elems)?;
            }
            SchemaForm::Properties {
                ref required,
                ref optional,
            } => {
                for sub_schema in required.values() {
                    Self::third_pass(missing_uris, schemas, root_schema, sub_schema)?;
                }

                for sub_schema in optional.values() {
                    Self::third_pass(missing_uris, schemas, root_schema, sub_schema)?;
                }
            }
            SchemaForm::Values(ref values) => {
                Self::third_pass(missing_uris, schemas, root_schema, values)?;
            }
            SchemaForm::Discriminator { ref mapping, .. } => {
                for sub_schema in mapping.values() {
                    Self::third_pass(missing_uris, schemas, root_schema, sub_schema)?;
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
        resolved_schema_id: Option<Url>,
        resolved_schema_def: Option<String>,
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
    use pretty_assertions::assert_eq;

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
                    resolved_schema_id: None,
                    resolved_schema_def: None,
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
                            [(
                                "a".to_owned(),
                                SerdeSchema {
                                    rxf: Some("#a".to_owned()),
                                    ..SerdeSchema::default()
                                }
                            )]
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
                    (
                        Some(Url::parse("http://example.com/foo").unwrap()),
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
                            form: SchemaForm::Ref {
                                uri: "#a".to_owned(),
                                resolved_schema_id: None,
                                resolved_schema_def: Some("a".to_owned()),
                            },
                            extra: HashMap::new(),
                        },
                    ),
                    (
                        None,
                        Schema {
                            root_data: Some(RootData {
                                id: None,
                                defs: [(
                                    "a".to_owned(),
                                    Schema {
                                        root_data: None,
                                        form: SchemaForm::Ref {
                                            uri: "#a".to_owned(),
                                            resolved_schema_id: None,
                                            resolved_schema_def: Some("a".to_owned()),
                                        },
                                        extra: HashMap::new(),
                                    }
                                )]
                                .iter()
                                .cloned()
                                .collect(),
                            }),
                            form: SchemaForm::Ref {
                                uri: "http://example.com/foo#a".to_owned(),
                                resolved_schema_id: Some(
                                    Url::parse("http://example.com/foo").unwrap()
                                ),
                                resolved_schema_def: Some("a".to_owned()),
                            },
                            extra: HashMap::new(),
                        },
                    ),
                ]
                .iter()
                .cloned()
                .collect()
            }
        );
    }

    #[test]
    fn resolve_refs_missing_uris() {
        let mut registry = Registry::new();
        assert_eq!(
            registry
                .register(vec![SerdeSchema {
                    id: Some("http://example.com/foo".to_owned()),
                    defs: Some(
                        [("a".to_owned(), SerdeSchema::default())]
                            .iter()
                            .cloned()
                            .collect()
                    ),
                    rxf: Some("http://example.com/bar".to_owned()),
                    ..SerdeSchema::default()
                },])
                .expect("error while registering schema"),
            vec![Url::parse("http://example.com/bar").unwrap(),]
        );
    }
}
