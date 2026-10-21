//! Implémentation HTTP/1.1 minimale, sans dépendance externe.
//!
//! Le moteur ne parle qu'à des clients locaux : une seule requête par
//! connexion (`Connection: close`), en-têtes usuels, corps délimité par
//! `Content-Length`.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};

/// Taille maximale acceptée pour le corps d'une requête (octets).
pub const MAX_BODY_BYTES: usize = 8 * 1024 * 1024;

/// Erreurs du protocole HTTP.
#[derive(Debug, thiserror::Error)]
pub enum HttpError {
    /// Requête malformée (ligne, en-têtes, cible ou encodage).
    #[error("requête HTTP malformée : {0}")]
    Malformed(String),
    /// Corps de requête plus gros que la limite.
    #[error("corps de requête trop volumineux (maximum {MAX_BODY_BYTES} octets)")]
    BodyTooLarge,
    /// `Transfer-Encoding` : non supporté (le corps doit être délimité par
    /// `Content-Length`).
    #[error("en-tête « Transfer-Encoding » non supporté")]
    UnsupportedTransferEncoding,
    /// Erreur d'E/S.
    #[error("erreur d'E/S : {0}")]
    Io(#[from] std::io::Error),
}

/// Requête HTTP décodée.
#[derive(Debug, Clone)]
pub struct Request {
    /// Méthode HTTP (`GET`, `PUT`, …).
    pub method: String,
    /// Chemin de la requête, décodé (sans query string).
    pub path: String,
    /// Paramètres de la query string, décodés.
    pub query: HashMap<String, String>,
    /// Corps brut de la requête.
    pub body: Vec<u8>,
}

impl Request {
    /// Valeur d'un paramètre de query string.
    pub fn param(&self, name: &str) -> Option<&str> {
        self.query.get(name).map(String::as_str)
    }

    /// Corps de la requête interprété comme du texte UTF-8.
    pub fn body_text(&self) -> Result<&str, HttpError> {
        std::str::from_utf8(&self.body)
            .map_err(|_| HttpError::Malformed("corps de requête non UTF-8".to_string()))
    }
}

/// Réponse HTTP prête à être écrite sur le socket.
#[derive(Debug, Clone)]
pub struct Response {
    /// Code de statut HTTP.
    pub status: u16,
    /// Corps de la réponse (JSON).
    pub body: String,
}

impl Response {
    /// Réponse JSON à partir d'une valeur `serde_json`.
    pub fn json(status: u16, value: &serde_json::Value) -> Response {
        let body = serde_json::to_string(value)
            .unwrap_or_else(|_| "{\"error\":\"sérialisation JSON impossible\"}".to_string());
        Response { status, body }
    }

    /// Réponse d'erreur : `{"error":"message"}`.
    pub fn error(status: u16, message: impl Into<String>) -> Response {
        let body = serde_json::json!({ "error": message.into() }).to_string();
        Response { status, body }
    }

    /// Réponse complète (en-têtes + corps), prête à être écrite.
    pub fn as_bytes(&self) -> Vec<u8> {
        let head = format!(
            "HTTP/1.1 {} {}\r\nContent-Type: application/json; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            self.status,
            status_text(self.status),
            self.body.len()
        );
        let mut bytes = head.into_bytes();
        bytes.extend_from_slice(self.body.as_bytes());
        bytes
    }
}

/// Libellé textuel d'un code de statut.
pub fn status_text(status: u16) -> &'static str {
    match status {
        200 => "OK",
        400 => "Bad Request",
        404 => "Not Found",
        405 => "Method Not Allowed",
        411 => "Length Required",
        413 => "Payload Too Large",
        500 => "Internal Server Error",
        501 => "Not Implemented",
        503 => "Service Unavailable",
        _ => "Unknown",
    }
}

