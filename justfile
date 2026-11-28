c:
	cargo check;
	cargo clippy;
t:
	cargo test -- --nocapture
f:
	cargo fmt
run:
	clear;
	cargo run;
clean:
	cargo clean;
	cd hydroxy_rc && flutter clean

install:
	cd hydroxy_daemon && just install;
	cd hydroxy_rc_tui && just install;
	cd hydroxy_rc && just build_release_web;
