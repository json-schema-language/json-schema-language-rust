use jsl::{Registry, Schema, SerdeSchema, Validator};
use serde::Deserialize;
use serde_json::Value;
use std::fs;

#[derive(Deserialize)]
struct TestSuite {
    name: String,
    registry: Vec<SerdeSchema>,
    schema: SerdeSchema,
    instances: Vec<TestCase>,
}

#[derive(Deserialize)]
struct TestCase {
    instance: Value,
    errors: Vec<Value>,
}

#[test]
fn spec() -> Result<(), std::io::Error> {
    let mut test_files: Vec<_> = fs::read_dir("spec/tests")?
        .map(|entry| entry.expect("error getting dir entry").path())
        .collect();
    test_files.sort();

    for path in test_files {
        println!("{:?}", &path);
        let file = fs::read(path)?;
        let suites: Vec<TestSuite> = serde_json::from_slice(&file)?;

        for (i, suite) in suites.into_iter().enumerate() {
            println!("{}", i);

            let mut registry = Registry::new();
            for serde_schema in suite.registry.iter().chain(&[suite.schema]) {
                let schema =
                    Schema::from_serde(serde_schema.clone()).expect("error creating schema");
                registry.register(schema).expect("error registering schema");
            }

            let validator = Validator::new(&registry);

            for (j, test_case) in suite.instances.into_iter().enumerate() {
                println!("{}/{}", i, j);
                assert_eq!(
                    test_case.errors.is_empty(),
                    validator
                        .validate(test_case.instance)
                        .expect("error validating instance")
                        .is_empty()
                )
            }
        }
    }

    Ok(())
}