/// Lit et décode une requête HTTP sur `reader`.
pub fn read_request<R: Read>(
    reader: &mut BufReader<R>,
    max_body: usize,
) -> Result<Request, HttpError> {
    let request_line = read_line(reader)?
        .ok_or_else(|| HttpError::Malformed("ligne de requête absente".to_string()))?;
    let mut parts = request_line.split_whitespace();
    let method = parts
        .next()
        .ok_or_else(|| HttpError::Malformed("méthode absente".to_string()))?
        .to_string();
    let target = parts
        .next()
        .ok_or_else(|| HttpError::Malformed("cible absente".to_string()))?;
    if let Some(version) = parts.next() {
        if !version.starts_with("HTTP/1.") {
            return Err(HttpError::Malformed(format!(
                "version HTTP non supportée : {version}"
            )));
        }
    }

    let mut content_length = 0usize;
    loop {
        let Some(header) = read_line(reader)? else {
            return Err(HttpError::Malformed("en-têtes interrompus".to_string()));
        };
        if header.is_empty() {
            break;
        }
        let Some((name, value)) = header.split_once(':') else {
            continue;
        };
        match name.trim().to_ascii_lowercase().as_str() {
            "content-length" => {
                content_length = value.trim().parse::<usize>().map_err(|_| {
                    HttpError::Malformed("en-tête « Content-Length » invalide".to_string())
                })?;
                if content_length > max_body {
                    return Err(HttpError::BodyTooLarge);
                }
            }
            "transfer-encoding" => return Err(HttpError::UnsupportedTransferEncoding),
            _ => {}
        }
    }

    let mut body = vec![0u8; content_length];
    if content_length > 0 {
        reader.read_exact(&mut body)?;
    }

    let (path, query) = split_target(target)?;
    Ok(Request {
        method,
        path,
        query,
        body,
    })
}

/// Écrit une réponse HTTP complète sur `writer`.
pub fn write_response<W: Write>(writer: &mut W, response: &Response) -> Result<(), HttpError> {
    writer.write_all(&response.as_bytes())?;
    writer.flush()?;
    Ok(())
}

/// Découpe la cible d'une requête en chemin et paramètres.
fn split_target(target: &str) -> Result<(String, HashMap<String, String>), HttpError> {
    if target.starts_with("http://") || target.starts_with("https://") {
        return Err(HttpError::Malformed(
            "forme absolue de la cible non supportée".to_string(),
        ));
    }
    let (raw_path, raw_query) = match target.split_once('?') {
        Some((path, query)) => (path, Some(query)),
        None => (target, None),
    };
    let path = percent_decode(raw_path, false)?;
    let mut query = HashMap::new();
    if let Some(raw_query) = raw_query {
        for pair in raw_query.split('&') {
            if pair.is_empty() {
                continue;
            }
            let (key, value) = match pair.split_once('=') {
                Some((key, value)) => (key, value),
                None => (pair, ""),
            };
            query.insert(percent_decode(key, true)?, percent_decode(value, true)?);
        }
    }
    Ok((path, query))
}

/// Décode une chaîne encodée en pourcentage (`%XX`).
///
/// `plus_as_space` active la conversion de `+` en espace (convention des
/// query strings) ; elle est désactivée pour les chemins, où `+` est un
/// caractère littéral.
fn percent_decode(input: &str, plus_as_space: bool) -> Result<String, HttpError> {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0usize;
    while index < bytes.len() {
        match bytes[index] {
            b'%' => {
                if index + 2 >= bytes.len() {
                    return Err(HttpError::Malformed(
                        "séquence d'échappement incomplète".to_string(),
                    ));
                }
                let hex = std::str::from_utf8(&bytes[index + 1..index + 3]).map_err(|_| {
                    HttpError::Malformed("séquence d'échappement invalide".to_string())
                })?;
                let value = u8::from_str_radix(hex, 16).map_err(|_| {
                    HttpError::Malformed("séquence d'échappement invalide".to_string())
                })?;
                out.push(value);
                index += 3;
            }
            b'+' if plus_as_space => {
                out.push(b' ');
                index += 1;
            }
            other => {
                out.push(other);
                index += 1;
            }
        }
    }
    String::from_utf8(out)
        .map_err(|_| HttpError::Malformed("paramètre mal encodé (UTF-8 invalide)".to_string()))
}

/// Lit une ligne (terminée par `\n`), `None` en fin de flux.
fn read_line<R: Read>(reader: &mut BufReader<R>) -> Result<Option<String>, HttpError> {
    let mut buffer = Vec::new();
    let read = reader.read_until(b'\n', &mut buffer)?;
    if read == 0 {
        return Ok(None);
    }
    let text = String::from_utf8_lossy(&buffer);
    Ok(Some(text.trim_end_matches(['\r', '\n']).to_string()))
}
