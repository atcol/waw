DIR_BUILD   := target
DIR_RELEASE := ${DIR_BUILD}/release
TF_DIR := ./deploy

.DEFAULT_GOAL := all 

.PHONY: all
all : build test install

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

.PHONY: build
build ${DIR_RELEASE}/tradingbot :
	cargo build --verbose

.PHONY: test
test : 
	cargo test --verbose

.PHONY: check
check :
	cargo fmt
	cargo clippy

.PHONY: install
install : 
	cargo install --path .

.PHONY: watch
watch :
	cargo-watch -x "test && cargo rustdoc"

.PHONY: tf-setup
tf-setup tf-setup.zip terraform :
	wget https://releases.hashicorp.com/terraform/0.12.28/terraform_0.12.28_linux_amd64.zip -O tf-setup.zip
	touch $@
	unzip -o tf-setup.zip
	chmod +x terraform
	./terraform --version

.PHONY: tf-init
tf-init : terraform
	./terraform init ${TF_DIR}

.PHONY: tf-plan
tf-plan : | tf-init
	./terraform plan ${TF_DIR}

tf-apply : | tf-init
	./terraform apply ${TF_DIR}

tf-destroy : | tf-init 
	./terraform destroy ${TF_DIR}
