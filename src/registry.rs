use crate::errors;
use crate::schema::Schema;
use crate::serde::SerdeSchema;
use typed_arena::Arena;
use url::Url;

pub struct Registry<'a> {
  schemas: Arena<Schema<'a>>,
}

impl<'a> Registry<'a> {
  pub fn register(schemas: &[SerdeSchema]) -> errors::Result<Vec<Url>> {}

  fn first_pass(schema: SerdeSchema) -> errors::Result<Schema<'a>> {
    let id = Url::parse(schema.id.unwrap_or_default())?;

    Ok(Schema { id })
  }
}
