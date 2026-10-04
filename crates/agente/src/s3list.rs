//! Repositorios dentro de un bucket S3: una lista mínima con ListObjectsV2
//! firmada con SigV4 (solo lectura, sin descargar nada). Un repositorio de
//! restic es un prefijo que contiene el objeto `config`.

use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};

type HmacSha256 = Hmac<Sha256>;

/// Cuántos prefijos se miran como mucho (para no hacer cientos de peticiones).
const MAX_PREFIXES: usize = 60;

pub struct Bucket {
    /// `https://host` (sin barra final).
    pub endpoint: String,
    pub host: String,
    pub bucket: String,
    pub region: String,
    pub key_id: String,
    pub secret: String,
}

impl Bucket {
    /// A partir de una ubicación `s3:https://host/bucket/…` (o `s3:host/bucket/…`).
    pub fn from_location(location: &str, region: Option<&str>, key_id: &str, secret: &str) -> Option<Self> {
        let rest = location.trim().strip_prefix("s3:")?;
        let (scheme, rest) = if let Some(r) = rest.strip_prefix("https://") {
            ("https", r)
        } else if let Some(r) = rest.strip_prefix("http://") {
            ("http", r)
        } else {
            ("https", rest)
        };
        let mut it = rest.split('/');
        let host = it.next()?.rsplit('@').next()?.to_lowercase();
        let bucket = it.next().filter(|b| !b.is_empty())?.to_string();
        let region = region.filter(|r| !r.trim().is_empty()).map(str::to_string).unwrap_or_else(|| region_from_host(&host));
        Some(Self { endpoint: format!("{scheme}://{host}"), host, bucket, region, key_id: key_id.into(), secret: secret.into() })
    }
}

