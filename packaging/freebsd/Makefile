# FreeBSD port for scout-cli
# Submit to the Ports tree (e.g. sysutils/scout-cli) or use as a local port.
# Before first build: run "make cargo-crates" in the port directory, then add
# the CARGO_CRATES output to this Makefile or to Makefile.crates (recommended).
# See: https://docs.freebsd.org/en/books/porters-handbook/special/#using-cargo

PORTNAME=	scout-cli
DISTVERSIONPREFIX=	v
DISTVERSION=	0.1.0
CATEGORIES=	sysutils
MAINTAINER=	contact@kiskolabs.com
COMMENT=	ScoutAPM CLI - query apps, endpoints, traces, metrics, and errors
WWW=		https://github.com/amkisko/scout-cli.rs
LICENSE=	MIT
LICENSE_FILE=	${WRKSRC}/LICENSE.md

USES=		cargo
USE_GITHUB=	yes
GH_ACCOUNT=	amkisko
GH_PROJECT=	scout-cli.rs

# Build and install only the scout binary from the workspace
CARGO_INSTALL_PATH=	scout

# Optional: place CARGO_CRATES in Makefile.crates if the list is long
# .include "${.CURDIR}/Makefile.crates"
.include <bsd.port.mk>
