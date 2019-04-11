use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

#[derive(Debug, PartialEq, Deserialize, Serialize, Default, Clone)]
pub struct SerdeSchema {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "definitions")]
    pub defs: Option<HashMap<String, SerdeSchema>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "ref")]
    pub rxf: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "type")]
    pub typ: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "elements")]
    pub elems: Option<Box<SerdeSchema>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "properties")]
    pub props: Option<HashMap<String, SerdeSchema>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "optionalProperties")]
    pub opt_props: Option<HashMap<String, SerdeSchema>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub values: Option<Box<SerdeSchema>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub discriminator: Option<SerdeDiscriminator>,

    #[serde(skip_serializing_if = "HashMap::is_empty")]
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

#[derive(Debug, PartialEq, Deserialize, Serialize, Default, Clone)]
pub struct SerdeDiscriminator {
    #[serde(rename = "propertyName")]
    pub tag: String,
    pub mapping: HashMap<String, SerdeSchema>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn roundtrip_json() {
        let data = r#"{
  "id": "http://example.com/foo",
  "definitions": {
    "a": {}
  },
  "ref": "http://example.com/bar",
  "type": "foo",
  "elements": {},
  "properties": {
    "a": {}
  },
  "optionalProperties": {
    "a": {}
  },
  "values": {},
  "discriminator": {
    "propertyName": "foo",
    "mapping": {
      "a": {}
    }
  },
  "extra": "foo"
}"#;

        let parsed: SerdeSchema = serde_json::from_str(data).expect("failed to parse json");
        assert_eq!(
            parsed,
            SerdeSchema {
                id: Some("http://example.com/foo".to_owned()),
                rxf: Some("http://example.com/bar".to_owned()),
                defs: Some(
                    [("a".to_owned(), SerdeSchema::default())]
                        .iter()
                        .cloned()
                        .collect()
                ),
                typ: Some("foo".to_owned()),
                elems: Some(Box::new(SerdeSchema::default())),
                props: Some(
                    [("a".to_owned(), SerdeSchema::default())]
                        .iter()
                        .cloned()
                        .collect()
                ),
                opt_props: Some(
                    [("a".to_owned(), SerdeSchema::default())]
                        .iter()
                        .cloned()
                        .collect()
                ),
                values: Some(Box::new(SerdeSchema::default())),
                discriminator: Some(SerdeDiscriminator {
                    tag: "foo".to_owned(),
                    mapping: [("a".to_owned(), SerdeSchema::default())]
                        .iter()
                        .cloned()
                        .collect(),
                }),
                extra: [("extra".to_owned(), json!("foo"))]
                    .iter()
                    .cloned()
                    .collect(),
            }
        );

        let round_trip = serde_json::to_string_pretty(&parsed).expect("failed to serialize json");
        assert_eq!(round_trip, data);
    }
}
