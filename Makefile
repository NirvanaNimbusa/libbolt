.PHONY: all debug bench test update doc clean

all:
	export RUSTFLAGS=-Awarnings
	cargo +nightly build
	cargo +nightly run

debug:
	export RUST_BACKTRACE=1 
	cargo +nightly build
	cargo +nightly run

release:
	cargo +nightly build --release
	cargo +nightly run --release

bench:
	cargo +nightly bench

test:
	# runs the unit test suite
	cargo +nightly test #-- --nocapture

update:
	# updates local git repos (for forked bn lib)
	cargo +nightly update

doc:
	# generates the documentation
	cargo +nightly doc

clean:
	cargo +nightly clean
