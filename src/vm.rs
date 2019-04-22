use crate::errors::JslError;
use crate::registry::Registry;
use crate::schema::{Form, Schema, Type};
use crate::validator::ValidationError;
use failure::{bail, Error};
use json_pointer::JsonPointer;
use serde_json::Value;
use url::Url;

pub fn validate(
    max_failures: usize,
    max_depth: usize,
    registry: &Registry,
    id: &Option<Url>,
    instance: &Value,
) -> Result<Vec<ValidationError>, Error> {
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
    instance_tokens: Vec<String>,
    schemas: Vec<(&'a Option<Url>, Vec<String>)>,
    errors: Vec<ValidationError>,
}

impl<'a> Vm<'a> {
    fn eval(&mut self, schema: &Schema, instance: &Value) -> Result<(), EvalError> {
        match schema.form() {
            Form::Type(typ) => match typ {
                Type::Null => {
                    if !instance.is_null() {
                        self.push_schema_token("type".to_owned());
                        self.push_err()?;
                        self.pop_schema_token();
                    }
                }
                Type::Boolean => {
                    if !instance.is_boolean() {
                        self.push_schema_token("type".to_owned());
                        self.push_err()?;
                        self.pop_schema_token();
                    }
                }
                Type::Number => {
                    if !instance.is_number() {
                        self.push_schema_token("type".to_owned());
                        self.push_err()?;
                        self.pop_schema_token();
                    }
                }
                Type::String => {
                    if !instance.is_string() {
                        self.push_schema_token("type".to_owned());
                        self.push_err()?;
                        self.pop_schema_token();
                    }
                }
            },
            _ => {}
        }

        Ok(())
    }

    fn push_schema_token(&mut self, token: String) {
        self.schemas
            .last_mut()
            .expect("unreachable: empty schema stack")
            .1
            .push(token);
    }

    fn pop_schema_token(&mut self) {
        self.schemas
            .last_mut()
            .expect("unreachable: empty schema stack")
            .1
            .pop();
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
            schema_id.clone().clone(),
        ));

        if self.errors.len() == self.max_failures {
            Err(EvalError::Internal)
        } else {
            Ok(())
        }
    }
}
