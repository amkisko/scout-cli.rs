# Gentoo ebuild for scout-cli
# Copy to a local overlay under app-misc/scout-cli/
# For a full offline build, generate an ebuild with: cargo install cargo-ebuild && cargo ebuild
# (then use the generated ebuild which includes CARGO_CRATE_URIS)

EAPI=8

inherit cargo

DESCRIPTION="ScoutAPM CLI - query apps, endpoints, traces, metrics, and errors"
HOMEPAGE="https://github.com/amkisko/scout-cli.rs"
SRC_URI="https://github.com/amkisko/scout-cli.rs/archive/refs/tags/v${PV}.tar.gz -> ${P}.tar.gz"
S="${WORKDIR}/scout-cli.rs-${PV}"

LICENSE="MIT"
SLOT="0"
KEYWORDS="~amd64 ~arm64"

RDEPEND="dev-libs/openssl:="
DEPEND="${RDEPEND}"

# Build only the scout binary from the workspace
CARGO_INSTALL_PATH=scout

src_install() {
	cargo_src_install
	einstalldocs
	dodoc LICENSE.md
}
