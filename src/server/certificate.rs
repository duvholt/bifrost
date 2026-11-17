use std::fs::File;
use std::io::{Read, Write};
use std::str::FromStr;

use camino::Utf8Path;
use der::asn1::{GeneralizedTime, OctetString};
use der::oid::db::rfc4519::COMMON_NAME;
use der::oid::db::rfc5280::ID_KP_SERVER_AUTH;
use der::pem::LineEnding;
use der::{DateTime, EncodePem};
use mac_address::MacAddress;
use p256::ecdsa::DerSignature;
use p256::elliptic_curve::Generate;
use p256::pkcs8::EncodePrivateKey;
use rustls_pki_types::CertificateDer;
use rustls_pki_types::pem::PemObject;
use sha1::Sha1;
use sha2::Digest;
use x509_cert::Certificate;
use x509_cert::attr::AttributeTypeAndValue;
use x509_cert::builder::{Builder, CertificateBuilder, profile::BuilderProfile};
use x509_cert::certificate::{CertificateInner, TbsCertificate};
use x509_cert::der::Decode;
use x509_cert::ext::pkix::{self, ExtendedKeyUsage, name::GeneralName};
use x509_cert::ext::{Extension, ToExtension};
use x509_cert::name::Name;
use x509_cert::serial_number::SerialNumber;
use x509_cert::spki::{SubjectPublicKeyInfoOwned, SubjectPublicKeyInfoRef};
use x509_cert::time::Validity;

use crate::error::{ApiError, ApiResult};

/// Self-signed Hue certificate with Hue-specific extensions.
struct HueProfile(Name);

impl BuilderProfile for HueProfile {
    fn get_issuer(&self, subject: &Name) -> Name {
        /* self-signed certificate, so subject == issuer */
        subject.clone()
    }

    fn get_subject(&self) -> Name {
        self.0.clone()
    }

    fn build_extensions(
        &self,
        spki: SubjectPublicKeyInfoRef<'_>,
        _issuer_spk: SubjectPublicKeyInfoRef<'_>,
        tbs: &TbsCertificate,
    ) -> x509_cert::builder::Result<Vec<Extension>> {
        let mut extensions = Vec::new();
        let subject = tbs.subject();

        /* Basic constraints extension */
        extensions.push(
            pkix::BasicConstraints {
                ca: false,
                path_len_constraint: None,
            }
            .to_extension(subject, &extensions)?,
        );

        /* Key Usage extension */
        extensions.push(
            pkix::KeyUsage(pkix::KeyUsages::DigitalSignature.into())
                .to_extension(subject, &extensions)?,
        );

        /* Extended Key Usage extension */
        extensions.push(
            (true, &ExtendedKeyUsage(vec![ID_KP_SERVER_AUTH]))
                .to_extension(subject, &extensions)?,
        );

        /* Subject Key Identifier extension */
        extensions.push(
            pkix::SubjectKeyIdentifier::try_from(spki.clone())?
                .to_extension(subject, &extensions)?,
        );

        /* Authority Key Identifier extension */
        let mut aki = pkix::AuthorityKeyIdentifier::try_from(spki.clone())?;
        aki.key_identifier = Some(OctetString::new(
            Sha1::digest(spki.subject_public_key.raw_bytes()).as_slice(),
        )?);
        aki.authority_cert_issuer = Some(vec![GeneralName::DirectoryName(tbs.issuer().clone())]);
        aki.authority_cert_serial_number = Some(tbs.serial_number().clone());
        extensions.push(aki.to_extension(subject, &extensions)?);

        Ok(extensions)
    }
}

/// Generate a self-signed X509 certificate, closely matching the type and style
/// used by a real Philips Hue bridge.
///
/// Great care has been taken to match the real certificates as close as
/// possible. These certificates match on the following parameters:
///
///  - Style of subject name
///  - Valid from/to
///  - Type of private key, signing key, and signature type
///  - Choice of X509v3 extensions, including the order of them
///
/// Only these differences are known to remain:
///
///  - This version generates "Extended Key Usage" *with* the "critical"
///    flag set, while the real certificates (unusually) don't.
///    This seems to be a benign difference, and would roughly double
///    the code size needed.
///  - Real hue bridge certificates are signed by a "root-bridge" cert,
///    acting as a kind of CA certificate for the instance certificate.
///    This also seems to have no negative impact.
///
pub fn generate(secret_key: &p256::SecretKey, mac: MacAddress) -> ApiResult<CertificateInner> {
    let public_key = secret_key.public_key();

    let bridge_id = hue::bridge_id(mac);

    let subject = Name::from_str(&format!("CN={bridge_id},O=Philips Hue,C=NL"))?;

    let serial_number = SerialNumber::new(&hue::bridge_id_raw(mac))?;

    /* Philips Hue seems to start their certificates at the beginning of 2017.. */
    let not_before = GeneralizedTime::from_date_time(DateTime::new(2017, 1, 1, 0, 0, 0)?).into();

    /* ..and end on the Y38K boundary (https://en.wikipedia.org/wiki/Year_2038_problem) */
    let not_after = GeneralizedTime::from_date_time(DateTime::new(2038, 1, 19, 3, 14, 7)?).into();

    let validity = Validity::new(not_before, not_after);

    /* Hue certs have an unusual combination of X509 extensions. */
    let profile = HueProfile(subject);

    let signer = ecdsa::SigningKey::<p256::NistP256>::from(secret_key);
    let pub_key = SubjectPublicKeyInfoOwned::from_key(&public_key)?;

    /* Make certificate builder, which will allow us to build the final cert */
    let builder = CertificateBuilder::new(profile, serial_number, validity, pub_key)?;

    /* Finally ready to build the certificate */
    Ok(builder.build::<_, DerSignature>(&signer)?)
}

