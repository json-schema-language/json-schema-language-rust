//! An error type for all JSL-related operations.

use failure::Fail;

/// An enum of possible errors that can emerge from this crate.
#[derive(Debug, Fail, PartialEq, Clone, Eq, Hash)]
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
    /// to a `definition` in the root schema. If a schema refers to a definition
    /// which does not exist, this error is returned.
    #[fail(display = "no such definition: {}", definition)]
    NoSuchDefinition { definition: String },

    /// The maximum depth during evaluating was exceeded.
    ///
    /// This likely means that your configured `max_depth` is too small, or that
    /// there is a infinite cyclical definition in your schemas.
    #[fail(display = "maximum reference depth exceeeded during validation")]
    MaxDepthExceeded,
}
