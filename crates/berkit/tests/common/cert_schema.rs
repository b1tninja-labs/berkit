//! The RFC 5280 Certificate schema as the typed layer can state it, shared by `real_der.rs` and `robustness.rs`.

use berkit::tag::Class;
use berkit::typed::{Field, Presence, Prim, Schema};

// --- RFC 5280 section 4.1.1 and 4.1.2, as far as the typed layer can say it -----------------------------------

const fn req(schema: Schema<'static>) -> Field<'static> {
    Field {
        schema,
        presence: Presence::Required,
    }
}

const fn opt(schema: Schema<'static>) -> Field<'static> {
    Field {
        schema,
        presence: Presence::Optional,
    }
}

// AlgorithmIdentifier ::= SEQUENCE { algorithm OBJECT IDENTIFIER, parameters ANY DEFINED BY algorithm OPTIONAL }.
/// The typed layer has no ANY, so the components are left unchecked.
const ALGORITHM: Schema<'static> = Schema::Sequence(None);
const BIT_STRING: Schema<'static> = Schema::Prim(Prim::BitString { named: false });
// Name ::= CHOICE { rdnSequence RDNSequence }; RDNSequence ::= SEQUENCE OF RelativeDistinguishedName;
// RelativeDistinguishedName ::= SET SIZE (1..MAX) OF AttributeTypeAndValue;
// AttributeTypeAndValue ::= SEQUENCE { type OID, value ANY }, whose components are left unchecked (no ANY).
const NAME: Schema<'static> = Schema::SequenceOf(&Schema::SetOf(Some(&Schema::Sequence(None))));
// Extension ::= SEQUENCE { extnID OID, critical BOOLEAN DEFAULT FALSE, extnValue OCTET STRING }.
const EXTENSION: Schema<'static> = Schema::Sequence(Some(&[
    req(Schema::Prim(Prim::Oid)),
    Field {
        schema: Schema::Prim(Prim::Boolean),
        presence: Presence::DefaultBool(false),
    },
    req(Schema::Prim(Prim::OctetString)),
]));
const TBS: Schema<'static> = Schema::Sequence(Some(&[
    Field {
        schema: Schema::Tagged {
            class: Class::ContextSpecific,
            number: 0,
            explicit: true,
            inner: &Schema::Prim(Prim::Integer),
        },
        presence: Presence::DefaultInt(0),
    },
    req(Schema::Prim(Prim::Integer)),
    req(ALGORITHM),
    req(NAME),
    // Validity ::= SEQUENCE { notBefore Time, notAfter Time } and Time is a CHOICE, which the layer cannot
    // express: the two times are checked below by tag.
    req(Schema::Sequence(None)),
    req(NAME),
    req(Schema::Sequence(Some(&[req(ALGORITHM), req(BIT_STRING)]))),
    opt(Schema::Tagged {
        class: Class::ContextSpecific,
        number: 1,
        explicit: false,
        inner: &BIT_STRING,
    }),
    opt(Schema::Tagged {
        class: Class::ContextSpecific,
        number: 2,
        explicit: false,
        inner: &BIT_STRING,
    }),
    opt(Schema::Tagged {
        class: Class::ContextSpecific,
        number: 3,
        explicit: true,
        inner: &Schema::SequenceOf(&EXTENSION),
    }),
]));
// Checked against RFC 5280 section 4.1 and Appendix A.1 (module `EXPLICIT TAGS`, so `[0]` and `[3]` are explicit,
// and `[1]` and `[2]` say IMPLICIT): field order, tags, optionality and the two DEFAULTs match. What the typed layer
// cannot say (ANY, CHOICE, SIZE (1..MAX), version-dependent fields) is tested by hand in `real_der.rs`.
pub const CERTIFICATE: Schema<'static> =
    Schema::Sequence(Some(&[req(TBS), req(ALGORITHM), req(BIT_STRING)]));
