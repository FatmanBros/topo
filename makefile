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
	claude-docker .

# VSCode extension (build + package + install)
vsc:
	cd vscode-topo && npm install && npm run compile && npm run package
	code --install-extension vscode-topo/topo-lang-*.vsix
