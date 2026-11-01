use nd_driver::{CommandClass, Commands, DriverError, Result, Risk};

const READS: &[&str] = &[
    "BITCOUNT",
    "BITFIELD_RO",
    "BITPOS",
    "DBSIZE",
    "DUMP",
    "ECHO",
    "EXISTS",
    "EXPIRETIME",
    "GEODIST",
    "GEOHASH",
    "GEOPOS",
    "GEORADIUSBYMEMBER_RO",
    "GEORADIUS_RO",
    "GEOSEARCH",
    "GET",
    "GETBIT",
    "GETRANGE",
    "HEXISTS",
    "HGET",
    "HGETALL",
    "HKEYS",
    "HLEN",
    "HMGET",
    "HRANDFIELD",
    "HSCAN",
    "HSTRLEN",
    "HVALS",
    "HTTL",
    "HPTTL",
    "HEXPIRETIME",
    "INFO",
    "KEYS",
    "LASTSAVE",
    "LCS",
    "LINDEX",
    "LLEN",
    "LOLWUT",
    "LPOS",
    "LRANGE",
    "MGET",
    "PEXPIRETIME",
    "PFCOUNT",
    "PING",
    "PTTL",
    "RANDOMKEY",
    "ROLE",
    "SCAN",
    "SCARD",
    "SDIFF",
    "SINTER",
    "SINTERCARD",
    "SISMEMBER",
    "SMEMBERS",
    "SMISMEMBER",
    "SORT_RO",
    "SRANDMEMBER",
    "SSCAN",
    "STRLEN",
    "SUBSTR",
    "SUNION",
    "TIME",
    "TOUCH",
    "TTL",
    "TYPE",
    "XINFO",
    "XLEN",
    "XPENDING",
    "XRANGE",
    "XREAD",
    "XREVRANGE",
    "ZCARD",
    "ZCOUNT",
    "ZDIFF",
    "ZINTER",
    "ZINTERCARD",
    "ZLEXCOUNT",
    "ZMSCORE",
    "ZRANDMEMBER",
    "ZRANGE",
    "ZRANGEBYLEX",
    "ZRANGEBYSCORE",
    "ZRANK",
    "ZREVRANGE",
    "ZREVRANGEBYLEX",
    "ZREVRANGEBYSCORE",
    "ZREVRANK",
    "ZSCAN",
    "ZSCORE",
    "ZUNION",
    "JSON.GET",
    "JSON.MGET",
    "JSON.TYPE",
    "JSON.STRLEN",
    "JSON.ARRLEN",
    "JSON.ARRINDEX",
    "JSON.OBJKEYS",
    "JSON.OBJLEN",
    "JSON.RESP",
    "JSON.DEBUG",
    "FT.SEARCH",
    "FT.AGGREGATE",
    "FT.INFO",
    "FT._LIST",
    "FT.EXPLAIN",
    "FT.PROFILE",
    "FT.TAGVALS",
    "FT.SPELLCHECK",
    "FT.SYNDUMP",
    "FT.DICTDUMP",
    "TS.GET",
    "TS.MGET",
    "TS.RANGE",
    "TS.REVRANGE",
    "TS.MRANGE",
    "TS.MREVRANGE",
    "TS.INFO",
    "TS.QUERYINDEX",
    "BF.EXISTS",
    "BF.MEXISTS",
    "BF.INFO",
    "CF.EXISTS",
    "CF.COUNT",
    "CF.INFO",
];

const READ_SUBCOMMANDS: &[(&str, &[&str])] = &[
    (
        "CLIENT",
        &[
            "LIST",
            "INFO",
            "ID",
            "GETNAME",
            "GETREDIR",
            "TRACKINGINFO",
            "NO-EVICT",
        ],
    ),
    (
        "COMMAND",
        &[
            "COUNT",
            "DOCS",
            "GETKEYS",
            "GETKEYSANDFLAGS",
            "INFO",
            "LIST",
            "",
        ],
    ),
    ("CONFIG", &["GET"]),
    (
        "MEMORY",
        &["USAGE", "STATS", "DOCTOR", "MALLOC-STATS", "HELP"],
    ),
    (
        "OBJECT",
        &["ENCODING", "FREQ", "IDLETIME", "REFCOUNT", "HELP"],
    ),
    ("SLOWLOG", &["GET", "LEN", "HELP"]),
    (
        "LATENCY",
        &["LATEST", "HISTORY", "DOCTOR", "GRAPH", "HISTOGRAM", "HELP"],
    ),
    (
        "ACL",
        &[
            "WHOAMI", "LIST", "USERS", "CAT", "GETUSER", "LOG", "DRYRUN", "HELP",
        ],
    ),
    ("SCRIPT", &["EXISTS", "HELP"]),
    ("FUNCTION", &["LIST", "DUMP", "STATS", "HELP"]),
    ("MODULE", &["LIST", "HELP"]),
    (
        "CLUSTER",
        &[
            "INFO",
            "NODES",
            "SLOTS",
            "SHARDS",
            "MYID",
            "KEYSLOT",
            "COUNTKEYSINSLOT",
        ],
    ),
    (
        "PUBSUB",
        &[
            "CHANNELS",
            "NUMSUB",
            "NUMPAT",
            "SHARDCHANNELS",
            "SHARDNUMSUB",
        ],
    ),
];

