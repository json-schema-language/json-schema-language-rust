use crate::errors::JslError;
use crate::registry::Registry;
use crate::schema::{Form, Schema, Type};
use crate::validator::ValidationError;
use failure::{bail, err_msg, Error};
use json_pointer::JsonPointer;
use serde_json::Value;
use std::borrow::Cow;
use url::Url;

pub fn validate<'a>(
    max_failures: usize,
    max_depth: usize,
    registry: &'a Registry,
    id: &'a Option<Url>,
    instance: &'a Value,
) -> Result<Vec<ValidationError<'a>>, Error> {
    if !registry.is_sealed() {
        bail!(JslError::Unsealed);
    }

    let mut vm = Vm {
        max_failures,
        max_depth,
        registry,
        instance_tokens: Vec::new(),
        schemas: vec![(id, vec![])],
        errors: Vec::new(),
    };

    let schema = if let Some(schema) = registry.get(id) {
        schema
    } else {
        bail!(JslError::NoSuchSchema);
    };

    match vm.eval(schema, instance) {
        Ok(()) | Err(EvalError::Internal) => Ok(vm.errors),
        Err(EvalError::Actual(error)) => Err(error),
    }
}

enum EvalError {
    Internal,
    Actual(Error),
}

struct Vm<'a> {
    max_failures: usize,
    max_depth: usize,
    registry: &'a Registry,
    instance_tokens: Vec<Cow<'a, str>>,
    schemas: Vec<(&'a Option<Url>, Vec<Cow<'a, str>>)>,
    errors: Vec<ValidationError<'a>>,
}

