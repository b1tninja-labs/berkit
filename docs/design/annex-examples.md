# Worked examples printed in X.690 (02/2021) | ISO/IEC 8825-1:2021, with Erratum 1 (09/2021)

Source: the X.690 (02/2021) PDF fetched by primarysource, extracted with
`pdftotext -raw` into `x690.txt` in this directory. Citations are `X.690 (02/2021) §n`.
Quotes are not used; the standard's text is paraphrased. Hex is lowercase; the standard
prints it as `NN16` pairs or `NN` pairs.

How each byte string was checked:

- **V1** = read from `pdftotext -raw` output (the text layer of the PDF) and re-derived by hand,
  with the construction shown. A script (`derive8.py`, `verify.py` here) rebuilt the bytes from
  the clauses and compared.
- The PDF was not rendered to an image (no `pdftoppm`/`pdfinfo` on this machine), so a defect in
  the text layer that is not in the printed page would not be seen. Say "text layer verified",
  not "page verified".
- Erratum 1 (09/2021) is a one-page covering note that replaces Figure 4 (identifier octets,
  high tag number), whose penultimate octet had a typo. No byte string below depends on Figure 4.
  I could not read Figure 4 itself (a drawing; `itu_errata` also gives only the note). Not verified.

## Summary of disagreements

None between the printed bytes and a correct derivation. One of my own first derivations
disagreed (Type4, see 8.14.4 below); the error was mine (explicit tag [2] is replaced, not
wrapped, by an IMPLICIT outer tag), and the printed bytes are right.

Two things that are not errata but that a corpus writer must not miss:

1. Annex A.3 is **BER, not DER**: the SET components are in declaration order (tags 61, a0, 42, a1,
   a2, a3), whereas §10.3 puts SET components in the canonical tag order of X.680 §8.6
   (universal, then application, then context-specific, each ascending), i.e. 61, 42, a0, a1, a2,
   a3. A DER rendering therefore differs from the printed bytes by the order of the title and
   number components only; length is the same. Derived below as A.3-DER.
2. The A.1 type is not the X.680 Annex G.1.2 type. X.680 G.1.2 (same personnel record, tutorial)
   has no context tags `[0]..[3]` and no IMPLICIT keywords, only `[APPLICATION n]` tags, so
   its BER encoding differs (no a0/a1/a2/a3 wrappers). The *value* is identical (X.680 G.1.3
   vs X.690 A.2). Do not use A.3 bytes for the X.680 G.1.2 type.

## Annex A (informative) - BER

### A.1 type, in outline (paraphrase of X.690 §A.1)

PersonnelRecord is `[APPLICATION 0] IMPLICIT SET` of: name (Name), title (`[0]` VisibleString,
explicit because the environment is explicit), number (EmployeeNumber), dateOfHire (`[1]` Date,
explicit), nameOfSpouse (`[2]` Name, explicit), children (`[3] IMPLICIT SEQUENCE OF
ChildInformation`, DEFAULT empty). ChildInformation is an untagged SET of name and `[0]` Date
(explicit). Name is `[APPLICATION 1] IMPLICIT SEQUENCE` of three VisibleStrings (givenName,
initial, familyName). EmployeeNumber is `[APPLICATION 2] IMPLICIT INTEGER`. Date is `[APPLICATION 3]
IMPLICIT VisibleString` (YYYYMMDD). The DEFAULT applies to `children`, not to the elements
(X.680 §G.1.2 makes this point).
Cite: X.690 (02/2021) §A.1.

### A.2 value

John P Smith, title Director, number 51, hired 19710917, spouse Mary T Smith, children Ralph T
Smith born 19571111 and Susan B Jones born 19590717. Cite: X.690 (02/2021) §A.2 (same value in
X.680 (02/2021) §G.1.3).

### A.3 representation (rule: BER; printed in definite-length form)

Hex as printed (136 octets: 60 81 85 then 133 content octets), re-flowed on one line:

```
60818561101a044a6f686e1a01501a05536d697468a00a1a084469726563746f72420133a10a43083139373130393137a21261101a044d6172791a01541a05536d697468a342311f61111a0552616c70681a01541a05536d697468a00a43083139353731313131311f61111a05537573616e1a01421a054a6f6e6573a00a43083139353930373137
```

Verification: the text layer of A.3 was parsed line by line (tag, length, quoted string or the
integer content `33`) and concatenated; the result was compared with bytes built from A.2 using
the clauses. Equal, 136 octets. Construction (all lengths from §8.1.3.3-8.1.3.5):

