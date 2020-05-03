use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::str::FromStr;

/**
 * service file format:
 *   File:
 *     Line |
 *     Line newline File
 *
 *   Line:
 *     Comment | Empty | Entry
 *
 *   Comment:
 *     # .* newline
 *
 *   Entry:
 *     ws* servicename ws+ port/protocol aliases
 */

fn discard_ws(input: &str, start_idx: usize) -> usize {
    let mut chars = input[start_idx..].chars();
    let mut end_idx = start_idx;

    loop {
        let c = chars.next();
        if c.is_none() || !c.unwrap().is_whitespace() {
            break;
        }

        end_idx += 1;
    }

    end_idx
}

/// A struct representing a line from /etc/services that has a service on it
#[derive(Debug, PartialEq)]
pub struct ServiceEntry {
    pub name: String,
    pub port: usize,
    pub protocol: String,
    pub aliases: Vec<String>,
}

fn is_comment(s: &str) -> bool {
    if let Some(c) = s.chars().next() {
        return c == '#';
    }

    false
}

impl FromStr for ServiceEntry {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut service = s.split_whitespace();

        let name = service.next();
        let name = name.unwrap().to_string();
        if is_comment(&name) {
            return Err("Malformed input");
        }

        let port_and_protocol = service.next();
        if port_and_protocol.is_none() {
            return Err("Could not find port and protocol field");
        }
        let mut port_and_protocol = port_and_protocol.unwrap().split("/");

        let port = port_and_protocol.next().unwrap();
        if is_comment(port) {
            return Err("Could not find port and protocol field");
        }
        let port = port.parse::<usize>();
        if port.is_err() {
            return Err("Malformed port");
        }
        let port = port.unwrap();

        let protocol = port_and_protocol.next();
        if protocol.is_none() {
            return Err("Could not find protocol");
        }
        let protocol = protocol.unwrap().to_string();
        if is_comment(&protocol) {
            return Err("Could not find protocol");
        }

        let mut aliases = Vec::new();
        for alias in service {
            if let Some(c) = alias.chars().next() {
                if c == '#' {
                    break;
                }
            }

            aliases.push(alias.to_string());
        }

        Ok(ServiceEntry {
            name,
            port,
            protocol,
            aliases,
        })
    }
}

/// Parse a file using the format described in `man services(5)`
/// if ignore_errs is true, then all parsing errors will be ignored. This is needed on some systems
/// which don't entirely respect the format in services(5) and omit a service name
pub fn parse_file(path: &Path, ignore_errs: bool) -> Result<Vec<ServiceEntry>, &'static str> {
    if !path.exists() || !path.is_file() {
        return Err("File does not exist or is not a regular file");
    }

    let file = File::open(path);
    if file.is_err() {
        return Err("Could not open file");
    }
    let file = file.unwrap();

    let mut entries = Vec::new();

    let lines = BufReader::new(file).lines();
    for line in lines {
        if let Err(_) = line {
            return Err("Error reading file");
        }
        let line = line.unwrap();

        let start = discard_ws(&line, 0);
        let entryline = &line[start..];
        match entryline.chars().next() {
            Some(c) => {
                if c == '#' {
                    continue;
                }
            }
            // empty line
            None => {
                continue;
            }
        };

        match entryline.parse() {
            Ok(entry) => {
                entries.push(entry);
            }
            Err(msg) => {
                if !ignore_errs {
                    return Err(msg);
                }
            }
        };
    }

    Ok(entries)
}

/// Parse /etc/services
pub fn parse_servicefile(ignore_errs: bool) -> Result<Vec<ServiceEntry>, &'static str> {
    parse_file(&Path::new("/etc/services"), ignore_errs)
}

#[cfg(test)]
mod tests {
    extern crate mktemp;
    use mktemp::Temp;

    use std::io::{Seek, SeekFrom, Write};

    use super::*;

    #[test]
    fn parse_entry() {
        assert_eq!(
            "tcpmux            1/tcp     # TCP Port Service Multiplexer".parse(),
            Ok(ServiceEntry {
                name: "tcpmux".to_string(),
                port: 1,
                protocol: "tcp".to_string(),
                aliases: vec!(),
            })
        );
    }

    #[test]
    fn parse_entry_multiple_aliases() {
        assert_eq!(
            "tcpmux 1/tcp tcpmultiplexer niceservice".parse(),
            Ok(ServiceEntry {
                name: "tcpmux".to_string(),
                port: 1,
                protocol: "tcp".to_string(),
                aliases: vec!("tcpmultiplexer".to_string(), "niceservice".to_string()),
            })
        );
    }