impl<'a> Vm<'a> {
    fn eval(&mut self, schema: &'a Schema, instance: &'a Value) -> Result<(), EvalError> {
        match schema.form() {
            Form::Empty => {}
            Form::Ref(ref id, ref def) => {
                if self.schemas.len() == self.max_depth {
                    return Err(EvalError::Actual(err_msg(JslError::MaxDepthExceeded)));
                }

                let schema_tokens = def
                    .as_ref()
                    .map(|def| vec![Cow::Borrowed("definitions"), Cow::Borrowed(def)])
                    .unwrap_or_else(|| vec![]);

                let root_schema = self
                    .registry
                    .get(id)
                    .expect("unreachable: ref'd schema not found");

                let root_schema_data = root_schema
                    .root_data()
                    .as_ref()
                    .expect("unreachable: non-root schema in registry");

                let refd_schema = if let Some(def) = def {
                    root_schema_data
                        .definitions()
                        .get(def)
                        .expect("unreachable: ref'd definition not found")
                } else {
                    root_schema
                };

                self.schemas.push((id, schema_tokens));
                self.eval(refd_schema, instance)?;
            }
            Form::Type(typ) => match typ {
                Type::Null => {
                    if !instance.is_null() {
                        self.push_schema_token("type");
                        self.push_err()?;
                        self.pop_schema_token();
                    }
                }
                Type::Boolean => {
                    if !instance.is_boolean() {
                        self.push_schema_token("type");
                        self.push_err()?;
                        self.pop_schema_token();
                    }
                }
                Type::Number => {
                    if !instance.is_number() {
                        self.push_schema_token("type");
                        self.push_err()?;
                        self.pop_schema_token();
                    }
                }
                Type::String => {
                    if !instance.is_string() {
                        self.push_schema_token("type");
                        self.push_err()?;
                        self.pop_schema_token();
                    }
                }
            },
            Form::Elements(ref sub_schema) => {
                self.push_schema_token("elements");
                if let Some(arr) = instance.as_array() {
                    for (i, elem) in arr.iter().enumerate() {
                        self.push_instance_token(Cow::Owned(i.to_string()));
                        self.eval(sub_schema, elem)?;
                        self.pop_instance_token();
                    }
                } else {
                    self.push_err()?;
                }
                self.pop_schema_token();
            }
            Form::Properties(ref required, ref optional, has_required) => {
                if let Some(obj) = instance.as_object() {
                    self.push_schema_token("properties");
                    for (property, sub_schema) in required {
                        self.push_schema_token(property);
                        if let Some(sub_instance) = obj.get(property) {
                            self.push_instance_token(property);
                            self.eval(sub_schema, sub_instance)?;
                            self.pop_instance_token();
                        } else {
                            self.push_err()?;
                        }
                        self.pop_schema_token();
                    }
                    self.pop_schema_token();

                    self.push_schema_token("optionalProperties");
                    for (property, sub_schema) in optional {
                        self.push_schema_token(property);
                        if let Some(sub_instance) = obj.get(property) {
                            self.push_instance_token(property);
                            self.eval(sub_schema, sub_instance)?;
                            self.pop_instance_token();
                        }
                        self.pop_schema_token();
                    }
                    self.pop_schema_token();
                } else {
                    // Sort of a weird corner-case in the spec: you have to
                    // check if the instance is an object at all. If it isn't,
                    // you produce an error related to `properties`. But if
                    // there wasn't a `properties` keyword, then you have to
                    // produce `optionalProperties` instead.
                    if *has_required {
                        self.push_schema_token("properties");
                    } else {
                        self.push_schema_token("optionalProperties");
                    }

                    self.push_err()?;
                    self.pop_schema_token();
                }
            }
            Form::Values(ref sub_schema) => {
                self.push_schema_token("values");
                if let Some(obj) = instance.as_object() {
                    for (property, sub_instance) in obj {
                        self.push_instance_token(property);
                        self.eval(sub_schema, sub_instance)?;
                        self.pop_instance_token();
                    }
                } else {
                    self.push_err()?;
                }
                self.pop_schema_token();
            }
            Form::Discriminator(ref tag, ref mapping) => {
                self.push_schema_token("discriminator");
                if let Some(obj) = instance.as_object() {
                    if let Some(instance_tag) = obj.get(tag) {
                        if let Some(instance_tag) = instance_tag.as_str() {
                            if let Some(sub_schema) = mapping.get(instance_tag) {
                                self.push_schema_token("mapping");
                                self.push_schema_token(instance_tag);
                                self.eval(sub_schema, instance)?;
                                self.pop_schema_token();
                                self.pop_schema_token();
                            } else {
                                self.push_schema_token("mapping");
                                self.push_instance_token(tag);
                                self.push_err()?;
                                self.pop_instance_token();
                                self.pop_schema_token();
                            }
                        } else {
                            self.push_schema_token("tag");
                            self.push_instance_token(tag);
                            self.push_err()?;
                            self.pop_instance_token();
                            self.pop_schema_token();
                        }
                    } else {
                        self.push_schema_token("tag");
                        self.push_err()?;
                        self.pop_schema_token();
                    }
                } else {
                    self.push_err()?;
                }
            }
        }

        Ok(())
    }

    fn push_schema_token<T: Into<Cow<'a, str>>>(&mut self, token: T) {
        self.schemas
            .last_mut()
            .expect("unreachable: empty schema stack")
            .1
            .push(token.into());
    }

    fn pop_schema_token(&mut self) {
        self.schemas
            .last_mut()
            .expect("unreachable: empty schema stack")
            .1
            .pop();
    }

    fn push_instance_token<T: Into<Cow<'a, str>>>(&mut self, token: T) {
        self.instance_tokens.push(token.into());
    }

    fn pop_instance_token(&mut self) {
        self.instance_tokens.pop();
    }

    fn push_err(&mut self) -> Result<(), EvalError> {
        let (schema_id, schema_path) = self
            .schemas
            .last()
            .as_ref()
            .expect("unreachable: empty schema stack");
        self.errors.push(ValidationError::new(
            JsonPointer::new(self.instance_tokens.clone()),
            JsonPointer::new(schema_path.clone()),
            schema_id,
        ));

        if self.errors.len() == self.max_failures {
            Err(EvalError::Internal)
        } else {
            Ok(())
        }
    }
}
