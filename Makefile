.PHONY: all setup bridge

all: bridge

setup:
	cargo install flutter_rust_bridge_codegen --version 2.3.0

bridge:
	flutter_rust_bridge_codegen generate --type-64bit-int --rust-root crates/aether_bridge --rust-input crate::api --dart-root apps/aether_app --dart-output apps/aether_app/lib/src/bridge

