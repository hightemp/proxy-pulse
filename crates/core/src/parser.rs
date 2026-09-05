use crate::model::{AppError, AppResult, Credentials, Protocol, Proxy};
use percent_encoding::percent_decode_str;
use serde::{Deserialize, Serialize};
use std::net::IpAddr;

pub const MAX_BYTES: usize = 20 * 1024 * 1024;
pub const MAX_ROWS: usize = 100_000;
const MAX_LINE: usize = 8192;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ImportOptions {
    pub format: String,
    pub delimiter: String,
    pub header: String,
    pub columns: Vec<String>,
    pub source_name: String,
}

impl Default for ImportOptions {
    fn default() -> Self {
        Self {
            format: "auto".into(),
            delimiter: ",".into(),
            header: "auto".into(),
            columns: Vec::new(),
            source_name: "Pasted text".into(),
        }
    }
}

#[derive(Clone)]
pub struct ParsedRow {
    pub proxy: Option<Proxy>,
    pub error: Option<AppError>,
    pub raw: String,
    pub label: String,
    pub source: String,
    pub line: usize,
    pub header: Option<String>,
    pub delimiter: Option<u8>,
}

pub struct ParsedImport {
    pub rows: Vec<ParsedRow>,
    pub ignored: usize,
}

fn error(code: &str, message: &str) -> AppError {
    AppError::new(code, message)
}

fn decode(value: &str) -> AppResult<String> {
    let bytes = value.as_bytes();
    for (i, byte) in bytes.iter().enumerate() {
        if *byte == b'%'
            && (i + 2 >= bytes.len()
                || !bytes[i + 1].is_ascii_hexdigit()
                || !bytes[i + 2].is_ascii_hexdigit())
        {
            return Err(error(
                "INVALID_FORMAT",
                "Invalid percent-encoding in credentials.",
            ));
        }
    }
    percent_decode_str(value)
        .decode_utf8()
        .map(|s| s.into_owned())
        .map_err(|_| error("INVALID_FORMAT", "Credentials must be valid UTF-8."))
}

fn host(value: &str) -> AppResult<String> {
    let value = value.trim();
    if value.is_empty()
        || value.contains(['/', '?', '#', '@', '%'])
        || value.chars().any(char::is_whitespace)
    {
        return Err(error(
            "INVALID_HOST",
            "Enter an IPv4 address, bracketed IPv6 address or DNS name.",
        ));
    }
    if value.starts_with('[') && value.ends_with(']') {
        return value[1..value.len() - 1]
            .parse::<std::net::Ipv6Addr>()
            .map(|ip| ip.to_string())
            .map_err(|_| error("INVALID_HOST", "Invalid IPv6 address."));
    }
    if value.contains(':') {
        return Err(error(
            "INVALID_HOST",
            "Enclose IPv6 addresses in square brackets.",
        ));
    }
    if value.chars().all(|c| c.is_ascii_digit() || c == '.') {
        return value
            .parse::<std::net::Ipv4Addr>()
            .map(|ip| ip.to_string())
            .map_err(|_| error("INVALID_HOST", "Invalid IPv4 address."));
    }
    match url::Host::parse(value) {
        Ok(url::Host::Domain(domain))
            if domain.len() <= 253
                && domain.trim_end_matches('.').split('.').all(|label| {
                    !label.is_empty()
                        && label.len() <= 63
                        && !label.starts_with('-')
                        && !label.ends_with('-')
                        && label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
                }) =>
        {
            Ok(domain.to_ascii_lowercase())
        }
        _ => Err(error("INVALID_HOST", "Invalid DNS name.")),
    }
}

fn make_proxy(
    host_text: &str,
    port: &str,
    username: Option<String>,
    password: Option<String>,
    protocol: Protocol,
) -> AppResult<Proxy> {
    let port = if !port.is_empty() && port.bytes().all(|b| b.is_ascii_digit()) {
        port.parse::<u16>().ok().filter(|p| *p != 0)
    } else {
        None
    }
    .ok_or_else(|| error("INVALID_PORT", "Port must be an integer from 1 to 65535."))?;
    if username
        .iter()
        .chain(password.iter())
        .any(|s| s.chars().any(char::is_control))
    {
        return Err(error(
            "INVALID_FORMAT",
            "Credentials must not contain control characters.",
        ));
    }
    if username.as_ref().is_some_and(String::is_empty) || (username.is_none() && password.is_some())
    {
        return Err(error(
            "UNSUPPORTED_AUTH",
            "A nonempty username is required when credentials are supplied.",
        ));
    }
    let proxy = Proxy {
        host: host(host_text)?,
        port,
        protocol,
        credentials: username.map(|username| Credentials { username, password }),
    };
    proxy.validate_auth(protocol)?;
    Ok(proxy)
}