| Element | Rule | Bytes | Len |
|---|---|---|---|
| givenName "John" | VisibleString 1a (§8.23) | 1a 04 4a6f686e | 6 |
| initial "P" | | 1a 01 50 | 3 |
| familyName "Smith" | | 1a 05 536d697468 | 7 |
| name | Name = APPLICATION 1 constructed (8.1.2.2, 8.9) = 0x61; 6+3+7 = 16 = 0x10 | 61 10 ... | 18 |
| title | [0] explicit (§8.14.3) = a0; inner 1a 08 "Director" = 10 = 0x0a | a0 0a 1a08 4469726563746f72 | 12 |
| number 51 | APPLICATION 2 primitive = 0x42; 51 = 0x33 (§8.3.3) | 42 01 33 | 3 |
| dateOfHire | [1] explicit = a1; Date = 0x43, 8 chars | a1 0a 43 08 3139373130393137 | 12 |
| nameOfSpouse | [2] explicit = a2; inner Name 61 10 ... = 18 = 0x12 | a2 12 61 10 ... | 20 |
| child 1 | SET = 0x31; name 61 11 (6+... 7+3+7=17=0x11) 19 octets, dateOfBirth a0 0a 43 08 .. 12 octets; 31 = 0x1f | 31 1f ... | 33 |
| child 2 | same shape | 31 1f ... | 33 |
| children | [3] IMPLICIT constructed = a3; 33+33 = 66 = 0x42 | a3 42 ... | 68 |
| record | APPLICATION 0 constructed = 0x60; content 18+12+3+12+20+68 = 133 = 0x85 > 127 so long form 81 85 | 60 81 85 ... | 136 |

Cite: X.690 (02/2021) §A.3. Rule: BER (declaration-order SET). Case ids: `x690.a3.personnel-record.ber`.

### A.3-DER (derived, not printed)

Rule: DER, by hand from X.690 §10.3 and X.680 §8.6. Swap `title` (a0 0a ...) and `number` (42 01 33)
so name (61) and number (42) come before the context tags:

```
60818561101a044a6f686e1a01501a05536d697468420133a00a1a084469726563746f72a10a43083139373130393137a21261101a044d6172791a01541a05536d697468a342311f61111a0552616c70681a01541a05536d697468a00a43083139353731313131311f61111a05537573616e1a01421a054a6f6e6573a00a43083139353930373137
```

Other DER conditions hold: definite minimal lengths (§10.1), no constructed strings (§10.2),
children present and non-empty so §11.5 does not apply, ChildInformation SETs already ordered
(61 before a0). Case id: `x690.a3.personnel-record.der-order`. Not printed by the standard:
mark as "constructed by hand".

## Annex B (informative) - identification

Values assigned by X.690 §12.2-12.4, listed in Annex B: BER `{joint-iso-itu-t asn1(1)
basic-encoding(1)}`, CER `{joint-iso-itu-t asn1(1) ber-derived(2) canonical-encoding(0)}`, DER
`{joint-iso-itu-t asn1(1) ber-derived(2) distinguished-encoding(1)}`. Also printed: OID-IRI values
"/ASN.1/Basic-Encoding", "/ASN.1/BER-Derived/Canonical-Encoding",
"/ASN.1/BER-Derived/Distinguished-Encoding", and object descriptors. The standard prints **no bytes**
for these. Derived by hand (rule: BER and DER identical; §8.19.4: first subidentifier 2*40+1 = 81 = 0x51;
the rest are < 128):

| OID | Arcs | Bytes | Cite |
|---|---|---|---|
| BER | 2.1.1 | 06 02 51 01 | §8.19.4, §8.19.2 |
| CER | 2.1.2.0 | 06 03 51 02 00 | same |
| DER | 2.1.2.1 | 06 03 51 02 01 | same |

Not derived: the OID-IRI and ObjectDescriptor encodings (their contents rules, §8.21.2, point
into X.680 §34.3 notation which I did not read, so I did not guess whether the slashes are in the
contents), and the numbers 12.2/12.3/12.4 beyond what Annex B lists. Not verified against any
implementation by design.

## Annex C (informative) - REAL

The annex is an algorithm for an artificial hardware format, not a table of values. No complete
encoded REAL is printed. Printed constants, all re-derived:

- C.2: contents are `1 S bb ff ee`, then exponent octets, then N. Base 16: bb = 10; F = 3: ff = 11;
  3-octet exponent: ee = 10. Derivation: positive 1 0 10 11 10 = 10101110 = 0xae; negative
  1 1 10 11 10 = 11101110 = 0xee. The standard prints both bit patterns (C.5 d) and C.6 a); match.
- C.5 d): length 9 = 1 + 3 (exponent) + 5 (mantissa); match.
- Exponent transmitted is E minus 9 (C.5 e)); mantissa is 5 octets (C.5 f)).

A hand-built illustration (not printed): E = 16, so exponent octets are 16 - 9 = 7 = 00 00 07;
N = 1 = 00 00 00 00 01; value = N * 2^3 * 16^7 = 2^31 under C.6 c) (multiply by 2^F and by B^E). Bytes
`09 09 ae 00 00 07 00 00 00 00 01`. Rule: valid BER (§8.5.7); invalid DER (§11.3.1: F must be zero and
base 2). Case id `x690.c.5.real-base16-f3`. The numeric value is derived from the C.6 formula, not
printed; mark as hand-constructed.

## Examples in the clauses

All verified in the text layer and re-derived. "Rule" column is what the printed form is.

