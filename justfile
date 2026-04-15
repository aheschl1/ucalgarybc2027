gen:
    cd schemas && npx buf generate

lint: 
    cd schemas && npx buf lint
    cargo clippy --all-targets

build: gen lint