fn split_address(value: &str) -> AppResult<(&str, &str)> {
    if value.starts_with('[') {
        let end = value
            .find(']')
            .ok_or_else(|| error("INVALID_HOST", "Missing closing IPv6 bracket."))?;
        let remainder = value[end + 1..]
            .strip_prefix(':')
            .ok_or_else(|| error("INVALID_PORT", "An explicit port is required."))?;
        Ok((&value[..=end], remainder))
    } else {
        value
            .split_once(':')
            .ok_or_else(|| error("INVALID_PORT", "An explicit port is required."))
    }
}

fn uri(value: &str) -> AppResult<Proxy> {
    let (scheme, authority) = value
        .split_once("://")
        .ok_or_else(|| error("INVALID_FORMAT", "Invalid proxy URI."))?;
    let protocol = Protocol::parse(scheme)?;
    if protocol == Protocol::Auto {
        return Err(error(
            "UNSUPPORTED_PROTOCOL",
            "Omit the scheme to use automatic detection.",
        ));
    }
    let url = url::Url::parse(value).map_err(|_| {
        error(
            "INVALID_FORMAT",
            "Invalid proxy URI. Encode reserved characters in credentials.",
        )
    })?;
    if !matches!(url.path(), "" | "/") || url.query().is_some() || url.fragment().is_some() {
        return Err(error(
            "INVALID_FORMAT",
            "A proxy URI cannot contain a path, query or fragment.",
        ));
    }
    let authority = authority.strip_suffix('/').unwrap_or(authority);
    let (credentials, endpoint) = match authority.rsplit_once('@') {
        Some((auth, endpoint)) => (Some(auth), endpoint),
        None => (None, authority),
    };
    let (host_text, port) = split_address(endpoint)?;
    if port.contains(':') {
        return Err(error(
            "INVALID_FORMAT",
            "Put URI credentials before the host: scheme://user:password@host:port.",
        ));
    }
    let (username, password) = match credentials {
        Some(auth) => {
            if auth.contains('@') {
                return Err(error(
                    "INVALID_FORMAT",
                    "Encode @ inside URI credentials as %40.",
                ));
            }
            match auth.split_once(':') {
                Some((user, pass)) => (Some(decode(user)?), Some(decode(pass)?)),
                None => (Some(decode(auth)?), None),
            }
        }
        None => (None, None),
    };
    make_proxy(host_text, port, username, password, protocol)
}

fn compact(value: &str, protocol: Protocol, reverse: bool) -> AppResult<Proxy> {
    if reverse {
        let parts: Vec<_> = value.splitn(3, ':').collect();
        if parts.len() != 3 {
            return Err(error("INVALID_FORMAT", "Use username:password:host:port."));
        }
        let (host_text, port) = split_address(parts[2])?;
        return make_proxy(
            host_text,
            port,
            Some(parts[0].into()),
            Some(parts[1].into()),
            protocol,
        );
    }
    if let Some((auth, endpoint)) = value.split_once('@') {
        let (user, pass) = auth
            .split_once(':')
            .ok_or_else(|| error("INVALID_FORMAT", "Use username:password@host:port."))?;
        if pass.contains(':') || endpoint.contains('@') {
            return Err(error(
                "INVALID_FORMAT",
                "Use an encoded URI or mapped CSV for special characters.",
            ));
        }
        let (host_text, port) = split_address(endpoint)?;
        return make_proxy(
            host_text,
            port,
            Some(user.into()),
            Some(pass.into()),
            protocol,
        );
    }
    let (host_text, rest) = split_address(value)?;
    let parts: Vec<_> = rest.split(':').collect();
    match parts.as_slice() {
        [port] => make_proxy(host_text, port, None, None, protocol),
        [port, user, pass] => make_proxy(
            host_text,
            port,
            Some((*user).into()),
            Some((*pass).into()),
            protocol,
        ),
        _ => Err(error(
            "INVALID_FORMAT",
            "Use host:port, host:port:user:password or an encoded proxy URI.",
        )),
    }
}

