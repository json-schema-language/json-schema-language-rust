use crate::schema::{Registry, ValidationFailure};
use serde_json::Value;
use url::Url;

pub fn validate(
    max_failures: usize,
    max_depth: usize,
    registry: &Registry,
    schema_uri: Url,
    instance: &Value,
) -> Vec<ValidationFailure> {
    let mut vm = Vm {
        max_failures,
        max_depth,
        registry,
        instance_tokens: Vec::new(),
        schemas: vec![(schema_uri, vec![])],
        failures: Vec::new(),
    };

    vm.eval(instance).unwrap();

    vm.failures
}

struct Vm<'a> {
    max_failures: usize,
    max_depth: usize,
    registry: &'a Registry,
    instance_tokens: Vec<String>,
    schemas: Vec<(Url, Vec<String>)>,
    failures: Vec<ValidationFailure>,
}

impl<'a> Vm<'a> {
    fn eval(&mut self, instance: &Value) -> Result<(), ()> {
        Ok(())
    }
}
