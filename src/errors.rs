//! An error type for all JSL-related operations.

use failure::Fail;
use url::Url;

/// An enum of possible errors that can emerge from this crate.
#[derive(Debug, Fail, PartialEq)]
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
  #[fail(display = "no definition: {} for schema with id: {:?}", definition, id)]
  NoSuchDefinition { id: Option<Url>, definition: String },

  /// A schema attempts to refer to something relative to its ID, but it has no
  /// ID.
  ///
  /// JSL resolves inter-schema references using the usual rules for URIs, where
  /// the `id` of a schema is used as the base URI for all `ref`s within that
  /// schema. But if a schema lacks an `id`, then references must either be
  /// *only* a fragment, or be an absolute URI. Otherwise, there's no meaningful
  /// way to resolve the reference.
  ///
  /// An example of one such unresolvable reference would be:
  ///
  /// ```json
  /// {
  ///     "ref": "/foo"
  /// }
  /// ```
  ///
  /// There's no way to resolve `/foo` without having a base URI, but the schema
  /// doesn't have a base URI to work from.
  #[fail(display = "relative reference in an anonymous schema")]
  RelativeRefFromAnonymousSchema,

  /// A non-root schema was given to a function which expected a root schema.
  #[fail(display = "non-root schema given when root schema was required")]
  NonRoot,

  /// A sealed registry was given to a function which expected a sealed schema.
  #[fail(display = "unsealed registry given when sealed registry was required")]
  Unsealed,

  /// An ID was given, but no schema with that ID exists.
  #[fail(display = "no schema with the given id found")]
  NoSuchSchema,

  /// The maximum depth during evaluating was exceeded.
  ///
  /// This likely means that your configured `max_depth` is too small, or that
  /// there is a infinite cyclical definition in your schemas.
  #[fail(display = "maximum reference depth exceeeded during validation")]
  MaxDepthExceeded,
}
