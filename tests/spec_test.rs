use jsl::{Registry, SerdeSchema};
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
    let test_files = fs::read_dir("spec/tests")?;
    for entry in test_files {
        let path = entry?.path();
        println!("{:?}", &path);
        let file = fs::read(path)?;
        let suites: Vec<TestSuite> = serde_json::from_slice(&file)?;

        for (i, suite) in suites.into_iter().enumerate() {
            println!("{}", i);

            let mut registry = Registry::new();
            assert!(registry.register(suite.registry).unwrap().is_empty());
            assert!(registry.register(vec![suite.schema]).unwrap().is_empty());

            for (j, test_case) in suite.instances.into_iter().enumerate() {
                println!("{}/{}", i, j);
                assert_eq!(
                    test_case.errors.is_empty(),
                    registry.validate(test_case.instance).is_empty()
                )
            }
        }
    }

    Ok(())
}