/// Región por el nombre del servidor (`s3.us-west-004.backblazeb2.com` → `us-west-004`).
fn region_from_host(host: &str) -> String {
    let parts: Vec<&str> = host.split('.').collect();
    if parts.len() >= 3 && (parts[0] == "s3" || parts[0].starts_with("s3-")) {
        let r = parts[1];
        if r != "amazonaws" && !r.is_empty() {
            return r.to_string();
        }
    }
    "us-east-1".into()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn sha256_hex(data: &[u8]) -> String {
    hex(&Sha256::digest(data))
}

fn hmac(key: &[u8], data: &[u8]) -> Vec<u8> {
    let mut mac = HmacSha256::new_from_slice(key).expect("HMAC acepta cualquier longitud de clave");
    mac.update(data);
    mac.finalize().into_bytes().to_vec()
}

/// Clave de firma de SigV4 (`AWS4` + secreto → fecha → región → servicio → `aws4_request`).
pub fn signing_key(secret: &str, date: &str, region: &str, service: &str) -> Vec<u8> {
    let k_date = hmac(format!("AWS4{secret}").as_bytes(), date.as_bytes());
    let k_region = hmac(&k_date, region.as_bytes());
    let k_service = hmac(&k_region, service.as_bytes());
    hmac(&k_service, b"aws4_request")
}

/// Codificación de SigV4 (RFC 3986; `/` también se codifica salvo en la ruta).
fn uri_encode(s: &str, keep_slash: bool) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => (b as char).to_string(),
            b'/' if keep_slash => "/".into(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

/// Cabeceras firmadas de un GET (path-style) con su `Authorization`.
pub fn sign_get(b: &Bucket, path: &str, query: &[(&str, String)], amz_date: &str) -> (String, Vec<(String, String)>) {
    let date = &amz_date[..8];
    let mut q: Vec<(String, String)> = query.iter().map(|(k, v)| (uri_encode(k, false), uri_encode(v, false))).collect();
    q.sort();
    let canonical_query = q.iter().map(|(k, v)| format!("{k}={v}")).collect::<Vec<_>>().join("&");
    let payload = "UNSIGNED-PAYLOAD";
    let canonical_uri = uri_encode(path, true);
    let canonical_request = format!(
        "GET\n{canonical_uri}\n{canonical_query}\nhost:{}\nx-amz-content-sha256:{payload}\nx-amz-date:{amz_date}\n\nhost;x-amz-content-sha256;x-amz-date\n{payload}",
        b.host
    );
    let scope = format!("{date}/{}/s3/aws4_request", b.region);
    let to_sign = format!("AWS4-HMAC-SHA256\n{amz_date}\n{scope}\n{}", sha256_hex(canonical_request.as_bytes()));
    let signature = hex(&hmac(&signing_key(&b.secret, date, &b.region, "s3"), to_sign.as_bytes()));
    let auth = format!("AWS4-HMAC-SHA256 Credential={}/{scope}, SignedHeaders=host;x-amz-content-sha256;x-amz-date, Signature={signature}", b.key_id);
    let url = format!("{}{canonical_uri}?{canonical_query}", b.endpoint);
    (url, vec![("x-amz-content-sha256".into(), payload.into()), ("x-amz-date".into(), amz_date.into()), ("Authorization".into(), auth)])
}

/// Valores de una etiqueta XML (`<Prefix>…</Prefix>`), sin un parser completo.
fn tags<'a>(xml: &'a str, outer: &str, inner: &str) -> Vec<String> {
    let mut out = Vec::new();
    let (open, close) = (format!("<{outer}>"), format!("</{outer}>"));
    let mut rest: &'a str = xml;
    while let Some(i) = rest.find(&open) {
        let after = &rest[i + open.len()..];
        let Some(j) = after.find(&close) else { break };
        let block = &after[..j];
        let (io, ic) = (format!("<{inner}>"), format!("</{inner}>"));
        if let (Some(a), Some(b)) = (block.find(&io), block.find(&ic)) {
            out.push(unescape(&block[a + io.len()..b]));
        }
        rest = &after[j + close.len()..];
    }
    out
}

fn unescape(s: &str) -> String {
    s.replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&apos;", "'").replace("&amp;", "&")
}

/// Una página de ListObjectsV2 con delimitador: (prefijos, claves).
fn list(b: &Bucket, prefix: &str) -> Result<(Vec<String>, Vec<String>), String> {
    let amz_date = chrono::Utc::now().format("%Y%m%dT%H%M%SZ").to_string();
    let query = [("list-type", "2".to_string()), ("delimiter", "/".to_string()), ("prefix", prefix.to_string()), ("max-keys", "1000".to_string())];
    let (url, headers) = sign_get(b, &format!("/{}", b.bucket), &query, &amz_date);
    let agent: ureq::Agent = ureq::Agent::config_builder().timeout_global(Some(std::time::Duration::from_secs(20))).http_status_as_error(false).build().into();
    let mut req = agent.get(&url);
    for (k, v) in &headers {
        req = req.header(k, v);
    }
    let mut resp = req.call().map_err(|e| format!("No se pudo conectar con la nube: {e}"))?;
    let status = resp.status().as_u16();
    let body = resp.body_mut().read_to_string().unwrap_or_default();
    if status == 403 {
        return Err("La clave no permite listar el bucket (falta el permiso de listar). Puedes escribir la ruta del repositorio a mano.".into());
    }
    if status != 200 {
        let code = tags(&format!("<x>{body}</x>"), "Error", "Code").into_iter().next().unwrap_or_default();
        return Err(format!("La nube respondió con un error ({status} {code})."));
    }
    let prefixes = tags(&body, "CommonPrefixes", "Prefix");
    let keys = tags(&body, "Contents", "Key");
    Ok((prefixes, keys))
}

/// Prefijos del bucket que son repositorios de restic (hasta dos niveles).
pub fn find_repos(b: &Bucket) -> Result<Vec<String>, String> {
    let mut found = Vec::new();
    let (level1, keys) = list(b, "")?;
    if keys.iter().any(|k| k == "config") {
        found.push(String::new());
    }
    let mut looked = 0;
    for p1 in level1.iter().take(MAX_PREFIXES) {
        looked += 1;
        let (level2, keys) = list(b, p1)?;
        if keys.iter().any(|k| *k == format!("{p1}config")) {
            found.push(p1.trim_end_matches('/').to_string());
            continue;
        }
        for p2 in level2.iter().take(MAX_PREFIXES.saturating_sub(looked)) {
            looked += 1;
            let (_, keys) = list(b, p2)?;
            if keys.iter().any(|k| *k == format!("{p2}config")) {
                found.push(p2.trim_end_matches('/').to_string());
            }
        }
        if looked >= MAX_PREFIXES {
            break;
        }
    }
    Ok(found)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clave_de_firma_como_aws() {
        // Ejemplo de la documentación de AWS (SigV4, «Deriving the signing key»).
        let k = signing_key("wJalrXUtnFEMI/K7MDENG+bPxRfiCYEXAMPLEKEY", "20150830", "us-east-1", "iam");
        assert_eq!(hex(&k), "c4afb1cc5771d871763a393e44b703571b55cc28424d1a5e86da6ed3c154a4b9");
    }

    #[test]
    fn firma_una_lista() {
        let b = Bucket::from_location("s3:https://s3.us-west-004.backblazeb2.com/copias-ana/portatil", None, "KEY", "SECRET").unwrap();
        assert_eq!((b.host.as_str(), b.bucket.as_str(), b.region.as_str()), ("s3.us-west-004.backblazeb2.com", "copias-ana", "us-west-004"));
        let (url, headers) =
            sign_get(&b, "/copias-ana", &[("list-type", "2".into()), ("prefix", "a b/".into()), ("delimiter", "/".into())], "20261001T120000Z");
        assert_eq!(url, "https://s3.us-west-004.backblazeb2.com/copias-ana?delimiter=%2F&list-type=2&prefix=a%20b%2F");
        let auth = &headers.iter().find(|h| h.0 == "Authorization").unwrap().1;
        assert!(auth.starts_with(
            "AWS4-HMAC-SHA256 Credential=KEY/20261001/us-west-004/s3/aws4_request, SignedHeaders=host;x-amz-content-sha256;x-amz-date, Signature="
        ));
        assert_eq!(auth.rsplit('=').next().unwrap().len(), 64);
        assert!(Bucket::from_location("s3:s3.amazonaws.com/b", None, "k", "s").is_some_and(|b| b.region == "us-east-1"));
    }

    #[test]
    fn lee_la_respuesta() {
        let xml = "<ListBucketResult><Contents><Key>a/config</Key></Contents><CommonPrefixes><Prefix>a/keys/</Prefix></CommonPrefixes><CommonPrefixes><Prefix>b&amp;c/</Prefix></CommonPrefixes></ListBucketResult>";
        assert_eq!(tags(xml, "Contents", "Key"), ["a/config"]);
        assert_eq!(tags(xml, "CommonPrefixes", "Prefix"), ["a/keys/", "b&c/"]);
    }
}
