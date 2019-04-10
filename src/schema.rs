use serde_json::Value;
use std::collections::HashMap;
use url::Url;

pub struct Schema<'a> {
  root_data: Option<RootData<'a>>,
  form: Box<SchemaForm<'a>>,
  extra: HashMap<String, Value>,
}

pub struct RootData<'a> {
  id: Url,
  defs: HashMap<String, Schema<'a>>,
}

pub enum SchemaForm<'a> {
  Empty,
  Ref {
    uri: Url,
    referent: &'a Schema<'a>,
  },
  Type(PrimitiveType),
  Elements(Schema<'a>),
  Properties {
    required: HashMap<String, Schema<'a>>,
    optional: HashMap<String, Schema<'a>>,
  },
  Values(Schema<'a>),
  Discriminator {
    tag: String,
    mapping: HashMap<String, Schema<'a>>,
  },
}

pub enum PrimitiveType {
  Null,
  Bool,
  Num,
  Str,
}
