all: build

build:
	cargo build --release

test: gen-mock mockrs
	mockrs serve tests/__fixtures__/db.json --port 3333 &
	cargo test -- --nocapture
	make kill-mock-server

gen-mock: tests/__fixtures__/db.json

delete-mocks:
	rm -rf tests/__fixtures__/db.json

./tests/__fixtures__/db.json:
	mockrs gen tests/__fixtures__/template.json --output tests/__fixtures__/db.json

kill-mock-server:
	killall mockrs

mockrs: ~/.cargo/bin/mockrs 

~/.cargo/bin/mockrs:
	cargo install --git https://github.com/PrivateRookie/mockrs.git

.INTERMEDIATE: kill-mock-server
