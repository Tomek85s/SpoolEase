pushd ../../esp-hal-app
cargo xtask ota build --input ../console-store/console/ --output ../spoolease-bin/bins/0.5/console/ota-unstable
popd
