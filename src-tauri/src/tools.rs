use std::process::Command;
use serde::{Deserialize, Serialize};

fn shell_cmd(script: &str) -> Command {
    #[cfg(unix)]
    {
        let mut cmd = Command::new("sh");
        cmd.arg("-c").arg(script);
        cmd
    }
    #[cfg(windows)]
    {
        let mut cmd = Command::new("cmd");
        cmd.arg("/C").arg(script);
        cmd
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct DnsRecord {
    pub name: String,
    pub ttl: String,
    pub record_class: String,
    pub record_type: String,
    pub value: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct DigResult {
    pub domain: String,
    pub records: Vec<DnsRecord>,
    pub query_time: String,
    pub server: String,
    pub status: String,
}

#[tauri::command]
pub fn dig_domain(domain: String, record_type: Option<String>) -> Result<DigResult, String> {
    let rt = record_type.unwrap_or_else(|| "ANY".to_string());

    let output = Command::new("dig")
        .arg(&domain)
        .arg(&rt)
        .arg("+noall")
        .arg("+answer")
        .arg("+stats")
        .arg("+comments")
        .output()
        .map_err(|e| format!("Failed to run dig: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("dig failed: {}", stderr));
    }

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();

    let mut records: Vec<DnsRecord> = Vec::new();
    let mut query_time = String::new();
    let mut server = String::new();
    let mut status = String::from("NOERROR");

    for line in stdout.lines() {
        let trimmed = line.trim();

        // Parse status from header comments
        if trimmed.starts_with(";; ->>HEADER<<-") {
            if let Some(s) = trimmed.split("status: ").nth(1) {
                if let Some(s) = s.split(',').next() {
                    status = s.trim().to_string();
                }
            }
            continue;
        }

        // Parse query time
        if trimmed.starts_with(";; Query time:") {
            query_time = trimmed.replace(";; Query time: ", "").trim().to_string();
            continue;
        }

        // Parse server
        if trimmed.starts_with(";; SERVER:") {
            server = trimmed.replace(";; SERVER: ", "").trim().to_string();
            continue;
        }

        // Skip comments and empty lines
        if trimmed.is_empty() || trimmed.starts_with(";;") || trimmed.starts_with(";") {
            continue;
        }

        // Parse answer records: name TTL CLASS TYPE VALUE
        let parts: Vec<&str> = trimmed.splitn(5, char::is_whitespace).collect();
        if parts.len() >= 5 {
            // Filter out empty parts from multiple whitespace
            let non_empty: Vec<&str> = trimmed.split_whitespace().collect();
            if non_empty.len() >= 5 {
                records.push(DnsRecord {
                    name: non_empty[0].to_string(),
                    ttl: non_empty[1].to_string(),
                    record_class: non_empty[2].to_string(),
                    record_type: non_empty[3].to_string(),
                    value: non_empty[4..].join(" "),
                });
            }
        }
    }

    Ok(DigResult {
        domain,
        records,
        query_time,
        server,
        status,
    })
}

#[derive(Serialize, Deserialize, Clone)]
pub struct TraceRecord {
    pub name: String,
    pub ttl: String,
    pub record_class: String,
    pub record_type: String,
    pub value: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct TraceHop {
    pub zone: String,
    pub server: String,
    pub server_ip: String,
    pub response_time: String,
    pub response_bytes: String,
    pub records: Vec<TraceRecord>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct DigTraceResult {
    pub domain: String,
    pub hops: Vec<TraceHop>,
    pub final_answers: Vec<TraceRecord>,
}

#[tauri::command]
pub fn dig_trace(domain: String, include_dnssec: Option<bool>) -> Result<DigTraceResult, String> {
    let output = Command::new("dig")
        .arg("+trace")
        .arg("+nodnssec")
        .arg(&domain)
        .output()
        .map_err(|e| format!("Failed to run dig +trace: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("dig +trace failed: {}", stderr));
    }

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let show_dnssec = include_dnssec.unwrap_or(false);

    // DNSSEC record types to filter out
    let dnssec_types = ["RRSIG", "NSEC", "NSEC3", "NSEC3PARAM", "DS", "DNSKEY"];

    let mut hops: Vec<TraceHop> = Vec::new();
    let mut current_records: Vec<TraceRecord> = Vec::new();
    let mut current_zone = String::new();

    for line in stdout.lines() {
        let trimmed = line.trim();

        // Skip empty lines, comments (except "Received" lines), and the header
        if trimmed.is_empty() || trimmed.starts_with(";; global options:") || trimmed.starts_with("; <<>>") {
            continue;
        }

        // Parse "Received" footer — marks end of a hop
        if trimmed.starts_with(";; Received") {
            // Format: ";; Received 239 bytes from 127.0.2.2#53(127.0.2.2) in 25 ms"
            let bytes = trimmed
                .split("Received ")
                .nth(1)
                .and_then(|s| s.split(" bytes").next())
                .unwrap_or("")
                .to_string();

            let server_ip = trimmed
                .split("from ")
                .nth(1)
                .and_then(|s| s.split('#').next())
                .unwrap_or("")
                .to_string();

            let server_name = trimmed
                .split('(')
                .nth(1)
                .and_then(|s| s.split(')').next())
                .unwrap_or(&server_ip)
                .to_string();

            let response_time = trimmed
                .split("in ")
                .last()
                .unwrap_or("")
                .to_string();

            if !current_records.is_empty() {
                hops.push(TraceHop {
                    zone: current_zone.clone(),
                    server: server_name,
                    server_ip,
                    response_time,
                    response_bytes: bytes,
                    records: current_records.clone(),
                });
                current_records.clear();
            }
            continue;
        }

        // Skip other comment lines
        if trimmed.starts_with(';') {
            continue;
        }

        // Parse DNS records: name TTL CLASS TYPE VALUE...
        // Handle continuation lines (lines starting with whitespace that are part of long values)
        let parts: Vec<&str> = trimmed.split_whitespace().collect();
        if parts.len() >= 5 {
            let record_type = parts[3];

            // Filter DNSSEC records unless explicitly requested
            if !show_dnssec && dnssec_types.contains(&record_type) {
                continue;
            }

            // Track the zone from the first record in each section
            if current_records.is_empty() {
                current_zone = parts[0].to_string();
            }

            current_records.push(TraceRecord {
                name: parts[0].to_string(),
                ttl: parts[1].to_string(),
                record_class: parts[2].to_string(),
                record_type: record_type.to_string(),
                value: parts[4..].join(" "),
            });
        }
    }

    // The last hop's records are the final answers
    let final_answers = if let Some(last) = hops.last() {
        last.records.clone()
    } else {
        Vec::new()
    };

    Ok(DigTraceResult {
        domain,
        hops,
        final_answers,
    })
}

#[derive(Serialize, Deserialize, Clone)]
pub struct CurlHeader {
    pub name: String,
    pub value: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct CurlTimings {
    pub dns_lookup: String,
    pub tcp_connect: String,
    pub tls_handshake: String,
    pub ttfb: String,
    pub total: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct CurlRedirect {
    pub status: u16,
    pub location: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct CurlResult {
    pub url: String,
    pub method: String,
    pub status_code: u16,
    pub status_text: String,
    pub response_headers: Vec<CurlHeader>,
    pub body: String,
    pub body_size: usize,
    pub timings: CurlTimings,
    pub redirects: Vec<CurlRedirect>,
    pub remote_ip: String,
    pub content_type: String,
    pub verbose_output: String,
}

#[tauri::command]
pub fn run_curl(
    url: String,
    method: Option<String>,
    headers: Option<Vec<String>>,
    body: Option<String>,
    follow_redirects: Option<bool>,
    extra_flags: Option<Vec<String>>,
) -> Result<CurlResult, String> {
    let method_str = method.unwrap_or_else(|| "GET".to_string());
    let follow = follow_redirects.unwrap_or(true);

    // Timing format string — curl writes this after the response
    let timing_format = "\n__CURL_META__\nstatus_code:%{http_code}\nremote_ip:%{remote_ip}\ntime_namelookup:%{time_namelookup}\ntime_connect:%{time_connect}\ntime_appconnect:%{time_appconnect}\ntime_starttransfer:%{time_starttransfer}\ntime_total:%{time_total}\ncontent_type:%{content_type}\n";

    let mut cmd = Command::new("curl");
    cmd.arg("-s") // silent (no progress)
        .arg("-i") // include response headers
        .arg("-X").arg(&method_str)
        .arg("-w").arg(timing_format);

    if follow {
        cmd.arg("-L");
    }

    // Add custom headers
    if let Some(ref hdrs) = headers {
        for h in hdrs {
            if !h.trim().is_empty() {
                cmd.arg("-H").arg(h);
            }
        }
    }

    // Add body
    if let Some(ref b) = body {
        if !b.trim().is_empty() {
            cmd.arg("-d").arg(b);
        }
    }

    // Add extra raw flags
    if let Some(ref flags) = extra_flags {
        for flag in flags {
            let trimmed = flag.trim();
            if !trimmed.is_empty() {
                // Split compound flags like "-svo /dev/null" into individual args
                for part in trimmed.split_whitespace() {
                    cmd.arg(part);
                }
            }
        }
    }

    cmd.arg(&url);

    let output = cmd
        .output()
        .map_err(|e| format!("Failed to run curl: {}", e))?;

    let raw = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    if !output.status.success() && raw.is_empty() {
        return Err(format!("curl failed: {}", stderr));
    }

    // Split on our meta marker
    let (http_part, meta_part) = if let Some(idx) = raw.find("\n__CURL_META__\n") {
        (&raw[..idx], &raw[idx..])
    } else {
        (raw.as_str(), "")
    };

    // Parse meta values
    let mut status_code: u16 = 0;
    let mut remote_ip = String::new();
    let mut time_namelookup = String::new();
    let mut time_connect = String::new();
    let mut time_appconnect = String::new();
    let mut time_starttransfer = String::new();
    let mut time_total = String::new();
    let mut content_type = String::new();

    for line in meta_part.lines() {
        if let Some(val) = line.strip_prefix("status_code:") {
            status_code = val.parse().unwrap_or(0);
        } else if let Some(val) = line.strip_prefix("remote_ip:") {
            remote_ip = val.to_string();
        } else if let Some(val) = line.strip_prefix("time_namelookup:") {
            time_namelookup = format_curl_time(val);
        } else if let Some(val) = line.strip_prefix("time_connect:") {
            time_connect = format_curl_time(val);
        } else if let Some(val) = line.strip_prefix("time_appconnect:") {
            time_appconnect = format_curl_time(val);
        } else if let Some(val) = line.strip_prefix("time_starttransfer:") {
            time_starttransfer = format_curl_time(val);
        } else if let Some(val) = line.strip_prefix("time_total:") {
            time_total = format_curl_time(val);
        } else if let Some(val) = line.strip_prefix("content_type:") {
            content_type = val.to_string();
        }
    }

    // Parse HTTP response(s) — may have multiple if redirects
    // Each starts with "HTTP/x.x STATUS TEXT\r\n" and ends with "\r\n\r\n"
    let mut redirects: Vec<CurlRedirect> = Vec::new();
    let mut final_headers: Vec<CurlHeader> = Vec::new();
    let mut final_status_text = String::new();
    let body;

    // Split into response blocks (each HTTP response)
    let mut remaining = http_part;
    loop {
        // Find the header/body boundary
        let header_end = if let Some(idx) = remaining.find("\r\n\r\n") {
            idx
        } else if let Some(idx) = remaining.find("\n\n") {
            idx
        } else {
            // No boundary found — treat everything as body
            body = remaining.to_string();
            break;
        };

        let header_block = &remaining[..header_end];
        let after_headers = if remaining[header_end..].starts_with("\r\n\r\n") {
            &remaining[header_end + 4..]
        } else {
            &remaining[header_end + 2..]
        };

        // Parse status line
        let mut lines = header_block.lines();
        let status_line = lines.next().unwrap_or("");
        let parts: Vec<&str> = status_line.splitn(3, ' ').collect();
        let resp_status: u16 = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(0);
        let resp_text = parts.get(2).unwrap_or(&"").to_string();

        // Parse headers
        let mut hdrs: Vec<CurlHeader> = Vec::new();
        let mut location = String::new();
        for line in lines {
            if let Some((name, value)) = line.split_once(':') {
                let name_trimmed = name.trim().to_string();
                let value_trimmed = value.trim().to_string();
                if name_trimmed.to_lowercase() == "location" {
                    location = value_trimmed.clone();
                }
                hdrs.push(CurlHeader {
                    name: name_trimmed,
                    value: value_trimmed,
                });
            }
        }

        // Check if this is a redirect (3xx) and there's more response after
        if (300..400).contains(&resp_status) && !after_headers.is_empty() && after_headers.starts_with("HTTP") {
            redirects.push(CurlRedirect {
                status: resp_status,
                location,
            });
            remaining = after_headers;
            continue;
        }

        // This is the final response
        final_status_text = resp_text;
        final_headers = hdrs;
        body = after_headers.to_string();
        break;
    }

    let body_size = body.len();

    Ok(CurlResult {
        url,
        method: method_str,
        status_code,
        status_text: final_status_text,
        response_headers: final_headers,
        body,
        body_size,
        timings: CurlTimings {
            dns_lookup: time_namelookup,
            tcp_connect: time_connect,
            tls_handshake: time_appconnect,
            ttfb: time_starttransfer,
            total: time_total,
        },
        redirects,
        remote_ip,
        content_type,
        verbose_output: stderr,
    })
}

fn format_curl_time(val: &str) -> String {
    let secs: f64 = val.trim().parse().unwrap_or(0.0);
    if secs == 0.0 {
        return "0ms".to_string();
    }
    let ms = secs * 1000.0;
    if ms < 1.0 {
        format!("{:.2}ms", ms)
    } else if ms < 1000.0 {
        format!("{:.0}ms", ms)
    } else {
        format!("{:.2}s", secs)
    }
}

// ── OpenSSL ──

#[derive(Serialize, Deserialize, Clone)]
pub struct CertChainEntry {
    pub depth: u32,
    pub subject: String,
    pub issuer: String,
    pub key_type: String,
    pub sig_alg: String,
    pub not_before: String,
    pub not_after: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct CertCheckResult {
    pub host: String,
    pub port: u16,
    pub subject: String,
    pub issuer: String,
    pub not_before: String,
    pub not_after: String,
    pub serial: String,
    pub fingerprint: String,
    pub san: Vec<String>,
    pub chain: Vec<CertChainEntry>,
    pub protocol: String,
    pub cipher: String,
    pub key_type: String,
    pub days_remaining: i64,
    pub is_expired: bool,
    pub raw_output: String,
}

#[tauri::command]
pub fn openssl_check_cert(host: String, port: Option<u16>) -> Result<CertCheckResult, String> {
    let p = port.unwrap_or(443);
    let connect = format!("{}:{}", host, p);

    // Get cert details via s_client piped to x509
    let detail_output = shell_cmd(&format!(
            "echo | openssl s_client -connect {} -servername {} 2>/dev/null | openssl x509 -noout -subject -issuer -dates -serial -fingerprint -ext subjectAltName 2>&1",
            connect, host
        ))
        .output()
        .map_err(|e| format!("Failed to run openssl: {}", e))?;

    let details = String::from_utf8_lossy(&detail_output.stdout).to_string();

    // Get chain + connection info via s_client
    let chain_output = shell_cmd(&format!(
            "echo | openssl s_client -connect {} -servername {} 2>&1",
            connect, host
        ))
        .output()
        .map_err(|e| format!("Failed to run openssl s_client: {}", e))?;

    let chain_raw = String::from_utf8_lossy(&chain_output.stdout).to_string();
    let chain_stderr = String::from_utf8_lossy(&chain_output.stderr).to_string();
    let full_output = format!("{}{}", chain_stderr, chain_raw);

    // Parse detail fields
    let mut subject = String::new();
    let mut issuer = String::new();
    let mut not_before = String::new();
    let mut not_after = String::new();
    let mut serial = String::new();
    let mut fingerprint = String::new();
    let mut san: Vec<String> = Vec::new();
    let mut in_san = false;

    for line in details.lines() {
        let trimmed = line.trim();
        if let Some(val) = trimmed.strip_prefix("subject=") {
            subject = val.trim().to_string();
        } else if let Some(val) = trimmed.strip_prefix("issuer=") {
            issuer = val.trim().to_string();
        } else if let Some(val) = trimmed.strip_prefix("notBefore=") {
            not_before = val.trim().to_string();
        } else if let Some(val) = trimmed.strip_prefix("notAfter=") {
            not_after = val.trim().to_string();
        } else if let Some(val) = trimmed.strip_prefix("serial=") {
            serial = val.trim().to_string();
        } else if trimmed.contains("Fingerprint=") {
            fingerprint = trimmed.to_string();
        } else if trimmed.starts_with("X509v3 Subject Alternative Name:") {
            in_san = true;
        } else if in_san {
            for part in trimmed.split(',') {
                let p = part.trim();
                if let Some(dns) = p.strip_prefix("DNS:") {
                    san.push(dns.to_string());
                } else if let Some(ip) = p.strip_prefix("IP Address:") {
                    san.push(ip.to_string());
                }
            }
            in_san = false;
        }
    }

    // Parse chain entries from s_client output
    let mut chain: Vec<CertChainEntry> = Vec::new();
    let mut protocol = String::new();
    let mut cipher = String::new();
    let mut key_type = String::new();

    let lines: Vec<&str> = full_output.lines().collect();
    let mut i = 0;
    while i < lines.len() {
        let trimmed = lines[i].trim();

        // Chain entries: " 0 s:CN=..." format
        if trimmed.starts_with("Certificate chain") {
            i += 1;
            while i < lines.len() {
                let cline = lines[i];
                if cline.starts_with("---") {
                    break;
                }
                // Parse " 0 s:CN=google.com"
                let ct = cline.trim();
                if ct.is_empty() { i += 1; continue; }

                // Depth line with subject
                if ct.chars().next().map_or(false, |c| c.is_ascii_digit()) {
                    let parts: Vec<&str> = ct.splitn(2, ' ').collect();
                    let depth: u32 = parts.first().and_then(|s| s.parse().ok()).unwrap_or(0);
                    let subj = parts.get(1).and_then(|s| s.strip_prefix("s:")).unwrap_or("").to_string();

                    let mut iss = String::new();
                    let mut kt = String::new();
                    let mut sa = String::new();
                    let mut nb = String::new();
                    let mut na = String::new();

                    // Read following indented lines
                    i += 1;
                    while i < lines.len() {
                        let sub = lines[i].trim();
                        if sub.starts_with("i:") {
                            iss = sub.strip_prefix("i:").unwrap_or("").to_string();
                        } else if sub.starts_with("a:") {
                            let aval = sub.strip_prefix("a:").unwrap_or("");
                            if let Some(pkey) = aval.strip_prefix("PKEY: ") {
                                let pparts: Vec<&str> = pkey.splitn(2, ';').collect();
                                kt = pparts.first().unwrap_or(&"").trim().to_string();
                                if let Some(sig) = pparts.get(1) {
                                    sa = sig.trim().strip_prefix("sigalg: ").unwrap_or(sig.trim()).to_string();
                                }
                            }
                        } else if sub.starts_with("v:") {
                            let vval = sub.strip_prefix("v:").unwrap_or("");
                            if let Some(rest) = vval.strip_prefix("NotBefore: ") {
                                let vparts: Vec<&str> = rest.splitn(2, "; NotAfter: ").collect();
                                nb = vparts.first().unwrap_or(&"").trim().to_string();
                                na = vparts.get(1).unwrap_or(&"").trim().to_string();
                            }
                        } else {
                            break;
                        }
                        i += 1;
                    }

                    chain.push(CertChainEntry {
                        depth,
                        subject: subj,
                        issuer: iss,
                        key_type: kt,
                        sig_alg: sa,
                        not_before: nb,
                        not_after: na,
                    });
                    continue;
                }
                i += 1;
            }
            continue;
        }

        // Protocol and cipher
        if let Some(val) = trimmed.strip_prefix("Protocol  :") {
            protocol = val.trim().to_string();
        } else if let Some(val) = trimmed.strip_prefix("Protocol:") {
            protocol = val.trim().to_string();
        }
        if let Some(val) = trimmed.strip_prefix("Cipher    :") {
            cipher = val.trim().to_string();
        } else if let Some(val) = trimmed.strip_prefix("Cipher:") {
            cipher = val.trim().to_string();
        }
        if trimmed.starts_with("Server public key is") {
            key_type = trimmed.replace("Server public key is ", "").to_string();
        }

        i += 1;
    }

    // Calculate days remaining
    let days_remaining = parse_openssl_date_days(&not_after);
    let is_expired = days_remaining < 0;

    Ok(CertCheckResult {
        host,
        port: p,
        subject,
        issuer,
        not_before,
        not_after,
        serial,
        fingerprint,
        san,
        chain,
        protocol,
        cipher,
        key_type,
        days_remaining,
        is_expired,
        raw_output: full_output,
    })
}

fn parse_openssl_date_days(date_str: &str) -> i64 {
    // Parse "Jul 15 21:42:44 2026 GMT" format using pure Rust
    let months = [
        ("Jan", 1), ("Feb", 2), ("Mar", 3), ("Apr", 4), ("May", 5), ("Jun", 6),
        ("Jul", 7), ("Aug", 8), ("Sep", 9), ("Oct", 10), ("Nov", 11), ("Dec", 12),
    ];
    let parts: Vec<&str> = date_str.split_whitespace().collect();
    if parts.len() < 4 {
        return 0;
    }
    let month = match months.iter().find(|(m, _)| *m == parts[0]) {
        Some((_, n)) => *n,
        None => return 0,
    };
    let day: u32 = parts[1].parse().unwrap_or(0);
    let year: i64 = parts[3].parse().unwrap_or(0);
    if day == 0 || year == 0 {
        return 0;
    }
    // Approximate days from epoch using year/month/day
    // (good enough for days-remaining calculation)
    let target_days = year * 365 + (year / 4) - (year / 100) + (year / 400)
        + [0, 31, 59, 90, 120, 151, 181, 212, 243, 273, 304, 334][(month - 1) as usize] as i64
        + day as i64;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    let now_days = now / 86400 + 719163; // days since year 0 (approx)
    target_days - now_days
}

#[derive(Serialize, Deserialize, Clone)]
pub struct CertDecodeResult {
    pub subject: String,
    pub issuer: String,
    pub not_before: String,
    pub not_after: String,
    pub serial: String,
    pub fingerprint: String,
    pub san: Vec<String>,
    pub key_type: String,
    pub sig_alg: String,
    pub version: String,
    pub raw_text: String,
}

#[tauri::command]
pub fn openssl_decode_cert(pem: String) -> Result<CertDecodeResult, String> {
    // Decode PEM cert text
    let output = shell_cmd(&format!(
            "echo '{}' | openssl x509 -noout -text 2>&1",
            pem.replace('\'', "'\\''")
        ))
        .output()
        .map_err(|e| format!("Failed to run openssl x509: {}", e))?;

    let raw_text = String::from_utf8_lossy(&output.stdout).to_string();

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        return Err(format!("Failed to decode certificate: {}{}", stderr, raw_text));
    }

    // Also get structured fields
    let fields_output = shell_cmd(&format!(
            "echo '{}' | openssl x509 -noout -subject -issuer -dates -serial -fingerprint -ext subjectAltName 2>&1",
            pem.replace('\'', "'\\''")
        ))
        .output()
        .map_err(|e| format!("Failed to parse certificate: {}", e))?;

    let fields = String::from_utf8_lossy(&fields_output.stdout).to_string();

    let mut subject = String::new();
    let mut issuer = String::new();
    let mut not_before = String::new();
    let mut not_after = String::new();
    let mut serial = String::new();
    let mut fingerprint = String::new();
    let mut san: Vec<String> = Vec::new();
    let mut in_san = false;

    for line in fields.lines() {
        let trimmed = line.trim();
        if let Some(val) = trimmed.strip_prefix("subject=") {
            subject = val.trim().to_string();
        } else if let Some(val) = trimmed.strip_prefix("issuer=") {
            issuer = val.trim().to_string();
        } else if let Some(val) = trimmed.strip_prefix("notBefore=") {
            not_before = val.trim().to_string();
        } else if let Some(val) = trimmed.strip_prefix("notAfter=") {
            not_after = val.trim().to_string();
        } else if let Some(val) = trimmed.strip_prefix("serial=") {
            serial = val.trim().to_string();
        } else if trimmed.contains("Fingerprint=") {
            fingerprint = trimmed.to_string();
        } else if trimmed.starts_with("X509v3 Subject Alternative Name:") {
            in_san = true;
        } else if in_san {
            for part in trimmed.split(',') {
                let p = part.trim();
                if let Some(dns) = p.strip_prefix("DNS:") {
                    san.push(dns.to_string());
                } else if let Some(ip) = p.strip_prefix("IP Address:") {
                    san.push(ip.to_string());
                }
            }
            in_san = false;
        }
    }

    // Extract key type and sig alg from raw text
    let mut key_type_val = String::new();
    let mut sig_alg_val = String::new();
    let mut version = String::new();

    for line in raw_text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("Public Key Algorithm:") {
            key_type_val = trimmed.replace("Public Key Algorithm:", "").trim().to_string();
        }
        if trimmed.starts_with("Signature Algorithm:") && sig_alg_val.is_empty() {
            sig_alg_val = trimmed.replace("Signature Algorithm:", "").trim().to_string();
        }
        if trimmed.starts_with("Version:") {
            version = trimmed.replace("Version:", "").trim().to_string();
        }
    }

    Ok(CertDecodeResult {
        subject,
        issuer,
        not_before,
        not_after,
        serial,
        fingerprint,
        san,
        key_type: key_type_val,
        sig_alg: sig_alg_val,
        version,
        raw_text,
    })
}

#[derive(Serialize, Deserialize, Clone)]
pub struct HashResult {
    pub input_size: usize,
    pub algorithm: String,
    pub output: String,
}

#[tauri::command]
pub fn openssl_hash(text: String, algorithm: String) -> Result<HashResult, String> {
    let input_size = text.len();

    let cmd_str = match algorithm.as_str() {
        "base64-encode" => format!("echo -n '{}' | openssl base64", text.replace('\'', "'\\''")),
        "base64-decode" => format!("echo -n '{}' | openssl base64 -d 2>&1", text.replace('\'', "'\\''")),
        "sha256" => format!("echo -n '{}' | openssl dgst -sha256 | awk '{{print $2}}'", text.replace('\'', "'\\''")),
        "sha1" => format!("echo -n '{}' | openssl dgst -sha1 | awk '{{print $2}}'", text.replace('\'', "'\\''")),
        "md5" => format!("echo -n '{}' | openssl dgst -md5 | awk '{{print $2}}'", text.replace('\'', "'\\''")),
        "sha512" => format!("echo -n '{}' | openssl dgst -sha512 | awk '{{print $2}}'", text.replace('\'', "'\\''")),
        _ => return Err(format!("Unknown algorithm: {}", algorithm)),
    };

    let output = shell_cmd(&cmd_str)
        .output()
        .map_err(|e| format!("Failed to run openssl: {}", e))?;

    let result = String::from_utf8_lossy(&output.stdout).trim().to_string();

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        return Err(format!("Hash failed: {}", stderr));
    }

    Ok(HashResult {
        input_size,
        algorithm,
        output: result,
    })
}

// ── Whois ──

#[derive(Serialize, Deserialize, Clone)]
pub struct WhoisField {
    pub key: String,
    pub value: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct WhoisResult {
    pub domain: String,
    pub registrar: String,
    pub creation_date: String,
    pub expiry_date: String,
    pub updated_date: String,
    pub nameservers: Vec<String>,
    pub status: Vec<String>,
    pub fields: Vec<WhoisField>,
    pub raw_output: String,
}

#[tauri::command]
pub fn run_whois(domain: String) -> Result<WhoisResult, String> {
    let output = Command::new("whois")
        .arg(&domain)
        .output()
        .map_err(|e| format!("Failed to run whois: {}", e))?;

    let raw = String::from_utf8_lossy(&output.stdout).to_string();

    let mut registrar = String::new();
    let mut creation_date = String::new();
    let mut expiry_date = String::new();
    let mut updated_date = String::new();
    let mut nameservers: Vec<String> = Vec::new();
    let mut status: Vec<String> = Vec::new();
    let mut fields: Vec<WhoisField> = Vec::new();

    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('%') || trimmed.starts_with('#') || trimmed.starts_with(">>>") {
            continue;
        }

        if let Some((key, value)) = trimmed.split_once(':') {
            let k = key.trim().to_lowercase();
            let v = value.trim().to_string();
            if v.is_empty() { continue; }

            match k.as_str() {
                "registrar" | "registrar name" => if registrar.is_empty() { registrar = v.clone(); },
                "creation date" | "created" | "registration date" => if creation_date.is_empty() { creation_date = v.clone(); },
                "registry expiry date" | "registrar registration expiration date" | "expiry date" | "expires" => if expiry_date.is_empty() { expiry_date = v.clone(); },
                "updated date" | "last updated" => if updated_date.is_empty() { updated_date = v.clone(); },
                "name server" | "nserver" => {
                    let ns = v.split_whitespace().next().unwrap_or("").to_lowercase();
                    if !ns.is_empty() && !nameservers.contains(&ns) {
                        nameservers.push(ns);
                    }
                },
                "domain status" | "status" => {
                    let s = v.split_whitespace().next().unwrap_or("").to_string();
                    if !s.is_empty() && !status.contains(&s) {
                        status.push(s);
                    }
                },
                _ => {}
            }

            fields.push(WhoisField {
                key: key.trim().to_string(),
                value: v,
            });
        }
    }

    Ok(WhoisResult {
        domain,
        registrar,
        creation_date,
        expiry_date,
        updated_date,
        nameservers,
        status,
        fields,
        raw_output: raw,
    })
}

// ── Ping ──

#[derive(Serialize, Deserialize, Clone)]
pub struct PingReply {
    pub seq: u32,
    pub ttl: u32,
    pub time_ms: f64,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct PingStats {
    pub transmitted: u32,
    pub received: u32,
    pub loss_percent: f64,
    pub min_ms: f64,
    pub avg_ms: f64,
    pub max_ms: f64,
    pub stddev_ms: f64,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct PingResult {
    pub host: String,
    pub ip: String,
    pub replies: Vec<PingReply>,
    pub stats: PingStats,
}

#[tauri::command]
pub fn run_ping(host: String, count: Option<u32>) -> Result<PingResult, String> {
    let c = count.unwrap_or(5);
    #[cfg(unix)]
    let output = Command::new("ping")
        .arg("-c").arg(c.to_string())
        .arg(&host)
        .output()
        .map_err(|e| format!("Failed to run ping: {}", e))?;
    #[cfg(windows)]
    let output = Command::new("ping")
        .arg("-n").arg(c.to_string())
        .arg(&host)
        .output()
        .map_err(|e| format!("Failed to run ping: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();

    let mut ip = String::new();
    let mut replies: Vec<PingReply> = Vec::new();
    let mut stats = PingStats {
        transmitted: 0, received: 0, loss_percent: 0.0,
        min_ms: 0.0, avg_ms: 0.0, max_ms: 0.0, stddev_ms: 0.0,
    };

    for line in stdout.lines() {
        let trimmed = line.trim();

        // "PING google.com (142.251.116.101): 56 data bytes"
        if trimmed.starts_with("PING ") {
            if let Some(start) = trimmed.find('(') {
                if let Some(end) = trimmed.find(')') {
                    ip = trimmed[start + 1..end].to_string();
                }
            }
        }

        // "64 bytes from 142.251.116.101: icmp_seq=0 ttl=105 time=25.639 ms"
        if trimmed.contains("icmp_seq=") && trimmed.contains("time=") {
            let seq = trimmed.split("icmp_seq=").nth(1)
                .and_then(|s| s.split_whitespace().next())
                .and_then(|s| s.parse::<u32>().ok()).unwrap_or(0);
            let ttl = trimmed.split("ttl=").nth(1)
                .and_then(|s| s.split_whitespace().next())
                .and_then(|s| s.parse::<u32>().ok()).unwrap_or(0);
            let time = trimmed.split("time=").nth(1)
                .and_then(|s| s.split_whitespace().next())
                .and_then(|s| s.parse::<f64>().ok()).unwrap_or(0.0);
            replies.push(PingReply { seq, ttl, time_ms: time });
        }

        // "3 packets transmitted, 3 packets received, 0.0% packet loss"
        if trimmed.contains("packets transmitted") {
            let parts: Vec<&str> = trimmed.split(',').collect();
            if let Some(tx) = parts.first() {
                stats.transmitted = tx.trim().split_whitespace().next()
                    .and_then(|s| s.parse().ok()).unwrap_or(0);
            }
            if let Some(rx) = parts.get(1) {
                stats.received = rx.trim().split_whitespace().next()
                    .and_then(|s| s.parse().ok()).unwrap_or(0);
            }
            if let Some(loss) = parts.get(2) {
                stats.loss_percent = loss.trim().split('%').next()
                    .and_then(|s| s.trim().parse().ok()).unwrap_or(0.0);
            }
        }

        // "round-trip min/avg/max/stddev = 15.082/19.527/25.639/4.468 ms"
        if trimmed.contains("min/avg/max") {
            if let Some(vals) = trimmed.split('=').nth(1) {
                let nums: Vec<f64> = vals.trim().split('/')
                    .filter_map(|s| s.trim().trim_end_matches(" ms").parse().ok())
                    .collect();
                if nums.len() >= 4 {
                    stats.min_ms = nums[0];
                    stats.avg_ms = nums[1];
                    stats.max_ms = nums[2];
                    stats.stddev_ms = nums[3];
                }
            }
        }
    }

    Ok(PingResult { host, ip, replies, stats })
}

// ── Traceroute ──

#[derive(Serialize, Deserialize, Clone)]
pub struct TracerouteHop {
    pub hop: u32,
    pub host: String,
    pub ip: String,
    pub times_ms: Vec<f64>,
    pub is_timeout: bool,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct TracerouteResult {
    pub target: String,
    pub target_ip: String,
    pub max_hops: u32,
    pub hops: Vec<TracerouteHop>,
}

fn parse_traceroute_line(trimmed: &str) -> Option<TracerouteHop> {
    let parts: Vec<&str> = trimmed.split_whitespace().collect();
    if parts.is_empty() { return None; }

    let hop_num: u32 = parts[0].parse().ok()?;

    if parts.len() >= 2 && parts[1] == "*" {
        return Some(TracerouteHop {
            hop: hop_num,
            host: "*".to_string(),
            ip: String::new(),
            times_ms: Vec::new(),
            is_timeout: true,
        });
    }

    let hop_host = parts.get(1).unwrap_or(&"").to_string();
    let hop_ip = parts.get(2)
        .map(|s| s.trim_start_matches('(').trim_end_matches(')').to_string())
        .unwrap_or_default();

    let mut times: Vec<f64> = Vec::new();
    if parts.len() > 3 {
        for p in &parts[3..] {
            if let Ok(t) = p.parse::<f64>() {
                times.push(t);
            }
        }
    }

    Some(TracerouteHop {
        hop: hop_num,
        host: hop_host,
        ip: hop_ip,
        times_ms: times,
        is_timeout: false,
    })
}

#[tauri::command]
pub async fn run_traceroute(host: String, max_hops: Option<u32>) -> Result<TracerouteResult, String> {
    let mh = max_hops.unwrap_or(15);
    let host_clone = host.clone();

    // Run in a blocking thread so we don't block the Tauri main thread
    tokio::task::spawn_blocking(move || {
        #[cfg(unix)]
        let output = Command::new("traceroute")
            .arg("-m").arg(mh.to_string())
            .arg("-w").arg("2")
            .arg("-q").arg("1")
            .arg(&host_clone)
            .output()
            .map_err(|e| format!("Failed to run traceroute: {}", e))?;
        #[cfg(windows)]
        let output = Command::new("tracert")
            .arg("-h").arg(mh.to_string())
            .arg("-w").arg("2000")
            .arg(&host_clone)
            .output()
            .map_err(|e| format!("Failed to run tracert: {}", e))?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let full = format!("{}\n{}", stderr, stdout);

        let mut target_ip = String::new();
        let mut hops: Vec<TracerouteHop> = Vec::new();

        for line in full.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() { continue; }

            if trimmed.starts_with("traceroute to") {
                if let Some(s) = trimmed.find('(') {
                    if let Some(e) = trimmed.find(')') {
                        target_ip = trimmed[s + 1..e].to_string();
                    }
                }
                continue;
            }

            if trimmed.starts_with("traceroute:") { continue; }

            if let Some(hop) = parse_traceroute_line(trimmed) {
                hops.push(hop);
            }
        }

        Ok(TracerouteResult {
            target: host_clone,
            target_ip,
            max_hops: mh,
            hops,
        })
    })
    .await
    .map_err(|e| format!("Task failed: {}", e))?
}

// ── Port Scanner ──

#[derive(Serialize, Deserialize, Clone)]
pub struct PortScanEntry {
    pub port: u16,
    pub service: String,
    pub is_open: bool,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct PortScanResult {
    pub host: String,
    pub ports: Vec<PortScanEntry>,
    pub open_count: usize,
    pub closed_count: usize,
}

fn port_service_name(port: u16) -> &'static str {
    match port {
        21 => "FTP",
        22 => "SSH",
        23 => "Telnet",
        25 => "SMTP",
        53 => "DNS",
        80 => "HTTP",
        110 => "POP3",
        143 => "IMAP",
        443 => "HTTPS",
        465 => "SMTPS",
        587 => "Submission",
        993 => "IMAPS",
        995 => "POP3S",
        3306 => "MySQL",
        3389 => "RDP",
        5432 => "PostgreSQL",
        5900 => "VNC",
        6379 => "Redis",
        8080 => "HTTP Alt",
        8443 => "HTTPS Alt",
        27017 => "MongoDB",
        _ => "",
    }
}

#[tauri::command]
pub fn run_port_scan(host: String, ports: Vec<u16>) -> Result<PortScanResult, String> {
    let mut results: Vec<PortScanEntry> = Vec::new();

    use std::net::{TcpStream, ToSocketAddrs};
    use std::time::Duration;

    let timeout = Duration::from_secs(3);

    for &port in &ports {
        let addr_str = format!("{}:{}", host, port);
        let is_open = addr_str
            .to_socket_addrs()
            .ok()
            .and_then(|mut addrs| addrs.next())
            .map(|addr| TcpStream::connect_timeout(&addr, timeout).is_ok())
            .unwrap_or(false);

        results.push(PortScanEntry {
            port,
            service: port_service_name(port).to_string(),
            is_open,
        });
    }

    let open_count = results.iter().filter(|r| r.is_open).count();
    let closed_count = results.len() - open_count;

    Ok(PortScanResult {
        host,
        ports: results,
        open_count,
        closed_count,
    })
}

