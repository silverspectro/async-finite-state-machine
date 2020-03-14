all: build

build:
	cargo +nightly build --release

test:
	cargo +nightly test