pub fn parse_line(value: &str, reverse: bool) -> AppResult<Proxy> {
    let value = value.trim();
    if value.len() > MAX_LINE {
        return Err(error("LINE_TOO_LONG", "A record must not exceed 8 KiB."));
    }
    if value.chars().any(|c| c.is_control() && c != '\t') {
        return Err(error(
            "INVALID_FORMAT",
            "Control characters are not allowed.",
        ));
    }
    let mut parts: Vec<&str> = value.split_whitespace().collect();
    if parts.is_empty() {
        return Err(error("INVALID_FORMAT", "Enter a proxy address."));
    }
    let mut protocol = None;
    if parts.len() > 1 {
        if let Ok(p) = Protocol::parse(parts[0]) {
            protocol = Some(p);
            parts.remove(0);
        }
        if let Some(p) = parts.last().and_then(|p| Protocol::parse(p).ok()) {
            if protocol.is_some_and(|first| first != p) {
                return Err(error(
                    "PROTOCOL_CONFLICT",
                    "Protocol declarations disagree.",
                ));
            }
            protocol = Some(p);
            parts.pop();
        }
    }
    if parts.len() == 1 {
        let mut result = if parts[0].contains("://") {
            uri(parts[0])?
        } else {
            compact(parts[0], protocol.unwrap_or(Protocol::Auto), reverse)?
        };
        if let Some(p) = protocol {
            if result.protocol != Protocol::Auto && result.protocol != p {
                return Err(error(
                    "PROTOCOL_CONFLICT",
                    "The URI scheme and protocol token disagree.",
                ));
            }
            result.protocol = p;
        }
        result.validate_auth(result.protocol)?;
        return Ok(result);
    }
    match parts.as_slice() {
        [host_text, port] => make_proxy(
            host_text,
            port,
            None,
            None,
            protocol.unwrap_or(Protocol::Auto),
        ),
        [host_text, port, user, pass] => make_proxy(
            host_text,
            port,
            Some((*user).into()),
            Some((*pass).into()),
            protocol.unwrap_or(Protocol::Auto),
        ),
        _ => Err(error(
            "INVALID_FORMAT",
            "Use host port [username password] [protocol], or select a column mapping.",
        )),
    }
}

fn column(value: &str) -> String {
    match value.trim().to_ascii_lowercase().as_str() {
        "host" | "ip" | "server" => "host",
        "port" => "port",
        "username" | "user" | "login" => "username",
        "password" | "pass" => "password",
        "protocol" | "type" | "scheme" => "protocol",
        "proxy" => "proxy",
        "label" => "label",
        _ => "ignore",
    }
    .into()
}

fn csv_quotes_valid(raw: &str, delimiter: u8) -> bool {
    // 0: field start, 1: unquoted, 2: quoted, 3: closing quote (or first escaped quote).
    let mut state = 0;
    for byte in raw.bytes() {
        if byte == b'"' {
            state = match state {
                0 | 3 => 2,
                2 => 3,
                _ => return false,
            };
        } else if byte == delimiter {
            if state != 2 {
                state = 0;
            }
        } else {
            if state == 3 {
                return false;
            }
            if state == 0 {
                state = 1;
            }
        }
    }
    state != 2
}

fn parse_record(record: &csv::StringRecord, columns: &[String]) -> AppResult<(Proxy, String)> {
    if record.len() != columns.len() {
        return Err(error(
            "INVALID_FORMAT",
            "Column count does not match the selected mapping.",
        ));
    }
    if record.iter().any(|v| v.contains(['\r', '\n'])) {
        return Err(error(
            "INVALID_FORMAT",
            "Multiline CSV fields are not supported.",
        ));
    }
    let get = |key: &str| {
        columns
            .iter()
            .position(|c| c == key)
            .and_then(|i| record.get(i))
    };
    let label = get("label").unwrap_or_default().to_owned();
    let protocol = get("protocol")
        .filter(|p| !p.trim().is_empty())
        .map(|p| Protocol::parse(p.trim()))
        .transpose()?;
    if let Some(value) = get("proxy") {
        let mut proxy = parse_line(value, false)?;
        if let Some(protocol) = protocol {
            if proxy.protocol != Protocol::Auto && proxy.protocol != protocol {
                return Err(error(
                    "PROTOCOL_CONFLICT",
                    "The proxy column and protocol column disagree.",
                ));
            }
            proxy.protocol = protocol;
        }
        proxy.validate_auth(proxy.protocol)?;
        return Ok((proxy, label));
    }
    let username = get("username").filter(|u| !u.is_empty()).map(str::to_owned);
    let password = get("password")
        .filter(|p| username.is_some() || !p.is_empty())
        .map(str::to_owned);
    let proxy = make_proxy(
        get("host").unwrap_or_default(),
        get("port").unwrap_or_default().trim(),
        username,
        password,
        protocol.unwrap_or(Protocol::Auto),
    )?;
    Ok((proxy, label))
}

