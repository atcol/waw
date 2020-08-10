DIR_BUILD     := target
DIR_RELEASE   := ${DIR_BUILD}/release
BIN_NAME      := "!set_this!"
.DEFAULT_GOAL := all

.PHONY: all
all : build test

.PHONY : ubuntu
ubuntu : 
	sudo apt install curl make build-essential libssl-dev x11-dev vflib3 vflib3-dev liblzma-dev liblzma-dev cmake libexpat1-dev


.PHONY: init
init :
	rustup toolchain install nightly
	rustup override set nightly
	rustup component add clippy
	rustup component add rustfmt
	cargo install cargo-watch
	cargo install cargo-edit
	cargo install cargo-tarpaulin
	cargo install cargo-audit
	cargo install cargo-outdated

.PHONY: build
build ${DIR_RELEASE}/${BIN_NAME} :
	cargo build --release

.PHONY: test
test :
	cargo test --verbose && \
	  cargo rustdoc

.PHONY: watch
watch :
	cargo-watch -x "test && cargo rustdoc"
