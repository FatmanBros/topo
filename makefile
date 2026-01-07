i:
	npm install
	cargo install --path .

dev:
	cargo run --bin topo -- dev

test:
	cargo test

claude:
	claude --dangerously-skip-permissions