pub const REFUSED: &[&str] = &[
    "SELECT",
    "SUBSCRIBE",
    "PSUBSCRIBE",
    "SSUBSCRIBE",
    "MONITOR",
    "MULTI",
    "EXEC",
    "DISCARD",
    "WATCH",
    "UNWATCH",
    "HELLO",
    "RESET",
    "QUIT",
    "SYNC",
    "PSYNC",
    "REPLCONF",
];

fn risk(name: &str, sub: &str) -> Option<Risk> {
    match (name, sub) {
        ("FLUSHDB" | "FLUSHALL", _) => Some(Risk::Flush),
        ("KEYS", _) => Some(Risk::FullScan),
        (
            "SHUTDOWN" | "DEBUG" | "REPLICAOF" | "SLAVEOF" | "SWAPDB" | "FAILOVER" | "BGREWRITEAOF",
            _,
        )
        | ("CONFIG", "SET" | "REWRITE" | "RESETSTAT")
        | ("CLIENT", "KILL" | "PAUSE")
        | ("SCRIPT" | "FUNCTION", "FLUSH")
        | ("ACL", "SETUSER" | "DELUSER" | "LOAD")
        | ("MODULE", "LOAD" | "LOADEX" | "UNLOAD")
        | ("CLUSTER", "RESET" | "FAILOVER" | "FORGET") => Some(Risk::Server),
        _ => None,
    }
}

pub fn split(text: &str) -> Commands {
    let b = text.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        while i < b.len() && b[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= b.len() {
            break;
        }
        let start = i;
        let comment = b[i] == b'#' || (b[i] == b'/' && b.get(i + 1) == Some(&b'/'));
        let mut quote: Option<u8> = None;
        while i < b.len() {
            let c = b[i];
            match quote {
                Some(q) if c == b'\\' && i + 1 < b.len() && (q == b'"' || b[i + 1] == b'\'') => {
                    i += 1;
                }
                Some(q) if c == q => quote = None,
                None if c == b'\n' => break,
                None if !comment
                    && (c == b'"' || c == b'\'')
                    && (i == start || b[i - 1].is_ascii_whitespace()) =>
                {
                    quote = Some(c);
                }
                Some(_) | None => {}
            }
            i += 1;
        }
        if !comment {
            let end = start + text[start..i].trim_end().len();
            out.push(start..end);
        }
    }
    out
}

fn hex(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    }
}

pub fn parse(line: &str) -> Result<Vec<Vec<u8>>> {
    let b = line.as_bytes();
    let mut args = Vec::new();
    let mut i = 0;
    loop {
        while i < b.len() && b[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= b.len() {
            return Ok(args);
        }
        let mut arg = Vec::new();
        let mut quote: Option<u8> = None;
        loop {
            let Some(&c) = b.get(i) else {
                if quote.is_some() {
                    return Err(DriverError::query("a quote is not closed"));
                }
                break;
            };
            match quote {
                Some(b'"') if c == b'\\' => {
                    let next = b.get(i + 1).copied();
                    match next {
                        Some(b'x') => {
                            let pair = b
                                .get(i + 2)
                                .copied()
                                .and_then(hex)
                                .zip(b.get(i + 3).copied().and_then(hex));
                            if let Some((h, l)) = pair {
                                arg.push(h * 16 + l);
                                i += 3;
                            } else {
                                arg.push(b'x');
                                i += 1;
                            }
                        }
                        Some(e) => {
                            arg.push(match e {
                                b'n' => b'\n',
                                b'r' => b'\r',
                                b't' => b'\t',
                                b'b' => 8,
                                b'a' => 7,
                                other => other,
                            });
                            i += 1;
                        }
                        None => return Err(DriverError::query("a quote is not closed")),
                    }
                }
                Some(b'\'') if c == b'\\' && b.get(i + 1) == Some(&b'\'') => {
                    arg.push(b'\'');
                    i += 1;
                }
                Some(q) if c == q => {
                    if b.get(i + 1).is_some_and(|n| !n.is_ascii_whitespace()) {
                        return Err(DriverError::Query {
                            message: "a closing quote must be followed by a space".into(),
                            code: None,
                            detail: None,
                            hint: None,
                            offset: u32::try_from(i).ok(),
                        });
                    }
                    i += 1;
                    break;
                }
                None if c.is_ascii_whitespace() => break,
                None if (c == b'"' || c == b'\'') && arg.is_empty() => quote = Some(c),
                Some(_) | None => arg.push(c),
            }
            i += 1;
        }
        args.push(arg);
    }
}

pub fn head(line: &str) -> (String, String) {
    let args = parse(line).unwrap_or_default();
    let word = |i: usize| {
        args.get(i)
            .map(|a| String::from_utf8_lossy(a).to_ascii_uppercase())
            .unwrap_or_default()
    };
    (word(0), word(1))
}

pub fn classify(line: &str) -> CommandClass {
    let (name, sub) = head(line);
    let read = READS.contains(&name.as_str())
        || READ_SUBCOMMANDS
            .iter()
            .any(|(n, subs)| *n == name && subs.contains(&sub.as_str()));
    CommandClass {
        write: !read,
        risk: risk(&name, &sub),
    }
}
