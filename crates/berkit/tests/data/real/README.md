# Real DER data for `real_der.rs`

Forty public CA certificates, none produced by this workspace, kept as DER. They are public trust-anchor and
intermediate certificates: no private key and nothing from a smart card is here. Nothing was downloaded: they
were read from this machine on 2026-11-13 by Python 3.14 from two places.

- Windows: `ssl.create_default_context().get_ca_certs(binary_form=True)`, which on Windows loads the ROOT and CA stores.
- certifi: `certifi` 's `cacert.pem` (the Mozilla root list), the PEM base64-decoded.

From the 164 distinct certificates of the two sources, 40 were chosen to vary key type (RSA 1024, 2048, 3072, 4096; EC P-256, P-384,
P-521), the version (one is X.509 v1, with no version field) and the signature algorithm. The outermost length is `82 xx xx`
in all of them: no certificate on this machine has a 1-octet, `81` or `83` outermost length. The short form and `81 xx` occur
inside every certificate. A four-octet-length form was never seen.

Files: `NN.der` the certificate; `NN.tree` the node dump of `refdump.py`; `mutations.expected` the reference's predicted
class and the sha256 of each mutation. `refdump.py expect .` regenerates the last two; `crosscheck.py .` compares the
top-level fields with the `cryptography` package.