pub fn extract_common_name(rdr: impl Read) -> ApiResult<Option<String>> {
    for chunk in CertificateDer::pem_reader_iter(rdr) {
        let cert = Certificate::from_der(&chunk?)?;

        for name in cert.tbs_certificate().subject().iter_rdn() {
            if let [
                AttributeTypeAndValue {
                    oid: COMMON_NAME,
                    value,
                },
            ] = name.as_ref().as_slice()
            {
                return Ok(Some(String::from_utf8(value.value().to_vec())?));
            }
        }
    }

    Ok(None)
}

pub fn generate_and_save(certpath: &Utf8Path, mac: MacAddress) -> ApiResult<()> {
    let secret_key = p256::SecretKey::generate();
    let cert = generate(&secret_key, mac)?;
    let mut fd = File::create(certpath)?;
    fd.write_all(secret_key.to_pkcs8_pem(LineEnding::LF)?.as_bytes())?;
    fd.write_all(cert.to_pem(LineEnding::LF)?.as_bytes())?;
    Ok(())
}

pub fn check_certificate(certpath: &Utf8Path, mac: MacAddress) -> ApiResult<()> {
    let cn = extract_common_name(File::open(certpath)?)?;
    let id = hue::bridge_id(mac);
    match cn {
        Some(cn) => {
            if cn == id {
                log::debug!("Found existing certificate for bridge id [{id}]");
            } else {
                log::warn!("Certificate found, but common name (CN) does not match!");
                log::warn!("  [{id}] (expected)");
                log::warn!("  [{cn}] {certpath}");
                return Ok(());
            }
        }
        None => {
            return Err(ApiError::CertificateInvalid(certpath.to_owned()));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use der::{DecodePem, Encode};
    use openssl::pkey::PKey;
    use openssl::x509::X509;

    const MAC: [u8; 6] = [0x00, 0x17, 0x88, 0x01, 0x02, 0x03];
    // generated using x509-cert 0.2.5
    const CERTIFICATE: &[u8] = br"-----BEGIN CERTIFICATE-----
MIICODCCAd6gAwIBAgIHF4j//gECAzAKBggqhkjOPQQDAjA+MQswCQYDVQQGEwJO
TDEUMBIGA1UECgwLUGhpbGlwcyBIdWUxGTAXBgNVBAMMEDAwMTc4OGZmZmUwMTAy
MDMwHhcNMTcwMTAxMDAwMDAwWhcNMzgwMTE5MDMxNDA3WjA+MQswCQYDVQQGEwJO
TDEUMBIGA1UECgwLUGhpbGlwcyBIdWUxGTAXBgNVBAMMEDAwMTc4OGZmZmUwMTAy
MDMwWTATBgcqhkjOPQIBBggqhkjOPQMBBwNCAARv8DuUkkHOHa3UNRnmlg4KhbQa
aaBcMoEDqivOFZTKFjxPdTpVvwHcU/bAsMfu54tAxv99JaluIoK5ic73HBRKo4HG
MIHDMAwGA1UdEwEB/wQCMAAwDgYDVR0PAQH/BAQDAgeAMBYGA1UdJQEB/wQMMAoG
CCsGAQUFBwMBMB0GA1UdDgQWBBQuBix5Zl580sMuRht8V8MsAPQUMDBsBgNVHSME
ZTBjgBQuBix5Zl580sMuRht8V8MsAPQUMKFCpEAwPjELMAkGA1UEBhMCTkwxFDAS
BgNVBAoMC1BoaWxpcHMgSHVlMRkwFwYDVQQDDBAwMDE3ODhmZmZlMDEwMjAzggcX
iP/+AQIDMAoGCCqGSM49BAMCA0gAMEUCIQCBqon5HAJrHVMu8COTbch4uzlaZqnU
Ve86cp9QOSbntQIge8PLkHjPf0up9c/A+JFe42SVYBpD21lLgltlgE8J/KM=
-----END CERTIFICATE-----
";

    #[test]
    fn certificate_contents() -> ApiResult<()> {
        let key = p256::SecretKey::from_slice(&[1; 32]).unwrap();
        let expected = Certificate::from_pem(CERTIFICATE)?;
        let actual = generate(&key, MacAddress::new(MAC))?;

        // ECDSA signatures can vary with the same input so we compare the data before signing instead
        assert_eq!(
            actual.tbs_certificate().to_der()?,
            expected.tbs_certificate().to_der()?
        );
        assert_eq!(actual.signature_algorithm(), expected.signature_algorithm());
        Ok(())
    }

    #[test]
    fn certificate_fields() -> ApiResult<()> {
        let key = p256::SecretKey::from_slice(&[1; 32]).unwrap();
        let mac = MacAddress::new([0xec, 0xb5, 0xfa, 0x2b, 0xce, 0x8c]);
        let cert = generate(&key, mac)?;
        let tbs = cert.tbs_certificate();

        assert_eq!(tbs.version(), x509_cert::certificate::Version::V3);
        assert_eq!(
            tbs.subject(),
            &Name::from_str("CN=ecb5fafffe2bce8c,O=Philips Hue,C=NL")?
        );
        assert_eq!(tbs.issuer(), tbs.subject());
        // DER INTEGER with nine content bytes: a leading zero plus the eight-byte bridge ID
        assert_eq!(
            tbs.serial_number().to_der()?,
            [
                0x02, 0x09, 0x00, 0xec, 0xb5, 0xfa, 0xff, 0xfe, 0x2b, 0xce, 0x8c
            ]
        );
        let aki = tbs
            .get_extension::<pkix::AuthorityKeyIdentifier>()?
            .unwrap()
            .1;
        assert_eq!(
            aki.authority_cert_serial_number.as_ref(),
            Some(tbs.serial_number())
        );
        assert_eq!(
            tbs.validity().not_before.to_date_time(),
            DateTime::new(2017, 1, 1, 0, 0, 0)?
        );
        assert_eq!(
            tbs.validity().not_after.to_date_time(),
            DateTime::new(2038, 1, 19, 3, 14, 7)?
        );
        //  ECDSA with SHA-256
        assert_eq!(tbs.signature().oid.to_string(), "1.2.840.10045.4.3.2");
        assert_eq!(tbs.signature().parameters, None);
        assert_eq!(cert.signature_algorithm(), tbs.signature());
        Ok(())
    }

    #[test]
    fn certificate_extensions() -> ApiResult<()> {
        let key = p256::SecretKey::from_slice(&[1; 32]).unwrap();
        let cert = generate(&key, MacAddress::new(MAC))?;
        let tbs = cert.tbs_certificate();
        let extensions = tbs.extensions().unwrap();
        let order: Vec<_> = extensions
            .iter()
            .map(|ext| (ext.extn_id.to_string(), ext.critical))
            .collect();
        assert_eq!(
            order,
            [
                // Basic Constraints
                ("2.5.29.19".into(), true),
                // Key Usage
                ("2.5.29.15".into(), true),
                // Extended key usage
                ("2.5.29.37".into(), true),
                // Subject Key Identifier
                ("2.5.29.14".into(), false),
                // Authority Key Identifier
                ("2.5.29.35".into(), false),
            ]
        );
        assert_eq!(
            tbs.get_extension::<pkix::BasicConstraints>()?.unwrap().1,
            pkix::BasicConstraints {
                ca: false,
                path_len_constraint: None,
            }
        );
        assert_eq!(
            tbs.get_extension::<pkix::KeyUsage>()?.unwrap().1,
            pkix::KeyUsage(pkix::KeyUsages::DigitalSignature.into())
        );
        assert_eq!(
            tbs.get_extension::<ExtendedKeyUsage>()?.unwrap().1,
            ExtendedKeyUsage(vec![ID_KP_SERVER_AUTH])
        );

        let ski = tbs
            .get_extension::<pkix::SubjectKeyIdentifier>()?
            .unwrap()
            .1;
        assert_eq!(
            hex::encode(ski.0.as_bytes()),
            "2e062c79665e7cd2c32e461b7c57c32c00f41430"
        );
        let aki = tbs
            .get_extension::<pkix::AuthorityKeyIdentifier>()?
            .unwrap()
            .1;
        assert_eq!(aki.key_identifier, Some(ski.0));
        assert_eq!(
            aki.authority_cert_issuer,
            Some(vec![GeneralName::DirectoryName(tbs.issuer().clone())])
        );
        assert_eq!(
            aki.authority_cert_serial_number,
            Some(tbs.serial_number().clone())
        );
        Ok(())
    }

    #[test]
    fn certificate_is_signed_by_supplied_key() -> ApiResult<()> {
        let key = p256::SecretKey::from_slice(&[2; 32]).unwrap();
        let cert = X509::from_der(&generate(&key, MacAddress::new(MAC))?.to_der()?)?;
        let key = PKey::private_key_from_pem(key.to_pkcs8_pem(LineEnding::LF)?.as_bytes())?;

        assert!(cert.public_key()?.public_eq(&key));
        assert!(cert.verify(&key)?);
        Ok(())
    }

    #[test]
    fn common_name() -> ApiResult<()> {
        let key = p256::SecretKey::from_slice(&[1; 32]).unwrap();
        let mac = MacAddress::new([0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff]);
        let pem = generate(&key, mac)?.to_pem(LineEnding::LF)?;
        assert_eq!(
            extract_common_name(pem.as_bytes())?.as_deref(),
            Some("aabbccfffeddeeff")
        );
        Ok(())
    }
}
