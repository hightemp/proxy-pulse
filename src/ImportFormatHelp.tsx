const formats = [
  {
    title: "Automatic protocol detection",
    description: "One proxy per line. Leave out the protocol to use Auto.",
    examples:
      "192.0.2.10:8080\nproxy.example:1080:demo-user:demo-pass\ndemo-user:demo-pass@proxy.example:1080\n[2001:db8::10]:1080",
  },
  {
    title: "Proxy URLs",
    description:
      "Supported schemes: http, https, socks4, socks4a, socks5 and socks5h.",
    examples:
      "http://proxy.example:8080\nhttps://demo-user:demo-pass@proxy.example:8443\nsocks5://demo-user:demo-pass@proxy.example:1080\nsocks5h://demo-user:demo-pass@[2001:db8::10]:1080\nsocks4://192.0.2.10:1080\nsocks4a://demo-user@proxy.example:1080",
  },
  {
    title: "Protocol before or after the address",
    description:
      "Protocol names are case-insensitive. Credentials can follow the port or precede the host.",
    examples:
      "192.0.2.10:64239:demo-user:demo-pass socks5\nproxy.example:1080 socks5\nsocks5 proxy.example:1080:demo-user:demo-pass\nhttps proxy.example:8443\ndemo-user:demo-pass@proxy.example:1080 socks5\n[2001:db8::10]:1080:demo-user:demo-pass socks5",
  },
  {
    title: "Space-separated fields",
    description: "Use host port [username password] [protocol] in this order.",
    examples:
      "proxy.example 1080\nproxy.example 1080 socks5\nproxy.example 1080 demo-user demo-pass socks5",
  },
  {
    title: "CSV and TSV",
    description:
      "Comma, semicolon and tab separators are supported. Use a header or choose a column mapping. Header aliases include ip/server, user/login, pass and type/scheme.",
    examples:
      "host,port,username,password,protocol\nproxy.example,1080,demo-user,demo-pass,socks5\n\nprotocol;host;port;username;password\nsocks5;proxy.example;1080;demo-user;demo-pass",
  },
  {
    title: "Reverse-order records",
    description:
      "Select username:password:host:port in Input format. This order is never guessed automatically.",
    examples: "demo-user:demo-pass:proxy.example:1080",
  },
  {
    title: "Special characters in credentials",
    description:
      "Encode reserved characters in a URL, or put credentials in mapped CSV/TSV columns. The example below contains username demo@user and password p:a%ss.",
    examples: "http://demo%40user:p%3Aa%25ss@proxy.example:8080",
  },
];

export default function ImportFormatHelp() {
  return (
    <>
      <p className="format-intro">
        All addresses and credentials below are examples. TXT lists may mix the
        supported line formats.
      </p>
      <div className="format-guide">
        {formats.map((format) => (
          <section key={format.title}>
            <h3 className="subheading">{format.title}</h3>
            <p>{format.description}</p>
            <pre className="format-examples">{format.examples}</pre>
          </section>
        ))}
      </div>
      <p>
        Ports are required (1–65535). Enclose IPv6 addresses in brackets. SOCKS4
        accepts an optional USERID, without a password. Empty lines and
        whole-line TXT comments starting with <code>#</code> or <code>//</code>{" "}
        are ignored.
      </p>
      <p>
        Import UTF-8 files up to 20 MiB, with up to 100,000 records and 8 KiB
        per record. Review the preview before importing; ambiguous or
        unsupported records show an error instead of silently changing their
        meaning.
      </p>
    </>
  );
}