pub fn parse_import(text: &str, options: &ImportOptions) -> AppResult<ParsedImport> {
    if text.len() > MAX_BYTES {
        return Err(error("IMPORT_TOO_LARGE", "Import must not exceed 20 MiB."));
    }
    let text = text.trim_start_matches('\u{feff}');
    let first = text
        .lines()
        .find(|line| !line.trim().is_empty())
        .unwrap_or_default();
    let delimiter = match options.format.as_str() {
        "csv" => Some(if options.delimiter == ";" { b';' } else { b',' }),
        "tsv" => Some(b'\t'),
        "auto" if first.contains('\t') => Some(b'\t'),
        "auto" if !first.contains("://") && first.contains(';') => Some(b';'),
        "auto" if !first.contains("://") && first.contains(',') => Some(b','),
        _ => None,
    };
    let mut rows = Vec::new();
    let mut ignored = 0;
    if let Some(delimiter) = delimiter {
        let mut columns = options
            .columns
            .iter()
            .map(|s| column(s))
            .collect::<Vec<_>>();
        let mut header = None;
        let mut first_record = true;
        for (index, raw) in text.lines().enumerate() {
            if raw.trim().is_empty() {
                ignored += 1;
                continue;
            }
            let line = index + 1;
            let result = if raw.len() > MAX_LINE {
                Err(error("LINE_TOO_LONG", "A record must not exceed 8 KiB."))
            } else if !csv_quotes_valid(raw, delimiter) {
                Err(error(
                    "INVALID_FORMAT",
                    "Invalid CSV quoting. Multiline fields are not supported.",
                ))
            } else {
                csv::ReaderBuilder::new()
                    .has_headers(false)
                    .flexible(true)
                    .delimiter(delimiter)
                    .from_reader(raw.as_bytes())
                    .records()
                    .next()
                    .ok_or_else(|| error("INVALID_FORMAT", "Empty CSV record."))?
                    .map_err(|_| error("INVALID_FORMAT", "Invalid CSV record."))
            };
            if first_record {
                first_record = false;
                if let Ok(record) = &result {
                    let mapped = record.iter().map(column).collect::<Vec<_>>();
                    let has_header = options.header == "yes"
                        || (options.header == "auto"
                            && ((mapped.contains(&"host".into())
                                && mapped.contains(&"port".into()))
                                || mapped.contains(&"proxy".into())));
                    if columns.is_empty() {
                        columns = if has_header {
                            mapped
                        } else {
                            let defaults: &[&str] = match record.len() {
                                1 => &["proxy"],
                                2 => &["host", "port"],
                                3 => &["host", "port", "protocol"],
                                4 => &["host", "port", "username", "password"],
                                5 => &["host", "port", "username", "password", "protocol"],
                                _ => &[],
                            };
                            defaults.iter().map(|s| (*s).into()).collect()
                        };
                    }
                    let mut unique = std::collections::HashSet::new();
                    if columns
                        .iter()
                        .filter(|c| c.as_str() != "ignore")
                        .any(|c| !unique.insert(c))
                    {
                        return Err(error(
                            "INVALID_MAPPING",
                            "Map each field to at most one column.",
                        ));
                    }
                    if has_header {
                        header = Some(raw.to_owned());
                        ignored += 1;
                        continue;
                    }
                    // A selected mapping must survive Original lines export and reimport.
                    header = Some(columns.join(&(delimiter as char).to_string()));
                }
            }
            let parsed = result.and_then(|r| parse_record(&r, &columns));
            let (proxy, issue, label) = match parsed {
                Ok((p, l)) => (Some(p), None, l),
                Err(e) => (None, Some(e), String::new()),
            };
            rows.push(ParsedRow {
                proxy,
                error: issue,
                raw: raw.to_owned(),
                label,
                source: options.source_name.clone(),
                line,
                header: header.clone(),
                delimiter: Some(delimiter),
            });
        }
    } else {
        for (index, line) in text.lines().enumerate() {
            let value = line.trim();
            if value.is_empty() || value.starts_with('#') || value.starts_with("//") {
                ignored += 1;
                continue;
            }
            let parsed = parse_line(value, options.format == "reverse");
            let (proxy, issue) = match parsed {
                Ok(p) => (Some(p), None),
                Err(e) => (None, Some(e)),
            };
            rows.push(ParsedRow {
                proxy,
                error: issue,
                raw: line.to_owned(),
                label: String::new(),
                source: options.source_name.clone(),
                line: index + 1,
                header: None,
                delimiter: None,
            });
        }
    }
    if rows.len() > MAX_ROWS {
        return Err(error(
            "TOO_MANY_ROWS",
            "Import must not exceed 100,000 records.",
        ));
    }
    Ok(ParsedImport { rows, ignored })
}

pub fn bracket_host(value: &str) -> String {
    if value.parse::<IpAddr>().is_ok_and(|ip| ip.is_ipv6()) {
        format!("[{value}]")
    } else {
        value.into()
    }
}
