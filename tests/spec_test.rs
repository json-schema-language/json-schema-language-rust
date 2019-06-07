use jsl::{Config, Schema, SerdeSchema, Validator};
use serde::Deserialize;
use serde_json::Value;
use std::fs;

#[derive(Deserialize)]
struct TestSuite {
    name: String,
    schema: SerdeSchema,
    #[serde(rename = "strictInstance")]
    strict_instance: bool,
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

            let schema = Schema::from_serde(suite.schema).expect("error parsing schema");

            let mut config = Config::new();
            config.strict_instance_semantics(suite.strict_instance);

            let validator = Validator::new_with_config(config);

            for (j, mut test_case) in suite.instances.into_iter().enumerate() {
                println!("{}/{}", i, j);

                let mut actual_errors: Vec<_> = validator
                    .validate(&schema, &test_case.instance)
                    .expect("error validating instance")
                    .into_iter()
                    .map(|error| TestCaseError {
                        instance_path: error.instance_path().to_string(),
                        schema_path: error.schema_path().to_string(),
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
