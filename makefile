i:
	npm install
	cargo install --path .

dev:
	cargo run --bin topo -- dev

test:
	npm test

e2e:
	cargo run --bin topo -- test

claude:
	claude --dangerously-skip-permissions
