//! An error type for all JSL-related operations.

use failure::Fail;
use url::Url;

/// An enum of possible errors that can emerge from this crate.
#[derive(Debug, Fail)]
pub enum JslError {
  /// A schema-like object did not take on a valid form.
  ///
  /// Only certain combinations of keywords make for valid JSL schemas. When a
  /// schema uses an invalid combination of keywords, it is said to not take on
  /// one of the valid forms. Converting a `SerdeSchema` with an invalid
  /// combination of keywords into a `Schema` will result in this error.
  #[fail(display = "invalid schema form")]
  InvalidForm,

  /// A schema-like object specified a keyword in an ambiguous manner.
  ///
  /// JSL prohibits schemas from specifying the same property twice in the same
  /// schema. This error arises when a `SerdeSchema`'s `properties`,
  /// `optionalProperties`, or `discriminator.propertyName` share a property in
  /// common, and one attempts to convert this into a `Schema`.
  #[fail(display = "ambiguous property: {}", property)]
  AmbiguousProperty { property: String },

  /// A schema refers to a definition which does not exist.
  ///
  /// Schemas may refer to one another using the `ref` keyword, which may refer
  /// to a `definition` of another schema. If a schema refers to another
  /// schema's definition, but that schema is already in a `Registry` and lacks
  /// such a definition, then the reference cannot be resolved, and this error
  /// is thrown.
  #[fail(display = "no definition: {} for schema with id: {}", definition, id)]
  NoSuchDefinition { id: Url, definition: String },

  /// A non-root schema was given to a function which expected a root schema.
  #[fail(display = "non-root schema given when root schema was required")]
  NonRoot,
}
