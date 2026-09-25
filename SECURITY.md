# Security policy

Please report vulnerabilities privately through **Security → Report a
vulnerability** on this repository, not in a public issue.

candlerail runs locally and binds to `127.0.0.1` by default. The web app has no
authentication, so don't expose it on a public interface. It never sends orders
to any exchange.