    #[test]
    fn test_parse_file() {
        let temp_file = Temp::new_file().unwrap();
        let temp_path = temp_file.as_path();
        let mut file = File::create(temp_path).unwrap();

        write!(
            file,
            "\
                # WELL KNOWN PORT NUMBERS\n\
                #
                rtmp              1/ddp    #Routing Table Maintenance Protocol\n\
                tcpmux            1/udp     # TCP Port Service Multiplexer\n\
                tcpmux            1/tcp     # TCP Port Service Multiplexer\n\
                #                          Mark Lottor <MKL@nisc.sri.com>\n\
                nbp               2/ddp    #Name Binding Protocol\n\
                compressnet       2/udp     # Management Utility\n\
                compressnet       2/tcp     # Management Utility\n\
                compressnet       3/udp     # Compression Process\n\
                compressnet       3/tcp     # Compression Process\n\
            "
        )
        .expect("Could not write to temp file");
        assert_eq!(
            parse_file(&temp_path, false),
            Ok(vec!(
                ServiceEntry {
                    name: "rtmp".to_string(),
                    port: 1,
                    protocol: "ddp".to_string(),
                    aliases: vec!(),
                },
                ServiceEntry {
                    name: "tcpmux".to_string(),
                    port: 1,
                    protocol: "udp".to_string(),
                    aliases: vec!(),
                },
                ServiceEntry {
                    name: "tcpmux".to_string(),
                    port: 1,
                    protocol: "tcp".to_string(),
                    aliases: vec!(),
                },
                ServiceEntry {
                    name: "nbp".to_string(),
                    port: 2,
                    protocol: "ddp".to_string(),
                    aliases: vec!(),
                },
                ServiceEntry {
                    name: "compressnet".to_string(),
                    port: 2,
                    protocol: "udp".to_string(),
                    aliases: vec!(),
                },
                ServiceEntry {
                    name: "compressnet".to_string(),
                    port: 2,
                    protocol: "tcp".to_string(),
                    aliases: vec!(),
                },
                ServiceEntry {
                    name: "compressnet".to_string(),
                    port: 3,
                    protocol: "udp".to_string(),
                    aliases: vec!(),
                },
                ServiceEntry {
                    name: "compressnet".to_string(),
                    port: 3,
                    protocol: "tcp".to_string(),
                    aliases: vec!(),
                },
            ))
        );
    }

    #[test]
    fn test_parse_file_errors() {
        let temp_file = Temp::new_file().unwrap();
        let temp_path = temp_file.as_path();
        let mut file = File::create(temp_path).unwrap();

        write!(file, "service\n").expect("");
        assert_eq!(
            parse_file(&temp_path, false),
            Err("Could not find port and protocol field")
        );

        file.set_len(0).expect("");
        file.seek(SeekFrom::Start(0)).expect("");
        write!(file, "service # 1/tcp\n").expect("");
        assert_eq!(
            parse_file(&temp_path, false),
            Err("Could not find port and protocol field")
        );

        file.set_len(0).expect("");
        file.seek(SeekFrom::Start(0)).expect("");
        write!(file, "service  1#/tcp\n").expect("");
        assert_eq!(parse_file(&temp_path, false), Err("Malformed port"));

        file.set_len(0).expect("");
        file.seek(SeekFrom::Start(0)).expect("");
        write!(file, "service  1/#tcp\n").expect("");
        assert_eq!(
            parse_file(&temp_path, false),
            Err("Could not find protocol")
        );

        file.set_len(0).expect("");
        file.seek(SeekFrom::Start(0)).expect("");
        write!(file, "service asdf/tcp\n").expect("");
        assert_eq!(parse_file(&temp_path, false), Err("Malformed port"));

        file.set_len(0).expect("");
        file.seek(SeekFrom::Start(0)).expect("");
        write!(file, "service asdf/\n").expect("");
        assert_eq!(parse_file(&temp_path, false), Err("Malformed port"));

        let temp_dir = Temp::new_dir().unwrap();
        let temp_dir_path = temp_dir.as_path();
        assert_eq!(
            parse_file(&temp_dir_path, false),
            Err("File does not exist or is not a regular file")
        );
    }

    #[test]
    fn test_parse_servicefile() {
        assert_eq!(parse_servicefile(true).is_ok(), true);
    }
}
