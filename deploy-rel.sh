pushd ../../esp-hal-app
cargo xtask ota build --input ../console-store/console/ --output ../spoolease-bin/bins/0.5/console/ota
cargo xtask web-install build --input ../console-store/console/ --output ../spoolease-bin/bins/0.5/console/web-install
popd
