use crate::errors::JslError;
use crate::schema::{Form, Schema, Type};
use crate::validator::ValidationError;
use chrono::DateTime;
use failure::{err_msg, Error};
use json_pointer::JsonPointer;
use serde_json::Value;
use std::borrow::Cow;

pub fn validate<'a>(
    max_failures: usize,
    max_depth: usize,
    strict_instance_semantics: bool,
    schema: &'a Schema,
    instance: &'a Value,
) -> Result<Vec<ValidationError<'a>>, Error> {
    let mut vm = Vm {
        max_failures,
        max_depth,
        strict_instance_semantics,
        root_schema: schema,
        instance_tokens: vec![],
        schema_tokens: vec![vec![]],
        errors: vec![],
    };

    match vm.eval(schema, instance, None) {
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
    strict_instance_semantics: bool,
    root_schema: &'a Schema,
    instance_tokens: Vec<Cow<'a, str>>,
    schema_tokens: Vec<Vec<Cow<'a, str>>>,
    errors: Vec<ValidationError<'a>>,
}

impl<'a> Vm<'a> {
    fn eval(
        &mut self,
        schema: &'a Schema,
        instance: &'a Value,
        parent_tag: Option<&'a str>,
    ) -> Result<(), EvalError> {
        match schema.form() {
            Form::Empty => {}
            Form::Ref(ref def) => {
                if self.schema_tokens.len() == self.max_depth {
                    return Err(EvalError::Actual(err_msg(JslError::MaxDepthExceeded)));
                }

                let refd_schema = &self.root_schema.definitions().as_ref().unwrap()[def];
                self.schema_tokens
                    .push(vec!["definitions".into(), def.into()]);
                self.eval(refd_schema, instance, None)?;
                self.schema_tokens.pop();
            }
            Form::Type(typ) => match typ {
                Type::Boolean => {
                    if !instance.is_boolean() {
                        self.push_schema_token("type");
                        self.push_err()?;
                        self.pop_schema_token();
                    }
                }
                Type::Number | Type::Float32 | Type::Float64 => {
                    if !instance.is_number() {
                        self.push_schema_token("type");
                        self.push_err()?;
                        self.pop_schema_token();
                    }
                }
                Type::Int8 => {
                    self.check_int(instance, -128.0, 127.0)?;
                }
                Type::Uint8 => {
                    self.check_int(instance, 0.0, 255.0)?;
                }
                Type::Int16 => {
                    self.check_int(instance, -32768.0, 32767.0)?;
                }
                Type::Uint16 => {
                    self.check_int(instance, 0.0, 65535.0)?;
                }
                Type::Int32 => {
                    self.check_int(instance, -2147483648.0, 2147483647.0)?;
                }
                Type::Uint32 => {
                    self.check_int(instance, 0.0, 4294967295.0)?;
                }
                Type::Int64 => {
                    self.check_int(instance, -9223372036854775808.0, 9223372036854775807.0)?;
                }
                Type::Uint64 => {
                    self.check_int(instance, 0.0, 18446744073709551615.0)?;
                }
                Type::String => {
                    if !instance.is_string() {
                        self.push_schema_token("type");
                        self.push_err()?;
                        self.pop_schema_token();
                    }
                }
                Type::Timestamp => {
                    if let Some(s) = instance.as_str() {
                        if DateTime::parse_from_rfc3339(s).is_err() {
                            self.push_schema_token("type");
                            self.push_err()?;
                            self.pop_schema_token();
                        }
                    } else {
                        self.push_schema_token("type");
                        self.push_err()?;
                        self.pop_schema_token();
                    }
                }
            },
            Form::Enum(ref values) => {
                if let Some(string) = instance.as_str() {
                    if !values.contains(string) {
                        self.push_schema_token("enum");
                        self.push_err()?;
                        self.pop_schema_token();
                    }
                } else {
                    self.push_schema_token("enum");
                    self.push_err()?;
                    self.pop_schema_token();
                }
            }
            Form::Elements(ref sub_schema) => {
                self.push_schema_token("elements");
                if let Some(arr) = instance.as_array() {
                    for (i, elem) in arr.iter().enumerate() {
                        self.push_instance_token(Cow::Owned(i.to_string()));
                        self.eval(sub_schema, elem, None)?;
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
                            self.eval(sub_schema, sub_instance, None)?;
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
                            self.eval(sub_schema, sub_instance, None)?;
                            self.pop_instance_token();
                        }
                        self.pop_schema_token();
                    }
                    self.pop_schema_token();

                    if self.strict_instance_semantics {
                        for key in obj.keys() {
                            let parent_match = parent_tag.map(|tag| key == tag).unwrap_or(false);

                            if !parent_match
                                && !required.contains_key(key)
                                && !optional.contains_key(key)
                            {
                                self.push_instance_token(key);
                                self.push_err()?;
                                self.pop_instance_token();
                            }
                        }
                    }
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
                        self.eval(sub_schema, sub_instance, None)?;
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
                                self.eval(sub_schema, instance, Some(tag))?;
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

    fn check_int(&mut self, instance: &Value, min: f64, max: f64) -> Result<(), EvalError> {
        if let Some(n) = instance.as_f64() {
            if n.fract() != 0.0 || n < min || n > max {
                self.push_schema_token("type");
                self.push_err()?;
                self.pop_schema_token();
            }
        } else {
            self.push_schema_token("type");
            self.push_err()?;
            self.pop_schema_token();
        }

        Ok(())
    }

    fn push_schema_token<T: Into<Cow<'a, str>>>(&mut self, token: T) {
        self.schema_tokens
            .last_mut()
            .expect("unreachable: empty schema stack")
            .push(token.into());
    }

    fn pop_schema_token(&mut self) {
        self.schema_tokens
            .last_mut()
            .expect("unreachable: empty schema stack")
            .pop();
    }

    fn push_instance_token<T: Into<Cow<'a, str>>>(&mut self, token: T) {
        self.instance_tokens.push(token.into());
    }

    fn pop_instance_token(&mut self) {
        self.instance_tokens.pop();
    }

    fn push_err(&mut self) -> Result<(), EvalError> {
        let schema_path = self
            .schema_tokens
            .last()
            .expect("unreachable: empty schema stack")
            .clone();

        self.errors.push(ValidationError::new(
            JsonPointer::new(self.instance_tokens.clone()),
            JsonPointer::new(schema_path),
        ));

        if self.errors.len() == self.max_failures {
            Err(EvalError::Internal)
        } else {
            Ok(())
        }
    }
}
