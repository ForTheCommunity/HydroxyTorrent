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