| Clause | ASN.1 type / value | Printed bytes (hex) | Rule | Derivation |
|---|---|---|---|---|
| §8.1.3.4 | length L = 38, short form | 26 (00100110) | BER, DER | bit 8 zero, 38 = 0x26 |
| §8.1.3.5 | length L = 201, long form | 81 c9 (10000001 11001001) | BER, DER (minimal) | 201 > 127; one length octet 0xc9 after 0x81 |
| §8.2.2 | BOOLEAN TRUE | 01 01 ff | BER; also DER (§11.1 demands ff) | tag 01, len 1, ff |
| §8.8.2 | NULL | 05 00 | BER, DER | tag 05, len 0 |
| §8.9 | SEQUENCE {name IA5String, ok BOOLEAN} = {"Smith", TRUE} | 30 0a 16 05 536d697468 01 01 ff | BER, DER | 7 + 3 = 10 = 0x0a |
| §8.6.4.2 | BIT STRING '0A3B5F291CD'H, primitive | 03 07 04 0a3b5f291cd0 | BER, DER | 44 bits = 6 octets + 1 initial (04 unused); content 0a 3b 5f 29 1c d0 |
| §8.6.4.2 | same, constructed (printed with indefinite length) | 23 80 03 03 00 0a3b 03 05 04 5f291cd0 00 00 | BER only (§10.2 forbids constructed strings in DER; indefinite also barred) | segments 0a3b (00 unused) and 5f291cd0 (04 unused), 2+... = 3 and 5 |
| §8.14.4 | Type1 ::= VisibleString "Jones" | 1a 05 4a6f6e6573 | BER, DER | |
| §8.14.4 | Type2 ::= [APPLICATION 3] IMPLICIT Type1 | 43 05 4a6f6e6573 | BER, DER | implicit replaces tag |
| §8.14.4 | Type3 ::= [2] Type2 (explicit environment) | a2 07 43 05 4a6f6e6573 | BER, DER | explicit: constructed wrapper, 7 = 2 + 5 |
| §8.14.4 | Type4 ::= [APPLICATION 7] IMPLICIT Type3 | 67 07 43 05 4a6f6e6573 | BER, DER | implicit replaces Type3's [2]; contents = Type3's contents (the 43 05 ... encoding, 7 octets). My first derivation had 67 09 a2 07 ...: wrong, see summary |
| §8.14.4 | Type5 ::= [2] IMPLICIT Type2 | 82 05 4a6f6e6573 | BER, DER | primitive, context 2 |
| §8.19.5 | OBJECT IDENTIFIER {2 999 3} | 06 03 88 37 03 | BER, DER | first subidentifier 2*40 + 999 = 1079 = 8*128 + 55 -> 88 37; then 03 |
| §8.20.5 | RELATIVE-OID {8571 3 2} | 0d 04 c2 7b 03 02 | BER, DER | 8571 = 66*128 + 123 -> c2 7b |
| §8.23.5.4 | VisibleString "Jones", primitive | 1a 05 4a6f6e6573 | BER, DER | |
| §8.23.5.4 | same, constructor, definite | 3a 09 04 03 4a6f6e 04 02 6573 | BER only | 5 + 4 = 9 |
| §8.23.5.4 | same, constructor, indefinite | 3a 80 04 03 4a6f6e 04 02 6573 00 00 | BER only | |

Examples that print values or strings, not whole encodings:

- §9.3 (CER set ordering): a SET with a [3] INTEGER, a [1] CHOICE, and an untagged CHOICE e whose
  smallest nested tag is [0]; the order is e, b, a. Order rule only, no bytes. Rule: the clause sits
  under CER (§9.3); §10.3 (DER) only notes that an untagged choice is placed by the tag of the chosen
  alternative, so do not reuse this example for DER. Derivation: nested minimum tags f -> 5,
  i -> 0, so e sorts as [0], then b [1], then a [3]; agrees with the printed order.
- §11.7.3, §11.7.5 (GeneralizedTime, CER/DER): valid "19920521000000Z", "19920622123421Z",
  "19920722132100.3Z"; invalid "19920520240000Z", "19920622123421.0Z", "19920722132100.30Z". Fraction
  "26.000" -> "26", "26.5200" -> "26.52". Tag 18 hex; length by counting characters.
- §11.8.4, §11.8.5 (UTCTime): valid "920521000000Z", "920622123421Z", "920722132100Z"; invalid
  "920520240000Z", "9207221321Z" (seconds omitted).

No printed worked example exists for INTEGER (§8.3), ENUMERATED, a real-value encoding in the
body (§8.5), CHOICE, SET OF, EXTERNAL, EMBEDDED PDV, string types other than VisibleString, the
high-tag-number form (§8.1.2.4; only Figure 4 shows it), indefinite-length SEQUENCE, or EOC beyond
the one in §8.6.4.2/§8.23.5.4 (the EOC octets themselves are defined in §8.1.5). Those cases in
the corpus are "constructed by hand" and must say so.
