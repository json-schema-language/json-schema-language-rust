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
    errors: Vec<TestCaseError>,
}

#[derive(Debug, Deserialize, PartialEq)]
struct TestCaseError {
    #[serde(rename = "instancePath")]
    instance_path: String,

    #[serde(rename = "schemaPath")]
    schema_path: String,

    #[serde(rename = "schemaURI")]
    schema_id: Option<String>,
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
            println!("{}: {}", i, suite.name);

            let mut registry = Registry::new();
            for serde_schema in suite.registry.iter().chain(&[suite.schema]) {
                let schema =
                    Schema::from_serde(serde_schema.clone()).expect("error creating schema");
                registry.register(schema).expect("error registering schema");
            }

            let validator = Validator::new(&registry);

            for (j, mut test_case) in suite.instances.into_iter().enumerate() {
                println!("{}/{}", i, j);

                let mut actual_errors: Vec<_> = validator
                    .validate(&test_case.instance)
                    .expect("error validating instance")
                    .into_iter()
                    .map(|error| TestCaseError {
                        instance_path: error.instance_path().to_string(),
                        schema_path: error.schema_path().to_string(),
                        schema_id: Some(
                            error
                                .schema_id()
                                .as_ref()
                                .map(|id| id.to_string())
                                .unwrap_or("".to_owned()),
                        ),
                    })
                    .collect();

                actual_errors
                    .sort_by_key(|err| format!("{},{}", err.schema_path, err.instance_path));
                test_case
                    .errors
                    .sort_by_key(|err| format!("{},{}", err.schema_path, err.instance_path));

                assert_eq!(actual_errors, test_case.errors);
            }
        }
    }

    Ok(())
}