| File | sha256 | Subject CN | Key | Outer length octets | Version | Source |
|---|---|---|---|---|---|---|
| 01.der | 18ce6cfe7bf14e60b2e347b8dfe868cb31d02ebb3ada271569f50343b46db3a4 | Amazon Root CA 3 | ecP-256 | 3 | v3 | certifi |
| 02.der | b085d70b964f191a73e4af0d54ae7a0e07aafdaf9b71dd0862138ab7325a24a2 | GlobalSign | ecP-256 | 3 | v3 | certifi |
| 03.der | beb00b30839b9bc32c32e4447905950641f26421b15ed089198b518ae2ea1b99 | e-Szigno Root CA 2017 | ecP-256 | 3 | v3 | certifi |
| 04.der | 31ad6648f8104138c738f39ea4320133393e3a18cc02296ef97c2ac9ef6731d0 | DigiCert Global Root G3 | ecP-384 | 3 | v3 | Windows store |
| 05.der | 358df39d764af9e1b766e9c972df352ee15cfac227af6ad1d70e8e4a6edcba02 | Microsoft ECC Root Certificate Authority 2017 | ecP-384 | 3 | v3 | Windows store |
| 06.der | 61799e3594f6fa6c9f619031e4a3c9f643fda38c083704a5075e5709a21b1ab7 | Microsoft TLS ECC Root G2 | ecP-384 | 3 | v3 | Windows store |
| 07.der | 3417bb06cc6007da1b961c920b8ab4ce3fad820e4aa30b9acbc4a74ebdcebc65 | SSL.com Root Certification Authority ECC | ecP-384 | 3 | v3 | Windows store |
| 08.der | b49141502d00663d740f2e7ec340c52800962666121a36d09cf7dd2b90384fb4 | e-Szigno TLS Root CA 2023 | ecP-521 | 3 | v3 | certifi |
| 09.der | 00bd2b0edd8340b1746cc395c0e355b2165853fdb93c52dadda8228b07002dce | (no CN) | rsa1024 | 3 | v3 | Windows store |
| 10.der | e7685634efacf69ace939a6b255b7b4fabef42935b50a265acb5cb6027e44e70 | (no CN) | rsa1024 | 3 | v1 | Windows store |
| 11.der | ab7036365c7154aa29c2c29f5d4191163b162a2225011357d56d07ffa7bc1f72 | Thawte Premium Server CA | rsa1024 | 3 | v3 | Windows store |
| 12.der | 513b2cecb810d4cde5dd85391adfc6c2dd60d87bb736d2b521484aa47a0ebef6 | (no CN) | rsa2048 | 3 | v3 | Windows store |
| 13.der | d8e0febc1db2e38d00940f37d27d41344d993e734b99d5656d9778d4d8143624 | Certum CA | rsa2048 | 3 | v3 | Windows store |
| 14.der | 0687260331a72403d909f105e69bcf0d32e1bd2493ffc6d9206d11bcd6770739 | DST Root CA X3 | rsa2048 | 3 | v3 | Windows store |
| 15.der | c8025f9fc65fdfc95b3ca8cc7867b9a587b5277973957917463fc813d0b625a9 | DigiCert Global G2 TLS RSA SHA256 2020 CA1 | rsa2048 | 3 | v3 | Windows store |
| 16.der | cbb522d7b7f127ad6a0113865bdf1cd4102e7d0759af635a7cf4720dc963c53b | GlobalSign | rsa2048 | 3 | v3 | Windows store |
| 17.der | b676ffa3179e8812093a1b5eafee876ae7a6aaf231078dad1bfb21cd2893764a | GlobalSign RSA OV SSL CA 2018 | rsa2048 | 3 | v3 | Windows store |
| 18.der | ebd41040e4bb3ec742c9e381d31ef2a41a48b6685c96e7cef3c1df6cd4331c99 | GlobalSign Root CA | rsa2048 | 3 | v3 | Windows store |
| 19.der | 4422e963ee53cd58cc9f85cd40bf5ffec0095fdf1a154535661c1c06bcadc69b | RapidSSL TLS RSA CA G1 | rsa2048 | 3 | v3 | Windows store |
| 20.der | f1c1b50ae5a20dd8030ec9f6bc24823dd367b5255759b4e71b61fce9f7375d73 | SecureTrust CA | rsa2048 | 3 | v3 | Windows store |
| 21.der | 8c54c334b66ba4e426772af4a3f9136c19a1aec729fdb28c535c07a5a4ef22e0 | Sectigo Public Server Authentication CA DV R36 | rsa3072 | 3 | v3 | Windows store |
| 22.der | 8eb2f17d668941c39a7fca0cee127ae0ebaf444610631cca3cd19eab46c5824a | Sectigo Public Server Authentication CA OV R40 | rsa3072 | 3 | v3 | Windows store |
| 23.der | b676f2eddae8775cd36cb0f63cd1d4603961f49e6265ba013a2f0307b6d0b804 | Certum Trusted Network CA 2 | rsa4096 | 3 | v3 | Windows store |
| 24.der | 2a575471e31340bc21581cbd2cf13e158463203ece94bcf9d3cc196bf09a5472 | GTS Root R1 | rsa4096 | 3 | v3 | Windows store |
| 25.der | d947432abde7b7fa90fc2e6b59101b1280e0e1c7e4e40fa3c6887fff57a7f4cf | GTS Root R1 | rsa4096 | 3 | v3 | Windows store |
| 26.der | 2cabeafe37d06ca22aba7391c0033d25982952c453647349763a3ab5ad6ccf69 | GlobalSign | rsa4096 | 3 | v3 | Windows store |
| 27.der | 4fa3126d8d3a11d1c4855a4f807cbad6cf919d3a5a88b03bea2c6372d93c40c9 | GlobalSign Root R46 | rsa4096 | 3 | v3 | Windows store |
| 28.der | 7a43bc7747d0633fbd90ff900c9242417c027dbdca05af72da9a70e3518dbe2e | GoDaddy TLS Intermediate CA DV - R1v1 | rsa4096 | 3 | v3 | Windows store |
| 29.der | 25cf3da8e9b97addbf92543c2b82527c8a4e2cff2062a6483040d4b64ace719f | GoDaddy TLS Root CA - R1 | rsa4096 | 3 | v3 | Windows store |
| 30.der | 96bcec06264976f37460779acf28c5a7cfe8a3c0aae11a8ffcee05c0bddf08c6 | ISRG Root X1 | rsa4096 | 3 | v3 | Windows store |
| 31.der | 5d56499be4d2e08bcfcad08a3e38723d50503bde706948e42f55603019e528ae | IdenTrust Commercial Root CA 1 | rsa4096 | 3 | v3 | Windows store |
| 32.der | 3dadfaf812dd1bbaef45834ccbd188f3cd97139e2ed1aca69c2dd63082142f8f | Microsoft Code Signing PCA 2024 | rsa4096 | 3 | v3 | Windows store |
| 33.der | 885de64c340e3ea70658f01e1145f957fcda27aabeea1ab9faa9fdb0102d4077 | Microsoft Root Certificate Authority | rsa4096 | 3 | v3 | Windows store |
| 34.der | ea7a25255d111fc3ce4cb8fabe3adf9c27bbe6db203f955066bab4c5a71f3d08 | Microsoft TLS G2 RSA CA OCSP 02 | rsa4096 | 3 | v3 | Windows store |
| 35.der | ac8ea9f2874fd368a3e778b1a0b165ee898db9b9687c17edcdc76908ab58c82c | Microsoft TLS G2 RSA CA OCSP 04 | rsa4096 | 3 | v3 | Windows store |
| 36.der | 38d895f5105bc2545822c17a2444f4b0ea3d298d2c15a38296aa7295a3d85ce9 | Microsoft TLS G2 RSA CA OCSP 16 | rsa4096 | 3 | v3 | Windows store |
| 37.der | ddcd1e8a20638d4aaff7201bb1d56452acd2c759f1686bdc38f73dd15732bdc2 | Microsoft TLS RSA Root G2 | rsa4096 | 3 | v3 | Windows store |
| 38.der | fe229efc927f6d77b738896752a21803a59736baa17bf5be9a50c72e219cbcd2 | Microsoft Windows Code Signing PCA 2024 | rsa4096 | 3 | v3 | Windows store |
| 39.der | 8faf7d2e2cb4709bb8e0b33666bf75a5dd45b5de480f8ea8d4bfe6bebc17f2ed | SSL.com TLS RSA Root CA 2022 | rsa4096 | 3 | v3 | Windows store |
| 40.der | 7bb647a62aeeac88bf257aa522d01ffea395e0ab45c73f93f65654ec38f25a06 | Sectigo Public Server Authentication Root R46 | rsa4096 | 3 | v3 | Windows store |
